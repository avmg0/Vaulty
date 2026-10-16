//! OS biometric prompt + keychain storage for the per-user unlock token.
//!
//! How biometric unlock works:
//! 1. While signed in, the user turns it on (re-entering their password). Core issues a random
//!    256-bit device token and stores the member's data key wrapped (encrypted) under it.
//! 2. The token itself goes into the OS keychain under this user's id.
//! 3. To unlock: show the OS prompt; on success read the token from the keychain and hand it to
//!    core, which uses it to unwrap the data key. Without the token the wrapped key is useless.
//!
//! The keychain item is readable by Vaulty without a prompt, so the prompt is enforced by the app
//! rather than by the OS. Binding the item to biometrics at the OS level (macOS/iOS access control,
//! Android Keystore) is planned alongside database encryption; see ROADMAP.md.

const SERVICE: &str = "com.vaulty.app";

fn account(user_id: &str) -> String {
    format!("biometric-token:{user_id}")
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod imp {
    use super::*;
    use robius_authentication::{AndroidText, BiometricStrength, Context, PolicyBuilder, Text, WindowsText};
    use std::sync::Mutex;

    pub fn available() -> bool {
        // There is no portable "is hardware present" query; we know a prompt can be shown
        // on macOS (Touch ID / password), Windows (Windows Hello) and Linux (polkit agent).
        cfg!(any(target_os = "macos", target_os = "windows", target_os = "linux"))
    }

    pub async fn prompt(reason: &'static str) -> Result<(), String> {
        let policy = PolicyBuilder::new()
            .biometrics(Some(BiometricStrength::Strong))
            .password(true) // device PIN/password fallback (e.g. Windows Hello PIN)
            .companion(true) // Apple Watch on macOS
            .action_ids(["com.vaulty.app.unlock"]) // Linux polkit action (packaging/com.vaulty.app.policy)
            .build()
            .ok_or("biometric policy is not supported on this platform")?;
        let text = Text {
            android: AndroidText { title: "Unlock Vaulty", subtitle: None, description: Some(reason) },
            apple: reason,
            windows: WindowsText::new_truncated("Unlock Vaulty", reason),
        };
        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        let tx = Mutex::new(Some(tx));
        Context::new(())
            .authenticate(text, &policy, move |res| {
                if let Some(tx) = tx.lock().ok().and_then(|mut t| t.take()) {
                    let _ = tx.send(res.is_ok());
                }
            })
            .map_err(|e| format!("couldn't show the unlock prompt: {e:?}"))?;
        match rx.await {
            Ok(true) => Ok(()),
            _ => Err("biometric check was cancelled or failed".into()),
        }
    }

    pub fn store_token(user_id: &str, token: &str) -> Result<(), String> {
        keyring::Entry::new(SERVICE, &account(user_id))
            .and_then(|e| e.set_password(token))
            .map_err(|e| format!("couldn't save to the system keychain: {e}"))
    }

    pub fn read_token(user_id: &str) -> Result<String, String> {
        keyring::Entry::new(SERVICE, &account(user_id))
            .and_then(|e| e.get_password())
            .map_err(|_| "biometric unlock isn't set up on this device; sign in with your password".into())
    }

    pub fn delete_token(user_id: &str) {
        if let Ok(e) = keyring::Entry::new(SERVICE, &account(user_id)) {
            let _ = e.delete_credential();
        }
    }
}

/// Mobile: wired up in the mobile phase (tauri-plugin-biometric + Keystore/Keychain).
#[cfg(any(target_os = "android", target_os = "ios"))]
mod imp {
    pub fn available() -> bool { false }
    pub async fn prompt(_: &'static str) -> Result<(), String> { Err("not available on mobile yet".into()) }
    pub fn store_token(_: &str, _: &str) -> Result<(), String> { Err("not available on mobile yet".into()) }
    pub fn read_token(_: &str) -> Result<String, String> { Err("not available on mobile yet".into()) }
    pub fn delete_token(_: &str) {}
}

pub use imp::*;

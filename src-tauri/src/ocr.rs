//! Reading photos of bills with the text recognition the system already has, so nothing is
//! downloaded and nothing leaves the computer: Apple's Vision on macOS, Windows' own OCR on
//! Windows, and Tesseract (if installed) on Linux. Each gives pieces of text with where they are
//! on the page; `vaulty_core::ocr` lays them out into lines.
use vaulty_core::{ocr::Piece, Error, Result};

/// The largest photo sent for recognition.
pub const MAX_IMAGE: usize = 25 * 1024 * 1024;

/// Which recogniser this computer has, for the window to explain ("" when there's none).
pub fn engine() -> &'static str {
    if cfg!(target_os = "macos") { "apple" }
    else if cfg!(windows) { "windows" }
    else if tesseract_path().is_some() { "tesseract" }
    else { "" }
}

pub fn recognize(bytes: &[u8], _mime: &str) -> Result<Vec<Piece>> {
    if bytes.len() > MAX_IMAGE { return Err(Error::Invalid("that photo is too large to read".into())); }
    imp::recognize(bytes)
}

fn tesseract_path() -> Option<std::path::PathBuf> {
    ["/usr/bin/tesseract", "/usr/local/bin/tesseract", "/opt/homebrew/bin/tesseract", "/snap/bin/tesseract"]
        .iter().map(std::path::PathBuf::from).find(|p| p.is_file())
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use objc2::{rc::Retained, AllocAnyThread};
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
    use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};

    pub fn recognize(bytes: &[u8]) -> Result<Vec<Piece>> {
        objc2::rc::autoreleasepool(|_| {
            let data = NSData::with_bytes(bytes);
            let options = NSDictionary::<NSString, objc2::runtime::AnyObject>::new();
            let handler = VNImageRequestHandler::initWithData_options(VNImageRequestHandler::alloc(), &data, &options);
            let request: Retained<VNRecognizeTextRequest> = VNRecognizeTextRequest::new();
            request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
            request.setUsesLanguageCorrection(true);
            let as_request: &VNRequest = &request;
            let requests = NSArray::from_slice(&[as_request]);
            handler.performRequests_error(&requests).map_err(|e| Error::Invalid(format!("the photo couldn't be read ({})", e.localizedDescription())))?;
            let mut out = vec![];
            if let Some(results) = request.results() {
                for obs in results.iter() {
                    let Some(best) = obs.topCandidates(1).firstObject() else { continue };
                    // Vision measures from the bottom-left corner; pieces are measured from the top-left.
                    let r = unsafe { obs.boundingBox() };
                    out.push(Piece { text: best.string().to_string(), x: r.origin.x, y: 1.0 - r.origin.y - r.size.height, w: r.size.width, h: r.size.height });
                }
            }
            Ok(out)
        })
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::{
        Graphics::Imaging::BitmapDecoder,
        Media::Ocr::OcrEngine,
        Security::Cryptography::CryptographicBuffer,
        Storage::Streams::InMemoryRandomAccessStream,
        Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
    };

    fn fail(e: windows::core::Error) -> Error { Error::Invalid(format!("the photo couldn't be read ({})", e.message())) }

    pub fn recognize(bytes: &[u8]) -> Result<Vec<Piece>> {
        let bytes = bytes.to_vec();
        // Windows' recognition wants a thread of its own (not the window's).
        std::thread::spawn(move || -> Result<Vec<Piece>> {
            unsafe { let _ = RoInitialize(RO_INIT_MULTITHREADED); }
            let stream = InMemoryRandomAccessStream::new().map_err(fail)?;
            let buffer = CryptographicBuffer::CreateFromByteArray(&bytes).map_err(fail)?;
            stream.WriteAsync(&buffer).map_err(fail)?.join().map_err(fail)?;
            stream.Seek(0).map_err(fail)?;
            let decoder = BitmapDecoder::CreateAsync(&stream).map_err(fail)?.join().map_err(fail)?;
            let (w, h) = (decoder.PixelWidth().map_err(fail)? as f64, decoder.PixelHeight().map_err(fail)? as f64);
            let max = OcrEngine::MaxImageDimension().map_err(fail)? as f64;
            if w > max || h > max { return Err(Error::Invalid("that photo is too large to read; try a smaller one".into())); }
            let bitmap = decoder.GetSoftwareBitmapAsync().map_err(fail)?.join().map_err(fail)?;
            let engine = OcrEngine::TryCreateFromUserProfileLanguages()
                .map_err(|_| Error::Invalid("Windows has no text recognition for your language; add one in Settings → Time & language".into()))?;
            let result = engine.RecognizeAsync(&bitmap).map_err(fail)?.join().map_err(fail)?;
            let mut out = vec![];
            for line in result.Lines().map_err(fail)? {
                for word in line.Words().map_err(fail)? {
                    let r = word.BoundingRect().map_err(fail)?;
                    out.push(Piece { text: word.Text().map_err(fail)?.to_string(), x: r.X as f64 / w, y: r.Y as f64 / h, w: r.Width as f64 / w, h: r.Height as f64 / h });
                }
            }
            Ok(out)
        }).join().map_err(|_| Error::Invalid("the photo couldn't be read".into()))?
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod imp {
    use super::*;
    use std::io::Write;
    use std::process::{Command, Stdio};

    pub fn recognize(bytes: &[u8]) -> Result<Vec<Piece>> {
        let Some(path) = tesseract_path() else {
            return Err(Error::Invalid("to read photos of bills, install Tesseract (for example: sudo apt install tesseract-ocr)".into()));
        };
        let mut child = Command::new(path).args(["stdin", "stdout", "--psm", "6", "tsv"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
            .map_err(|_| Error::Invalid("the photo couldn't be read".into()))?;
        if let Some(mut stdin) = child.stdin.take() { stdin.write_all(bytes).map_err(|_| Error::Invalid("the photo couldn't be read".into()))?; }
        let out = child.wait_with_output().map_err(|_| Error::Invalid("the photo couldn't be read".into()))?;
        if !out.status.success() { return Err(Error::Invalid("the photo couldn't be read".into())); }
        Ok(vaulty_core::ocr::from_tesseract_tsv(&String::from_utf8_lossy(&out.stdout)))
    }
}

# Vaulty backup files (`.vaulty`)

A `.vaulty` file is a password-protected backup of one profile. It uses standard, well-reviewed
building blocks, so it can be opened without Vaulty: the script below needs only Python and two
common libraries.

## Layout

The file is JSON:

```json
{
  "format": "vaulty-backup",
  "version": 1,
  "created_at": "2026-10-19T08:00:00Z",
  "hint": "optional reminder you chose, stored unencrypted",
  "kdf": { "name": "argon2id", "version": 19, "memory_kib": 65536, "iterations": 3, "parallelism": 1, "salt": "<hex>" },
  "cipher": "xchacha20-poly1305",
  "nonce": "<24 bytes, hex>",
  "data": "<ciphertext with 16-byte tag, hex>"
}
```

- **Key:** Argon2id (version 0x13) of the backup password with the salt and the settings in `kdf`,
  32 bytes long.
- **Encryption:** XChaCha20-Poly1305 with that key and `nonce`.
- **Associated data:** these fields joined with `|`, in this order:
  `format, version, created_at, hint, kdf.name, kdf.version, kdf.memory_kib, kdf.iterations, kdf.parallelism, kdf.salt, cipher, nonce`.
  A missing hint is an empty string; numbers are written in decimal. Changing any of them makes the file fail to open.
- **Plaintext:** Vaulty's normal JSON backup (`"version": 2`): every table of the profile (accounts,
  categories, ledger entries and postings, cards, recurring payments and more), with money in
  integer minor units (paise, cents).
- **Document files:** the plaintext may also have `"document_files"`: an object from each file's id
  (as in the `vault_document_files` table) to its contents in standard base64. Files are left out
  when they add up to more than 60 MB; their details are still in the tables. Plain (unencrypted)
  JSON exports never contain document files.

Only the hint and the date are readable without the password. Choose a hint that means nothing to anyone else.

## Opening a backup without Vaulty

```python
# pip install argon2-cffi cryptography
import json, sys, getpass
from argon2.low_level import hash_secret_raw, Type
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms

def hchacha20(key, nonce16):
    # XChaCha20 subkey: HChaCha20 over the first 16 bytes of the nonce.
    import struct
    c = list(struct.unpack("<4I", b"expand 32-byte k")) + list(struct.unpack("<8I", key)) + list(struct.unpack("<4I", nonce16))
    def rotl(v, n): return ((v << n) & 0xffffffff) | (v >> (32 - n))
    def qr(a, b, cc, d):
        c[a] = (c[a] + c[b]) & 0xffffffff; c[d] = rotl(c[d] ^ c[a], 16)
        c[cc] = (c[cc] + c[d]) & 0xffffffff; c[b] = rotl(c[b] ^ c[cc], 12)
        c[a] = (c[a] + c[b]) & 0xffffffff; c[d] = rotl(c[d] ^ c[a], 8)
        c[cc] = (c[cc] + c[d]) & 0xffffffff; c[b] = rotl(c[b] ^ c[cc], 7)
    for _ in range(10):
        qr(0, 4, 8, 12); qr(1, 5, 9, 13); qr(2, 6, 10, 14); qr(3, 7, 11, 15)
        qr(0, 5, 10, 15); qr(1, 6, 11, 12); qr(2, 7, 8, 13); qr(3, 4, 9, 14)
    return struct.pack("<8I", *(c[0:4] + c[12:16]))

f = json.load(open(sys.argv[1]))
k = f["kdf"]
password = getpass.getpass("Backup password: ").encode()
key = hash_secret_raw(password, bytes.fromhex(k["salt"]), time_cost=k["iterations"], memory_cost=k["memory_kib"],
                      parallelism=k["parallelism"], hash_len=32, type=Type.ID, version=k["version"])
aad = "|".join(str(x) for x in [f["format"], f["version"], f["created_at"], f.get("hint") or "", k["name"], k["version"],
                                k["memory_kib"], k["iterations"], k["parallelism"], k["salt"], f["cipher"], f["nonce"]]).encode()
nonce = bytes.fromhex(f["nonce"])
plain = ChaCha20Poly1305(hchacha20(key, nonce[:16])).decrypt(b"\0\0\0\0" + nonce[16:], bytes.fromhex(f["data"]), aad)
json.dump(json.loads(plain), sys.stdout, indent=2)
```

Run it as `python3 open_vaulty_backup.py my-backup.vaulty > backup.json`.

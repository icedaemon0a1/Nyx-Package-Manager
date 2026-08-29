//! Fast content-identity hashing (BLAKE3).
//!
//! Used as the fast fingerprint for the future security scan cache and for
//! package cache/transaction integrity. Cryptographic package-signature
//! verification (SHA-256/OpenPGP) is a separate concern handled elsewhere
//! (Phase 3) — this module is only the fast content-identity hash.

use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Blake3Hash(pub [u8; 32]);

impl Blake3Hash {
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            use std::fmt::Write;
            let _ = write!(s, "{b:02x}");
        }
        s
    }

    /// Sharded CAS path component, e.g. `a1/92/a192....meta`, matching the
    /// scan-cache layout described in the brief.
    pub fn shard_path(&self) -> (String, String, String) {
        let hex = self.to_hex();
        (hex[0..2].to_string(), hex[2..4].to_string(), hex)
    }
}

pub fn hash_bytes(data: &[u8]) -> Blake3Hash {
    Blake3Hash(*blake3::hash(data).as_bytes())
}

/// Stream-hash a file without loading it fully into memory, bounding peak
/// memory independently of file size (per streaming-scan requirement).
pub fn hash_file(path: &Path) -> std::io::Result<Blake3Hash> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(Blake3Hash(*hasher.finalize().as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip_and_shard() {
        let h = hash_bytes(b"hello nyx");
        let hex = h.to_hex();
        assert_eq!(hex.len(), 64);
        let (a, b, full) = h.shard_path();
        assert_eq!(format!("{a}{b}"), &hex[..4]);
        assert_eq!(full, hex);
    }

    #[test]
    fn file_hash_matches_bytes_hash() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"identical content").unwrap();
        let a = hash_bytes(b"identical content");
        let b = hash_file(tmp.path()).unwrap();
        assert_eq!(a, b);
    }
}

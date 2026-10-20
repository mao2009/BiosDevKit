//! Streams bytes to verify SHA-256 and byte length against a manifest.
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::{self, Read};

#[derive(Debug, Clone)]
pub struct Artifact {
    pub logical_name: String,
    pub sha256: String,
    pub byte_length: u64,
}
pub fn verify<R: Read>(
    manifest: &[Artifact],
    mut streams: HashMap<String, R>,
) -> Result<(), String> {
    let mut seen = HashSet::new();
    for entry in manifest {
        if entry.logical_name.is_empty() || !seen.insert(&entry.logical_name) {
            return Err(format!("duplicate or empty name: {}", entry.logical_name));
        }
        if entry.sha256.len() != 64 || !entry.sha256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) {
            return Err("invalid SHA-256 digest".into());
        }
        let mut reader = streams.remove(&entry.logical_name).ok_or_else(|| format!("missing stream: {}", entry.logical_name))?;
        let mut digest = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0; 65536];
        loop {
            let n = reader.read(&mut buffer).map_err(|e: io::Error| e.to_string())?;
            if n == 0 { break; }
            size = size.checked_add(n as u64).ok_or("size overflow")?;
            digest.update(&buffer[..n]);
        }
        if size != entry.byte_length || format!("{:x}", digest.finalize()) != entry.sha256 {
            return Err(format!("byte length or SHA-256 mismatch: {}", entry.logical_name));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn entry() -> Artifact { Artifact { logical_name: "empty".into(), byte_length: 0, sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into() } }
    #[test]
    fn verifies_empty_and_rejects_corruption() {
        let streams = HashMap::from([("empty".into(), Cursor::new(Vec::<u8>::new()))]);
        assert!(verify(&[entry()], streams).is_ok());
        let streams = HashMap::from([("empty".into(), Cursor::new(b"changed".to_vec()))]);
        assert!(verify(&[entry()], streams).is_err());
    }
    #[test]
    fn rejects_duplicate_and_missing() {
        assert!(verify::<Cursor<Vec<u8>>>(&[entry(), entry()], HashMap::from([("empty".into(), Cursor::new(vec![]))])).is_err());
        assert!(verify::<Cursor<Vec<u8>>>(&[entry()], HashMap::new()).is_err());
    }
}

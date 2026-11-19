//! Exact source occurrence identity used by an entry contract's published row.
//! Computing this digest grants no package authority or declaration acceptance.
pub fn entry_contract_digest(manifest: &str, relative_source: &str, source: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    for bytes in [
        b"omega.program-entry-physical-contract-package-source.v1".as_slice(),
        manifest.as_bytes(),
        relative_source.as_bytes(),
        source,
    ] {
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    digest.finalize().into()
}

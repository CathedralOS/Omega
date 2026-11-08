/*
We define stable package-source lineages (`SourceLineage`), immutable source
resolutions (`ImmutableSourceResolution`), and domain-separated SHA-256 identity
digests.

`domain_digest!` generates 32-byte digest wrappers that expose borrowed digest
bytes directly without formatting scratch. `decode_hex` validates length and
ASCII hex characters up front and allocates the exact decoded byte count.
*/

/*
Package lockfiles and review records must bind a package to a canonical source
lineage regardless of whether the operator wrote an HTTPS or SSH Git locator,
while keeping workspace members and host-local external paths distinct.
Separating lineage normalization (`git`, `local`, `locator`) from content
resolution (`resolution`) ensures lock verification and cache keys never
conflate transport syntax with authenticated source content.
*/

/*
- Git and known-host lineage normalization: `git`, `locator`
- Workspace member and external local lineages: `local`
- Authenticated commit/tree and content resolutions: `resolution`
*/

use sha2::{Digest, Sha256};

macro_rules! domain_digest {
    ($name:ident, $domain:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn derive(canonical_evidence: &[u8]) -> Self {
                let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(b"");
                hasher.bytes($domain);
                hasher.bytes(canonical_evidence);
                Self(hasher.finish())
            }

            pub fn parse_hex(value: &str) -> Result<Self, IdentityError> {
                let bytes = decode_hex(value).ok_or(IdentityError::InvalidDigest)?;
                let bytes = bytes.try_into().map_err(|_| IdentityError::InvalidDigest)?;
                Ok(Self(bytes))
            }

            pub fn to_hex(&self) -> String {
                encode_hex(&self.0)
            }

            pub const fn digest(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

pub(crate) mod digest;
mod error;
mod git;
mod local;
mod locator;
mod resolution;

pub use error::IdentityError;
pub use git::{GitTransport, SourceLineage};
pub use local::{
    ExternalLocalLineage, ExternalSourceContext, SourceContentDigest, SourceRelativePath,
    WorkspaceLineageIdentity, WorkspaceMemberLineage,
};
pub use resolution::{GitCommitId, GitObjectIdAlgorithm, GitTreeId, ImmutableSourceResolution};

pub(super) fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

pub(super) fn hash_optional_field(hasher: &mut Sha256, bytes: Option<&[u8]>) {
    match bytes {
        Some(bytes) => {
            hasher.update([1]);
            hash_field(hasher, bytes);
        }
        None => hasher.update([0]),
    }
}

pub(super) fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut decoded = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().as_chunks::<2>().0 {
        let high = hex_value(pair[0]).expect("prevalidated hexadecimal");
        let low = hex_value(pair[1]).expect("prevalidated hexadecimal");
        decoded.push((high << 4) | low);
    }
    Some(decoded)
}

pub(crate) fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(super) fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests;

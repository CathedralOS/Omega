//! Stable package names, aliases, source lineages, and immutable resolutions.

use sha2::{Digest, Sha256};

macro_rules! domain_digest {
    ($name:ident, $domain:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn derive(canonical_evidence: &[u8]) -> Self {
                let mut hasher = Sha256::new();
                hash_field(&mut hasher, $domain);
                hash_field(&mut hasher, canonical_evidence);
                Self(hasher.finalize().into())
            }

            pub fn parse_hex(value: &str) -> Result<Self, IdentityError> {
                let bytes = decode_hex(value).ok_or(IdentityError::InvalidDigest)?;
                let bytes = bytes.try_into().map_err(|_| IdentityError::InvalidDigest)?;
                Ok(Self(bytes))
            }

            pub fn to_hex(&self) -> String {
                encode_hex(&self.0)
            }
        }
    };
}

mod error;
mod git;
mod local;
mod locator;
mod names;
mod resolution;

pub use error::IdentityError;
pub(crate) use git::GitRequestedNetworkEndpoint;
pub use git::{
    GenericGitLineage, GitHubRepositoryLineage, GitLabRepositoryLineage, GitTransport,
    SourceLineage,
};
pub use local::{
    ExternalLocalLineage, ExternalSourceContext, SourceContentDigest, WorkspaceLineageIdentity,
    WorkspaceMemberLineage, WorkspaceMemberPath,
};
pub use names::{AliasName, PackageKey, PackageName};
pub use resolution::{GitCommitId, GitObjectIdAlgorithm, GitTreeId, ImmutableSourceResolution};

pub(super) fn is_snake_case(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && !value.ends_with('_')
        && !value.contains("__")
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

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
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_value(pair[0])?;
            let low = hex_value(pair[1])?;
            Some((high << 4) | low)
        })
        .collect()
}

fn hex_value(byte: u8) -> Option<u8> {
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

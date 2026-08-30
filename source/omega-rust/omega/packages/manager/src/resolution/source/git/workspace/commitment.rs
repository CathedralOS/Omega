use sha2::{Digest, Sha256};

const BUILD_DECLARATION_COMMITMENT_DOMAIN: &[u8] = b"omega-git-workspace-build-declaration-v1";

/// SHA-256 commitment to the exact authenticated bytes of one `build.omg`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildDeclarationCommitment([u8; 32]);

impl BuildDeclarationCommitment {
    pub fn derive(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(
            u64::try_from(BUILD_DECLARATION_COMMITMENT_DOMAIN.len())
                .expect("fixed domain length fits u64")
                .to_be_bytes(),
        );
        hasher.update(BUILD_DECLARATION_COMMITMENT_DOMAIN);
        hasher.update(
            u64::try_from(bytes.len())
                .expect("host slice length fits u64")
                .to_be_bytes(),
        );
        hasher.update(bytes);
        Self(hasher.finalize().into())
    }

    pub fn matches(&self, bytes: &[u8]) -> bool {
        self == &Self::derive(bytes)
    }

    pub fn to_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut encoded = String::with_capacity(self.0.len() * 2);
        for byte in self.0 {
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
        encoded
    }
}

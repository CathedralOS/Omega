use super::super::encoding::encode_hex;

/// Domain-separated identity of one complete canonical source-closure question.
///
/// This identifies the question only. It is not source authenticity, package
/// admission, a compiler result, or a package instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalSourceClosureSubjectFingerprint(pub(in super::super) [u8; 32]);

impl CanonicalSourceClosureSubjectFingerprint {
    pub fn to_hex(&self) -> String {
        encode_hex(&self.0)
    }
}

use crate::VocabularyMarker;

/// Canonical semantic-module digest bytes.
///
/// `terminal-codec` is the authority that computes this value from
/// canonical bytes. The representation lives here so verified consumers can
/// retain exact program identity without depending upward on the codec crate.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticFingerprint([u8; 32]);

impl SemanticFingerprint {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for SemanticFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for SemanticFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalPsiIdentity {
    pub vocabulary_marker: VocabularyMarker,
    pub program_fingerprint: SemanticFingerprint,
}

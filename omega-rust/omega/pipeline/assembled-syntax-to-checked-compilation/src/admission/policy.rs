//! Whether a compilation also writes its auxiliary reports.

/// Controls auxiliary compiler reports independently from executable/object
/// installation. Semantic validation, trust-lock enforcement, and requested
/// output installation run under both policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactEmissionPolicy {
    Full,
    OutputOnly,
}

impl ArtifactEmissionPolicy {
    pub const fn emits_auxiliary_artifacts(self) -> bool {
        matches!(self, Self::Full)
    }
}

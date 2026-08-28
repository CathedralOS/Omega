use crate::compiler::{ArtifactEmissionPolicy, CompileOptions};

/// Explicitly test-only adapter onto the production compiler route.
///
/// This type remains only while integration tests move to [`super::CompileRequest`].
/// It owns no alternate stages, entry override, worker policy, or output path.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileHarnessRequest {
    pub(super) options: CompileOptions,
    pub(super) artifact_policy: ArtifactEmissionPolicy,
}

#[doc(hidden)]
impl CompileHarnessRequest {
    pub fn new(options: CompileOptions) -> Self {
        Self {
            options,
            artifact_policy: ArtifactEmissionPolicy::Full,
        }
    }

    pub fn with_artifact_policy(mut self, artifact_policy: ArtifactEmissionPolicy) -> Self {
        self.artifact_policy = artifact_policy;
        self
    }
}

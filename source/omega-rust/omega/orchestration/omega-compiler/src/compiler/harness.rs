use crate::compiler::{ArtifactEmissionPolicy, CompileOptions};

/// Explicitly test-only compiler controls. Entry overrides and worker ceilings
/// cannot enter the production [`super::CompileRequest`].
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileHarnessRequest {
    pub(super) options: CompileOptions,
    pub(super) entry_machine_name: Option<String>,
    pub(super) worker_count: Option<usize>,
    pub(super) artifact_policy: ArtifactEmissionPolicy,
}

#[doc(hidden)]
impl CompileHarnessRequest {
    pub fn new(options: CompileOptions) -> Self {
        Self {
            options,
            entry_machine_name: None,
            worker_count: None,
            artifact_policy: ArtifactEmissionPolicy::Full,
        }
    }

    pub fn with_test_entry(mut self, entry_machine_name: impl Into<String>) -> Self {
        self.entry_machine_name = Some(entry_machine_name.into());
        self
    }

    pub fn with_worker_count(mut self, worker_count: usize) -> Self {
        self.worker_count = Some(worker_count.max(1));
        self
    }

    pub fn with_artifact_policy(mut self, artifact_policy: ArtifactEmissionPolicy) -> Self {
        self.artifact_policy = artifact_policy;
        self
    }
}

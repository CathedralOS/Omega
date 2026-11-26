// Pre-execution inputs (`BuildExecutionInputs`) and contract projection
// (`BuildExecutionContract`) supplied before a `Build` activation starts.
// Carries the execution target profile, writable staging directory, filesystem
// and evaluation sponsors, `BuildSnapshotRequest`, and optional restricted
// build-grant consent guard.

use super::{BuildSnapshotRequest, RestrictedBuildGrants};
use crate::PackageCompilationInputs;
use std::path::PathBuf;
use target::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildExecutionContract {
    /// The caller may designate this activation's retained diagnostics as a bounded StandardOutput.
    pub diagnostic_output_bytes: Option<u64>,
    pub profile: Option<TargetProfile>,
    pub snapshot: Option<BuildSnapshotRequest>,
}

#[derive(Default)]
pub struct BuildExecutionInputs {
    /// The caller may designate this activation's retained diagnostics as a bounded StandardOutput.
    pub diagnostic_output_bytes: Option<u64>,
    pub interpreter_providers: crate::InterpretedProviderSelections,
    pub profile: Option<TargetProfile>, // Compilation requests default to the host, independently of Product target.
    pub directory: Option<PathBuf>, // Writable staging, separate from immutable source snapshots.
    pub filesystem_sponsor: Option<filesystem_authority::FilesystemSponsor>,
    pub evaluation_sponsor: Option<evaluation_accounting::BuildEvaluationSponsor>,
    pub snapshot: Option<BuildSnapshotRequest>,
    pub restricted_grants: Option<Box<dyn RestrictedBuildGrants>>, // Absence reports restricted requests without executing them.
    // A dependency's Build executed while resolving its consumer's sources.
    // The request's subject is the consumer, so this Build checks only what
    // its entry reaches (wiki/spec/build/validation_subjects.md#orchestration).
    pub dependency_activation: bool,
}

impl BuildExecutionInputs {
    /// Authenticate supplied package defaults before selecting Build source files.
    pub fn admit_interpreter_providers(
        &mut self,
        inputs: Option<&crate::PackageCompilationInputs>,
        libraries: &[crate::source_imports::SuppliedBuildLibrary],
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        if let Some(inputs) = inputs {
            self.interpreter_providers.extend_package_defaults(
                inputs.admitted_package_provider_selections(
                    build_declarations::DependencyPurpose::Build,
                )?,
            );
        }
        for library in libraries {
            self.interpreter_providers
                .extend_package_defaults(library.provider_defaults()?);
        }
        Ok(())
    }

    pub fn contract(&self) -> BuildExecutionContract {
        BuildExecutionContract {
            diagnostic_output_bytes: self.diagnostic_output_bytes,
            profile: self.profile,
            snapshot: self.snapshot.clone(),
        }
    }

    /// Whether this Build is typed and judged whole under a provisional owner
    /// before it runs: only a root Build that has no package graph yet. A
    /// dependency Build checks only what its entry reaches, so it needs no
    /// check-only typing of its file.
    pub fn checks_whole_subject_first(
        &self,
        inputs: Option<&PackageCompilationInputs>,
        has_anonymous_source: bool,
    ) -> bool {
        !self.dependency_activation && inputs.is_none() && has_anonymous_source
    }

    pub fn resolve_compilation_defaults(
        mut self,
        inputs: Option<&PackageCompilationInputs>,
        has_anonymous_source: bool,
    ) -> Self {
        self.profile = self.profile.or_else(TargetProfile::host_if_supported);
        if self.snapshot.is_none()
            && (has_anonymous_source
                || inputs.is_some_and(|inputs| {
                    inputs.canonical_source_metadata(inputs.root()).is_some()
                }))
        {
            self.snapshot = Some(BuildSnapshotRequest::for_activation(
                self.dependency_activation,
            ));
        }
        self
    }
}

//! An admitted Build retains its filesystem observations and published source separately.
//! Completing the activation settles its output files and transfers the immutable source bytes.

mod programs;
pub use programs::{
    BuildProgramBinding, BuildProgramInvocation, DelegatedProgram, ForwardedProgramFiles,
    ProgramOutputBacking, ProgramOutputSink,
};

mod build_admission;
pub mod build_value;
mod completed_outputs;
mod execution_inputs;
mod filesystem_scope;
mod observation_identity;
mod observations;
mod output_custody;
mod restricted_requests;
mod snapshot_request;
mod source_publications;

pub use build_admission::{SelectedBuildMachine, admit_build_machine, select_build_machine};
pub use completed_outputs::RetainedBuildOutputs;
pub use execution_inputs::{BuildExecutionContract, BuildExecutionInputs};
pub use filesystem_scope::preparation::prepare_filesystem_scope;
pub use filesystem_scope::runtime::{BuildFilesystemEffects, BuildFilesystemRuntime};
pub use filesystem_scope::successor_directory::SuccessorDirectory;
pub use filesystem_scope::{
    BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildMachineCapturedFilesystemRoot,
    BuildMachineExecutionMode, BuildMachineFilesystemAccess, BuildMachineFilesystemScope,
};
pub use observation_identity::BuildObservationIdentity;
pub use observations::*;
pub use restricted_requests::{
    ProgramDeclarationOwner, RestrictedBuildBounds, RestrictedBuildGrant, RestrictedBuildGrantRoot,
    RestrictedBuildGrants, RestrictedBuildOperation, RestrictedBuildRequest, RestrictedBuildSite,
    RestrictedBuildSubject, RestrictedProgramLaunch,
};
pub use snapshot_request::{BuildSnapshotCapture, BuildSnapshotRequest};
pub use source_publications::BuildSourcePublications;

use diagnostics::Diagnostic;
use filesystem_authority::{BuildOutputObligation, BuildOutputReceipt};

fn message<T, E: std::fmt::Display>(result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| error.to_string())
}

pub struct CapturedBuildObservations<'scope> {
    scope: &'scope BuildMachineFilesystemScope,
    filesystem_host_observed: bool,
    filesystem_operation_schema_version: u32,
    filesystem_operation_attempts: Vec<BuildFilesystemOperationAttempt>,
    generated_sources: Option<Vec<build_output::PackageGeneratedSource>>,
    output_obligations: Vec<BuildOutputObligation>,
    output_receipts: Vec<BuildOutputReceipt>,
    build_log: Vec<u8>,
    filesystem_storage: Vec<evaluation_accounting::EvaluationStorageLease>,
    filesystem_effects_retained: bool,
}

impl<'scope> CapturedBuildObservations<'scope> {
    pub fn retain_filesystem_effects(
        &mut self,
        effects: BuildFilesystemEffects,
    ) -> Result<(), Vec<Diagnostic>> {
        if effects.scope != *self.scope {
            return Err(vec![Diagnostic::error(
                "Build filesystem effects belong to another admitted scope",
            )]);
        }
        if self.filesystem_effects_retained {
            return Err(vec![Diagnostic::error(
                "Build filesystem effects were retained more than once",
            )]);
        }
        self.filesystem_host_observed = !effects.attempts.is_empty();
        self.filesystem_operation_attempts = effects.attempts;
        self.output_obligations = effects.obligations;
        self.output_receipts = effects.receipts;
        self.filesystem_storage = effects.storage;
        self.filesystem_effects_retained = true;
        Ok(())
    }

    pub fn new(scope: &'scope BuildMachineFilesystemScope) -> Self {
        Self {
            scope,
            filesystem_host_observed: false,
            filesystem_operation_schema_version:
                filesystem_authority::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
            filesystem_operation_attempts: Vec::new(),
            generated_sources: None,
            output_obligations: Vec::new(),
            output_receipts: Vec::new(),
            build_log: Vec::new(),
            filesystem_storage: Vec::new(),
            filesystem_effects_retained: false,
        }
    }

    pub fn retain_source_publications(
        &mut self,
        publications: BuildSourcePublications,
        account: &evaluation_accounting::EvaluationInvocationAccount,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.generated_sources.is_some() {
            return Err(vec![Diagnostic::error(
                "Build source publications were retained more than once",
            )]);
        }
        self.generated_sources = Some(
            publications
                .finish(account)
                .map_err(|reason| vec![Diagnostic::error(reason)])?,
        );
        Ok(())
    }

    pub fn retain_build_log(&mut self, log: Vec<u8>) -> Result<(), Vec<Diagnostic>> {
        if !self.build_log.is_empty() {
            return Err(vec![Diagnostic::error(
                "Build log was retained more than once",
            )]);
        }
        self.build_log = log;
        Ok(())
    }

    pub fn settle(
        self,
        filesystem_reachable: bool,
        machine_name: &str,
    ) -> Result<BuildObservationSummary, Vec<Diagnostic>> {
        let scope = self.scope;
        let staged_output_tree = scope.staged_output_tree(filesystem_reachable)?;
        if self.filesystem_host_observed && !filesystem_reachable {
            return Err(vec![Diagnostic::error(format!(
                "build-time evaluation of `{machine_name}` observed filesystem host state without admitted filesystem reach"
            ))]);
        }
        scope.verify_required_outputs(staged_output_tree.as_ref(), machine_name)?;
        let required_output_settlements = output_custody::settle_build_output_obligations(
            &self.output_obligations,
            &self.output_receipts,
            staged_output_tree.as_ref(),
            machine_name,
        )?;
        let product_source_tree = match scope.product_source_directory.as_deref() {
            Some(directory) => {
                let staging = staged_output_tree.as_ref().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "a selected Product source directory requires private activation staging",
                    )]
                })?;
                Some(staging.select_directory(directory)?)
            }
            None => None,
        };
        let published = self.generated_sources.unwrap_or_default();
        let generated_sources = match &product_source_tree {
            None => published,
            Some(tree) if published.is_empty() => tree.source_files()?,
            Some(_) => {
                return Err(vec![Diagnostic::error(
                    "a Build with a selected Product source directory cannot also publish files individually",
                )]);
            }
        };
        Ok(BuildObservationSummary {
            filesystem_host_observed: self.filesystem_host_observed,
            filesystem_operation_schema_version: self.filesystem_operation_schema_version,
            filesystem_operation_attempts: self.filesystem_operation_attempts,
            canonical_source_metadata_identity: scope.canonical_source_metadata_identity(),
            activation: scope.activation(),
            captured_source_inventory: scope.captured_source_inventory(),
            named_input_inventories: scope.named_input_inventories(),
            product_source_tree,
            generated_sources,
            required_output_settlements,
            staged_output_tree,
            build_log: self.build_log,
        })
    }
}

/*
Build-machine admission, filesystem scope custody, interpreter observation
collection, and output obligation settlement.

We own the pre-execution and post-execution boundaries around `Build` evaluation:
1. Before execution, `execution_inputs`, `snapshot_request`, `filesystem_scope`,
   `build_admission`, and `restricted_requests` validate the selected `Build`
   machine, construct its initial `BuildTimeValue`, establish read-only or
   scoped filesystem access, and project restricted host requests in package
   vocabulary.
2. During and after execution, `CapturedBuildObservations` collects filesystem
   operation attempts, included-source handoffs, output obligations/receipts,
   and `Build.log` bytes, then settles them (`settle`) against the staged output
   tree into a durable `BuildObservationSummary` and `RetainedBuildOutputs`.
*/

/*
Keeping observation capture separate from `Build`-value validation and output
settlement preserves diagnostic ordering while ensuring that partially settled
or unsealed outputs can never be exported as generated Omega source or published
into `completed/` output sets.
*/

/*
See `build_admission.rs` for machine selection and service-reach checks,
`filesystem_scope.rs` for scoped source and output custody, `build_value.rs` for
`Build` value admission and extraction, and `observations.rs` /
`output_custody.rs` / `completed_outputs.rs` for settlement and publication.
*/

mod build_admission;
mod build_receiver;
#[doc(hidden)]
pub mod build_record_codec;
pub mod build_value;
mod completed_outputs;
mod execution_inputs;
mod filesystem_scope;
mod observation_identity;
mod observations;
mod output_custody;
mod restricted_requests;
mod snapshot_request;

pub use build_admission::{SelectedBuildMachine, admit_build_machine, select_build_machine};
pub use completed_outputs::RetainedBuildOutputs;
pub use execution_inputs::{BuildExecutionContract, BuildExecutionInputs};
pub use filesystem_scope::preparation::prepare_filesystem_scope;
pub use filesystem_scope::runtime::{BuildFilesystemEffects, BuildFilesystemRuntime};
pub use filesystem_scope::{
    BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildMachineFilesystemScope,
};
pub use observation_identity::BuildObservationIdentity;
pub use observations::*;
pub use restricted_requests::{
    RestrictedBuildBounds, RestrictedBuildGrant, RestrictedBuildGrantRoot, RestrictedBuildGrants,
    RestrictedBuildOperation, RestrictedBuildRequest,
};
pub use snapshot_request::{BuildSnapshotCapture, BuildSnapshotRequest};

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
    included_source_handoffs: Vec<BuildIncludedSourceHandoff>,
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
        self.included_source_handoffs = effects.included;
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
            included_source_handoffs: Vec::new(),
            output_obligations: Vec::new(),
            output_receipts: Vec::new(),
            build_log: Vec::new(),
            filesystem_storage: Vec::new(),
            filesystem_effects_retained: false,
        }
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
        Ok(BuildObservationSummary {
            schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
            filesystem_host_observed: self.filesystem_host_observed,
            filesystem_operation_schema_version: self.filesystem_operation_schema_version,
            filesystem_operation_attempts: self.filesystem_operation_attempts,
            canonical_source_metadata_identity: scope.canonical_source_metadata_identity(),
            activation: scope.activation(),
            captured_source_inventory: scope.captured_source_inventory(),
            named_input_inventories: scope.named_input_inventories(),
            included_source_handoffs: self.included_source_handoffs,
            required_output_settlements,
            staged_output_tree,
            build_log: self.build_log,
        })
    }
}

impl BuildObservationSummary {
    pub fn select_generated_sources(
        &self,
        machine_name: &str,
    ) -> Result<Vec<build_output::PackageGeneratedSource>, Vec<Diagnostic>> {
        output_custody::select_generated_sources(
            self.staged_output_tree.as_ref(),
            &self.included_source_handoffs,
            machine_name,
        )
    }
}

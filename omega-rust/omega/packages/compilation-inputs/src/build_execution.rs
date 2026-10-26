//! Custody of one build execution: project interpreter observations, then
//! settle sealed outputs before publishing evidence or generated source.
//!
//! Capture precedes Build-value validation; settlement follows it. Keeping
//! these two operations separate preserves diagnostic order without exposing
//! partially settled observation records to the compiler.
//! Restricted host requests project the admitted execution mode and scope
//! before execution, so package review and consumption share one vocabulary.

mod build_admission;
#[doc(hidden)]
pub mod build_record_codec;
pub mod build_value;
mod completed_outputs;
mod filesystem_scope;
mod observation_identity;
mod observation_projection;
mod observations;
mod output_custody;
mod restricted_requests;
mod snapshot_request;

pub use build_admission::{SelectedBuildMachine, admit_build_machine, select_build_machine};
pub use completed_outputs::RetainedBuildOutputs;
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

use checked_interpreter::EvaluationObservations;
use diagnostics::Diagnostic;
use filesystem_authority::{BuildOutputObligation, BuildOutputReceipt};

/// Interpreter observations whose output obligations have not yet settled.
/// Only successful settlement publishes a durable observation summary.
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
    /// Consume observations issued by this activation's filesystem runtime.
    /// The collector still checks actual sealed files at normal return.
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
    /// Begin an activation's host observation collector before execution.
    /// Admitted scope is input custody, not evidence that an operation ran.
    /// Each installed host adapter must contribute its actual observations;
    /// successful settlement still inspects staged output and obligations.
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

    /// Transfer the one activation log after normal return. No host effect is
    /// repeated and no second payload copy is needed for settlement.
    pub fn retain_build_log(&mut self, log: Vec<u8>) -> Result<(), Vec<Diagnostic>> {
        if !self.build_log.is_empty() {
            return Err(vec![Diagnostic::error(
                "Build log was retained more than once",
            )]);
        }
        self.build_log = log;
        Ok(())
    }

    /// Rejoin interpreter root identities and generated-source handoffs to
    /// this activation's admitted filesystem custody.
    pub fn capture(
        observations: &EvaluationObservations,
        scope: &'scope BuildMachineFilesystemScope,
        machine_name: &str,
    ) -> Result<Self, Vec<Diagnostic>> {
        let filesystem_operation_attempts =
            observation_projection::project_filesystem_operation_attempts(
                observations,
                scope,
                machine_name,
            )?;
        let included_source_handoffs = observation_projection::project_included_source_handoffs(
            observations,
            filesystem_operation_attempts.len(),
            machine_name,
        )?;
        Ok(Self {
            scope,
            filesystem_host_observed: observations.filesystem_host_observed(),
            filesystem_operation_schema_version: observations.filesystem_operation_schema_version(),
            filesystem_operation_attempts,
            included_source_handoffs,
            output_obligations: observations.build_output_obligations().to_vec(),
            output_receipts: observations.build_output_receipts().to_vec(),
            build_log: observations.build_log().to_vec(),
            filesystem_storage: Vec::new(),
            filesystem_effects_retained: true,
        })
    }

    /// Capture staged outputs, reject unadmitted host observations, then
    /// discharge both caller-required and authored output obligations.
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
    /// Select only source handoffs sealed by this settled execution.
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

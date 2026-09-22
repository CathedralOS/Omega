//! Reconstruct boundary-operator applications and bind their custody to the published artifact.

#[path = "dynamic_calls.rs"]
mod dynamic_calls;
#[path = "float_comparisons.rs"]
mod float_comparisons;
#[path = "integer_comparisons.rs"]
mod integer_comparisons;
#[path = "local_initializers.rs"]
mod local_initializers;
#[path = "structural_returns.rs"]
mod structural_returns;

use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;
use semantic_vocabulary::{MachineId, OperationId};
use std::collections::BTreeSet;

fn unsupported<T>(message: &'static str) -> Result<T, &'static str> {
    Err(message)
}
/// Checked source demand scope retained beside one exact Terminal artifact.
///
/// The fields are private so downstream realization cannot replace checked
/// D29 custody with a caller-authored count or Boolean. The receipt retains the
/// complete checked demand roster and is useful only for the canonical artifact
/// produced by the same lowering operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoundaryOperatorApplicationScope {
    terminal_artifact_identity: terminal_codec::TerminalArtifactIdentity,
    applications: Vec<checked_trees::CheckedBoundaryOperatorApplicationDemand>,
    occurrences: Vec<CheckedBoundaryOperatorApplicationOccurrence>,
    dynamic_call_occurrences: Vec<CheckedDynamicCallOccurrence>,
}

impl CheckedBoundaryOperatorApplicationScope {
    pub fn validate_for_artifact(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
    ) -> Result<(), &'static str> {
        if self.terminal_artifact_identity != artifact.manifest().identity() {
            return Err("checked boundary-operator scope belongs to a different Terminal artifact");
        }
        Ok(())
    }

    pub fn applications(&self) -> &[checked_trees::CheckedBoundaryOperatorApplicationDemand] {
        &self.applications
    }

    pub fn is_empty(&self) -> bool {
        self.applications.is_empty()
    }

    pub fn occurrences(&self) -> &[CheckedBoundaryOperatorApplicationOccurrence] {
        &self.occurrences
    }

    /// Exact source-produced dynamic calls replayed into the admitted
    /// descriptor-table lane, one row per surviving Terminal occurrence.
    pub fn dynamic_call_occurrences(&self) -> &[CheckedDynamicCallOccurrence] {
        &self.dynamic_call_occurrences
    }
}

/// Which checked rebound roster one source-produced dynamic-call occurrence
/// rejoins. Both lanes are local descriptor-table materializations; forwarded
/// and stored forms emit different catalog species and stay unadmitted here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedDynamicCallLane {
    /// `rebound_scalar_calls`: a scalar dispatch through a materialized
    /// rebound descriptor (`CallDynamicScalar`).
    ReboundScalar,
    /// `rebound_unit_calls`: a Unit dispatch through a materialized rebound
    /// descriptor (`CallDynamicUnit`).
    ReboundUnit,
}

/// Compiler-private join from one retained checked rebound dynamic-call plan
/// to the exact Terminal operation and descriptor-table row produced for it.
/// `plan_index` addresses the checked `dynamic_dispatch` roster for `lane`;
/// the machine/operation pair names the emitted `CallDynamic*` occurrence the
/// native projection must retain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedDynamicCallOccurrence {
    lane: CheckedDynamicCallLane,
    plan_index: usize,
    terminal_machine: MachineId,
    terminal_operation: OperationId,
    descriptor_ordinal: u32,
}

impl CheckedDynamicCallOccurrence {
    pub(super) const fn new(
        lane: CheckedDynamicCallLane,
        plan_index: usize,
        terminal_machine: MachineId,
        terminal_operation: OperationId,
        descriptor_ordinal: u32,
    ) -> Self {
        Self {
            lane,
            plan_index,
            terminal_machine,
            terminal_operation,
            descriptor_ordinal,
        }
    }

    pub const fn lane(self) -> CheckedDynamicCallLane {
        self.lane
    }

    /// Index into the lane's checked rebound roster.
    pub const fn plan_index(self) -> usize {
        self.plan_index
    }

    pub const fn terminal_machine(self) -> MachineId {
        self.terminal_machine
    }

    pub const fn terminal_operation(self) -> OperationId {
        self.terminal_operation
    }

    /// Owner-local rebound descriptor ordinal the dispatch consumes.
    pub const fn descriptor_ordinal(self) -> u32 {
        self.descriptor_ordinal
    }
}

/// Compiler-private join from one retained checked D29 demand to the exact
/// Terminal operation produced for it. The application index addresses the
/// immutable roster in the checked boundary-operator application scope.
///
/// This is source-to-Terminal custody, not canonical D29 coverage: public
/// source-free application identity and the role-specific realization
/// companion remain later compiler-owned projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedBoundaryOperatorApplicationOccurrence {
    application_index: usize,
    terminal_operation: OperationId,
}

impl CheckedBoundaryOperatorApplicationOccurrence {
    pub const fn application_index(self) -> usize {
        self.application_index
    }

    pub const fn terminal_operation(self) -> OperationId {
        self.terminal_operation
    }
}

pub fn checked_boundary_operator_scope(
    checked: &CheckedTrees,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    lowered: &LoweredPsi,
) -> Result<CheckedBoundaryOperatorApplicationScope, &'static str> {
    let semantic = terminal_codec::terminal_psi_identity(&lowered.semantic_module)
        .map_err(|_| "checked boundary-operator scope has invalid semantics")?;
    if semantic != artifact.manifest().semantic() {
        return Err("checked boundary-operator scope semantics differ from the published artifact");
    }
    Ok(CheckedBoundaryOperatorApplicationScope {
        terminal_artifact_identity: artifact.manifest().identity(),
        applications: checked.facts.operators.boundary_applications.clone(),
        occurrences: checked_boundary_operator_occurrences(checked, lowered)?,
        dynamic_call_occurrences: dynamic_calls::replay(checked, lowered)?,
    })
}

fn checked_boundary_operator_occurrences(
    checked: &CheckedTrees,
    lowered: &LoweredPsi,
) -> Result<Vec<CheckedBoundaryOperatorApplicationOccurrence>, &'static str> {
    let mut occurrences = Vec::new();
    let matched_ieee_float_fmas = local_initializers::replay(checked, lowered, &mut occurrences)?;
    structural_returns::replay(checked, lowered, &mut occurrences)?;
    float_comparisons::replay(checked, lowered, &mut occurrences)?;
    integer_comparisons::replay(checked, lowered, &mut occurrences)?;
    occurrences.sort_by_key(|occurrence| occurrence.terminal_operation.get());
    let application_indices = occurrences
        .iter()
        .map(|occurrence| occurrence.application_index)
        .collect::<BTreeSet<_>>();
    let terminal_operations = occurrences
        .iter()
        .map(|occurrence| occurrence.terminal_operation)
        .collect::<BTreeSet<_>>();
    if application_indices.len() != occurrences.len()
        || terminal_operations.len() != occurrences.len()
    {
        return unsupported(
            "checked boundary-operator applications do not map one-to-one onto Terminal operations",
        );
    }
    if matched_ieee_float_fmas != lowered.selected_ieee_float_fma_occurrences.len() {
        return unsupported(
            "selected IEEE FMA Terminal occurrences do not all rejoin checked boundary applications",
        );
    }
    Ok(occurrences)
}

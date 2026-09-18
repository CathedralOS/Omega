use calling_conventions::{
    BoundaryEntryPlan, ValidatedBoundaryEntryPlan, ValuePlacement, ValueShape,
};
use effects::provider_plan::BoundaryCallingPlanCommitment;
use language_semantics::CarryPolicy;

use crate::{
    ProgramEntryPhysicalContractPlan, ProgramEntrySourceSignatureIdentity,
    ProgramStorageEntryRootRole, SelectedProgramEntrySourceSignature,
};

/// Exact semantic calling-plan application custody carried through the native
/// binder.
///
/// A schema method's `calling_plan_report_fingerprint`/`calling_plan_commitment`
/// name the complete target-closed calling-plan *application* — the semantic
/// signature, target, opaque representations, and canonical validated
/// `BoundaryEntryPlan` selected by the `Calling<C>` relationship — never the
/// raw ABI plan. Recomputing that application identity needs the build-side
/// materialized signature, which this crate cannot name without an upward
/// dependency, so the native binder replays its retained realization and hands
/// this pair in beside the exact validated plan the commitment covers.
/// Comparing the schema fields against the plan's own
/// `contract_report_fingerprint`/`contract_commitment_digest` crosses identity
/// namespaces and could only ever match a fabricated schema row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticCallingApplication<'plan> {
    boundary_entry_plan: &'plan ValidatedBoundaryEntryPlan,
    report_fingerprint: u64,
    commitment: BoundaryCallingPlanCommitment,
}

impl<'plan> OptimizedProgramStorageSemanticCallingApplication<'plan> {
    /// Retain the binder's replayed application identity next to the exact
    /// canonical plan the commitment covers. The pair is data here; the
    /// binder's replay (`BoundaryCallingPlanRealization`'s validated
    /// application) is what proves the commitment binds this plan.
    pub const fn new(
        boundary_entry_plan: &'plan ValidatedBoundaryEntryPlan,
        report_fingerprint: u64,
        commitment: BoundaryCallingPlanCommitment,
    ) -> Self {
        Self {
            boundary_entry_plan,
            report_fingerprint,
            commitment,
        }
    }

    /// The exact canonical ABI plan the retained application commits to.
    pub const fn boundary_entry_plan(&self) -> &'plan BoundaryEntryPlan {
        self.boundary_entry_plan.plan()
    }

    /// Compact report coordinate of the complete calling-plan application.
    pub const fn report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }

    /// Domain-separated commitment to the complete calling-plan application.
    pub const fn commitment(&self) -> BoundaryCallingPlanCommitment {
        self.commitment
    }
}

/// Explicit status of the separately retained physical entry contract.
///
/// The semantic contract requires this plan to remain paired with the selected
/// target slot, but cannot use it to construct or invoke a bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStoragePhysicalEntryDisposition {
    PlannedNotInvokedV1,
}

/// One exact qualified semantic root and its address-free ABI placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticRoot {
    pub(super) role: ProgramStorageEntryRootRole,
    pub(super) parameter_index: usize,
    pub(super) carrier_identity: String,
    pub(super) parameter_type_identity: String,
    pub(super) domain: String,
    pub(super) effective_carry: CarryPolicy,
    pub(super) shape: ValueShape,
    pub(super) placement: ValuePlacement,
}

impl OptimizedProgramStorageSemanticRoot {
    pub const fn role(&self) -> ProgramStorageEntryRootRole {
        self.role
    }

    pub const fn parameter_index(&self) -> usize {
        self.parameter_index
    }

    pub fn carrier_identity(&self) -> &str {
        &self.carrier_identity
    }

    pub fn parameter_type_identity(&self) -> &str {
        &self.parameter_type_identity
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub const fn effective_carry(&self) -> CarryPolicy {
        self.effective_carry
    }

    pub const fn shape(&self) -> ValueShape {
        self.shape
    }

    pub const fn placement(&self) -> &ValuePlacement {
        &self.placement
    }
}

/// Validated declaration-only contract for a future clean semantic wrapper.
///
/// A higher compiler layer must separately join this contract to the exact
/// Terminal `MachineId` and private object symbol before emitting anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticEntryContract {
    pub(super) target: target::NativeTarget,
    pub(super) target_slot: target::ProgramEntrySlotDeclaration,
    pub(super) requirement_identity: String,
    pub(super) source_signature: SelectedProgramEntrySourceSignature,
    pub(super) source_signature_identity: ProgramEntrySourceSignatureIdentity,
    pub(super) semantic_boundary_entry_plan: BoundaryEntryPlan,
    pub(super) semantic_calling_plan_report_fingerprint: u64,
    /// Compact report coordinate of the exact source calling-plan application
    /// the selected schema committed to. Kept beside, never instead of, the
    /// raw plan coordinate above: the two live in distinct identity namespaces.
    pub(super) semantic_calling_application_report_fingerprint: u64,
    /// Domain-separated commitment to that same exact application.
    pub(super) semantic_calling_application_commitment: BoundaryCallingPlanCommitment,
    pub(super) roots: [OptimizedProgramStorageSemanticRoot; 2],
    pub(super) physical_contract: ProgramEntryPhysicalContractPlan,
    pub(super) physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition,
}

impl OptimizedProgramStorageSemanticEntryContract {
    pub const fn target(&self) -> target::NativeTarget {
        self.target
    }

    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn source_signature(&self) -> &SelectedProgramEntrySourceSignature {
        &self.source_signature
    }

    pub const fn source_signature_identity(&self) -> ProgramEntrySourceSignatureIdentity {
        self.source_signature_identity
    }

    pub const fn semantic_boundary_entry_plan(&self) -> &BoundaryEntryPlan {
        &self.semantic_boundary_entry_plan
    }

    pub const fn semantic_calling_plan_report_fingerprint(&self) -> u64 {
        self.semantic_calling_plan_report_fingerprint
    }

    pub const fn semantic_calling_application_report_fingerprint(&self) -> u64 {
        self.semantic_calling_application_report_fingerprint
    }

    pub const fn semantic_calling_application_commitment(&self) -> BoundaryCallingPlanCommitment {
        self.semantic_calling_application_commitment
    }

    pub const fn roots(&self) -> &[OptimizedProgramStorageSemanticRoot; 2] {
        &self.roots
    }

    pub const fn physical_contract(&self) -> &ProgramEntryPhysicalContractPlan {
        &self.physical_contract
    }

    pub const fn physical_disposition(&self) -> OptimizedProgramStoragePhysicalEntryDisposition {
        self.physical_disposition
    }
}

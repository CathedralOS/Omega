//! Affine cleanup plans: partial and nominal affine discards, caller
//! requirements, claim transfers and call custody.

use crate::checked_trees::CheckedMemberIdentity;
use crate::checked_trees::flow::terminal::{
    CheckedUnitEffectMachinePlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment,
};
use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;
use typed_trees::type_identity::TypeIdentityId;

/// Conditions and their paths share the ordinary Unit plan's storage namespace.
/// They are control dependencies, not permission to dispose a payload. A nested
/// condition follows its enclosing condition in the retained span.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedCleanupControlPlans {
    pub conditions: arena::Arena<CheckedCleanupCaseCondition>,
    pub paths: arena::Arena<CheckedUnitStructuralPathSegment>,
    /// One durable owner for sequence and graph normal completion transactions.
    pub normal_exit_rows: arena::Arena<CheckedNormalExitCleanupPlan>,
    pub normal_exit_actions: arena::Arena<CheckedNormalCleanupAction>,
    pub normal_exit_regions: arena::Arena<super::CheckedUnitStructuralArgumentPlan>,
}

impl CheckedCleanupControlPlans {
    pub fn normal_exits(
        &self,
        rows: arena::HandleSpan<CheckedNormalExitCleanupPlan>,
    ) -> &[CheckedNormalExitCleanupPlan] {
        self.normal_exit_rows.span_or_empty(rows)
    }

    pub fn normal_actions(
        &self,
        rows: arena::HandleSpan<CheckedNormalExitCleanupPlan>,
    ) -> impl Iterator<Item = &CheckedNormalCleanupAction> {
        self.normal_exits(rows)
            .iter()
            .flat_map(|row| self.normal_exit_actions.span_or_empty(row.actions))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedCleanupCaseCondition {
    pub path: arena::HandleSpan<CheckedUnitStructuralPathSegment>,
    pub type_identity: TypeIdentityId,
    pub case_identity: CheckedMemberIdentity,
}

/// One claim-free affine structural place disposed on the normal edge selected
/// by its containing plan. An empty path denotes the whole root; a nonempty path
/// denotes a retained residual subtree. The root is a structural parameter or
/// call-result binding or retained expression, with canonical semantic path
/// identity. A projection preserves that owner's identity through disposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitPartialAffineDiscardPlan {
    pub source: CheckedUnitStructuralArgumentSourcePlan,
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub type_identity: TypeIdentityId,
    /// Conditions in `CheckedUnitEffectPlans::cleanup_control`. A guarded
    /// whole-region discard disposes only the selected alternative; it must
    /// not intrinsically retire the home needed by other cleanup branches.
    pub case_conditions: arena::HandleSpan<CheckedCleanupCaseCondition>,
}

impl Default for CheckedUnitPartialAffineDiscardPlan {
    fn default() -> Self {
        Self {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                expression: arena::Handle::invalid(),
            },
            path: Vec::new(),
            type_identity: TypeIdentityId::default(),
            case_conditions: Default::default(),
        }
    }
}

/// Checked-only carrier for direct-record-field partial cleanup. Its machines
/// stay separate from `CheckedUnitEffectPlans` because their return cleanup is
/// path-sensitive rather than a whole-root discard; the shapes they name live
/// in that roster's one structural type namespace.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedPartialAffineUnitCleanupPlans {
    pub machines: Vec<CheckedPartialAffineUnitCleanupMachinePlan>,
}

impl CheckedPartialAffineUnitCleanupPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedPartialAffineUnitCleanupMachinePlan> {
        self.machines
            .iter()
            .find(|plan| plan.machine.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPartialAffineUnitCleanupMachinePlan {
    /// Complete ordinary Unit signature/call/return plan. This machine is not
    /// published through `CheckedUnitEffectPlans` while its return cleanup is
    /// still path-sensitive.
    pub machine: CheckedUnitEffectMachinePlan,
    /// Exact maximal residual subtrees after every source-ordered projected
    /// call commits. Pairwise prefix-disjoint paths are grouped recursively at
    /// selected ancestors while retaining reverse declaration order at every
    /// record level.
    pub residual_affine_discards: Vec<CheckedUnitPartialAffineDiscardPlan>,
}

/// Declaration metadata for nominal cleanup callers. Executable actions and
/// their source exit coordinates belong to `CheckedUnitEffectPlans`' shared
/// cleanup owner; this catalogue never owns a competing body or action stream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedNominalAffineUnitCleanupPlans {
    pub machines: Vec<CheckedNominalAffineUnitCleanupMachinePlan>,
}

impl CheckedNominalAffineUnitCleanupPlans {
    pub fn for_machine(
        &self,
        machine: SymbolHandle,
    ) -> Option<&CheckedNominalAffineUnitCleanupMachinePlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedNominalAffineUnitCleanupMachinePlan {
    /// Declaration metadata only. The published body and its normal-exit spans
    /// stay in CheckedUnitEffectPlans; this catalogue owns no disposal stream.
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub attachment_type_identity: Option<TypeIdentityId>,
    /// Direct source caller prerequisites consumed by ordinary signature lowering.
    pub caller_requirements: Vec<CheckedUnitNominalAffineCallerRequirementPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitNominalAffineCallerRequirementPlan {
    pub source_parameter_index: u32,
    pub field_identity: CheckedMemberIdentity,
    pub expected: bool,
}

/// One whole affine owner disposed by its exact checked nominal cleanup.
/// The source belongs to the ordinary state value namespace; parameter indexes
/// are dense structural indexes. Type identity joins it to the attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitNominalAffineCleanupPlan {
    /// Exact source claims consumed by this selected nominal disposition.
    pub consumed_claims: Vec<language_semantics::PermissionClaimIdentity>,
    pub source: CheckedUnitStructuralArgumentSourcePlan,
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub type_identity: TypeIdentityId,
    pub cleanup_machine: SymbolHandle,
    pub cleanup_state: SymbolHandle,
    pub cleanup_contract_report_fingerprint: u64,
    /// Select this exact occurrence before lending its intact receiver. These
    /// conditions do not discharge the callable's ordinary prerequisites.
    pub case_conditions: arena::HandleSpan<CheckedCleanupCaseCondition>,
    // The callable owns its complete prerequisite roster. Retaining a second,
    // simplified copy here would make cleanup accept a different contract from
    // ordinary calls. Emission binds obligations to that callable's signature.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitNominalAffineCleanupRequirementPlan {
    /// Normalized declaration identity, using the same `#identity`/name
    /// convention as [`CheckedUnitStructuralFieldPlan::identity`].
    pub field_identity: CheckedMemberIdentity,
    pub expected: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedUnitClaimTransferPlan {
    pub claim_identity: language_semantics::PermissionClaimIdentity,
    /// Dense index into this call's structural argument list. The callee-local
    /// claim identity is reconstructed by terminal verification.
    pub argument_index: u32,
    /// Exact typed path relative to that argument's selected value. Case
    /// segments retain the discriminator guards on conditional custody.
    pub path: Vec<CheckedUnitStructuralPathSegment>,
}

/// Ordinary structural result custody is separate from its value binding.
/// The input rows move caller claims into the callee; the result rows bind
/// the claims the callee returns, one per linear claim path of its declared
/// result type. A result claim continues whichever claim the callee's
/// returning edge holds at that path, so different exits may return
/// different inputs or fresh establishments there; the checked join receipts
/// keep that per-exit lineage. An empty record is valid only for an
/// independently checked unqualified, claim-free result. Checking decides
/// the record once (`validation::structural_call_custody`) and lowering reads
/// it as recorded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralCallCustodyPlan {
    /// Complete source-loan group carried by a root reference result. Runtime
    /// selection chooses the referent, but every possible source stays live.
    /// An empty span means no locally held reference-result resource.
    pub reference_loans: arena::HandleSpan<crate::BorrowLoanFact>,
    /// Borrowed actuals whose descriptor homes supply this root reference
    /// result. These are disposal dependencies, including anonymous operands;
    /// owned descriptor transfers do not retain their former holder.
    pub retained_reference_inputs: Vec<u32>,
    /// Exact ingress leaves forwarded by a terminal call with no local holder.
    /// The checked return occurrence owns symbolic backing geometry; it is not
    /// a recipe for forming one descriptor per possible runtime element.
    pub forwarded_reference_sources: Vec<crate::CheckedReturnedReferencePlan>,
    pub result_qualifications: Vec<SemanticDomainId>,
    pub claim_transfers: Vec<CheckedUnitClaimTransferPlan>,
    /// Canonical path order; empty exactly when the result type carries no
    /// linear claim.
    pub result_claims: Vec<CheckedStructuralResultClaimPlan>,
}

/// One claim the caller holds beneath a structural call result after normal
/// completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralResultClaimPlan {
    /// The caller's identity for the received claim. An input returned on
    /// every exit keeps its transferred identity; a result received by a
    /// local carries the local's establishment. `Unknown` marks a result no
    /// caller place receives (a terminal call's value returned at once), so
    /// lowering gives it a claim only the returning edge names.
    pub claim: language_semantics::PermissionClaimIdentity,
    pub path: Vec<CheckedUnitStructuralPathSegment>,
}

/// One action in a selected normal edge's reverse establishment order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedNormalCleanupAction {
    Discard(CheckedUnitPartialAffineDiscardPlan),
    InvokeNominal(CheckedUnitNominalAffineCleanupPlan),
    /// End this exact home occurrence after its selected dying-owner partition
    /// has completed. Region disposal alone does not retire a containing home:
    /// a replacement ends the old field value while retaining its parent.
    RetireHome {
        source: CheckedUnitStructuralArgumentSourcePlan,
        type_identity: TypeIdentityId,
    },
}

impl Default for CheckedNormalCleanupAction {
    fn default() -> Self {
        Self::Discard(CheckedUnitPartialAffineDiscardPlan::default())
    }
}

impl CheckedNormalCleanupAction {
    pub fn case_conditions(&self) -> arena::HandleSpan<CheckedCleanupCaseCondition> {
        match self {
            Self::Discard(discard) => discard.case_conditions,
            Self::InvokeNominal(cleanup) => cleanup.case_conditions,
            Self::RetireHome { .. } => Default::default(),
        }
    }

    pub fn nominal(&self) -> Option<&CheckedUnitNominalAffineCleanupPlan> {
        match self {
            Self::Discard(_) | Self::RetireHome { .. } => None,
            Self::InvokeNominal(cleanup) => Some(cleanup),
        }
    }
    pub fn path(&self) -> &[CheckedUnitStructuralPathSegment] {
        match self {
            Self::Discard(discard) => &discard.path,
            Self::InvokeNominal(cleanup) => &cleanup.path,
            Self::RetireHome { .. } => &[],
        }
    }

    pub fn type_identity(&self) -> TypeIdentityId {
        match self {
            Self::Discard(discard) => discard.type_identity,
            Self::InvokeNominal(cleanup) => cleanup.type_identity,
            Self::RetireHome { type_identity, .. } => *type_identity,
        }
    }
    pub fn source(&self) -> &CheckedUnitStructuralArgumentSourcePlan {
        match self {
            Self::Discard(discard) => &discard.source,
            Self::InvokeNominal(cleanup) => &cleanup.source,
            Self::RetireHome { source, .. } => source,
        }
    }
}

impl CheckedUnitNominalAffineCleanupPlan {
    pub fn source_parameter_index(&self) -> Option<u32> {
        match self.source {
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                Some(parameter_index)
            }
            _ => None,
        }
    }
}

/// Exact source outgoing coordinate, including the selected continuation arm.
/// These actions consume retained custody; they never establish entry premises.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedNormalExitCleanupPlan {
    /// Candidate owners retained by source checking across selected transfers.
    /// Their ordered actions run only where the exact emitted occurrence remains
    /// live at this death edge; this roster grants no transfer or birth authority.
    pub selected_regions: arena::HandleSpan<super::CheckedUnitStructuralArgumentPlan>,
    pub exit: crate::FlowClaimJoinExit,
    pub actions: arena::HandleSpan<CheckedNormalCleanupAction>,
}

//! Composed unit control plans: state plans, natural ranks, case returns and
//! closed-sum successors.

use crate::checked_trees::flow::terminal::{
    CheckedProviderAttachmentRequirementPlan, CheckedScalarBinding, CheckedScalarBranchDestination,
    CheckedScalarGuardedExit, CheckedStructuralControlSuccessorPlan, CheckedStructuralResultPlan,
    CheckedStructuralScalarParameterPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralParameterPlan,
};
use crate::{CheckedScalarExpression, ClosedScalarContractValue};
use language_semantics::{SemanticDomainId, ServiceReachPlan, ServiceReachSummary};
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedComposedUnitControlMachinePlan {
    pub machine: SymbolHandle,
    pub result: CheckedControlResultPlan,
    /// Exact cyclic-state subjects and natural-valued measures selected by the
    /// checked witness. Empty means this shared plan retains no witness.
    pub natural_ranks: Vec<CheckedStateNaturalRank>,
    /// Free helpers have no attachment; attached bodies retain their authored owner.
    pub attachment_type_identity: Option<String>,
    pub provider_attachment_requirements: Vec<CheckedProviderAttachmentRequirementPlan>,
    pub body_qualifications: Vec<SemanticDomainId>,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: crate::MachineContractCommitment,
    pub contract_service_reach: ServiceReachPlan,
    pub service_reach: ServiceReachSummary,
    pub states: Vec<CheckedComposedUnitControlStatePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedControlResultPlan {
    Unit,
    Structural(CheckedStructuralResultPlan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarCaseFieldPlan {
    /// Authored evaluation order, independent of declaration field order.
    pub field_ordinal: u32,
    pub field_identity: String,
    pub primitive_type: PrimitiveType,
    pub expression: CheckedScalarExpression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStateNaturalRank {
    pub state: SymbolHandle,
    pub parameter: SymbolHandle,
    pub parameter_position: u32,
    pub measure: CheckedNaturalRankMeasure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedNaturalRankMeasure {
    ByteSequenceLength,
    UnsignedParameter { primitive_type: PrimitiveType },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedComposedUnitControlStatePlan {
    pub state: SymbolHandle,
    pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    /// Proof-only erased scalar formals in authored order, retaining their
    /// authored parameter positions. They own no runtime argument lane.
    pub erased_scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    /// Lowered `requires` clauses for a non-entry state, in authored contract
    /// order. `None` marks a clause outside the admitted closed namespace;
    /// emission admits the state only when every row is `Some`.
    pub requires: Vec<Option<ClosedScalarContractValue>>,
    pub entry_claims: Vec<CheckedUnitEntryClaimPlan>,
    /// Ordered primitive declarations and storage assignments before this state's
    /// effects and terminator. Initializer expressions remain in the exact
    /// checked scalar-expression table under these statement ordinals.
    pub bindings: Vec<CheckedScalarBinding>,
    /// Initializers retained independently by this composed-control plan.
    /// Terminal admission requires exact agreement with the scalar-expression
    /// fact at the corresponding binding coordinate before emitting either.
    pub binding_initializers: Vec<CheckedScalarExpression>,
    /// Ordered effect operations only. Control exits remain in `terminator`.
    pub operations: Vec<CheckedUnitEffectOperationPlan>,
    pub terminator: CheckedComposedUnitControlTerminatorPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralCaseReturnPlan {
    pub statement_ordinal: u32,
    pub case_identity: String,
    pub fields: Vec<CheckedScalarCaseFieldPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedComposedUnitControlTerminatorPlan {
    ReturnUnit,
    /// Return a completed result from the ordinary structural value namespace.
    ReturnStructural {
        result: crate::CheckedUnitStructuralReturnPlan,
    },
    ReturnCase {
        result: CheckedStructuralCaseReturnPlan,
    },
    /// Guards and coverage belong to the shared source-owned scalar tail roster.
    /// These construction payloads bind its destinations by exact source coordinate,
    /// using the same local child storage as the composed plan's other exits.
    Guarded {
        arms: arena::HandleSpan<CheckedScalarGuardedExit>,
        fallback: Option<CheckedScalarBranchDestination>,
        return_values: Vec<CheckedUnitEffectOperationPlan>,
    },
    Jump {
        successor: CheckedStructuralControlSuccessorPlan,
    },
    Conditional {
        /// Exact checked scalar expression selected by the authored guard.
        /// The current family admits either one Boolean state parameter, a
        /// closed expression, or the bounded one-local conditional lane.
        guard: CheckedScalarExpression,
        when_true: CheckedStructuralControlSuccessorPlan,
        when_false: CheckedStructuralControlSuccessorPlan,
    },
    /// Consume one whole owned parameter or exact result from this state's
    /// operation prefix and transfer control through the exact closed case
    /// roster. Payload scalars are introduced only on their selected edge;
    /// they are not speculative reads from inactive storage.
    ClosedSum {
        subject: CheckedUnitStructuralArgumentPlan,
        cases: Vec<CheckedClosedSumCaseSuccessorPlan>,
    },
}

impl CheckedComposedUnitControlStatePlan {
    /// Complete operation dependencies, not runtime evaluation order.
    pub fn operation_dependencies(&self) -> impl Iterator<Item = &CheckedUnitEffectOperationPlan> {
        let selected = match &self.terminator {
            CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } => {
                return_values.as_slice()
            }
            _ => &[],
        };
        self.operations.iter().chain(selected)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedClosedSumCaseSuccessorPlan {
    pub case_identity: String,
    pub successor: CheckedStructuralControlSuccessorPlan,
    pub payloads: Vec<CheckedClosedSumPayloadTransferPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedClosedSumPayloadTransferPlan {
    pub field_identity: String,
    pub primitive_type: PrimitiveType,
    pub target_scalar_parameter_index: u32,
}

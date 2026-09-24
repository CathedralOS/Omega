//! Composed unit control plans: state plans, natural ranks, case returns and
//! closed-sum successors.

use crate::checked_trees::flow::terminal::{
    CheckedCallScalarArgument, CheckedProviderAttachmentRequirementPlan, CheckedScalarBinding,
    CheckedScalarBranchDestination, CheckedScalarGuardedExit,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralResultPlan,
    CheckedStructuralScalarParameterPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralParameterPlan,
};
use crate::checked_trees::values::CheckedErasedProofParameterPlan;
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
    /// One primitive scalar value. Each returning state completes it through
    /// `CheckedComposedUnitControlTerminatorPlan::ReturnScalar`.
    Scalar {
        primitive_type: PrimitiveType,
    },
}

impl CheckedControlResultPlan {
    pub fn structural_identity(&self) -> Option<&str> {
        match self {
            Self::Structural(result) => Some(&result.type_identity),
            Self::Unit | Self::Scalar { .. } => None,
        }
    }
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
    /// Erased formals whose carriers are proof-only and admit no scalar
    /// lane. The contract term lane carries their semantic type identities.
    pub erased_proof_parameters: Vec<CheckedErasedProofParameterPlan>,
    /// Lowered `requires` clauses for a non-entry state, in authored contract
    /// order. Producers emit only `Some` rows — an authored clause that fails
    /// the closed-namespace lowering leaves the whole plan unadmitted at check
    /// time rather than recording `None`. The `Option` retains emission's
    /// independent recheck: a plan built by any later producer still admits
    /// the state only when every row is `Some`.
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
    /// Complete a scalar-result machine with the same completion an ordinary
    /// single-state body retains; see `CheckedScalarReturnPlan`.
    ReturnScalar {
        completion: CheckedScalarReturnPlan,
    },
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
        /// Exact checked Boolean value recorded under the `Guard` role at
        /// `when_true.statement_ordinal`: either the retained pure expression
        /// (parameter reads, closed comparisons, locals, structural
        /// projections) or the unique computation root that coordinate owns
        /// (selected comparisons, calls), evaluated once before either edge.
        /// The `GuardedJumps` arm roster carries the same contract.
        guard: CheckedCallScalarArgument,
        when_true: CheckedStructuralControlSuccessorPlan,
        when_false: CheckedStructuralControlSuccessorPlan,
    },
    /// The two-way conditional whose other arm names an authored
    /// `(expression)` target: that arm returns its value instead of
    /// transferring to a named state. `jump` carries the named arm's ordinary
    /// custody plan; `return_arm` is the value arm, structural or scalar as the
    /// state's result is. `return_when_true` records which arm position the
    /// authored `(expression)` target occupied. (A conditional whose two arms
    /// are both `(expression)` targets checks as `Guarded` or scalar `Exits`,
    /// never this variant.)
    ConditionalReturn {
        guard: CheckedCallScalarArgument,
        jump: CheckedStructuralControlSuccessorPlan,
        return_arm: CheckedConditionalReturnArm,
        return_when_true: bool,
    },
    /// Ordered Boolean guards each select a named-state edge. Guards evaluate
    /// in authored order — the same sequencing the shared scalar tail roster
    /// records — and the authored `_` arm is the fallback edge holding where
    /// every guard fails. Each successor carries the same per-edge custody
    /// plan as a conditional edge.
    GuardedJumps {
        arms: Vec<CheckedGuardedJumpPlan>,
        fallback: CheckedStructuralControlSuccessorPlan,
    },
    /// Consume one whole owned parameter or exact result from this state's
    /// operation prefix and transfer control through the exact closed case
    /// roster. Payload scalars are introduced only on their selected edge;
    /// they are not speculative reads from inactive storage.
    ClosedSum {
        subject: CheckedUnitStructuralArgumentPlan,
        cases: Vec<CheckedClosedSumCaseSuccessorPlan>,
    },
    /// Leave checked execution without cleanup, a successor, or an
    /// established value — the standalone authored `crash` exit. The
    /// statement ordinal names its exact authored transition so the crash
    /// contract's checked-site row can be rejoined downstream.
    Crash {
        statement_ordinal: u32,
    },
}

/// How a returning state of a scalar-result graph completes its value. These
/// are the two completions an ordinary single-state body retains
/// (`CheckedUnitEffectMachinePlan::scalar_result` and `scalar_control`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarReturnPlan {
    /// The state's final expression: its last operation binds the returned
    /// value (a pure or computed `Return`-role expression, or a returned call
    /// result), or a final name returns an earlier immutable binding.
    Binding(crate::CheckedUnitScalarResultBindingPlan),
    /// Value-only transition exits after the ordered prefix: an unconditional
    /// return, or conditional/ordered arms that each return. Every value is the
    /// one checking recorded under the `Return` (or `ContinuationReturn`) role
    /// at the arm's exact statement coordinate.
    Exits(crate::CheckedUnitScalarControlPlan),
}

impl CheckedComposedUnitControlStatePlan {
    /// Complete operation dependencies, not runtime evaluation order.
    pub fn operation_dependencies(&self) -> impl Iterator<Item = &CheckedUnitEffectOperationPlan> {
        let selected = match &self.terminator {
            CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } => {
                return_values.as_slice()
            }
            CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                return_arm: CheckedConditionalReturnArm::Structural(operation),
                ..
            } => std::slice::from_ref(operation),
            _ => &[],
        };
        self.operations.iter().chain(selected)
    }

    /// Control-flow successors of this state's terminator, in terminator
    /// order. Return, crash, and value exits contribute no edges.
    pub fn successors(&self) -> Vec<&CheckedStructuralControlSuccessorPlan> {
        match &self.terminator {
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. }
            | CheckedComposedUnitControlTerminatorPlan::Guarded { .. }
            | CheckedComposedUnitControlTerminatorPlan::ReturnCase { .. }
            | CheckedComposedUnitControlTerminatorPlan::Crash { .. }
            | CheckedComposedUnitControlTerminatorPlan::ReturnStructural { .. } => Vec::new(),
            CheckedComposedUnitControlTerminatorPlan::Jump { successor } => vec![successor],
            CheckedComposedUnitControlTerminatorPlan::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true, when_false],
            CheckedComposedUnitControlTerminatorPlan::ConditionalReturn { jump, .. } => vec![jump],
            CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback } => arms
                .iter()
                .map(|arm| &arm.successor)
                .chain(std::iter::once(fallback))
                .collect(),
            CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } => {
                cases.iter().map(|case| &case.successor).collect()
            }
        }
    }

    /// Reachable-state mask over `states` in authored roster order: `true`
    /// exactly at positions reachable from `states[0]` through terminator
    /// successors. `None` when an edge names a target absent from the roster.
    /// Callers decide whether an unreachable or missing target declines the
    /// plan or is pruned.
    pub fn live_mask(states: &[Self]) -> Option<Vec<bool>> {
        let mut visited = vec![false; states.len()];
        if states.is_empty() {
            return Some(visited);
        }
        let mut ready = vec![0_usize];
        visited[0] = true;
        let mut next = 0;
        while let Some(source) = ready.get(next).copied() {
            next += 1;
            for successor in states[source].successors() {
                let target = states
                    .iter()
                    .position(|state| state.state == successor.target_state)?;
                if !visited[target] {
                    visited[target] = true;
                    ready.push(target);
                }
            }
        }
        Some(visited)
    }
}

/// One guarded arm of an ordered transition chain: the exact checked scalar
/// expression the authored guard selected and the selected edge's custody plan.
/// The value arm of a `ConditionalReturn`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedConditionalReturnArm {
    /// The arm's `EstablishStructuralValue` producer, in the same local child
    /// storage `Guarded::return_values` uses.
    Structural(CheckedUnitEffectOperationPlan),
    /// The value checking recorded under the `Return` role at the arm's
    /// transition, evaluated only on that arm.
    Scalar {
        statement_ordinal: u32,
        primitive_type: PrimitiveType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedGuardedJumpPlan {
    /// Exact checked Boolean value at `successor.statement_ordinal` under the
    /// `Guard` role: the retained pure expression or that coordinate's unique
    /// computation root.
    pub guard: CheckedCallScalarArgument,
    pub successor: CheckedStructuralControlSuccessorPlan,
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

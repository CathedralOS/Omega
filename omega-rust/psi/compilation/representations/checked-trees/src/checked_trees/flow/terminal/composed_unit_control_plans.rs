//! Composed unit control plans: state plans, natural ranks, case returns and
//! closed-sum successors.

use crate::checked_trees::flow::terminal::{
    CheckedCallScalarArgument, CheckedProviderAttachmentRequirementPlan,
    CheckedReferenceResultSourcePlan, CheckedScalarBinding, CheckedScalarBranchDestination,
    CheckedScalarGuardedExit, CheckedStructuralControlSuccessorPlan, CheckedStructuralResultPlan,
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
    /// For a structural result holding references: each reference leaf of
    /// the result and the entry parameter it borrows from, as the ordinary
    /// structural return names them. Empty for a reference-free result.
    pub result_reference_sources: Vec<CheckedReferenceResultSourcePlan>,
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

/// One cyclic state's rank, stated over that state's own parameters. Emission
/// evaluates the same measure on the state's entry and, from the actual
/// arguments, on every arriving successor edge; the verifier substitutes one
/// into the other and proves each edge's comparison. `parameter` is the
/// measured subject — for a distance, its climbing lower subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStateNaturalRank {
    pub state: SymbolHandle,
    pub parameter: SymbolHandle,
    pub parameter_position: u32,
    pub measure: CheckedNaturalRankMeasure,
}

/// The measures the termination checker proves, each a total function of
/// one state's parameters. A new order the checker learns belongs here as
/// one more measure, not as another producer route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedNaturalRankMeasure {
    /// `Slice::Length`: the subject slice's live byte length.
    ByteSequenceLength,
    /// `Nat::Descending`: the fixed-width integer subject itself, signed or
    /// unsigned. The checker proves it descends on every cyclic edge.
    IntegerParameter { primitive_type: PrimitiveType },
    /// `Nat::BoundedDistance` and `Nat::IncreasingTo(limit)`: the natural
    /// distance from the climbing lower subject (`parameter`) up to `upper`.
    /// Both are unsigned parameters of `primitive_type`; every cyclic edge
    /// advances the lower subject and keeps `upper`.
    UnsignedDistance {
        primitive_type: PrimitiveType,
        upper: SymbolHandle,
        upper_position: u32,
    },
    /// The same distance with a `u64` cursor and a collection's live length
    /// as its bound. The upper subject is the collection parameter, not a
    /// fabricated scalar parameter for its length. Cyclic edges preserve it.
    CollectionLengthDistance {
        collection: SymbolHandle,
        collection_position: u32,
    },
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
    /// value or Unit completion: that arm returns instead of
    /// transferring to a named state. `jump` carries the named arm's ordinary
    /// custody plan; `return_arm` is Unit, structural or scalar as the
    /// state's result is. `return_when_true` records which arm position the
    /// authored return target occupied. (A conditional whose two arms
    /// are both `(expression)` targets checks as `Guarded` or scalar `Exits`,
    /// never this variant.)
    ConditionalReturn {
        guard: CheckedCallScalarArgument,
        jump: CheckedStructuralControlSuccessorPlan,
        return_arm: CheckedConditionalReturnArm,
        return_when_true: bool,
    },
    /// Ordered Boolean guards each select their arm's edge: a named-state
    /// transfer, or an authored `(expression)` arm that returns its value
    /// like a conditional's return arm. Guards evaluate in authored order —
    /// the same sequencing the shared scalar tail roster records — and the
    /// authored `_` arm is the fallback holding where every guard fails: an
    /// ordinary successor edge, or an authored `(expression)` target that
    /// returns like a conditional's return arm. Each successor carries the
    /// same per-edge custody plan as a conditional edge.
    GuardedJumps {
        arms: Vec<CheckedGuardedJumpArmPlan>,
        fallback: CheckedGuardedJumpFallbackPlan,
    },
    /// The two-way conditional whose arms both complete the machine without
    /// a named-state transfer, at least one through a terminal machine call
    /// (`CheckedConditionalReturnArm::Call`). Each arm runs only when its
    /// guard outcome selects it. `guard` is the same exact checked Boolean
    /// value `Conditional::guard` names, recorded at the true arm's
    /// transition ordinal. (Two `(expression)` arms check as `Guarded`,
    /// never this variant.)
    ConditionalCompletion {
        guard: CheckedCallScalarArgument,
        when_true: CheckedConditionalReturnArm,
        when_false: CheckedConditionalReturnArm,
    },
    /// Consume one whole owned parameter or exact result from this state's
    /// operation prefix and transfer control through the exact closed case
    /// roster. Payload scalars are introduced only on their selected edge;
    /// they are not speculative reads from inactive storage. A case whose
    /// arm is an authored `(expression)` closes its own edge with the
    /// value's producer instead of a transfer.
    ClosedSum {
        subject: CheckedUnitStructuralArgumentPlan,
        cases: Vec<CheckedClosedSumCasePlan>,
    },
    /// Leave checked execution without cleanup, a successor, or an
    /// established value — the standalone authored `crash` exit. The
    /// statement ordinal names its exact authored transition so the crash
    /// contract's checked-site row can be rejoined downstream.
    Crash {
        statement_ordinal: u32,
    },
}

impl CheckedComposedUnitControlTerminatorPlan {
    /// Control-flow successors of this terminator, in terminator order.
    /// Return, crash, and value exits contribute no edges.
    pub fn successors(&self) -> Vec<&CheckedStructuralControlSuccessorPlan> {
        match self {
            Self::ReturnUnit
            | Self::ReturnScalar { .. }
            | Self::Guarded { .. }
            | Self::ReturnCase { .. }
            | Self::Crash { .. }
            | Self::ReturnStructural { .. }
            | Self::ConditionalCompletion { .. } => Vec::new(),
            Self::Jump { successor } => vec![successor],
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true, when_false],
            Self::ConditionalReturn { jump, .. } => vec![jump],
            Self::GuardedJumps { arms, fallback } => arms
                .iter()
                .filter_map(|arm| arm.successor())
                .chain(match fallback {
                    CheckedGuardedJumpFallbackPlan::Successor(successor) => Some(successor),
                    CheckedGuardedJumpFallbackPlan::Return(_) => None,
                })
                .collect(),
            Self::ClosedSum { cases, .. } => cases
                .iter()
                .filter_map(|case| match &case.outcome {
                    CheckedClosedSumCaseOutcomePlan::Successor(successor) => Some(successor),
                    CheckedClosedSumCaseOutcomePlan::Return(_) => None,
                })
                .collect(),
        }
    }

    /// Selected completion arms, in authored order. Each arm retains its own
    /// preparation namespace; these sequences are never concatenated as one
    /// unconditional execution path.
    pub fn return_arms(&self) -> Vec<&CheckedConditionalReturnArm> {
        match self {
            Self::ConditionalReturn { return_arm, .. } => vec![return_arm],
            Self::ConditionalCompletion {
                when_true,
                when_false,
                ..
            } => {
                vec![when_true, when_false]
            }
            Self::GuardedJumps { arms, fallback } => {
                let mut returned: Vec<_> = arms
                    .iter()
                    .filter_map(|arm| match arm {
                        CheckedGuardedJumpArmPlan::Return(plan) => Some(&plan.return_arm),
                        CheckedGuardedJumpArmPlan::Jump(_) => None,
                    })
                    .collect();
                if let CheckedGuardedJumpFallbackPlan::Return(arm) = fallback {
                    returned.push(arm);
                }
                returned
            }
            Self::ClosedSum { cases, .. } => cases
                .iter()
                .filter_map(|case| match &case.outcome {
                    CheckedClosedSumCaseOutcomePlan::Return(arm) => Some(arm),
                    CheckedClosedSumCaseOutcomePlan::Successor(_) => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Mutable completion arms for reconciling their retained calls in place.
    pub fn return_arms_mut(&mut self) -> Vec<&mut CheckedConditionalReturnArm> {
        match self {
            Self::ConditionalReturn { return_arm, .. } => vec![return_arm],
            Self::ConditionalCompletion {
                when_true,
                when_false,
                ..
            } => {
                vec![when_true, when_false]
            }
            Self::GuardedJumps { arms, fallback } => {
                let mut returned: Vec<_> = arms
                    .iter_mut()
                    .filter_map(|arm| match arm {
                        CheckedGuardedJumpArmPlan::Return(plan) => Some(&mut plan.return_arm),
                        CheckedGuardedJumpArmPlan::Jump(_) => None,
                    })
                    .collect();
                if let CheckedGuardedJumpFallbackPlan::Return(arm) = fallback {
                    returned.push(arm);
                }
                returned
            }
            Self::ClosedSum { cases, .. } => cases
                .iter_mut()
                .filter_map(|case| match &mut case.outcome {
                    CheckedClosedSumCaseOutcomePlan::Return(arm) => Some(arm),
                    CheckedClosedSumCaseOutcomePlan::Successor(_) => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Terminal invocations retained by the completion arms, in arm order.
    pub fn arm_calls(&self) -> Vec<&CheckedUnitEffectOperationPlan> {
        self.return_arms()
            .into_iter()
            .filter_map(|arm| match arm {
                CheckedConditionalReturnArm::Call { operation, .. } => Some(operation),
                _ => None,
            })
            .collect()
    }

    /// `arm_calls`, for reconciling each call's operands in place.
    pub fn arm_calls_mut(&mut self) -> Vec<&mut CheckedUnitEffectOperationPlan> {
        self.return_arms_mut()
            .into_iter()
            .filter_map(|arm| match arm {
                CheckedConditionalReturnArm::Call { operation, .. } => Some(operation),
                _ => None,
            })
            .collect()
    }
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
        let selected: Vec<_> = match &self.terminator {
            CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } => {
                return_values.iter().collect()
            }
            _ => self
                .terminator
                .return_arms()
                .into_iter()
                .flat_map(CheckedConditionalReturnArm::operation_dependencies)
                .collect(),
        };
        // A composite argument established on an edge runs its operation
        // there, after selection; it is still one of this state's
        // dependencies, calls included.
        let established = self
            .successors()
            .into_iter()
            .flat_map(|successor| &successor.transfers)
            .filter_map(|transfer| match &transfer.source {
                crate::CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                    operands,
                    establishment,
                } => Some((operands, establishment.as_ref())),
                _ => None,
            })
            .flat_map(|(operands, establishment)| {
                operands.iter().chain(std::iter::once(establishment))
            });

        self.operations.iter().chain(selected).chain(established)
    }

    /// The terminal machine calls this state's conditional arms run
    /// (`CheckedConditionalReturnArm::Call`). Each runs after the state's
    /// whole operation sequence, on its selected arm only.
    pub fn arm_calls(&self) -> Vec<&CheckedUnitEffectOperationPlan> {
        self.terminator.arm_calls()
    }

    /// Control-flow successors of this state's terminator, in terminator
    /// order. Return, crash, and value exits contribute no edges.
    pub fn successors(&self) -> Vec<&CheckedStructuralControlSuccessorPlan> {
        self.terminator.successors()
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

/// The completion arm of a `ConditionalReturn`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedConditionalReturnArm {
    /// Normal Unit completion at this authored transition, with the same
    /// ownership disposal as a Unit-returning state. It is not a jump to a
    /// synthesized source state or a value producer with a dummy scalar.
    Unit { statement_ordinal: u32 },
    /// The arm's `EstablishStructuralValue` producer, in the same local child
    /// storage `Guarded::return_values` uses.
    Structural(CheckedUnitEffectOperationPlan),
    /// The value checking recorded under the `Return` role at the arm's
    /// transition, evaluated only on that arm.
    Scalar {
        statement_ordinal: u32,
        primitive_type: PrimitiveType,
    },
    /// A terminal machine invocation at this authored transition
    /// (state_contracts.md, "Terminal calls in transition arms"): the call
    /// operation runs only on this arm, and its normal result completes the
    /// machine with the disposal a Unit or scalar arm performs. A scalar
    /// result is the call's own binding at the arm's transition, numbered
    /// after the state's retained scalar bindings. The call's coordinate
    /// names the arm's transition. Operand preparation retains the ordinary
    /// call schedule: it runs inside this arm and interleaves with scalar
    /// arguments according to their authored positions.
    Call {
        operands: crate::CheckedOperandSequence,
        operation: CheckedUnitEffectOperationPlan,
    },
}

impl CheckedConditionalReturnArm {
    /// The operation this arm runs when selected: a value producer or a
    /// terminal call. Unit and scalar arms run no operation of their own.
    pub fn operation(&self) -> Option<&CheckedUnitEffectOperationPlan> {
        match self {
            Self::Structural(operation) | Self::Call { operation, .. } => Some(operation),
            Self::Unit { .. } | Self::Scalar { .. } => None,
        }
    }

    /// All selected-arm dependencies, including preparation for its final
    /// invocation. The ordinary argument scheduler owns their execution order.
    pub fn operation_dependencies(&self) -> impl Iterator<Item = &CheckedUnitEffectOperationPlan> {
        let operands = match self {
            Self::Call { operands, .. } => Some(operands),
            _ => None,
        };
        operands
            .into_iter()
            .flat_map(|operands| operands.iter())
            .chain(self.operation())
    }

    /// The authored transition ordinal this arm completes at, when the arm
    /// retains one.
    pub fn statement_ordinal(&self) -> Option<u32> {
        match self {
            Self::Unit { statement_ordinal }
            | Self::Scalar {
                statement_ordinal, ..
            } => Some(*statement_ordinal),
            Self::Structural(
                CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                | CheckedUnitEffectOperationPlan::EstablishReference { result, .. }
                | CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. }
                | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. },
            ) => Some(result.statement_index),
            Self::Structural(_) => None,
            Self::Call { operation, .. } => {
                operation.call().map(|call| call.coordinate.statement_index)
            }
        }
    }
}

/// One guarded arm of an ordered transition chain: the exact checked scalar
/// expression the authored guard selected and the selected edge's custody plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedGuardedJumpPlan {
    /// Exact checked Boolean value at `successor.statement_ordinal` under the
    /// `Guard` role: the retained pure expression or that coordinate's unique
    /// computation root.
    pub guard: CheckedCallScalarArgument,
    pub successor: CheckedStructuralControlSuccessorPlan,
}

/// One arm of an ordered `GuardedJumps` chain — selected by its guard, the
/// arm either transfers to its named state or returns its authored
/// `(expression)` value exactly like the chain's fallback does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedGuardedJumpArmPlan {
    /// A guarded transfer to the arm's selected named state.
    Jump(CheckedGuardedJumpPlan),
    /// A guarded arm whose authored `(expression)` target returns the
    /// established value — the same completion a `ConditionalReturn` arm
    /// carries, still observed only on this arm's true edge.
    Return(CheckedGuardedJumpReturnPlan),
}

/// A guarded chain arm that returns its authored `(expression)` value: the
/// Boolean value observed at `statement_ordinal` under the `Guard` role and
/// the arm's returned-value plan share the one coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedGuardedJumpReturnPlan {
    /// Exact checked Boolean value at `statement_ordinal` under the `Guard`
    /// role: the retained pure expression or that coordinate's unique
    /// computation root — the same record `CheckedGuardedJumpPlan::guard`
    /// names.
    pub guard: CheckedCallScalarArgument,
    /// This arm's authored transition ordinal; the arm's guard row and its
    /// returned value share it.
    pub statement_ordinal: u32,
    /// The arm's returned value, structural or scalar as the state's result is.
    pub return_arm: CheckedConditionalReturnArm,
}

impl CheckedGuardedJumpArmPlan {
    /// The exact Boolean guard this arm is selected under.
    pub fn guard(&self) -> &CheckedCallScalarArgument {
        match self {
            CheckedGuardedJumpArmPlan::Jump(plan) => &plan.guard,
            CheckedGuardedJumpArmPlan::Return(plan) => &plan.guard,
        }
    }

    /// The authored transition ordinal this arm answers.
    pub fn statement_ordinal(&self) -> u32 {
        match self {
            CheckedGuardedJumpArmPlan::Jump(plan) => plan.successor.statement_ordinal,
            CheckedGuardedJumpArmPlan::Return(plan) => plan.statement_ordinal,
        }
    }

    /// The arm's named-state successor plan, when it transfers; a return arm
    /// has no successor edge.
    pub fn successor(&self) -> Option<&CheckedStructuralControlSuccessorPlan> {
        match self {
            CheckedGuardedJumpArmPlan::Jump(plan) => Some(&plan.successor),
            CheckedGuardedJumpArmPlan::Return(_) => None,
        }
    }
}

/// The closing arm of a `GuardedJumps` tail — the edge every prior guard
/// falling through reaches: an ordinary named-state successor, or an authored
/// `(expression)` arm that returns its value exactly like a
/// `ConditionalReturn` arm. The shared scalar tail roster already proved the
/// arm's coverage, so a value fallback needs no guard of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedGuardedJumpFallbackPlan {
    Successor(CheckedStructuralControlSuccessorPlan),
    Return(CheckedConditionalReturnArm),
}

impl CheckedGuardedJumpFallbackPlan {
    /// The authored transition ordinal this closing arm answers.
    pub fn statement_ordinal(&self) -> u32 {
        match self {
            CheckedGuardedJumpFallbackPlan::Successor(successor) => successor.statement_ordinal,
            CheckedGuardedJumpFallbackPlan::Return(return_arm) => {
                return_arm.statement_ordinal().unwrap_or(0)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedClosedSumCasePlan {
    pub case_identity: String,
    /// One case edge's checked outcome: an ordinary transfer to the case's
    /// named successor state, or the authored `(expression)` arm's returned
    /// value, produced on this case's edge block only.
    pub outcome: CheckedClosedSumCaseOutcomePlan,
    pub payloads: Vec<CheckedClosedSumPayloadTransferPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedClosedSumCaseOutcomePlan {
    Successor(CheckedStructuralControlSuccessorPlan),
    /// The arm's `(expression)` value — the same plan a `ConditionalReturn`
    /// arm carries. Payload fields are never in scope for it: a return arm
    /// has no argument list the checker could attribute the read through.
    Return(CheckedConditionalReturnArm),
}

impl CheckedClosedSumCasePlan {
    /// The authored transition ordinal this case edge answers.
    pub fn statement_ordinal(&self) -> u32 {
        match &self.outcome {
            CheckedClosedSumCaseOutcomePlan::Successor(successor) => successor.statement_ordinal,
            CheckedClosedSumCaseOutcomePlan::Return(return_arm) => {
                return_arm.statement_ordinal().unwrap_or(0)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedClosedSumPayloadTransferPlan {
    pub field_identity: String,
    pub primitive_type: PrimitiveType,
    pub target_scalar_parameter_index: u32,
}

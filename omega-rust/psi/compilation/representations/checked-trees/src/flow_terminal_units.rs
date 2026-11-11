/*
We define checked Unit-machine Terminal lowering plans, including composed unit
control (`composed_unit_control_plans`), unit calls (`unit_call_plans`), unit
effect operations (`unit_effect_plans`), and `CheckedUnitPlan` (`unit_plans`) in
`CheckedTrees`.
*/

pub(crate) mod composed_unit_control_plans {

    use crate::CheckedMemberIdentity;
    use crate::flow::terminal::{
        CheckedCallScalarArgument, CheckedScalarBranchDestination, CheckedScalarGuardedExit,
        CheckedStructuralControlSuccessorPlan, CheckedStructuralResultPlan,
        CheckedStructuralScalarParameterPlan, CheckedUnitEffectOperationPlan,
        CheckedUnitEntryClaimPlan, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralParameterPlan,
    };
    use crate::values::CheckedErasedProofParameterPlan;
    use crate::{CheckedScalarExpression, ClosedScalarContractValue};
    use language_semantics::{SemanticDomainId, ServiceReachPlan, ServiceReachSummary};
    use semantic_vocabulary::BoundaryCompletion::External;
    use symbols::SymbolHandle;
    use typed_trees::type_identity::TypeIdentityId;
    use typed_trees::types::PrimitiveType;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedComposedUnitControlMachinePlan {
        pub machine: SymbolHandle,
        pub result: CheckedControlResultPlan,
        pub result_reference_sources: Vec<crate::flow::terminal::CheckedReturnedReferencePlan>,
        pub natural_ranks: Vec<CheckedStateNaturalRank>,
        pub rank_components: arena::Arena<CheckedIntegerRankObservation>,
        pub preserving_rank_edges: Vec<CheckedRankEdgeCoordinate>,
        pub body_qualifications: Vec<SemanticDomainId>,
        pub contract_report_fingerprint: u64,
        pub contract_commitment: crate::MachineContractCommitment,
        pub contract_service_reach: ServiceReachPlan,
        pub service_reach: ServiceReachSummary,
        pub states: Vec<CheckedComposedUnitControlStatePlan>,
        /*
            The closed `requires` clauses that parameter data declarations
            impose on a state's arrival, stored once per distinct set. Every
            state of one machine usually takes the same parameters, so its
            declaration rows are identical; a large record such as a parser
            carries hundreds of them, and a copy per state multiplied each
            plan by its state count. A state names its set by
            `declaration_requires`.
        */
        pub declaration_requirements: Vec<CheckedDeclarationRequirementSet>,
    }

    /* One distinct set of declaration-imposed state requirements; see above. */
    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedDeclarationRequirementSet {
        pub clauses: Vec<ClosedScalarContractValue>,
    }

    impl CheckedComposedUnitControlMachinePlan {
        pub fn state_position(&self, state: SymbolHandle) -> Option<usize> {
            state_position(&self.states, state)
        }

        /*
            The closed clauses a state's arrival must satisfy beyond its
            authored rows: its parameters' declaration requirements, in the
            order the declarations state them, or none when the state names no
            set. Checking interns every set a state names in this same plan,
            and lowering consumes the plan without re-checking it, so the
            index always resolves.
        */
        pub fn declaration_requirements(
            &self,
            state: &CheckedComposedUnitControlStatePlan,
        ) -> &[ClosedScalarContractValue] {
            state.declaration_requires.map_or(&[], |set| {
                &self.declaration_requirements[set as usize].clauses
            })
        }

        pub fn successor_position(
            &self,
            successor: &CheckedStructuralControlSuccessorPlan,
        ) -> Option<usize> {
            successor_position(&self.states, successor)
        }
    }

    fn successor_position(
        states: &[CheckedComposedUnitControlStatePlan],
        successor: &CheckedStructuralControlSuccessorPlan,
    ) -> Option<usize> {
        match successor.target_occurrence {
            Some(index) => states
                .get(index as usize)
                .filter(|state| state.state == successor.target_state)
                .map(|_| index as usize),
            None => state_position(states, successor.target_state),
        }
    }

    pub fn state_position(
        states: &[CheckedComposedUnitControlStatePlan],
        state: SymbolHandle,
    ) -> Option<usize> {
        states.iter().position(|candidate| candidate.state == state)
    }

    /// One selected return and the payload copies completed before its operands.
    /// The producer joins the exact outgoing target; lowering consumes this list.
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedGuardedReturnValue {
        pub statement_ordinal: u32,
        pub is_continuation: bool,
        pub transition_target: typed_trees::statement::TransitionTargetHandle,
        pub operation: CheckedUnitEffectOperationPlan,
        pub copied_payloads: Vec<crate::CheckedSelectedPayloadCopy>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedControlResultPlan {
        Unit,
        Structural(CheckedStructuralResultPlan),
        Scalar { primitive_type: PrimitiveType },
    }

    impl CheckedControlResultPlan {
        pub fn structural_identity(&self) -> Option<TypeIdentityId> {
            match self {
                Self::Structural(result) => Some(result.type_identity),
                Self::Unit | Self::Scalar { .. } => None,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedScalarCaseFieldPlan {
        pub field_ordinal: u32,
        pub field_identity: CheckedMemberIdentity,
        pub primitive_type: PrimitiveType,
        pub expression: CheckedScalarExpression,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedRankEdgeCoordinate {
        pub source_state: SymbolHandle,
        pub target_state: SymbolHandle,
        pub statement_ordinal: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStateNaturalRank {
        pub state: SymbolHandle,
        pub components: arena::HandleSpan<CheckedIntegerRankObservation>,
    }

    impl Default for CheckedStateNaturalRank {
        fn default() -> Self {
            Self {
                state: SymbolHandle::invalid(),
                components: Default::default(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedIntegerRankObservation {
        pub parameter: SymbolHandle,
        pub parameter_position: u32,
        pub measure: CheckedNaturalRankMeasure,
        pub function: CheckedNaturalRankFunction,
        pub observation: crate::CheckedScalarComputationHandle,
    }

    impl Default for CheckedIntegerRankObservation {
        fn default() -> Self {
            Self {
                parameter: SymbolHandle::invalid(),
                parameter_position: 0,
                measure: CheckedNaturalRankMeasure::SequenceLength {
                    coordinate: crate::CheckedRankCoordinateHandle::invalid(),
                },
                function: CheckedNaturalRankFunction::Identity,
                observation: crate::CheckedScalarComputationHandle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedNaturalRankFunction {
        #[default]
        Identity,
        Complement,
    }

    impl CheckedNaturalRankFunction {
        pub fn complement_ceiling(self, primitive: PrimitiveType) -> Option<u64> {
            match (self, primitive) {
                (Self::Complement, PrimitiveType::U8) => Some(u64::from(u8::MAX)),
                (Self::Complement, PrimitiveType::U16) => Some(u64::from(u16::MAX)),
                (Self::Complement, PrimitiveType::U32) => Some(u64::from(u32::MAX)),
                (Self::Complement, PrimitiveType::U64) => Some(u64::MAX),
                _ => None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedNaturalRankMeasure {
        DeclaredMeasure {
            formation: crate::CheckedRankApplicationFormationHandle,
            measure: SymbolHandle,
            bindings: arena::HandleSpan<crate::CheckedRankCoordinateBinding>,
            primitive_type: PrimitiveType,
        },
        SequenceLength {
            coordinate: crate::CheckedRankCoordinateHandle,
        },
        IntegerValue {
            coordinate: crate::CheckedRankCoordinateHandle,
            primitive_type: PrimitiveType,
        },
        Distance {
            lower: arena::Handle<crate::CheckedRankOperand>,
            upper: arena::Handle<crate::CheckedRankOperand>,
        },
        IncreasingTo {
            primitive_type: PrimitiveType,
            upper: typed_trees::expression::ExpressionHandle,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedStateContractRequirement {
        Closed(ClosedScalarContractValue),
        SourceExpression(typed_trees::expression::ExpressionHandle),
        SourceFact(arena::Handle<typed_trees::domain::ProofFact>),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedComposedUnitControlStatePlan {
        /// Admitted payload births for the exact selected outgoing targets.
        pub selected_copies: Vec<crate::CheckedSelectedPayloadCopies>,
        pub state: SymbolHandle,
        pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
        pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
        pub parameter_storage: Vec<crate::flow::terminal::CheckedUnitParameterStoragePlan>,
        pub erased_scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
        pub erased_proof_parameters: Vec<CheckedErasedProofParameterPlan>,
        /* The state's authored `requires` rows. */
        pub requires: Vec<CheckedStateContractRequirement>,
        /*
            The machine's declaration requirement set this state's arrival
            also satisfies, after `requires`; see
            `CheckedComposedUnitControlMachinePlan::declaration_requirements`.
        */
        pub declaration_requires: Option<u32>,
        pub entry_claims: Vec<CheckedUnitEntryClaimPlan>,
        pub operations: Vec<CheckedUnitEffectOperationPlan>,
        pub place_loans: Vec<crate::flow::terminal::CheckedPlaceLoanPlan>,
        pub terminator: CheckedComposedUnitControlTerminatorPlan,
        pub terminator_statement_ordinal: u32,
        pub statement_count: u32,
        pub selected_exit: Option<crate::FlowClaimJoinExit>,
        pub normal_exit_cleanups: crate::flow::terminal::CheckedNormalExitCleanups,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedComposedUnitControlTerminatorPlan {
        InvokeNoNormal {
            coordinate: crate::flow::terminal::CheckedUnitCallCoordinate,
        },
        EvaluateNoNormal,
        ReturnUnit,
        /// A result-less expression selects ordinary call completions. Unit has
        /// no fabricated scalar or structural return home.
        CompleteExpression {
            completion: CheckedUnitCompletionPlan,
        },
        /// Complete a scalar-result machine with the same completion an ordinary
        /// single-state body retains; see `CheckedScalarReturnPlan`.
        ReturnScalar {
            completion: CheckedScalarReturnPlan,
        },
        ReturnStructural {
            result: crate::CheckedUnitStructuralReturnPlan,
        },
        Guarded {
            arms: arena::HandleSpan<CheckedScalarGuardedExit>,
            fallback: Option<CheckedScalarBranchDestination>,
            return_values: Vec<CheckedGuardedReturnValue>,
        },
        Jump {
            successor: CheckedStructuralControlSuccessorPlan,
        },
        Conditional {
            guard: CheckedCallScalarArgument,
            when_true: CheckedStructuralControlSuccessorPlan,
            when_false: CheckedStructuralControlSuccessorPlan,
        },
        ConditionalReturn {
            guard: CheckedCallScalarArgument,
            jump: CheckedStructuralControlSuccessorPlan,
            return_arm: CheckedConditionalReturnArm,
            return_when_true: bool,
        },
        GuardedJumps {
            arms: Vec<CheckedGuardedJumpArmPlan>,
            fallback: CheckedGuardedJumpFallbackPlan,
        },
        ConditionalCompletion {
            guard: CheckedCallScalarArgument,
            when_true: CheckedConditionalReturnArm,
            when_false: CheckedConditionalReturnArm,
        },
        ClosedSum {
            subject: CheckedUnitStructuralArgumentPlan,
            subject_type: crate::types::TypeReferenceHandle,
            subject_kind: CheckedClosedSumSubjectKind,
            cases: Vec<CheckedClosedSumCasePlan>,
        },
        Crash {
            statement_ordinal: u32,
        },
    }

    impl CheckedComposedUnitControlTerminatorPlan {
        pub fn has_normal_continuation(
            &self,
            operations: &[CheckedUnitEffectOperationPlan],
            guarded_exits: &crate::CheckedGuardedExitPlans,
        ) -> Option<bool> {
            if operations
                .iter()
                .any(|operation| !operation.has_normal_continuation())
            {
                return Some(false);
            }
            Some(match self {
                Self::CompleteExpression { completion } => {
                    completion.expression.normal_continuation
                }
                Self::InvokeNoNormal { .. } | Self::EvaluateNoNormal | Self::Crash { .. } => false,
                Self::Guarded {
                    arms,
                    fallback,
                    return_values,
                } => {
                    let arms = guarded_exits.exits.span(*arms)?;
                    if return_values.len() != arms.len() + usize::from(fallback.is_some()) {
                        return None;
                    }
                    let destinations = arms
                        .iter()
                        .map(|arm| &arm.destination)
                        .chain(fallback.iter());
                    let mut normal = false;
                    for (destination, returned) in destinations.zip(return_values) {
                        let CheckedScalarBranchDestination::Return {
                            statement_ordinal,
                            is_continuation,
                        } = destination
                        else {
                            return None;
                        };
                        if returned.statement_ordinal != *statement_ordinal
                            || returned.is_continuation != *is_continuation
                            || returned.operation.source_statement_index()
                                != Some(*statement_ordinal)
                        {
                            return None;
                        }
                        normal |= returned.operation.has_normal_continuation();
                    }
                    normal
                }
                Self::ConditionalCompletion {
                    when_true,
                    when_false,
                    ..
                } => when_true.has_normal_continuation() || when_false.has_normal_continuation(),
                Self::GuardedJumps { arms, fallback } => {
                    arms.iter().any(|arm| match arm {
                        CheckedGuardedJumpArmPlan::Return(plan) => {
                            plan.return_arm.has_normal_continuation()
                        }
                        CheckedGuardedJumpArmPlan::Jump(plan) => {
                            plan.successor.has_normal_continuation()
                        }
                    }) || match fallback {
                        CheckedGuardedJumpFallbackPlan::Return(arm) => {
                            arm.has_normal_continuation()
                        }
                        CheckedGuardedJumpFallbackPlan::Successor(successor) => {
                            successor.has_normal_continuation()
                        }
                    }
                }
                Self::ClosedSum { cases, .. } => cases.iter().any(|case| match &case.outcome {
                    CheckedClosedSumCaseOutcomePlan::Return(arm) => arm.has_normal_continuation(),
                    CheckedClosedSumCaseOutcomePlan::Successor(successor) => {
                        successor.has_normal_continuation()
                    }
                }),
                Self::Jump { successor } => successor.has_normal_continuation(),
                Self::Conditional {
                    when_true,
                    when_false,
                    ..
                } => when_true.has_normal_continuation() || when_false.has_normal_continuation(),
                Self::ConditionalReturn {
                    jump, return_arm, ..
                } => jump.has_normal_continuation() || return_arm.has_normal_continuation(),
                _ => true,
            })
        }

        pub fn abandons_custody(
            &self,
            operations: &[CheckedUnitEffectOperationPlan],
            guarded_exits: &crate::CheckedGuardedExitPlans,
        ) -> Option<bool> {
            if has_external_completion(operations) {
                return Some(true);
            }
            let abandons = CheckedConditionalReturnArm::abandons_custody;
            Some(match self {
                Self::CompleteExpression { completion } => completion.abandons_custody,
                Self::Crash { .. } => true,
                Self::ConditionalCompletion {
                    when_true,
                    when_false,
                    ..
                } => abandons(when_true) && abandons(when_false),
                Self::Guarded { arms, fallback, .. } => {
                    let arms = guarded_exits.exits.span(*arms)?;
                    (!arms.is_empty() || fallback.is_some())
                        && arms.iter().all(|arm| {
                            matches!(
                                arm.destination,
                                CheckedScalarBranchDestination::Crash { .. }
                            )
                        })
                        && fallback.as_ref().is_none_or(|destination| {
                            matches!(destination, CheckedScalarBranchDestination::Crash { .. })
                        })
                }
                Self::GuardedJumps { arms, fallback } => arms.iter().all(|arm| {
                    matches!(
                        arm, CheckedGuardedJumpArmPlan::Return(plan) if abandons(&plan.return_arm)
                    )
                }) && matches!(fallback,
                    CheckedGuardedJumpFallbackPlan::Return(arm) if abandons(arm)
                ),
                Self::ClosedSum { cases, .. } => !cases.is_empty() && cases.iter().all(|case| {
                    matches!(
                        &case.outcome, CheckedClosedSumCaseOutcomePlan::Return(arm) if abandons(arm)
                    )
                }),
                Self::InvokeNoNormal { .. }
                | Self::EvaluateNoNormal
                | Self::ReturnUnit
                | Self::ReturnScalar { .. }
                | Self::ReturnStructural { .. }
                | Self::Jump { .. }
                | Self::Conditional { .. }
                | Self::ConditionalReturn { .. } => false,
            })
        }

        pub fn all_paths_crash(
            &self,
            operations: &[CheckedUnitEffectOperationPlan],
            guarded_exits: &crate::CheckedGuardedExitPlans,
        ) -> Option<bool> {
            if has_external_completion(operations)
                || matches!(self, Self::CompleteExpression { completion } if completion.expression.invocation_scopes().into_iter().any(|operations| has_external_completion(operations.iter())))
                || self
                    .return_arms()
                    .iter()
                    .any(|arm| has_external_completion(arm.operation_dependencies()))
            {
                return Some(false);
            }
            self.abandons_custody(operations, guarded_exits)
        }

        pub fn successors(&self) -> Vec<&CheckedStructuralControlSuccessorPlan> {
            match self {
                Self::CompleteExpression { .. }
                | Self::InvokeNoNormal { .. }
                | Self::EvaluateNoNormal
                | Self::ReturnUnit
                | Self::ReturnScalar { .. }
                | Self::Guarded { .. }
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

        pub fn successors_mut(&mut self) -> Vec<&mut CheckedStructuralControlSuccessorPlan> {
            match self {
                Self::Jump { successor } => vec![successor],
                Self::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![when_true, when_false],
                Self::ConditionalReturn { jump, .. } => vec![jump],
                Self::GuardedJumps { arms, fallback } => arms
                    .iter_mut()
                    .filter_map(|arm| match arm {
                        CheckedGuardedJumpArmPlan::Jump(plan) => Some(&mut plan.successor),
                        CheckedGuardedJumpArmPlan::Return(_) => None,
                    })
                    .chain(match fallback {
                        CheckedGuardedJumpFallbackPlan::Successor(successor) => Some(successor),
                        CheckedGuardedJumpFallbackPlan::Return(_) => None,
                    })
                    .collect(),
                Self::ClosedSum { cases, .. } => cases
                    .iter_mut()
                    .filter_map(|case| match &mut case.outcome {
                        CheckedClosedSumCaseOutcomePlan::Successor(successor) => Some(successor),
                        CheckedClosedSumCaseOutcomePlan::Return(_) => None,
                    })
                    .collect(),
                Self::CompleteExpression { .. }
                | Self::InvokeNoNormal { .. }
                | Self::EvaluateNoNormal
                | Self::ReturnUnit
                | Self::ReturnScalar { .. }
                | Self::Guarded { .. }
                | Self::Crash { .. }
                | Self::ReturnStructural { .. }
                | Self::ConditionalCompletion { .. } => Vec::new(),
            }
        }

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
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedScalarReturnPlan {
        Binding(crate::CheckedUnitScalarResultBindingPlan),
        Exits(crate::CheckedUnitScalarControlPlan),
    }

    impl CheckedComposedUnitControlStatePlan {
        pub fn operation_dependencies(
            &self,
        ) -> impl Iterator<Item = &CheckedUnitEffectOperationPlan> {
            let selected: Vec<_> = match &self.terminator {
                CheckedComposedUnitControlTerminatorPlan::CompleteExpression { completion } => {
                    completion
                        .expression
                        .invocation_scopes()
                        .into_iter()
                        .flat_map(|operations| operations.iter())
                        .collect()
                }
                CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } => {
                    return_values
                        .iter()
                        .map(|returned| &returned.operation)
                        .collect()
                }
                _ => self
                    .terminator
                    .return_arms()
                    .into_iter()
                    .flat_map(CheckedConditionalReturnArm::operation_dependencies)
                    .collect(),
            };
            let established = self
                .successors()
                .into_iter()
                .flat_map(|successor| successor.operand_operations());

            self.operations.iter().chain(selected).chain(established)
        }

        pub fn successors(&self) -> Vec<&CheckedStructuralControlSuccessorPlan> {
            self.terminator.successors()
        }

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
                    let target = successor_position(states, successor)?;
                    if !visited[target] {
                        visited[target] = true;
                        ready.push(target);
                    }
                }
            }
            Some(visited)
        }
    }

    /// A result-less selection owns each selected call transaction. The source
    /// handle records provenance; consumers do not join it back to a call roster.
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitCompletionPlan {
        pub statement_ordinal: u32,
        pub expression: CheckedUnitCompletionExpression,
        pub abandons_custody: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitCompletionExpression {
        pub source: crate::CheckedCompletionExpressionHandle,
        pub expression: typed_trees::expression::ExpressionHandle,
        pub normal_continuation: bool,
        pub kind: CheckedUnitCompletionKind,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitCompletionKind {
        Invocation(crate::CheckedOperandSequence),
        Selection(CheckedUnitCompletionSelection),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitCompletionSelection {
        Scalar {
            subject: crate::CheckedScalarComputationHandle,
            arms: Vec<CheckedUnitScalarCompletionArm>,
        },
        Structural {
            expression: typed_trees::expression::ExpressionHandle,
            value: crate::CheckedStructuralValueHandle,
            place: crate::CheckedUnitStructuralArgumentPlan,
            alias: typed_trees::expression::ExpressionHandle,
            arms: Vec<CheckedUnitStructuralCompletionArm>,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitScalarCompletionArm {
        pub source_arm: arena::Handle<typed_trees::expression::TableMatchArm>,
        pub condition: CheckedUnitScalarCompletionCondition,
        pub completion: CheckedUnitCompletionExpression,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitScalarCompletionCondition {
        Boolean(crate::CheckedScalarComputationHandle),
        Integer(crate::CheckedScalarComputationHandle),
        Predicate(crate::CheckedScalarComputationHandle),
        Default,
    }

    impl CheckedUnitScalarCompletionCondition {
        pub fn computation(&self) -> Option<crate::CheckedScalarComputationHandle> {
            match self {
                Self::Boolean(value) | Self::Integer(value) | Self::Predicate(value) => {
                    Some(*value)
                }
                Self::Default => None,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitStructuralCompletionArm {
        pub source_arm: arena::Handle<typed_trees::expression::TableMatchArm>,
        pub predicate: Option<crate::CheckedScalarComputationHandle>,
        pub completion: CheckedUnitCompletionExpression,
    }

    impl CheckedUnitCompletionExpression {
        pub fn subject(&self) -> Option<crate::CheckedDispatchSubject> {
            let CheckedUnitCompletionKind::Selection(selection) = &self.kind else {
                return None;
            };
            Some(match selection {
                CheckedUnitCompletionSelection::Scalar { subject, .. } => {
                    crate::CheckedDispatchSubject::Scalar(*subject)
                }
                CheckedUnitCompletionSelection::Structural {
                    expression,
                    value,
                    place,
                    alias,
                    ..
                } => crate::CheckedDispatchSubject::Structural {
                    expression: *expression,
                    value: *value,
                    place: place.clone(),
                    alias: *alias,
                },
            })
        }

        pub fn arms(
            &self,
        ) -> impl DoubleEndedIterator<Item = (Option<crate::CheckedScalarComputationHandle>, &Self)>
        {
            let (scalar, structural) = match &self.kind {
                CheckedUnitCompletionKind::Invocation(_) => (None, None),
                CheckedUnitCompletionKind::Selection(CheckedUnitCompletionSelection::Scalar {
                    arms,
                    ..
                }) => (Some(arms.as_slice()), None),
                CheckedUnitCompletionKind::Selection(
                    CheckedUnitCompletionSelection::Structural { arms, .. },
                ) => (None, Some(arms.as_slice())),
            };
            scalar
                .into_iter()
                .flatten()
                .map(|arm| (arm.condition.computation(), &arm.completion))
                .chain(
                    structural
                        .into_iter()
                        .flatten()
                        .map(|arm| (arm.predicate, &arm.completion)),
                )
        }

        pub fn invocation_scopes(&self) -> Vec<&crate::CheckedOperandSequence> {
            let mut pending = vec![self];
            let mut scopes = Vec::new();
            while let Some(expression) = pending.pop() {
                match &expression.kind {
                    CheckedUnitCompletionKind::Invocation(operations) => scopes.push(operations),
                    CheckedUnitCompletionKind::Selection(
                        CheckedUnitCompletionSelection::Scalar { arms, .. },
                    ) => pending.extend(arms.iter().rev().map(|arm| &arm.completion)),
                    CheckedUnitCompletionKind::Selection(
                        CheckedUnitCompletionSelection::Structural { arms, .. },
                    ) => pending.extend(arms.iter().rev().map(|arm| &arm.completion)),
                }
            }
            scopes
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedConditionalReturnArm {
        Completion(CheckedUnitCompletionPlan),
        /// Ordinary prepared calls complete without a materialized result value.
        EvaluateUnit {
            statement_ordinal: u32,
            operations: crate::CheckedOperandSequence,
        },
        /// A required operand stops before this arm's enclosing invocation enters.
        /// The exact evaluated prefix owns its calls; no parent call is published.
        EvaluateNoNormal {
            statement_ordinal: u32,
            operations: crate::CheckedOperandSequence,
        },
        /// Selected abnormal completion. The exact authored crash site owns its
        /// cause and frontier; it supplies neither a result nor normal cleanup.
        Crash {
            statement_ordinal: u32,
        },
        /// Normal Unit completion at this authored transition, with the same
        /// ownership disposal as a Unit-returning state. It is not a jump to a
        /// synthesized source state or a value producer with a dummy scalar.
        Unit {
            statement_ordinal: u32,
        },
        /// The arm's `EstablishStructuralValue` producer, in the same local child
        /// storage `Guarded::return_values` uses.
        Structural(CheckedUnitEffectOperationPlan),
        Scalar {
            statement_ordinal: u32,
            primitive_type: PrimitiveType,
        },
        Call {
            operands: crate::CheckedOperandSequence,
            operation: CheckedUnitEffectOperationPlan,
            completions: crate::CheckedOperandSequence,
        },
    }

    impl CheckedConditionalReturnArm {
        pub fn has_normal_continuation(&self) -> bool {
            if let Self::Completion(plan) = self {
                return plan.expression.normal_continuation;
            }
            !matches!(self, Self::Crash { .. } | Self::EvaluateNoNormal { .. })
                && self
                    .operation_dependencies()
                    .all(CheckedUnitEffectOperationPlan::has_normal_continuation)
        }

        pub fn abandons_custody(&self) -> bool {
            if let Self::Completion(plan) = self {
                return plan.abandons_custody;
            }
            matches!(self, Self::Crash { .. })
                || has_external_completion(self.operation_dependencies())
        }

        pub fn operation(&self) -> Option<&CheckedUnitEffectOperationPlan> {
            match self {
                Self::Structural(operation) | Self::Call { operation, .. } => Some(operation),
                Self::Completion(_)
                | Self::Crash { .. }
                | Self::Unit { .. }
                | Self::Scalar { .. }
                | Self::EvaluateNoNormal { .. }
                | Self::EvaluateUnit { .. } => None,
            }
        }

        pub fn operation_dependencies(
            &self,
        ) -> impl Iterator<Item = &CheckedUnitEffectOperationPlan> {
            let operands = match self {
                Self::Call { operands, .. } => Some(operands),
                Self::EvaluateNoNormal { operations, .. }
                | Self::EvaluateUnit { operations, .. } => Some(operations),
                _ => None,
            };
            let completions = match self {
                Self::Completion(plan) => Some(plan),
                _ => None,
            };
            completions
                .into_iter()
                .flat_map(|plan| plan.expression.invocation_scopes())
                .flat_map(|operations| operations.iter())
                .chain(
                    operands
                        .into_iter()
                        .flat_map(|operands| operands.iter())
                        .chain(self.operation())
                        .chain(
                            match self {
                                Self::Call { completions, .. } => Some(completions),
                                _ => None,
                            }
                            .into_iter()
                            .flat_map(|completions| completions.iter()),
                        ),
                )
        }

        /// Mutually exclusive calls may reuse private result ordinals. Catalog
        /// consumers prepare each invocation against the same predecessor scope.
        pub fn operation_scopes(&self) -> Vec<Vec<&CheckedUnitEffectOperationPlan>> {
            match self {
                Self::Completion(plan) => plan
                    .expression
                    .invocation_scopes()
                    .into_iter()
                    .map(|operations| operations.iter().collect())
                    .collect(),
                _ => vec![self.operation_dependencies().collect()],
            }
        }

        /// The authored transition ordinal this arm completes at, when the arm
        /// retains one.
        pub fn statement_ordinal(&self) -> Option<u32> {
            match self {
                Self::Completion(plan) => Some(plan.statement_ordinal),
                Self::EvaluateUnit {
                    statement_ordinal, ..
                } => Some(*statement_ordinal),
                Self::Crash { statement_ordinal }
                | Self::Unit { statement_ordinal }
                | Self::EvaluateNoNormal {
                    statement_ordinal, ..
                }
                | Self::Scalar {
                    statement_ordinal, ..
                } => Some(*statement_ordinal),
                Self::Structural(
                    CheckedUnitEffectOperationPlan::EstablishLocal(
                        crate::CheckedUnitLocalBindingPlan::StructuralValue { result, .. }
                        | crate::CheckedUnitLocalBindingPlan::ByteSequence { result, .. },
                    )
                    | CheckedUnitEffectOperationPlan::EstablishReference { result, .. }
                    | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. },
                ) => Some(result.statement_index),
                Self::Structural(operation) | Self::Call { operation, .. } => {
                    operation.source_statement_index()
                }
            }
        }
    }

    fn has_external_completion<'operation>(
        operations: impl IntoIterator<Item = &'operation CheckedUnitEffectOperationPlan>,
    ) -> bool {
        operations
            .into_iter()
            .filter_map(CheckedUnitEffectOperationPlan::call)
            .any(|call| matches!(call.completion(), External(_)))
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedGuardedJumpPlan {
        pub guard: CheckedCallScalarArgument,
        pub successor: CheckedStructuralControlSuccessorPlan,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedGuardedJumpArmPlan {
        Jump(CheckedGuardedJumpPlan),
        Return(CheckedGuardedJumpReturnPlan),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedGuardedJumpReturnPlan {
        pub guard: CheckedCallScalarArgument,
        pub statement_ordinal: u32,
        pub return_arm: CheckedConditionalReturnArm,
    }

    impl CheckedGuardedJumpArmPlan {
        pub fn guard(&self) -> &CheckedCallScalarArgument {
            match self {
                CheckedGuardedJumpArmPlan::Jump(plan) => &plan.guard,
                CheckedGuardedJumpArmPlan::Return(plan) => &plan.guard,
            }
        }

        pub fn statement_ordinal(&self) -> u32 {
            match self {
                CheckedGuardedJumpArmPlan::Jump(plan) => plan.successor.statement_ordinal,
                CheckedGuardedJumpArmPlan::Return(plan) => plan.statement_ordinal,
            }
        }

        pub fn successor(&self) -> Option<&CheckedStructuralControlSuccessorPlan> {
            match self {
                CheckedGuardedJumpArmPlan::Jump(plan) => Some(&plan.successor),
                CheckedGuardedJumpArmPlan::Return(_) => None,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedGuardedJumpFallbackPlan {
        Successor(CheckedStructuralControlSuccessorPlan),
        Return(CheckedConditionalReturnArm),
    }

    impl CheckedGuardedJumpFallbackPlan {
        pub fn statement_ordinal(&self) -> u32 {
            match self {
                CheckedGuardedJumpFallbackPlan::Successor(successor) => successor.statement_ordinal,
                CheckedGuardedJumpFallbackPlan::Return(return_arm) => {
                    return_arm.statement_ordinal().unwrap_or(0)
                }
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedClosedSumSubjectKind {
        #[default]
        Place,
        Computed,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedClosedSumCasePlan {
        pub case_identity: CheckedMemberIdentity,
        pub outcome: CheckedClosedSumCaseOutcomePlan,
        pub payloads: Vec<CheckedClosedSumPayloadTransferPlan>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedClosedSumCaseOutcomePlan {
        Successor(CheckedStructuralControlSuccessorPlan),
        Return(CheckedConditionalReturnArm),
    }

    impl CheckedClosedSumCasePlan {
        pub fn statement_ordinal(&self) -> u32 {
            match &self.outcome {
                CheckedClosedSumCaseOutcomePlan::Successor(successor) => {
                    successor.statement_ordinal
                }
                CheckedClosedSumCaseOutcomePlan::Return(return_arm) => {
                    return_arm.statement_ordinal().unwrap_or(0)
                }
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedClosedSumPayloadTransferPlan {
        pub field_identity: CheckedMemberIdentity,
        pub primitive_type: PrimitiveType,
        pub target_scalar_parameter_index: u32,
    }
}

pub(crate) mod unit_call_plans {
    use language_semantics::ServiceReachSummary;
    use semantic_vocabulary::BoundaryCompletion;
    use symbols::SymbolHandle;

    use crate::flow::terminal::{
        CheckedCallScalarArgument, CheckedStructuralCallCustodyPlan, CheckedUnitCallCoordinate,
        CheckedUnitClaimTransferPlan, CheckedUnitScalarResultBindingPlan,
        CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralResultBindingPlan,
    };
    use crate::values::CheckedProofTerm;
    use crate::{MachineContractCommitment, NominalMachineUseSite};
    use language_core::activation_storage::ActivationStorageOperation;
    use typed_trees::type_identity::TypeIdentityId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitCallTarget {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub contract_report_fingerprint: u64,
        pub service_reach: ServiceReachSummary,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitCallRoute {
        DescriptorTable {
            operation: language_core::descriptor_tables::DescriptorTableOperation,
            service: language_semantics::ServiceReachId,
            result_type: typed_trees::types::PrimitiveType,
        },
        ActivationStorage {
            operation: ActivationStorageOperation,
            bound: Option<u64>,
        },
        Module {
            contract_commitment: Option<MachineContractCommitment>,
            erased_scalar_arguments: Vec<CheckedCallScalarArgument>,
            erased_proof_arguments: Vec<CheckedProofTerm>,
        },
        Boundary,
        Dynamic {
            selector: CheckedCallScalarArgument,
            interface_identity: TypeIdentityId,
            requirement: SymbolHandle,
            family_operands: Vec<CheckedCallScalarArgument>,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitCallResult {
        NoNormal,
        Unit,
        Scalar(CheckedUnitScalarResultBindingPlan),
        Structural {
            binding: CheckedUnitStructuralResultBindingPlan,
            discard_result_on_return: bool,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitCallTransfers {
        Claims(Vec<CheckedUnitClaimTransferPlan>),
        StructuralCustody(CheckedStructuralCallCustodyPlan),
        CompletionReceipts(Vec<CheckedUnitClaimTransferPlan>),
        Abandonment,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CheckedUnitCallParts {
        pub coordinate: CheckedUnitCallCoordinate,
        pub source_site: Option<NominalMachineUseSite>,
        pub target: CheckedUnitCallTarget,
        pub route: CheckedUnitCallRoute,
        pub completion: BoundaryCompletion,
        pub scalar_arguments: Vec<CheckedCallScalarArgument>,
        pub structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        pub result: CheckedUnitCallResult,
        pub transfers: CheckedUnitCallTransfers,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitCallPlan {
        pub coordinate: CheckedUnitCallCoordinate,
        source_site: Option<NominalMachineUseSite>,
        pub target: CheckedUnitCallTarget,
        route: CheckedUnitCallRoute,
        completion: BoundaryCompletion,
        pub scalar_arguments: Vec<CheckedCallScalarArgument>,
        pub structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        result: CheckedUnitCallResult,
        transfers: CheckedUnitCallTransfers,
    }

    impl CheckedUnitCallPlan {
        pub fn new(parts: CheckedUnitCallParts) -> Option<Self> {
            use CheckedUnitCallResult as Result;
            use CheckedUnitCallTransfers as Transfers;
            let CheckedUnitCallParts {
                coordinate,
                source_site,
                target,
                route,
                completion,
                scalar_arguments,
                structural_arguments,
                result,
                transfers,
            } = parts;
            let admitted = match completion {
            BoundaryCompletion::External(_) => matches!(
                (&route, &result, &transfers),
                (
                    CheckedUnitCallRoute::Boundary,
                    Result::Unit,
                    Transfers::Abandonment
                )
            ),
            BoundaryCompletion::NormalReturn => match (&route, &result, &transfers) {
                (
                    CheckedUnitCallRoute::DescriptorTable {
                        operation,
                        result_type,
                        ..
                    },
                    Result::Unit | Result::Scalar(_),
                    Transfers::Claims(claims),
                ) => {
                    claims.is_empty()
                        && match &result {
                            Result::Scalar(result) => result.primitive_type == *result_type,
                            _ => true,
                        }
                        && match operation {
                            language_core::descriptor_tables::DescriptorTableOperation::ActivateTask {
                                entries,
                            } => {
                                *result_type == typed_trees::types::PrimitiveType::U8
                                    && (2..=8192).contains(entries)
                                    && structural_arguments.len() == 3
                                    && scalar_arguments.len() == 1
                            }
                            language_core::descriptor_tables::DescriptorTableOperation::ReloadSegments { entries, .. } => {
                                *result_type == typed_trees::types::PrimitiveType::U8
                                    && (1..=8192).contains(entries)
                                    && structural_arguments.len() == 2
                                    && scalar_arguments.len() == 1
                            }
                            language_core::descriptor_tables::DescriptorTableOperation::ReadTask
                            | language_core::descriptor_tables::DescriptorTableOperation::ReadSegment { .. } => {
                                *result_type == typed_trees::types::PrimitiveType::U16
                                    && structural_arguments.is_empty() && scalar_arguments.is_empty()
                            }
                        }
                }
                (
                    CheckedUnitCallRoute::Module {
                        contract_commitment,
                        ..
                    },
                    Result::NoNormal,
                    Transfers::Claims(_),
                ) => contract_commitment.is_some(),
                (
                    CheckedUnitCallRoute::ActivationStorage {
                        operation: ActivationStorageOperation::Establish,
                        bound: Some(_),
                    },
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => true,
                (
                    CheckedUnitCallRoute::ActivationStorage {
                        operation: ActivationStorageOperation::BorrowBytes,
                        bound: None,
                    },
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => true,
                (
                    CheckedUnitCallRoute::ActivationStorage {
                        operation: ActivationStorageOperation::Release,
                        bound: None,
                    },
                    Result::Unit,
                    Transfers::Claims(_),
                ) => true,
                (
                    CheckedUnitCallRoute::ActivationStorage {
                        operation: ActivationStorageOperation::Split,
                        bound: Some(_),
                    },
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => true,
                (
                    CheckedUnitCallRoute::ActivationStorage {
                        operation: ActivationStorageOperation::Merge,
                        bound: None,
                    },
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => true,
                (
                    CheckedUnitCallRoute::Module {
                        contract_commitment,
                        ..
                    },
                    Result::Unit,
                    Transfers::Claims(_),
                ) => contract_commitment.is_none(),
                (
                    CheckedUnitCallRoute::Module {
                        contract_commitment,
                        ..
                    },
                    Result::Scalar(_),
                    Transfers::Claims(_),
                ) => contract_commitment.is_some(),
                (
                    CheckedUnitCallRoute::Module {
                        contract_commitment,
                        ..
                    },
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => contract_commitment.is_some(),
                (
                    CheckedUnitCallRoute::Boundary,
                    Result::Unit | Result::Scalar(_),
                    Transfers::CompletionReceipts(_),
                ) => true,
                (
                    CheckedUnitCallRoute::Boundary,
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => true,
                (
                    CheckedUnitCallRoute::Dynamic { .. },
                    Result::Unit | Result::Scalar(_),
                    Transfers::Claims(_),
                ) => true,
                (
                    CheckedUnitCallRoute::Dynamic { .. },
                    Result::Structural { .. },
                    Transfers::StructuralCustody(_),
                ) => true,
                _ => false,
            },
        };
            admitted.then_some(Self {
                coordinate,
                source_site,
                target,
                route,
                completion,
                scalar_arguments,
                structural_arguments,
                result,
                transfers,
            })
        }

        pub fn into_parts(self) -> CheckedUnitCallParts {
            CheckedUnitCallParts {
                coordinate: self.coordinate,
                source_site: self.source_site,
                target: self.target,
                route: self.route,
                completion: self.completion,
                scalar_arguments: self.scalar_arguments,
                structural_arguments: self.structural_arguments,
                result: self.result,
                transfers: self.transfers,
            }
        }

        pub fn is_boundary(&self) -> bool {
            matches!(self.route, CheckedUnitCallRoute::Boundary)
        }

        pub fn is_module(&self) -> bool {
            matches!(self.route, CheckedUnitCallRoute::Module { .. })
        }

        pub fn route(&self) -> &CheckedUnitCallRoute {
            &self.route
        }

        pub fn completion(&self) -> BoundaryCompletion {
            self.completion
        }

        pub fn has_normal_continuation(&self) -> bool {
            self.completion == BoundaryCompletion::NormalReturn
                && self.result != CheckedUnitCallResult::NoNormal
        }

        pub fn result(&self) -> &CheckedUnitCallResult {
            &self.result
        }

        pub fn transfers(&self) -> &CheckedUnitCallTransfers {
            &self.transfers
        }

        pub fn source_site(&self) -> Option<NominalMachineUseSite> {
            self.source_site
        }

        pub fn contract_commitment(&self) -> Option<MachineContractCommitment> {
            match &self.route {
                CheckedUnitCallRoute::Module {
                    contract_commitment,
                    ..
                } => *contract_commitment,
                CheckedUnitCallRoute::ActivationStorage { .. }
                | CheckedUnitCallRoute::DescriptorTable { .. }
                | CheckedUnitCallRoute::Boundary
                | CheckedUnitCallRoute::Dynamic { .. } => None,
            }
        }

        pub fn erased_scalar_arguments(&self) -> Option<&[CheckedCallScalarArgument]> {
            match &self.route {
                CheckedUnitCallRoute::Module {
                    erased_scalar_arguments,
                    ..
                } => Some(erased_scalar_arguments),
                CheckedUnitCallRoute::ActivationStorage { .. }
                | CheckedUnitCallRoute::DescriptorTable { .. }
                | CheckedUnitCallRoute::Boundary
                | CheckedUnitCallRoute::Dynamic { .. } => None,
            }
        }

        pub fn erased_proof_arguments(&self) -> Option<&[CheckedProofTerm]> {
            match &self.route {
                CheckedUnitCallRoute::Module {
                    erased_proof_arguments,
                    ..
                } => Some(erased_proof_arguments),
                CheckedUnitCallRoute::ActivationStorage { .. }
                | CheckedUnitCallRoute::DescriptorTable { .. }
                | CheckedUnitCallRoute::Boundary
                | CheckedUnitCallRoute::Dynamic { .. } => None,
            }
        }

        pub fn scalar_operands(&self) -> Vec<&CheckedCallScalarArgument> {
            self.dynamic_selector()
                .into_iter()
                .chain(&self.scalar_arguments)
                .collect()
        }

        pub fn dynamic_selector(&self) -> Option<&CheckedCallScalarArgument> {
            match &self.route {
                CheckedUnitCallRoute::Dynamic { selector, .. } => Some(selector),
                _ => None,
            }
        }

        pub fn scalar_result(&self) -> Option<&CheckedUnitScalarResultBindingPlan> {
            match &self.result {
                CheckedUnitCallResult::Scalar(result) => Some(result),
                _ => None,
            }
        }

        pub fn structural_result(&self) -> Option<&CheckedUnitStructuralResultBindingPlan> {
            match &self.result {
                CheckedUnitCallResult::Structural { binding, .. } => Some(binding),
                _ => None,
            }
        }

        pub fn structural_result_mut(
            &mut self,
        ) -> Option<(&mut CheckedUnitStructuralResultBindingPlan, &mut bool)> {
            match &mut self.result {
                CheckedUnitCallResult::Structural {
                    binding,
                    discard_result_on_return,
                } => Some((binding, discard_result_on_return)),
                _ => None,
            }
        }

        pub fn discard_result_on_return(&self) -> Option<bool> {
            match &self.result {
                CheckedUnitCallResult::Structural {
                    discard_result_on_return,
                    ..
                } => Some(*discard_result_on_return),
                _ => None,
            }
        }

        pub fn claim_transfers(&self) -> &[CheckedUnitClaimTransferPlan] {
            match &self.transfers {
                CheckedUnitCallTransfers::Claims(claims)
                | CheckedUnitCallTransfers::CompletionReceipts(claims) => claims,
                CheckedUnitCallTransfers::StructuralCustody(custody) => &custody.claim_transfers,
                CheckedUnitCallTransfers::Abandonment => &[],
            }
        }

        pub fn structural_custody(&self) -> Option<&CheckedStructuralCallCustodyPlan> {
            match &self.transfers {
                CheckedUnitCallTransfers::StructuralCustody(custody) => Some(custody),
                _ => None,
            }
        }

        pub fn structural_custody_mut(&mut self) -> Option<&mut CheckedStructuralCallCustodyPlan> {
            match &mut self.transfers {
                CheckedUnitCallTransfers::StructuralCustody(custody) => Some(custody),
                _ => None,
            }
        }
    }
}

pub(crate) mod unit_effect_plans {

    use crate::flow::terminal::structural_return::CheckedUnitStructuralReturnPlan;
    use crate::flow::terminal::{
        CheckedBoundaryMachinePlan, CheckedCallScalarArgument,
        CheckedComposedUnitControlMachinePlan, CheckedScalarStateTerminator,
        CheckedStructuralScalarParameterPlan, CheckedTrivialAffineStructuralLocalPlan,
        CheckedUnitCallCoordinate, CheckedUnitCallPlan, CheckedUnitCallResult,
        CheckedUnitEntryClaimPlan, CheckedUnitScalarResultBindingPlan,
        CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralDomainPlan, CheckedUnitStructuralParameterPlan,
        CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan,
        CheckedUnitStructuralTypePlan,
    };
    use crate::values::CheckedErasedProofParameterPlan;
    use diagnostics::{CoverageForm, Diagnostic};
    use language_semantics::{SemanticDomainId, ServiceReachPlan, ServiceReachSummary};
    use source::SourceSpan;
    use symbols::{SymbolHandle, SymbolTable};
    use typed_trees::type_identity::TypeIdentityId;
    use typed_trees::types::PrimitiveType;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitCleanupBoundary {
        Call(CheckedUnitCallCoordinate),
        Replacement {
            statement_index: u32,
            destination: CheckedUnitStructuralArgumentPlan,
            old_value: CheckedUnitStructuralArgumentPlan,
            receiver_path: Vec<CheckedUnitStructuralPathSegment>,
        },
        StatementEntry {
            statement_index: u32,
        },
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitEffectPlans {
        pub cleanup_control: crate::CheckedCleanupControlPlans,
        pub structural_types: Vec<CheckedUnitStructuralTypePlan>,
        pub layout_observations: Vec<CheckedLayoutObservation>,
        pub structural_domains: Vec<CheckedUnitStructuralDomainPlan>,
        pub boundary_machines: Vec<CheckedBoundaryMachinePlan>,
        pub calling_signatures: Vec<crate::flow::terminal::CheckedCallingSignaturePlan>,
        pub opaque_representations: Vec<crate::flow::terminal::CheckedOpaqueRepresentation>,
        pub dynamic_slots: Vec<crate::CheckedDynamicSlotPlan>,
        pub machines: Vec<CheckedUnitEffectMachinePlan>,
        pub composed_machines: Vec<CheckedComposedUnitControlMachinePlan>,
        pub gaps: Vec<CheckedUnitPlanGap>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedLayoutObservation {
        pub owner: SymbolHandle,
        pub expression: typed_trees::expression::ExpressionHandle,
        pub subject: TypeIdentityId,
        pub measurement: language_core::data_layout_queries::DataLayoutQueryKind,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitPlanGap {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub statement_index: Option<u32>,
        pub reason: CheckedUnitPlanGapReason,
        pub source_span: SourceSpan,
        pub coverage: CoverageForm,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitPlanGapReason {
        Unsupported(std::borrow::Cow<'static, str>),
        Callee(SymbolHandle),
        BoundaryTarget(SymbolHandle),
        UnselectedOpaqueRepresentation(SymbolHandle),
    }

    impl CheckedUnitEffectPlans {
        pub fn adopt_structural_types(
            &mut self,
            types: impl IntoIterator<Item = CheckedUnitStructuralTypePlan>,
        ) -> Result<(), (TypeIdentityId, String)> {
            for shape in types {
                if !shape.identity.is_valid() {
                    return Err((shape.identity, "empty structural type identity".into()));
                }
                match crate::checked_structural_type_by_identity(
                    &self.structural_types,
                    shape.identity,
                ) {
                    Some(existing) if *existing != shape => {
                        return Err((
                            shape.identity,
                            format!("existing plan {existing:?}; incoming plan {shape:?}"),
                        ));
                    }
                    Some(_) => {}
                    None => self.structural_types.push(shape),
                }
            }
            Ok(())
        }

        pub fn callable_entry_for_machine(
            &self,
            machine: SymbolHandle,
        ) -> Option<CheckedUnitCallableEntry<'_>> {
            let mut entries = self
                .machines
                .iter()
                .filter(|plan| plan.machine == machine)
                .map(CheckedUnitEffectMachinePlan::callable_entry)
                .chain(
                    self.composed_machines
                        .iter()
                        .filter(|plan| plan.machine == machine)
                        .filter_map(CheckedComposedUnitControlMachinePlan::callable_entry),
                );
            let entry = entries.next()?;
            entries.next().is_none().then_some(entry)
        }

        pub fn for_machine(&self, machine: SymbolHandle) -> Option<&CheckedUnitEffectMachinePlan> {
            self.machines.iter().find(|plan| plan.machine == machine)
        }

        pub fn composed_for_machine(
            &self,
            machine: SymbolHandle,
        ) -> Option<&CheckedComposedUnitControlMachinePlan> {
            self.composed_machines
                .iter()
                .find(|plan| plan.machine == machine)
        }

        pub fn gap_for_machine(&self, machine: SymbolHandle) -> Option<&CheckedUnitPlanGap> {
            self.gaps.iter().find(|row| row.machine == machine)
        }

        pub fn gap_diagnostic(
            &self,
            machine: SymbolHandle,
            symbols: &SymbolTable,
        ) -> Option<Diagnostic> {
            let name = |symbol: SymbolHandle| symbols.display_path(symbol, "::");
            let mut gap = self.gap_for_machine(machine)?;
            let mut chain = vec![machine];
            while let CheckedUnitPlanGapReason::Callee(target)
            | CheckedUnitPlanGapReason::BoundaryTarget(target) = gap.reason
            {
                let Some(next) = self.gap_for_machine(target) else {
                    break;
                };
                if chain.contains(&target) {
                    break;
                }
                chain.push(target);
                gap = next;
            }
            let construct = match gap.coverage {
                CoverageForm::MachineHeader => "its declaration",
                CoverageForm::EntrySignature => "its signature",
                CoverageForm::StateSignature => "a state",
                CoverageForm::StatementForm => "a statement",
                CoverageForm::CallForm => "a call",
                CoverageForm::ExpressionForm => "an expression",
                CoverageForm::TransitionForm => "a transition",
            };
            let reason = match gap.reason {
                CheckedUnitPlanGapReason::Unsupported(ref detail) => format!(
                    "`{}` has no executable body: this compiler does not lower {construct} of it yet ({detail})",
                    name(gap.machine)
                ),
                CheckedUnitPlanGapReason::Callee(target) => format!(
                    "`{}` has no executable body: its call of `{}` reaches no executable callee plan",
                    name(gap.machine),
                    name(target)
                ),
                CheckedUnitPlanGapReason::BoundaryTarget(target) => format!(
                    "`{}` has no executable body: it calls boundary `{}`, which has no boundary plan",
                    name(gap.machine),
                    name(target)
                ),
                CheckedUnitPlanGapReason::UnselectedOpaqueRepresentation(opaque) => format!(
                    "`{}` uses opaque boundary data `{opaque}` by value, but the build selects no `OpaqueRepresentation<{opaque}, _>` application; add `Build::select_representation<{opaque}, <conformance>>(builder)`",
                    name(gap.machine),
                    opaque = name(opaque)
                ),
            };
            let reason = if gap.state.is_valid() {
                match gap.statement_index {
                    Some(statement_index) => format!(
                        "{reason}; in state `{}`, statement {statement_index}",
                        name(gap.state)
                    ),
                    None => format!("{reason}; in state `{}`", name(gap.state)),
                }
            } else {
                reason
            };
            let message = if chain.len() == 1 {
                reason
            } else {
                format!(
                    "`{}` calls {}; {reason}",
                    name(machine),
                    chain[1..]
                        .iter()
                        .map(|symbol| format!("`{}`", name(*symbol)))
                        .collect::<Vec<_>>()
                        .join(", which calls ")
                )
            };
            let diagnostic = match gap.reason {
                CheckedUnitPlanGapReason::UnselectedOpaqueRepresentation(_) => {
                    Diagnostic::error(message)
                }
                _ => Diagnostic::incomplete(
                    message,
                    gap.coverage.exceeded(gap.source_span.span.start as u64),
                ),
            };
            Some(diagnostic.with_source_span(gap.source_span))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitEffectOperationPlan {
        CaptureAssignmentDestination {
            statement_index: u32,
            destination_expression: typed_trees::expression::ExpressionHandle,
            destination: CheckedUnitStructuralArgumentPlan,
        },
        EstablishReference {
            formation_expression: typed_trees::expression::ExpressionHandle,
            backing_type: typed_trees::types::TypeReferenceHandle,
            source_contract: crate::CheckedReferenceSourceContract,
            result: CheckedUnitStructuralResultBindingPlan,
            source: CheckedUnitStructuralArgumentPlan,
            storage: crate::flow::terminal::CheckedReferenceStorage,
        },
        ReleaseReference {
            statement_index: u32,
            source: CheckedUnitStructuralArgumentSourcePlan,
            path: Vec<CheckedUnitStructuralPathSegment>,
            case_conditions: arena::HandleSpan<crate::flow::terminal::CheckedCleanupCaseCondition>,
            select_evaluated_storage: bool,
            defer_to_exit: bool,
            loans: arena::HandleSpan<crate::BorrowLoanFact>,
            selection_complements: Vec<CheckedUnitSelectedReferenceRetirementPlan>,
            owner_cleanups: arena::HandleSpan<crate::flow::terminal::CheckedReferenceOwnerCleanup>,
        },
        EstablishLocal(CheckedUnitLocalBindingPlan),
        EvaluateStructuralValue {
            statement_index: u32,
            value: crate::CheckedStructuralValueHandle,
            calls: Vec<CheckedStructuralValueCall>,
            operand_source: Option<crate::CheckedArrayConstructionSource>,
        },
        EstablishViewSubslice {
            result: CheckedUnitStructuralResultBindingPlan,
            source: CheckedUnitStructuralArgumentPlan,
        },
        ContinuationCleanup {
            partitions: arena::HandleSpan<crate::CheckedCleanupPartitionRef>,
            boundary: CheckedUnitCleanupBoundary,
            selected_regions: Vec<CheckedUnitStructuralArgumentPlan>,
            cleanup_actions: Vec<crate::CheckedNormalCleanupAction>,
        },
        Call(CheckedUnitCallPlan),
        RegisterAccess {
            coordinate: CheckedUnitCallCoordinate,
            operation: language_core::inline_assembly::AsmRegisterOperation,
            result: Option<CheckedUnitScalarResultBindingPlan>,
            arguments: Vec<CheckedCallScalarArgument>,
            service_reach: ServiceReachSummary,
        },
        GlobalDescriptorTable {
            coordinate: CheckedUnitCallCoordinate,
            operation: language_core::inline_assembly::AsmGlobalDescriptorTableOperation,
            storage: CheckedUnitStructuralArgumentPlan,
            service: language_semantics::ServiceReachId,
            service_reach: ServiceReachSummary,
        },
        Store(crate::CheckedStorePlan),
        AtomicAccess(crate::CheckedAtomicAccessPlan),
        AtomicFence {
            statement_index: u32,
            declaration: SymbolHandle,
            ordering: language_core::atomic::FenceOrdering,
            argument: CheckedUnitStructuralArgumentPlan,
        },
        MoveStructuralField {
            result: CheckedUnitStructuralResultBindingPlan,
            source: CheckedUnitStructuralArgumentPlan,
            expression: typed_trees::expression::ExpressionHandle,
        },
        Complete {
            statement_index: u32,
            trivial_affine_local_discard_ordinals: Vec<u32>,
            trivial_affine_discards: Vec<u32>,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedUnitLocalBindingPlan {
        Primitive {
            statement_index: u32,
            symbol: SymbolHandle,
            type_identity: TypeIdentityId,
            primitive_type: PrimitiveType,
            value: CheckedCallScalarArgument,
        },
        TrivialAffine {
            statement_index: u32,
            declaration_ordinal: u32,
            type_identity: TypeIdentityId,
        },
        Structural {
            statement_index: u32,
            declaration_ordinal: u32,
            symbol: SymbolHandle,
            result: CheckedUnitStructuralResultBindingPlan,
            initial_value: Option<CheckedStructuralLocalInitialization>,
            qualifications: Vec<SemanticDomainId>,
            discard_result_on_return: bool,
        },
        Scalar {
            result: CheckedUnitScalarResultBindingPlan,
            value: CheckedCallScalarArgument,
        },
        ByteSequence {
            result: CheckedUnitStructuralResultBindingPlan,
            value: crate::CheckedByteSequenceValuePlan,
            qualifications: Vec<SemanticDomainId>,
        },
        ScalarArray {
            qualifications: Vec<SemanticDomainId>,
            source: crate::CheckedArrayConstructionSource,
            result: CheckedUnitStructuralResultBindingPlan,
            elements: Vec<CheckedCallScalarArgument>,
        },
        StructuralValue {
            result: CheckedUnitStructuralResultBindingPlan,
            value: crate::CheckedStructuralValueHandle,
            calls: Vec<CheckedStructuralValueCall>,
            operand_source: Option<crate::CheckedArrayConstructionSource>,
            discard_result_on_return: bool,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStructuralLocalInitialization {
        source: CheckedUnitStructuralArgumentPlan,
        source_binding_ordinal: u32,
        pub value: crate::CheckedStructuralValueHandle,
    }

    impl CheckedStructuralLocalInitialization {
        pub fn completed_result(
            result: &CheckedUnitStructuralResultBindingPlan,
            value: crate::CheckedStructuralValueHandle,
        ) -> Self {
            Self {
                source: CheckedUnitStructuralArgumentPlan::structural_result(
                    result.binding_ordinal,
                    result.type_identity,
                    crate::CheckedStructuralAccess::Owned,
                ),
                source_binding_ordinal: result.binding_ordinal,
                value,
            }
        }

        pub fn source(&self) -> &CheckedUnitStructuralArgumentPlan {
            &self.source
        }

        pub fn source_binding_ordinal(&self) -> u32 {
            self.source_binding_ordinal
        }
    }

    impl CheckedUnitLocalBindingPlan {
        pub fn structural_result_binding(&self) -> Option<&CheckedUnitStructuralResultBindingPlan> {
            match self {
                Self::Structural { result, .. }
                | Self::ByteSequence { result, .. }
                | Self::ScalarArray { result, .. }
                | Self::StructuralValue { result, .. } => Some(result),
                Self::Primitive { .. } | Self::TrivialAffine { .. } | Self::Scalar { .. } => None,
            }
        }

        pub fn source_statement_index(&self) -> Option<u32> {
            match self {
                Self::Primitive {
                    statement_index, ..
                }
                | Self::TrivialAffine {
                    statement_index, ..
                }
                | Self::Structural {
                    statement_index, ..
                } => Some(*statement_index),
                Self::Scalar { result, .. } => Some(result.statement_index),
                Self::ByteSequence { result, .. }
                | Self::ScalarArray { result, .. }
                | Self::StructuralValue { result, .. } => Some(result.statement_index),
            }
        }

        pub fn discardable_structural_result(
            &self,
        ) -> Option<(&CheckedUnitStructuralResultBindingPlan, bool)> {
            match self {
                Self::Structural {
                    result,
                    discard_result_on_return,
                    ..
                }
                | Self::StructuralValue {
                    result,
                    discard_result_on_return,
                    ..
                } => Some((result, *discard_result_on_return)),
                _ => None,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitSelectedReferenceRetirementPlan {
        pub selection: arena::Handle<crate::FlowOwnedSelectionReceipt>,
        pub path: Vec<CheckedUnitStructuralPathSegment>,
        pub loans: arena::HandleSpan<crate::BorrowLoanFact>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedOperandSequence {
        operations: arena::Arena<OperandOperation>,
        order: arena::HandleSpan<OperandOperation>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    struct OperandOperation(Option<crate::CheckedUnitEffectOperationPlan>);

    impl CheckedOperandSequence {
        pub fn new(
            operations: impl IntoIterator<Item = crate::CheckedUnitEffectOperationPlan>,
        ) -> Self {
            let mut sequence = Self::default();
            for operation in operations {
                sequence
                    .operations
                    .append_to_span(&mut sequence.order, OperandOperation(Some(operation)));
            }
            sequence
        }

        pub fn iter(
            &self,
        ) -> impl DoubleEndedIterator<Item = &crate::CheckedUnitEffectOperationPlan> {
            self.operations
                .span_or_empty(self.order)
                .iter()
                .map(|operation| {
                    operation
                        .0
                        .as_ref()
                        .expect("published operand span contains operations")
                })
        }

        pub fn len(&self) -> usize {
            self.order.len()
        }

        pub fn is_empty(&self) -> bool {
            self.order.is_empty()
        }

        pub fn get(&self, ordinal: usize) -> Option<&crate::CheckedUnitEffectOperationPlan> {
            self.operations
                .span_or_empty(self.order)
                .get(ordinal)?
                .0
                .as_ref()
        }

        pub fn get_mut(
            &mut self,
            ordinal: usize,
        ) -> Option<&mut crate::CheckedUnitEffectOperationPlan> {
            self.operations
                .span_mut_or_empty(self.order)
                .get_mut(ordinal)?
                .0
                .as_mut()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedStructuralValueCall {
        pub value: crate::CheckedStructuralValueHandle,
        operands: CheckedOperandSequence,
        outcome: CheckedValueInvocationOutcome,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    enum CheckedValueInvocationOutcome {
        Absent,
        Prepared {
            operation: CheckedUnitEffectOperationPlan,
            completions: CheckedOperandSequence,
        },
        Stopped,
    }

    impl Default for CheckedStructuralValueCall {
        fn default() -> Self {
            Self {
                value: crate::CheckedStructuralValueHandle::invalid(),
                operands: CheckedOperandSequence::default(),
                outcome: CheckedValueInvocationOutcome::Absent,
            }
        }
    }

    impl CheckedStructuralValueCall {
        pub fn new(
            value: crate::CheckedStructuralValueHandle,
            operation: CheckedUnitEffectOperationPlan,
        ) -> Option<Self> {
            Self::with_operands(value, Default::default(), operation, Default::default())
        }

        pub fn with_operands(
            value: crate::CheckedStructuralValueHandle,
            operands: CheckedOperandSequence,
            operation: CheckedUnitEffectOperationPlan,
            completions: CheckedOperandSequence,
        ) -> Option<Self> {
            if operands
                .iter()
                .any(|operation| !operation.has_normal_continuation())
            {
                return None;
            }
            let call = operation.call()?;
            let admitted = call.structural_result().is_some()
                || (*call.result() == CheckedUnitCallResult::NoNormal
                    && completions.iter().next().is_none());
            admitted.then_some(Self {
                value,
                operands,
                outcome: CheckedValueInvocationOutcome::Prepared {
                    operation,
                    completions,
                },
            })
        }

        pub fn stopped(
            value: crate::CheckedStructuralValueHandle,
            operands: CheckedOperandSequence,
        ) -> Option<Self> {
            if operands.iter().last()?.has_normal_continuation() {
                return None;
            }
            Some(Self {
                value,
                operands,
                outcome: CheckedValueInvocationOutcome::Stopped,
            })
        }

        pub fn has_normal_continuation(&self) -> bool {
            match &self.outcome {
                CheckedValueInvocationOutcome::Prepared { operation, .. } => {
                    operation.has_normal_continuation()
                }
                CheckedValueInvocationOutcome::Stopped => false,
                CheckedValueInvocationOutcome::Absent => true,
            }
        }

        pub fn operands(&self) -> &CheckedOperandSequence {
            &self.operands
        }
        pub fn operands_mut(&mut self) -> &mut CheckedOperandSequence {
            &mut self.operands
        }
        pub fn operation(&self) -> Option<&CheckedUnitEffectOperationPlan> {
            match &self.outcome {
                CheckedValueInvocationOutcome::Prepared { operation, .. } => Some(operation),
                CheckedValueInvocationOutcome::Stopped | CheckedValueInvocationOutcome::Absent => {
                    None
                }
            }
        }
        pub fn operation_mut(&mut self) -> Option<&mut CheckedUnitEffectOperationPlan> {
            match &mut self.outcome {
                CheckedValueInvocationOutcome::Prepared { operation, .. } => Some(operation),
                CheckedValueInvocationOutcome::Stopped | CheckedValueInvocationOutcome::Absent => {
                    None
                }
            }
        }
        pub fn completions(&self) -> Option<&CheckedOperandSequence> {
            match &self.outcome {
                CheckedValueInvocationOutcome::Prepared { completions, .. } => Some(completions),
                CheckedValueInvocationOutcome::Stopped | CheckedValueInvocationOutcome::Absent => {
                    None
                }
            }
        }
        pub fn completions_mut(&mut self) -> Option<&mut CheckedOperandSequence> {
            match &mut self.outcome {
                CheckedValueInvocationOutcome::Prepared { completions, .. } => Some(completions),
                CheckedValueInvocationOutcome::Stopped | CheckedValueInvocationOutcome::Absent => {
                    None
                }
            }
        }
        pub fn operation_dependencies(
            &self,
        ) -> impl DoubleEndedIterator<Item = &CheckedUnitEffectOperationPlan> {
            self.operands.iter().chain(self.operation()).chain(
                self.completions()
                    .into_iter()
                    .flat_map(CheckedOperandSequence::iter),
            )
        }
        pub fn structural_result(&self) -> Option<&CheckedUnitStructuralResultBindingPlan> {
            self.operation()?.call()?.structural_result()
        }
    }

    #[derive(Clone, Copy)]
    pub enum CheckedStructuralInvocation<'a> {
        Call(&'a CheckedUnitCallPlan),
        BorrowedInstruction {
            coordinate: CheckedUnitCallCoordinate,
            storage: &'a CheckedUnitStructuralArgumentPlan,
        },
    }

    impl<'a> CheckedStructuralInvocation<'a> {
        pub fn coordinate(self) -> CheckedUnitCallCoordinate {
            match self {
                Self::Call(call) => call.coordinate,
                Self::BorrowedInstruction { coordinate, .. } => coordinate,
            }
        }

        pub fn structural_operands(self) -> &'a [CheckedUnitStructuralArgumentPlan] {
            match self {
                Self::Call(call) => &call.structural_arguments,
                Self::BorrowedInstruction { storage, .. } => std::slice::from_ref(storage),
            }
        }
    }

    impl CheckedUnitEffectOperationPlan {
        pub fn structural_value_evaluation(
            &self,
        ) -> Option<(
            crate::CheckedStructuralValueHandle,
            &[CheckedStructuralValueCall],
        )> {
            match self {
                Self::EstablishLocal(CheckedUnitLocalBindingPlan::StructuralValue {
                    value,
                    calls,
                    ..
                })
                | Self::EvaluateStructuralValue { value, calls, .. } => Some((*value, calls)),
                _ => None,
            }
        }

        pub fn has_normal_continuation(&self) -> bool {
            match self {
                Self::EvaluateStructuralValue { .. } => false,
                Self::Call(call) => call.has_normal_continuation(),
                _ => true,
            }
        }
        pub fn established_reference_source<'plans>(
            &'plans self,
            values: &'plans crate::CheckedStructuralValuePlans,
        ) -> Option<&'plans CheckedUnitStructuralArgumentPlan> {
            match self {
                Self::EstablishReference { source, .. } => Some(source),
                Self::EstablishLocal(CheckedUnitLocalBindingPlan::StructuralValue {
                    value,
                    ..
                }) if values.nodes.is_valid(*value) => match &values.nodes.get(*value).kind {
                    crate::CheckedStructuralValueKind::Reference { source, .. } => Some(source),
                    _ => None,
                },
                _ => None,
            }
        }

        pub fn structural_result_binding(&self) -> Option<&CheckedUnitStructuralResultBindingPlan> {
            match self {
                Self::EstablishReference { result, .. }
                | Self::EstablishViewSubslice { result, .. }
                | Self::MoveStructuralField { result, .. } => Some(result),
                Self::EstablishLocal(binding) => binding.structural_result_binding(),
                Self::AtomicAccess(access) => access.structural_result.as_ref(),
                Self::Call(call) => call.structural_result(),
                _ => None,
            }
        }

        pub fn statement_structural_result_binding(
            &self,
        ) -> Option<&CheckedUnitStructuralResultBindingPlan> {
            match self {
                Self::EstablishLocal(
                    CheckedUnitLocalBindingPlan::StructuralValue {
                        operand_source: Some(_),
                        ..
                    }
                    | CheckedUnitLocalBindingPlan::ScalarArray {
                        source: crate::CheckedArrayConstructionSource::CallArgument { .. },
                        ..
                    },
                ) => None,
                Self::Call(call) if call.coordinate.call_ordinal != 0 => None,
                _ => self.structural_result_binding(),
            }
        }

        pub fn store_statement_index(&self) -> Option<u32> {
            match self {
                Self::Store(store) if store.installed_value().is_none() => {
                    Some(store.statement_index)
                }
                _ => None,
            }
        }

        pub fn new_store(
            statement_index: u32,
            root: crate::CheckedStoreRoot,
            path: Vec<CheckedUnitStructuralPathSegment>,
            leaf: crate::CheckedStoreLeaf,
            value: crate::CheckedStoreValue,
        ) -> Self {
            Self::Store(crate::CheckedStorePlan {
                statement_index,
                captured_selection: Vec::new(),
                destination: crate::CheckedStoreDestination { root, path, leaf },
                value,
            })
        }

        pub fn store(&self) -> Option<&crate::CheckedStorePlan> {
            match self {
                Self::Store(store) => Some(store),
                _ => None,
            }
        }

        pub fn installed_value(&self) -> Option<&CheckedUnitStructuralArgumentPlan> {
            match self {
                Self::EstablishLocal(CheckedUnitLocalBindingPlan::Structural {
                    initial_value,
                    ..
                }) => initial_value.as_ref().map(|initial| &initial.source),
                _ => self
                    .store()
                    .and_then(crate::CheckedStorePlan::installed_value),
            }
        }

        pub fn stored_bytes(&self) -> Option<&crate::CheckedByteSequenceValuePlan> {
            self.store().and_then(|store| store.value.bytes())
        }

        pub fn source_statement_index(&self) -> Option<u32> {
            match self {
                Self::EstablishReference { result, .. }
                | Self::EstablishViewSubslice { result, .. }
                | Self::MoveStructuralField { result, .. } => Some(result.statement_index),
                Self::EstablishLocal(binding) => binding.source_statement_index(),
                Self::EvaluateStructuralValue {
                    statement_index, ..
                } => Some(*statement_index),
                Self::Call(call) => Some(
                    call.structural_result()
                        .map_or(call.coordinate.statement_index, |result| {
                            result.statement_index
                        }),
                ),
                Self::CaptureAssignmentDestination {
                    statement_index, ..
                } => Some(*statement_index),
                Self::RegisterAccess { coordinate, .. }
                | Self::GlobalDescriptorTable { coordinate, .. } => {
                    Some(coordinate.statement_index)
                }
                Self::AtomicAccess(access) => Some(access.statement_index),
                Self::AtomicFence {
                    statement_index, ..
                } => Some(*statement_index),
                Self::Store(store) => Some(store.statement_index),
                Self::ContinuationCleanup { .. }
                | Self::ReleaseReference { .. }
                | Self::Complete { .. } => None,
            }
        }

        pub fn structural_operands(&self) -> &[CheckedUnitStructuralArgumentPlan] {
            match self {
                Self::Call(call) => &call.structural_arguments,
                Self::AtomicFence { argument, .. } => std::slice::from_ref(argument),
                Self::GlobalDescriptorTable { storage, .. } => std::slice::from_ref(storage),
                Self::AtomicAccess(access) => std::slice::from_ref(&access.receiver),
                _ => &[],
            }
        }

        pub fn call(&self) -> Option<&CheckedUnitCallPlan> {
            match self {
                Self::Call(call) => Some(call),
                _ => None,
            }
        }

        pub fn structural_invocation(&self) -> Option<CheckedStructuralInvocation<'_>> {
            match self {
                Self::Call(call) => Some(CheckedStructuralInvocation::Call(call)),
                Self::GlobalDescriptorTable {
                    coordinate,
                    storage,
                    ..
                } => Some(CheckedStructuralInvocation::BorrowedInstruction {
                    coordinate: *coordinate,
                    storage,
                }),
                _ => None,
            }
        }

        pub fn invocation_coordinate(&self) -> Option<CheckedUnitCallCoordinate> {
            match self {
                Self::Call(call) => Some(call.coordinate),
                Self::RegisterAccess { coordinate, .. }
                | Self::GlobalDescriptorTable { coordinate, .. } => Some(*coordinate),
                _ => None,
            }
        }

        pub fn call_mut(&mut self) -> Option<&mut CheckedUnitCallPlan> {
            match self {
                Self::Call(call) => Some(call),
                _ => None,
            }
        }

        pub fn discardable_structural_result(
            &self,
        ) -> Option<(&CheckedUnitStructuralResultBindingPlan, bool)> {
            match self {
                Self::EstablishLocal(binding) => binding.discardable_structural_result(),
                Self::Call(call) => call
                    .structural_result()
                    .zip(call.discard_result_on_return()),
                Self::AtomicAccess(access) => access
                    .structural_result
                    .as_ref()
                    .map(|result| (result, access.discard_result_on_return)),
                _ => None,
            }
        }

        pub fn discardable_structural_result_mut(
            &mut self,
        ) -> Option<(&mut CheckedUnitStructuralResultBindingPlan, &mut bool)> {
            match self {
                Self::EstablishLocal(
                    CheckedUnitLocalBindingPlan::StructuralValue {
                        result,
                        discard_result_on_return,
                        ..
                    }
                    | CheckedUnitLocalBindingPlan::Structural {
                        result,
                        discard_result_on_return,
                        ..
                    },
                ) => Some((result, discard_result_on_return)),
                Self::Call(call) => call.structural_result_mut(),
                Self::AtomicAccess(access) => access
                    .structural_result
                    .as_mut()
                    .map(|result| (result, &mut access.discard_result_on_return)),
                _ => None,
            }
        }

        pub fn discardable_structural_result_by_ordinal_mut(
            &mut self,
            binding_ordinal: u32,
        ) -> Option<(&mut CheckedUnitStructuralResultBindingPlan, &mut bool)> {
            if self
                .discardable_structural_result()
                .is_some_and(|(result, _)| result.binding_ordinal == binding_ordinal)
            {
                return self.discardable_structural_result_mut();
            }
            let calls = match self {
                Self::EstablishLocal(CheckedUnitLocalBindingPlan::StructuralValue {
                    calls,
                    ..
                })
                | Self::EvaluateStructuralValue { calls, .. } => calls,
                _ => return None,
            };
            for call in calls {
                for operand in call
                    .operands
                    .operations
                    .span_mut_or_empty(call.operands.order)
                {
                    if let Some(result) = operand
                        .0
                        .as_mut()
                        .expect("published operand span contains operations")
                        .discardable_structural_result_by_ordinal_mut(binding_ordinal)
                    {
                        return Some(result);
                    }
                }
                if let Some(result) = call.operation_mut().and_then(|operation| {
                    operation.discardable_structural_result_by_ordinal_mut(binding_ordinal)
                }) {
                    return Some(result);
                }
            }
            None
        }

        pub fn with_value_calls(&self) -> impl Iterator<Item = &Self> {
            let mut pending = vec![self];
            std::iter::from_fn(move || {
                let operation = pending.pop()?;
                if let Self::EstablishLocal(CheckedUnitLocalBindingPlan::StructuralValue {
                    calls,
                    ..
                })
                | Self::EvaluateStructuralValue { calls, .. } = operation
                {
                    for call in calls.iter().rev() {
                        pending.extend(call.operation_dependencies().rev());
                    }
                }
                Some(operation)
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitScalarControlPlan {
        pub primitive_type: PrimitiveType,
        pub terminator: CheckedScalarStateTerminator,
        /// Payload values born only after their exact return arm is selected.
        pub selected_copies: Vec<CheckedScalarReturnCopies>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedScalarReturnCopies {
        pub statement_ordinal: u32,
        pub is_continuation: bool,
        pub transition_target: typed_trees::statement::TransitionTargetHandle,
        pub copied_payloads: Vec<CheckedSelectedPayloadCopy>,
    }

    /// A selected private copy whose source identity and disposal were admitted.
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedSelectedPayloadCopy {
        pub value: crate::CheckedStructuralValueHandle,
        pub type_identity: TypeIdentityId,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedSelectedPayloadCopies {
        pub statement_ordinal: u32,
        pub transition_target: typed_trees::statement::TransitionTargetHandle,
        pub values: Vec<CheckedSelectedPayloadCopy>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitParameterStoragePlan {
        pub source_position: u32,
        pub symbol: SymbolHandle,
        pub type_identity: TypeIdentityId,
        pub primitive_type: PrimitiveType,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedUnitEffectMachinePlan {
        pub result: crate::flow::terminal::CheckedControlResultPlan,
        pub normal_exit_cleanups: crate::flow::terminal::CheckedNormalExitCleanups,
        pub scalar_result: Option<CheckedUnitScalarResultBindingPlan>,
        pub scalar_control: Option<CheckedUnitScalarControlPlan>,
        pub structural_result: Option<CheckedUnitStructuralReturnPlan>,
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
        pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
        pub parameter_storage: Vec<CheckedUnitParameterStoragePlan>,
        pub erased_scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
        pub erased_proof_parameters: Vec<CheckedErasedProofParameterPlan>,
        pub trivial_affine_locals: Vec<CheckedTrivialAffineStructuralLocalPlan>,
        pub entry_claims: Vec<CheckedUnitEntryClaimPlan>,
        pub body_qualifications: Vec<SemanticDomainId>,
        pub contract_report_fingerprint: u64,
        pub contract_commitment: crate::MachineContractCommitment,
        pub contract_service_reach: ServiceReachPlan,
        pub service_reach: ServiceReachSummary,
        pub operations: Vec<CheckedUnitEffectOperationPlan>,
        pub statement_count: u32,
        pub place_loans: Vec<crate::flow::terminal::CheckedPlaceLoanPlan>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CheckedUnitCallableEntry<'plan> {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub returns_unit: bool,
        pub structural_parameters: &'plan [CheckedUnitStructuralParameterPlan],
        pub scalar_parameters: &'plan [CheckedStructuralScalarParameterPlan],
        pub erased_scalar_parameters: &'plan [CheckedStructuralScalarParameterPlan],
        pub erased_proof_parameters: &'plan [CheckedErasedProofParameterPlan],
        pub entry_claims: &'plan [CheckedUnitEntryClaimPlan],
        pub contract_report_fingerprint: u64,
        pub contract_service_reach: ServiceReachPlan,
    }

    impl CheckedUnitEffectMachinePlan {
        pub fn callable_entry(&self) -> CheckedUnitCallableEntry<'_> {
            CheckedUnitCallableEntry {
                machine: self.machine,
                state: self.state,
                returns_unit: self.result == crate::flow::terminal::CheckedControlResultPlan::Unit,
                structural_parameters: &self.structural_parameters,
                scalar_parameters: &self.scalar_parameters,
                erased_scalar_parameters: &self.erased_scalar_parameters,
                erased_proof_parameters: &self.erased_proof_parameters,
                entry_claims: &self.entry_claims,
                contract_report_fingerprint: self.contract_report_fingerprint,
                contract_service_reach: self.contract_service_reach,
            }
        }
    }

    impl CheckedComposedUnitControlMachinePlan {
        pub fn state(
            &self,
            state: SymbolHandle,
        ) -> Option<&crate::flow::terminal::CheckedComposedUnitControlStatePlan> {
            self.states
                .iter()
                .find(|candidate| candidate.state == state)
        }

        pub fn callable_entry(&self) -> Option<CheckedUnitCallableEntry<'_>> {
            let state = self.states.first()?;
            Some(CheckedUnitCallableEntry {
                machine: self.machine,
                state: state.state,
                returns_unit: matches!(
                    self.result,
                    crate::flow::terminal::CheckedControlResultPlan::Unit
                ),
                structural_parameters: &state.structural_parameters,
                scalar_parameters: &state.scalar_parameters,
                erased_scalar_parameters: &state.erased_scalar_parameters,
                erased_proof_parameters: &state.erased_proof_parameters,
                entry_claims: &state.entry_claims,
                contract_report_fingerprint: self.contract_report_fingerprint,
                contract_service_reach: self.contract_service_reach,
            })
        }
    }

    #[macro_export]
    macro_rules! byte_sequence_establishment_pattern {
        () => {
            $crate::CheckedUnitEffectOperationPlan::EstablishLocal(
                $crate::CheckedUnitLocalBindingPlan::ByteSequence { .. },
            ) | $crate::CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
        };
    }

    #[macro_export]
    macro_rules! reference_hold_pattern {
        () => {
            $crate::CheckedUnitEffectOperationPlan::EstablishReference { .. }
                | $crate::CheckedUnitEffectOperationPlan::ReleaseReference { .. }
        };
    }

    #[macro_export]
    macro_rules! structural_field_write_pattern {
        () => {
            $crate::CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                | $crate::CheckedUnitEffectOperationPlan::CaptureAssignmentDestination { .. }
        };
    }
}

pub(crate) mod unit_plans {
    use symbols::SymbolHandle;

    use crate::flow::FlowFacts;
    use crate::flow::terminal::{
        CheckedComposedUnitControlMachinePlan, CheckedNominalAffineUnitCleanupMachinePlan,
        CheckedPartialAffineUnitCleanupMachinePlan, CheckedTerminalSignatureEligibility,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CheckedUnitPlan<'a> {
        NominalAffineCleanup(&'a CheckedNominalAffineUnitCleanupMachinePlan),
        PartialAffineCleanup(&'a CheckedPartialAffineUnitCleanupMachinePlan),
        ComposedControl(&'a CheckedComposedUnitControlMachinePlan),
    }

    impl<'a> CheckedUnitPlan<'a> {
        pub fn for_machine(flow: &'a FlowFacts, machine: SymbolHandle) -> Option<Self> {
            flow.terminal_nominal_affine_unit_cleanups
                .for_machine(machine)
                .map(Self::NominalAffineCleanup)
                .or_else(|| {
                    flow.terminal_partial_affine_unit_cleanups
                        .for_machine(machine)
                        .map(Self::PartialAffineCleanup)
                })
                .or_else(|| {
                    flow.terminal_unit_effects
                        .composed_for_machine(machine)
                        .map(Self::ComposedControl)
                })
        }

        pub fn machine(self) -> SymbolHandle {
            match self {
                Self::NominalAffineCleanup(plan) => plan.machine,
                Self::PartialAffineCleanup(plan) => plan.machine.machine,
                Self::ComposedControl(plan) => plan.machine,
            }
        }

        pub fn admits_signature(self, signature: CheckedTerminalSignatureEligibility) -> bool {
            matches!(
                signature,
                CheckedTerminalSignatureEligibility::Eligible
                    | CheckedTerminalSignatureEligibility::FreeUnitEffect
            )
        }

        pub fn is_duplicated_in(self, flow: &FlowFacts) -> bool {
            let machine = self.machine();
            let rows = match self {
                Self::NominalAffineCleanup(_) => flow
                    .terminal_nominal_affine_unit_cleanups
                    .machines
                    .iter()
                    .filter(|plan| plan.machine == machine)
                    .count(),
                Self::PartialAffineCleanup(_) => flow
                    .terminal_partial_affine_unit_cleanups
                    .machines
                    .iter()
                    .filter(|plan| plan.machine.machine == machine)
                    .count(),
                Self::ComposedControl(_) => flow
                    .terminal_unit_effects
                    .composed_machines
                    .iter()
                    .filter(|plan| plan.machine == machine)
                    .count(),
            };
            rows > 1
        }
    }
}

//! Whole-owner call transfers along retained execution topology.
//!
//! Permission occurrences do not say whether a call executes: both sides of a
//! short circuit belong to one statement. Count actual invocation operands,
//! adding sequential evaluation and taking the minimum of selected alternatives.
//! Different calls in two arms can therefore retire the same owner without
//! pretending either call runs unconditionally. This is a query on the producer's
//! executable plan, not a second ownership checker or a source-expression replay.

use crate::{
    CheckedByteSequenceValue, CheckedByteSequenceValuePlan, CheckedCallScalarArgument,
    CheckedConditionalReturnArm, CheckedElementViewRegion, CheckedGuardedExitPlans,
    CheckedScalarBranchDestination, CheckedScalarCaseConstruction, CheckedScalarComputationHandle,
    CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument,
    CheckedScalarDispatchPattern, CheckedScalarExpressionRole, CheckedScalarReturnPlan,
    CheckedScalarStateTerminator, CheckedScalarStoreValue, CheckedStoreValue,
    CheckedStructuralAccess, CheckedStructuralRecordFieldValue, CheckedStructuralValueCall,
    CheckedStructuralValueHandle, CheckedStructuralValueKind, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralResultBindingPlan, CheckedValueFacts,
};
use symbols::SymbolHandle;

impl CheckedUnitStructuralArgumentSourcePlan {
    /// The producer associated with current storage, for result metadata only.
    /// Named custody stays on the local; this does not retarget captured uses
    /// or grant access to a producer consumed by the receiving assignment.
    pub fn current_result_binding(
        &self,
        operations: &[CheckedUnitEffectOperationPlan],
    ) -> Option<u32> {
        let mut source = self.clone();
        let mut prefix = operations;
        loop {
            let symbol = match source {
                Self::StructuralResult { binding_ordinal } => return Some(binding_ordinal),
                Self::StructuralLocal { symbol } => symbol,
                _ => return None,
            };
            let mut previous = None;
            for (index, operation) in prefix.iter().enumerate().rev() {
                if operation.ends_local_occurrence(symbol) {
                    let CheckedUnitEffectOperationPlan::Store(store) = operation else {
                        return None;
                    };
                    let value = store.installed_value()?;
                    if !store.destination.path.is_empty() || !value.path.is_empty() {
                        return None;
                    }
                    previous = Some((index, value.source.clone()));
                    break;
                }
                if let Some(result) = operation.statement_structural_result_binding()
                    && result.local.symbol == symbol
                {
                    return Some(result.binding_ordinal);
                }
            }
            let (end, prior) = previous?;
            prefix = &prefix[..end];
            source = prior;
        }
    }
}

impl CheckedUnitEffectOperationPlan {
    /// Rebinding starts a new occurrence at the same named local. Consumers
    /// through this operation can use its prior occurrence; later consumers
    /// must not be charged to the local's initial producer.
    pub fn ends_local_occurrence(&self, local: SymbolHandle) -> bool {
        local.is_valid()
            && matches!(self, Self::Store(store)
                if matches!(store.destination.leaf, crate::CheckedStoreLeaf::OwnedRebind { .. })
                    && store.destination.root == (crate::CheckedStoreRoot::Local { symbol: local }))
    }

    /// Whole transfers guaranteed before this operation's normal continuation.
    /// `local` is zero for an anonymous result; callers obtain named identity
    /// only from `statement_structural_result_binding`, never from the enclosing
    /// statement of an intermediate call operand. Projections leave residual
    /// custody and are deliberately not whole-owner transfers.
    pub fn definite_call_transfers(
        &self,
        values: &CheckedValueFacts,
        producer: &CheckedUnitEffectOperationPlan,
        local: SymbolHandle,
    ) -> usize {
        CallTransfers::for_producer(values, producer, local)
            .map_or(0, |transfers| transfers.operation(self))
    }
}

impl CheckedCallScalarArgument {
    /// Whole transfers completed while evaluating this value. A guard owns
    /// the same ordered computation as a local initializer; short-circuit
    /// alternatives must not be flattened into unconditional call operands.
    pub fn definite_call_transfers(
        &self,
        values: &CheckedValueFacts,
        producer: &CheckedUnitEffectOperationPlan,
        local: SymbolHandle,
    ) -> usize {
        CallTransfers::for_producer(values, producer, local)
            .map_or(0, |transfers| transfers.scalar(self))
    }
}

struct CallTransfers<'facts> {
    values: &'facts CheckedValueFacts,
    result: &'facts CheckedUnitStructuralResultBindingPlan,
    local: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
}

impl CheckedScalarReturnPlan {
    /// Construction scratch describing each selected normal path's whole
    /// transfers. A caller may join its exact cleanup coordinate to these
    /// counts; a minimum alone loses mixed transfer/disposal partitions.
    pub fn selected_exit_call_transfers(
        &self,
        values: &CheckedValueFacts,
        exits: &CheckedGuardedExitPlans,
        state: SymbolHandle,
        producer: &CheckedUnitEffectOperationPlan,
        local: SymbolHandle,
    ) -> Option<Vec<(CheckedScalarBranchDestination, usize)>> {
        let Self::Exits(completion) = self else {
            return None;
        };
        CallTransfers::for_producer(values, producer, local)?.scalar_exit_paths(
            state,
            exits,
            &completion.terminator,
        )
    }

    /// A scalar exit evaluates its retained guard and selected returned value.
    /// Those computations can consume structural owners even though the
    /// completed result is scalar. A returned binding was evaluated earlier
    /// by the ordinary operation sequence and contributes no second transfer.
    pub fn definite_call_transfers(
        &self,
        values: &CheckedValueFacts,
        exits: &CheckedGuardedExitPlans,
        state: SymbolHandle,
        producer: &CheckedUnitEffectOperationPlan,
        local: SymbolHandle,
    ) -> usize {
        let Self::Exits(completion) = self else {
            return 0;
        };
        let Some(transfers) = CallTransfers::for_producer(values, producer, local) else {
            return 0;
        };
        transfers.scalar_exits(state, exits, &completion.terminator)
    }
}

impl CheckedConditionalReturnArm {
    /// The arm's computations and invocation share the same transfer query as
    /// ordinary statements. Dependency enumeration alone omits scalar roots
    /// and flattens selective computations into apparently unconditional calls.
    pub fn definite_call_transfers(
        &self,
        values: &CheckedValueFacts,
        state: SymbolHandle,
        producer: &CheckedUnitEffectOperationPlan,
        local: SymbolHandle,
    ) -> usize {
        let Some(transfers) = CallTransfers::for_producer(values, producer, local) else {
            return 0;
        };
        match self {
            Self::Scalar {
                statement_ordinal, ..
            } => transfers.scalar_at(
                state,
                *statement_ordinal,
                CheckedScalarExpressionRole::Return,
            ),
            Self::Structural(operation) => transfers.operation(operation),
            Self::Call {
                operands,
                operation,
            } => {
                operands
                    .iter()
                    .map(|operand| transfers.operation(operand))
                    .sum::<usize>()
                    + transfers.operation(operation)
            }
            Self::Crash { .. } | Self::Unit { .. } => 0,
        }
    }
}

impl<'facts> CallTransfers<'facts> {
    fn scalar_at(
        &self,
        state: SymbolHandle,
        statement: u32,
        role: CheckedScalarExpressionRole,
    ) -> usize {
        self.values
            .scalar_computations
            .root_at(state, statement, role)
            .map_or(0, |root| self.computation(root.root))
    }

    fn scalar_destination(
        &self,
        state: SymbolHandle,
        destination: &CheckedScalarBranchDestination,
    ) -> Option<usize> {
        match destination {
            CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            } => Some(self.scalar_at(
                state,
                *statement_ordinal,
                if *is_continuation {
                    CheckedScalarExpressionRole::ContinuationReturn
                } else {
                    CheckedScalarExpressionRole::Return
                },
            )),
            // An edge needs its own transfer accounting; a scalar completion
            // cannot claim that a jump consumed this owner. Abnormal exits
            // have no normal custody successor and are absent from the meet.
            CheckedScalarBranchDestination::Jump(_) => Some(0),
            CheckedScalarBranchDestination::Crash { .. } => None,
        }
    }

    fn scalar_exits(
        &self,
        state: SymbolHandle,
        exits: &CheckedGuardedExitPlans,
        terminator: &CheckedScalarStateTerminator,
    ) -> usize {
        self.scalar_exit_paths(state, exits, terminator)
            .and_then(|paths| paths.into_iter().map(|(_, count)| count).min())
            .unwrap_or(0)
    }

    fn scalar_exit_paths(
        &self,
        state: SymbolHandle,
        exits: &CheckedGuardedExitPlans,
        terminator: &CheckedScalarStateTerminator,
    ) -> Option<Vec<(CheckedScalarBranchDestination, usize)>> {
        let mut paths = Vec::new();
        let mut append = |destination: &CheckedScalarBranchDestination, prefix: usize| {
            if let Some(transfers) = self.scalar_destination(state, destination) {
                paths.push((destination.clone(), prefix + transfers));
            }
        };
        match terminator {
            CheckedScalarStateTerminator::Return { statement_ordinal } => append(
                &CheckedScalarBranchDestination::Return {
                    statement_ordinal: *statement_ordinal,
                    is_continuation: false,
                },
                0,
            ),
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal,
                when_true,
                when_false,
            } => {
                let prefix = self.scalar_at(
                    state,
                    *guard_statement_ordinal,
                    CheckedScalarExpressionRole::Guard,
                );
                append(when_true, prefix);
                append(when_false, prefix);
            }
            CheckedScalarStateTerminator::Guarded { arms, fallback } => {
                let mut prefix = 0;
                for arm in exits.exits.span(*arms)? {
                    prefix += self.scalar_at(
                        state,
                        arm.guard_statement_ordinal,
                        CheckedScalarExpressionRole::Guard,
                    );
                    append(&arm.destination, prefix);
                }
                if let Some(fallback) = fallback {
                    append(fallback, prefix);
                }
            }
            CheckedScalarStateTerminator::Jump(edge) => {
                append(&CheckedScalarBranchDestination::Jump(edge.clone()), 0);
            }
            CheckedScalarStateTerminator::Crash { .. } => {}
        }
        Some(paths)
    }

    fn for_producer(
        values: &'facts CheckedValueFacts,
        producer: &'facts CheckedUnitEffectOperationPlan,
        local: SymbolHandle,
    ) -> Option<Self> {
        let result = producer.structural_result_binding()?;
        let expression = match producer {
            CheckedUnitEffectOperationPlan::EstablishLocal(
                crate::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
            ) => values.structural_values.nodes.get(*value).expression,
            CheckedUnitEffectOperationPlan::Call(call) => match call.source_site().flatten() {
                Some(crate::NominalMachineUseSite::Expression(expression)) => expression,
                _ => arena::Handle::invalid(),
            },
            _ => arena::Handle::invalid(),
        };
        Some(Self {
            values,
            result,
            local,
            expression,
        })
    }
    fn argument(&self, argument: &CheckedUnitStructuralArgumentPlan) -> usize {
        let whole_owner = argument.access == CheckedStructuralAccess::Owned
            && argument.path.is_empty()
            && argument.type_identity == self.result.type_identity
            && match argument.source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                    binding_ordinal == self.result.binding_ordinal
                }
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                    self.local.is_valid() && symbol == self.local
                }
                CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } => {
                    self.expression.is_valid() && expression == self.expression
                }
                _ => false,
            };
        usize::from(whole_owner) + self.argument_evaluation(argument) + self.produced(argument)
    }

    /// An anonymous operand the call consumes or lends is completed before
    /// the call: a whole owner moved into that construction leaves its source
    /// as surely as one passed directly. The operand's retained value root
    /// names exactly the members it takes; an ambiguous or missing root moves
    /// nothing here, so accounting still demands another disposition.
    fn produced(&self, argument: &CheckedUnitStructuralArgumentPlan) -> usize {
        let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } =
            argument.source
        else {
            return 0;
        };
        let plans = &self.values.structural_values;
        let mut roots = plans
            .roots
            .iter()
            .map(|(_, root)| root)
            .filter(|root| root.expression == expression);
        let (Some(root), None) = (roots.next(), roots.next()) else {
            return 0;
        };
        self.constructed(root.root, plans.calls_for_root(root.root).unwrap_or(&[]))
    }

    /// Whole owners a completed construction takes as its members, plus the
    /// transfers evaluating its children performs.
    fn constructed(
        &self,
        root: CheckedStructuralValueHandle,
        calls: &[CheckedStructuralValueCall],
    ) -> usize {
        let plans = &self.values.structural_values;
        match &plans.nodes.get(root).kind {
            CheckedStructuralValueKind::QuotientCall { arguments, .. } => arguments
                .iter()
                .map(|argument| self.structural(*argument, calls))
                .sum(),
            CheckedStructuralValueKind::Place(source) => self.argument(source),
            crate::record_or_structural_case_pattern!(fields, ..) => plans
                .record_fields
                .span_or_empty(*fields)
                .iter()
                .map(|field| match field.value {
                    CheckedStructuralRecordFieldValue::Scalar(value) => self.computation(value),
                    CheckedStructuralRecordFieldValue::Structural(value) => {
                        self.constructed(value, calls)
                    }
                })
                .sum(),
            CheckedStructuralValueKind::FixedArray { elements } => elements
                .iter()
                .map(|element| self.constructed(*element, calls))
                .sum(),
            _ => self.structural(root, calls),
        }
    }

    fn argument_evaluation(&self, argument: &CheckedUnitStructuralArgumentPlan) -> usize {
        argument
            .computation_operands()
            .map(|operand| self.computation(operand))
            .sum()
    }

    fn scalar(&self, value: &CheckedCallScalarArgument) -> usize {
        match value {
            CheckedCallScalarArgument::Pure(_) => 0,
            CheckedCallScalarArgument::Computation(root) => self.computation(*root),
        }
    }

    fn store(&self, value: &CheckedScalarStoreValue) -> usize {
        match value {
            CheckedScalarStoreValue::Computation(root) => self.computation(*root),
            CheckedScalarStoreValue::Pure(_) | CheckedScalarStoreValue::ScalarResult { .. } => 0,
        }
    }

    fn case(&self, case: &CheckedScalarCaseConstruction) -> usize {
        self.values
            .scalar_computations
            .case_fields
            .span_or_empty(case.fields)
            .iter()
            .map(|field| self.computation(field.value))
            .sum()
    }

    fn pattern(&self, pattern: &CheckedScalarDispatchPattern) -> usize {
        match pattern {
            CheckedScalarDispatchPattern::Value(value) => self.computation(*value),
            CheckedScalarDispatchPattern::Wildcard => 0,
        }
    }

    fn computation(&self, root: CheckedScalarComputationHandle) -> usize {
        let plans = &self.values.scalar_computations;
        match &plans.nodes.get(root).kind {
            CheckedScalarComputationKind::Call {
                callee,
                arguments,
                structural_arguments,
                ..
            } => {
                callee
                    .selector()
                    .map_or(0, |selector| self.computation(selector))
                    + plans
                        .operands
                        .span_or_empty(*arguments)
                        .iter()
                        .map(|argument| self.computation(*argument))
                        .sum::<usize>()
                    + plans
                        .structural_arguments
                        .span_or_empty(*structural_arguments)
                        .iter()
                        .map(|argument| match argument {
                            CheckedScalarComputationStructuralArgument::Place(argument) => {
                                self.argument(argument)
                            }
                            CheckedScalarComputationStructuralArgument::Case(case) => {
                                self.case(case)
                            }
                        })
                        .sum::<usize>()
            }
            CheckedScalarComputationKind::Apply { operands, .. } => plans
                .operands
                .span_or_empty(*operands)
                .iter()
                .map(|operand| self.computation(*operand))
                .sum(),
            CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                self.computation(*condition)
                    + self
                        .computation(*when_true)
                        .min(self.computation(*when_false))
            }
            CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                let mut prefix = 0;
                let alternatives = plans.dispatch_arms.span_or_empty(*arms).iter().map(|arm| {
                    prefix += self.pattern(&arm.pattern);
                    prefix + self.computation(arm.value)
                });
                self.computation(*subject) + alternatives.min().unwrap_or(0)
            }
            CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                self.computation(*left) + self.computation(*right)
            }
            crate::cast_or_qualification_triple_pattern!(operand, ..) => self.computation(*operand),
            CheckedScalarComputationKind::CaseMembership {
                subject,
                receiver_value,
                ..
            } => {
                let observation = match subject {
                    CheckedScalarComputationStructuralArgument::Place(argument) => {
                        self.argument_evaluation(argument)
                    }
                    CheckedScalarComputationStructuralArgument::Case(case) => self.case(case),
                };
                observation
                    + if receiver_value.is_valid() {
                        self.structural(
                            *receiver_value,
                            self.values
                                .structural_values
                                .calls_for_root(*receiver_value)
                                .unwrap_or(&[]),
                        )
                    } else {
                        0
                    }
            }
            CheckedScalarComputationKind::StructuralField {
                subject,
                receiver_value,
                ..
            } => {
                self.argument_evaluation(subject)
                    + if receiver_value.is_valid() {
                        self.structural(
                            *receiver_value,
                            self.values
                                .structural_values
                                .calls_for_root(*receiver_value)
                                .unwrap_or(&[]),
                        )
                    } else {
                        0
                    }
            }
            CheckedScalarComputationKind::ByteSequenceLiteralComparison { subject, .. }
            | CheckedScalarComputationKind::ByteSequencePredicate { subject, .. } => {
                self.argument_evaluation(subject)
            }
            CheckedScalarComputationKind::RetainedConstant { .. }
            | CheckedScalarComputationKind::Value(_) => 0,
        }
    }

    fn structural(
        &self,
        root: CheckedStructuralValueHandle,
        calls: &[CheckedStructuralValueCall],
    ) -> usize {
        let plans = &self.values.structural_values;
        match &plans.nodes.get(root).kind {
            CheckedStructuralValueKind::QuotientCall { arguments, .. } => arguments
                .iter()
                .map(|argument| self.structural(*argument, calls))
                .sum(),
            CheckedStructuralValueKind::Call { .. } => calls
                .iter()
                .find(|call| call.value == root)
                .map_or(0, |call| {
                    call.operands()
                        .iter()
                        .map(|operation| self.operation(operation))
                        .sum::<usize>()
                        + self.operation(call.operation())
                }),
            CheckedStructuralValueKind::Case(case) => self.case(case),
            crate::record_or_structural_case_pattern!(fields, ..) => plans
                .record_fields
                .span_or_empty(*fields)
                .iter()
                .map(|field| match field.value {
                    CheckedStructuralRecordFieldValue::Scalar(value) => self.computation(value),
                    CheckedStructuralRecordFieldValue::Structural(value) => {
                        self.structural(value, calls)
                    }
                })
                .sum(),
            CheckedStructuralValueKind::FixedArray { elements } => elements
                .iter()
                .map(|element| self.structural(*element, calls))
                .sum(),
            CheckedStructuralValueKind::ScalarArray { elements } => elements
                .iter()
                .map(|element| self.computation(*element))
                .sum(),
            CheckedStructuralValueKind::Scalar { value: element }
            | CheckedStructuralValueKind::ZeroedScalarArray { element, .. } => {
                self.computation(*element)
            }
            CheckedStructuralValueKind::Projection { source, .. }
            | CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
                self.structural(*source, calls)
            }
            CheckedStructuralValueKind::Dispatch { subject, arms } => {
                let mut prefix = 0;
                let alternatives = plans.dispatch_arms.span_or_empty(*arms).iter().map(|arm| {
                    prefix += self.pattern(&arm.pattern);
                    prefix + self.structural(arm.value, calls)
                });
                self.computation(*subject) + alternatives.min().unwrap_or(0)
            }
            CheckedStructuralValueKind::BorrowedSliceView { source, region }
            | CheckedStructuralValueKind::MutableSliceView { source, region } => {
                self.argument_evaluation(source)
                    + match region {
                        CheckedElementViewRegion::WholeCollection => 0,
                        CheckedElementViewRegion::RepresentationBytes { start, .. } => {
                            self.computation(*start)
                        }
                    }
            }
            CheckedStructuralValueKind::Reference {
                source,
                source_value,
                ..
            } => {
                self.argument_evaluation(source)
                    + if source_value.is_valid() {
                        self.structural(*source_value, calls)
                    } else {
                        0
                    }
            }
            CheckedStructuralValueKind::Place(source)
            | CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } => {
                self.argument_evaluation(source)
            }
            CheckedStructuralValueKind::ByteLiteralView { .. }
            | CheckedStructuralValueKind::ZeroedStructural {} => 0,
        }
    }

    fn bytes(
        &self,
        value: &CheckedByteSequenceValuePlan,
        root: arena::Handle<CheckedByteSequenceValue>,
    ) -> usize {
        match value.nodes.get(root) {
            CheckedByteSequenceValue::Descriptor { source, .. } => self.argument_evaluation(source),
            CheckedByteSequenceValue::Concatenation { left, right, .. } => {
                self.bytes(value, *left) + self.bytes(value, *right)
            }
            CheckedByteSequenceValue::Literal(_)
            | CheckedByteSequenceValue::CallResult { .. }
            | CheckedByteSequenceValue::Whole { .. }
            | CheckedByteSequenceValue::Field { .. } => 0,
        }
    }

    fn operation(&self, operation: &CheckedUnitEffectOperationPlan) -> usize {
        match operation {
            CheckedUnitEffectOperationPlan::Call(call) => {
                call.structural_arguments
                    .iter()
                    .map(|argument| self.argument(argument))
                    .sum::<usize>()
                    + call
                        .scalar_arguments
                        .iter()
                        .map(|argument| self.scalar(argument))
                        .sum::<usize>()
            }
            CheckedUnitEffectOperationPlan::EstablishLocal(binding) => match binding {
                crate::CheckedUnitLocalBindingPlan::Scalar { value, .. }
                | crate::CheckedUnitLocalBindingPlan::Primitive { value, .. } => self.scalar(value),
                crate::CheckedUnitLocalBindingPlan::ScalarArray { elements, .. } => {
                    elements.iter().map(|value| self.scalar(value)).sum()
                }
                crate::CheckedUnitLocalBindingPlan::StructuralValue { value, calls, .. } => {
                    self.structural(*value, calls)
                }
                crate::CheckedUnitLocalBindingPlan::ByteSequence { value, .. } => {
                    self.bytes(value, value.root)
                }
                crate::CheckedUnitLocalBindingPlan::TrivialAffine { .. }
                | crate::CheckedUnitLocalBindingPlan::Structural { .. } => 0,
            },
            // An installed binding that names a structural local is the
            // local's whole occurrence, counted as an ordinary argument.
            CheckedUnitEffectOperationPlan::Store(store) => match &store.value {
                CheckedStoreValue::Scalar { value, .. } => self.store(value),
                CheckedStoreValue::Bytes(value) => self.bytes(value, value.root),
                CheckedStoreValue::Place(value)
                    if store.installed_value().is_some()
                        && matches!(
                            value.source,
                            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
                        ) =>
                {
                    self.argument(value)
                }
                CheckedStoreValue::Place(value) => self.argument_evaluation(value),
                CheckedStoreValue::CaseLiteral(_) => 0,
            },
            CheckedUnitEffectOperationPlan::EstablishReference { source, .. }
            | CheckedUnitEffectOperationPlan::EstablishViewSubslice { source, .. }
            | CheckedUnitEffectOperationPlan::MoveStructuralField { source, .. } => {
                self.argument_evaluation(source)
            }
            CheckedUnitEffectOperationPlan::CaptureAssignmentDestination {
                destination, ..
            } => self.argument_evaluation(destination),
            CheckedUnitEffectOperationPlan::AtomicAccess(atomic) => atomic
                .event
                .operands()
                .iter()
                .map(|operand| self.scalar(operand))
                .sum(),
            CheckedUnitEffectOperationPlan::AtomicFence { argument, .. } => self.argument(argument),
            CheckedUnitEffectOperationPlan::ReleaseReference { .. }
            | CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
            | CheckedUnitEffectOperationPlan::PortWrite { .. }
            | CheckedUnitEffectOperationPlan::Complete { .. } => 0,
        }
    }
}

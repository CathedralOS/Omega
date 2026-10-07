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
    CheckedElementViewRegion, CheckedScalarCaseConstruction, CheckedScalarComputationHandle,
    CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument,
    CheckedScalarDispatchPattern, CheckedStoreValue, CheckedStructuralAccess,
    CheckedStructuralRecordFieldValue, CheckedStructuralValueCall, CheckedStructuralValueHandle,
    CheckedStructuralValueKind, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan,
    CheckedValueFacts,
};
use symbols::SymbolHandle;

impl CheckedUnitEffectOperationPlan {
    /// Rebinding starts a new occurrence at the same named local. Consumers
    /// through this operation can use its prior occurrence; later consumers
    /// must not be charged to the local's initial producer.
    pub fn ends_local_occurrence(&self, local: SymbolHandle) -> bool {
        local.is_valid()
            && matches!(self,
            Self::RebindStructuralLocal { destination, .. }
                if destination.source == (CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: local }))
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
        let Some(result) = producer.structural_result_binding() else {
            return 0;
        };
        let expression = match producer {
            Self::EstablishStructuralValue { value, .. } => {
                values.structural_values.nodes.get(*value).expression
            }
            Self::Call(call) => match call.source_site().flatten() {
                Some(crate::NominalMachineUseSite::Expression(expression)) => expression,
                _ => arena::Handle::invalid(),
            },
            _ => arena::Handle::invalid(),
        };
        CallTransfers {
            values,
            result,
            local,
            expression,
        }
        .operation(self)
    }
}

struct CallTransfers<'facts> {
    values: &'facts CheckedValueFacts,
    result: &'facts CheckedUnitStructuralResultBindingPlan,
    local: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
}

impl CallTransfers<'_> {
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
        usize::from(whole_owner) + self.argument_evaluation(argument)
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

    fn store(&self, value: &CheckedStoreValue) -> usize {
        match value {
            CheckedStoreValue::Computation(root) => self.computation(*root),
            CheckedStoreValue::Pure(_) | CheckedStoreValue::ScalarResult { .. } => 0,
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
            CheckedScalarComputationKind::Qualification { operand, .. }
            | CheckedScalarComputationKind::AuthoredCast { operand, .. }
            | CheckedScalarComputationKind::BooleanToInteger { operand, .. } => {
                self.computation(*operand)
            }
            CheckedScalarComputationKind::CaseMembership { subject, .. } => match subject {
                CheckedScalarComputationStructuralArgument::Place(argument) => {
                    self.argument_evaluation(argument)
                }
                CheckedScalarComputationStructuralArgument::Case(case) => self.case(case),
            },
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
            CheckedScalarComputationKind::Value(_) => 0,
        }
    }

    fn structural(
        &self,
        root: CheckedStructuralValueHandle,
        calls: &[CheckedStructuralValueCall],
    ) -> usize {
        let plans = &self.values.structural_values;
        match &plans.nodes.get(root).kind {
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
            CheckedStructuralValueKind::Record { fields, .. }
            | CheckedStructuralValueKind::StructuralCase { fields, .. } => plans
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
            CheckedStructuralValueKind::ZeroedScalarArray { element, .. } => {
                self.computation(*element)
            }
            CheckedStructuralValueKind::Projection { source, .. } => {
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
            CheckedStructuralValueKind::Place(source)
            | CheckedStructuralValueKind::Reference { source }
            | CheckedStructuralValueKind::ScalarCasePlace { source, .. }
            | CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } => {
                self.argument_evaluation(source)
            }
            CheckedStructuralValueKind::ByteLiteralView { .. }
            | CheckedStructuralValueKind::ViewElementCopy { .. }
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
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { value, .. }
            | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { value, .. }
            | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { value, .. } => {
                self.scalar(value)
            }
            CheckedUnitEffectOperationPlan::EstablishScalarArray { elements, .. } => {
                elements.iter().map(|value| self.scalar(value)).sum()
            }
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, calls, .. } => {
                self.structural(*value, calls)
            }
            CheckedUnitEffectOperationPlan::EstablishByteSequence { value, .. } => {
                self.bytes(value, value.root)
            }
            CheckedUnitEffectOperationPlan::ByteSequenceStore(store) => {
                self.bytes(&store.value, store.value.root)
            }
            CheckedUnitEffectOperationPlan::EstablishReference { source, .. }
            | CheckedUnitEffectOperationPlan::EstablishViewSubslice { source, .. }
            | CheckedUnitEffectOperationPlan::MoveStructuralField { source, .. } => {
                self.argument_evaluation(source)
            }
            CheckedUnitEffectOperationPlan::StoreStructuralField {
                destination, value, ..
            }
            | CheckedUnitEffectOperationPlan::RebindStructuralLocal {
                destination, value, ..
            } => {
                self.argument_evaluation(destination)
                    + if matches!(
                        value.source,
                        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
                    ) {
                        self.argument(value)
                    } else {
                        self.argument_evaluation(value)
                    }
            }
            CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(store) => {
                self.argument_evaluation(&store.value)
            }
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
                self.store(&store.value)
            }
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
                self.store(&store.value)
            }
            CheckedUnitEffectOperationPlan::ByteSequenceWrite(store) => self.store(&store.value),
            CheckedUnitEffectOperationPlan::ElementViewWrite(store) => self.store(&store.value),
            CheckedUnitEffectOperationPlan::RepresentationViewStore(store) => {
                self.store(&store.value)
            }
            CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(_) => 0,
            CheckedUnitEffectOperationPlan::AtomicAccess(atomic) => atomic
                .event
                .operands()
                .iter()
                .map(|operand| self.scalar(operand))
                .sum(),
            CheckedUnitEffectOperationPlan::ReleaseReference { .. }
            | CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
            | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
            | CheckedUnitEffectOperationPlan::EstablishStructuralLocal { .. }
            | CheckedUnitEffectOperationPlan::PortWrite { .. }
            | CheckedUnitEffectOperationPlan::Complete { .. } => 0,
        }
    }
}

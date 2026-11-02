//! Scalar computations can consume structural owners. Cleanup must follow
//! their retained evaluation graph just as it follows standalone calls:
//! sequential operands accumulate moves, selected alternatives intersect the
//! definite moves, and neither path grants a move from an observation.

use super::{Query, Transfers, ValueReceiver};
use checked_trees::{
    CheckedByteSequenceValue as SequenceValue, CheckedByteSequenceValuePlan,
    CheckedCallScalarArgument as ScalarArgument, CheckedScalarComputationHandle,
    CheckedScalarComputationKind as ComputationKind,
    CheckedScalarComputationStructuralArgument as StructuralArgument, CheckedScalarStoreValue,
    CheckedStoreValue, CheckedUnitEffectOperationPlan as OperationPlan,
    CheckedUnitLocalBindingPlan as BindingPlan, CheckedUnitStructuralArgumentPlan,
};

impl Query<'_> {
    pub(super) fn dispatch_subject(
        &self,
        subject: &checked_trees::CheckedDispatchSubject,
    ) -> Option<Transfers> {
        let mut output = Transfers::default();
        for operand in subject.scalar_operands(&self.facts.values.scalar_computations.operands) {
            output.append(self.computation(operand)?);
        }
        let value = subject.structural_value();
        if value.is_valid() {
            output.append(self.value(
                value,
                &[],
                false,
                ValueReceiver::Unbound,
                &[],
                &mut Vec::new(),
            )?);
        }
        Some(output)
    }
    pub(super) fn operation_computations(&self, operation: &OperationPlan) -> Option<Transfers> {
        let mut output = Transfers::default();
        if let Some(call) = operation.call() {
            for operand in call.scalar_operands() {
                output.append(self.scalar(operand)?);
            }
            for argument in &call.structural_arguments {
                output.append(self.argument_computations(argument)?);
            }
        }
        match operation {
            OperationPlan::EstablishLocal(binding) => match binding {
                BindingPlan::Scalar { value, .. } | BindingPlan::Primitive { value, .. } => {
                    output.append(self.scalar(value)?)
                }
                BindingPlan::ScalarArray { elements, .. } => {
                    for element in elements {
                        output.append(self.scalar(element)?);
                    }
                }
                BindingPlan::ByteSequence { value, .. } => {
                    output.append(self.bytes(value, value.root)?)
                }
                _ => {}
            },
            OperationPlan::Store(store) => match &store.value {
                CheckedStoreValue::Scalar {
                    value: CheckedScalarStoreValue::Computation(root),
                    ..
                } => {
                    output.append(self.computation(*root)?);
                }
                CheckedStoreValue::Place(argument) => {
                    output.append(self.argument_computations(argument)?)
                }
                CheckedStoreValue::Bytes(value) => output.append(self.bytes(value, value.root)?),
                _ => {}
            },
            OperationPlan::EstablishReference { source, .. }
            | OperationPlan::EstablishViewSubslice { source, .. }
            | OperationPlan::MoveStructuralField { source, .. } => {
                output.append(self.argument_computations(source)?);
            }
            OperationPlan::CaptureAssignmentDestination { destination, .. } => {
                output.append(self.argument_computations(destination)?);
            }
            OperationPlan::AtomicAccess(atomic) => {
                for operand in atomic.event.operands() {
                    output.append(self.scalar(operand)?);
                }
            }
            OperationPlan::AtomicFence { argument, .. } => {
                output.append(self.argument_computations(argument)?);
            }
            _ => {}
        }
        Some(output)
    }

    fn scalar(&self, operand: &ScalarArgument) -> Option<Transfers> {
        match operand {
            ScalarArgument::Pure(_) => Some(Transfers::default()),
            ScalarArgument::Computation(root) => self.computation(*root),
        }
    }

    pub(super) fn argument_computations(
        &self,
        argument: &CheckedUnitStructuralArgumentPlan,
    ) -> Option<Transfers> {
        let mut output = Transfers::default();
        for operand in
            argument.computation_operands(&self.facts.values.scalar_computations.operands)
        {
            output.append(self.computation(operand)?);
        }
        Some(output)
    }

    fn structural_operand_computations(&self, operand: &StructuralArgument) -> Option<Transfers> {
        match operand {
            StructuralArgument::Place(argument) => self.argument_computations(argument),
            StructuralArgument::Case(case) => {
                let mut output = Transfers::default();
                for field in self
                    .facts
                    .values
                    .scalar_computations
                    .case_fields
                    .span(case.fields)?
                {
                    output.append(self.computation(field.value)?);
                }
                Some(output)
            }
        }
    }

    fn bytes(
        &self,
        value: &CheckedByteSequenceValuePlan,
        root: arena::Handle<SequenceValue>,
    ) -> Option<Transfers> {
        match value.nodes.get(root) {
            SequenceValue::Descriptor { source, .. } => self.argument_computations(source),
            SequenceValue::Concatenation { left, right, .. } => {
                let mut output = self.bytes(value, *left)?;
                output.append(self.bytes(value, *right)?);
                Some(output)
            }
            _ => Some(Transfers::default()),
        }
    }

    pub(super) fn computation(&self, root: CheckedScalarComputationHandle) -> Option<Transfers> {
        if self.completed_computations.contains(&root) {
            return Some(Transfers::default());
        }
        let plans = &self.facts.values.scalar_computations;
        if !plans.nodes.is_valid(root) {
            return None;
        }
        let mut output = Transfers::default();
        match &plans.nodes.get(root).kind {
            ComputationKind::Call {
                source_call,
                callee,
                arguments,
                structural_arguments,
                ..
            } => {
                if let Some(selector) = callee.selector() {
                    output.append(self.computation(selector)?);
                }
                for argument in plans.operands.span(*arguments)? {
                    output.append(self.computation(*argument)?);
                }
                for operand in plans.structural_arguments.span(*structural_arguments)? {
                    output.append(self.structural_operand_computations(operand)?);
                    if let StructuralArgument::Place(argument) = operand {
                        let call = self.facts.flow.control.calls.get(source_call.statement()?);
                        output.append(self.argument_transfers(
                            argument,
                            ValueReceiver::Call {
                                statement_index: u32::try_from(call.statement_index).ok()?,
                                structural_result: false,
                            },
                            &mut Vec::new(),
                        )?);
                    }
                }
            }
            ComputationKind::WithOperands {
                operands, operand, ..
            } => {
                let mut scope = self.clone();
                for dependency in plans.evaluation_operands.span(*operands)? {
                    match dependency {
                        checked_trees::CheckedScalarEvaluationOperand::Scalar(value) => {
                            output.append(scope.computation(*value)?);
                            scope.completed_computations.push(*value);
                        }
                        checked_trees::CheckedScalarEvaluationOperand::Structural(value) => {
                            output.append(scope.value(
                                *value,
                                &[],
                                false,
                                ValueReceiver::Unbound,
                                &[],
                                &mut Vec::new(),
                            )?);
                            scope.established_values.push(*value);
                        }
                    }
                }
                output.append(scope.computation(*operand)?);
            }
            ComputationKind::Apply { operands, .. }
            | ComputationKind::StorageObservation { operands, .. } => {
                for operand in plans.operands.span(*operands)? {
                    output.append(self.computation(*operand)?);
                }
            }
            ComputationKind::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                output.append(self.computation(*condition)?);
                output.append(Transfers::alternatives(vec![
                    self.computation(*when_true)?,
                    self.computation(*when_false)?,
                ]));
            }
            ComputationKind::Dispatch { subject, arms, .. } => {
                output.append(self.dispatch_subject(subject)?);
                let mut prefix = Vec::new();
                let mut alternatives = Vec::new();
                for arm in plans.dispatch_arms.span(*arms)? {
                    if let Some(pattern) = arm.pattern.computation() {
                        prefix.push(pattern);
                    }
                    let mut branch = Transfers::default();
                    for pattern in &prefix {
                        branch.append(self.computation(*pattern)?);
                    }
                    branch.append(self.computation(arm.value)?);
                    alternatives.push(branch);
                }
                output.append(Transfers::alternatives(alternatives));
            }
            ComputationKind::SelectedComparison { left, right, .. } => {
                output.append(self.computation(*left)?);
                output.append(self.computation(*right)?);
            }
            checked_trees::cast_or_qualification_triple_pattern!(operand, ..) => {
                output.append(self.computation(*operand)?)
            }
            ComputationKind::CaseMembership { subject, .. } => {
                output.append(self.structural_operand_computations(subject)?)
            }
            ComputationKind::StorageAddress { subject, .. }
            | ComputationKind::StructuralField { subject, .. }
            | ComputationKind::ByteSequenceLiteralComparison { subject, .. }
            | ComputationKind::ByteSequencePredicate { subject, .. } => {
                output.append(self.argument_computations(subject)?)
            }
            ComputationKind::RetainedConstant { .. } | ComputationKind::Value(_) => {}
        }
        Some(output)
    }
}

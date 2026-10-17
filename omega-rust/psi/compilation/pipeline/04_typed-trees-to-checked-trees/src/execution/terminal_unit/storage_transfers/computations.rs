//! Scalar computations can consume structural owners. Cleanup must follow
//! their retained evaluation graph just as it follows standalone calls:
//! sequential operands accumulate moves, selected alternatives intersect the
//! definite moves, and neither path grants a move from an observation.

use super::{Destination, Query, Transfers};
use checked_trees::{
    CheckedByteSequenceValue, CheckedByteSequenceValuePlan, CheckedCallScalarArgument,
    CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarComputationStructuralArgument, CheckedScalarDispatchPattern,
    CheckedScalarStoreValue, CheckedStoreValue, CheckedUnitEffectOperationPlan,
    CheckedUnitLocalBindingPlan, CheckedUnitStructuralArgumentPlan,
};

impl Query<'_> {
    pub(super) fn operation_computations(
        &self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Option<Transfers> {
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
            CheckedUnitEffectOperationPlan::EstablishLocal(binding) => match binding {
                CheckedUnitLocalBindingPlan::Scalar { value, .. }
                | CheckedUnitLocalBindingPlan::Primitive { value, .. } => {
                    output.append(self.scalar(value)?)
                }
                CheckedUnitLocalBindingPlan::ScalarArray { elements, .. } => {
                    for element in elements {
                        output.append(self.scalar(element)?);
                    }
                }
                CheckedUnitLocalBindingPlan::ByteSequence { value, .. } => {
                    output.append(self.bytes(value, value.root)?)
                }
                _ => {}
            },
            CheckedUnitEffectOperationPlan::Store(store) => match &store.value {
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
            CheckedUnitEffectOperationPlan::EstablishReference { source, .. }
            | CheckedUnitEffectOperationPlan::EstablishViewSubslice { source, .. }
            | CheckedUnitEffectOperationPlan::MoveStructuralField { source, .. } => {
                output.append(self.argument_computations(source)?);
            }
            CheckedUnitEffectOperationPlan::CaptureAssignmentDestination {
                destination, ..
            } => {
                output.append(self.argument_computations(destination)?);
            }
            CheckedUnitEffectOperationPlan::AtomicAccess(atomic) => {
                for operand in atomic.event.operands() {
                    output.append(self.scalar(operand)?);
                }
            }
            CheckedUnitEffectOperationPlan::AtomicFence { argument, .. } => {
                output.append(self.argument_computations(argument)?);
            }
            _ => {}
        }
        Some(output)
    }

    fn scalar(&self, operand: &CheckedCallScalarArgument) -> Option<Transfers> {
        match operand {
            CheckedCallScalarArgument::Pure(_) => Some(Transfers::default()),
            CheckedCallScalarArgument::Computation(root) => self.computation(*root),
        }
    }

    pub(super) fn argument_computations(
        &self,
        argument: &CheckedUnitStructuralArgumentPlan,
    ) -> Option<Transfers> {
        let mut output = Transfers::default();
        for operand in argument.computation_operands() {
            output.append(self.computation(operand)?);
        }
        Some(output)
    }

    fn structural_operand_computations(
        &self,
        operand: &CheckedScalarComputationStructuralArgument,
    ) -> Option<Transfers> {
        match operand {
            CheckedScalarComputationStructuralArgument::Place(argument) => {
                self.argument_computations(argument)
            }
            CheckedScalarComputationStructuralArgument::Case(case) => {
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
        root: arena::Handle<CheckedByteSequenceValue>,
    ) -> Option<Transfers> {
        match value.nodes.get(root) {
            CheckedByteSequenceValue::Descriptor { source, .. } => {
                self.argument_computations(source)
            }
            CheckedByteSequenceValue::Concatenation { left, right, .. } => {
                let mut output = self.bytes(value, *left)?;
                output.append(self.bytes(value, *right)?);
                Some(output)
            }
            _ => Some(Transfers::default()),
        }
    }

    pub(super) fn computation(&self, root: CheckedScalarComputationHandle) -> Option<Transfers> {
        let plans = &self.facts.values.scalar_computations;
        if !plans.nodes.is_valid(root) {
            return None;
        }
        let mut output = Transfers::default();
        match &plans.nodes.get(root).kind {
            CheckedScalarComputationKind::Call {
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
                let call = self.facts.flow.control.calls.get(*source_call);
                for operand in plans.structural_arguments.span(*structural_arguments)? {
                    output.append(self.structural_operand_computations(operand)?);
                    if let CheckedScalarComputationStructuralArgument::Place(argument) = operand
                        && self.belongs(argument)
                    {
                        output.append(self.leaf(
                            argument.path.clone(),
                            Destination::Call {
                                statement_index: u32::try_from(call.statement_index).ok()?,
                                structural_result: false,
                            },
                        ));
                    }
                }
            }
            CheckedScalarComputationKind::Apply { operands, .. }
            | CheckedScalarComputationKind::StorageObservation { operands, .. } => {
                for operand in plans.operands.span(*operands)? {
                    output.append(self.computation(*operand)?);
                }
            }
            CheckedScalarComputationKind::Select {
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
            CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                output.append(self.computation(*subject)?);
                let mut prefix = Vec::new();
                let mut alternatives = Vec::new();
                for arm in plans.dispatch_arms.span(*arms)? {
                    if let CheckedScalarDispatchPattern::Value(pattern) = arm.pattern {
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
            CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                output.append(self.computation(*left)?);
                output.append(self.computation(*right)?);
            }
            checked_trees::cast_or_qualification_triple_pattern!(operand, ..) => {
                output.append(self.computation(*operand)?)
            }
            CheckedScalarComputationKind::CaseMembership { subject, .. } => {
                output.append(self.structural_operand_computations(subject)?)
            }
            CheckedScalarComputationKind::StructuralField { subject, .. }
            | CheckedScalarComputationKind::ByteSequenceLiteralComparison { subject, .. }
            | CheckedScalarComputationKind::ByteSequencePredicate { subject, .. } => {
                output.append(self.argument_computations(subject)?)
            }
            CheckedScalarComputationKind::RetainedConstant { .. }
            | CheckedScalarComputationKind::Value(_) => {}
        }
        Some(output)
    }
}

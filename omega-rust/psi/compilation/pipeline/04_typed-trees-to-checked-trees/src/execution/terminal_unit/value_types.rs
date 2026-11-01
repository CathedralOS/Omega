//! Retain every evaluated structural producer's complete type vocabulary.
//!
//! A scalar computation can own a structural receiver, dispatch subject,
//! authority, argument, or sequencing operand. Calls are one such production,
//! not the inventory of values scalar evaluation may establish. Follow those
//! checked relationships and structural children before catalog pruning. A
//! descriptor's occurrence type can differ from its receiving call signature
//! after runtime indices are erased; both remain real type dependencies.
//! This retains declarations only, never construction or loan authority.

use super::{CheckFacts, types};
use checked_trees::{
    CheckedScalarComputationKind as Computation, CheckedScalarEvaluationOperand,
    CheckedStructuralValueHandle, CheckedUnitStructuralArgumentSourcePlan,
};

/// Direct structural dependencies of the complete checked scalar roster.
/// Their scalar children are already in that roster; `retain` follows the
/// structural graph. No source expression is reparsed or evaluated here.
fn scalar_value_roots(facts: &CheckFacts) -> Vec<CheckedStructuralValueHandle> {
    let mut by_expression: symbols::SymbolKeyMap<
        typed_trees::expression::ExpressionHandle,
        Vec<CheckedStructuralValueHandle>,
    > = symbols::SymbolKeyMap::default();
    for (_, root) in facts.values.structural_values.roots.iter() {
        by_expression
            .entry(root.expression)
            .or_default()
            .push(root.root);
    }
    let computations = &facts.values.scalar_computations;
    let mut roots = Vec::new();
    for (_, node) in computations.nodes.iter() {
        match &node.kind {
            Computation::CaseMembership { receiver_value, .. }
            | Computation::StructuralField { receiver_value, .. } => roots.push(*receiver_value),
            Computation::Dispatch { subject, .. } => roots.push(subject.structural_value()),
            Computation::Call {
                callee,
                structural_arguments,
                ..
            } => {
                roots.push(callee.authority_value());
                for argument in computations
                    .structural_arguments
                    .span_or_empty(*structural_arguments)
                {
                    if let Some(argument) = argument.as_place()
                        && let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                            expression,
                        } = argument.source
                        && let Some(values) = by_expression.get(&expression)
                    {
                        roots.extend(values.iter().copied());
                    }
                }
            }
            Computation::WithOperands { operands, .. } => {
                for operand in computations.evaluation_operands.span_or_empty(*operands) {
                    if let CheckedScalarEvaluationOperand::Structural(value) = operand {
                        roots.push(*value);
                    }
                }
            }
            Computation::StorageAddress { .. }
            | Computation::ByteSequenceLiteralComparison { .. }
            | Computation::ByteSequencePredicate { .. }
            | Computation::SelectedComparison { .. }
            | Computation::Qualification { .. }
            | Computation::RetainedConstant { .. }
            | Computation::Value(_)
            | Computation::AuthoredCast { .. }
            | Computation::BooleanToInteger { .. }
            | Computation::Select { .. }
            | Computation::Apply { .. }
            | Computation::StorageObservation { .. } => {}
        }
    }
    roots.retain(|root| root.is_valid());
    roots
}

pub(super) fn retain(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut types::ShapeCollector<'_>,
    mut pending: Vec<checked_trees::CheckedStructuralValueHandle>,
) -> Vec<typed_trees::type_identity::TypeIdentityId> {
    pending.extend(scalar_value_roots(facts));
    let plans = &facts.values.structural_values;
    let mut identities = Vec::new();
    let mut visited = Vec::new();
    while let Some(node) = pending.pop() {
        if visited.contains(&node) {
            continue;
        }
        visited.push(node);
        // A structural result owns its prepared operand schedule. Those
        // operands can form descriptors before the result is produced, even
        // when no scalar node or enclosing statement names their types.
        for call in plans
            .call_schedule(node)
            .map(|(_, calls)| calls)
            .unwrap_or(&[])
        {
            for operation in call
                .operands()
                .iter()
                .chain(call.operation())
                .flat_map(checked_trees::CheckedUnitEffectOperationPlan::with_value_calls)
            {
                if let Some((value, _)) = operation.structural_value_evaluation() {
                    pending.push(value);
                }
            }
        }
        let node = plans.nodes.get(node);
        let binders = program
            .machine_by_symbol(node.machine)
            .map(|machine| types::machine_binders(program, machine))
            .unwrap_or_default();
        if node.type_reference.is_valid()
            && let Some(identity) = shapes.add_stored_type(node.type_reference, &binders, &[])
        {
            identities.push(identity);
        }
        match &node.kind {
            checked_trees::CheckedStructuralValueKind::Place(source) => {
                identities.push(source.type_identity);
            }
            checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { source, .. }
            | checked_trees::CheckedStructuralValueKind::BorrowedSliceView { source, .. }
            | checked_trees::CheckedStructuralValueKind::MutableSliceView { source, .. } => {
                identities.push(source.type_identity);
            }
            checked_trees::CheckedStructuralValueKind::Reference {
                source,
                source_value,
                ..
            } => {
                identities.push(source.type_identity);
                if source_value.is_valid() {
                    pending.push(*source_value);
                }
            }
            checked_trees::CheckedStructuralValueKind::Projection { source, .. }
            | checked_trees::CheckedStructuralValueKind::ViewSubslice { source, .. }
            | checked_trees::CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
                pending.push(*source);
            }
            checked_trees::CheckedStructuralValueKind::QuotientCall { arguments, .. } => {
                pending.extend(arguments.iter().copied());
            }
            checked_trees::CheckedStructuralValueKind::Record { fields, .. }
            | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                for field in plans.record_fields.span_or_empty(*fields) {
                    if let checked_trees::CheckedStructuralRecordFieldValue::Structural(nested) =
                        field.value
                    {
                        pending.push(nested);
                    }
                }
            }
            checked_trees::CheckedStructuralValueKind::FixedArray { elements } => {
                pending.extend(elements.iter().copied());
            }
            checked_trees::CheckedStructuralValueKind::Dispatch { subject, arms } => {
                let value = subject.structural_value();
                if value.is_valid() {
                    pending.push(value);
                }
                pending.extend(
                    plans
                        .dispatch_arms
                        .span_or_empty(*arms)
                        .iter()
                        .map(|arm| arm.value),
                );
            }
            _ => {}
        }
    }
    identities
}

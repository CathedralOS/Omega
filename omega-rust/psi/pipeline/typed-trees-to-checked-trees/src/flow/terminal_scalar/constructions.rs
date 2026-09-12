//! Structural values retain a type namespace without inventing source parameters.

use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationPlans, CheckedUnitStructuralTypePlan,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(super) fn retain_shapes(
    program: &TypedTrees,
    plans: &CheckedScalarComputationPlans,
    state: SymbolHandle,
    shapes: &mut Vec<CheckedUnitStructuralTypePlan>,
) -> Option<()> {
    let mut pending = plans
        .roots
        .iter()
        .filter_map(|(_, root)| (root.state == state).then_some(root.root))
        .collect::<Vec<_>>();
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if !plans.nodes.is_valid(handle) {
            return None;
        }
        if visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        match &plans.nodes.get(handle).kind {
            CheckedScalarComputationKind::CaseMembership {
                subject:
                    checked_trees::CheckedScalarComputationStructuralArgument::Place(_)
                    | checked_trees::CheckedScalarComputationStructuralArgument::Array { .. },
                ..
            } => {}
            CheckedScalarComputationKind::CaseMembership {
                subject: checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
                ..
            } => {
                for shape in super::super::terminal_unit::scalar_case_value_shapes(
                    program,
                    subject.type_reference,
                )? {
                    if let Some(existing) = shapes
                        .iter()
                        .find(|existing| existing.identity == shape.identity)
                    {
                        if existing != &shape {
                            return None;
                        }
                    } else {
                        shapes.push(shape);
                    }
                }
                pending.extend(
                    plans
                        .case_fields
                        .span(subject.fields)?
                        .iter()
                        .map(|field| field.value),
                );
            }
            CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                pending.extend([*left, *right])
            }
            CheckedScalarComputationKind::Qualification { operand, .. } => pending.push(*operand),
            CheckedScalarComputationKind::Value(_) => {}
            CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                pending.push(*subject);
                for arm in plans.dispatch_arms.span(*arms)? {
                    if let checked_trees::CheckedScalarDispatchPattern::Value(value) = arm.pattern {
                        pending.push(value);
                    }
                    pending.push(arm.value);
                }
            }
            CheckedScalarComputationKind::Call {
                arguments,
                structural_arguments,
                ..
            } => {
                pending.extend(plans.operands.span(*arguments)?);
                for argument in plans.structural_arguments.span(*structural_arguments)? {
                    if let checked_trees::CheckedScalarComputationStructuralArgument::Array {
                        elements,
                        ..
                    } = argument
                    {
                        pending.extend(plans.operands.span(*elements)?);
                    }
                }
            }
            CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
                ..
            } => pending.extend([*condition, *when_true, *when_false]),
            CheckedScalarComputationKind::Apply { operands, .. } => {
                pending.extend(plans.operands.span(*operands)?)
            }
        }
    }
    Some(())
}

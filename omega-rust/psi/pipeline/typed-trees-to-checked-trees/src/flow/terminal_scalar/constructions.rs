//! Structural values retain a type namespace without inventing source parameters.

use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationPlans, CheckedUnitStructuralTypePlan,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

/// This operation slice establishes fresh records; structural calls, ownership
/// transfers, references and selected structural results keep their existing
/// owners until their custody joins scalar graph emission. Scalar operands are
/// ordinary computations, not a literal-only constructor shortcut.
pub(super) fn fresh_record_root<'plans>(
    program: &TypedTrees,
    plans: &'plans checked_trees::CheckedStructuralValuePlans,
    machine: SymbolHandle,
    state: SymbolHandle,
    ordinal: u32,
    local: &typed_trees::statement::TableLocalData,
) -> Option<&'plans checked_trees::CheckedStructuralValueRoot> {
    let root = plans.root_at(state, ordinal)?;
    if local.is_mutable
        || root.machine != machine
        || root.expression != local.initial_value
        || root.type_reference != local.type_reference
    {
        return None;
    }
    let mut pending = vec![root.root];
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if !plans.nodes.is_valid(handle) || visited.contains(&handle) {
            return None;
        }
        visited.push(handle);
        let node = plans.nodes.get(handle);
        if !program
            .expression_table
            .expression_is_valid(node.expression)
        {
            return None;
        }
        let checked_trees::CheckedStructuralValueKind::Record { fields, .. } = &node.kind else {
            return None;
        };
        for field in plans.record_fields.span(*fields)? {
            if let checked_trees::CheckedStructuralRecordFieldValue::Structural(child) = field.value
            {
                pending.push(child);
            }
        }
    }
    Some(root)
}

pub(super) fn retain_record_locals(
    program: &TypedTrees,
    plans: &checked_trees::CheckedStructuralValuePlans,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    shapes: &mut Vec<CheckedUnitStructuralTypePlan>,
) -> Option<()> {
    use typed_trees::statement::StatementNode;
    for (ordinal, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take_while(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(_) | StatementNode::Assignment(_) | StatementNode::Call(_)
            )
        })
        .enumerate()
    {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        if program
            .primitive_type_reference(local.type_reference)
            .is_some()
        {
            continue;
        }
        fresh_record_root(
            program,
            plans,
            machine.symbol,
            state.symbol,
            u32::try_from(ordinal).ok()?,
            local,
        )?;
        for shape in
            super::super::terminal_unit::scalar_graph_record_shapes(program, local.type_reference)?
        {
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
    }
    Some(())
}

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
            CheckedScalarComputationKind::Value(_)
            | CheckedScalarComputationKind::StructuralField { .. } => {}
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

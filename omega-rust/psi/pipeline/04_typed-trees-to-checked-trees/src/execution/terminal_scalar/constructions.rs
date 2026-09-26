//! Structural values retain a type namespace without inventing source parameters.

use crate::checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationPlans, CheckedUnitStructuralTypePlan,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbols::SymbolHandle;

/// Fresh records and whole-place copies or moves share ordered value
/// establishment. Structural calls, references and selected
/// results keep their existing owners until their custody joins scalar graph
/// emission. Scalar operands are ordinary computations.
pub(super) fn record_value_root<'plans>(
    program: &TypedTrees,
    plans: &'plans crate::checked_trees::CheckedStructuralValuePlans,
    machine: SymbolHandle,
    state: SymbolHandle,
    ordinal: u32,
    local: &symbol_resolved_trees_to_typed_trees::typed_trees::statement::TableLocalData,
) -> Option<&'plans crate::checked_trees::CheckedStructuralValueRoot> {
    let root = plans.root_at(state, ordinal)?;
    if root.machine != machine
        || root.expression != local.initial_value
        || root.type_reference != local.type_reference
    {
        return None;
    }
    let mut pending = vec![(root.root, local.type_reference)];
    let mut visited = Vec::new();
    while let Some((handle, reference)) = pending.pop() {
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
        match &node.kind {
            crate::checked_trees::CheckedStructuralValueKind::Record { fields, .. }
            | crate::checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                for field in plans.record_fields.span(*fields)? {
                    if let crate::checked_trees::CheckedStructuralRecordFieldValue::Structural(
                        child,
                    ) = field.value
                    {
                        pending.push((child, field.type_reference));
                    }
                }
            }
            crate::checked_trees::CheckedStructuralValueKind::FixedArray { elements } => {
                let symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
                    program.type_reference_table.type_reference(reference)
                else {
                    return None;
                };
                pending.extend(elements.iter().map(|element| (*element, *element_type)));
            }
            crate::checked_trees::CheckedStructuralValueKind::Place(argument)
                if argument.access == crate::checked_trees::CheckedStructuralAccess::Owned
                    && argument.path.is_empty()
                    && matches!(
                        program.type_multiplicity(reference),
                        language_semantics::Multiplicity::Unrestricted
                            | language_semantics::Multiplicity::Affine
                    )
                    && program.normalized_type_identity(reference).as_str()
                        == argument.type_identity => {}
            _ => return None,
        }
    }
    Some(root)
}

pub(super) fn retain_record_locals(
    program: &TypedTrees,
    plans: &crate::checked_trees::CheckedStructuralValuePlans,
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    shapes: &mut Vec<CheckedUnitStructuralTypePlan>,
) -> Option<()> {
    use symbol_resolved_trees_to_typed_trees::typed_trees::statement::StatementNode;
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
        if local.name.as_str().starts_with("__arm_destructure#") {
            // A destructure marker carries provenance about the matched
            // operand, not a runtime construction: it keeps the unit
            // sentinel and establishes no structural value. Admit it when
            // the authored subject resolves to a canonical rooted place.
            if local.is_mutable
                || crate::flow::contextual_canonical_place_from_expression(
                    program,
                    state.symbol,
                    ordinal,
                    local.initial_value,
                )
                .is_none()
            {
                return None;
            }
            continue;
        }
        if program
            .primitive_type_reference(local.type_reference)
            .is_some()
        {
            continue;
        }
        record_value_root(
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
                    crate::checked_trees::CheckedScalarComputationStructuralArgument::Place(_)
                    | crate::checked_trees::CheckedScalarComputationStructuralArgument::Array { .. },
                ..
            } => {}
            CheckedScalarComputationKind::CaseMembership {
                subject:
                    crate::checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
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
            CheckedScalarComputationKind::Qualification { operand, .. }
            | CheckedScalarComputationKind::BooleanToInteger { operand, .. } => {
                pending.push(*operand)
            }
            CheckedScalarComputationKind::Value(_)
            | CheckedScalarComputationKind::StructuralField { .. } => {}
            CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                pending.push(*subject);
                for arm in plans.dispatch_arms.span(*arms)? {
                    if let crate::checked_trees::CheckedScalarDispatchPattern::Value(value) =
                        arm.pattern
                    {
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
                    if let crate::checked_trees::CheckedScalarComputationStructuralArgument::Case(
                        subject,
                    ) = argument
                    {
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
                    if let crate::checked_trees::CheckedScalarComputationStructuralArgument::Array {
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

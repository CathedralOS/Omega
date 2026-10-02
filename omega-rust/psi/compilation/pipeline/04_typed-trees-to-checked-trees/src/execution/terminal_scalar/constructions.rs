//! Structural values retain a type namespace without inventing source parameters.

use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationPlans, CheckedUnitStructuralTypePlan,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

/// Fresh records and whole-place copies or moves share ordered value
/// establishment. Structural calls, references and selected
/// results keep their existing owners until their custody joins scalar graph
/// emission. Scalar operands are ordinary computations.
pub(super) fn record_value_root<'plans>(
    program: &TypedTrees,
    plans: &'plans checked_trees::CheckedStructuralValuePlans,
    machine: SymbolHandle,
    state: SymbolHandle,
    ordinal: u32,
    local: &typed_trees::statement::TableLocalData,
) -> Option<&'plans checked_trees::CheckedStructuralValueRoot> {
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
        // A zeroed establishment keys its node on the local's invalid
        // initializer handle — no authored expression exists to validate.
        if !matches!(
            node.kind,
            checked_trees::CheckedStructuralValueKind::ZeroedStructural {}
        ) && !program
            .expression_table
            .expression_is_valid(node.expression)
        {
            return None;
        }
        match &node.kind {
            // The whole declared shape begins zero and its fields are written
            // by later stores — the establishment carries no operand children.
            checked_trees::CheckedStructuralValueKind::ZeroedStructural {} => {}
            checked_trees::CheckedStructuralValueKind::Record { fields, .. }
            | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                for field in plans.record_fields.span(*fields)? {
                    if let checked_trees::CheckedStructuralRecordFieldValue::Structural(child) =
                        field.value
                    {
                        pending.push((child, field.type_reference));
                    }
                }
            }
            checked_trees::CheckedStructuralValueKind::FixedArray { elements } => {
                let typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
                    program.type_reference_table.type_reference(reference)
                else {
                    return None;
                };
                pending.extend(elements.iter().map(|element| (*element, *element_type)));
            }
            checked_trees::CheckedStructuralValueKind::Place(argument)
                if argument.access == checked_trees::CheckedStructuralAccess::Owned
                    && argument.path.is_empty()
                    && matches!(
                        program.type_multiplicity(reference),
                        language_semantics::Multiplicity::Unrestricted
                            | language_semantics::Multiplicity::Affine
                    )
                    && program.normalized_type_identity(reference).as_str()
                        == argument.type_identity => {}
            // A whole read of an earlier copyable local snapshots its current
            // contents. The graph binds that snapshot into a fresh home exactly
            // as it copies an owned unrestricted transfer.
            checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { source, .. }
                if source.access == checked_trees::CheckedStructuralAccess::SharedBorrow
                    && source.path.is_empty()
                    && matches!(
                        source.source,
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                            symbol,
                        } if crate::semantic::calls::find_state(program, state)
                            .and_then(|authored| program.state_local_declaration(authored, symbol))
                            .is_some_and(|(local_ordinal, local)| {
                                u32::try_from(local_ordinal).is_ok_and(|position| position < ordinal)
                                    && local.initial_value.is_valid()
                            })
                    )
                    && program.type_multiplicity(reference)
                        == language_semantics::Multiplicity::Unrestricted
                    && program.normalized_type_identity(reference).as_str()
                        == source.type_identity => {}
            _ => return None,
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
            CheckedScalarComputationKind::Qualification { operand, .. }
            | CheckedScalarComputationKind::AuthoredCast { operand, .. }
            | CheckedScalarComputationKind::BooleanToInteger { operand, .. } => {
                pending.push(*operand)
            }
            CheckedScalarComputationKind::ByteSequenceLiteralComparison {
                operand_type, ..
            }
            | CheckedScalarComputationKind::ByteSequencePredicate { operand_type, .. } => {
                for shape in
                    super::super::terminal_unit::scalar_operand_shapes(program, *operand_type)?
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
                    if let checked_trees::CheckedScalarComputationStructuralArgument::Case(
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
            CheckedScalarComputationKind::Apply { operands, .. }
            | CheckedScalarComputationKind::BoundaryCall {
                arguments: operands,
                ..
            } => pending.extend(plans.operands.span(*operands)?),
        }
    }
    Some(())
}

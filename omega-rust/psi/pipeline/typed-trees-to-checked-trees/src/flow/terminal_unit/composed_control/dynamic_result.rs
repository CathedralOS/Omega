//! Immediate binary Unit control over one direct named-dynamic scalar result.

use super::*;

pub(crate) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    machine: &typed_trees::machine::Machine,
    entry: &typed_trees::state::State,
    dynamic: &checked_trees::CheckedDynamicScalarCallPlan,
    stored: Option<&checked_trees::DynamicDescriptorStorageFact>,
) -> Option<checked_trees::CheckedDynamicUnitContinuationPlan> {
    if dynamic.caller_structural_scalar_field_store.is_some() {
        return None;
    }
    let [machine_entry, when_true_state, when_false_state] = program.machine_states(machine) else {
        return None;
    };
    if machine_entry.symbol != entry.symbol {
        return None;
    }
    let statements = program.statement_table.statements(entry.statement_nodes);
    let (StatementNode::Transition(when_false), preceding) = statements.split_last()? else {
        return None;
    };
    let (StatementNode::Transition(when_true), prefix) = preceding.split_last()? else {
        return None;
    };
    let guard_ordinal = u32::try_from(prefix.len()).ok()?;
    let TransitionGuardNode::When(_) = when_true.guard else {
        return None;
    };
    if guard_ordinal != dynamic.coordinate.statement_index.checked_add(1)?
        || when_true.exit != TransitionExit::Ordinary
        || when_false.exit != TransitionExit::Ordinary
        || when_false.guard != TransitionGuardNode::Always
        || when_true.continuation.is_valid()
        || when_false.continuation.is_valid()
    {
        return None;
    }
    let binders = machine_binders(program, machine);
    let mut attachment = None;
    for state in [entry, when_true_state, when_false_state] {
        if !is_unit(program, state.return_type) || !program.state_contracts(state).is_empty() {
            return None;
        }
        let (state_attachment, structural, scalar) =
            structural_scalar_signature(program, shapes, machine, state, &binders, false)?;
        if !structural.is_empty()
            || !scalar.is_empty()
            || !super::topology::only_implicit_reference_self_is_omitted(
                program,
                state,
                &structural,
                &scalar,
            )
            || attachment
                .as_ref()
                .is_some_and(|identity| identity != &state_attachment)
        {
            return None;
        }
        attachment = Some(state_attachment);
    }
    if attachment.as_deref() != Some(dynamic.caller_attachment_type_identity.as_str()) {
        return None;
    }
    let guard = facts
        .values
        .scalar_expressions
        .expression_at(
            entry.symbol,
            guard_ordinal,
            CheckedScalarExpressionRole::Guard,
        )?
        .clone();
    if !exact_result_guard(&guard, dynamic.result) {
        return None;
    }
    let admitted_local_discards = match stored {
        Some(storage) => {
            let StatementNode::LocalData(destination) = statements.get(storage.statement_index)?
            else {
                return None;
            };
            if storage.machine != machine.symbol
                || storage.state != entry.symbol
                || destination.symbol != storage.destination_binding
                || super::super::types::type_graph_requires_nominal_drop(
                    program,
                    destination.type_reference,
                )
            {
                return None;
            }
            vec![storage.destination_binding]
        }
        None => Vec::new(),
    };
    let successors = [
        super::topology::successor(
            program,
            facts,
            machine,
            entry,
            &[],
            &[],
            &[],
            &[],
            guard_ordinal,
            when_true,
            when_true_state.symbol,
            &admitted_local_discards,
        )?,
        super::topology::successor(
            program,
            facts,
            machine,
            entry,
            &[],
            &[],
            &[],
            &[],
            guard_ordinal.checked_add(1)?,
            when_false,
            when_false_state.symbol,
            &admitted_local_discards,
        )?,
    ];
    let leaves = vec![
        super::leaves::build(
            program,
            facts,
            machine,
            when_true_state,
            boundaries,
            &[],
            &[],
        )?,
        super::leaves::build(
            program,
            facts,
            machine,
            when_false_state,
            boundaries,
            &[],
            &[],
        )?,
    ];
    if leaves.iter().any(|leaf| {
        leaf.operations.iter().any(|operation| match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { .. } => false,
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                claim_transfers,
                ..
            } => !structural_arguments.is_empty() || !claim_transfers.is_empty(),
            _ => true,
        })
    }) {
        return None;
    }
    let true_flow = state_flow(facts, machine.symbol, when_true_state.symbol)?;
    let false_flow = state_flow(facts, machine.symbol, when_false_state.symbol)?;
    let provider_attachment_requirements = checked_composed_provider_attachment_requirements(
        program,
        shapes,
        machine,
        attachment.as_ref()?,
        &[
            (
                when_true_state,
                facts.flow.control.calls.span_or_empty(true_flow.calls),
                &leaves[0].operations,
            ),
            (
                when_false_state,
                facts.flow.control.calls.span_or_empty(false_flow.calls),
                &leaves[1].operations,
            ),
        ],
    )?;
    Some(checked_trees::CheckedDynamicUnitContinuationPlan {
        guard,
        when_true: successors[0].clone(),
        when_false: successors[1].clone(),
        trivial_affine_local_discard: stored.map(|storage| storage.destination_binding),
        leaves,
        provider_attachment_requirements,
    })
}

fn exact_result_guard(
    guard: &CheckedScalarExpression,
    result: CheckedUnitScalarResultBindingPlan,
) -> bool {
    let CheckedScalarExpression::Boolean(boolean) = guard else {
        return false;
    };
    exact_result_boolean_guard(boolean, result)
}

fn exact_result_boolean_guard(
    boolean: &CheckedBooleanExpression,
    result: CheckedUnitScalarResultBindingPlan,
) -> bool {
    let is_result = |expression: &CheckedScalarExpression| {
        matches!(expression,
            CheckedScalarExpression::Local { position, primitive_type }
                if *position == result.binding_ordinal as usize
                    && *primitive_type == result.primitive_type)
    };
    let is_literal = |expression: &CheckedScalarExpression| {
        matches!(expression, CheckedScalarExpression::IntegerLiteral { .. })
    };
    match boolean {
        CheckedBooleanExpression::Local { position } => {
            result.primitive_type == PrimitiveType::Bool
                && *position == result.binding_ordinal as usize
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            (is_result(left) && is_literal(right)) || (is_literal(left) && is_result(right))
        }
        CheckedBooleanExpression::Equal { left, right } => match (left.as_ref(), right.as_ref()) {
            (CheckedBooleanExpression::Constant(true), guard)
            | (guard, CheckedBooleanExpression::Constant(true)) => {
                exact_result_boolean_guard(guard, result)
            }
            _ => false,
        },
        _ => false,
    }
}

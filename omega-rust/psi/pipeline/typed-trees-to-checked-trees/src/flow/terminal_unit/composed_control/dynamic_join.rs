use super::*;

pub(in crate::flow::terminal_unit) struct DynamicJoinControlTopology {
    pub entry_state: SymbolHandle,
    pub attachment_type_identity: String,
    pub scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub guard: CheckedScalarExpression,
    pub successors: [CheckedStructuralControlSuccessorPlan; 2],
}

/// Reuse the ordinary composed-control topology proof for the dynamic join
/// lane. The first rung accepts only an implicit borrowed `self`, one Boolean
/// entry parameter, and two custody-free leaves; the leaf operations are owned
/// by the dynamic call plans instead of the general effect planner.
pub(in crate::flow::terminal_unit) fn admit_dynamic_join_control_topology(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<DynamicJoinControlTopology> {
    let [entry, when_true_state, when_false_state] = program.machine_states(machine) else {
        return None;
    };
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, entry_structural, scalar_parameters) =
        structural_scalar_signature(program, shapes, machine, entry, &binders, false)?;
    let (true_attachment, true_structural, true_scalar) =
        structural_scalar_signature(program, shapes, machine, when_true_state, &binders, false)?;
    let (false_attachment, false_structural, false_scalar) =
        structural_scalar_signature(program, shapes, machine, when_false_state, &binders, false)?;
    let [scalar_parameter] = scalar_parameters.as_slice() else {
        return None;
    };
    if [entry, when_true_state, when_false_state]
        .iter()
        .any(|state| {
            !is_unit(program, state.return_type) || !program.state_contracts(state).is_empty()
        })
        || scalar_parameter.source_position != 1
        || scalar_parameter.primitive_type != PrimitiveType::Bool
        || attachment_type_identity != true_attachment
        || attachment_type_identity != false_attachment
        || !entry_structural.is_empty()
        || !true_structural.is_empty()
        || !false_structural.is_empty()
        || !true_scalar.is_empty()
        || !false_scalar.is_empty()
        || !topology::only_implicit_reference_self_is_omitted(
            program,
            entry,
            &entry_structural,
            &scalar_parameters,
        )
        || !topology::only_implicit_reference_self_is_omitted(
            program,
            when_true_state,
            &true_structural,
            &true_scalar,
        )
        || !topology::only_implicit_reference_self_is_omitted(
            program,
            when_false_state,
            &false_structural,
            &false_scalar,
        )
    {
        return None;
    }
    let [
        StatementNode::Transition(when_true),
        StatementNode::Transition(when_false),
    ] = program.statement_table.statements(entry.statement_nodes)
    else {
        return None;
    };
    if when_true.exit != TransitionExit::Ordinary
        || !matches!(when_true.guard, TransitionGuardNode::When(_))
        || when_false.exit != TransitionExit::Ordinary
        || when_false.guard != TransitionGuardNode::Always
        || when_true.continuation.is_valid()
        || when_false.continuation.is_valid()
    {
        return None;
    }
    let guard = guards::exact_guard(
        facts.values.scalar_expressions.expression_at(
            entry.symbol,
            0,
            CheckedScalarExpressionRole::Guard,
        )?,
        &scalar_parameters,
        &[],
    )?;
    let successors = [
        topology::successor(
            program,
            facts,
            machine,
            entry,
            &[],
            &[],
            &[],
            &[],
            0,
            when_true,
            when_true_state.symbol,
            &[],
        )?,
        topology::successor(
            program,
            facts,
            machine,
            entry,
            &[],
            &[],
            &[],
            &[],
            1,
            when_false,
            when_false_state.symbol,
            &[],
        )?,
    ];
    Some(DynamicJoinControlTopology {
        entry_state: entry.symbol,
        attachment_type_identity,
        scalar_parameters,
        guard,
        successors,
    })
}

use super::*;

pub(super) fn guarded_slice_tail_call(
    program: &TypedTrees,
    machine: &Machine,
    state: &psi_typed_trees::state::State,
    statement: &StatementNode,
    argument: ExpressionHandle,
    measure_position: usize,
) -> bool {
    let Some(witness) = machine.termination_plan.implementation_witness.as_ref() else {
        return false;
    };
    if !matches!(witness.view_path.as_str(), "" | "Slice::Length")
        || !witness.view_arguments.is_empty()
        || (witness.ranking_view.is_valid()
            && witness.ranking_view.canonical_path() != Some("Slice::Length"))
    {
        return false;
    }
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    let Some(parameter) = program.state_parameters(entry).get(measure_position) else {
        return false;
    };
    let Some(subjects) =
        psi_typed_trees::ranking::resolve_machine_witness_subjects(program, machine)
    else {
        return false;
    };
    if !matches!(subjects.as_slice(), [subject] if matches!(
        program.expression_table.expression(*subject),
        ExpressionNode::Name(path) if path.symbol == parameter.symbol
    )) {
        return false;
    }
    if !crate::state_reference_parameter_binding_is_stable(
        program,
        machine,
        state,
        parameter.symbol,
    ) {
        return false;
    }
    let StatementNode::Transition(transition) = statement else {
        return false;
    };
    let TransitionGuardNode::When(guard) = transition.guard else {
        return false;
    };
    if !crate::slice_tail_strictly_decreases(program, guard, argument, parameter) {
        return false;
    }
    let entry_name = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or_default();
    // A repeated argument handle is not call-site identity. In particular,
    // a recursive guard call or false-arm call cannot borrow the true arm's
    // nonempty premise even if its arguments are structurally interned.
    let mut outside_calls = Vec::new();
    collect_self_entry_call_arguments(program, entry_name, guard, &mut outside_calls);
    match program
        .statement_table
        .transition_target(transition.continuation)
    {
        TransitionTargetNode::Value(value) => {
            collect_self_entry_call_arguments(program, entry_name, *value, &mut outside_calls);
        }
        TransitionTargetNode::Named { .. } if transition.continuation.is_valid() => return false,
        _ => {}
    }
    if !outside_calls.is_empty() {
        return false;
    }
    let TransitionTargetNode::Value(value) =
        program.statement_table.transition_target(transition.target)
    else {
        return false;
    };
    let mut calls = Vec::new();
    collect_self_entry_call_arguments(program, entry_name, *value, &mut calls);
    calls
        .iter()
        .any(|arguments| arguments.get(measure_position) == Some(&argument))
}

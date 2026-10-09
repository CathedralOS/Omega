use crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup;

use super::{
    ExpressionHandle, ExpressionNode, Machine, StatementNode, TransitionGuardNode,
    TransitionTargetNode,
};
pub(super) fn guarded_slice_tail_call<'a>(
    program: &'a dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    call_ordinal: usize,
    statement: &StatementNode,
    argument: ExpressionHandle,
    measure_position: usize,
    bound_lookup: &mut Option<ImmutableBoundLookup<'a>>,
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
    let Some(subjects) = typed_trees::ranking::resolve_machine_witness_subjects(program, machine)
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
    let Some(transition) = statement.as_transition() else {
        return false;
    };
    let TransitionGuardNode::When(guard) = transition.guard else {
        return false;
    };
    if !crate::slice_tail_strictly_decreases_with_bound_lookup(
        program,
        guard,
        argument,
        parameter,
        bound_lookup,
    ) {
        return false;
    }
    // The exact taken occurrence owns this guard, including when expression
    // handles are shared by the guard or another arm.
    crate::transition_call_target(program, machine, state, statement_index, call_ordinal)
        == Some(transition.target)
        && matches!(
            program.statement_table.transition_target(transition.target),
            TransitionTargetNode::Value(_)
        )
}

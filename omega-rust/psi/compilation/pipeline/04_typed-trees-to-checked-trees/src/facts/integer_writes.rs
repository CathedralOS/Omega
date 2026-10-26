//! Preserve intermediate writes independently from return and mutation summaries.
//!
//! The existing assignment-frame inventory owns destinations, scalar lowering
//! owns RHS algebra, and flow owns pre-store facts. Joining those occurrences
//! here lets call admission ask about every value written, including a value
//! overwritten before return. This is body evidence, not a published precondition
//! or a claim that a transitive call's writes have already been admitted.
use checked_trees::{
    CheckedOperatorFacts, CheckedValueFacts, CheckedValueOrigin, CheckedValueStatementRole,
    FlowFacts, MutationFacts, PerformedIntegerWrite,
};

pub(super) mod obligations;

/// Join independently checked receiving constraints at one authored write.
/// A declared integer bound and an interval-only data domain can both constrain
/// the same field. Neither supersedes the other, and duplicate rows must not
/// turn a completed occurrence into an ambiguous decision handle.
pub(super) fn retain_assignment_decision(
    program: &dyn ::validation::CheckingInput,
    proof: &mut checked_trees::ProofFacts,
    decision: checked_trees::CheckedIntegerAssignmentDecision,
) {
    let existing = proof
        .integer_assignments
        .iter()
        .find_map(|(handle, retained)| {
            (retained.atomic_resident == decision.atomic_resident
                && retained.machine == decision.machine
                && retained.state == decision.state
                && retained.statement_index == decision.statement_index
                && retained.target == decision.target
                && retained.value == decision.value
                && program.primitive_type_reference(retained.target_type)
                    == program.primitive_type_reference(decision.target_type))
            .then_some(handle)
        });
    if let Some(handle) = existing {
        let retained = proof.integer_assignments.get_mut(handle);
        retained.minimum = retained.minimum.clone().max(decision.minimum);
        retained.maximum = retained.maximum.clone().min(decision.maximum);
    } else {
        proof.integer_assignments.append(decision);
    }
}

pub(super) fn retain(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    values: &CheckedValueFacts,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    frames: Option<&validation::CallFrameResolver<'_>>,
    mutation: &mut MutationFacts,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let flow_state = flow.control.state(machine.symbol, state.symbol);
            for (statement_index, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                let Some(assignment) = statement.as_assignment() else {
                    continue;
                };
                let primitive = validation::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    assignment.target,
                )
                .map(|reference| validation::assignment_value_type(program, reference))
                .and_then(|reference| program.primitive_type_reference(reference));
                let Some(primitive) =
                    primitive.filter(|primitive| primitive.accepts_integer_literal())
                else {
                    continue;
                };
                let origin = CheckedValueOrigin::StateStatement {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    statement_index,
                    role: CheckedValueStatementRole::AssignmentValue,
                };
                let mut ranges = values
                    .expression_values(assignment.value)
                    .map(|(_, value)| value)
                    .filter(|value| value.origin == origin);
                let integer_range = ranges.next().and_then(|value| {
                    ranges
                        .next()
                        .is_none()
                        .then(|| value.integer_range.clone())
                        .flatten()
                });
                let contexts = flow_state
                    .and_then(|state| {
                        flow.control
                            .statements
                            .span_or_empty(state.statements)
                            .iter()
                            .find(|fact| fact.statement_index == statement_index)
                    })
                    .map(|statement| {
                        flow.semantic_constraint_contexts(statement.pre_store_constraints)
                            .collect::<Vec<_>>()
                    });
                let entry_condition = contexts.as_ref().and_then(|contexts| {
                    super::boolean_predicates::entry_path_condition(
                        semantic,
                        contexts,
                        machine.symbol,
                    )
                });
                let operand_writes =
                    frames.map(|frames| frames.statement_value_write_frame(machine, statement));
                let entry_value = crate::values::lower_entry_scalar(
                    program,
                    operators,
                    machine,
                    state,
                    statement_index,
                    assignment.value,
                    primitive,
                    &|place| {
                        let paths = operand_writes.as_ref()?.complete_paths()?;
                        if paths.iter().any(|written| {
                            validation::frame_paths_overlap(written, &place.label(program))
                        }) {
                            return None;
                        }
                        super::entry_parameter_place_before_statement(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            place,
                        )
                    },
                );
                mutation.integer_writes.insert(PerformedIntegerWrite {
                    machine: machine.symbol,
                    state: state.symbol,
                    statement_index,
                    destination: assignment.target,
                    value: assignment.value,
                    primitive_type: Some(primitive),
                    frame: frames.map_or_else(facts::NormalizedWriteFrame::opaque, |frames| {
                        frames.assignment_write_frame(machine, statement)
                    }),
                    integer_range,
                    entry_value,
                    entry_condition,
                });
            }
        }
    }
}

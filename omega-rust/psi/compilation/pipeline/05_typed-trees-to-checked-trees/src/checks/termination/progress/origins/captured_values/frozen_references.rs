//! A completed copy of an entry descriptor freezes its invocation referent,
//! independently of later rebinding of the slot that originally carried it.
//! The original referent's contents remain live and independently protected.

use crate::flow;
use checked_trees::{FlowCallFact, FlowFacts, FlowOperandFact, FlowStateFact};
use language_semantics::ProgressSubject;
use typed_trees::machine::Machine;
use validation::CanonicalPlace;

#[allow(clippy::too_many_arguments)]
pub(super) fn entry_subject(
    program: &dyn validation::CheckingInput,
    flow: &FlowFacts,
    machine: &Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
    capture: &FlowOperandFact,
    source: &CanonicalPlace,
    frames: &validation::CallFrameResolver<'_>,
) -> Option<ProgressSubject> {
    let entry = program.machine_states(machine).first()?;
    if capture.state_symbol != entry.symbol || state.state_symbol != entry.symbol {
        return None;
    }
    let parameter = program.state_parameter_by_symbol(entry, source.root.as_symbol()?)?;
    validation::reference_shell(program, parameter.type_reference)?;
    let descriptor = CanonicalPlace {
        root: source.root,
        segments: Vec::new(),
    };
    // Absence is meaningful only for this authenticated invocation formal.
    // A stored replacement, unavailable capture or recovered later binding
    // cannot stand in for the original descriptor observed by the copy.
    if validation::completed_storage::at_snapshot(
        program,
        &flow.control,
        capture.storage_snapshot,
        &descriptor,
        validation::completed_storage::StoredObservation::Binding,
    )
    .is_some()
    {
        return None;
    }
    let statements = program.statement_table.statements(entry.statement_nodes);
    let order = program
        .statement_table
        .admitted_execution_order(entry.statement_nodes);
    let end = order.position(call.statement_index)?;
    for index in &order.indices()[..end] {
        let statement = statements.get(*index)?;
        // Binding replacement does not write the old referent. Omit this
        // whole-slot event only when its RHS has no writes of its own; a
        // merged footprint may otherwise also contain a real pointee write.
        if let Some(assignment) = statement.as_assignment()
            && frames.assignment_replaces_reference_binding(machine, statement) == Some(true)
            && validation::single_segment_name_symbol(program, assignment.target)
                == Some(parameter.symbol)
            && frames
                .expression_write_frame(machine, assignment.value)
                .complete_paths()
                .is_some_and(<[_]>::is_empty)
        {
            continue;
        }
        let statement = flow.state_statement(state, *index)?;
        if !flow::entry_origin_is_stable(
            program,
            &flow.control,
            statement.completed_content_writes,
            statement.completed_content_unknown_write,
            source,
        ) {
            return None;
        }
    }
    let (operands, _) = flow::captured_call_operands(program, flow, state, call)?;
    if !flow::operand_entry_origin_is_stable(program, flow, operands.last()?, source) {
        return None;
    }
    // All load evidence came from the completed source resolution. This
    // recovers only its original entry subject, never a current slot read.
    super::super::super::fact_subjects::subject_from_place(source.root, &source.segments)
}

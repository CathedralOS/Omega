//! Exact predecessors of a completed value, independent of its consumer.
//!
//! A stored copy links to the value observed before that read. Declaration
//! initializers and later storage observations cannot substitute for that link.
//! Alternatives are all predecessors: discovery may visit them independently,
//! but a transported judgment must hold for every selected possibility.

use arena::Handle;
use checked_trees::{CheckFacts, FlowOperandFact};
use facts::PlaceSegment;

pub(crate) fn predecessors(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    value: Handle<FlowOperandFact>,
    projection: &[PlaceSegment],
) -> Option<Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>> {
    if !facts.flow.control.operands.is_valid(value) {
        return None;
    }
    let capture = facts.flow.control.operands.get(value);
    if !capture.alternatives.is_empty() {
        return Some(
            capture
                .alternatives
                .handles()
                .map(|alternative| (alternative, projection.to_vec()))
                .collect(),
        );
    }
    if !projection.is_empty()
        && let Some(selected) = super::captured_operand_handle_at_path(
            program,
            &facts.flow.control,
            value,
            capture.type_reference,
            projection,
        )
        && selected != value
    {
        return Some(vec![(selected, Vec::new())]);
    }
    let state = program.state_by_symbol(capture.state_symbol)?;
    let state_flow = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, flow)| (flow.state_symbol == capture.state_symbol).then_some(flow))?;
    let mut source = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        capture.statement_index,
        capture.expression,
    )?;
    source.segments.extend_from_slice(projection);
    let super::stored_values::StoredOperand::Value {
        capture: stored,
        reference,
        projection: stored_projection,
    } = super::stored_values::before_statement(
        program,
        &facts.semantic,
        &facts.flow.control,
        state,
        state_flow.statements,
        capture.statement_index,
        capture.entry_origin_writes,
        capture.entry_origin_unknown_write,
        &source,
        super::stored_values::StoredObservation::Contents,
    )?
    else {
        return None;
    };
    reference
        .is_valid()
        .then_some(vec![(stored, stored_projection)])
}

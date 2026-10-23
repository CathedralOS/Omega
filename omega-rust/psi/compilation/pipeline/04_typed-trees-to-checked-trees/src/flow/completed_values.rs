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
    predecessors_in_flow(
        program,
        &facts.flow,
        &facts.semantic,
        value,
        projection,
        super::stored_values::StoredObservation::Contents,
    )
}

/// The completed-producer relation is shared by checking passes before a full
/// CheckFacts wrapper exists. Binding observations freeze descriptors; contents
/// observations retain the existing detached-value rule.
pub(crate) fn predecessors_in_flow(
    program: &dyn validation::CheckingInput,
    flow: &checked_trees::FlowFacts,
    semantic: &facts::FactPlan,
    value: Handle<FlowOperandFact>,
    projection: &[PlaceSegment],
    observation: super::stored_values::StoredObservation,
) -> Option<Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>> {
    if !flow.control.operands.is_valid(value) {
        return None;
    }
    let capture = flow.control.operands.get(value);
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
        && let Some((selected, _, consumed)) = super::operand_values::captured_operand_child_at_path(
            program,
            &flow.control,
            capture,
            capture.type_reference,
            projection,
        )
        && selected != value
    {
        return Some(vec![(selected, projection[consumed..].to_vec())]);
    }
    let state = program.state_by_symbol(capture.state_symbol)?;
    let state_flow = flow
        .control
        .states
        .iter()
        .find_map(|(_, flow)| (flow.state_symbol == capture.state_symbol).then_some(flow))?;
    if let Some(member) = program.expression_table.member_node(capture.expression) {
        let statement = flow.state_statement(state_flow, capture.statement_index)?;
        let mut observations = flow
            .control
            .value_observations
            .span_or_empty(statement.value_observations)
            .iter()
            .filter(|observation| {
                observation.expression == capture.expression
                    && observation.constraints == capture.constraints
            });
        if let Some(observation) = observations.next() {
            if observations.next().is_some()
                || !flow.control.operands.is_valid(observation.receiver)
            {
                return None;
            }
            let receiver = flow.control.operands.get(observation.receiver);
            // A member of an owned completed value selects that receiver's
            // captured subtree, even when no source variable names it. A
            // reference instead observes live storage and must keep the
            // write-sensitive reaching-store route below.
            if validation::reference_shell(program, receiver.type_reference).is_none() {
                let symbol = validation::effective_member_symbol(program, member.receiver, member);
                if !symbol.is_valid() || program.any_field_owner(symbol).is_none() {
                    return None;
                }
                let mut path = Vec::new();
                if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
                    path.push(PlaceSegment::Case { variant });
                }
                path.push(PlaceSegment::Field { symbol });
                path.extend_from_slice(projection);
                return Some(vec![(observation.receiver, path)]);
            }
        }
    }
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
        semantic,
        &flow.control,
        state,
        state_flow.statements,
        capture.statement_index,
        capture.entry_origin_writes,
        capture.entry_origin_unknown_write,
        &source,
        observation,
    )?
    else {
        return None;
    };
    reference
        .is_valid()
        .then_some(vec![(stored, stored_projection)])
}

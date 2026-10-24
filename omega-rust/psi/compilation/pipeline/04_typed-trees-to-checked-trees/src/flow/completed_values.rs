//! Exact predecessors of a completed value, independent of its consumer.
//!
//! A stored copy links to the value observed before that read. Declaration
//! initializers and later storage observations cannot substitute for that link.
//! Alternatives are all predecessors: discovery may visit them independently,
//! but a transported judgment must hold for every selected possibility.

use arena::Handle;
use checked_trees::{CheckFacts, FlowOperandFact};
use facts::PlaceSegment;

/// The actual completions before a value join, rather than its intersected
/// post-join custody. This follows only retained alternatives: stored-value
/// predecessors are earlier observations and do not own this completion.
pub(crate) fn terminal_alternatives(
    control: &checked_trees::FlowControlFacts,
    value: Handle<FlowOperandFact>,
) -> Option<Vec<Handle<FlowOperandFact>>> {
    let mut pending = vec![value];
    let mut visited = Vec::new();
    let mut completed = Vec::new();
    while let Some(value) = pending.pop() {
        if !control.operands.is_valid(value) || visited.contains(&value) {
            return None;
        }
        visited.push(value);
        let capture = control.operands.get(value);
        if let checked_trees::FlowOperandCompletion::Selected(alternatives) = capture.completion {
            let rows = control.operand_alternatives.span(alternatives)?;
            if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid()) { return None; }
            pending.extend(rows.iter().map(|row| row.value));
        } else {
            completed.push(value);
        }
    }
    Some(completed)
}

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
    _semantic: &facts::FactPlan,
    value: Handle<FlowOperandFact>,
    projection: &[PlaceSegment],
    observation: super::stored_values::StoredObservation,
) -> Option<Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>> {
    if !flow.control.operands.is_valid(value) {
        return None;
    }
    let capture = flow.control.operands.get(value);
    // A value source preserves inline bytes, including a pointer descriptor.
    // It cannot freeze contents behind that pointer at its earlier epoch.
    let crosses_reference = (0..=projection.len()).any(|length| {
        validation::project_type_reference_from_segments(
            program,
            capture.type_reference,
            &projection[..length],
        )
        .is_some_and(|reference| validation::reference_shell(program, reference).is_some())
    });
    if capture.value_source.is_valid()
        && (matches!(
            observation,
            super::stored_values::StoredObservation::Binding
        ) || !crosses_reference)
    {
        return flow
            .control
            .operands
            .is_valid(capture.value_source)
            .then(|| vec![(capture.value_source, projection.to_vec())]);
    }
    if let checked_trees::FlowOperandCompletion::Copied { copy, path } = capture.completion
        && (matches!(observation, super::stored_values::StoredObservation::Binding) || !crosses_reference)
    {
        if !flow.control.copied_values.is_valid(copy) { return None; }
        let copied = flow.control.copied_values.get(copy);
        let mut selected = flow.control.operand_child_paths.span(path)?.to_vec();
        selected.extend_from_slice(projection);
        return Some(vec![(copied.source, selected)]);
    }
    if let checked_trees::FlowOperandCompletion::Selected(alternatives) = capture.completion {
        let rows = flow.control.operand_alternatives.span(alternatives)?;
        if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid()) { return None; }
        return Some(rows.iter().map(|row| (row.value, projection.to_vec())).collect());
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
    let captures = super::stored_values::at_snapshot(
        program,
        &flow.control,
        capture.storage_snapshot,
        &source,
        observation,
    )?
    .captures()?;
    let mut predecessors = Vec::new();
    for selected in captures {
        if !selected.reference.is_valid() {
            return None;
        }
        predecessors.push((selected.capture, selected.projection));
    }
    Some(predecessors)
}

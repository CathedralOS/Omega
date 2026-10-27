//! Cleanup contracts consume the same captured predicate judgment.

use super::{
    BoundObservation, BoundPredicate, CheckFacts, FlowStateFact, PredicateTransport,
    retain_bindings, transport,
};

/// Rejoin a constructed value's scalar observation with its typed completion. The
/// active destination token owns availability; its original place/point names
/// one actual child capture even after an outer projection kept only a loan.
/// Neither source syntax nor an initializer can replace a missing capture.
pub(crate) fn cleanup_predicates(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    contexts: &[facts::FactContextHandle],
    required: &mut BoundPredicate,
    premises: &[BoundPredicate],
    mut stable: impl FnMut(&BoundObservation) -> bool,
) -> PredicateTransport {
    let mut captures = Vec::new();
    for (position, binding) in required.bindings.iter().enumerate() {
        if !stable(binding) {
            continue;
        }
        let mut origin = None;
        let mut ambiguous = false;
        let mut origin_depth = 0;
        for fact in contexts.iter().flat_map(|context| {
            facts
                .semantic
                .context_view(facts.semantic.contexts.get(*context))
                .facts()
        }) {
            let facts::FactPayload::AssignedValueCapture { source, point } = fact.payload else {
                continue;
            };
            let Some(destination) = fact.place.as_place().and_then(|place| {
                crate::flow::canonical_place_from_semantic_place(
                    program,
                    &facts.semantic,
                    facts.semantic.places.get(place),
                )
            }) else {
                continue;
            };
            if destination.root != binding.place.root {
                continue;
            }
            let Some(relative) = binding
                .place
                .segments
                .strip_prefix(destination.segments.as_slice())
            else {
                continue;
            };
            let depth = destination.segments.len();
            if origin.is_some() && depth < origin_depth {
                continue;
            }
            if depth > origin_depth {
                origin = None;
                ambiguous = false;
            }
            origin_depth = depth;
            let candidate = (source, point, relative.to_vec());
            if origin
                .as_ref()
                .is_some_and(|previous| *previous != candidate)
            {
                ambiguous = true;
            }
            origin = Some(candidate);
        }
        let Some((
            source,
            facts::ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            relative,
        )) = origin
        else {
            continue;
        };
        if ambiguous || machine_symbol != state.machine_symbol {
            continue;
        }
        let source = facts.semantic.places.get(source);
        let Some(parent) = source.root.as_expression() else {
            continue;
        };
        let path = facts.semantic.place_segments.span_or_empty(source.segments);
        let mut matching = facts
            .flow
            .control
            .operand_children
            .iter()
            .filter_map(|(_, child)| {
                let operand =
                    crate::flow::CapturedOperand::recorded(&facts.flow.control, child.value)?;
                (child.parent == parent
                    && operand.state_symbol == state_symbol
                    && operand.statement_index == statement_index
                    && facts
                        .flow
                        .control
                        .operand_child_paths
                        .span_or_empty(child.path)
                        == path)
                    .then_some((child, operand))
            });
        let Some((child, operand)) = matching.next() else {
            continue;
        };
        let selected = validation::project_type_reference_from_segments(
            program,
            child.type_reference,
            &relative,
        );
        // Extending a non-tail child span copies its reference rows. Those
        // aliases still name one completed operand, not another evaluation.
        // Distinct operand handles remain ambiguous even when their expressions
        // or facts look equal; the declared type is part of the capture identity.
        if matching.any(|(other, _)| {
            other.value != child.value || other.type_reference != child.type_reference
        }) || selected.and_then(|reference| program.primitive_type_reference(reference))
            != binding.operand.primitive_type()
        {
            continue;
        }
        let Some(mut values) = crate::flow::captured_operand_values(
            program,
            &facts.flow,
            &facts.semantic,
            state,
            operand,
            child.type_reference,
            &relative,
        ) else {
            continue;
        };
        if values.len() != 1 {
            continue;
        }
        let captured = values.pop().expect("one completed scalar");
        let Some(observed) =
            crate::flow::captured_observation_places(program, &facts.flow, &captured)
        else {
            continue;
        };
        let [observed] = observed.as_slice() else {
            continue;
        };
        {
            let Some((machine, body)) =
                validation::find_state(program, captured.observation.state_symbol)
            else {
                continue;
            };
            if !validation::value_has_builtin_coordinates(
                program,
                machine,
                body,
                captured.observation_projection.expression,
            ) {
                continue;
            }
        }
        let mut copied = binding.clone();
        copied.place = observed.projected.clone();
        retain_bindings(
            program,
            facts,
            &captured,
            operand.row(),
            &[copied],
            position,
            &mut captures,
        );
    }
    if captures.is_empty() {
        return PredicateTransport::default();
    }
    let frames = validation::CallFrameResolver::new(program);
    transport(
        program,
        facts,
        state,
        required,
        &captures,
        premises,
        frames.as_ref(),
    )
}

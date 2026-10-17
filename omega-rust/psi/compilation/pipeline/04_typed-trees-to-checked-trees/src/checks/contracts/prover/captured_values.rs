//! Scalar observations of completed values share one copy/view law.
//! Normal-result proofs project the retained operand tree, not source
//! initializers. Copies keep their completion facts; references observe
//! captured backing at the consumer's live frontier. Every possible captured
//! value and referent must establish the same scalar independently.

use checked_trees::{CheckFacts, FlowConstraintRef, FlowOperandFact, FlowStateFact};

/// `consumer_statement` is the first statement frontier after this completed
/// consumption, including its nested calls. Its constraints exclude the goal.
#[allow(clippy::too_many_arguments)]
pub(in crate::checks::contracts) fn scalar_at_path(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    capture: &FlowOperandFact,
    path: &[facts::PlaceSegment],
    consumer_statement: usize,
    invocation: arena::HandleSpan<FlowConstraintRef>,
    frames: &validation::CallFrameResolver<'_>,
) -> Option<facts::ScalarValue> {
    let values = crate::flow::captured_operand_values(
        program,
        &facts.flow,
        &facts.semantic,
        state,
        capture,
        capture.type_reference,
        path,
    )?;
    let mut agreed = None;
    for value in values {
        // Later guards belong to the observed copy, not its reaching producer.
        // Try those namespaces independently; never union their premise sets.
        let scalar = [
            (
                value.observation,
                value.observation_type,
                crate::flow::captured_observation_places(program, &facts.flow, &value),
            ),
            (
                value.operand,
                value.type_reference,
                crate::flow::captured_value_places(program, &facts.flow, &value),
            ),
        ]
        .into_iter()
        .find_map(|(operand, reference, places)| {
            let contexts = crate::flow::operand_contexts(
                program,
                &facts.flow,
                &facts.semantic,
                operand,
                reference,
                invocation,
            );
            let statement = if crate::flow::operand_binds_stable_copy(program, reference) {
                operand.statement_index.checked_add(1)?
            } else {
                consumer_statement
            };
            let predicates = crate::facts::boolean_predicates::retained(
                program,
                &facts.operators,
                &facts.semantic,
                &contexts,
            );
            let mut candidate = None;
            for place in places? {
                let scalar = crate::values::scalar_value_at_place(
                    program,
                    &facts.semantic,
                    contexts
                        .iter()
                        .map(|context| facts.semantic.contexts.get(*context)),
                    &place.projected,
                )
                .or_else(|| {
                    super::boolean_value_at_place(
                        program,
                        &facts.operators,
                        &facts.semantic,
                        &contexts,
                        &place.projected,
                    )
                    .map(facts::ScalarValue::Boolean)
                })
                .or_else(|| {
                    predicates.iter().find_map(|predicate| {
                        predicate.constant_at_place(&place.projected, |operand| {
                            crate::values::scalar_value_at_place(
                                program,
                                &facts.semantic,
                                contexts
                                    .iter()
                                    .map(|context| facts.semantic.contexts.get(*context)),
                                operand,
                            )
                        })
                    })
                })
                .or_else(|| {
                    super::call_guarantees::scalar_at_place(
                        program,
                        facts,
                        state,
                        statement,
                        &contexts,
                        frames,
                        &place.projected,
                    )
                })?;
                if candidate
                    .as_ref()
                    .is_some_and(|previous| *previous != scalar)
                {
                    return None;
                }
                candidate = Some(scalar);
            }
            candidate
        })?;
        if agreed.as_ref().is_some_and(|previous| *previous != scalar) {
            return None;
        }
        agreed = Some(scalar);
    }
    agreed
}

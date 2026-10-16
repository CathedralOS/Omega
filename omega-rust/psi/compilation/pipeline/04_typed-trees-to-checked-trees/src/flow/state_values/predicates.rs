//! Relational state inputs come from live completed edges. Scratch rows retain
//! canonical places across sweeps; only reseeding allocates current fact handles.
use validation::CanonicalPlace;

use crate::facts::boolean_predicates::{self, BoundObservation, BoundPredicate};
use crate::flow::FlowBuildContext;
use arena::HandleSpan;
use checked_trees::FlowSemanticContextRef;
use facts::{FactOrigin, FactPlan, ProgramPoint, ScalarValue};
use typed_trees::{expression::ExpressionHandle, state::State};

pub(super) fn meet(previous: &mut Vec<BoundPredicate>, incoming: &[BoundPredicate]) -> bool {
    let before = previous.len();
    previous.retain(|predicate| incoming.contains(predicate));
    previous.len() != before
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    state: &State,
    destination: &State,
    statement_index: usize,
    arguments: &[ExpressionHandle],
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> Vec<BoundPredicate> {
    let contexts = crate::flow::active_context_handles(build, contexts);
    let mut transfers = Vec::new();
    let mut arguments = arguments.iter();
    for parameter in program.state_parameters(destination) {
        let source = {
            arguments.next().and_then(|argument| {
                build.canonical_place_at(program, state.symbol, statement_index, *argument)
            })
        };
        if let Some(source) = source {
            transfers.push((source, parameter.symbol));
        }
    }
    let rebase = |binding: &BoundObservation| {
        let source = &binding.place;
        let mut candidate = None;
        for (argument, parameter) in &transfers {
            if argument.root != source.root || !source.segments.starts_with(&argument.segments) {
                continue;
            }
            let place = CanonicalPlace {
                root: facts::PlaceRoot::Symbol(*parameter),
                segments: source.segments[argument.segments.len()..].to_vec(),
            };
            let reference =
                validation::canonical_place_type_reference(program, destination.symbol, 0, &place)?;
            if program.normalized_type_identity(reference)
                != program.normalized_type_identity(binding.type_reference)
            {
                continue;
            }
            // Multiple equivalent copies are not a single destination identity.
            // Keep the first exact copy; later writes invalidate that copy's
            // relation independently, never a differently bound sibling.
            if candidate.is_none() {
                candidate = Some(place);
            }
        }
        candidate
    };
    let mut result = Vec::new();
    for predicate in boolean_predicates::retained(program, build.operators, semantic, &contexts) {
        // Forwarded observations keep their relational meaning. Only an input
        // that disappears may fold from the exact outgoing edge's live facts.
        let Some(mut predicate) = boolean_predicates::fold(&predicate, |place| {
            let binding = predicate
                .bindings
                .iter()
                .find(|binding| binding.place == *place)?;
            if rebase(binding).is_some() {
                return None;
            }
            crate::checks::contracts::boolean_value_at_place(
                program,
                build.operators,
                semantic,
                &contexts,
                place,
            )
            .map(ScalarValue::Boolean)
            .or_else(|| {
                crate::values::scalar_value_at_place(
                    program,
                    semantic,
                    contexts
                        .iter()
                        .map(|context| semantic.contexts.get(*context)),
                    place,
                )
            })
        }) else {
            continue;
        };
        let mut complete = true;
        for binding in &mut predicate.bindings {
            let Some(place) = rebase(binding) else {
                complete = false;
                break;
            };
            binding.place = place;
        }
        if complete
            && !matches!(
                predicate.expression,
                checked_trees::CheckedBooleanExpression::Constant(true)
            )
            && !result.contains(&predicate)
        {
            result.push(predicate);
        }
    }
    result
}

pub(super) fn append(semantic: &mut FactPlan, predicates: &[BoundPredicate], point: ProgramPoint) {
    for predicate in predicates {
        boolean_predicates::seed(semantic, predicate, point, FactOrigin::StatementTransfer);
    }
}

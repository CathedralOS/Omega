//! Projection paths entry rank observation must retain for one measure.

use crate::checks::current_scalar_values::parameter_projection_paths;
use typed_trees::{expression::ExpressionHandle, machine::Machine, state::State};
use validation::{
    CheckingInput, RankingRangeMeasure, canonical_place_from_expression_in_state,
    expression_traversal::children,
};

pub(in crate::checks::termination) fn entry_observation_paths(
    program: &dyn CheckingInput,
    machine: &Machine,
    state: &State,
    measure: RankingRangeMeasure,
    extra_expressions: &[ExpressionHandle],
) -> Vec<Vec<facts::PlaceSegment>> {
    let (subject, paired) = match measure {
        RankingRangeMeasure::Single(subject) | RankingRangeMeasure::Computed { subject, .. } => {
            (subject, None)
        }
        RankingRangeMeasure::Distance { lower, upper } => (lower, Some(upper)),
        RankingRangeMeasure::IncreasingTo { subject, limit } => (subject, Some(limit)),
        RankingRangeMeasure::Field { .. } | RankingRangeMeasure::SliceLength(_) => {
            return parameter_projection_paths(program, machine, state);
        }
    };
    let mut expressions = vec![subject];
    expressions.extend(paired);
    expressions.extend_from_slice(extra_expressions);
    let record = match measure {
        RankingRangeMeasure::Computed {
            subject,
            parameter,
            body,
            ..
        } => {
            expressions.push(body);
            let formal = program.measures().iter().find_map(|declaration| {
                declaration
                    .parameter
                    .as_ref()
                    .filter(|formal| formal.symbol == parameter)
            });
            if formal.is_none_or(|formal| {
                program
                    .primitive_type_reference(formal.type_reference)
                    .is_some()
            }) {
                None
            } else {
                super::declaration_inputs::RecordInput::prepare(
                    program, machine, state, subject, parameter, body,
                )
            }
        }
        _ => None,
    };
    let mut paths = parameter_projection_paths(program, machine, state);
    let mut pending = expressions;
    while let Some(expression) = pending.pop() {
        if let Some(place) =
            canonical_place_from_expression_in_state(program, state.symbol, 0, expression)
            && place
                .root
                .as_symbol()
                .is_some_and(|symbol| program.state_parameter_by_symbol(state, symbol).is_some())
            && !place.segments.is_empty()
            && !paths.contains(&place.segments)
        {
            paths.push(place.segments);
        }
        children(
            program,
            program.expression_table.expression(expression),
            |child| pending.push(child),
        );
    }
    if let Some(record) = record {
        for path in record.paths() {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    paths
}

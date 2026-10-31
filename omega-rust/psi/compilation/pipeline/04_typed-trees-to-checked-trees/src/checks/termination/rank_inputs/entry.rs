//! Prepare the integer entry judgment from the same typed coordinate capture
//! used by inferred participants. Witness and invariant selection stay outside.

use super::EntryInputs;
use typed_trees::{expression::ExpressionHandle, machine::Machine};
use validation::{RankingRangeMeasure, ScopedArithmeticValue};

pub(in crate::checks::termination) fn prove(
    program: &dyn validation::CheckingInput,
    machine: &Machine,
    range: ExpressionHandle,
    measure: RankingRangeMeasure,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<bool> {
    if validation::proof_only_classification(program).is_proof_machine(program, machine) {
        return None;
    }
    let (subject, paired) = match measure {
        RankingRangeMeasure::Single(subject) | RankingRangeMeasure::Computed { subject, .. } => {
            (subject, None)
        }
        RankingRangeMeasure::Distance { lower, upper } => (lower, Some(upper)),
        RankingRangeMeasure::IncreasingTo { subject, limit } => (subject, Some(limit)),
        // Nominal record and collection views retain their genuine source
        // projection owners; no failed integer capture retries another proof.
        RankingRangeMeasure::Field { .. } | RankingRangeMeasure::SliceLength(_) => return None,
    };
    Some(
        (|| {
            let root = program.machine_states(machine).first()?;
            let authored_range = if range.is_valid() {
                Some(program.expression_table.range_node(range)?)
            } else {
                None
            };
            let mut expressions = vec![subject];
            expressions.extend(paired);
            if let Some(range) = authored_range {
                expressions.extend([range.start, range.end]);
            }
            let mut paths = crate::checks::current_scalar_values::parameter_projection_paths(
                program, machine, root,
            );
            let mut pending = expressions;
            while let Some(expression) = pending.pop() {
                if let Some(place) = validation::canonical_place_from_expression_in_state(
                    program,
                    root.symbol,
                    0,
                    expression,
                ) && place
                    .root
                    .as_symbol()
                    .is_some_and(|symbol| program.state_parameter_by_symbol(root, symbol).is_some())
                    && !place.segments.is_empty()
                    && !paths.contains(&place.segments)
                {
                    paths.push(place.segments);
                }
                validation::expression_traversal::children(
                    program,
                    program.expression_table.expression(expression),
                    |child| pending.push(child),
                );
            }
            let prepared = super::at_entry(program, machine, root, frames, &paths)?;
            let subject = prepared.expression(subject)?;
            let paired_subject = match paired {
                Some(value) => Some(prepared.expression(value)?),
                None => None,
            };
            let (range_start, range_end) = match authored_range {
                Some(range) => (
                    Some(prepared.expression(range.start)?),
                    Some(prepared.expression(range.end)?),
                ),
                None => (None, None),
            };
            let inputs = prepared
                .coordinates
                .iter()
                .filter_map(|coordinate| {
                    let (minimum, maximum) = coordinate.primitive.integer_bounds()?;
                    Some(ScopedArithmeticValue::BoundedInteger {
                        value: Box::new(coordinate.entry_value.clone()),
                        minimum: Some(numerics::bignum::BigInt::from_i128(minimum)),
                        maximum: Some(numerics::bignum::BigInt::from_i128(maximum)),
                    })
                })
                .collect();
            Some(validation::prove_captured_ranking_range_entry(
                program,
                machine,
                range,
                measure,
                &validation::CapturedRankEntry {
                    subject,
                    paired_subject,
                    range_start,
                    range_end,
                    hypotheses: prepared.values.captured_hypotheses().to_vec(),
                    inputs,
                },
            ))
        })()
        .unwrap_or(false),
    )
}

impl EntryInputs<'_> {
    /// The value roster is useful only after every runtime projection in it
    /// has independently admitted geometry and entry selector bounds.
    fn expression(
        &self,
        expression: ExpressionHandle,
    ) -> Option<validation::ScopedArithmeticExpression> {
        let scoped = self.values.current(expression)?;
        let mut pending = vec![&scoped];
        while let Some(term) = pending.pop() {
            for binding in &term.bindings {
                if let validation::ScopedArithmeticBinder::Occurrence(expression) = binding.binder {
                    let program = self.values.program();
                    let place_expression = validation::collection_length_receiver(
                        program,
                        self.values.machine(),
                        Some(self.values.state()),
                        expression,
                    )
                    .unwrap_or(expression);
                    if let Some(place) = validation::canonical_place_from_expression_in_state(
                        program,
                        self.values.state().symbol,
                        0,
                        place_expression,
                    ) && let Some(parameter) = place.root.as_symbol()
                        && program
                            .state_parameter_by_symbol(self.values.state(), parameter)
                            .is_some()
                    {
                        let observation = if place_expression != expression {
                            validation::RankPremiseObservation::Length
                        } else {
                            validation::RankPremiseObservation::Value
                        };
                        if !self.coordinates.iter().any(|coordinate| {
                            coordinate.parameter == parameter
                                && coordinate.observation == observation
                                && self
                                    .values
                                    .same_scalar_projection(&coordinate.path, &place.segments)
                        }) {
                            return None;
                        }
                    }
                }
                match &binding.value {
                    ScopedArithmeticValue::Term(term)
                    | ScopedArithmeticValue::CollectionLength(term)
                    | ScopedArithmeticValue::AdmittedIntegerRemainder(term) => pending.push(term),
                    _ => {}
                }
            }
        }
        Some(scoped)
    }
}

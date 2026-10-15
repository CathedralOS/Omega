//! Formula identity joins exact observations, not lowering proof coverage.
//! A guard's decided subtraction and a requirement's undecided subtraction
//! denote the same operation on the same values. Normalize cloned scratch
//! formulas only, clearing decisions rather than granting them: published
//! operations, formation obligations, policies and captured epochs stay intact.

use super::{Boolean, BoundPredicate, Observation, Scalar, map_observations};

pub(super) fn normalize(predicates: &[BoundPredicate]) -> Option<(Vec<Boolean>, Vec<Observation>)> {
    let mut observations = Vec::new();
    let expressions = predicates
        .iter()
        .map(|predicate| {
            let mut expression = predicate.expression.clone();
            map_observations(&mut expression, &mut |operand| {
                let binding = predicate.binding(operand)?;
                let primitive_type = operand.primitive_type()?;
                let key = (binding.place.clone(), primitive_type);
                let position = observations
                    .iter()
                    .position(|known| known == &key)
                    .unwrap_or_else(|| {
                        let position = observations.len();
                        observations.push(key);
                        position
                    });
                *operand = Scalar::Parameter {
                    position,
                    primitive_type,
                };
                Some(())
            })?;
            clear_boolean_formation_decisions(&mut expression);
            Some(expression)
        })
        .collect::<Option<Vec<_>>>()?;
    Some((expressions, observations))
}

fn clear_boolean_formation_decisions(expression: &mut Boolean) {
    match expression {
        Boolean::Not(operand) => clear_boolean_formation_decisions(operand),
        facts::checked_boolean_triple_pattern!(left, right, ..) => {
            clear_boolean_formation_decisions(left);
            clear_boolean_formation_decisions(right);
        }
        Boolean::IntegerComparison { left, right, .. }
        | Boolean::ScalarIeeeFloatComparison { left, right, .. } => {
            clear_scalar_formation_decisions(left);
            clear_scalar_formation_decisions(right);
        }
        Boolean::IntegerMathComparison { left, right, .. } => {
            for expression in [left, right] {
                let handles = expression
                    .terms
                    .iter()
                    .map(|(handle, _)| handle)
                    .collect::<Vec<_>>();
                for handle in handles {
                    if let checked_trees::CheckedIntegerMathTerm::Embed { operand, .. } =
                        expression.terms.get_mut(handle)
                    {
                        clear_scalar_formation_decisions(operand);
                    }
                }
            }
        }
        // Other admitted forms contain no selected scalar computations.
        _ => {}
    }
}

fn clear_scalar_formation_decisions(expression: &mut Scalar) {
    match expression {
        Scalar::IntegerBinary {
            left,
            right,
            formation_decided,
            ..
        } => {
            *formation_decided = false;
            clear_scalar_formation_decisions(left);
            clear_scalar_formation_decisions(right);
        }
        Scalar::IntegerExactCast {
            operand,
            formation_decided,
            ..
        } => {
            *formation_decided = false;
            clear_scalar_formation_decisions(operand);
        }
        Scalar::IntegerBitwiseNot { operand, .. }
        | Scalar::IntegerWiden { operand, .. }
        | Scalar::RepresentationView { operand, .. } => clear_scalar_formation_decisions(operand),
        Scalar::Boolean(expression) => clear_boolean_formation_decisions(expression),
        _ => {}
    }
}

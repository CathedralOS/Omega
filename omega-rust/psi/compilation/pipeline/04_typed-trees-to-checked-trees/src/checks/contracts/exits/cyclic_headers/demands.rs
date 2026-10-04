//! Backward demand transport substitutes private current-frame atoms only.
//! Invocation atoms stay immutable even when their source symbol also names a
//! reentered parameter. An actual argument is read in the pre-edge caller's
//! roster, and its replacement is never substituted again on the same edge.
//! These are bounded search expressions, not facts or new typed source nodes.

use super::{Header, ScopedArithmeticBinding, ScopedArithmeticExpression, ScopedArithmeticValue};

/// Bounds all nested substitution and cloning work across one machine's
/// state graph, including alternative incoming state-simple paths.
pub(super) const TRANSPORT_WORK: usize = 16_384;
const SUBSTITUTION_DEPTH: usize = 64;

fn charge(remaining: &mut usize) -> Option<()> {
    *remaining = remaining.checked_sub(1)?;
    Some(())
}

pub(super) fn substitute(
    demand: &ScopedArithmeticExpression,
    header: &Header<'_, '_>,
    replacements: &[ScopedArithmeticBinding],
    remaining: &mut usize,
) -> Option<ScopedArithmeticExpression> {
    let current = header.current_atoms();
    let substitutions = current
        .iter()
        .filter_map(|source| {
            replacements
                .iter()
                .find(|replacement| replacement.binder == source.binder)
                .map(|replacement| (&source.value, &replacement.value))
        })
        .collect::<Vec<_>>();
    expression(demand, &substitutions, remaining, SUBSTITUTION_DEPTH)
}

fn expression(
    source: &ScopedArithmeticExpression,
    substitutions: &[(&ScopedArithmeticValue, &ScopedArithmeticValue)],
    remaining: &mut usize,
    depth: usize,
) -> Option<ScopedArithmeticExpression> {
    charge(remaining)?;
    let next_depth = depth.checked_sub(1)?;
    let mut bindings = Vec::new();
    for binding in &source.bindings {
        charge(remaining)?;
        bindings.push(ScopedArithmeticBinding {
            binder: binding.binder,
            value: value(&binding.value, substitutions, remaining, next_depth)?,
        });
    }
    Some(ScopedArithmeticExpression {
        expression: source.expression,
        bindings,
    })
}

fn value(
    source: &ScopedArithmeticValue,
    substitutions: &[(&ScopedArithmeticValue, &ScopedArithmeticValue)],
    remaining: &mut usize,
    depth: usize,
) -> Option<ScopedArithmeticValue> {
    charge(remaining)?;
    if let ScopedArithmeticValue::Atom { .. } = source {
        for (current, replacement) in substitutions {
            charge(remaining)?;
            if source == *current {
                // Empty substitution performs a bounded clone. Applying the
                // same edge again would corrupt simultaneous swaps and would
                // also rebind invocation values captured by the argument.
                return value(replacement, &[], remaining, depth);
            }
        }
    }
    Some(match source {
        ScopedArithmeticValue::Atom { .. } | ScopedArithmeticValue::Integer(_) => source.clone(),
        ScopedArithmeticValue::IntegerOperation {
            operator,
            left,
            right,
        } => {
            let next_depth = depth.checked_sub(1)?;
            ScopedArithmeticValue::IntegerOperation {
                operator: *operator,
                left: Box::new(value(left, substitutions, remaining, next_depth)?),
                right: Box::new(value(right, substitutions, remaining, next_depth)?),
            }
        }
        ScopedArithmeticValue::Term(term) => {
            ScopedArithmeticValue::Term(expression(term, substitutions, remaining, depth)?)
        }
        ScopedArithmeticValue::AdmittedIntegerRemainder(term) => {
            ScopedArithmeticValue::AdmittedIntegerRemainder(expression(
                term,
                substitutions,
                remaining,
                depth,
            )?)
        }
    })
}

//! Producer-local affine-definition candidate indexing.

pub(super) mod recording;

use semantic_vocabulary::{Proposition, ScalarTerm};

pub(crate) use recording::{CheckedWord, DefinitionIndex};

/// Visit every `(row, input)` pair where semantic row `row` defines a value
/// by an affine-shaped expression that reads the value `input`, in row order.
/// Selection is syntactic only; the kernel replays every chosen prefix.
fn visit_affine_inputs(
    semantic_axioms: &[Proposition],
    mut candidate: impl FnMut(usize, &ScalarTerm),
) {
    for (index, proposition) in semantic_axioms.iter().enumerate() {
        let Proposition::Equal(left, right) = proposition else {
            continue;
        };
        for (target, expression) in [(left, right), (right, left)] {
            if !matches!(target, ScalarTerm::Value { .. }) {
                continue;
            }
            for input in affine_inputs(expression) {
                candidate(index, input);
            }
        }
    }
}

fn affine_inputs(expression: &ScalarTerm) -> impl Iterator<Item = &ScalarTerm> {
    let inputs = match expression {
        // A bitwise and resumes a chain at either operand; the kernel admits
        // the reading only when the sibling operand lands as a non-negative
        // mask literal.
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. }
        | ScalarTerm::ExactIntegerDivide { left, right, .. }
        | ScalarTerm::ExactIntegerRemainder { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. } => {
            [Some(left.as_ref()), Some(right.as_ref())]
        }
        // Only the dividend of a wrapping division resumes a chain; the
        // divisor must resolve as a literal and never carries a bound.
        ScalarTerm::ExactIntegerSubtract { left, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, .. }
        | ScalarTerm::WrappingIntegerDivide { left, .. } => [Some(left.as_ref()), None],
        _ => [None, None],
    };
    inputs
        .into_iter()
        .flatten()
        .filter(|input| matches!(input, ScalarTerm::Value { .. }))
}

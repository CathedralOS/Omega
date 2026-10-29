//! A storage projection captures selectors as values, not mutable spellings.
//! Source segments remain available for typed observation planning; equality
//! of an element uses its completed selector, independently of that syntax.

use super::super::{Binding, ScalarField, ScopedOccurrence, admitted, scoped};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use validation::ScopedArithmeticValue;

#[derive(Clone, Debug)]
pub(in crate::checks::current_scalar_values) enum ScalarProjection {
    Field(SymbolHandle),
    Case(SymbolHandle),
    Element {
        source: facts::PlaceSegment,
        selector: ScopedArithmeticValue,
    },
}

impl PartialEq for ScalarProjection {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Field(left), Self::Field(right)) | (Self::Case(left), Self::Case(right)) => {
                left == right
            }
            (
                Self::Element { selector: left, .. },
                Self::Element {
                    selector: right, ..
                },
            ) => left == right,
            _ => false,
        }
    }
}

/// Captured recipes share a coordinate only when the existing scoped integer
/// judgment proves their selectors equal. Equality of source handles or carrier
/// types supplies no such proof; structural equality is only the fast path.
pub(in crate::checks::current_scalar_values) fn same_projection(
    occurrence: ScopedOccurrence<'_, '_>,
    left: &[ScalarProjection],
    right: &[ScalarProjection],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            if left == right {
                return true;
            }
            let (
                ScalarProjection::Element { selector: left, .. },
                ScalarProjection::Element {
                    selector: right, ..
                },
            ) = (left, right)
            else {
                return false;
            };
            validation::scoped_arithmetic_values_relation(
                occurrence.program,
                occurrence.machine,
                occurrence.hypotheses,
                typed_trees::expression::BinaryOperator::Equal,
                left,
                right,
            ) == validation::StrictArithmeticImplicationJudgment::Proven
        })
}

pub(in crate::checks::current_scalar_values) fn projection_has_prefix(
    occurrence: ScopedOccurrence<'_, '_>,
    path: &[ScalarProjection],
    prefix: &[ScalarProjection],
) -> bool {
    path.len() >= prefix.len() && same_projection(occurrence, &path[..prefix.len()], prefix)
}

pub(in crate::checks::current_scalar_values) fn source_path(
    path: &[ScalarProjection],
) -> Vec<facts::PlaceSegment> {
    path.iter()
        .map(|segment| match segment {
            ScalarProjection::Field(symbol) => facts::PlaceSegment::Field { symbol: *symbol },
            ScalarProjection::Case(variant) => facts::PlaceSegment::Case { variant: *variant },
            ScalarProjection::Element { source, .. } => *source,
        })
        .collect()
}

pub(in crate::checks::current_scalar_values) fn capture_path(
    path: &[facts::PlaceSegment],
    mut capture: impl FnMut(ExpressionHandle) -> Option<ScopedArithmeticValue>,
) -> Option<Vec<ScalarProjection>> {
    path.iter()
        .map(|segment| {
            Some(match segment {
                facts::PlaceSegment::Field { symbol } => ScalarProjection::Field(*symbol),
                facts::PlaceSegment::Case { variant } => ScalarProjection::Case(*variant),
                facts::PlaceSegment::FixedIndex { index } => ScalarProjection::Element {
                    source: *segment,
                    selector: ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(
                        u64::try_from(*index).ok()?,
                    )),
                },
                facts::PlaceSegment::Index { expression } => ScalarProjection::Element {
                    source: *segment,
                    selector: capture(*expression)?,
                },
                _ => return None,
            })
        })
        .collect()
}

/// Entry inference may use values already present in the invocation, never a
/// producer that will execute in the body. Check the whole executable tree:
/// a folded length can otherwise hide an effectful collection underneath it.
pub(in crate::checks::current_scalar_values) fn entry_selector_value(
    occurrence: ScopedOccurrence<'_, '_>,
    expression: ExpressionHandle,
    bindings: &[Binding],
    fields: &[ScalarField],
) -> Option<ScopedArithmeticValue> {
    if !validation::expression_traversal::inert_value_tree_with_operations(
        occurrence.program,
        expression,
        |operation| admitted(occurrence, operation, bindings, fields, 0),
    ) {
        return None;
    }
    let mut expressions = Vec::new();
    validation::expression_traversal::collect_expression_tree(
        occurrence.program,
        expression,
        &mut expressions,
    );
    if expressions.iter().any(|expression| {
        let typed_trees::expression::ExpressionNode::Name(name) =
            occurrence.program.expression_table.expression(*expression)
        else {
            return false;
        };
        occurrence
            .program
            .state_parameter_by_symbol(occurrence.state, name.symbol)
            .is_none()
            && !bindings
                .iter()
                .any(|binding| binding.symbol == name.symbol && binding.value.is_some())
            && occurrence
                .program
                .const_declaration_by_symbol(name.symbol)
                .is_none()
    }) {
        return None;
    }
    selector_value(occurrence, expression, bindings, fields)
}

pub(in crate::checks::current_scalar_values) fn selector_value(
    occurrence: ScopedOccurrence<'_, '_>,
    expression: ExpressionHandle,
    bindings: &[Binding],
    fields: &[ScalarField],
) -> Option<ScopedArithmeticValue> {
    if let typed_trees::expression::ExpressionNode::Name(name) =
        occurrence.program.expression_table.expression(expression)
        && name.symbol == name.head_symbol
        && let Some(binding) = bindings
            .iter()
            .find(|binding| binding.symbol == name.symbol)
    {
        return binding.value.clone();
    }
    if !admitted(occurrence, expression, bindings, fields, 0) {
        return None;
    }
    let term = scoped(occurrence, expression, bindings, fields)?;
    Some(
        crate::checks::contracts::prover::call_guarantees::arithmetic::completed_scalar_value(
            occurrence.program,
            &crate::checks::contracts::prover::call_guarantees::callable::Callable::Machine {
                machine: occurrence.machine,
                state: occurrence.state,
            },
            term,
        ),
    )
}

pub(in crate::checks::current_scalar_values) fn storage_path(
    occurrence: ScopedOccurrence<'_, '_>,
    expression: ExpressionHandle,
    bindings: &[Binding],
    fields: &[ScalarField],
) -> Option<(SymbolHandle, Vec<ScalarProjection>)> {
    let place = validation::canonical_place_from_expression(occurrence.program, expression)?;
    let facts::PlaceRoot::Symbol(owner) = place.root else {
        return None;
    };
    Some((
        owner,
        capture_path(&place.segments, |selector| {
            selector_value(occurrence, selector, bindings, fields)
        })?,
    ))
}

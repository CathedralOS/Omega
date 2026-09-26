//! Values an expression has independently of its operator structure.
//!
//! A closed record projection, a fixed array length, or a complete anonymous
//! numeric subtree answers the walk before any operator obligation is
//! considered. A member read that has lost its declared field binding answers
//! nothing, so it cannot lend a bound to the arithmetic around it.

use super::{Analysis, ExpressionWalk, NEUTRAL};
use crate::validation::proof_contracts::arithmetic_domains::integer_ranges::{
    integer_bit_width, literal_interval,
};
use crate::validation::proof_contracts::arithmetic_domains::{
    ArithmeticDomain, Diagnostic, ExpressionHandle, ExpressionNode, Interval, Machine, State,
    TypeReferenceNode, TypedTrees, declared_place_type_raw,
};
use crate::validation::value_custody::literals;
use language_semantics::declaration_selection::CollectionMeasure;

pub(super) fn analyze(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Analysis> {
    let members = member_custody(walk, expression, diagnostics);
    if members == MemberCustody::Broken {
        return Some(NEUTRAL);
    }
    if let Some(value) = literals::closed_record_integer_projection(walk.program, expression)
        && let Some(primitive) = value.primitive
        && let Some(literal) = literals::land_integer_value(&value.value, primitive)
    {
        return Some(Analysis {
            domain: Some(ArithmeticDomain::Exact),
            interval: literal_interval(&literal),
            primitive: Some(primitive),
        });
    }
    if let Some(length) = fixed_array_length(walk.program, walk.machine, walk.state, expression) {
        return Some(Analysis {
            interval: Interval {
                low: Some(length),
                high: Some(length),
            },
            ..NEUTRAL
        });
    }
    // A member read folded into the anonymous value may have lost the field's
    // declared carrier during evaluation; the structural walk still owes that
    // carrier its operand and overflow obligations.
    if members == MemberCustody::Absent {
        return anonymous_value(walk, expression, diagnostics);
    }
    None
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MemberCustody {
    /// The expression reads no member.
    Absent,
    /// Every member read keeps its declared field binding.
    Retained,
    /// A member read resolves through the loose name fallback but not through
    /// its declared field's symbol.
    Broken,
}

/// A member read that has lost record custody cannot supply a point bound or a
/// carrier to any enclosing arithmetic. Every such read is reported.
fn member_custody(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> MemberCustody {
    let program = walk.program;
    let mut custody = MemberCustody::Absent;
    let mut pending = vec![expression];
    while let Some(handle) = pending.pop() {
        let node = program.expression_table.expression(handle);
        if matches!(node, ExpressionNode::Member(_)) {
            if custody == MemberCustody::Absent {
                custody = MemberCustody::Retained;
            }
            if literals::closed_record_integer_projection(program, handle).is_none()
                && program
                    .closed_integer_value_in(handle, walk.machine.symbol)
                    .is_some()
            {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "member read in {} lost its declared field binding -- a foreign or \
                         missing field cannot supply a point bound",
                        walk.owner,
                    ))
                    .with_source_span(program.expression_table.source_span(handle)),
                );
                custody = MemberCustody::Broken;
            }
        }
        literals::expression_children::children(program, node, |child| pending.push(child));
    }
    custody
}

fn fixed_array_length(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<i64> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    if CollectionMeasure::from_authored_spelling(member.member.as_str())
        != Some(CollectionMeasure::Length)
        || member.case_variant.is_some()
    {
        return None;
    }
    let mut receiver = declared_place_type_raw(program, machine, state, member.receiver)?;
    loop {
        match program.type_reference_table.type_reference(receiver) {
            TypeReferenceNode::Reference { referee, .. } => receiver = *referee,
            TypeReferenceNode::FixedArray {
                length:
                    symbol_resolved_trees_to_typed_trees::typed_trees::types::FixedArrayLength::Literal(
                        length,
                    ),
                ..
            } => return i64::try_from(*length).ok(),
            // A constrained byte carrier can have a live length distinct from
            // its capacity. Do not strip that shell to invent an exact length.
            _ => return None,
        }
    }
}

/// A complete anonymous subtree chooses its exact value before an integer
/// operand/destination requests a rendering. Never truncate a child quotient
/// before a later multiplication can cancel its denominator.
fn anonymous_value(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Analysis> {
    let program = walk.program;
    let evaluated = literals::anonymous_numeric_value(program, expression, &mut |expression| {
        literals::has_anonymous_operator_meaning(program, expression)
    })?;
    let Some(value) = evaluated.value.to_integer_exact() else {
        if walk
            .target_primitive
            .is_some_and(|primitive| integer_bit_width(primitive).is_some())
        {
            diagnostics.push(
                Diagnostic::error(format!(
                    "anonymous operand `{}` is not an integer in {}; type an operand before \
                     division if integer division was intended",
                    evaluated.value, walk.owner,
                ))
                .with_source_span(program.expression_table.source_span(expression)),
            );
        }
        // Without an integer destination this is an exact rational value, not
        // a truncated integer interval that may manufacture zero.
        return Some(NEUTRAL);
    };
    let interval = value
        .to_i64()
        .map_or(Interval::UNBOUNDED, |value| Interval {
            low: Some(value),
            high: Some(value),
        });
    Some(Analysis {
        domain: None,
        interval,
        primitive: None,
    })
}

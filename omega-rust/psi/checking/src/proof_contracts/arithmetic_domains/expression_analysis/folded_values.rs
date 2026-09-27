//! Values an expression has independently of its operator structure.
//!
//! A closed record projection, a collection length, or a complete anonymous
//! numeric subtree answers the walk before any operator obligation is
//! considered. A member read that has lost its declared field binding answers
//! nothing, so it cannot lend a bound to the arithmetic around it.

use super::{Analysis, ExpressionWalk, NEUTRAL};
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    integer_bit_width, literal_interval,
};
use crate::proof_contracts::arithmetic_domains::{
    ArithmeticDomain, Diagnostic, ExpressionHandle, ExpressionNode, Interval, Machine,
    PrimitiveType, State, TypeReferenceNode, TypedTrees, declared_place_type_raw,
};
use crate::value_custody::literals;

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
    if let Some(interval) =
        collection_length_interval(walk.program, walk.machine, walk.state, expression)
    {
        return Some(Analysis {
            // Counts keep their declared carrier even when their value is known.
            // They do not select an arithmetic policy for enclosing operations.
            interval,
            primitive: Some(PrimitiveType::U64),
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
        crate::expression_traversal::children(program, node, |child| pending.push(child));
    }
    custody
}

fn collection_length_interval(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<Interval> {
    let receiver = crate::collection_length_receiver(program, machine, state, expression)?;
    let mut receiver = declared_place_type_raw(program, machine, state, receiver)?;
    loop {
        // A bounded byte buffer's capacity limits its live length; it is not
        // the exact length of every value carried by the constrained type.
        if let Some(capacity) = crate::bounded_byte_buffer_capacity(program, receiver) {
            return Some(Interval {
                low: Some(0),
                high: i64::try_from(capacity).ok(),
            });
        }
        match program.type_reference_table.type_reference(receiver) {
            TypeReferenceNode::Reference { referee, .. } => receiver = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => receiver = *base_type,
            TypeReferenceNode::FixedArray {
                length: typed_trees::types::FixedArrayLength::Literal(length),
                ..
            } => {
                let length = i64::try_from(*length).ok();
                return Some(Interval {
                    low: Some(length.unwrap_or(0)),
                    high: length,
                });
            }
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

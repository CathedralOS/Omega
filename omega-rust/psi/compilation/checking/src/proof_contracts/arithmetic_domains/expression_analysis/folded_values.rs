//! Values an expression has independently of its operator structure.
//!
//! A closed record projection, a collection length, or a complete anonymous
//! numeric subtree answers the walk before any operator obligation is
//! considered. The caller checks evaluated children first. Projection queries
//! retain declared field custody; anonymous evaluation never selects a field
//! leaf or erases its carrier.

use super::{Analysis, ExpressionWalk, NEUTRAL};
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    integer_bit_width, literal_interval,
};
use crate::proof_contracts::arithmetic_domains::{
    ArithmeticDomain, Diagnostic, ExpressionHandle, Interval, PrimitiveType, TypeReferenceNode,
};
use crate::value_custody::literals;

pub(super) fn analyze(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Analysis> {
    if let Some(value) = literals::closed_record_integer_projection(walk.program, expression)
        && let Some(primitive) = value.primitive
        && let Some(literal) = literals::land_integer_value(&value.value, primitive)
    {
        return Some(Analysis {
            recipe: None,
            domain: Some(ArithmeticDomain::Exact),
            interval: literal_interval(&literal),
            primitive: Some(primitive),
        });
    }
    if let Some(interval) = collection_length_interval(walk, expression) {
        let interval = super::super::place_path(walk.program, expression)
            .and_then(|path| walk.environment.get(&path))
            .map_or(interval, |retained| interval.intersect(retained));
        // A builtin count is still an observed integer. Its early carrier
        // answer must consume the same surviving order facts as an ordinary
        // place read; otherwise coarse content frames erase the string cache
        // and hide already-proved facts about an unchanged descriptor.
        let interval = walk.machine.map_or(interval, |machine| {
            super::super::ordered_values::refine_place_interval(
                walk.program,
                machine,
                walk.state,
                walk.environment,
                expression,
                interval,
            )
        });
        return Some(Analysis {
            recipe: None,
            // Counts keep their declared carrier even when their value is known.
            // They do not select an arithmetic policy for enclosing operations.
            interval,
            primitive: Some(PrimitiveType::U64),
            ..NEUTRAL
        });
    }
    anonymous_value(walk, expression, diagnostics)
}

fn collection_length_interval(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
) -> Option<Interval> {
    let program = walk.program;
    let mut receiver = if let Some(machine) = walk.machine {
        let receiver = crate::collection_length_receiver(program, machine, walk.state, expression)?;
        if let Some(state) = walk.state
            && let Some(length) = crate::closed_collection_length(program, machine, state, receiver)
        {
            let length = i64::try_from(length).ok();
            return Some(Interval {
                low: Some(length.unwrap_or(0)),
                high: length,
            });
        }
        walk.declared_type(receiver)?
    } else {
        let member = program.expression_table.member_node(expression)?;
        let receiver = walk.result_type(member.receiver)?;
        if crate::collection_operations::collection_measure_member(
            program,
            member,
            crate::collection_operations::MeasureReceiver::Declared(receiver),
        ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
        {
            return None;
        }
        receiver
    };
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
            // A dynamic extent still denotes a u64 count. Unknown length is
            // an unknown value of that carrier, not an anonymous expression
            // exempt from exact conversion and arithmetic obligations.
            TypeReferenceNode::Slice { .. } | TypeReferenceNode::FixedArray { .. } => {
                return Some(Interval {
                    low: Some(0),
                    high: None,
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
        recipe: None,
        domain: None,
        interval,
        primitive: None,
    })
}

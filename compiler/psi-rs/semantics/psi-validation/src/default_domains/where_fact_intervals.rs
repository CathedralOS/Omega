//! Read-only interval hypotheses derived from standing default-domain facts.
//!
//! This module does not participate in write-frame inference. It projects
//! only facts that the write validator has already established at every legal
//! observation point, and refuses cyclic or unsupported bounds.

use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::DataDefinition;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;

use super::place_queries::data_definition_for_expression;
use super::symbolic_values::integer_literal_value;
use crate::arithmetic_domains::Interval;

/// R2 rung 3 slice 7 -- READER HYPOTHESES: the standing where facts refine
/// a field READ's interval. Sound because the write net is TOTAL (every
/// write path re-proves the facts) and gated reads are access-gated, so
/// the facts hold at every legal observation. Bounds come from literals or
/// the co-field's DECLARED range (declared ranges always hold), never from
/// flow values.
pub(crate) fn where_fact_interval(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<Interval> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    let definition = data_definition_for_expression(program, machine, state, member.receiver)?;
    field_fact_interval(program, definition, member.member.as_str(), 0)
}

/// The per-field core of the reader hypotheses: the interval the
/// definition's standing where facts pin on `field`. `depth` bounds the
/// TRANSITIVE chain (`count <= mid, mid <= capacity[0..=100]` chains
/// through the unranged middle field); a cyclic pair exhausts the cap and
/// resolves to None -- over-refusal only.
fn field_fact_interval(
    program: &TypedTrees,
    definition: &DataDefinition,
    field: &str,
    depth: usize,
) -> Option<Interval> {
    if depth >= 4 || definition.where_facts.is_empty() {
        return None;
    }

    let mut interval = Interval {
        low: None,
        high: None,
    };
    let mut refined = false;
    for fact in program.proof_facts.span_or_empty(definition.where_facts) {
        let psi_typed_trees::domain::ProofFact::Expression(fact_expression) = fact else {
            continue;
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*fact_expression)
        else {
            continue;
        };
        // R2 rung 3 slice 10 -- PRODUCT hypotheses (`count * stride <= len`,
        // ch12's canonical shape): when OUR field is one FACTOR of a
        // product bounded above, the field's upper bound is
        // bound.high / co-factor.low (floor) -- SOUND iff the co-factor's
        // lower bound is >= 1 (from its declared range or a sibling
        // literal fact) and the field's primitive is UNSIGNED (>= 0).
        if matches!(
            binary.operator,
            BinaryOperator::LessOrEqual | BinaryOperator::Less
        ) && let ExpressionNode::Binary(product) =
            program.expression_table.expression(binary.left)
            && matches!(product.operator, BinaryOperator::Multiply)
        {
            let factor = if side_names_field(program, product.left, field) {
                Some(product.right)
            } else if side_names_field(program, product.right, field) {
                Some(product.left)
            } else {
                None
            };
            if let Some(factor) = factor
                && field_is_unsigned(program, definition, field)
                && let Some(factor_low) = factor_lower_bound(program, definition, factor)
                && factor_low >= 1
                && let Some(bound) = bound_source_interval(program, definition, binary.right, depth)
                && let Some(mut bound_high) = bound.high
            {
                if matches!(binary.operator, BinaryOperator::Less) {
                    bound_high = bound_high.saturating_sub(1);
                }
                let high = bound_high.div_euclid(factor_low);
                interval.high = Some(interval.high.map_or(high, |current| current.min(high)));
                refined = true;
                continue;
            }
        }
        let (field_side_left, other) = if side_names_field(program, binary.left, field) {
            (true, binary.right)
        } else if side_names_field(program, binary.right, field) {
            (false, binary.left)
        } else {
            continue;
        };
        let Some(other_interval) = bound_source_interval(program, definition, other, depth) else {
            continue;
        };
        let operator = if field_side_left {
            binary.operator
        } else {
            match binary.operator {
                BinaryOperator::Less => BinaryOperator::Greater,
                BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
                BinaryOperator::Greater => BinaryOperator::Less,
                BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
                other_operator => other_operator,
            }
        };
        match operator {
            BinaryOperator::LessOrEqual => {
                if let Some(high) = other_interval.high {
                    interval.high = Some(interval.high.map_or(high, |current| current.min(high)));
                    refined = true;
                }
            }
            BinaryOperator::Less => {
                if let Some(high) = other_interval.high.and_then(|high| high.checked_sub(1)) {
                    interval.high = Some(interval.high.map_or(high, |current| current.min(high)));
                    refined = true;
                }
            }
            BinaryOperator::GreaterOrEqual => {
                if let Some(low) = other_interval.low {
                    interval.low = Some(interval.low.map_or(low, |current| current.max(low)));
                    refined = true;
                }
            }
            BinaryOperator::Greater => {
                if let Some(low) = other_interval.low.and_then(|low| low.checked_add(1)) {
                    interval.low = Some(interval.low.map_or(low, |current| current.max(low)));
                    refined = true;
                }
            }
            _ => {}
        }
    }
    refined.then_some(interval)
}

fn side_names_field(program: &TypedTrees, expression: ExpressionHandle, field: &str) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .last()
            .is_some_and(|member| member.as_str() == field),
        _ => false,
    }
}

/// The bound-supplying side's SOUND interval: a literal is itself; a
/// co-field name reads its DECLARED range (or full type width) from the
/// data definition's own members.
fn bound_source_interval(
    program: &TypedTrees,
    definition: &DataDefinition,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<Interval> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => {
            let literal = value.text().parse::<i64>().ok()?;
            Some(Interval {
                low: Some(literal),
                high: Some(literal),
            })
        }
        ExpressionNode::Name(path) => {
            let name = program
                .expression_table
                .name_path_members(path.members)
                .last()?;
            let handle =
                program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        psi_typed_trees::data::DataMember::Field(data_field)
                            if data_field.name == *name =>
                        {
                            Some(data_field.type_reference)
                        }
                        _ => None,
                    })?;
            crate::arithmetic_domains::range_constraint_interval(program, handle)
                .or_else(|| field_fact_interval(program, definition, name.as_str(), depth + 1))
        }
        _ => None,
    }
}

/// Slice 10: is the definition's named field an UNSIGNED primitive (its
/// values are >= 0 -- the product-division soundness guard)?
fn field_is_unsigned(program: &TypedTrees, definition: &DataDefinition, field: &str) -> bool {
    use psi_typed_trees::types::PrimitiveType;
    program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(data_field)
                if data_field.name.as_str() == field =>
            {
                program.primitive_type_reference(data_field.type_reference)
            }
            _ => None,
        })
        .is_some_and(|primitive| {
            matches!(
                primitive,
                PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
            )
        })
}

/// Slice 10: a co-factor's LOWER bound -- its declared range, or a sibling
/// literal fact (`stride >= 40` / `40 <= stride`), single level.
fn factor_lower_bound(
    program: &TypedTrees,
    definition: &DataDefinition,
    factor: ExpressionHandle,
) -> Option<i64> {
    let ExpressionNode::Name(path) = program.expression_table.expression(factor) else {
        return None;
    };
    let factor_name = program
        .expression_table
        .name_path_members(path.members)
        .last()?
        .as_str();
    if let Some(interval) = bound_source_interval(program, definition, factor, 4)
        && let Some(low) = interval.low
    {
        return Some(low);
    }
    for fact in program.proof_facts.span_or_empty(definition.where_facts) {
        let psi_typed_trees::domain::ProofFact::Expression(expression) = fact else {
            continue;
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression)
        else {
            continue;
        };
        let bound = match binary.operator {
            BinaryOperator::GreaterOrEqual
                if side_names_field(program, binary.left, factor_name) =>
            {
                integer_literal_value(program, binary.right).map(|value| value as i64)
            }
            BinaryOperator::Greater if side_names_field(program, binary.left, factor_name) => {
                integer_literal_value(program, binary.right)
                    .map(|value| (value as i64).saturating_add(1))
            }
            BinaryOperator::LessOrEqual if side_names_field(program, binary.right, factor_name) => {
                integer_literal_value(program, binary.left).map(|value| value as i64)
            }
            BinaryOperator::Less if side_names_field(program, binary.right, factor_name) => {
                integer_literal_value(program, binary.left)
                    .map(|value| (value as i64).saturating_add(1))
            }
            _ => None,
        };
        if let Some(bound) = bound {
            return Some(bound);
        }
    }
    None
}

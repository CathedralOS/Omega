use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use crate::proof_contracts::contract_entailment::RankingRangeCallSite;

use super::projection::{RankOrder, RankProjection, unwrapped};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Comparison {
    Equal,
    Strict,
}

pub(super) fn argument_comparison(
    program: &TypedTrees,
    machine: SymbolHandle,
    rank: &RankProjection,
    argument: ExpressionHandle,
    guards: &[(ExpressionHandle, bool)],
    site: &RankingRangeCallSite<'_>,
) -> Option<Comparison> {
    let RankOrder::Lexicographic {
        data,
        fields: components,
        ..
    } = &rank.order
    else {
        return None;
    };
    // A call issued from a subordinate state spells the site's formals, not
    // the member's authored entry names. The discovered telescope translates
    // each carried formal back to its entry role so the unchanged rank
    // components and the guarded decrement name the same values the member's
    // own ranking judgment proved at this arrival.
    let role = |symbol: SymbolHandle| site_role(program, site, symbol);
    if rank.is_subject(program, argument, &role) {
        return Some(Comparison::Equal);
    }
    let ExpressionNode::StructLiteral(literal) = program
        .expression_table
        .expression(unwrapped(program, argument))
    else {
        return None;
    };
    if literal.type_symbol != *data || literal.case_name.is_some() || literal.case_symbol.is_some()
    {
        return None;
    }
    let fields = program.expression_table.struct_fields(literal.fields);
    let mut strict = false;
    for field in components {
        let mut matching = fields.iter().filter(|value| value.field_symbol == *field);
        let value = matching.next()?.value;
        if matching.next().is_some() {
            return None;
        }
        // A strict earlier component allows arbitrary later components, but
        // all fields still have exact, unique declaration associations.
        if strict {
            continue;
        }
        strict = component_comparison(program, machine, rank, value, *field, guards, &role)?
            == Comparison::Strict;
    }
    Some(if strict {
        Comparison::Strict
    } else {
        Comparison::Equal
    })
}

/// The entry role a spelled symbol carries at the issuing site: each non-self
/// formal answers to the entry parameter its telescope slot recorded. Any
/// other name -- an entry-state spelling of the parameter itself, a local, or
/// a global -- stands for itself, so the identity telescope keeps an
/// entry-state call unchanged. A formal with no discovered role answers
/// invalid rather than borrowing an entry premise it never carried.
fn site_role(
    program: &TypedTrees,
    site: &RankingRangeCallSite<'_>,
    symbol: SymbolHandle,
) -> SymbolHandle {
    program
        .state_parameters(site.state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(site.entry_parameters)
        .find_map(|(parameter, role)| (parameter.symbol == symbol).then_some(*role))
        .unwrap_or(symbol)
}

fn component_comparison(
    program: &TypedTrees,
    machine: SymbolHandle,
    rank: &RankProjection,
    value: ExpressionHandle,
    field: SymbolHandle,
    guards: &[(ExpressionHandle, bool)],
    role: &dyn Fn(SymbolHandle) -> SymbolHandle,
) -> Option<Comparison> {
    if rank.is_component(program, value, field, role) {
        return Some(Comparison::Equal);
    }
    let ExpressionNode::Binary(binary) = program
        .expression_table
        .expression(unwrapped(program, value))
    else {
        return None;
    };
    if binary.operator != BinaryOperator::Subtract
        || !rank.is_component(program, binary.left, field, role)
    {
        return None;
    }
    let amount = integer(program, machine, binary.right)?;
    (amount > 0
        && guards.iter().any(|(guard, truth)| {
            guard_proves_lower_bound(program, machine, rank, *guard, *truth, field, amount, role)
        }))
    .then_some(Comparison::Strict)
}

/// A step amount or bound is the closed integer its spelling already
/// evaluates to under the owning machine's operator selection: a literal, or
/// any constant arithmetic tree. A value that does not fold closed reads as
/// no amount, not an approximated one.
fn integer(
    program: &TypedTrees,
    machine: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<i64> {
    let expression = unwrapped(program, expression);
    if let ExpressionNode::Integer(literal) = program.expression_table.expression(expression) {
        return literal.value_i64();
    }
    program
        .closed_integer_value_in(expression, machine)?
        .value
        .to_i64()
}

/// Facts come from the current arm or an earlier failed dispatch guard, all
/// over the same unchanged entry binding. No rendered-name cache or facts
/// from another state/caller contribute to this lower-bound proof.
fn guard_proves_lower_bound(
    program: &TypedTrees,
    machine: SymbolHandle,
    rank: &RankProjection,
    guard: ExpressionHandle,
    truth: bool,
    field: SymbolHandle,
    minimum: i64,
    role: &dyn Fn(SymbolHandle) -> SymbolHandle,
) -> bool {
    if !guard.is_valid() {
        return false;
    }
    let ExpressionNode::Binary(binary) = program
        .expression_table
        .expression(unwrapped(program, guard))
    else {
        return false;
    };
    if matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        for (condition, boolean) in [(binary.left, binary.right), (binary.right, binary.left)] {
            if let ExpressionNode::Boolean(value) = program
                .expression_table
                .expression(unwrapped(program, boolean))
            {
                return guard_proves_lower_bound(
                    program,
                    machine,
                    rank,
                    condition,
                    truth == (*value == (binary.operator == BinaryOperator::Equal)),
                    field,
                    minimum,
                    role,
                );
            }
        }
    }
    if (binary.operator == BinaryOperator::And && truth)
        || (binary.operator == BinaryOperator::Or && !truth)
    {
        return guard_proves_lower_bound(
            program,
            machine,
            rank,
            binary.left,
            truth,
            field,
            minimum,
            role,
        ) || guard_proves_lower_bound(
            program,
            machine,
            rank,
            binary.right,
            truth,
            field,
            minimum,
            role,
        );
    }
    let operator = if truth {
        binary.operator
    } else {
        match binary.operator {
            BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
            BinaryOperator::LessOrEqual => BinaryOperator::Greater,
            BinaryOperator::Greater => BinaryOperator::LessOrEqual,
            BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
            BinaryOperator::Equal => BinaryOperator::NotEqual,
            BinaryOperator::NotEqual => BinaryOperator::Equal,
            _ => return false,
        }
    };
    let (operator, bound) = if rank.is_component(program, binary.left, field, role) {
        (operator, integer(program, machine, binary.right))
    } else if rank.is_component(program, binary.right, field, role) {
        let operator = match operator {
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            BinaryOperator::Equal => BinaryOperator::Equal,
            BinaryOperator::NotEqual => BinaryOperator::NotEqual,
            _ => return false,
        };
        (operator, integer(program, machine, binary.left))
    } else {
        return false;
    };
    bound.is_some_and(|bound| match operator {
        BinaryOperator::Greater => bound >= minimum - 1,
        BinaryOperator::GreaterOrEqual | BinaryOperator::Equal => bound >= minimum,
        BinaryOperator::NotEqual => bound == 0 && minimum == 1,
        _ => false,
    })
}

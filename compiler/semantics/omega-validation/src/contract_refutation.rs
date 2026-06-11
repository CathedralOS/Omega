//! Refutation checks for empty-body proof machines.
//!
//! An empty-body machine with `requires`/`ensures` contracts is a proof
//! artifact: the claim is that the requirements entail the guarantees. The
//! full entailment engine is future work, but two refutations are already
//! decidable and sound here:
//!
//! - constant arithmetic: an ensures fact whose sides fold to constants that
//!   compare false can never be entailed by anything,
//! - strict-order asymmetry: `requires a < b` can never entail `ensures b < a`
//!   because strict orders are asymmetric.
//!
//! Both checks fire only when the machine body is empty (no statements that
//! could establish new facts) and only when the requires facts are not
//! themselves visibly contradictory (a contradictory hypothesis set makes any
//! ensures vacuously true, the proof-theoretic `absurd` case).

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::SignatureContractKind;

pub(crate) fn validate_machine_contract_refutation(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let body_is_empty = program.machine_states(machine).iter().all(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .is_empty()
    });
    if !body_is_empty {
        return;
    }

    let mut requires = Vec::new();
    let mut ensures = Vec::new();
    for contract in program.machine_contracts(machine) {
        let bucket = match contract.kind {
            SignatureContractKind::Requires => &mut requires,
            SignatureContractKind::Ensures => &mut ensures,
            SignatureContractKind::Boundary => continue,
        };
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(expression) = fact {
                bucket.push(*expression);
            }
        }
    }
    if ensures.is_empty() {
        return;
    }

    // Vacuity gates: visibly contradictory requirements entail everything.
    if requires
        .iter()
        .any(|fact| constant_boolean(program, *fact) == Some(false))
    {
        return;
    }
    let required_strict_orders = strict_order_facts(program, &requires);
    let requires_self_contradicts = required_strict_orders
        .iter()
        .any(|(_, smaller, larger)| contains_strict_order(&required_strict_orders, larger, smaller));
    if requires_self_contradicts {
        return;
    }

    for fact in &ensures {
        if constant_boolean(program, *fact) == Some(false) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` ensures contract proof fact `{}` is disproved by constant arithmetic",
                machine.name,
                program.expression_table.display_name(*fact)
            )));
            continue;
        }

        if let Some((smaller, larger)) = strict_order_operands(program, *fact) {
            if let Some((requires_fact, _, _)) = required_strict_orders
                .iter()
                .find(|(_, required_smaller, required_larger)| {
                    *required_smaller == larger && *required_larger == smaller
                })
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` ensures contract proof fact `{}` contradicts requires proof fact `{}`: strict orders are asymmetric",
                    machine.name,
                    program.expression_table.display_name(*fact),
                    program.expression_table.display_name(*requires_fact)
                )));
            }
        }
    }
}

/// Normalized strict-order facts as (fact, smaller, larger) display-name pairs.
fn strict_order_facts(
    program: &TypedTrees,
    facts: &[ExpressionHandle],
) -> Vec<(ExpressionHandle, String, String)> {
    facts
        .iter()
        .filter_map(|fact| {
            strict_order_operands(program, *fact)
                .map(|(smaller, larger)| (*fact, smaller, larger))
        })
        .collect()
}

fn contains_strict_order(
    orders: &[(ExpressionHandle, String, String)],
    smaller: &str,
    larger: &str,
) -> bool {
    orders
        .iter()
        .any(|(_, candidate_smaller, candidate_larger)| {
            candidate_smaller == smaller && candidate_larger == larger
        })
}

/// `a < b` and `b > a` both normalize to (smaller `a`, larger `b`). Operand
/// identity is structural spelling within one contract scope, where the same
/// spelling resolves to the same parameter or place.
fn strict_order_operands(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(String, String)> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    let (smaller, larger) = match binary.operator {
        BinaryOperator::Less => (binary.left, binary.right),
        BinaryOperator::Greater => (binary.right, binary.left),
        _ => return None,
    };
    let smaller = program.expression_table.display_name(smaller);
    let larger = program.expression_table.display_name(larger);
    if smaller == larger {
        return None;
    }
    Some((smaller, larger))
}

fn constant_boolean(program: &TypedTrees, expression: ExpressionHandle) -> Option<bool> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(*value),
        ExpressionNode::Mutable(inner) => constant_boolean(program, *inner),
        ExpressionNode::Unary(unary) => constant_boolean(program, unary.operand).map(|value| !value),
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual => {
                let left = constant_integer(program, binary.left)?;
                let right = constant_integer(program, binary.right)?;
                Some(match binary.operator {
                    BinaryOperator::Equal => left == right,
                    BinaryOperator::NotEqual => left != right,
                    BinaryOperator::Less => left < right,
                    BinaryOperator::LessOrEqual => left <= right,
                    BinaryOperator::Greater => left > right,
                    BinaryOperator::GreaterOrEqual => left >= right,
                    _ => unreachable!(),
                })
            }
            BinaryOperator::And => {
                match (
                    constant_boolean(program, binary.left),
                    constant_boolean(program, binary.right),
                ) {
                    (Some(false), _) | (_, Some(false)) => Some(false),
                    (Some(true), Some(true)) => Some(true),
                    _ => None,
                }
            }
            BinaryOperator::Or => {
                match (
                    constant_boolean(program, binary.left),
                    constant_boolean(program, binary.right),
                ) {
                    (Some(true), _) | (_, Some(true)) => Some(true),
                    (Some(false), Some(false)) => Some(false),
                    _ => None,
                }
            }
            _ => None,
        },
        _ => None,
    }
}

fn constant_integer(program: &TypedTrees, expression: ExpressionHandle) -> Option<i64> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => Some(*value),
        ExpressionNode::Mutable(inner) => constant_integer(program, *inner),
        ExpressionNode::Binary(binary) => {
            let left = constant_integer(program, binary.left)?;
            let right = constant_integer(program, binary.right)?;
            match binary.operator {
                BinaryOperator::Add => left.checked_add(right),
                BinaryOperator::Subtract => left.checked_sub(right),
                BinaryOperator::Multiply => left.checked_mul(right),
                BinaryOperator::Divide => left.checked_div(right),
                BinaryOperator::Modulo => left.checked_rem(right),
                _ => None,
            }
        }
        _ => None,
    }
}

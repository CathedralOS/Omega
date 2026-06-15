//! Mixed-domain rejection (frozen decision 17, stage S2). Arithmetic domains are
//! OPERAND-driven: the domain (`Wrapping`/`Saturating`/`Trapping`, or the default
//! `Exact`) lives on each value's type. A binary ARITHMETIC operation whose two
//! operands carry DIFFERENT explicit domains is a hard error -- "explicit wins",
//! no implicit domain mixing. Literals (and any operand whose domain cannot be
//! resolved to a declared type) are NEUTRAL and adopt the other operand's domain,
//! so `count + 1` where `count: u8 in Saturating` is fine.

use omega_core::arithmetic::ArithmeticDomain;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;

use crate::places::declared_place_type_raw;

/// Walk a value expression and reject any binary arithmetic op that mixes two
/// different explicit arithmetic domains. `owner` describes the site for the
/// diagnostic (e.g. "machine `M` state `s` local `c`").
pub(crate) fn validate_arithmetic_domains(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    let _ = domain_of(program, machine, state, expression, owner, diagnostics);
}

/// Operators whose result is an arithmetic value (so the operand domains must
/// agree). Comparisons and logical `and`/`or` produce a `bool` and carry no
/// arithmetic domain.
fn is_arithmetic(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
    )
}

/// Resolve an expression's arithmetic domain while CHECKING every nested binary
/// for mixed-domain operands (pushing a diagnostic on a conflict). `None` means
/// NEUTRAL: a literal, a comparison/logical (`bool`) result, or an expression
/// whose domain cannot be resolved -- it adopts whatever it is combined with.
fn domain_of(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ArithmeticDomain> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let left = domain_of(program, machine, state, binary.left, owner, diagnostics);
            let right = domain_of(program, machine, state, binary.right, owner, diagnostics);
            if !is_arithmetic(binary.operator) {
                return None;
            }
            if let (Some(left_domain), Some(right_domain)) = (left, right)
                && left_domain != right_domain
            {
                diagnostics.push(Diagnostic::error(format!(
                    "mixed arithmetic domains in {owner}: one operand is `{}` and the other is \
                     `{}`. Decision 17 forbids implicit domain mixing -- cross domains with an \
                     explicit `as` cast, or declare both operands in the same domain.",
                    left_domain.name(),
                    right_domain.name(),
                )));
            }
            // The operation's result domain: the non-`Exact` side wins (an exact
            // value or literal adopts the other operand's domain).
            match (left, right) {
                (Some(left_domain), Some(right_domain)) => Some(if left_domain
                    == ArithmeticDomain::Exact
                {
                    right_domain
                } else {
                    left_domain
                }),
                (Some(domain), None) | (None, Some(domain)) => Some(domain),
                (None, None) => None,
            }
        }
        // Literals are domain-neutral (polymorphic): they adopt the operand they
        // are combined with.
        ExpressionNode::Integer(_) | ExpressionNode::Float(_) | ExpressionNode::Boolean(_) => None,
        // A place (`x`, `self.field`): its declared type carries the domain.
        // Anything else (a cast, call, etc.) is neutral for now.
        _ => declared_place_type_raw(program, machine, state, expression)
            .map(|handle| program.arithmetic_domain_for_type_reference(handle)),
    }
}

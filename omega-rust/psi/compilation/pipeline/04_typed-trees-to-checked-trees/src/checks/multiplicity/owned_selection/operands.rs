//! operands slice of owned_selection.

use super::unsupported;
use arena::Handle;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern, TableMatchArm};
use typed_trees::statement::StatementNode;

/// A call moves no selection custody when it carries no receiver and every
/// target parameter is unrestricted: its product is then fresh owned storage
/// and its arguments cannot smuggle an owned leaf the receipt does not name.
/// An unresolvable target conservatively counts as a transfer.
pub(super) fn call_moves_no_ownership(
    program: &dyn ::validation::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    !call.receiver.is_valid()
        && validation::call_target_parameters(program, call.target_symbol).is_some_and(
            |parameters| {
                parameters.iter().all(|parameter| {
                    program.type_multiplicity(parameter.type_reference)
                        == Multiplicity::Unrestricted
                })
            },
        )
}

/// The hidden-ownership operand rule shared by fresh arm constructions and
/// fresh call arguments. A call operand stays admissible only while it moves
/// no custody itself; borrows, atomics, custody-moving calls, and named owned
/// operands all keep their explicit rejections.
pub(super) fn hidden_ownership_operands(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    operands: &[ExpressionHandle],
    message: &str,
) -> Result<(), Diagnostic> {
    if operands.iter().any(
        |operand| match program.expression_table.expression(*operand) {
            ExpressionNode::Call(call) => !call_moves_no_ownership(program, call),
            ExpressionNode::Borrow(_) | ExpressionNode::Atomic(_) => true,
            _ => false,
        },
    ) {
        return Err(unsupported());
    }
    let tag_operands = case_tag_operands(program, machine, state, operands);
    if operands.iter().any(|operand| {
        matches!(
            program.expression_table.expression(*operand),
            ExpressionNode::Name(_)
        ) && !tag_operands.contains(operand)
            && validation::expression_result_type_reference(program, machine, state, *operand)
                .is_none_or(|reference| {
                    program.type_multiplicity(reference) != Multiplicity::Unrestricted
                })
    }) {
        return Err(Diagnostic::error(message));
    }
    Ok(())
}

/// An exact case-membership comparison observes the subject's tag: authored
/// `subject in Type::Case` lowers to an equality whose right operand is a
/// symbol-stamped case reference with no result type and whose left operand is
/// read, not transferred. Both operands are exempt from the
/// unrestricted-operand rule; calls, borrows and atomic subjects still reject
/// separately, and operands of genuine authored equality are unchanged.
pub(super) fn case_tag_operands(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    operands: &[ExpressionHandle],
) -> Vec<ExpressionHandle> {
    operands
        .iter()
        .copied()
        .flat_map(
            |operand| match program.expression_table.expression(operand) {
                ExpressionNode::Binary(binary)
                    if validation::has_exact_case_membership_meaning(
                        program,
                        machine,
                        Some(state),
                        operand,
                        binary,
                    ) =>
                {
                    vec![binary.left, binary.right]
                }
                _ => Vec::new(),
            },
        )
        .collect()
}

pub(super) fn names_symbol(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    matches!(program.expression_table.expression(expression), ExpressionNode::Name(path) if path.symbol == symbol)
}

pub(super) fn expression_names(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    expression_nodes(program, expression)
        .iter()
        .any(|expression| names_symbol(program, *expression, symbol))
}

pub(super) fn statement_expressions(
    program: &dyn ::validation::CheckingInput,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    program
        .tables
        .expression_roots(statement)
        .into_iter()
        .filter(|(role, root)| {
            *role != typed_trees::statement::StatementRootRole::BindingImplementation
                || root.is_valid()
        })
        .flat_map(|(_, root)| expression_nodes(program, root))
        .collect()
}

pub(super) fn expression_nodes(
    program: &dyn ::validation::CheckingInput,
    root: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    let mut pending = vec![root];
    let mut nodes = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || nodes.contains(&expression) {
            continue;
        }
        nodes.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                pending.push(dispatch.subject);
                for (_, arm) in reachable_arms(program, dispatch.arms) {
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Atomic(atomic) => pending.extend([atomic.value, atomic.result]),
            ExpressionNode::ArrayLiteral(values) => {
                pending.extend_from_slice(program.expression_table.expression_handles(*values))
            }
            ExpressionNode::Call(call) => {
                pending.push(call.receiver);
                pending
                    .extend_from_slice(program.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Range(range) => pending.extend([range.start, range.end]),
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            _ => {}
        }
    }
    nodes
}

/// Count the reachable complete arm paths through nested selections: every
/// non-`match` arm value is one edge the linear frontier must agree on.
pub(super) fn terminal_arm_values(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> usize {
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => reachable_arms(program, dispatch.arms)
            .iter()
            .map(|(_, arm)| terminal_arm_values(program, arm.value))
            .sum(),
        _ => 1,
    }
}

/// Ordered first-match reachability. The source type checker still visits all
/// authored arms. Only literal equality and complete Boolean/wildcard coverage
/// remove ownership alternatives here; no predicate theorem is invented.
pub(in crate::checks) fn reachable_arms(
    program: &dyn ::validation::CheckingInput,
    arms: HandleSpan<TableMatchArm>,
) -> Vec<(Handle<TableMatchArm>, &TableMatchArm)> {
    let mut selected = Vec::new();
    let mut patterns = Vec::new();
    let mut booleans = [false; 2];
    for (ordinal, arm) in program.expression_table.match_arms(arms).iter().enumerate() {
        let mut covered = false;
        match arm.pattern {
            MatchPattern::Wildcard => covered = true,
            MatchPattern::Value(pattern) => {
                let node = program.expression_table.expression(pattern);
                let duplicate = patterns.iter().any(|previous| {
                    match (program.expression_table.expression(*previous), node) {
                        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => {
                            left == right
                        }
                        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left
                            .value_bignum()
                            .zip(right.value_bignum())
                            .is_some_and(|(left, right)| left == right),
                        _ => false,
                    }
                });
                if duplicate {
                    continue;
                }
                patterns.push(pattern);
                if let ExpressionNode::Boolean(value) = node {
                    booleans[usize::from(*value)] = true;
                    covered = booleans == [true, true];
                }
            }
        }
        let handle = arms.handle_at(ordinal as u32);
        selected.push((handle, arm));
        if covered {
            break;
        }
    }
    selected
}

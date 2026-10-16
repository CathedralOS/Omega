//! Explicit callee references in typed state bodies.
//!
//! This is structural inspection, not an execution/reachability analysis:
//! every expression arm is visited, while implicit operator dispatch, cleanup,
//! and provider selection are not inferred.

use crate::TypedTrees;
use crate::expression::ExpressionHandle;
use crate::state::State;
use symbols::SymbolHandle;

/// Reusable scratch for inspecting successive states without retaining a
/// previous program's references. Results contain explicit call and named
/// transition target symbols once each, in first-encounter order.
#[derive(Default)]
pub struct ExplicitStateCallees {
    visited: Vec<ExpressionHandle>,
    called: Vec<SymbolHandle>,
}

impl ExplicitStateCallees {
    /// Inspect one state's statement and expression graph. Each call clears
    /// the prior result and graph-visit guard while retaining their capacity.
    /// Zero handles contribute no callee. Declaration validity, target policy,
    /// and executable reach remain the consuming phase's responsibility.
    pub fn collect(&mut self, typed: &TypedTrees, state: &State) -> &[SymbolHandle] {
        self.called.clear();
        self.visited.clear();
        for statement in typed.statement_table.statements(state.statement_nodes) {
            collect_statement_callees(typed, statement, &mut self.visited, &mut self.called);
        }
        &self.called
    }
}

/// Retain each nonzero callee once, in first-encounter order.
fn note_callee(called: &mut Vec<symbols::SymbolHandle>, symbol: symbols::SymbolHandle) {
    if symbol.is_valid() && !called.contains(&symbol) {
        called.push(symbol);
    }
}

/// Inspect explicit calls in every statement position, including initializers,
/// guards, and transition arguments.
fn collect_statement_callees(
    typed: &TypedTrees,
    statement: &crate::statement::StatementNode,
    visited: &mut Vec<crate::expression::ExpressionHandle>,
    called: &mut Vec<symbols::SymbolHandle>,
) {
    use crate::statement::StatementNode;
    match statement {
        StatementNode::Assembly(_) => {} // Its instruction window is visited in source order.
        StatementNode::Call(call) => {
            note_callee(called, call.target_symbol);
            for argument in typed.expression_table.expression_handles(call.arguments) {
                collect_expression_callees(typed, *argument, visited, called);
            }
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_callees(typed, assignment.target, visited, called);
            collect_expression_callees(typed, assignment.value, visited, called);
        }
        StatementNode::Expression(expression) => {
            collect_expression_callees(typed, *expression, visited, called);
        }
        StatementNode::LocalData(local) => {
            collect_expression_callees(typed, local.initial_value, visited, called);
        }
        StatementNode::AssemblyFact(fact) => {
            for expression in typed.tables.proof_fact_expression_roots(fact.fact) {
                collect_expression_callees(typed, expression, visited, called);
            }
        }
        StatementNode::Transition(transition) => {
            if let crate::statement::TransitionGuardNode::When(guard) = &transition.guard {
                collect_expression_callees(typed, *guard, visited, called);
            }
            for target in transition.targets() {
                collect_transition_target_callees(typed, target, visited, called);
            }
        }
        StatementNode::RootBinding(_) => {}
    }
}

/// Named transitions reference a state directly; value targets and arguments
/// may contain expression calls.
fn collect_transition_target_callees(
    typed: &TypedTrees,
    target: crate::statement::TransitionTargetHandle,
    visited: &mut Vec<crate::expression::ExpressionHandle>,
    called: &mut Vec<symbols::SymbolHandle>,
) {
    use crate::statement::TransitionTargetNode;
    if !typed.statement_table.transition_target_is_valid(target) {
        return;
    }
    match typed.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            note_callee(called, path.symbol);
            for argument in typed.expression_table.expression_handles(*arguments) {
                collect_expression_callees(typed, *argument, visited, called);
            }
        }
        TransitionTargetNode::Value(expression) => {
            collect_expression_callees(typed, *expression, visited, called);
        }
        TransitionTargetNode::Terminal => {}
    }
}

/// Every callee inside one expression graph. `visited` is the graph's own
/// shared-subexpression guard: the table is a DAG, not a tree.
fn collect_expression_callees(
    typed: &TypedTrees,
    expression: crate::expression::ExpressionHandle,
    visited: &mut Vec<crate::expression::ExpressionHandle>,
    called: &mut Vec<symbols::SymbolHandle>,
) {
    use crate::expression::{ExpressionNode, MatchPattern};
    if !expression.is_valid() || visited.contains(&expression) {
        return;
    }
    visited.push(expression);
    match typed.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            note_callee(called, call.target_symbol);
            collect_expression_callees(typed, call.receiver, visited, called);
            for argument in typed.expression_table.expression_handles(call.arguments) {
                collect_expression_callees(typed, *argument, visited, called);
            }
        }
        ExpressionNode::Match(dispatch) => {
            collect_expression_callees(typed, dispatch.subject, visited, called);
            for arm in typed.expression_table.match_arms(dispatch.arms) {
                if let MatchPattern::Value(value) = arm.pattern {
                    collect_expression_callees(typed, value, visited, called);
                }
                collect_expression_callees(typed, arm.value, visited, called);
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in typed.expression_table.expression_handles(*values) {
                collect_expression_callees(typed, *value, visited, called);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            collect_expression_callees(typed, atomic.value, visited, called);
            collect_expression_callees(typed, atomic.result, visited, called);
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_callees(typed, binary.left, visited, called);
            collect_expression_callees(typed, binary.right, visited, called);
        }
        ExpressionNode::Cast(cast) => {
            collect_expression_callees(typed, cast.value, visited, called);
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_callees(typed, indexed.collection, visited, called);
            collect_expression_callees(typed, indexed.index, visited, called);
        }
        ExpressionNode::Member(member) => {
            collect_expression_callees(typed, member.receiver, visited, called);
        }
        ExpressionNode::Borrow(borrow) => {
            collect_expression_callees(typed, borrow.target, visited, called);
        }
        ExpressionNode::Range(range) => {
            collect_expression_callees(typed, range.start, visited, called);
            collect_expression_callees(typed, range.end, visited, called);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in typed.expression_table.struct_fields(literal.fields) {
                collect_expression_callees(typed, field.value, visited, called);
            }
        }
        ExpressionNode::Unary(unary) => {
            collect_expression_callees(typed, unary.operand, visited, called);
        }
        crate::literal_expression_pattern!() => {}
    }
}

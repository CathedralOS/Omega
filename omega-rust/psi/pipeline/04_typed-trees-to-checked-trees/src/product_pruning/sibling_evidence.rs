//! Evidence rows keyed by an expression rather than a machine survive pruning
//! as dead evidence (see the module contract). Provider-selected operator
//! uses are the one family a later settlement phase plans executions from, so
//! pruning a target sibling clears the selection on every operator use inside
//! its body: the row stays addressable, but reads as an unselected use that
//! no planner executes.

use std::collections::HashSet;

use checked_trees::{
    CheckFacts, CheckedBoundaryOperatorApplicationUseSite, CheckedProviderPlanCommitment,
};
use typed_trees::{
    TypedTrees,
    domain::ProofFact,
    expression::{ExpressionHandle, ExpressionNode},
    machine::Machine,
    proposition::PropositionApplication,
    signature::SignatureContract,
    statement::{StatementHandle, StatementNode, TransitionGuardNode, TransitionTargetNode},
};

/// The statement and expression handles a pruned machine's body owns.
#[derive(Default)]
pub(super) struct DeadEvidence {
    pub(super) statements: HashSet<StatementHandle>,
    pub(super) expressions: HashSet<ExpressionHandle>,
}

/// Every statement and expression handle reachable from `machine`'s states,
/// statements, signature contracts, and state contracts.
pub(super) fn collect_machine_evidence(
    program: &TypedTrees,
    machine: &Machine,
    dead: &mut DeadEvidence,
) {
    let expressions = &mut dead.expressions;
    for contract in program.machine_contracts(machine) {
        collect_contract(program, expressions, contract);
    }
    for state in program.machine_states(machine) {
        for contract in program.state_contracts(state) {
            collect_contract(program, expressions, contract);
        }
        for (handle, statement) in program
            .statement_table
            .iter_statements(state.statement_nodes)
        {
            dead.statements.insert(handle);
            collect_statement(program, expressions, statement);
        }
    }
}

/// Drop or unselect the operator evidence rows that live in dead bodies: a
/// boundary application demand at a dead site leaves, and an operator use at
/// a dead expression loses its provider selection so the dispatch planners
/// skip it as an unselected use.
pub(super) fn drop_dead_operator_evidence(facts: &mut CheckFacts, dead: &DeadEvidence) {
    facts
        .operators
        .boundary_applications
        .retain(|row| !site_is_dead(&row.site, dead));
    let dead = &dead.expressions;
    let cleared = CheckedProviderPlanCommitment::from_digest([0; 32]);
    facts.operators.uses.for_each_mut(|_, row| {
        if dead.contains(&row.expression) {
            row.provider_plan_report_fingerprint = 0;
            row.provider_plan_commitment = cleared;
        }
    });
    facts.operators.named_uses.for_each_mut(|_, row| {
        if dead.contains(&row.expression) {
            row.provider_plan_report_fingerprint = 0;
            row.provider_plan_commitment = cleared;
        }
    });
    facts
        .operators
        .named_requirement_uses
        .for_each_mut(|_, row| {
            if dead.contains(&row.expression) {
                row.provider_plan_report_fingerprint = 0;
                row.provider_plan_commitment = cleared;
            }
        });
}

fn site_is_dead(site: &CheckedBoundaryOperatorApplicationUseSite, dead: &DeadEvidence) -> bool {
    match site {
        CheckedBoundaryOperatorApplicationUseSite::Expression { expression, .. }
        | CheckedBoundaryOperatorApplicationUseSite::MatchEquality { expression, .. } => {
            dead.expressions.contains(expression)
        }
        CheckedBoundaryOperatorApplicationUseSite::Statement(statement) => {
            dead.statements.contains(statement)
        }
    }
}

fn collect_contract(
    program: &TypedTrees,
    expressions: &mut HashSet<ExpressionHandle>,
    contract: &SignatureContract,
) {
    for fact in program.tables.proof_facts.span_or_empty(contract.facts) {
        collect_proof_fact(program, expressions, fact);
    }
}

fn collect_proof_fact(
    program: &TypedTrees,
    expressions: &mut HashSet<ExpressionHandle>,
    fact: &ProofFact,
) {
    match fact {
        ProofFact::Expression(expression) => collect_expression(program, expressions, *expression),
        ProofFact::Membership(membership) => {
            collect_expression(program, expressions, membership.value);
        }
        ProofFact::Proposition(application) => {
            collect_proposition(program, expressions, application);
        }
    }
}

fn collect_proposition(
    program: &TypedTrees,
    expressions: &mut HashSet<ExpressionHandle>,
    application: &PropositionApplication,
) {
    for argument in program
        .expression_table
        .expression_handles(application.arguments)
    {
        collect_expression(program, expressions, *argument);
    }
}

fn collect_statement(
    program: &TypedTrees,
    expressions: &mut HashSet<ExpressionHandle>,
    statement: &StatementNode,
) {
    match statement {
        StatementNode::RootBinding(binding) => {
            collect_expression(program, expressions, binding.receiver);
            if binding.implementation_operand.is_valid() {
                collect_expression(program, expressions, binding.implementation_operand);
            }
        }
        StatementNode::AssemblyFact(fact) => {
            collect_expression(program, expressions, fact.expression);
        }
        StatementNode::Assignment(assignment) => {
            collect_expression(program, expressions, assignment.target);
            collect_expression(program, expressions, assignment.value);
        }
        StatementNode::Call(call) => {
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression(program, expressions, *argument);
            }
        }
        StatementNode::Expression(expression) => {
            collect_expression(program, expressions, *expression);
        }
        StatementNode::LocalData(local) => {
            collect_expression(program, expressions, local.initial_value);
        }
        StatementNode::Transition(transition) => {
            for target_handle in [transition.target, transition.continuation] {
                match program.statement_table.transition_target(target_handle) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in program.expression_table.expression_handles(*arguments) {
                            collect_expression(program, expressions, *argument);
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        collect_expression(program, expressions, *expression);
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
            if let TransitionGuardNode::When(guard) = transition.guard {
                collect_expression(program, expressions, guard);
            }
        }
    }
}

fn collect_expression(
    program: &TypedTrees,
    expressions: &mut HashSet<ExpressionHandle>,
    expression: ExpressionHandle,
) {
    if !expression.is_valid() || !expressions.insert(expression) {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(match_expression) => {
            collect_expression(program, expressions, match_expression.subject);
            for arm in program.expression_table.match_arms(match_expression.arms) {
                if let typed_trees::expression::MatchPattern::Value(pattern) = &arm.pattern {
                    collect_expression(program, expressions, *pattern);
                }
                collect_expression(program, expressions, arm.value);
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                collect_expression(program, expressions, *element);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            collect_expression(program, expressions, atomic.value);
            collect_expression(program, expressions, atomic.result);
        }
        ExpressionNode::Binary(binary) => {
            collect_expression(program, expressions, binary.left);
            collect_expression(program, expressions, binary.right);
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::ZeroValue(_) => {}
        ExpressionNode::Cast(cast) => {
            collect_expression(program, expressions, cast.value);
        }
        ExpressionNode::Call(call) => {
            collect_expression(program, expressions, call.receiver);
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression(program, expressions, *argument);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression(program, expressions, indexed.collection);
            collect_expression(program, expressions, indexed.index);
        }
        ExpressionNode::Member(member) => {
            collect_expression(program, expressions, member.receiver);
        }
        ExpressionNode::Borrow(borrow) => {
            collect_expression(program, expressions, borrow.target);
        }
        ExpressionNode::Range(range) => {
            collect_expression(program, expressions, range.start);
            collect_expression(program, expressions, range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                collect_expression(program, expressions, field.value);
            }
        }
        ExpressionNode::Unary(unary) => {
            collect_expression(program, expressions, unary.operand);
        }
    }
}

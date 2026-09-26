//! One traversal over a machine's call sites (statement calls, expression
//! calls, transition arguments, and signature/contract facts) shared by every
//! pass that re-points call targets after symbol assignment: closed-conformance
//! routing and target-sibling binding. Each pass supplies only its decision;
//! the reach over the body is defined once here.

use crate::symbol_resolved_trees::SymbolResolvedTrees;
use crate::symbol_resolved_trees::name::DiagnosticName;
use symbols::SymbolHandle;

/// One call the traversal offers to a decision.
pub(crate) struct CallSite<'a> {
    /// The target state symbol ordinary resolution assigned, or invalid.
    pub(crate) target_symbol: SymbolHandle,
    /// The spelled call target.
    pub(crate) target: &'a DiagnosticName,
    /// The call names a member of the machine's own subject: it has no
    /// receiver, a `self` receiver, or a target already among the subject's
    /// states.
    pub(crate) local_receiver: bool,
}

/// Visit every call site reachable from `machine`'s states and contracts and
/// replace the target of each one `decide` answers.
pub(crate) fn retarget_machine_calls(
    program: &mut SymbolResolvedTrees,
    machine: SymbolHandle,
    subject_states: &[SymbolHandle],
    decide: &mut dyn FnMut(&CallSite<'_>) -> Option<SymbolHandle>,
) {
    let Some((state_spans, contract_expressions)) = program
        .machines
        .iter()
        .find(|candidate| candidate.symbol == machine)
        .map(|machine| {
            let state_spans = program
                .machine_state_handles(machine.states)
                .iter()
                .map(|handle| program.machine_state(*handle).statement_nodes)
                .collect::<Vec<_>>();
            let mut contract_expressions = Vec::new();
            append_contract_expressions(program, machine.contracts, &mut contract_expressions);
            for state in program
                .machine_state_handles(machine.states)
                .iter()
                .map(|handle| program.machine_state(*handle))
            {
                append_contract_expressions(program, state.contracts, &mut contract_expressions);
            }
            (state_spans, contract_expressions)
        })
    else {
        return;
    };
    let mut visited = Vec::new();
    for expression in contract_expressions {
        retarget_expression(program, expression, subject_states, decide, &mut visited);
    }
    for statements in state_spans {
        retarget_statement_span(program, statements, subject_states, decide);
    }
}

fn append_contract_expressions(
    program: &SymbolResolvedTrees,
    contracts: arena::HandleSpan<crate::symbol_resolved_trees::signature::SignatureContract>,
    expressions: &mut Vec<crate::symbol_resolved_trees::expression::ExpressionHandle>,
) {
    for contract in program.signature_contracts(contracts) {
        for fact in program.proof_facts(contract.facts) {
            match fact {
                crate::symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                    expressions.push(*expression);
                }
                crate::symbol_resolved_trees::domain::ProofFact::Membership(membership) => {
                    expressions.push(membership.value);
                }
            }
        }
    }
}

fn retarget_statement_span(
    program: &mut SymbolResolvedTrees,
    statements: arena::HandleSpan<crate::symbol_resolved_trees::statement::StatementNode>,
    subject_states: &[SymbolHandle],
    decide: &mut dyn FnMut(&CallSite<'_>) -> Option<SymbolHandle>,
) {
    for offset in 0..statements.count() {
        let handle = arena::Handle::from_parts(
            statements.start().arena_index() + offset,
            statements.start().generation(),
        );
        let statement = program.tables.bodies.statements.statement(handle).clone();
        let mut expressions = Vec::new();
        match statement {
            crate::symbol_resolved_trees::statement::StatementNode::RootBinding(binding) => {
                expressions.push(binding.receiver);
                if binding.implementation_operand.is_valid() {
                    expressions.push(binding.implementation_operand);
                }
            }
            crate::symbol_resolved_trees::statement::StatementNode::AssemblyFact(fact) => {
                expressions.push(fact.expression);
            }
            crate::symbol_resolved_trees::statement::StatementNode::Assignment(assignment) => {
                expressions.extend([assignment.target, assignment.value]);
            }
            crate::symbol_resolved_trees::statement::StatementNode::Call(call) => {
                expressions.extend_from_slice(
                    program
                        .tables
                        .bodies
                        .statements
                        .expression_handles(call.arguments),
                );
                let site = CallSite {
                    target_symbol: call.target_symbol,
                    target: &call.target,
                    local_receiver: call.receiver.is_empty()
                        || call.receiver_starts_at_self
                        || subject_states.contains(&call.target_symbol),
                };
                if let Some(target) = decide(&site)
                    && let crate::symbol_resolved_trees::statement::StatementNode::Call(call) =
                        program.tables.bodies.statements.statement_mut(handle)
                {
                    call.target_symbol = target;
                }
            }
            crate::symbol_resolved_trees::statement::StatementNode::ProofOutputBindingStatement(
                package,
            ) => {
                expressions.push(package.call);
            }
            crate::symbol_resolved_trees::statement::StatementNode::Expression(expression) => {
                expressions.push(expression);
            }
            crate::symbol_resolved_trees::statement::StatementNode::LocalData(local) => {
                expressions.push(local.initial_value);
            }
            crate::symbol_resolved_trees::statement::StatementNode::Transition(transition) => {
                if let crate::symbol_resolved_trees::statement::TransitionGuardNode::When(guard) =
                    transition.guard
                {
                    expressions.push(guard);
                }
                append_transition_target_expressions(
                    &program.tables.bodies.statements,
                    transition.target,
                    &mut expressions,
                );
                if transition.continuation.is_valid() {
                    append_transition_target_expressions(
                        &program.tables.bodies.statements,
                        transition.continuation,
                        &mut expressions,
                    );
                }
            }
        }
        let mut visited = Vec::new();
        for expression in expressions {
            retarget_expression(program, expression, subject_states, decide, &mut visited);
        }
    }
}

fn append_transition_target_expressions(
    statements: &crate::symbol_resolved_trees::statement::StatementTable,
    target: crate::symbol_resolved_trees::statement::TransitionTargetHandle,
    expressions: &mut Vec<crate::symbol_resolved_trees::expression::ExpressionHandle>,
) {
    match statements.transition_target(target) {
        crate::symbol_resolved_trees::statement::TransitionTargetNode::Named {
            arguments, ..
        } => {
            expressions.extend_from_slice(statements.expression_handles(*arguments));
        }
        crate::symbol_resolved_trees::statement::TransitionTargetNode::Value(value) => {
            expressions.push(*value);
        }
        crate::symbol_resolved_trees::statement::TransitionTargetNode::SelfTarget
        | crate::symbol_resolved_trees::statement::TransitionTargetNode::Terminal => {}
    }
}

fn retarget_expression(
    program: &mut SymbolResolvedTrees,
    expression: crate::symbol_resolved_trees::expression::ExpressionHandle,
    subject_states: &[SymbolHandle],
    decide: &mut dyn FnMut(&CallSite<'_>) -> Option<SymbolHandle>,
    visited: &mut Vec<u32>,
) {
    if !expression.is_valid() || visited.contains(&expression.arena_index()) {
        return;
    }
    visited.push(expression.arena_index());
    let node = program
        .tables
        .bodies
        .expressions
        .expression(expression)
        .clone();
    let mut children = Vec::new();
    match node {
        crate::symbol_resolved_trees::expression::ExpressionNode::Match(dispatch) => {
            children.push(dispatch.subject);
            for arm in program.tables.bodies.expressions.match_arms(dispatch.arms) {
                if let crate::symbol_resolved_trees::expression::MatchPattern::Value(pattern) =
                    arm.pattern
                {
                    children.push(pattern);
                }
                children.push(arm.value);
            }
        }
        crate::symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            children
                .extend_from_slice(program.tables.bodies.expressions.expression_handles(values));
        }
        crate::symbol_resolved_trees::expression::ExpressionNode::Atomic(atomic) => {
            children.extend([atomic.value, atomic.result]);
        }
        crate::symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
            children.extend([binary.left, binary.right]);
        }
        crate::symbol_resolved_trees::expression::ExpressionNode::Cast(cast) => {
            children.push(cast.value);
        }
        crate::symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                children.push(call.receiver);
            }
            children.extend_from_slice(
                program
                    .tables
                    .bodies
                    .expressions
                    .expression_handles(call.arguments),
            );
            let site = CallSite {
                target_symbol: call.target_symbol,
                target: &call.target,
                local_receiver: !call.receiver.is_valid()
                    || expression_is_self(&program.tables.bodies.expressions, call.receiver)
                    || subject_states.contains(&call.target_symbol),
            };
            if let Some(target) = decide(&site)
                && let crate::symbol_resolved_trees::expression::ExpressionNode::Call(call) =
                    program.tables.bodies.expressions.expression_mut(expression)
            {
                call.target_symbol = target;
            }
        }
        crate::symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            children.extend([indexed.collection, indexed.index]);
        }
        crate::symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
            children.push(membership.value);
        }
        crate::symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            children.push(member.receiver);
        }
        crate::symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
            children.push(inner.target);
        }
        crate::symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
            children.extend([range.start, range.end]);
        }
        crate::symbol_resolved_trees::expression::ExpressionNode::StructLiteral(literal) => {
            children.extend(
                program
                    .tables
                    .bodies
                    .expressions
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            );
        }
        crate::symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
            children.push(unary.operand);
        }
        crate::symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
        | crate::symbol_resolved_trees::expression::ExpressionNode::Float(_)
        | crate::symbol_resolved_trees::expression::ExpressionNode::Integer(_)
        | crate::symbol_resolved_trees::expression::ExpressionNode::Name(_)
        | crate::symbol_resolved_trees::expression::ExpressionNode::String(_)
        | crate::symbol_resolved_trees::expression::ExpressionNode::ZeroValue(_) => {}
    }
    for child in children {
        retarget_expression(program, child, subject_states, decide, visited);
    }
}

fn expression_is_self(
    expressions: &crate::symbol_resolved_trees::expression::ExpressionTable,
    expression: crate::symbol_resolved_trees::expression::ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        crate::symbol_resolved_trees::expression::ExpressionNode::Name(path) => path.is_self_value,
        crate::symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
            expression_is_self(expressions, inner.target)
        }
        _ => false,
    }
}

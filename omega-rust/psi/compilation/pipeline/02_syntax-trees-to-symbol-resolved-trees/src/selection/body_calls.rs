//! One traversal over a machine's call sites (statement calls, expression
//! calls, transition callees and arguments, signature/contract facts, and `via` expressions)
//! shared by every pass that re-points call targets after symbol assignment: closed-conformance
//! routing and target-sibling binding. Each pass supplies only its decision;
//! the reach over the body is defined once here.

use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::name::DiagnosticName;
use symbols::JoinedPath;
use symbols::SymbolHandle;

/// One call the traversal offers to a decision.
pub(crate) struct CallSite<'a> {
    /// The target state symbol ordinary resolution assigned, or invalid.
    pub(crate) target_symbol: SymbolHandle,
    /// The spelled call target.
    pub(crate) target: &'a DiagnosticName,
    /// The resolved declaration the call is qualified by, or invalid.
    pub(crate) receiver_symbol: SymbolHandle,
    /// The call is unqualified: it names no receiver declaration.
    pub(crate) local_receiver: bool,
    /// Authored operands, excluding any receiver.
    pub(crate) argument_count: usize,
}

/// Visit every call site reachable from `machine`'s states, contracts and `via`
/// expressions and replace the target of each one `decide` answers.
pub(crate) fn retarget_machine_calls(
    program: &mut SymbolResolvedTrees,
    machine: SymbolHandle,
    decide: &mut dyn FnMut(&CallSite<'_>) -> Option<SymbolHandle>,
) {
    let Some((state_spans, declaration_expressions)) = program.machine(machine).map(|machine| {
        let state_spans = program
            .machine_state_handles(machine.states)
            .iter()
            .map(|handle| program.machine_state(*handle).statement_nodes)
            .collect::<Vec<_>>();
        let mut declaration_expressions = Vec::new();
        append_contract_expressions(program, machine.contracts, &mut declaration_expressions);
        // A bodyless external leaf still owns executable compile-time
        // expressions. Rebind their calls by the same exact-family rule
        // as body calls, before typing commits their producer identities.
        declaration_expressions.extend(
            program
                .machine_trait_conformances(machine.satisfies)
                .iter()
                .map(|conformance| conformance.via_expression),
        );
        for state in program
            .machine_state_handles(machine.states)
            .iter()
            .map(|handle| program.machine_state(*handle))
        {
            append_contract_expressions(program, state.contracts, &mut declaration_expressions);
        }
        (state_spans, declaration_expressions)
    }) else {
        return;
    };
    let mut visited = Vec::new();
    for expression in declaration_expressions {
        retarget_expression(program, expression, decide, &mut visited);
    }
    for statements in state_spans {
        retarget_statement_span(program, statements, decide);
    }
}

fn append_contract_expressions(
    program: &SymbolResolvedTrees,
    contracts: arena::HandleSpan<symbol_resolved_trees::signature::SignatureContract>,
    expressions: &mut Vec<symbol_resolved_trees::expression::ExpressionHandle>,
) {
    for contract in program.signature_contracts(contracts) {
        for fact in program.proof_facts(contract.facts) {
            match fact {
                symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                    expressions.push(*expression);
                }
                symbol_resolved_trees::domain::ProofFact::Membership(membership) => {
                    expressions.push(membership.value);
                }
            }
        }
    }
}

fn retarget_statement_span(
    program: &mut SymbolResolvedTrees,
    statements: arena::HandleSpan<symbol_resolved_trees::statement::StatementNode>,
    decide: &mut dyn FnMut(&CallSite<'_>) -> Option<SymbolHandle>,
) {
    for handle in statements.handles() {
        let statement = program.tables.bodies.statements.statement(handle).clone();
        let mut expressions = Vec::new();
        match statement {
            symbol_resolved_trees::statement::StatementNode::Assembly(handle) => {
                let graph = program.tables.declarations.assembly_graphs.get(handle);
                let blocks = program
                    .tables
                    .declarations
                    .assembly_blocks
                    .span_or_empty(graph.blocks)
                    .to_vec();
                for block in blocks {
                    expressions.extend_from_slice(
                        program
                            .tables
                            .declarations
                            .assembly_expressions
                            .span_or_empty(block.requires),
                    );
                    expressions.extend_from_slice(
                        program
                            .tables
                            .declarations
                            .assembly_expressions
                            .span_or_empty(block.ensures),
                    );
                    retarget_statement_span(program, block.statement_nodes, decide);
                }
            }
            symbol_resolved_trees::statement::StatementNode::RootBinding(binding) => {
                expressions.push(binding.receiver);
                if binding.implementation_operand.is_valid() {
                    expressions.push(binding.implementation_operand);
                }
            }
            symbol_resolved_trees::statement::StatementNode::AssemblyFact(fact) => {
                expressions.push(fact.expression);
            }
            symbol_resolved_trees::statement::StatementNode::Assignment(assignment) => {
                expressions.extend([assignment.target, assignment.value]);
            }
            symbol_resolved_trees::statement::StatementNode::Call(call) => {
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
                    receiver_symbol: call.receiver_symbol,
                    local_receiver: call.receiver.is_empty(),
                    argument_count: program
                        .tables
                        .bodies
                        .statements
                        .expression_handles(call.arguments)
                        .len(),
                };
                if let Some(target) = decide(&site)
                    && let symbol_resolved_trees::statement::StatementNode::Call(call) =
                        program.tables.bodies.statements.statement_mut(handle)
                {
                    call.target_symbol = target;
                }
            }
            symbol_resolved_trees::statement::StatementNode::ProofOutputBindingStatement(
                package,
            ) => {
                expressions.push(package.call);
            }
            symbol_resolved_trees::statement::StatementNode::Expression(expression) => {
                expressions.push(expression);
            }
            symbol_resolved_trees::statement::StatementNode::LocalData(local) => {
                expressions.push(local.initial_value);
            }
            symbol_resolved_trees::statement::StatementNode::Transition(transition) => {
                if let symbol_resolved_trees::statement::TransitionGuardNode::When(guard) =
                    transition.guard
                {
                    expressions.push(guard);
                }
                retarget_transition_target(program, transition.target, decide, &mut expressions);
                if transition.continuation.is_valid() {
                    retarget_transition_target(
                        program,
                        transition.continuation,
                        decide,
                        &mut expressions,
                    );
                }
            }
        }
        let mut visited = Vec::new();
        for expression in expressions {
            retarget_expression(program, expression, decide, &mut visited);
        }
    }
}

fn retarget_transition_target(
    program: &mut SymbolResolvedTrees,
    target: symbol_resolved_trees::statement::TransitionTargetHandle,
    decide: &mut dyn FnMut(&CallSite<'_>) -> Option<SymbolHandle>,
    expressions: &mut Vec<symbol_resolved_trees::expression::ExpressionHandle>,
) {
    let node = program
        .tables
        .bodies
        .statements
        .transition_target(target)
        .clone();
    match node {
        symbol_resolved_trees::statement::TransitionTargetNode::Named {
            path,
            arguments,
            source_span,
            ..
        } => {
            let members = program
                .tables
                .bodies
                .statements
                .name_path_members(path.members);
            let spelling = DiagnosticName::new(members.joined_path(), source_span);
            let site = CallSite {
                target_symbol: path.symbol,
                target: &spelling,
                receiver_symbol: path.head_symbol,
                local_receiver: members.len() == 1,
                argument_count: program
                    .tables
                    .bodies
                    .statements
                    .expression_handles(arguments)
                    .len(),
            };
            if let Some(replacement) = decide(&site)
                && let symbol_resolved_trees::statement::TransitionTargetNode::Named {
                    path: retained,
                    ..
                } = program
                    .tables
                    .bodies
                    .statements
                    .transition_target_mut(target)
            {
                if retained.head_symbol == retained.symbol {
                    retained.head_symbol = replacement;
                }
                retained.symbol = replacement;
            }
            expressions.extend_from_slice(
                program
                    .tables
                    .bodies
                    .statements
                    .expression_handles(arguments),
            );
        }
        symbol_resolved_trees::statement::TransitionTargetNode::Value(value) => {
            expressions.push(value);
        }
        symbol_resolved_trees::statement::TransitionTargetNode::Terminal => {}
    }
}

fn retarget_expression(
    program: &mut SymbolResolvedTrees,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
    decide: &mut dyn FnMut(&CallSite<'_>) -> Option<SymbolHandle>,
    visited: &mut Vec<symbol_resolved_trees::expression::ExpressionHandle>,
) {
    if !expression.is_valid() || visited.contains(&expression) {
        return;
    }
    visited.push(expression);
    let node = program
        .tables
        .bodies
        .expressions
        .expression(expression)
        .clone();
    let mut children = Vec::new();
    match node {
        symbol_resolved_trees::expression::ExpressionNode::Match(dispatch) => {
            children.push(dispatch.subject);
            for arm in program.tables.bodies.expressions.match_arms(dispatch.arms) {
                if let symbol_resolved_trees::expression::MatchPattern::Value(pattern) = arm.pattern
                {
                    children.push(pattern);
                }
                children.push(arm.value);
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            children
                .extend_from_slice(program.tables.bodies.expressions.expression_handles(values));
        }
        symbol_resolved_trees::expression::ExpressionNode::Atomic(atomic) => {
            children.extend([atomic.value, atomic.result]);
        }
        symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
            children.extend([binary.left, binary.right]);
        }
        symbol_resolved_trees::expression::ExpressionNode::Cast(cast) => {
            children.push(cast.value);
        }
        symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
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
            let receiver_symbol = match program.tables.bodies.expressions.expression(call.receiver)
            {
                symbol_resolved_trees::expression::ExpressionNode::Name(path)
                    if call.receiver.is_valid() =>
                {
                    path.symbol
                }
                _ => SymbolHandle::invalid(),
            };
            let site = CallSite {
                target_symbol: call.target_symbol,
                target: &call.target,
                receiver_symbol,
                local_receiver: !call.receiver.is_valid(),
                argument_count: program
                    .tables
                    .bodies
                    .expressions
                    .expression_handles(call.arguments)
                    .len(),
            };
            if let Some(target) = decide(&site)
                && let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
                    program.tables.bodies.expressions.expression_mut(expression)
            {
                call.target_symbol = target;
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            children.extend([indexed.collection, indexed.index]);
        }
        symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
            children.push(membership.value);
        }
        symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            children.push(member.receiver);
        }
        symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
            children.push(inner.target);
        }
        symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
            children.extend([range.start, range.end]);
        }
        symbol_resolved_trees::expression::ExpressionNode::StructLiteral(literal) => {
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
        symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
            children.push(unary.operand);
        }
        symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
        | symbol_resolved_trees::expression::ExpressionNode::Float(_)
        | symbol_resolved_trees::expression::ExpressionNode::UndeterminedNaN(_)
        | symbol_resolved_trees::expression::ExpressionNode::Integer(_)
        | symbol_resolved_trees::expression::ExpressionNode::Name(_)
        | symbol_resolved_trees::expression::ExpressionNode::String(_)
        | symbol_resolved_trees::expression::ExpressionNode::ZeroValue(_) => {}
    }
    for child in children {
        retarget_expression(program, child, decide, visited);
    }
}

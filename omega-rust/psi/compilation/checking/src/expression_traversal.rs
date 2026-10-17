//! Structural expression edges and occurrence collection shared by semantic checks.
//!
//! `children` exposes every immediate value edge in source order. It retains
//! invalid handles and repeated edges: destination custody uses their exact
//! positions, including an absent call receiver at position zero. Consumers own
//! pruning, live-handle validation, and visitation state.
//!
//! `collect_expression_nodes` is the recursive preorder used for occurrence
//! queries. It ignores zero handles and records each generational handle once.
//! This is structural traversal; it does not select executable match arms or
//! propagate destination types. `expression_has_inert_prefix` instead retains
//! executable occurrences and ordered effects before a particular captured read;
//! match-derived place and returned-window proofs share that evidence query.

use std::collections::HashSet;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, TableMatchExpression,
};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// Only already-inert value trees permit another observation of an argument.
/// Unknown operator and indexing behavior is deliberately not called pure here;
/// preparation has no checked effect judgment authorizing such a replay.
pub fn inert_value_tree(program: &dyn crate::CheckingInput, expression: ExpressionHandle) -> bool {
    inert_value_tree_with_calls(program, expression, |_| false)
}

/// Permit a call only when its caller supplies an exact no-write judgment.
/// Operators and atomic evaluation retain their conservative boundary; this
/// callback does not authorize unknown executable node families.
pub fn inert_value_tree_with_calls(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    mut allow_call: impl FnMut(ExpressionHandle) -> bool,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    let mut expressions = Vec::new();
    collect_expression_tree(program, expression, &mut expressions);
    expressions.iter().all(|expression| {
        if matches!(
            program.expression_table.expression(*expression),
            ExpressionNode::Call(_)
        ) {
            return allow_call(*expression);
        }
        matches!(
            program.expression_table.expression(*expression),
            ExpressionNode::Name(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::String(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::Borrow(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Match(_)
        )
    })
}

/// This expression-only query has no flow-frame judgment for a completed
/// invocation. Decline match substitution after any call or atomic event,
/// regardless of whether the subject is a field, scalar or indexed place.
/// Walk executable edges in source order, including earlier sibling operands;
/// the enclosing call executes only after its arguments. Later effects cannot
/// change a coordinate already captured. Repeated handles remain occurrences,
/// so one earlier visit cannot conceal a later read following an effect.
/// The checked route binds selected authored operators to ordinary Call nodes
/// during 03b preparation, including declared indexing. This query consumes
/// that prepared input; operator syntax is not an independent purity claim.
pub fn expression_has_inert_prefix(
    program: &dyn crate::CheckingInput,
    patterns: &[ExpressionHandle],
    root: ExpressionHandle,
    endpoint: ExpressionHandle,
) -> bool {
    struct Prefix {
        effect_seen: bool,
        found: bool,
        stable: bool,
    }
    fn visit(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        endpoint: ExpressionHandle,
        depth: usize,
        prefix: &mut Prefix,
    ) -> bool {
        if !expression.is_valid() {
            return true;
        }
        if depth >= 64 {
            return false;
        }
        let node = program.expression_table.expression(expression);
        let mut complete = true;
        executable_children(program, node, |child| {
            complete &= visit(program, child, endpoint, depth + 1, prefix);
        });
        if matches!(node, ExpressionNode::Call(_) | ExpressionNode::Atomic(_)) {
            prefix.effect_seen = true;
        }
        if expression == endpoint {
            prefix.found = true;
            prefix.stable &= !prefix.effect_seen;
        }
        complete
    }
    let mut prefix = Prefix {
        effect_seen: false,
        found: false,
        stable: true,
    };
    let mut complete = true;
    for pattern in patterns {
        complete &= visit(program, *pattern, endpoint, 0, &mut prefix);
    }
    // A coordinate evaluated by a pattern precedes this arm's equality.
    // Even a repeated handle in the selected body cannot grant that earlier
    // occurrence an observation established only after pattern evaluation.
    if prefix.found {
        return false;
    }
    complete &= visit(program, root, endpoint, 0, &mut prefix);
    complete && prefix.found && prefix.stable
}

pub fn children(
    program: &dyn crate::CheckingInput,
    node: &ExpressionNode,
    mut child: impl FnMut(ExpressionHandle),
) {
    match node {
        ExpressionNode::Match(dispatch) => {
            for expression in match_children(program, *dispatch) {
                child(expression);
            }
        }
        ExpressionNode::Binary(binary) => {
            child(binary.left);
            child(binary.right);
        }
        ExpressionNode::Unary(unary) => child(unary.operand),
        ExpressionNode::Borrow(borrow) => child(borrow.target),
        ExpressionNode::Cast(cast) => child(cast.value),
        ExpressionNode::Atomic(atomic) => {
            child(atomic.value);
            child(atomic.result);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                child(*element);
            }
        }
        ExpressionNode::Call(call) => {
            child(call.receiver);
            for argument in program.expression_table.expression_handles(call.arguments) {
                child(*argument);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            child(indexed.collection);
            child(indexed.index);
        }
        ExpressionNode::Member(member) => child(member.receiver),
        ExpressionNode::Range(range) => {
            child(range.start);
            child(range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                child(field.value);
            }
        }
        typed_trees::literal_expression_pattern!() => {}
    }
}

/// Immediate executable value edges in source order. Atomic's result edge
/// carries structural evidence; only its value is evaluated. Every other
/// expression uses the complete structural roster, including record fields,
/// range endpoints and value patterns. Callers retain origin and visitation.
pub(crate) fn executable_children(
    program: &dyn crate::CheckingInput,
    node: &ExpressionNode,
    mut child: impl FnMut(ExpressionHandle),
) {
    if let ExpressionNode::Atomic(atomic) = node {
        child(atomic.value);
    } else {
        children(program, node, child);
    }
}

/// Preserve occurrence identity for checks that consume selected declarations
/// or result types. This is the same postorder as the node-only convenience API.
pub fn visit_post_order(
    program: &dyn crate::CheckingInput,
    root: ExpressionHandle,
    visited: &mut Vec<ExpressionHandle>,
    mut arm: impl FnMut(ExpressionHandle, &ExpressionNode),
) {
    fn inner(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        visited: &mut Vec<ExpressionHandle>,
        arm: &mut dyn FnMut(ExpressionHandle, &ExpressionNode),
    ) {
        if !expression.is_valid() || visited.contains(&expression) {
            return;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        children(program, node, |child| inner(program, child, visited, arm));
        arm(expression, node);
    }
    inner(program, root, visited, &mut arm);
}

/// Preorder worklist walk over every reachable expression edge, each handle
/// once. `arm` observes a node before its children walk; returning true ends
/// the walk early and propagates the flag.
pub fn visit_pre_order(
    program: &dyn crate::CheckingInput,
    pending: Vec<ExpressionHandle>,
    mut arm: impl FnMut(ExpressionHandle, &ExpressionNode) -> bool,
) -> bool {
    let mut pending = pending;
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression)
            || visited.contains(&expression)
        {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        if arm(expression, node) {
            return true;
        }
        children(program, node, |child| pending.push(child));
    }
    false
}

/// Climb `Member` (non-variant) and `Indexed` projections to the root
/// expression, returning each projected node outermost-first plus the root.
/// `None` on a cycle, an invalid cursor, or more than `max_depth` edges.
pub fn projection_climb(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    max_depth: usize,
) -> Option<(Vec<ExpressionHandle>, ExpressionHandle)> {
    let mut projections = Vec::new();
    let mut cursor = expression;
    loop {
        if !program.expression_table.expression_is_valid(cursor)
            || projections.contains(&cursor)
            || projections.len() >= max_depth
        {
            return None;
        }
        cursor = match program.expression_table.expression(cursor) {
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                projections.push(cursor);
                member.receiver
            }
            ExpressionNode::Indexed(indexed) => {
                projections.push(cursor);
                indexed.collection
            }
            _ => break,
        };
    }
    Some((projections, cursor))
}

/// The symbol a single-member `Name` argument resolves, through a shared
/// borrow: `&field` and `field` name the same symbol.
pub fn single_member_argument_symbol(
    program: &dyn crate::CheckingInput,
    argument: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    let named = match program.expression_table.expression(argument) {
        ExpressionNode::Borrow(borrow)
            if borrow.access == language_core::ReferenceAccess::Shared =>
        {
            borrow.target
        }
        _ => argument,
    };
    let path = program.expression_table.name_path_node(named)?;
    (path.symbol.is_valid()
        && program
            .expression_table
            .name_path_members(path.members)
            .len()
            == 1)
        .then_some(path.symbol)
}

/// The last member of the `Name` path `value` names, when it names one.
pub fn name_path_last_member<'a>(
    program: &'a dyn crate::CheckingInput,
    value: ExpressionHandle,
) -> Option<&'a str> {
    let path = program.expression_table.name_path_node(value)?;
    program
        .expression_table
        .name_path_members(path.members)
        .last()
        .map(|member| member.as_str())
}

/// Strip `Atomic` wrappers: an atomically-evaluated step is still a step.
pub fn unwrapped(
    program: &dyn crate::CheckingInput,
    mut expression: ExpressionHandle,
) -> ExpressionHandle {
    while let ExpressionNode::Atomic(atomic) = program.expression_table.expression(expression) {
        expression = atomic.value;
    }
    expression
}

/// Push every value edge in reverse source order so a stack `pop` yields
/// source-order traversal.
pub fn push_children_reversed(
    program: &dyn crate::CheckingInput,
    node: &ExpressionNode,
    pending: &mut Vec<ExpressionHandle>,
) {
    let mut edges = Vec::new();
    children(program, node, |child| edges.push(child));
    pending.extend(edges.into_iter().rev());
}

/// Structural readers visit every source edge, not just executable arms.
pub(crate) fn match_children(
    program: &dyn crate::CheckingInput,
    dispatch: TableMatchExpression,
) -> impl Iterator<Item = ExpressionHandle> + '_ {
    std::iter::once(dispatch.subject).chain(
        program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .flat_map(|arm| {
                let pattern = match arm.pattern {
                    MatchPattern::Value(pattern) => Some(pattern),
                    MatchPattern::Wildcard => None,
                };
                pattern.into_iter().chain(std::iter::once(arm.value))
            }),
    )
}

/// Append first visits in recursive preorder, sharing the caller's occurrence roster.
pub(crate) fn collect_expression_nodes<Hashing: std::hash::BuildHasher>(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    nodes: &mut Vec<ExpressionHandle>,
    seen: &mut HashSet<ExpressionHandle, Hashing>,
) {
    if !expression.is_valid() || !seen.insert(expression) {
        return;
    }
    nodes.push(expression);
    children(
        program,
        program.expression_table.expression(expression),
        |child| {
            collect_expression_nodes(program, child, nodes, seen);
        },
    );
}

/// The single-segment `Name` path's resolved symbol — `None` for member
/// chains, projections, or an unresolved path.
pub fn single_segment_name_symbol(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    let path = program.expression_table.name_path_node(expression)?;
    is_single_segment_name_path(program, path).then_some(path.symbol)
}

/// `path` resolves to its own root symbol through exactly one segment — the
/// bare `name` reference shape, not a member path or an unresolved name.
pub fn is_single_segment_name_path(
    program: &dyn crate::CheckingInput,
    path: &typed_trees::expression::TableNamePath,
) -> bool {
    program.expression_table.name_path_is_single_member(path)
}

/// Append structural expression handles in preorder, once per existing roster.
/// Atomic result edges are structural evidence too; executable occurrence
/// traversal deliberately has a different edge contract.
pub fn collect_expression_tree(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    handles: &mut Vec<ExpressionHandle>,
) {
    if !expression.is_valid() || handles.contains(&expression) {
        return;
    }
    handles.push(expression);
    children(
        program,
        program.expression_table.expression(expression),
        |child| {
            collect_expression_tree(program, child, handles);
        },
    );
}

/// Collect the structural roots rewritten by typed preparation. An assembly
/// fact contributes evidence; a root binding contributes its implementation
/// operand, not the receiver evaluated by the executable occurrence visitor.
pub fn collect_statement_expression_trees(
    program: &dyn crate::CheckingInput,
    statement: &typed_trees::statement::StatementNode,
    handles: &mut Vec<ExpressionHandle>,
) {
    match statement {
        typed_trees::statement::StatementNode::Assembly(_) => {}
        typed_trees::statement::StatementNode::RootBinding(binding) => {
            if binding.implementation_operand.is_valid() {
                collect_expression_tree(program, binding.implementation_operand, handles);
            }
        }
        typed_trees::statement::StatementNode::AssemblyFact(fact) => {
            for expression in program.tables.proof_fact_expression_roots(fact.fact) {
                collect_expression_tree(program, expression, handles);
            }
        }
        typed_trees::statement::StatementNode::Assignment(assignment) => {
            collect_expression_tree(program, assignment.target, handles);
            collect_expression_tree(program, assignment.value, handles);
        }
        typed_trees::statement::StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression_tree(program, *argument, handles);
            }
        }
        typed_trees::statement::StatementNode::Expression(expression) => {
            collect_expression_tree(program, *expression, handles)
        }
        typed_trees::statement::StatementNode::LocalData(local) => {
            collect_expression_tree(program, local.initial_value, handles)
        }
        typed_trees::statement::StatementNode::Transition(transition) => {
            if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_tree(program, guard, handles);
            }
            collect_transition_target_expression_trees(program, transition.target, handles);
            collect_transition_target_expression_trees(program, transition.continuation, handles);
        }
    }
}

// Follow a declaring type without expanding nominal data into another scope.
pub fn collect_type_expressions(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    visited: &mut Vec<TypeReferenceHandle>,
    expressions: &mut Vec<ExpressionHandle>,
) {
    if !reference.is_valid() || visited.contains(&reference) {
        return;
    }
    visited.push(reference);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_expressions(program, *referee, visited, expressions);
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            collect_type_expressions(program, *element_type, visited, expressions);
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_type_expressions(program, *argument, visited, expressions);
            }
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_expressions(program, *base_type, visited, expressions);
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } => {
                        collect_expression_tree(program, *minimum, expressions);
                        collect_expression_tree(program, *maximum, expressions);
                    }
                    TypeConstraintNode::Domain(domain) => {
                        for argument in &domain.arguments {
                            collect_type_expressions(program, *argument, visited, expressions);
                        }
                    }
                    TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReferenceNode::ConstExpression(expression) => {
            collect_expression_tree(program, *expression, expressions);
        }
        TypeReferenceNode::Named { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => {}
    }
}

pub fn collect_transition_target_expression_trees(
    program: &dyn crate::CheckingInput,
    target: typed_trees::statement::TransitionTargetHandle,
    handles: &mut Vec<ExpressionHandle>,
) {
    if !target.is_valid() {
        return;
    }
    match program.statement_table.transition_target(target) {
        typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                collect_expression_tree(program, *argument, handles);
            }
        }
        typed_trees::statement::TransitionTargetNode::Value(expression) => {
            collect_expression_tree(program, *expression, handles)
        }
        typed_trees::statement::TransitionTargetNode::Terminal => {}
    }
}

/// Append the roots a proof fact carries without traversing their children.
/// Copying and occurrence checking choose their own structural/runtime walks.
pub fn append_fact_expression_roots(
    program: &dyn crate::CheckingInput,
    fact: &typed_trees::domain::ProofFact,
    expressions: &mut Vec<ExpressionHandle>,
) {
    match fact {
        typed_trees::domain::ProofFact::Expression(expression) => expressions.push(*expression),
        typed_trees::domain::ProofFact::Membership(membership) => {
            expressions.push(membership.value)
        }
        typed_trees::domain::ProofFact::Proposition(application) => expressions.extend(
            program
                .expression_table
                .expression_handles(application.arguments)
                .iter()
                .copied(),
        ),
    }
}

/// Append structural expression trees from a contract's proof-fact span.
pub fn collect_contract_facts(
    program: &dyn crate::CheckingInput,
    facts: arena::HandleSpan<typed_trees::domain::ProofFact>,
    handles: &mut Vec<ExpressionHandle>,
) {
    for fact in program.proof_facts.span_or_empty(facts) {
        match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                collect_expression_tree(program, *expression, handles)
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                collect_expression_tree(program, membership.value, handles)
            }
            typed_trees::domain::ProofFact::Proposition(application) => {
                for argument in program
                    .expression_table
                    .expression_handles(application.arguments)
                {
                    collect_expression_tree(program, *argument, handles);
                }
            }
        }
    }
}

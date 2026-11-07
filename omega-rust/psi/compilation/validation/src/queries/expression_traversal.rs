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

use crate::frozen_program::expression_table;
use language_core::ReferenceAccess;
use std::collections::HashSet;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMatchExpression};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// Leaves and structural construction are inert only when every embedded
/// operation has the caller's exact meaning/effect judgment. In particular,
/// call-free operator syntax is not evidence of an intrinsic operation.
pub fn inert_value_tree_with_operations(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    mut allow_operation: impl FnMut(ExpressionHandle) -> bool,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    let mut expressions = Vec::new();
    collect_expression_tree(program, expression, &mut expressions);
    expressions.iter().all(|expression| {
        matches!(
            expression_table(program).expression(*expression),
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
        ) || allow_operation(*expression)
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
/// during Psi 04 preparation, including declared indexing. This query consumes
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
        let node = expression_table(program).expression(expression);
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

/// Completed sibling subtrees before one operation's operand evaluation.
/// The enclosing operation and later siblings have not executed. An endpoint
/// inside a match arm sees the subject and preceding patterns, not sibling arm
/// bodies. A completed selection before the endpoint remains a whole subtree:
/// its consumer still unions every possible effect. Repeated endpoint handles
/// decline: a static handle cannot select between different occurrences.
pub fn expressions_before_occurrence(
    program: &dyn crate::CheckingInput,
    roots: &[ExpressionHandle],
    endpoint: ExpressionHandle,
) -> Option<Vec<ExpressionHandle>> {
    fn visit(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        endpoint: ExpressionHandle,
        depth: usize,
        occurrences: &mut usize,
        prefix: &mut Vec<ExpressionHandle>,
        captured: &mut Vec<ExpressionHandle>,
    ) -> Option<()> {
        if !expression.is_valid() {
            return Some(());
        }
        if depth >= 64 {
            return None;
        }
        if expression == endpoint {
            *occurrences += 1;
            if *occurrences == 1 {
                captured.clone_from(prefix);
            }
            return Some(());
        }
        let start = prefix.len();
        let node = expression_table(program).expression(expression);
        if let ExpressionNode::Match(dispatch) = node {
            visit(
                program,
                dispatch.subject,
                endpoint,
                depth + 1,
                occurrences,
                prefix,
                captured,
            )?;
            for arm in expression_table(program).match_arms(dispatch.arms) {
                for pattern in expression_table(program).match_pattern_operands(arm.pattern) {
                    visit(
                        program,
                        pattern,
                        endpoint,
                        depth + 1,
                        occurrences,
                        prefix,
                        captured,
                    )?;
                }
                let patterns_end = prefix.len();
                visit(
                    program,
                    arm.value,
                    endpoint,
                    depth + 1,
                    occurrences,
                    prefix,
                    captured,
                )?;
                // Only a failed pattern reaches the next arm. The selected
                // body cannot donate effects to that mutually exclusive path.
                prefix.truncate(patterns_end);
            }
        } else {
            let mut completed = Some(());
            executable_children(program, node, |child| {
                if visit(
                    program,
                    child,
                    endpoint,
                    depth + 1,
                    occurrences,
                    prefix,
                    captured,
                )
                .is_none()
                {
                    completed = None;
                }
            });
            completed?;
        }
        prefix.truncate(start);
        prefix.push(expression);
        Some(())
    }
    let mut occurrences = 0;
    let mut prefix = Vec::new();
    let mut captured = Vec::new();
    for root in roots {
        visit(
            program,
            *root,
            endpoint,
            0,
            &mut occurrences,
            &mut prefix,
            &mut captured,
        )?;
    }
    (occurrences == 1).then_some(captured)
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
            if let Some(once) =
                crate::value_custody::atomic_operations::atomic_once_operands(program, atomic)
            {
                child(once.place);
                for operand in once.operands {
                    child(operand);
                }
            } else {
                child(atomic.value);
                child(atomic.result);
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in expression_table(program).expression_handles(*elements) {
                child(*element);
            }
        }
        ExpressionNode::Call(call) => {
            child(call.receiver);
            for argument in expression_table(program).expression_handles(call.arguments) {
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
            for field in expression_table(program).struct_fields(literal.fields) {
                child(field.value);
            }
        }
        ExpressionNode::TypeExpression(_)
        | ExpressionNode::SavedSubject(_)
        | typed_trees::literal_expression_pattern!() => {}
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
        if let Some(once) =
            crate::value_custody::atomic_operations::atomic_once_operands(program, atomic)
        {
            child(once.place);
            for operand in once.operands {
                child(operand);
            }
        } else {
            child(atomic.value);
        }
    } else {
        children(program, node, child);
    }
}

/// `TypedTrees::subtree_any` over executable edges: a compare-exchange-once
/// carrier evaluates only its place and operands, never its retained call node.
pub fn executable_subtree_any(
    program: &dyn crate::CheckingInput,
    root: ExpressionHandle,
    edges: typed_trees::SubtreeEdges,
    mut visit: impl FnMut(ExpressionHandle, &ExpressionNode) -> typed_trees::SubtreeVisit,
) -> bool {
    program.subtree_any(root, edges, |expression, node| {
        match (visit(expression, node), node) {
            (typed_trees::SubtreeVisit::Descend, ExpressionNode::Atomic(atomic)) => {
                match crate::value_custody::atomic_operations::atomic_once_operands(program, atomic)
                {
                    Some(once) => typed_trees::SubtreeVisit::InsteadEach(
                        std::iter::once(once.place).chain(once.operands).collect(),
                    ),
                    None => typed_trees::SubtreeVisit::Descend,
                }
            }
            (visit, _) => visit,
        }
    })
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
        let node = expression_table(program).expression(expression);
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
        if !expression_table(program).expression_is_valid(expression)
            || visited.contains(&expression)
        {
            continue;
        }
        visited.push(expression);
        let node = expression_table(program).expression(expression);
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
        if !expression_table(program).expression_is_valid(cursor)
            || projections.contains(&cursor)
            || projections.len() >= max_depth
        {
            return None;
        }
        cursor = match expression_table(program).expression(cursor) {
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
    let named = match expression_table(program).expression(argument) {
        ExpressionNode::Borrow(borrow) if borrow.access == ReferenceAccess::Shared => borrow.target,
        _ => argument,
    };
    let path = expression_table(program).name_path_node(named)?;
    (path.symbol.is_valid()
        && expression_table(program)
            .name_path_members(path.members)
            .len()
            == 1)
        .then_some(path.symbol)
}

/// The last member of the `Name` path `value` names, when it names one.
pub fn name_path_last_member(
    program: &dyn crate::CheckingInput,
    value: ExpressionHandle,
) -> Option<&str> {
    let path = expression_table(program).name_path_node(value)?;
    expression_table(program)
        .name_path_members(path.members)
        .last()
        .map(|member| member.as_str())
}

/// Strip `Atomic` wrappers: an atomically-evaluated step is still a step.
pub fn unwrapped(
    program: &dyn crate::CheckingInput,
    mut expression: ExpressionHandle,
) -> ExpressionHandle {
    while let ExpressionNode::Atomic(atomic) = expression_table(program).expression(expression) {
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
        expression_table(program)
            .match_arms(dispatch.arms)
            .iter()
            .flat_map(|arm| {
                expression_table(program)
                    .match_pattern_operands(arm.pattern)
                    .chain(std::iter::once(arm.value))
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
        expression_table(program).expression(expression),
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
    let path = expression_table(program).name_path_node(expression)?;
    is_single_segment_name_path(program, path).then_some(path.symbol)
}

/// `path` resolves to its own root symbol through exactly one segment — the
/// bare `name` reference shape, not a member path or an unresolved name.
pub fn is_single_segment_name_path(
    program: &dyn crate::CheckingInput,
    path: &typed_trees::expression::TableNamePath,
) -> bool {
    expression_table(program).name_path_is_single_member(path)
}

/// Report the source obligations that leave a dispatch subject without storage.
///
/// Name scope, case-membership identity, and float-format combination are the
/// same checks ordinary validation runs later. A subject whose result type is
/// missing only because one of those obligations failed must surface that
/// obligation at the completion fence. This does not invent a type or admit
/// the expression.
pub fn record_unresolved_subject_source_obligations(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    prior_statements: &[typed_trees::statement::StatementNode],
    symbols: &crate::TopLevelSymbols<'_>,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let mut names = Vec::new();
    collect_expression_tree(program, expression, &mut names);
    for name in names {
        crate::machine_calls::calls::report_unresolved_name_use(
            program,
            machine,
            state,
            prior_statements,
            symbols,
            name,
            diagnostics,
        );
    }
    if let Err(mut construction) =
        crate::validate_value_construction(program, machine, state, expression, &mut Vec::new())
    {
        diagnostics.append(&mut construction);
    }
    crate::value_custody::expression_types::validate_float_expression_formats(
        program,
        state.symbol,
        expression,
        &|operand| crate::expression_result_type_reference(program, machine, state, operand),
        diagnostics,
    );
}

/// Append structural expression handles in preorder, once per existing list.
/// Atomic result edges are structural evidence too; executable occurrence
/// traversal deliberately has a different edge contract.
pub fn collect_expression_tree(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    handles: &mut Vec<ExpressionHandle>,
) {
    collect_expression_trees(program, std::iter::once(expression), handles);
}

/// Append several structural roots to one list. Reuse its visited set across
/// roots instead of copying the accumulated list before every root. Existing
/// entries, including duplicates, remain in order; append each new node once
/// in preorder.
/// The program stays borrowed for this collection, so visitation cannot outlive
/// the input or conceal a later mutation.
pub fn collect_expression_trees(
    program: &dyn crate::CheckingInput,
    roots: impl IntoIterator<Item = ExpressionHandle>,
    handles: &mut Vec<ExpressionHandle>,
) {
    let mut roots = roots.into_iter();
    let Some(first) = roots.next() else {
        return;
    };
    let mut seen: HashSet<_, symbols::BuildSymbolHasher> = handles.iter().copied().collect();
    collect_expression_nodes(program, first, handles, &mut seen);
    for expression in roots {
        collect_expression_nodes(program, expression, handles, &mut seen);
    }
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
        typed_trees::statement::StatementNode::AssemblyFact(fact) => {
            collect_expression_trees(
                program,
                program.tables.proof_fact_expression_roots(fact.fact),
                handles,
            );
        }
        typed_trees::statement::StatementNode::Assignment(assignment) => {
            collect_expression_trees(program, [assignment.target, assignment.value], handles);
        }
        typed_trees::statement::StatementNode::Call(call) => {
            collect_expression_trees(
                program,
                program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied(),
                handles,
            );
        }
        typed_trees::statement::StatementNode::Expression(expression) => {
            collect_expression_tree(program, *expression, handles)
        }
        typed_trees::statement::StatementNode::LocalData(local) => {
            collect_expression_tree(program, local.initial_value, handles)
        }
        typed_trees::statement::StatementNode::Transition(transition) => {
            let guard = match transition.guard {
                typed_trees::statement::TransitionGuardNode::When(guard) => Some(guard),
                _ => None,
            };
            collect_expression_trees(
                program,
                guard
                    .into_iter()
                    .chain(transition_target_expression_roots(
                        program,
                        transition.target,
                    ))
                    .chain(transition_target_expression_roots(
                        program,
                        transition.continuation,
                    )),
                handles,
            );
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
    collect_expression_trees(
        program,
        transition_target_expression_roots(program, target),
        handles,
    );
}

fn transition_target_expression_roots(
    program: &dyn crate::CheckingInput,
    target: typed_trees::statement::TransitionTargetHandle,
) -> impl Iterator<Item = ExpressionHandle> + '_ {
    let roots = if !target.is_valid() {
        &[]
    } else {
        match program.statement_table.transition_target(target) {
            typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
                program.statement_table.expression_handles(*arguments)
            }
            typed_trees::statement::TransitionTargetNode::Value(expression) => {
                std::slice::from_ref(expression)
            }
            typed_trees::statement::TransitionTargetNode::Terminal => &[],
        }
    };
    roots.iter().copied()
}

/// Append the roots a proof fact carries without traversing their children.
/// Copying and occurrence checking choose their own structural/runtime walks.
pub fn append_fact_expression_roots(
    program: &dyn crate::CheckingInput,
    fact: &ProofFact,
    expressions: &mut Vec<ExpressionHandle>,
) {
    match fact {
        ProofFact::Expression(expression) => expressions.push(*expression),
        ProofFact::Membership(membership) => expressions.push(membership.value),
        ProofFact::Proposition(application) => expressions.extend(
            expression_table(program)
                .expression_handles(application.arguments)
                .iter()
                .copied(),
        ),
    }
}

/// Append structural expression trees from a contract's proof-fact span.
pub fn collect_contract_facts(
    program: &dyn crate::CheckingInput,
    facts: arena::HandleSpan<ProofFact>,
    handles: &mut Vec<ExpressionHandle>,
) {
    collect_proof_fact_expression_trees(program, program.proof_facts.span_or_empty(facts), handles);
}

/// Collect one caller-owned group of proof facts, including prepared facts that
/// are not attached to a declaration. Different declaration scopes supply
/// separate lists; sharing traversal state does not join their meanings.
pub fn collect_proof_fact_expression_trees<'program>(
    program: &'program dyn crate::CheckingInput,
    facts: impl IntoIterator<Item = &'program ProofFact>,
    handles: &mut Vec<ExpressionHandle>,
) {
    collect_expression_trees(
        program,
        facts.into_iter().flat_map(|fact| {
            let roots = match fact {
                ProofFact::Expression(expression) => std::slice::from_ref(expression),
                ProofFact::Membership(membership) => std::slice::from_ref(&membership.value),
                ProofFact::Proposition(application) => {
                    expression_table(program).expression_handles(application.arguments)
                }
            };
            roots.iter().copied()
        }),
        handles,
    );
}

#[cfg(test)]
mod collection_tests {
    use super::*;
    use typed_trees::{
        TypedTrees, domain::ProofMembershipFact, proposition::PropositionApplication,
    };

    #[test]
    fn batched_roots_keep_seed_order_full_generations_and_shared_children() {
        let mut program = TypedTrees::default();
        let first = program
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        let shared = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let last = program
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        let left_children = program
            .expression_table
            .insert_expression_handles([first, shared]);
        let left = program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(left_children));
        let right_children = program
            .expression_table
            .insert_expression_handles([shared, last]);
        let right = program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(right_children));
        let other_generation =
            ExpressionHandle::from_parts(right.arena_index(), right.generation().wrapping_add(1));
        let seed = vec![first, first, other_generation];
        let mut nodes = seed.clone();
        collect_expression_trees(
            &program,
            [left, right, left, ExpressionHandle::invalid()],
            &mut nodes,
        );
        let mut expected = seed;
        expected.extend([left, shared, right, last]);
        assert_eq!(nodes, expected);
    }

    #[test]
    fn proof_fact_roots_keep_order_and_independent_lists() {
        let mut program = TypedTrees::default();
        let first = program
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        let second = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let third = program
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        let arguments = program
            .expression_table
            .insert_expression_handles([third, first]);
        let facts = [
            ProofFact::Expression(first),
            ProofFact::Membership(ProofMembershipFact {
                value: second,
                ..Default::default()
            }),
            ProofFact::Proposition(PropositionApplication {
                proposition: symbols::SymbolHandle::invalid(),
                name: typed_trees::name::Identifier::from("Predicate"),
                binder_arguments: Vec::new().into(),
                arguments,
            }),
        ];
        let mut first_scope = Vec::new();
        let mut second_scope = Vec::new();
        collect_proof_fact_expression_trees(&program, &facts, &mut first_scope);
        collect_proof_fact_expression_trees(&program, &facts, &mut second_scope);
        assert_eq!(first_scope, [first, second, third]);
        assert_eq!(second_scope, first_scope);
    }
}

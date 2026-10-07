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
//! propagate destination types.

use std::collections::HashSet;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, TableMatchExpression,
};

pub fn children(
    program: &TypedTrees,
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
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// Visit every reachable expression node after its value edges, each once.
/// `arm` observes a node only after the node's children already walked.
pub fn visit_post_order(
    program: &TypedTrees,
    root: ExpressionHandle,
    visited: &mut Vec<ExpressionHandle>,
    mut arm: impl FnMut(&ExpressionNode),
) {
    fn inner(
        program: &TypedTrees,
        expression: ExpressionHandle,
        visited: &mut Vec<ExpressionHandle>,
        arm: &mut dyn FnMut(&ExpressionNode),
    ) {
        if !expression.is_valid() || visited.contains(&expression) {
            return;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        children(program, node, |child| inner(program, child, visited, arm));
        arm(node);
    }
    inner(program, root, visited, &mut arm);
}

/// Climb `Member` (non-variant) and `Indexed` projections to the root
/// expression, returning each projected node outermost-first plus the root.
/// `None` on a cycle, an invalid cursor, or more than `max_depth` edges.
pub fn projection_climb(
    program: &TypedTrees,
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
    program: &TypedTrees,
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
    let ExpressionNode::Name(path) = program.expression_table.expression(named) else {
        return None;
    };
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
    program: &'a TypedTrees,
    value: ExpressionHandle,
) -> Option<&'a str> {
    let ExpressionNode::Name(path) = program.expression_table.expression(value) else {
        return None;
    };
    program
        .expression_table
        .name_path_members(path.members)
        .last()
        .map(|member| member.as_str())
}

/// Strip `Atomic` wrappers: an atomically-evaluated step is still a step.
pub fn unwrapped(program: &TypedTrees, mut expression: ExpressionHandle) -> ExpressionHandle {
    while let ExpressionNode::Atomic(atomic) = program.expression_table.expression(expression) {
        expression = atomic.value;
    }
    expression
}

/// Push every value edge in reverse source order so a stack `pop` yields
/// source-order traversal.
pub fn push_children_reversed(
    program: &TypedTrees,
    node: &ExpressionNode,
    pending: &mut Vec<ExpressionHandle>,
) {
    let mut edges = Vec::new();
    children(program, node, |child| edges.push(child));
    pending.extend(edges.into_iter().rev());
}

/// Structural readers visit every source edge, not just executable arms.
pub(crate) fn match_children(
    program: &TypedTrees,
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
    program: &TypedTrees,
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
pub(crate) fn single_segment_name_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    Some(path.symbol)
}

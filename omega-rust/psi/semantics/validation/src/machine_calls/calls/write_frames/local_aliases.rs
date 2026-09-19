//! Pure local-alias queries for caller-visible write frames.
//!
//! This leaf rebases relative paths through already-canonical alias origins
//! and detects syntactic mutable reborrows or reference-shaped replacements of
//! stable parameter/local aliases. It also resolves direct place expressions
//! through those established origins. It neither recursively infers origins,
//! mutates alias bindings, nor resolves call frames.

use super::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, frame_place_path, split_place_root,
};
use super::type_capabilities::type_reference_is_reference;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableCall};

pub(super) fn rebase_local_alias_path(
    relative: &str,
    aliases: &[(String, FramePlaceOrigin)],
) -> String {
    let (root, suffix) = split_place_root(relative);
    aliases
        .iter()
        .find_map(|(alias, origin)| {
            (alias == root).then(|| match origin.precision {
                FramePathPrecision::Exact => append_place_suffix(&origin.path, suffix),
                FramePathPrecision::CollectionCoarse => origin.path.clone(),
            })
        })
        .unwrap_or_else(|| relative.to_owned())
}

/// Resolve one direct typed place through already-established parameter,
/// caller-isolated-local, single-origin stable-alias, or divergent
/// exclusive-alias origins. Exact aliases compose the authored suffix; a
/// collection-coarse origin remains coarse and cannot be narrowed by a later
/// member projection. A divergent binding contributes its whole proven
/// referent set rather than collapsing to one route or to the bare local
/// name; a candidate that cannot be spelled fails the entire place closed.
pub(super) fn stable_alias_place_origins(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    divergent_aliases: &[(String, Vec<FramePlaceOrigin>)],
    allow_isolated_local: bool,
) -> Option<Vec<FramePlaceOrigin>> {
    let expression = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => inner.target,
        _ => expression,
    };
    let origin = frame_place_path(program, expression)?;
    let (root, suffix) = split_place_root(&origin.path);
    if root == "self"
        || parameters
            .iter()
            .any(|parameter| parameter.name.as_str() == root)
        || (allow_isolated_local && isolated_local_roots.iter().any(|local| local == root))
    {
        return Some(vec![origin]);
    }
    let parents = aliases
        .iter()
        .find_map(|(alias, parent)| (alias == root).then_some(std::slice::from_ref(parent)))
        .or_else(|| {
            divergent_aliases
                .iter()
                .find_map(|(alias, parents)| (alias == root).then_some(parents.as_slice()))
        })?;
    parents
        .iter()
        .map(|parent| {
            (!(!allow_isolated_local
                && isolated_local_roots
                    .iter()
                    .any(|local| local == split_place_root(&parent.path).0)))
            .then(|| match parent.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: append_place_suffix(&parent.path, suffix),
                    precision: origin.precision,
                    source: parent.source.append_source(&origin.source),
                },
                FramePathPrecision::CollectionCoarse => FramePlaceOrigin {
                    path: parent.path.clone(),
                    precision: FramePathPrecision::CollectionCoarse,
                    source: parent.source.append_source(&origin.source),
                },
            })
        })
        .collect()
}

/// An exclusive `&mut`/`&write` borrow of a bare binding is neutral whenever
/// the binding's referent is already established: a tracked alias carries its
/// proven parent, a parameter or `self` spells its own storage, and an
/// isolated local stays caller-invisible. A non-reference binding is a plain
/// place borrow rather than a reborrow. Only a reference binding whose origin
/// cannot be resolved at all keeps failing closed.
pub(super) fn expression_reborrows_unresolved_reference_binding(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    divergent_aliases: &[(String, Vec<FramePlaceOrigin>)],
) -> bool {
    expression_reborrows_reference_binding(program, expression, &|target| {
        stable_alias_place_origins(
            program,
            target,
            parameters,
            isolated_local_roots,
            aliases,
            divergent_aliases,
            true,
        )
        .is_none()
            && super::caller_aliases::caller_binding_type(program, machine, target)
                .is_none_or(|reference| type_reference_is_reference(program, reference))
    })
}

pub(super) fn expression_reborrows_stable_alias_binding(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    expression_reborrows_reference_binding(program, expression, &|target| {
        frame_place_path(program, target).is_some_and(|place| {
            let (root, suffix) = split_place_root(&place.path);
            suffix.is_empty()
                && (parameters.iter().any(|parameter| {
                    type_reference_is_reference(program, parameter.type_reference)
                        && (parameter.is_self && root == "self" || root == parameter.name.as_str())
                }) || aliases.iter().any(|(name, _)| root == name))
        })
    })
}

pub(super) fn expression_reborrows_reference_binding(
    program: &TypedTrees,
    expression: ExpressionHandle,
    is_reference_binding: &impl Fn(ExpressionHandle) -> bool,
) -> bool {
    expression_has_exclusive_borrow(program, expression, &|target| {
        matches!(
            program.expression_table.expression(target),
            ExpressionNode::Name(_)
        ) && is_reference_binding(target)
    })
}

pub(super) fn expression_has_exclusive_borrow(
    program: &TypedTrees,
    expression: ExpressionHandle,
    is_reference_binding: &impl Fn(ExpressionHandle) -> bool,
) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() {
            continue;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                for arm in program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .rev()
                {
                    pending.push(arm.value);
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                }
                pending.push(dispatch.subject);
            }
            ExpressionNode::Borrow(inner) => {
                if inner.access.is_exclusive() && is_reference_binding(inner.target) {
                    return true;
                }
                pending.push(inner.target);
            }
            ExpressionNode::Atomic(atomic) => pending.extend([atomic.value, atomic.result]),
            ExpressionNode::Call(call) => {
                pending.push(call.receiver);
                pending.extend(program.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend(program.expression_table.expression_handles(*elements))
            }
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Range(range) => pending.extend([range.start, range.end]),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
    false
}

/// Whether a statement touches one of the given place roots: a call receiver
/// spelling or any place-shaped node inside its value expressions. The write
/// transfer keeps a write-capable local it cannot name an origin for opaque;
/// a later write through it, an exclusive reborrow, or a transport into
/// another binding would each lose the referent set it denotes. The pure tail
/// return is admitted separately by the caller.
pub(super) fn statement_mentions_place_roots(
    program: &TypedTrees,
    statement: &StatementNode,
    roots: &[String],
) -> bool {
    if let StatementNode::Call(call) = statement
        && program
            .statement_table
            .name_path_members(call.receiver)
            .first()
            .is_some_and(|root| roots.iter().any(|name| name == root.as_str()))
    {
        return true;
    }
    super::statement_value_expression_roots(program, statement)
        .into_iter()
        .any(|expression| expression_mentions_place_roots(program, expression, roots))
}

/// Whether a statement call's mentions of divergent exclusive-alias roots
/// stay confined to argument positions that lend the whole referent set: a
/// direct exclusive reborrow `&mut <place>` rooted at the binding, or the
/// bare binding passed as the actual. A divergent receiver, a member-read
/// actual, or any nested mention would instead name only the binding slot
/// and part of the set, so those statements remain opaque.
pub(super) fn call_mentions_divergent_roots_only_through_reborrow_arguments(
    program: &TypedTrees,
    call: &TableCall,
    divergent_roots: &[String],
) -> bool {
    if program
        .statement_table
        .name_path_members(call.receiver)
        .first()
        .is_some_and(|root| divergent_roots.iter().any(|name| name == root.as_str()))
    {
        return false;
    }
    program
        .statement_table
        .expression_handles(call.arguments)
        .iter()
        .all(|argument| {
            !expression_mentions_place_roots(program, *argument, divergent_roots)
                || argument_is_divergent_reborrow_or_binding(program, *argument, divergent_roots)
        })
}

fn argument_is_divergent_reborrow_or_binding(
    program: &TypedTrees,
    argument: ExpressionHandle,
    divergent_roots: &[String],
) -> bool {
    match program.expression_table.expression(argument) {
        ExpressionNode::Borrow(inner) if inner.access.is_exclusive() => {
            divergent_rooted_place_spine(program, inner.target, divergent_roots)
        }
        // The bare binding passes its whole referent set; a member-path name
        // (`alias.field`) reads an interior reference value instead, which
        // stays opaque.
        ExpressionNode::Name(name) => {
            program
                .expression_table
                .name_path_members(name.members)
                .len()
                == 1
                && frame_place_path(program, argument).is_some_and(|place| {
                    divergent_roots
                        .iter()
                        .any(|root| *root == split_place_root(&place.path).0)
                })
        }
        _ => false,
    }
}

/// The place spine of an admitted divergent reborrow: member and index
/// projections compose onto every candidate, the spine terminates at the
/// binding's own name, and no index selectee mentions a divergent root.
fn divergent_rooted_place_spine(
    program: &TypedTrees,
    expression: ExpressionHandle,
    divergent_roots: &[String],
) -> bool {
    let Some(place) = frame_place_path(program, expression) else {
        return false;
    };
    if !divergent_roots
        .iter()
        .any(|root| *root == split_place_root(&place.path).0)
    {
        return false;
    }
    let mut spine = expression;
    loop {
        match program.expression_table.expression(spine) {
            ExpressionNode::Name(_) => return true,
            ExpressionNode::Member(member) => spine = member.receiver,
            ExpressionNode::Indexed(indexed) => {
                if expression_mentions_place_roots(program, indexed.index, divergent_roots) {
                    return false;
                }
                spine = indexed.collection;
            }
            _ => return false,
        }
    }
}

pub(super) fn expression_mentions_place_roots(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &[String],
) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() {
            continue;
        }
        if frame_place_path(program, expression).is_some_and(|place| {
            roots
                .iter()
                .any(|root| *root == split_place_root(&place.path).0)
        }) {
            return true;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                for arm in program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .rev()
                {
                    pending.push(arm.value);
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                }
                pending.push(dispatch.subject);
            }
            ExpressionNode::Borrow(inner) => pending.push(inner.target),
            ExpressionNode::Atomic(atomic) => pending.extend([atomic.value, atomic.result]),
            ExpressionNode::Call(call) => {
                pending.push(call.receiver);
                pending.extend(program.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend(program.expression_table.expression_handles(*elements))
            }
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Range(range) => pending.extend([range.start, range.end]),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
    false
}

/// A bare write through `alias` (`alias = 1`) targets the borrowed place, but
/// Psi also permits a mutable-reference local declared with plain `let` to be
/// rebound (`alias = &mut other`). Accept an exact origin only while the RHS is
/// proven value-shaped; unknown/reference-shaped replacements fail closed.
pub(super) fn expression_may_rebind_mutable_alias(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .any(|arm| expression_may_rebind_mutable_alias(program, machine, state, arm.value)),
        ExpressionNode::Borrow(_) => true,
        ExpressionNode::Call(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Indexed(_) => {
            crate::value_custody::places::expression_result_is_reference(
                program, machine, state, expression,
            )
            .unwrap_or(true)
        }
        ExpressionNode::Cast(cast) => type_reference_is_reference(program, cast.target_type),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::String(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

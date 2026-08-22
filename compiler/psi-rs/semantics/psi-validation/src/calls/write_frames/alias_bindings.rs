//! Stable local mutable-alias rebinding for write-frame inference.
//!
//! This leaf owns rebinding admission and the mutation of an already-known
//! alias slot. Recursive origin inference remains in the parent and enters
//! through one callback over an immutable view of the established aliases.

use super::local_aliases::expression_may_rebind_mutable_alias;
use super::place_paths::FramePlaceOrigin;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;

/// Update one local mutable-reference binding when its replacement has another
/// directly representable origin. Existing aliases retain their already-
/// canonicalized origins, so rebinding an upstream local never redirects a
/// previously established reborrow.
pub(super) fn rebind_stable_local_mutable_alias_origin<ResolveOrigin>(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: &str,
    value: ExpressionHandle,
    aliases: &mut [(String, FramePlaceOrigin)],
    resolve_origin: ResolveOrigin,
) -> Option<bool>
where
    ResolveOrigin: FnOnce(&[(String, FramePlaceOrigin)]) -> Option<FramePlaceOrigin>,
{
    let Some(position) = aliases.iter().position(|(alias, _)| alias == target) else {
        return Some(false);
    };
    if !expression_may_rebind_mutable_alias(program, machine, state, value) {
        return Some(false);
    }
    let origin = resolve_origin(aliases)?;
    aliases[position].1 = origin;
    Some(true)
}

/// Check whether a reborrow expression can replace an established local alias
/// without mutating it. The origin callback runs only after exact target and
/// reference-shaped replacement admission, preserving fail-closed ordering.
pub(super) fn stable_local_mutable_alias_rebinding_is_representable<ResolveOrigin>(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: &str,
    value: ExpressionHandle,
    aliases: &[(String, FramePlaceOrigin)],
    resolve_origin: ResolveOrigin,
) -> bool
where
    ResolveOrigin: FnOnce(&[(String, FramePlaceOrigin)]) -> Option<FramePlaceOrigin>,
{
    aliases.iter().any(|(alias, _)| alias == target)
        && expression_may_rebind_mutable_alias(program, machine, state, value)
        && resolve_origin(aliases).is_some()
}

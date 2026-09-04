//! Caller-isolated initializer admission for transparent returned-place
//! analysis.
//!
//! This leaf owns the symbol-table precondition and caller-isolated write
//! fence after the shared typed value check. Expression-call frame collection
//! remains in the parent and enters through one callback.

use super::local_aliases::rebase_local_alias_path;
use super::place_paths::{FramePlaceOrigin, split_place_root};
use super::transparent_effects::expression_is_effectful_for_transparent_result;
use crate::symbols::MachineSymbols;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::machine::Machine;

/// A caller-isolated scratch local cannot itself redirect a returned place.
/// Its initializer may therefore precede a transparent returned-place result
/// when it is syntactically effect-free, or when its validated expression
/// writes only previously established caller-isolated scratch locals. The
/// caller first checks every effectful value through the shared typed
/// value-expression traversal, including complete non-rebinding call frames.
/// This predicate adds the local-only write fence to that evidence.
pub(super) fn isolated_local_initializer_preserves_transparent_result<'program, CollectWrites>(
    program: &'program TypedTrees,
    current_machine: &'program Machine,
    expression: ExpressionHandle,
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    collect_writes: CollectWrites,
) -> bool
where
    CollectWrites: FnOnce(&MachineSymbols<'program>, &mut Vec<String>) -> Option<()>,
{
    if !expression_is_effectful_for_transparent_result(program, expression) {
        return true;
    }
    let mut diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, current_machine, &mut diagnostics);
    if !diagnostics.is_empty() {
        return false;
    }
    let mut written = Vec::new();
    collect_writes(&machine_symbols, &mut written).is_some()
        && written.iter().all(|path| {
            let path = rebase_local_alias_path(path, aliases);
            let (root, _) = split_place_root(&path);
            isolated_local_roots.iter().any(|local| local == root)
        })
}

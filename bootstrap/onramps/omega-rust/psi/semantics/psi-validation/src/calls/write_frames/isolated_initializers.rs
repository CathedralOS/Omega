//! Caller-isolated initializer admission for transparent returned-place
//! analysis.
//!
//! This leaf owns the exact syntactic budget, symbol-table precondition, and
//! caller-isolated write fence. Recursive expression-call frame collection
//! remains in the parent and enters through one callback.

use super::isolation::isolated_local_initializer_call_tree_is_bounded;
use super::place_paths::split_place_root;
use super::transparent_effects::expression_is_effectful_for_transparent_result;
use crate::symbols::MachineSymbols;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::machine::Machine;

/// A caller-isolated scratch local cannot itself redirect a returned place.
/// Its initializer may therefore precede a transparent returned-place result
/// when it is syntactically effect-free, or when it is a direct-call tree of
/// maximum depth two whose inferred frames are complete and write only
/// previously established caller-isolated scratch locals. Keep deeper or
/// computed call shapes and every caller-visible or opaque call fenced: this
/// predicate proves only that the initializer cannot perturb the returned-
/// place relation.
pub(super) fn isolated_local_initializer_preserves_transparent_result<'program, CollectWrites>(
    program: &'program TypedTrees,
    current_machine: &'program Machine,
    expression: ExpressionHandle,
    isolated_local_roots: &[String],
    collect_writes: CollectWrites,
) -> bool
where
    CollectWrites: FnOnce(&MachineSymbols<'program>, &mut Vec<String>) -> Option<()>,
{
    if !expression_is_effectful_for_transparent_result(program, expression) {
        return true;
    }
    if !isolated_local_initializer_call_tree_is_bounded(program, expression, 2) {
        return false;
    }

    let mut diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, current_machine, &mut diagnostics);
    if !diagnostics.is_empty() {
        return false;
    }
    let mut written = Vec::new();
    collect_writes(&machine_symbols, &mut written).is_some()
        && written.iter().all(|path| {
            let (root, _) = split_place_root(path);
            isolated_local_roots.iter().any(|local| local == root)
        })
}

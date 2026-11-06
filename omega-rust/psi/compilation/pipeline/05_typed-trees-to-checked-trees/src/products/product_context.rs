//! Keep Product execution separate from the Build code already checked.
//!
//! This selects declaration roots after source admission. It preserves checked
//! backing rows and completed values, independently of optional optimization.

use std::collections::HashSet;
use std::sync::Arc;

use checked_trees::CheckedTrees;
use source::DependencyScope;
use symbols::SymbolHandle;
use typed_trees::references::{DeclarationSelection, collect_program_declarations};

use super::checked_projection::retain_facts;
use super::product_pruning::dependencies::MachineIndex;

pub fn project_checked_product_context(
    checked: Arc<CheckedTrees>,
    execution_roots: &[SymbolHandle],
) -> Arc<CheckedTrees> {
    let original_roots = collect_program_declarations(&checked.typed);
    if original_roots.iter().all(|symbol| {
        checked
            .symbols
            .symbol_is_in_dependency_scope(*symbol, DependencyScope::Product)
            || checked
                .symbols
                .symbol_provenance_source_span(*symbol)
                .is_none()
    }) {
        return checked;
    }
    let typed =
        validation::execution_projection::project_product_program(&*checked, execution_roots);
    let original = DeclarationSelection::from_symbols(original_roots);
    let selected = DeclarationSelection::from_program(&typed);
    let index = MachineIndex::build(&*checked);
    let retained = typed
        .machines()
        .iter()
        .map(|machine| machine.symbol)
        .collect::<HashSet<_>>();
    let pruned = index
        .machine_symbols
        .difference(&retained)
        .copied()
        .collect();
    let checked = Arc::try_unwrap(checked).unwrap_or_else(|shared| (*shared).clone());
    let mut facts = checked.facts;
    retain_facts(&mut facts, &retained, &pruned, &index);
    // Build-only bounds were checked in their own context. Shared owners keep
    // their requirements; an unknown owner remains an obligation.
    facts.integer_range_obligations.retain(|obligation| {
        owner_retained(
            &checked.typed.symbols,
            &original,
            &selected,
            obligation.owner(),
        )
    });
    Arc::new(CheckedTrees::with_roots(typed, facts))
}

fn owner_retained(
    symbols: &symbols::SymbolTable,
    original: &DeclarationSelection,
    selected: &DeclarationSelection,
    mut owner: SymbolHandle,
) -> bool {
    while owner.is_valid() {
        if original.contains(owner) {
            return selected.contains(owner);
        }
        owner = symbols.get(owner).parent;
    }
    true
}

use super::MachineDeclarations;
use crate::machine::Machine;
use arena::OrderedRootArena;
use symbols::SymbolHandle;

fn machine(symbol: SymbolHandle, is_public: bool) -> Machine {
    Machine {
        symbol,
        is_public,
        ..Machine::default()
    }
}

#[test]
fn selection_keeps_first_duplicate_and_exact_symbol_generation() {
    let symbol = SymbolHandle::from_parts(3, 1);
    let next_generation = SymbolHandle::from_parts(3, 2);
    let mut declarations = MachineDeclarations::default();
    let hidden = declarations.push(machine(symbol, false));
    declarations.push(machine(next_generation, false));
    declarations.push(machine(symbol, true));
    assert!(!declarations.by_symbol(symbol).unwrap().is_public);
    assert!(!declarations.by_symbol(next_generation).unwrap().is_public);
    declarations.retain(|machine| machine.symbol != symbol || machine.is_public);
    assert!(declarations.by_symbol(symbol).unwrap().is_public);
    assert!(!declarations.contains(hidden));
    // Selection does not move a previously published declaration identity.
    assert!(!declarations.get(hidden).is_public);
    declarations.retain(|machine| machine.symbol == symbol);
    assert!(declarations.by_symbol(next_generation).is_none());
}

#[test]
fn mutable_storage_and_clones_cannot_share_stale_positions() {
    let original = SymbolHandle::from_parts(1, 1);
    let replacement = SymbolHandle::from_parts(2, 1);
    let mut declarations = MachineDeclarations::default();
    let handle = declarations.push(machine(original, false));
    assert!(declarations.by_symbol(original).is_some());
    let clone = declarations.clone();
    declarations.get_mut(handle).symbol = replacement;
    assert!(declarations.by_symbol(original).is_none());
    assert!(declarations.by_symbol(replacement).is_some());
    assert!(clone.by_symbol(original).is_some());
    assert!(clone.by_symbol(replacement).is_none());
    declarations[0].symbol = original;
    assert!(declarations.by_symbol(replacement).is_none());
    declarations.for_each_mut(|machine| machine.symbol = replacement);
    assert!(declarations.by_symbol(original).is_none());
    declarations.push(machine(original, true));
    assert!(declarations.by_symbol(original).unwrap().is_public);
}

#[test]
fn replacing_equal_revision_arena_replaces_derived_relationships() {
    let original = SymbolHandle::from_parts(1, 1);
    let replacement = SymbolHandle::from_parts(2, 1);
    let mut declarations = MachineDeclarations::default();
    declarations.push(machine(original, false));
    assert!(declarations.by_symbol(original).is_some());
    let mut unrelated = OrderedRootArena::default();
    unrelated.push(machine(replacement, true));
    assert_eq!(declarations.revision(), unrelated.revision());
    *declarations = unrelated;
    assert!(declarations.by_symbol(original).is_none());
    assert!(declarations.by_symbol(replacement).unwrap().is_public);
    let mut unrelated = OrderedRootArena::default();
    unrelated.push(machine(original, false));
    declarations = unrelated.into();
    assert!(declarations.by_symbol(original).is_some());
    assert!(declarations.by_symbol(replacement).is_none());
}

//! Store tests: semantic-key lookup, candidate ordering, invalidation, and
//! capacity refusal. Derivations here are opaque payload markers — the store
//! indexes evidence, it does not verify it.

use super::{DerivationStore, DerivationStoreFull};
use crate::obligations::{
    BoundedValueObligation, ProofObligation, ProofObligationKey, ProofObligationOwner, ProofPlan,
    proof_obligation_key,
};
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

struct Program {
    typed_trees: TypedTrees,
    machine: SymbolHandle,
    data: [SymbolHandle; 2],
    int_type: SymbolHandle,
}

fn program() -> Program {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let machine = SymbolTableBuilder::child_handles(
        builder.insert_children(root, [(SymbolKind::Machine, SymbolNameRef::Static("Main"))]),
    )
    .next()
    .expect("machine");
    let members = SymbolTableBuilder::child_handles(builder.insert_children(
        machine,
        [
            (SymbolKind::Data, SymbolNameRef::Static("count")),
            (SymbolKind::Data, SymbolNameRef::Static("total")),
            (SymbolKind::BuiltinType, SymbolNameRef::Static("Int")),
        ],
    ))
    .collect::<Vec<_>>();
    Program {
        typed_trees: TypedTrees {
            symbols: builder.finish(),
            ..TypedTrees::default()
        },
        machine,
        data: [members[0], members[1]],
        int_type: members[2],
    }
}

impl Program {
    fn int_reference(&mut self) -> TypeReferenceHandle {
        self.typed_trees
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: self.int_type,
                name: Identifier::generated("Int"),
            })
    }
}

fn bounded_value(
    machine: SymbolHandle,
    machine_name: &str,
    data_symbol: SymbolHandle,
    data_name: &str,
    base: TypeReferenceHandle,
) -> ProofObligation {
    ProofObligation::BoundedValue(BoundedValueObligation {
        owner: ProofObligationOwner::MachineOwnedData {
            machine_symbol: machine,
            machine: Identifier::generated(machine_name),
            data_symbol,
            data: Identifier::generated(data_name),
        },
        base_type: base,
        constraints: arena::HandleSpan::empty(),
    })
}

/// The key for "bounded `data`" obligations: `data` selects the semantic
/// row, everything else stays fixed.
fn key_for(
    plan: &ProofPlan<'_>,
    machine: SymbolHandle,
    data: SymbolHandle,
    base: TypeReferenceHandle,
) -> ProofObligationKey {
    proof_obligation_key(plan, &bounded_value(machine, "Main", data, "count", base))
}

#[test]
fn lookup_uses_semantic_identity_not_spelling() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let count = program.data[0];
    let plan = ProofPlan::new(&program.typed_trees);

    let mut store = DerivationStore::new();
    let obligation = bounded_value(machine, "Main", count, "count", base);
    let id = store
        .store_for(&plan, &obligation, "derivation")
        .expect("unbounded store accepts");

    // A display rename beside the same resolved symbols is the same
    // obligation, so its candidates include the retained derivation.
    let renamed = bounded_value(machine, "RenamedMachine", count, "renamed", base);
    let candidates = store.candidates_for(&plan, &renamed).collect::<Vec<_>>();
    assert_eq!(candidates, [(id, &"derivation")]);
    assert_eq!(store.derivation(id), Some(&"derivation"));

    // A different resolved owner is a different obligation: a miss, not a
    // collision.
    let other = bounded_value(machine, "Main", program.data[1], "count", base);
    assert_eq!(store.candidates_for(&plan, &other).count(), 0);
}

#[test]
fn candidates_arrive_in_insertion_order() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let [count, total] = program.data;
    let plan = ProofPlan::new(&program.typed_trees);
    let count_key = key_for(&plan, machine, count, base);
    let total_key = key_for(&plan, machine, total, base);

    let mut store = DerivationStore::new();
    let first = store.store(count_key.clone(), 1).expect("first");
    let second = store.store(count_key.clone(), 2).expect("second");
    let third = store.store(count_key.clone(), 3).expect("third");
    store.store(total_key, 9).expect("other key");

    assert_eq!(
        store.candidates(&count_key).collect::<Vec<_>>(),
        [(first, &1), (second, &2), (third, &3)],
        "several derivations may prove one obligation; the index returns all"
    );
    assert_eq!(store.len(), 4);
    assert_eq!(store.key_count(), 2);
}

#[test]
fn invalidation_frees_the_key_row_and_stales_its_ids() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let [count, total] = program.data;
    let plan = ProofPlan::new(&program.typed_trees);
    let count_key = key_for(&plan, machine, count, base);
    let total_key = key_for(&plan, machine, total, base);

    let mut store = DerivationStore::new();
    let stale = store.store(count_key.clone(), 1).expect("stored");
    let kept = store.store(total_key, 2).expect("stored");

    assert_eq!(store.invalidate(&count_key), 1);
    assert_eq!(store.candidates(&count_key).count(), 0);
    assert_eq!(
        store.derivation(stale),
        None,
        "a freed id resolves to nothing"
    );
    assert_eq!(store.derivation(kept), Some(&2));
    assert_eq!(store.len(), 1);

    // The freed slot is reusable, but its bumped generation keeps the stale
    // id from aliasing the new occupant.
    let fresh_key = key_for(&plan, machine, count, base);
    let recycled = store.store(fresh_key, 3).expect("reuses the slot");
    assert_eq!(store.derivation(stale), None);
    assert_eq!(store.derivation(recycled), Some(&3));
}

#[test]
fn dependency_sweep_drops_every_row_naming_the_changed_dependency() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let [count, total] = program.data;
    let plan = ProofPlan::new(&program.typed_trees);
    let count_key = key_for(&plan, machine, count, base);
    let total_key = key_for(&plan, machine, total, base);
    assert!(
        count_key.as_str().contains("::count"),
        "the canonical key embeds the data symbol's resolved identity"
    );

    let mut store = DerivationStore::new();
    let first = store.store(count_key.clone(), 1).expect("stored");
    let second = store.store(count_key.clone(), 2).expect("stored");
    let kept = store.store(total_key, 3).expect("stored");

    // `count`'s declaration changed: every row whose canonical identity
    // still names it is affected, whatever key the caller can reproduce.
    let removed = store.invalidate_where(|key| key.as_str().contains("::count"));
    assert_eq!(removed, 2);
    assert_eq!(store.candidates(&count_key).count(), 0);
    assert_eq!(store.derivation(first), None);
    assert_eq!(store.derivation(second), None);
    assert_eq!(store.derivation(kept), Some(&3));
    assert_eq!(store.len(), 1);
    assert_eq!(store.key_count(), 1);
}

#[test]
fn dependency_sweep_without_a_match_drops_nothing() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let count = program.data[0];
    let plan = ProofPlan::new(&program.typed_trees);
    let count_key = key_for(&plan, machine, count, base);

    let mut store = DerivationStore::new();
    let id = store.store(count_key.clone(), 1).expect("stored");
    assert_eq!(
        store.invalidate_where(|key| key.as_str().contains("::nonexistent")),
        0
    );
    assert_eq!(store.len(), 1);
    assert_eq!(store.derivation(id), Some(&1));
}

#[test]
fn capacity_refusal_is_explicit() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let [count, total] = program.data;
    let plan = ProofPlan::new(&program.typed_trees);

    let mut store = DerivationStore::with_capacity(1);
    store
        .store(key_for(&plan, machine, count, base), 1)
        .expect("first fits");
    assert_eq!(
        store.store(key_for(&plan, machine, total, base), 2),
        Err(DerivationStoreFull),
        "a bounded store refuses instead of evicting retained evidence"
    );
    assert_eq!(store.len(), 1);
    assert_eq!(store.capacity(), Some(1));
}

#[test]
fn clear_drops_everything() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let count = program.data[0];
    let plan = ProofPlan::new(&program.typed_trees);

    let mut store = DerivationStore::new();
    let id = store
        .store_for(
            &plan,
            &bounded_value(machine, "Main", count, "count", base),
            1,
        )
        .expect("stored");
    store.clear();
    assert!(store.is_empty());
    assert_eq!(store.key_count(), 0);
    assert_eq!(store.derivation(id), None);
}

#[test]
fn stale_ids_from_another_store_do_not_resolve() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let count = program.data[0];
    let plan = ProofPlan::new(&program.typed_trees);
    let key = key_for(&plan, machine, count, base);

    let mut first = DerivationStore::new();
    let foreign = first.store(key.clone(), 1).expect("stored");

    let second = DerivationStore::<u32>::new();
    assert_eq!(second.derivation(foreign), None);

    // A distinct key object with the same canonical text addresses the same
    // semantic row.
    let aliases = first.candidates(&key).collect::<Vec<_>>();
    assert_eq!(aliases, [(foreign, &1)]);
}

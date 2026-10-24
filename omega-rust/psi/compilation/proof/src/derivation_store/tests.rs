//! Store tests: semantic-key lookup, candidate ordering, invalidation, and
//! capacity refusal. Derivations here are opaque payload markers — the store
//! indexes evidence, it does not verify it.

use super::{DerivationStore, DerivationStoreFull};
use crate::obligations::{ProofObligationKey, ProofPlan, proof_obligation_key};
use crate::test_program::{bounded_value, program};
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

/// The key for "bounded `data`" obligations: `data` selects the semantic
/// row, everything else stays fixed.
fn key_for(
    plan: &ProofPlan<'_>,
    machine: SymbolHandle,
    data: SymbolHandle,
    base: TypeReferenceHandle,
) -> ProofObligationKey {
    proof_obligation_key(
        plan,
        &bounded_value(
            machine,
            "Main",
            data,
            "count",
            base,
            arena::HandleSpan::empty(),
        ),
    )
    .expect("formed bounded obligation has a canonical key")
}

#[test]
fn lookup_uses_semantic_identity_not_spelling() {
    let mut program = program();
    let base = program.int_reference();
    let machine = program.machine;
    let count = program.data[0];
    let plan = ProofPlan::new(&program.typed_trees);

    let mut store = DerivationStore::new();
    let obligation = bounded_value(
        machine,
        "Main",
        count,
        "count",
        base,
        arena::HandleSpan::empty(),
    );
    let id = store
        .store_for(&plan, &obligation, "derivation")
        .expect("unbounded store accepts")
        .expect("formed obligation is indexable");

    // A display rename beside the same resolved symbols is the same
    // obligation, so its candidates include the retained derivation.
    let renamed = bounded_value(
        machine,
        "RenamedMachine",
        count,
        "renamed",
        base,
        arena::HandleSpan::empty(),
    );
    let candidates = store.candidates_for(&plan, &renamed).collect::<Vec<_>>();
    assert_eq!(candidates, [(id, &"derivation")]);
    assert_eq!(store.derivation(id), Some(&"derivation"));

    // A different resolved owner is a different obligation: a miss, not a
    // collision.
    let other = bounded_value(
        machine,
        "Main",
        program.data[1],
        "count",
        base,
        arena::HandleSpan::empty(),
    );
    assert_eq!(store.candidates_for(&plan, &other).count(), 0);
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
            &bounded_value(
                machine,
                "Main",
                count,
                "count",
                base,
                arena::HandleSpan::empty(),
            ),
            1,
        )
        .expect("stored")
        .expect("formed obligation is indexable");
    store.clear();
    assert!(store.is_empty());
    assert_eq!(store.key_count(), 0);
    assert_eq!(store.derivation(id), None);
}

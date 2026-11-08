/*
    Persistent derivation store keyed by canonical `ProofObligationKey` identity.

    We serialize and look up accepted proof certificates by their semantic
    obligation fingerprint so repeated obligations re-verify cached evidence
    through the admission kernel instead of repeating search.
*/

use std::collections::BTreeMap;
use std::fmt;

use arena::{Arena, Handle};

use crate::obligations::{ProofObligation, ProofObligationKey, ProofPlan, proof_obligation_key};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DerivationId {
    arena_index: u32,
    generation: u32,
}

impl DerivationId {
    fn from_handle<D>(handle: Handle<Option<D>>) -> Self {
        Self {
            arena_index: handle.arena_index(),
            generation: handle.generation(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DerivationStoreFull;

impl fmt::Display for DerivationStoreFull {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("derivation store is at capacity")
    }
}

impl std::error::Error for DerivationStoreFull {}

#[derive(Debug)]
pub struct DerivationStore<D> {
    derivations: Arena<Option<D>>,
    index: BTreeMap<ProofObligationKey, Vec<Handle<Option<D>>>>,
    capacity: Option<usize>,
}

impl<D> DerivationStore<D> {
    pub fn new() -> Self {
        Self {
            derivations: Arena::new(),
            index: BTreeMap::new(),
            capacity: None,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            derivations: Arena::new(),
            index: BTreeMap::new(),
            capacity: Some(capacity),
        }
    }

    pub const fn capacity(&self) -> Option<usize> {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.derivations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.derivations.is_empty()
    }

    pub fn key_count(&self) -> usize {
        self.index.len()
    }

    pub fn store(
        &mut self,
        key: ProofObligationKey,
        derivation: D,
    ) -> Result<DerivationId, DerivationStoreFull> {
        if self
            .capacity
            .is_some_and(|capacity| self.derivations.len() >= capacity)
        {
            return Err(DerivationStoreFull);
        }
        let handle = self.derivations.insert(Some(derivation));
        self.index.entry(key).or_default().push(handle);
        Ok(DerivationId::from_handle(handle))
    }

    pub fn store_for(
        &mut self,
        proof_plan: &ProofPlan<'_>,
        obligation: &ProofObligation,
        derivation: D,
    ) -> Result<Option<DerivationId>, DerivationStoreFull> {
        let Some(key) = proof_obligation_key(proof_plan, obligation) else {
            return Ok(None);
        };
        self.store(key, derivation).map(Some)
    }

    pub fn candidates<'store>(
        &'store self,
        key: &ProofObligationKey,
    ) -> impl Iterator<Item = (DerivationId, &'store D)> + use<'store, D> {
        self.index
            .get(key)
            .into_iter()
            .flatten()
            .filter_map(move |handle| {
                self.derivations
                    .get(*handle)
                    .as_ref()
                    .map(|derivation| (DerivationId::from_handle(*handle), derivation))
            })
    }

    pub fn candidates_for<'store>(
        &'store self,
        proof_plan: &ProofPlan<'_>,
        obligation: &ProofObligation,
    ) -> impl Iterator<Item = (DerivationId, &'store D)> + use<'store, D> {
        proof_obligation_key(proof_plan, obligation)
            .into_iter()
            .flat_map(move |key| self.candidates(&key))
    }

    pub fn derivation(&self, id: DerivationId) -> Option<&D> {
        let handle = Handle::from_parts(id.arena_index, id.generation);
        self.derivations.get(handle).as_ref()
    }

    pub fn invalidate(&mut self, key: &ProofObligationKey) -> usize {
        let Some(handles) = self.index.remove(key) else {
            return 0;
        };
        let removed = handles.len();
        for handle in handles {
            self.derivations.free(handle);
        }
        removed
    }

    pub fn invalidate_where(
        &mut self,
        mut affected: impl FnMut(&ProofObligationKey) -> bool,
    ) -> usize {
        let stale: Vec<ProofObligationKey> = self
            .index
            .keys()
            .filter(|key| affected(key))
            .cloned()
            .collect();
        let mut removed = 0;
        for key in stale {
            removed += self.invalidate(&key);
        }
        removed
    }

    pub fn clear(&mut self) {
        self.derivations.clear();
        self.index.clear();
    }
}

impl<D> Default for DerivationStore<D> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{DerivationStore, DerivationStoreFull};
    use crate::obligations::{ProofObligationKey, ProofPlan, proof_obligation_key};
    use crate::test_program::{bounded_value, main_owned_value, program};
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceHandle;

    fn key_for(
        plan: &ProofPlan<'_>,
        machine: SymbolHandle,
        data: SymbolHandle,
        base: TypeReferenceHandle,
    ) -> ProofObligationKey {
        proof_obligation_key(plan, &main_owned_value(machine, data, "count", base))
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
        let obligation = main_owned_value(machine, count, "count", base);
        let id = store
            .store_for(&plan, &obligation, "derivation")
            .expect("unbounded store accepts")
            .expect("formed obligation is indexable");

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

        let other = main_owned_value(machine, program.data[1], "count", base);
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
            .store_for(&plan, &main_owned_value(machine, count, "count", base), 1)
            .expect("stored")
            .expect("formed obligation is indexable");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.key_count(), 0);
        assert_eq!(store.derivation(id), None);
    }
}

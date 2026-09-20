//! Semantic index over retained proof derivations.
//!
//! `wiki/drafts/proof_search_cache.md` explores retaining a successful
//! derivation so an unchanged obligation does not repeat proof search. This
//! module is the lookup substrate that draft requires: untrusted derivations
//! stored in a content arena, indexed by each obligation's canonical
//! semantic identity (`ProofObligationKey`) rather than by source position
//! or spelling. A lookup returns *candidate* derivations — retained evidence
//! the caller must still re-decide through the admission kernel against the
//! receiving obligation — never a trusted verdict. A miss and a capacity
//! refusal are explicit outcomes; the index has no path that accepts an
//! unchecked claim.
//!
//! Layout: `derivations` is the content store; its generational handles are
//! the derivation identities, so a freed entry's `DerivationId` can never
//! resolve to a different derivation when its slot is reused. `index` maps
//! each semantic key to its retained candidates in insertion order;
//! `ProofObligationKey` is `Ord`, so a `BTreeMap` keeps iteration
//! deterministic across runs. The key already embeds the encoding schema and
//! every semantic input the draft lists — selected obligations, resolved
//! symbols, constraints, operand payloads — so dependency changes produce a
//! different key and `invalidate` drops the stale row's storage.
//!
//! Out of scope here: persistence format, cross-compilation reuse policy,
//! and consultation inside the checking loop — `DERIVATION-RECHECK-CACHE`
//! owns wiring this index into `check_proof_plan`. The store also does not
//! deduplicate derivations by content: identical derivations stored twice
//! are distinct candidates the recheck decides independently.

use std::collections::BTreeMap;
use std::fmt;

use arena::{Arena, Handle};

use crate::obligations::{ProofObligation, ProofObligationKey, ProofPlan, proof_obligation_key};

/// Identity of one retained derivation inside a `DerivationStore`.
///
/// The id copies the arena handle's slot and generation, so it stays
/// meaningful after the store it came from — or none — is asked to resolve
/// it: a stale id resolves to nothing rather than aliasing a reused slot.
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

    fn handle<D>(self) -> Handle<Option<D>> {
        Handle::from_parts(self.arena_index, self.generation)
    }
}

/// The store refused a derivation because it already holds `capacity`
/// entries. A refusal is an explicit outcome — never a silent eviction of
/// retained evidence and never an acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DerivationStoreFull;

impl fmt::Display for DerivationStoreFull {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("derivation store is at capacity")
    }
}

impl std::error::Error for DerivationStoreFull {}

/// Retained proof derivations indexed by obligation semantic identity.
///
/// `D` is the derivation payload the producer retains — the complete
/// statement and dependency context an independent re-decision needs, such
/// as the certificate packages `checker::certificate` emits. The store is
/// agnostic to `D` because indexing, not verification, is its
/// responsibility.
#[derive(Debug)]
pub struct DerivationStore<D> {
    derivations: Arena<Option<D>>,
    index: BTreeMap<ProofObligationKey, Vec<Handle<Option<D>>>>,
    capacity: Option<usize>,
}

impl<D> DerivationStore<D> {
    /// An unbounded store.
    pub fn new() -> Self {
        Self {
            derivations: Arena::new(),
            index: BTreeMap::new(),
            capacity: None,
        }
    }

    /// A store holding at most `capacity` derivations; inserts beyond it
    /// fail with [`DerivationStoreFull`].
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            derivations: Arena::new(),
            index: BTreeMap::new(),
            capacity: Some(capacity),
        }
    }

    /// The configured capacity, when the store is bounded.
    pub const fn capacity(&self) -> Option<usize> {
        self.capacity
    }

    /// Retained derivations currently occupying the store.
    pub fn len(&self) -> usize {
        self.derivations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.derivations.is_empty()
    }

    /// Distinct semantic keys the index currently maps to candidates.
    pub fn key_count(&self) -> usize {
        self.index.len()
    }

    /// Retain `derivation` under `key`; returns its store identity.
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

    /// Compute `obligation`'s canonical semantic identity in `proof_plan`
    /// and retain `derivation` under it.
    pub fn store_for(
        &mut self,
        proof_plan: &ProofPlan<'_>,
        obligation: &ProofObligation,
        derivation: D,
    ) -> Result<DerivationId, DerivationStoreFull> {
        self.store(proof_obligation_key(proof_plan, obligation), derivation)
    }

    /// The candidates retained under `key`, in the order they were stored.
    /// Each returned derivation is evidence the caller must still re-decide
    /// against the receiving obligation — a hit is not an accepted verdict.
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

    /// The candidates retained under `obligation`'s canonical semantic
    /// identity in `proof_plan`.
    pub fn candidates_for<'store>(
        &'store self,
        proof_plan: &ProofPlan<'_>,
        obligation: &ProofObligation,
    ) -> impl Iterator<Item = (DerivationId, &'store D)> + use<'store, D> {
        self.candidates(&proof_obligation_key(proof_plan, obligation))
    }

    /// The derivation `id` names, or `None` when `id` is stale or never
    /// issued by this store.
    pub fn derivation(&self, id: DerivationId) -> Option<&D> {
        self.derivations.get(id.handle()).as_ref()
    }

    /// Drop every derivation retained under `key`; returns how many entries
    /// were dropped. This is the invalidation granularity the semantic key
    /// provides: a dependency change changes the key, and the stale row's
    /// slots return to the arena.
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

    /// Drop every retained derivation and index row.
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
mod tests;

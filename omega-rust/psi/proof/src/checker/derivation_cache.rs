//! Retained-derivation recheck cache consulted by the bounded certificate
//! routes inside `check_proof_plan`.
//!
//! `wiki/drafts/reference/proof_search_cache.md` explores retaining a successful
//! derivation so an unchanged obligation does not repeat proof search; the
//! `derivation_store` index is the lookup substrate and this module is the
//! consultation policy the draft requires: before a certificate route
//! re-derives a covered bounded leg, the store is asked for derivations
//! retained under the obligation's canonical semantic identity, and every
//! candidate is re-decided through the same admission kernel that decides
//! fresh certificates. An accepted candidate discharges the leg — the
//! producer never runs. A candidate the kernel refuses is counted and passed
//! over, a key with no candidates is a miss, and a full store refuses the
//! retention explicitly: no path accepts an unchecked claim and a cache
//! failure leaves the ordinary derivation in place.
//!
//! Only kernel-accepted certificates are retained — a stored entry means
//! "the admission kernel accepted this package", so re-decision on a later
//! consult is a deterministic accept under an unchanged kernel. Retention is
//! still re-decided on every consult because the store is untrusted
//! substrate: callers may hand it derivations from anywhere, and the kernel,
//! not the store, is the authority.
//!
//! `ProofDerivationCache` is caller-owned so reuse policy — within one
//! compilation, across compilations, or not at all — stays with whoever
//! drives `check_proof_plan`. `DerivationCacheReport` tallies every
//! consultation outcome so the draft's hit-rate and rejection axes are
//! measurable without a persistence format.

use proof_admission::AcceptedFact;

use crate::checker::certificate::BoundedValueCertificate;
use crate::derivation_store::{DerivationStore, DerivationStoreFull};
use crate::obligations::{ProofObligation, ProofObligationKey, ProofPlan, proof_obligation_key};

/// Tallies every recheck-cache consultation outcome.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DerivationCacheReport {
    /// Certificate-route legs that asked the store for a retained
    /// derivation — the hit-rate denominator.
    pub consultations: u64,
    /// Legs a retained derivation discharged after the kernel re-decided it
    /// — the hit-rate numerator.
    pub reused: u64,
    /// Retained candidates the admission kernel rejected on re-decision.
    /// Each is passed over; the fresh derivation still decides the leg.
    pub rejected_candidates: u64,
    /// Certificates retained after the kernel accepted them.
    pub retained: u64,
    /// Retentions refused because the store was at capacity.
    pub capacity_refusals: u64,
    /// Retained derivations dropped by explicit invalidation — a named key
    /// or a dependency sweep. This is the draft's invalidation axis made
    /// measurable.
    pub invalidated: u64,
}

/// Caller-owned cache of bounded-leg certificates for proof-plan rechecks.
///
/// The wrapped store indexes each retained certificate by the obligation's
/// canonical `ProofObligationKey`, so a display rename or position change
/// still hits while any semantic dependency change produces a different key.
#[derive(Debug, Default)]
pub struct ProofDerivationCache {
    store: DerivationStore<BoundedValueCertificate>,
    report: DerivationCacheReport,
}

impl ProofDerivationCache {
    /// An unbounded cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// A cache retaining at most `capacity` derivations; retentions beyond
    /// it are explicit refusals — the leg's verdict is unaffected.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            store: DerivationStore::with_capacity(capacity),
            report: DerivationCacheReport::default(),
        }
    }

    /// Retained derivations awaiting recheck.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    /// Distinct obligation keys currently indexing candidates.
    pub fn key_count(&self) -> usize {
        self.store.key_count()
    }

    /// Every consultation outcome the cache has observed.
    pub fn report(&self) -> DerivationCacheReport {
        self.report
    }

    /// Drop every derivation retained under `key`; returns how many entries
    /// were dropped. A dependency change produces a different key, so this
    /// is the granularity at which stale evidence is removed.
    pub fn invalidate(&mut self, key: &ProofObligationKey) -> usize {
        let removed = self.store.invalidate(key);
        self.report.invalidated += removed as u64;
        removed
    }

    /// Drop every retained derivation whose key `affected` accepts; returns
    /// how many entries were dropped and tallies them under
    /// [`DerivationCacheReport::invalidated`]. A dependency change affects
    /// every row still naming it, so the sweep frees them without knowing
    /// each affected key (typically `|key| key.as_str().contains(...)`).
    pub fn invalidate_where(&mut self, affected: impl FnMut(&ProofObligationKey) -> bool) -> usize {
        let removed = self.store.invalidate_where(affected);
        self.report.invalidated += removed as u64;
        removed
    }

    /// Drop every retained derivation and reset the index.
    pub fn clear(&mut self) {
        self.store.clear();
    }
}

/// One obligation's borrowed view of a [`ProofDerivationCache`]: the
/// canonical semantic key under which the certificate routes consult and
/// retain. `check_proof_plan` constructs one per decided obligation when a
/// cache is attached.
pub(crate) struct DerivationConsultation<'a> {
    /// Canonical semantic identity of the receiving obligation.
    key: ProofObligationKey,
    /// The caller's cache.
    cache: &'a mut ProofDerivationCache,
}

impl DerivationConsultation<'_> {
    pub(crate) fn new<'cache>(
        proof_plan: &ProofPlan<'_>,
        obligation: &ProofObligation,
        cache: &'cache mut ProofDerivationCache,
    ) -> DerivationConsultation<'cache> {
        DerivationConsultation {
            key: proof_obligation_key(proof_plan, obligation),
            cache,
        }
    }

    /// Ask the store for derivations retained under this obligation's key
    /// and let the admission kernel re-decide every candidate. The first
    /// candidate the kernel accepts discharges the leg and is returned as
    /// the accepted fact; rejected candidates are counted and passed over,
    /// and an empty row is a miss — the producer still decides the leg.
    pub(crate) fn recheck(&mut self) -> Option<AcceptedFact> {
        self.cache.report.consultations += 1;
        // Candidate ids are collected before re-deciding so a rejected
        // candidate can be counted without holding a store borrow.
        let candidates: Vec<_> = self
            .cache
            .store
            .candidates(&self.key)
            .map(|(id, _)| id)
            .collect();
        for id in candidates {
            let Some(candidate) = self.cache.store.derivation(id) else {
                continue;
            };
            match candidate.verify() {
                Ok(fact) => {
                    self.cache.report.reused += 1;
                    return Some(fact);
                }
                Err(_) => {
                    self.cache.report.rejected_candidates += 1;
                }
            }
        }
        None
    }

    /// Retain a certificate the kernel just accepted so a later recheck of a
    /// semantically identical obligation can reuse it. A capacity refusal is
    /// explicit: the leg's verdict stands either way.
    pub(crate) fn retain(&mut self, certificate: BoundedValueCertificate) {
        match self.cache.store.store(self.key.clone(), certificate) {
            Ok(_) => {
                self.cache.report.retained += 1;
            }
            Err(DerivationStoreFull) => {
                self.cache.report.capacity_refusals += 1;
            }
        }
    }
}

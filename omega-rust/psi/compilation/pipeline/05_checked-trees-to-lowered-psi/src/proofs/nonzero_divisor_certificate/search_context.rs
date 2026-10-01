//! Invocation-owned search fuel and proposition-context identity.
//!
//! A candidate may recursively open many implication/endpoint searches. Their
//! local limits cannot bound the total, so every producer charges one shared
//! deterministic scope-row allowance. Exhaustion records an Incomplete resource
//! and never bypasses certificate checking. Proof memos use the invocation
//! generation: warm worker caches must not grant extra search work or preserve
//! another invocation's budget-exhausted refusal. Pure fact indexes remain shared.
//!
//! Producer caches match their scopes by full content, and the machine's
//! value-type table is the largest part of that content. A token is the
//! position of a context's content in this worker's list of distinct
//! contexts, so two tokens are equal exactly when the two contexts are, and a
//! cache comparing tokens keeps the same hits and misses as one comparing the
//! tables.
//!
//! Resolving a token still compares content. A top-level search borrows one
//! unchanged context for its whole duration, so its entry resolves that token
//! once and nested lookups naming the same borrowed object reuse it. The
//! shortcut trusts an address only while the guard holding that borrow is
//! alive; any other context resolves by content.

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;

use semantic_vocabulary::PropositionContext;

use crate::lowering_error::ProofSearchCapacity;
use crate::proofs::ProofSearchDeclines;

thread_local! {
    /// Distinct proposition contexts this worker has proved under; a
    /// context's token is its index here.
    static CONTEXTS: RefCell<Vec<PropositionContext>> = const { RefCell::new(Vec::new()) };

    /// The innermost live search's borrowed context and its resolved token.
    static ACTIVE: Cell<Option<(*const PropositionContext, usize)>> = const { Cell::new(None) };

    // Search work belongs to the invocation, never the worker's history.
    static WORK: Cell<Option<SearchWork>> = const { Cell::new(None) };
    static NEXT_SEARCH: Cell<u64> = const { Cell::new(0) };

    /// One decline ledger per live search scope, innermost last. A refused
    /// draw belongs to the scope it happened under, so the entry can drain
    /// it even when a nested producer records the refusal.
    static DECLINES: RefCell<Vec<ProofSearchDeclines>> = const { RefCell::new(Vec::new()) };
}

/// A search's generation partitions budget-sensitive proof memos. Pure
/// content-derived indexes may stay shared: only proof attempts spend fuel.
#[derive(Clone, Copy)]
struct SearchWork {
    generation: u64,
    requested: u64,
    limit: u64,
}

pub(super) fn generation() -> u64 {
    WORK.with(Cell::get).map_or(0, |work| work.generation)
}

/// Charge a traversal before entering its uncached search cascade. The
/// roster width models the facts a leaf may inspect; recursion shares this
/// same allowance. Neither elapsed time nor another worker changes it.
pub(super) fn spend_scope(assumptions: usize, axioms: usize) -> Option<()> {
    let amount = (assumptions as u64).saturating_add(axioms as u64).max(1);
    spend(amount)
}

pub(super) fn spend(amount: u64) -> Option<()> {
    let refused = WORK.with(|cell| {
        let Some(mut work) = cell.get() else {
            return None;
        };
        // Once exhausted, unwind without incrementing demand for every
        // fallback visited on the way out. Retain the first refused draw.
        if work.requested > work.limit {
            return Some(work.requested);
        }
        work.requested = work.requested.saturating_add(amount);
        cell.set(Some(work));
        (work.requested > work.limit).then_some(work.requested)
    });
    if let Some(requested) = refused {
        record_decline(ProofSearchCapacity::IntegerCertificateSearchWork, requested);
        None
    } else {
        Some(())
    }
}

/// Account one refused draw's total demand on `capacity` in the innermost
/// live search's ledger. Route internals tested without a scope decline
/// with no ledger to carry the refusal.
pub(super) fn record_decline(capacity: ProofSearchCapacity, requested: u64) {
    DECLINES.with(|cell| {
        if let Some(ledger) = cell.borrow_mut().last_mut() {
            ledger.record_demand(capacity, requested);
        }
    });
}

/// The content token for `context`.
pub(super) fn token(context: &PropositionContext) -> usize {
    if let Some((active, token)) = ACTIVE.with(Cell::get)
        && std::ptr::eq(active, context)
    {
        return token;
    }
    content_token(context)
}

/// Resolve the token by content, allocating a slot on first sight.
fn content_token(context: &PropositionContext) -> usize {
    CONTEXTS.with(|cell| {
        let mut contexts = cell.borrow_mut();
        match contexts.iter().position(|existing| existing == context) {
            Some(token) => token,
            None => {
                contexts.push(context.clone());
                contexts.len() - 1
            }
        }
    })
}

/// One top-level search over a borrowed context. While this guard lives, the
/// borrow keeps the context at its address unchanged, so token lookups for
/// that address skip the content comparison. Dropping it restores the
/// enclosing search's context.
pub(super) struct SearchContext<'context> {
    enclosing: Option<(*const PropositionContext, usize)>,
    enclosing_work: Option<SearchWork>,
    context: PhantomData<&'context PropositionContext>,
}

impl<'context> SearchContext<'context> {
    pub(super) fn enter(context: &'context PropositionContext) -> Self {
        Self::with_limit(
            context,
            ProofSearchCapacity::IntegerCertificateSearchWork.limit(),
        )
    }

    fn with_limit(context: &'context PropositionContext, limit: u64) -> Self {
        let generation = NEXT_SEARCH.with(|next| {
            let generation = next
                .get()
                .checked_add(1)
                .expect("search generation exhausted");
            next.set(generation);
            generation
        });
        let enclosing_work = WORK.with(|work| {
            work.replace(Some(SearchWork {
                generation,
                requested: 0,
                limit,
            }))
        });
        let token = token(context);
        let enclosing =
            ACTIVE.with(|active| active.replace(Some((std::ptr::from_ref(context), token))));
        DECLINES.with(|cell| cell.borrow_mut().push(ProofSearchDeclines::default()));
        Self {
            enclosing,
            enclosing_work,
            context: PhantomData,
        }
    }

    pub(super) fn report(&self, kind: &str, proved: bool) {
        if crate::proofs::proof_timing::enabled()
            && let Some(work) = WORK.with(Cell::get)
        {
            eprintln!(
                "[proof-work] kind={kind} proved={proved} requested={} limit={}",
                work.requested, work.limit
            );
        }
    }

    /// This scope's recorded declines, moved out so the caller's own ledger
    /// can carry them into fallback search and finalization. The scope's
    /// cell is the innermost live one, so undrained nested scopes can only
    /// contribute their own refusals.
    pub(super) fn take_declines(&self) -> ProofSearchDeclines {
        DECLINES.with(|cell| {
            cell.borrow_mut()
                .last_mut()
                .map(std::mem::take)
                .unwrap_or_default()
        })
    }
}

impl Drop for SearchContext<'_> {
    fn drop(&mut self) {
        WORK.with(|work| work.set(self.enclosing_work));
        ACTIVE.with(|active| active.set(self.enclosing));
        DECLINES.with(|cell| {
            cell.borrow_mut().pop();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{SearchContext, spend};
    use crate::lowering_error::{LoweringError, ProofSearchCapacity};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, ObligationId, Proposition, ScalarTerm, ScalarType,
        ValueId,
    };
    use std::collections::BTreeSet;

    #[test]
    fn exhausted_integer_search_keeps_its_named_incomplete_demand() {
        let context = semantic_vocabulary::PropositionContext::from_value_types([]).unwrap();
        let search = SearchContext::enter(&context);
        let capacity = ProofSearchCapacity::IntegerCertificateSearchWork;
        assert!(spend(capacity.limit()).is_some());
        assert!(spend(7).is_none());
        assert!(spend(100).is_none());
        let obligation = ObligationId::new(1).unwrap();
        assert_eq!(
            search.take_declines().unproved_obligation_error(obligation),
            LoweringError::ProofSearchCapacityExhausted {
                capacity,
                requested: capacity.limit() + 7,
                obligation
            }
        );
    }

    #[test]
    fn integer_search_memos_do_not_cross_budget_lifetimes() {
        let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let identity = ValueId::new(1).unwrap();
        let context = semantic_vocabulary::PropositionContext::from_value_types([(
            identity,
            ScalarType::Integer(integer),
        )])
        .unwrap();
        let value = ScalarTerm::value(identity, ScalarType::Integer(integer));
        let literal = |number| ScalarTerm::integer(integer, IntegerValue::Signed(number)).unwrap();
        let assumptions = vec![Proposition::LessOrEqual(value.clone(), literal(10))];
        let goal = Proposition::LessThan(value, literal(11));
        let prove = |limit| {
            let _search = SearchContext::with_limit(&context, limit);
            super::super::certificate_entry::prove_with_machine_parameters(
                &context,
                &goal,
                &assumptions,
                &[],
                &BTreeSet::from([identity]),
            )
        };
        let limit = ProofSearchCapacity::IntegerCertificateSearchWork.limit();
        let cold = prove(limit).expect("cited inclusive bound proves the adjacent strict bound");
        assert!(
            prove(0).is_none(),
            "a warm proof must not bypass a fresh search budget"
        );
        assert_eq!(
            prove(limit),
            Some(cold.clone()),
            "exhausted negative answers do not poison later searches"
        );
        assert!(prove(0).is_none());
        assert_eq!(prove(limit), Some(cold));
    }

    #[test]
    fn nested_integer_search_restores_the_enclosing_budget() {
        let context = semantic_vocabulary::PropositionContext::from_value_types([]).unwrap();
        let _outer = SearchContext::with_limit(&context, 3);
        assert!(spend(2).is_some());
        {
            let _inner = SearchContext::with_limit(&context, 1);
            assert!(spend(2).is_none());
        }
        assert!(spend(1).is_some());
        assert!(spend(1).is_none());
    }
}

//! Content tokens for the proposition context a certificate search runs under.
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

    /// One decline ledger per live search scope, innermost last. A refused
    /// draw belongs to the scope it happened under — not to the producer
    /// call chain between them — so a route nested arbitrarily deep in one
    /// search records where the search's entry can drain it, exactly as
    /// token lookups resolve against the innermost context.
    static DECLINES: RefCell<Vec<ProofSearchDeclines>> = const { RefCell::new(Vec::new()) };
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
    context: PhantomData<&'context PropositionContext>,
}

impl<'context> SearchContext<'context> {
    pub(super) fn enter(context: &'context PropositionContext) -> Self {
        let token = token(context);
        let enclosing =
            ACTIVE.with(|active| active.replace(Some((std::ptr::from_ref(context), token))));
        DECLINES.with(|cell| cell.borrow_mut().push(ProofSearchDeclines::default()));
        Self {
            enclosing,
            context: PhantomData,
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
        ACTIVE.with(|active| active.set(self.enclosing));
        DECLINES.with(|cell| {
            cell.borrow_mut().pop();
        });
    }
}

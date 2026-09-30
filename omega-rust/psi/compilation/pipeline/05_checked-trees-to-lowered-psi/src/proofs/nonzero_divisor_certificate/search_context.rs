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

thread_local! {
    /// Distinct proposition contexts this worker has proved under; a
    /// context's token is its index here.
    static CONTEXTS: RefCell<Vec<PropositionContext>> = const { RefCell::new(Vec::new()) };

    /// The innermost live search's borrowed context and its resolved token.
    static ACTIVE: Cell<Option<(*const PropositionContext, usize)>> = const { Cell::new(None) };
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
        Self {
            enclosing,
            context: PhantomData,
        }
    }
}

impl Drop for SearchContext<'_> {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(self.enclosing));
    }
}

//! Content tokens for the cited scope a producer searches under.
//!
//! A scope is one `(assumptions, semantic_axioms)` pair. Producers memoize
//! scope-derived indexes and answers, matched by full content, and ask for
//! them at every goal of a search: the scope comparison itself becomes a
//! repeated cost. A token names one interned scope content. Equal tokens
//! always name equal content; equal content can carry two tokens only after
//! the older interning was evicted. Caches of pure derivations key on the
//! token alone and at worst recompute. Caches whose hits must match exactly
//! when content matches compare the token first and fall back to content, so
//! their hits are unchanged.
//!
//! Resolving a token still compares content. A search frame borrows its
//! scope's slices for its whole duration, so a frame registers them and
//! nested lookups naming the same borrowed slices reuse the frame's token.
//! A slice is recognized only while the frame borrowing it is alive; any
//! other slice resolves by content.

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;
use std::thread::LocalKey;

use semantic_vocabulary::{Proposition, PropositionContext};

use super::search_context;

/// Interned scope contents kept per worker. Evicting one only retires its
/// token; a later interning of the same content receives a fresh token.
const MAXIMUM_SCOPES: usize = 64;

type SliceIdentity = (*const Proposition, usize);

struct Frame {
    assumptions: SliceIdentity,
    semantic_axioms: SliceIdentity,
    token: Option<u64>,
}

thread_local! {
    /// Interned scope contents, most recently used last.
    static SCOPES: RefCell<Vec<(Vec<Proposition>, Vec<Proposition>, u64)>> =
        const { RefCell::new(Vec::new()) };

    /// The next token to issue; tokens are never reused on a worker.
    static NEXT_TOKEN: Cell<u64> = const { Cell::new(0) };

    /// Live search frames, innermost last.
    static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
}

fn identity(slice: &[Proposition]) -> SliceIdentity {
    (slice.as_ptr(), slice.len())
}

/// The token of this scope's content.
pub(super) fn token(assumptions: &[Proposition], semantic_axioms: &[Proposition]) -> u64 {
    let assumptions_identity = identity(assumptions);
    let axioms_identity = identity(semantic_axioms);
    let framed = FRAMES.with(|frames| {
        let mut frames = frames.borrow_mut();
        let frame = frames.iter_mut().rev().find(|frame| {
            frame.assumptions == assumptions_identity && frame.semantic_axioms == axioms_identity
        })?;
        Some(
            *frame
                .token
                .get_or_insert_with(|| intern(assumptions, semantic_axioms)),
        )
    });
    framed.unwrap_or_else(|| intern(assumptions, semantic_axioms))
}

fn intern(assumptions: &[Proposition], semantic_axioms: &[Proposition]) -> u64 {
    SCOPES.with(|scopes| {
        let mut scopes = scopes.borrow_mut();
        if let Some(index) = scopes
            .iter()
            .rposition(|(seen_assumptions, seen_axioms, _)| {
                seen_assumptions.as_slice() == assumptions
                    && seen_axioms.as_slice() == semantic_axioms
            })
        {
            let entry = scopes.remove(index);
            let token = entry.2;
            scopes.push(entry);
            return token;
        }
        let token = NEXT_TOKEN.with(|next| {
            let token = next.get();
            next.set(token + 1);
            token
        });
        scopes.push((assumptions.to_vec(), semantic_axioms.to_vec(), token));
        if scopes.len() > MAXIMUM_SCOPES {
            scopes.remove(0);
        }
        token
    })
}

/// One search frame over a borrowed scope. While this guard lives, the
/// borrows keep both slices at their addresses unchanged, so token lookups
/// for those exact slices skip the content comparison. Guards nest with the
/// call stack; dropping one retires its registration.
pub(super) struct ScopeFrame<'scope> {
    scope: PhantomData<&'scope [Proposition]>,
}

impl<'scope> ScopeFrame<'scope> {
    pub(super) fn enter(
        assumptions: &'scope [Proposition],
        semantic_axioms: &'scope [Proposition],
    ) -> Self {
        FRAMES.with(|frames| {
            frames.borrow_mut().push(Frame {
                assumptions: identity(assumptions),
                semantic_axioms: identity(semantic_axioms),
                token: None,
            });
        });
        Self { scope: PhantomData }
    }
}

impl Drop for ScopeFrame<'_> {
    fn drop(&mut self) {
        FRAMES.with(|frames| {
            frames.borrow_mut().pop();
        });
    }
}

/// Scope derivations one cache keeps per worker.
const MAXIMUM_DERIVED: usize = 16;

/// A per-worker cache of one pure scope derivation, keyed by scope token
/// (and context token when the derivation reads the context) and most
/// recently used last. A derivation is a function of that content alone, so
/// a hit is exactly what recomputing would produce.
pub(super) struct ScopeDerived<T> {
    entries: Vec<(DerivationKey, Rc<T>)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct DerivationKey {
    context: Option<usize>,
    scope: u64,
}

impl<T> ScopeDerived<T> {
    pub(super) const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

/// `derive`'s result for this scope, computed once while the scope's token
/// stays cached.
pub(super) fn derived<T: 'static>(
    cache: &'static LocalKey<RefCell<ScopeDerived<T>>>,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    derive: impl FnOnce() -> T,
) -> Rc<T> {
    let key = DerivationKey {
        context: None,
        scope: token(assumptions, semantic_axioms),
    };
    derived_for(cache, key, derive)
}

/// `derived` for a derivation that also reads the proposition context.
pub(super) fn derived_in_context<T: 'static>(
    cache: &'static LocalKey<RefCell<ScopeDerived<T>>>,
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    derive: impl FnOnce() -> T,
) -> Rc<T> {
    let key = DerivationKey {
        context: Some(search_context::token(context)),
        scope: token(assumptions, semantic_axioms),
    };
    derived_for(cache, key, derive)
}

fn derived_for<T: 'static>(
    cache: &'static LocalKey<RefCell<ScopeDerived<T>>>,
    key: DerivationKey,
    derive: impl FnOnce() -> T,
) -> Rc<T> {
    let found = cache.with(|cell| {
        let mut cache = cell.borrow_mut();
        let index = cache.entries.iter().rposition(|(seen, _)| *seen == key)?;
        let entry = cache.entries.remove(index);
        let value = entry.1.clone();
        cache.entries.push(entry);
        Some(value)
    });
    if let Some(value) = found {
        return value;
    }
    let value = Rc::new(derive());
    cache.with(|cell| {
        let mut cache = cell.borrow_mut();
        cache.entries.push((key, value.clone()));
        if cache.entries.len() > MAXIMUM_DERIVED {
            cache.entries.remove(0);
        }
    });
    value
}

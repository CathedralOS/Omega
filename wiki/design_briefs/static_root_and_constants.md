# Design Brief — Constants & the Static Root

> **For:** Omega maintainer · **Status:** SETTLED (chat 2026-07-04, Zach). ·
> **Driver:** Cathedral's boot/ABI/facts code is constant-heavy and wrote
> C-style free-floating `NAME: T = value` declarations, which Omega had no home
> for — exposing three tangled holes: where const lives, where static lives, and
> why `main`'s `&self` looked like a generated-static-ref hack. · **Depends on:**
> [`build_time_evaluation.md`](build_time_evaluation.md) (const-position eval),
> `chapter_16_drops_and_cleanup.md` (the cleanup facts the const restriction
> reads), `chapter_14` (the `::` path rule). · **Companion:** the freestanding
> entry (`freestanding_boot_and_hardware_facts.md`; `main` is the entry).

## Bottom line

The three holes are **two different problems**, and separating them dissolves
most of the difficulty:

1. **`const`** = an immutable compile-time **value** (no identity). Free-floating
   + namespaced, Rust-like; restricted to *pure values*. Not authority → no
   capability-model concern. **Closed.**
2. **static state** = mutable runtime **state** (has identity). There is **no
   free-floating `static`**; there is exactly **one root — `main`'s `&self`** —
   and all persistent state is its subtree, reached only by borrowing *down*.
   This is the capability model applied to storage.
3. **`main`'s `&self`** is not a hack — it is the static root, made explicit: the
   single static allocation, established by the entry before `main` runs, the
   origin of both authority and persistent state.

## 1. `const` — a pure value, free-floating, namespaced

A `const` is a named value whose initializer is evaluated at build time (an
effect-free expression in constant position — `build_time_evaluation.md`). So it
is a *value*, not storage:

```omega
pub const PAGE_SIZE: u64 = 4096;
pub const EFI_SUCCESS: EfiStatus = EfiStatus { code: 0 };
```

- **Free-floating, namespaced by package/module** (the default), resolved by the
  `::` path rule (a `const` is a compile-time name: `memory::PAGE_SIZE`). It may
  also be **type-scoped** when it genuinely belongs to a type
  (`EfiStatus::SUCCESS`) — declared like a machine (`const Type::NAME = …`),
  **outside** the `data` block, never a member of it. (Binding *unrelated*
  constants to a `data` symbol is worse design; scope only when related.)
- **Never in the `data` block → never in `sizeof`, by construction.** This is
  Rust's separation (constants live in `impl`, not the struct), achieved via
  Omega's existing `::` type-scoping instead of a separate block. No "exclude
  const members from layout" rule is needed, because a const is never a member.
- **Immutable and a PURE VALUE** — the const's type must have **no cleanup
  obligation, no shared ownership, no interior mutability**. It is copied freely
  at each use, so it is trivially borrowable and trivially thread-safe (no shared
  identity to race on). The restriction is *checked from the cleanup facts*
  (ch16); a type with a drop obligation (an `Arc`-like handle, a lock, a
  `Cell`-like cell) cannot be a `const`. This is precisely the rule that avoids
  Rust's interior-mutability-in-const footgun (each use inlines a fresh copy —
  surprising for a cell, harmless for a pure value) — Omega forbids the
  surprising case rather than linting it.
- **Not authority.** A constant grants nothing, so free-floating constants are
  fully consistent with the capability model. The thing the model forbids is
  ambient *mutable* state / capabilities — a `const` is neither.

## 2. static state — one root, no free-floating `static`

**There is no `static` keyword and no free-floating mutable static.** The reason
is the same one that makes ambient authority forbidden, at the storage layer:

- A generic `static FOO` is hard to reason about *because it is name-reachable
  from anywhere* — "who holds `&mut FOO`" becomes a **global** analysis. That is
  the same shape as ambient authority: reachable-by-name-from-nowhere.
- So: **exactly one static root — `main`'s `&self` — and every other piece of
  persistent state is a field of it, reached only by borrowing down**, threaded
  as parameters. You cannot *name* a static out of nowhere; you can only use a
  borrow someone handed you.

Two payoffs:

- **Borrow-checking over static goes local.** It is the ordinary borrow story
  over an owned tree, not a special global escape hatch — because there is no
  global name to grab.
- **Thread-safety becomes ordinary.** One-root does *not* invent a static-safety
  mechanism; it makes static state *subject to the existing one*. Two threads
  wanting `&mut` into the root's subtree → refused by ordinary aliasing. Shared
  `&` across threads → the ordinary `Send`/`Share` story. Static stops being
  special. (Generic statics needed bespoke thread-safety analysis *precisely
  because* they were name-reachable.)

## 3. `main`'s `&self` — the root, made explicit

`main`'s `&self` is **the static root**: the single static allocation, its
subtree the program's entire persistent state, established by the entry/runtime
*before* `main` runs. Naming it dissolves the "magic": it is the most
load-bearing allocation in the program, honestly labelled, and it is the one
blessed bootstrap step — worth documenting as *the* trusted setup rather than
pretending it isn't there. On a foreign OS the entry stub constructs it; on
Cathedral the launcher / SAS hand-off constructs it; under the boot entry the
firmware hand-off does.

## The unification

This is **the capability model applied to storage.** The capability model: no
ambient *authority* — everything is a held/passed capability descending from a
root. The static-root discipline: no ambient *state* — everything persistent is
a field descending from a root. They are the same principle, and `main`'s
`&self` is where both roots **coincide** — the single origin of authority and of
persistent state. So "one static root" is not a new invariant to bolt on; it is
the invariant the capability model already forces, reaching the storage layer.

## Honest caveat

The discipline leans on there *being* a coherent single root — which works
because the capability model already insists on it (root mints everything). A
subsystem genuinely needing name-reachable global mutable state would pinch — but
that is exactly the ambient-authority thing Cathedral refuses, so the pinch is a
feature, not a gap.

## What Omega does

- **Add `const`** (ch1): free-floating or `Type::`-scoped; build-time-evaluated;
  pure-value restriction checked from the cleanup facts. Excluded from `sizeof`
  by construction (never a `data` member).
- **No `static` keyword.** Persistent mutable state is `main`'s subtree,
  borrow-reached; document the entry's root allocation as the bootstrap step.
- Cathedral's free-floating constants become `const`; foreign enums that want
  full typing use case discriminants (ch1); most memory-type tags stay named
  `const` u32s (robust to unknown firmware kinds).

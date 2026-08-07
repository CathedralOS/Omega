# Chapter 11: Invariant Windows

Writes never fail an invariant check. Invariants are checked where a value
can be observed.

Chapter 7 owns what invariants mean; this chapter owns when they must be
re-established. No explicit `relax` construct exists—the checker derives an
invariant window from writes and closes it at the next consumption point.

```omega
data Span
where
    start <= end,
{
    start: u32;
    end: u32;
}

machine Span::shift(&mut self, delta: u32 [0..=1000]) {
    self.start = self.start + delta;   // may transiently break start <= end
    self.end = self.end + delta;       // coupling restored
}
```

The first write may violate the coupling; nothing rejects it. The domain is
proven again from the flow facts at the next point where the value could be
observed — here, the machine's return.

Working interpretation:

- A write the checker can prove domain-preserving changes nothing: every
  fact stays standing. This is the common case and every landed
  range-checked store.
- A write it cannot prove opens a **window** on the place: the domain is
  suspended there, and the flow facts record what was actually written.
- A window must close — the domain re-proven from the flow facts — at the
  next **consumption point**. An unclosed window at a consumption point is a
  compile error citing both ends: the write that opened the window and the
  point that needed it closed.
- Nothing can observe a value mid-window. That is not a new rule; it is the
  list of consumption points.

## Consumption Points

A consumption point is anywhere in-domain-ness could be observed:

- **A read that relies on a domain fact.** Reading a field into arithmetic
  that needs its range; reading a dependent place whose coupling names the
  written witness.
- **Creating a borrow.** A borrow hands out a view someone will trust.
- **Any call.** The callee assumes the default domain of everything its
  signature can reach.
- **A transition.** Parameter refinements and explicit state `requires` are
  proven at every in-edge; proof debt never crosses the state graph. A
  `self` back-edge is checked after preceding mutations, not from the stale
  entry assumption.
- **Return and scope expiration.**
- **Any boundary or capability-carrying call.** The world can observe
  memory, whether or not the call names the place.

## Failure Does Not Cancel A Window

A modeled recoverable failure is an ordinary sum outcome handled at a call or
transition edge. Calls and transitions are consumption points, so every path to
that outcome already has all reachable invariant windows closed. There is no
`cancel`, `poison`, `unstable`, or other runtime state for proof debt: a window
closes by re-establishing its facts, or the program is rejected.

Proof facts have no mutable runtime truth bit. A runtime check or admitted
receipt may establish a fact, and artifacts may retain its evidence provenance,
but later code cannot retroactively downgrade the fact. If an admitted provider
lied or hardware violated the execution model, the earlier proof was unsound;
marking metadata afterward cannot repair code that already relied on it.

A crash route while a shared invariant window is open derives a containment
demand wide enough to terminate everything that could observe the broken
state. In a context that promises narrower survival, that route must be
disproved before the site; merely publishing `crashes Trap` does not make the
window safe. A resource may lower this damage minimum only through an explicit
owner-death recovery protocol whose acquisition outcome forces survivors to
observe and repair the abandoned invariant. There is no ambient poisoning or
asynchronous destruction hidden from the checked graph.

Unestablished storage is different from an established value whose invariant
was later broken. If establishment fails, no `T` exists and the raw storage may
be released through its ordinary storage claim. Once `T` is established, its
cleanup and obligations may depend on its invariant, so it cannot be discarded
mid-window.

## Exclusivity Is The Borrow Checker

A window is sound because nothing can see into it, and that is the ordinary
ownership discipline, not a separate mechanism:

- Aliased mutation does not exist, so no second path reads the place
  mid-window.
- A live borrow of a dependent place pins its witnesses
  ([Chapter 12](chapter_12_dependent_types.md)): a loan is continuous
  observation, so a write to a pinned witness while the loan lives is a
  borrow error — the window cannot even open.

## Gated Types Are A Window Since Birth

A gated type's un-established storage ([Chapter 7](chapter_7_types_constraints_invariants.md),
[Chapter 12](chapter_12_dependent_types.md)) is the same state: the domain
is not yet proven, and nothing may observe the value as the type. The
construction literal or `as` qualification is its closing consumption point.
Establishment is monotone **as observed**: a later write may open a window,
but every consumption point closes it, so no observer ever sees an
established place fall back.

## Multi-State Construction Ends At The Transition

Transitions are consumption points, so a window never spans states. A value
that cannot be made valid within one state body is constructed with temps
and moved in whole, or staged behind a gated type whose establishment
happens where the value becomes real.

## Temps And Init-Syntax Remain Good Style

```omega
machine Body::set_mass(&mut self, delta: i32) {
    let next_mass: i32 = self.mass + delta;
    self.mass = next_mass;
}
```

Temps keep invalid intermediate values out of memory entirely, and
init-syntax constructs a valid whole and moves it in atomically. Windows
make the in-place form *legal*; they do not make it preferable. Reach for a
window when a real location must be mutated in place — large structures,
buffer initialization, representation surgery — and let temps carry
everything else.

## Helpers During A Window

A call is a consumption point, so a broken whole cannot be passed to a
helper. Split the helper over the decoupled fields instead — the borrow
checker splits the paths:

```omega
machine Tree::rotate_step(left: &mut NodeId, right: &mut NodeId) { ... }

machine Tree::rotate(&mut self) {
    Tree::rotate_step(&mut self.left, &mut self.right);
}
```

Whether a whole-value mid-window helper signature is ever needed is an open
question deferred until a real case demands it.

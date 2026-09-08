# Chapter 11: Invariant Windows

Updating a coupled value can require several writes. Omega permits the default
domain to be temporarily unproved while exclusive access prevents observation,
then requires it to be restored at the next consumption point. There is no
source `invariant` or `relax` keyword and no runtime proof-debt flag.

Writes still need ordinary access, type, arithmetic, and ownership validity.
An invariant window postpones the coupled-domain proof, not those obligations.
For example:

```omega
data Span
where
    start <= end,
{
    start: u32;
    end: u32;
}

machine Span::shift(&mut self, delta: u32 [0..=1000])
requires
    self.end <= u32::Maximum - delta
{
    self.start = self.start + delta;
    self.end = self.end + delta;
}
```

The precondition makes the additions representable. The first assignment may
temporarily break `start <= end`; the second restores it before return. A
domain-preserving write needs no window. Otherwise the checker records the
actual new contents and the obligation to re-establish the domain.

[Dependent values](../spec/language/dependent_values.md#invariant-windows) owns
the exact rules. This chapter explains how to use them; it does not claim that
every relational proof pattern is implemented by today's checker.

## Consumption Points

A consumption point is a use that may rely on the value's domain:

- A read relying on a range or dependent coupling.
- Borrow creation or a call.
- A state transition, return, or scope expiration.
- A boundary/capability-carrying call, even when it does not name the place.

The checker must close the relevant windows there. If it cannot, the diagnostic
names both the opening write and the consuming use. State arrivals prove their
own contracts; a backedge cannot reuse entry facts invalidated by its writes.

## Failure Does Not Cancel A Window

Recoverable failure is an ordinary outcome, not permission to discard proof
debt. Calls and transitions still close the reachable windows. Once a value is
established, its cleanup and obligations may rely on its invariant, so it cannot
simply be thrown away mid-window.

Failed initial establishment is different: no value yet exists, and the raw
storage remains under its storage claim. A crash may abandon an open window,
but abandonment evidence proves no survivor safe. Continued use needs an
independent closed-custody recovery boundary or an explicit owner-death protocol
that forces recovery before reuse. See
[crash semantics](../spec/terminal-psi/calls_and_outcomes.md#crash).

A later metadata change cannot repair an earlier false admission or hardware
violation. Proof evidence records why a fact was trusted; it is not a mutable
truth bit that can retroactively revoke earlier reasoning.

## Exclusivity Is The Borrow Checker

Ordinary ownership excludes another observer while a window is open. A live
borrow of a dependent place also pins the witnesses on which its validity
depends. A conflicting witness write is therefore a borrow error: the window
cannot open in the first place.

Write-only access is exclusive but cannot inspect the referent. It must restore
validity from written inputs, static structure, and supplied facts. A cross-field
coupling may therefore require whole-value replacement through `&write`, whereas
independent elements can admit smaller writes. See
[write-only access](../spec/language/ownership.md#borrows-and-aliases).

## Gated Types Are A Window Since Birth

A gated type's zeroed storage is not yet an established value. Construction or
checked qualification proves its default domain before observation. That resembles
an open window, but differs in ownership: failed construction has not created the
value or its value-specific cleanup duties.

After establishment, later mutation may open a window, but every consumption
closes it. No observer sees the established value fall back to invalid storage.
[Default domains](../spec/language/dependent_values.md#default-domains-and-zero-initialization)
owns gating and containment.

## Multi-State Construction Ends At The Transition

Windows do not cross state transitions. If a value cannot be made valid in one
state body, carry independent temporary values or unestablished storage and
establish the whole where it becomes observable. A state signature is not a way
to pass a broken established value onward.

## Temps And Init-Syntax Remain Good Style

Building a valid replacement before installing it can keep invalid intermediate
values out of the destination altogether. In-place windows are useful when a
real location must change, such as a large structure or buffer. They make that
update legal; they do not require it when a simple whole-value construction works.

## Helpers During A Window

A helper cannot receive a broken whole as if its default domain held. A narrower
signature can operate on independent fields when their own borrow and consumption
obligations are satisfied:

```omega
machine Tree::rotate_step(left: &mut NodeId, right: &mut NodeId) {
    ... // field update omitted
}
```

Narrow parameters make the helper's reachable state explicit. They do not waive
a coupling or a witness loan. The current model supplies no whole-value
mid-window helper escape; a concrete need for one would require separate design.

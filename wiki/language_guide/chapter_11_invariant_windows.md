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
requires
    start <= end,
{
    start: u32;
    end: u32;
}

machine Span::shift(span: &mut Span, delta: u32)
requires
    delta <= 1000
    span.end <= integer::max<u32>() - delta
{
    span.start = span.start + delta;
    span.end = span.end + delta;
}
```

The carrier query uses the ratified ordinary core
[integer API](../spec/language/numeric_values.md#core-integer-and-float-apis),
whose implementation remains `CORE-INTEGER-FLOAT-APIS` work rather than a
primitive-member alias. The precondition makes the additions representable. The first assignment may
temporarily break `start <= end`; the second restores it before return. A
domain-preserving write needs no window. Otherwise the checker records the
actual new contents and the obligation to re-establish the domain.
The mutable access comes from the explicit `span` parameter, not from the
qualified machine name. A caller supplies it as `Span::shift(&mut span, delta)`.

[Dependent values](../spec/language/dependent_values.md#invariant-windows) owns
the exact rules. This chapter explains how to use them; it does not claim that
every relational proof pattern is implemented by today's checker.

## Consumption Points

A consumption point is a use that may rely on the value's domain:

- A read relying on a range or dependent coupling.
- A borrow or call consuming the affected domain.
- A transfer, return, scope expiration or loan handback that releases custody
  or supplies the value under that domain.
- A boundary/capability call that may expose the affected storage, including
  through authority not named as an argument.

The checker must close the relevant windows there. If it cannot, the diagnostic
names both the opening write or storage cast and the consuming use. Machine
arrivals prove their own contracts; a backedge cannot reuse entry facts invalidated
by its writes. A borrower's internal repair transfer can retain the lender's
restoration obligation under the receiving contract, as explained below.
A call proved unable to observe, expose or interfere with the broken coupling
need not close its window. That requires complete access/effect/outcome evidence;
merely writing nothing is insufficient. Unknown effects remain conservative.

## Lending Does Not Erase The Owner's Domain

Suppose a library declares `Ascii` as a domain whose predicate excludes bytes
above 127. A local `bytes: [u8; 2] in Ascii` must still satisfy that predicate
when a mutable loan returns its storage. A helper may temporarily break it under
the ordinary window rules, but returning 255 in the array without restoration
is invalid. Giving the helper an unqualified view does not erase that debt.

This differs from an unqualified array lent to a machine that merely requires
ASCII contents on entry. That extra input fact may expire unless the machine
promises to preserve it. The original storage declaration determines what must
be restored; neither `Ascii` nor any other encoding name is a compiler builtin.

A domain without predicates has no predicate to restore. If its correctness
depends on controlled establishment or mutation, its declared routes and access
policy enforce that protocol and cannot be bypassed by a cast. An empty,
unrestricted tag by itself makes no such promise. See
[lender-domain restoration](../spec/language/dependent_values.md#lent-storage-retains-its-declared-domains).

## Failure Does Not Cancel A Window

Recoverable failure is an ordinary outcome, not permission to discard proof
debt. Calls and transitions must satisfy their receiving contracts and preserve
any delegated restoration obligation. Handback still requires restoration. Once a value is
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

## Storage Casts Can Open Initial Establishment

An exclusive `as` storage cast may open a window before the target is a valid
value. This is useful for constructing a constrained boundary-data header in
fresh backing. The cast establishes neither the domain nor authority: alignment,
extent, write permission and exclusive custody must already hold. Initialize or
validate the contents before ordinary typed use; target-shaped storage is not
permission to read invalid enum tags or references.

The cast's pending-view formation is allowed, but later consumption fences remain.
No shared reader, valid-value call, return or IPC publication can observe the
unfinished target. Ending the loan owes the original source's validity/custody.
Abandoned initialization owes cleanup of any established resource-bearing fields,
not a destructor for a whole value that never existed. See the
[recast rules](../spec/layouts/recasts.md#deferred-establishment-on-exclusive-storage-casts)
and [byte-backed header example](chapter_20_memory_layout_abi.md#initializing-a-stricter-shape-in-byte-backing).

## Construction Windows Do Not Cross Terminal Transfers

An unfinished whole cannot cross a terminal transfer as a valid value of its
declared type. Carry independent temporary values or unestablished storage and
establish the whole where it becomes observable. A successor's `ensures` does
not excuse failing its parameter's default domain on entry. This differs from
delegating a named qualification's repair through an already valid weaker view.

## Repair Through Existing Contracts

`requires` describes what a repair step can assume; `ensures` describes what it
restores. A helper can accept a structurally valid tree with one imbalance and
promise balance on completion. An enclosing operation can use that guarantee
to fulfill its own promise, including when the helper is a terminal successor.
The original lender's restoration obligation remains attached throughout.

| Guarantee | What must hold |
| --- | --- |
| Data default domain | Every ordinary receiver of that type may assume it at entry. |
| Stored named qualification | The owner receives storage satisfying it back; a weaker borrowed repair view does not erase the duty. |
| Operation `requires` / `ensures` | The stated fact holds on entry / the matching completed outcome. An entry-only fact may expire. |

This uses existing contracts, not a new open/close keyword. Your code performs
the repair; the checker composes its evidence. After a write, fresh proof of a
predicate-only domain's conditions can re-establish membership. A guarantee
about another object, an old value or a different outcome cannot do so.

For example, balance may be a named qualification of a tree whose default domain
describes structural validity. A weak repair view still satisfies that default
domain, and a helper promising balance can restore the qualified owner's loan.
If balance is instead part of the default domain, every `&mut Tree` receiver
requires balance already. The repair rule does not silently weaken that type.

These rules are ratified; compiler conformance and regression coverage remain
tracked by **DOMAIN-ACCESS-AUTHORITY** and the
[repair-facts plan](../drafts/designs/invariant_repair_facts.md). No compile-time
speedup or runtime repair is implied by this clarification.

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
machine rotate_step(left: &mut NodeId, right: &mut NodeId) {
    ... // field update omitted
}
```

Narrow parameters make the helper's reachable state explicit. They do not waive
a coupling or a witness loan. A whole-value helper must receive a valid value
under its parameter type; a weaker view can delegate a lender's named-domain
repair only under the contract and custody rules above.

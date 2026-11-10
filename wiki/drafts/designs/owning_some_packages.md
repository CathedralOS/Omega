# Owning some packages

Ratified 2026-11-08 as documentation. A `some S` package may own its hidden
state through a uniform carrier, closing the "owning arbitrary erased state is
separate work" deferral in
[some packages](../../spec/language/some_packages.md#owned-state-through-uniform-carriers).
No compiler support or fixture outcome is claimed here. Delete this record when
`OWNING-SOME-PACKAGES` on [TASKS.md](../../../TASKS.md) lands and the spec
section suffices.

## Context and gap

The borrowed form admits `state: &'a mut S` and descriptors over it. A
reference to a helper's local cannot survive the helper through packaging, so a
factory could not return an owned opaque object: the real owner had to live in
a registry, in caller-supplied storage sized for a type the caller is not meant
to know, in an enumerated sum, or in a task. The boundaries this blocks are the
ordinary ones: a host that owns heterogeneous processors from independent
factories, and a job queue that accepts heterogeneous jobs with linear payloads
and later runs or cancels them. The bounded-worker pattern in
[chapter 18](../../language_guide/chapter_18_concurrency.md) is that queue.

`Task<T>` is already an owned erased value: hidden state, code selected while
the state was known, accountable storage and explicit linear settlement. The
owning package is that relationship made available to ordinary data, without
the execution.

## Decisions

1. Keep `some`. No new binder, keyword or privileged type.
2. The delta is one admitted field form: a by-value field whose type owns the
   witness through a uniform carrier. `&mut Owner<S>` was already admitted as a
   reference to a sized referent.
3. Uniformity is an inductive structural judgment. The witness may occur only
   in custody or evidence indices or behind admitted references, never in a
   layout position, and the judgment covers every parameter and result of every
   stored callable. `machine() -> Owner<S>` passes where `Owner<S>` is uniform;
   `machine() -> S` rejects, as V1 already requires.
4. The carrier is library data over `Extent in Granted & Resident<P, S>`. The
   qualified extent owns one complete live `S` and returns to its provider; an
   allocator family wraps it with its return handle where one is needed. The
   first owner family lands in `alloc` as the first customer.
5. Packing allocates nothing. It moves an established owner into the record.
6. Stored operations take the owner, by reference or by value, not `&mut S`.
   The consumer never specializes over the hidden type.
7. Each stored operation keeps its own callable contract, checked at
   construction as for any machine value. One operation's reach or suspension
   does not attach to another because they share a record.
8. Linear first. Consumption is any stored operation taking the owner by value.
   There is no universal `settle`; a package may offer several consuming
   operations, such as `run` and `cancel`. The package stays `[linear]` even
   when a particular hidden state would be affine.
9. An abstract witness introduced by a valid opening may construct another
   package within its scope under the ordinary checks. It grants no equality
   with another package's witness and extends no contained loan.
10. Pending or failure outcomes return the remaining ownership as a package
    exposing only the operations still valid in that state. A half-closed
    reader comes back as a pending-settlement package, not a reusable reader.
11. Automatic cleanup needs no mechanism of its own. Under
    [owned cleanup consumers](owned_cleanup.md) the package's owner
    selects an ordinary `dropped by` machine that takes the package by value,
    takes it apart with a consuming pattern and calls a stored consuming
    operation on the hidden owner.
    Eligibility is checked from that operation's callable contract. Storing a
    consuming operation admits no disposal by itself, and the package stays
    `[linear]`.

Specify before the queue, supervisor and callback library surfaces settle.
Implement alongside or after `RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY`,
never ahead of the borrowed machinery it builds on.

## Reused and new

| Reused as written | New |
| --- | --- |
| `Extent`, `Granted`, `Resident<P, T>` custody and retirement | one admitted field form under `some` |
| machine-value callable contracts and construction checks | the uniformity judgment stated inductively |
| pending or failure outcomes retain the live input | opened witnesses as construction witnesses |
| component-boundary and era rules for descriptors and state layouts | |
| carry axes across suspension | |

## Sketch

```omega
data Job<P, R, some S> [linear] {
    state: Extent in Granted & Resident<P, S>;
    run: machine(state: Extent in Granted & Resident<P, S>) -> R
        reaches none
        crashes none;
    cancel: machine(state: Extent in Granted & Resident<P, S>)
        reaches none
        crashes none;
}
```

A producer establishes the resident with its allocator, selects `run` and
`cancel` over the concrete state, and constructs the record. A consumer opens
the job and transfers `state` to exactly one of them; that operation knows the
concrete owner and returns the storage to its provider. An owner family that
must name a specific provider wraps the extent with that provider's handle; the
field stays uniform because `S` occurs only in the custody index. Inline
`state: S` stays out because no single layout represents it.

## Acceptance

- Two factories return one external package type with different hidden states.
- One container holds both; ownership transfers through insertion and removal.
- A hidden zero-sized linear obligation must still be settled.
- Two allocations of the same payload from different pools; the wrong return
  path rejects.
- A captured borrow prevents the package from outliving its source.
- A thread- or suspension-restricted hidden resource blocks an incompatible
  crossing.
- A close failure returns either a valid reusable package or a distinct
  pending-settlement package.
- A live returned view prevents premature consumption.
- A uniform descriptor whose result is bare unknown-layout `S` rejects.
- Repacking after failure cannot drop an obligation or restore an interface
  whose invariant no longer holds.

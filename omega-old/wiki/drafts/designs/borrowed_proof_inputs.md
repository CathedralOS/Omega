# Stable reads of borrowed proof inputs

Ratified 2026-11-12 as `shared-observation-denotational-content`. A predicate
may read slice metadata, ordinary fields beside references, or data reached
through shared references when each read is authorized and stable. The presence
of a reference in the argument type does not by itself forbid the predicate.
Admitting the invocation does not prove that its Boolean result is true.

The rules live in [proof admission](../../spec/proofs/contracts.md#borrowed-proof-inputs)
and [fact validity](../../spec/terminal-psi/fact_validity.md#borrowed-read-dependencies).
Remaining implementation belongs to **PROOF-KERNEL-CORE** in
[TASKS.md](../../../TASKS.md). This document records required behavior and
acceptance, not fresh execution results or a completed compiler change.

## Read dependencies

The first example asks only about a collection's size; the second reads an
integer reached through a field. In descriptive notation, not new source syntax:

```text
Held<'a> has slot: &'a u64

has_two(items: &[Held<'a>]) = (items.len == 2)
    observed input: items.len
    no observation of element contents or slot referents

slot_positive(held: &Held<'a>) = (held.slot > 0)
    observed input: slot's referent identity and its contents version
    retain relevant intermediate field/path selections
```

Both predicates are admissible with their ordinary checked termination, empty
reach, failure/authority and valid-access conditions. There is no implicit copy
of the elements or whole reachable graph. Unread fields still retain ordinary
input validity, loan, lifetime, access, qualification and custody obligations.

Apply the same rules through records, arrays, slices and nested paths, including
helper calls. Use existing place, loan, version and call-summary mechanisms;
do not add a per-container proof exception. A helper's summary must include its
actual storage dependencies under the ordinary interface rules. Unknown or
incomplete dependencies are conservative, never an empty read footprint.

## Stability and fact reuse

If the referenced integer changes from 7 to 0 after a permitted write, the
reference may still identify the same storage. A fact about the old value does
not establish positivity of the current contents. Pointer identity, source
expression spelling and empty reach cannot replace storage/version agreement.
Writes through potentially aliasing paths and helper calls obey the same rule;
proved-disjoint writes preserve unaffected facts.

Shared loans provide their ordinary scoped stability, not permanent immutability
of every reachable place. A proof may reason about an authorized captured value
after storage changes, but must keep that value distinct from a later read.
The checker may not invent reaching-value evidence, renew an expired loan or
forget an overlapping write to reuse a call result.

Stable RAM reads are distinct from External/device and Atomic observations
under [access contracts](../../spec/resources/access.md). Shared access alone
cannot make those observations repeatable. Use an admitted runtime observation
and its captured result, or an applicable established stability contract.
Atomicity alone establishes no equality between separate reads.

Symbolic proof admission does not authorize constant materialization, exporting
runtime pointers, introducing reference/slice generic atoms, extending loans or
copying a borrowed graph into the evaluator. Existing staging and reference
escape restrictions remain. No keyword, special reference type or mandatory
deep snapshot is added.

## Acceptance

- Migrate the existing `fail/layouts/fixed_layout_policy_borrowed_content` to a
  positive control, or retain its program as an equivalent positive witness.
  Its proof reads only `storage.value`; `Payload.left` is unread. Layout is
  not an admission exception. Preserve both matching and nonmatching values
  so accepting the predicate never means assuming its result is true.
- Migrate the existing
  `fail/facts/computed_subslice_extent_nested_reference_elements_rejected`
  program to a positive control for `items.len == 2` over reference-bearing
  elements. Reuse the frozen fixture sources rather than adding new facts files.
  Element/referent writes that leave the metadata and input validity intact do
  not create an observation dependency for a known length-only predicate.
- Qualify a predicate that actually follows a shared reference and reads the
  integer, then nested paths through records, array/slice elements and helpers.
  Each path must retain exact access, loan, backing and observed-version evidence.
- Reusing a prior positive result after a permitted overlapping write from 7
  to 0 must not prove a current positive read. Include aliasing and helper-write
  controls, unchanged-reference carriers, expired loans and changed intermediate
  reference selections. Captured earlier values remain distinguishable from
  current storage. Proved-disjoint writes retain the unaffected fact.
- Helper summaries cannot erase dependencies; a missing/unknown summary must
  not act as an empty footprint or permit stale-result reuse. Published and
  local summaries retain their ordinary scope and admission rules.
- Unaccounted External/device/Atomic reads cannot enter pure proof reasoning
  merely through shared references. Invalid access and lifetime, forbidden
  resource duplication, reference escape and runtime-to-constant pointer uses
  retain their rejection controls. A valid captured observation may still be
  used under its own contract.
- Carry the same dependencies through source checking, proof construction,
  semantic evaluation where admitted, lowering, artifact receipt and native/Psi
  execution of executable witnesses. Do not repair only the direct-record case
  while leaving slices, arrays or helpers on a type-shape blacklist.

These are delivery obligations for the implementation, not instructions to
weaken stale-fact controls or update corpus ledgers in this ratification PR.

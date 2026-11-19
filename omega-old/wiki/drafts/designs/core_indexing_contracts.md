# Core indexing contract migration

Owner-ratified on 2026-11-05. This is the implementation plan for
**CORE-INDEXING-CONTRACTS** in [TASKS.md](../../../TASKS.md), not an open proposal.
The normative owners are [indexing and ranges](../../spec/language/expressions.md#contract-gated-core-operations),
[generic sealed primitives](../../spec/language/machines.md#generic-sealed-primitives),
[collection bounds](../../spec/language/numeric_values.md#collection-bounds),
and [Terminal structural access](../../spec/terminal-psi/structural_access.md).

## Outcome

Bracket calls must prove the selected core operation's whole contract through
ordinary call-contract checking, then lower directly to the sealed primitive.
A separate indexing recognizer must not decide which requirements matter.
Named helpers remain ordinary library code; basic access requires no forwarding
call or optimizer inlining.

The scope includes by-copy reads with unrestricted elements, shared element
borrows, mutable access and writes, shared and explicitly mutable ranges,
fixed arrays including const extents, slices, and bounded byte carriers.
Preserve existing overload selection, bare-range borrowing, evaluation order,
origin/lifetime, loan, restoration and numeric rules. This does not introduce
return-type-directed overload selection or move non-copy elements through a
by-copy read.

## Starting implementation

The Rust reference compiler admits declaration-free builtin brackets; the
lowered-form checker, the only checker, owns steps 3 and 4 below. The target
is to accept a well-formed declaration and prove every requirement at its
calls. Generic/lifetime-bearing sealed catalog rows are a prerequisite.

`slice_index` and `slice_range` have named callers; their `slice_views` helpers
use brackets internally. `SliceAccess` has no discovered callers or
implementations. These wrappers do not currently own builtin indexing's
authority. The immediate migration is in the Rust producer and bundled core;
the Omega-written compiler must implement the same ratified contract when it
adds indexing. Other-language bootstrap rungs are not being redesigned.

## Delivery order

1. Admit generic sealed rows. Normalize type, const and lifetime binders and
   the full contract under exact toolchain source/package custody. Test binder
   renaming, correct instantiation and borrowed-result lifetime substitution.
   Reject forged same-name/same-signature declarations, mismatched constraints,
   contracts and binder kinds. Do not admit arbitrary bodyless user machines.
2. Declare the core access families and resolve brackets to exact declarations.
   A transitional resolved selection is identity plumbing, not completion of
   contract ownership. The final path has no declaration-free fallback and no
   mandatory checked wrapper. Preserve existing named helper customers.
3. Discharge each instantiated requirement through the lowered-form checker's
   ordinary call-contract checking. A requirement it cannot prove refuses the
   call; no failure or unknown result becomes assumed evidence, and proving the
   bounds never bypasses another unproved requirement.
4. Give the lowered-form checker the facts and entailment that indexing
   customers need: const-generic extents, captured/hoisted selectors,
   assignment-length changes, result guarantees, ordering chains, range
   formation and inclusive-end arithmetic. Preserve exact value versions,
   disjointness and overlapping-write invalidation. Make these conclusions
   usable by named calls and bracket calls alike.
5. Map each discharged indexing occurrence to its own Terminal `RuntimeIndex`
   obligation, retaining its selected contract, exact actuals and bound
   evidence. A membership bit alone is not a certificate; keep access/loan
   evidence separate. Do not replace evidence transport with repeated proof
   search or change verification placement.
6. Remove unused `SliceAccess`, collapse redundant `slice_views`, and retain
   named helpers as ordinary checked calls to the selected core operations.
   Update the guide and library implementation notice when delivery is
   complete.

## Acceptance and measurement

- A test index operator with an extra well-formed requirement is accepted as a
  declaration. Calls proving the whole contract pass; calls proving only bounds
  reject at the call. Both direct named calls and bracket calls use that contract.
- Prove the same bounds through caller requirements, branches, returned
  guarantees and checked theorems. Wrong-view, stale-value, signed-negative,
  invalid endpoint, overflow and conflicting-loan controls remain rejected for
  their intended reason. A forged primitive cannot bypass any obligation.
- Reuse the existing slice, array, range, byte-view and contract fixtures and
  samples, including `field_view_index_contract`,
  `runtime_imported_slice_provider_composition_exit`,
  `imported_range_retains_backing_loan` and `computed_subslice_extent`.
  Exercise the repository's nine value placements without adding another facts
  matrix. Verify Terminal/native agreement on all four hosted targets and
  confirm primitive accesses do not introduce per-element forwarding calls.
- Check emitted evidence identity against altered selectors, extents and sibling
  views using the existing receiving verifier tests. Preserve both numerical
  obligations and storage authority through the Terminal and native forms.
- Attribute fixture movement against the fixed base. An invalid-contract
  fixture becoming accepted is a regression; a safe proof-limit fixture becoming
  accepted requires explicit semantic review and appropriate reclassification.
  Do not re-pin unexpected outcomes to hide regressions.
- Measure per-function checker/entailment cost with existing attribution
  tools. Keep common constant and runtime-bound accesses cheap;
  correctness, complete contract checking and preserved evidence are mandatory.

Completion means the declared contract owns admission across this whole scope,
existing positives and intended negatives retain their semantics, and the old
indexing-only admission path is gone. Merely changing selection status or moving
the same privileged checker behind a new function is not completion.

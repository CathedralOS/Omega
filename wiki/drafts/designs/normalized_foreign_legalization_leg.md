# NORMALIZED-FOREIGN-LEGALIZATION — resume recipe

Board row: `TASKS.md` `**NORMALIZED-ABI-LOWERING.**` (P4) — the "finish
aggregate and descriptor foreign transport" leg. Draft for
`NEW-NAL-LEGALIZATION-RESUME-RECIPE`, recorded on linux x86-64 against
`2dbfecd98e` (origin/main), claims snapshot ~2026-09-21T10:0xZ.

The recipe exists because the implementing surfaces are live-fenced, not
because the work is unclear: at snapshot, both mirror files sit inside
`NEW-NF-LEGALIZER-AGGREGATE-SOURCE-CUSTODY` (Devin / z56, expires
2026-09-21T13:34Z). Resume by claiming that surface again (or coordinating
with its holder) — `claim` on the two paths returns exit 2 while it is live.
Delete this recipe once legalization and selection admit the owned-aggregate
and descriptor argument classes with their mirrored witnesses.

## Landed state — do not rebuild

The a2t admission contract is complete and witnessed. A normalized foreign
call's structural formal is classified at
`02_abstract-operations-to-target-operations/src/validation/structural_call_arguments.rs::normalized_foreign_call`
(~:1101) against the calling policy using
`validation/structural_shapes.rs::boundary_formal_shape` (:372):

- `StructuralAccess::Owned` → `classified_boundary_shape` (:407) — the
  policy's by-value aggregate classification (scalar-leaf classes under
  AAPCS / SysV eightbytes / one integer view under Microsoft x64). Sources:
  the caller parameter's declared `Placement` (multiplicity `Unrestricted`),
  or `TargetStructuralArgumentSource::StructuralHome { psi_operation }` —
  the result of a dominating `Call` in the same block carrying
  `TargetStructuralHomeLayout::Aggregate` whose shape equals the call plan's
  result placement, with `Affine` result multiplicity and no
  qualifications/claims (:1546-1600). The home must be unique and must not
  be the consuming call itself.
- Borrowed (`Shared`/`Mutable`/`WriteOnly`) `ByteSequence(BorrowedView)`
  formal → `reconstruct` (:24) — the stored two-word descriptor crosses by
  value under the aggregate class. Sources: the whole stored view (empty
  path) or a stored descriptor field via `borrowed_view_field_offset` (:286).
- Any other borrowed formal → one pointer-width word (the case both mirrors
  already carry).

a2t witnesses, all in
`02_abstract-operations-to-target-operations/src/tests/normalized_foreign_calls.rs`:
`normalized_foreign_owned_aggregate_arguments_replay_across_native_targets`
(:1885), `normalized_foreign_borrowed_view_descriptors_replay_whole_place_and_stored_field`
(:1931), `normalized_foreign_owned_and_descriptor_arguments_reject_substituted_rows`
(:1971), `normalized_foreign_owned_aggregate_from_call_result_replays_affine_home`
(:2247).

## Where it stops today

A source-produced scalar+record foreign call reaches
`Selection(Legalization(SourceCustodyMismatch))`. Both mirrors admit only
the borrowed single-pointer class:

- `03_target-operations-to-selected-instructions/src/legalization/scalar_graph_input/normalized_foreign.rs::structural_argument_at`
  (:148) — requires a non-empty all-`Field` path, borrowed access,
  `Unrestricted` multiplicity, empty qualifications, and a destination of
  exactly one pointer-width integer word; `source` is always the caller
  parameter `Placement`. Owned whole-place, owned-from-call-result, and
  `BorrowedView` descriptor arguments all fail closed here.
- `03_target-operations-to-selected-instructions/src/selection/scalar_call_abi/normalized_foreign.rs`
  — `plan_operand_views` (:43) accepts only single-word register/stack
  placements (`byte_size == shape.byte_size`), so a multi-word by-value
  aggregate or two-word descriptor destination yields no catalog row;
  `validate` (:180; structural loop ~:264) re-derives the same borrowed-envelope: field-only path,
  borrowed access, `shape == borrowed_reference(projected)`,
  `source == Placement(...)`, `destination.shape == integer(pointer)`.

## Resume steps

1. Legalization admission (`structural_argument_at`). Dispatch on
   `semantic.access` before the borrowed-only checks:
   - `Owned` + empty path → recompute the root shape via
     `structural_reference_input::shape`, require the destination to equal
     the plan's `boundary_formal_shape`-classified placement (aggregate
     class, matching byte size and alignment — never `borrowed_reference`),
     and accept `source` = caller `Placement` or the a2t-shaped
     `StructuralHome` producer (dominating same-block `Call`, unique
     aggregate home, `Affine` result, no qualifications). The vocabulary
     already exists: `TargetStructuralArgumentSource::StructuralHome` at
     `target_operations/values/structural.rs:59`.
   - Borrowed `ByteSequence(BorrowedView)` formal → empty path (whole stored
     view) or field path terminating in a stored descriptor field;
     destination = the plan's reconstructed two-word descriptor placement.
   - Keep failing closed: non-`Field` interior segments, qualifications on
     the declaration, multiplicity mismatches (`Unrestricted` for borrowed
     and `Placement`-sourced owned; `Affine` for `StructuralHome`), and any
     plan/substitution drift.
2. Selection operand views. Extend `plan_operand_views` past the
   single-word envelope: emit per-word `Register`/`Stack` operand views for
   multi-word by-value placements in the plan's canonical bank order; keep
   the callback's private slot excluded from the view list (materialization
   bytes own it) and keep the unique-row match in `call_key`.
3. Selection `validate`. Re-derive the widened envelope structurally, not
   from claims: replace the borrowed-only `shape == borrowed_reference` /
   `destination == integer pointer` checks with the
   `boundary_formal_shape` classification mirrored crate-locally, plus the
   `StructuralHome` source case (dominating producer, unique, `Affine`).
   `structural_parameter_positions` stays: operand count must still equal
   plan parameters minus scalars minus the callback slot.
4. Witnesses. Mirror the four a2t tests in
   `03_target-operations-to-selected-instructions/src/tests/legalization/normalized_foreign.rs`
   (borrowed flat-record precedent: `flat_record_lane_projects_source_rooted_borrow_and_replays`,
   :363) — owned whole-place replay across native targets, stored view and
   stored descriptor field replay, owned-from-call-result affine home, and
   substituted-row rejection. Then the matching-host source leg in the
   `source_evaluated_native_realization` family — today scalar-only and
   macOS ARM64-gated (`compiler/tests/source_evaluated_native_realization/scalar_native_arguments.rs:5`);
   the row's acceptance needs a scalar+record foreign call executing on a
   matching host.

## Scoped checks

```text
cargo nextest run -p target-operations-to-selected-instructions --lib
cargo nextest run -p abstract-operations-to-target-operations --lib
mbx nextest run -p compiler --test source_evaluated_native_realization --no-fail-fast \
  --no-tests fail -E 'test(~normalized_foreign)'
```

## Not this leg

Dynamic descriptor calls stay under
**RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY**; callback transport stays
under **CALLBACK-PRIVATE-MATERIALIZATION**; cross-target selection replay is
not runtime evidence for the matching-host acceptance.

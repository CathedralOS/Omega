slice_0089

| entry id | file:line | verdict | one-line reason |
|---|---|---|---|
| A0680 | CO:3156-3173 | REACHABLE | Linear root in `owned_places` can be dropped after cleanup when empty-claim `validate_owned_claim_frontier` succeeds with `&\|_, _\| true`. |
| A0681 | CO:3490; CO:3582 | CLOSED | `unit_validation` rejects empty blocks before ownership replay reaches cleanup ordinals. |
| A0682 | CO:6461; CO:1368-1406 | CLOSED | Unlisted ops still go through `pins.check_operation` using the same `operation_observations` arms as descriptor loans. |
| A0683 | CO:5824-5826; CO:2518 | UNDECIDED | No-contract `discard_subtree` no-op is real; whether a valid unit can reach it with inconsistent live state is not fully traced. |
| A0684 | CO:4302-4308 | CLOSED | Missing effect catalog does not skip `validate_normal_return`’s expected-vs-live comparison. |
| A0685 | CO:6008-6019 | CLOSED | `psi_operation()` is `None` only on control/completion ops with no operation-result producer id. |
| A0686 | CO:6636-6669 | REACHABLE | Post-match overlap loop is skipped when both operands are `CallRegion::OrdinaryStorage`. |
| A0687 | CO:7458-7460 | CLOSED | Location-less atomics are admitted earlier only without resident obligation; those need no storage authority check. |

### A0680

1. Read `validate_return_cleanup_actions` in full at `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership.rs:3136-3191 (read).
2. Read `validate_owned_claim_frontier` at `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural.rs:876-1001 (read).
3. Search: `rg validate_return_cleanup_actions /agent/repos/omega/omega-rust` → 3 hits (definition, import, single production call at CO:8556) (read).
4. Caller chain read: replay `O::Return` / `O::ReturnUnit` / `O::ReturnStructural` → `cleanup_frontier` → `validate_return_cleanup_actions` (CO:8555-8577); then `reject_live_linear_claim` only on `O::Return` (CO:8572-8573) (read).
5. Input to the accepting branch: after `apply_edge_cleanup_actions`, a `StructuralMultiplicity::Linear` entry still in `remaining.owned_places`, no `partial_custody_paths`, no `claims` with `input == Some(place)`, and `place_structural_type` is `Some` such that `validate_owned_claim_frontier(..., std::iter::empty(), &|_, _| true)` returns `Ok(())` (CO:3156-3172, structural.rs:931-936 when `owns_linear_claim` is false with empty accounted claims) (read).

**Verdict:** REACHABLE — a function can finish return cleanup with a linear owned root still logically unconsumed while `owned_places` is cleared because the empty claim frontier validates, and no claim-based return check fires (`reject_live_linear_claim` only inspects `claims`, CO:3766-3777).

**Unsure:** Whether every linear root in `owned_places` must have `owns_linear_claim` true in production units; I did not read all unit builders that populate cleanup actions.

### A0681

1. Read `consume_affine_cleanup` discard paths at CO:3488-3507 and CO:3580-3591 (read; function name from enclosing context CO:3376+).
2. Read `FunctionControlFlow` construction at `unit_validation.rs:5679-5684` (read).

**Input:** DiscardResidual / nominal cleanup uses `block.nodes.len().saturating_sub(1)` as the operation ordinal (CO:3490-3491, CO:3581-3582).

**Verdict:** CLOSED — empty blocks error at `unit_validation.rs:5680-5684` (`EmptyBlock`) before ownership replay; sat_sub(1) on len 0 is therefore not on the validated unit path.

**Unsure:** Nothing material.

### A0682

1. Read `apply_operation` through the `_ => {}` arm at CO:5995-6462 and the follow-on argument/overlap/`call_effects.stage` logic CO:6463-6681 (read).
2. Read `operation_observations` match including CO:1368-1406 and fallback CO:1448-1456 (read).
3. Read replay ordering: `value_use.observations` CO:7698, `apply_operation` CO:7699-7710, later `pins.check_operation` CO:7937-7951 (read).

**Input:** Any operation whose inner `match` hits `_ => {}` still reaches `pins.check_operation` with `operation_observations` for conflict detection.

**Verdict:** CLOSED — missing `apply_operation` access arms are not the acceptance gate; descriptor loans re-derive observations on the same operation set (e.g. `ElementViewRead` at CO:1368-1380) and `conflicted_operation` runs at CO:7937-7951 / CO:1059-1140.

**Unsure:** Whether any producer-only operation never reaches optimization-unit validation.

### A0683

1. Read `discard_subtree` in full at CO:5816-5859 (read through first error paths).
2. Read caller `discard_owned` CO:5806-5814 (read).
3. Read `consume_edge_sources` branch CO:2514-2525 (read).
4. Search: `rg 'discard_subtree\(' /agent/repos/omega/omega-rust` → 15 hits across semantics and lowering crates (read); production semantics callers include CO:3500-3507, CO:5813 (inferred from grep).

**Input:** `source` place with `structural_source_contract(function, source) == None` on a path that calls `discard_subtree` (early `return Ok(())` at CO:5824-5826).

**Verdict:** UNDECIDED — the no-op is in the code, but I did not read enough of unit construction / place contracts to prove a validated program can hold live reference subtree state while cleanup still removes owned maps.

**Unsure:** Exact place kinds that can lack `structural_source_contract` yet still sit in `owned_places` or cleanup discard paths.

### A0684

1. Read `remaining_at_return` at CO:4294-4317 (read; function body partially read to CO:4339).
2. Read caller `validate_normal_return` at CO:7379-7409 (read).

**Input:** `functions.get(&machine).and_then(Option::as_ref)` is `None` (no stored reference slot effect summary).

**Verdict:** CLOSED — early return of all live references at CO:4306-4307 still flows into `validate_normal_return`, which rejects unless `expected` and `ordered` match pairwise (CO:7396-7408).

**Unsure:** Edge case where `expected` and full `live` coincidentally match without catalog; not proved unreachable.

### A0685

1. Read guard at CO:6008-6019 (read).
2. Read `AbstractOperation::psi_operation` at `omega-rust/omega/compilation/representations/abstract-operations/src/operations.rs:1089-1179 (read).

**Input:** Operations with `psi_operation() == None` (jumps, returns, `StructuralCase`, etc., operations.rs:1171-1178).

**Verdict:** CLOSED — those variants have no operation-result producer id to pin-check; disturbed-pin checks for shared state still run via `pins.check_operation` (CO:7937-7951).

**Unsure:** Whether any invalid pin could exist without corresponding `StructuralPlaceKind::OperationResult`.

### A0686

1. Read overlap loop CO:6633-6669 (read).
2. Read ordinary-argument normalization CO:6623-6630 (read).
3. Read pre-call `value_use.moving` / `observing` for call args CO:7911-7916 (partial read; did not read `moving` overlap semantics in full).

**Input:** Two call arguments both classified `CallRegion::OrdinaryStorage` with non-shared accesses.

**Verdict:** REACHABLE — the overlap predicate requires at least one side not `OrdinaryStorage` (CO:6644-6645), so two ordinary non-reference operands are not mutually checked here.

**Unsure:** Whether descriptor loans or call staging always catch overlapping mutable ordinary operands.

### A0687

1. Read `validate_atomic_access` CO:7447-7492 (read).
2. Read upstream atomic admission `unit_validation.rs:1925-1938` (read).

**Input:** `O::AtomicEvent` with `event.location() == None`.

**Verdict:** CLOSED — units with location-less atomics and `resident_obligation` are rejected at `unit_validation.rs:1931-1937`; allowed location-less events skip storage authority by design (CO:7458-7460).

**Unsure:** Nothing material.

### What I did not read

- Full bodies of `remaining_at_return` after CO:4339, `call_effects::stage`, and `conflicted_operation` helper internals beyond CO:1140.
- Entire replay loop after CO:7720 for every post-operation owned-place guard.
- All fifteen `discard_subtree` call sites outside `current_ownership.rs`.
- Unit producer invariants guaranteeing cleanup actions vs linear owned roots.

SLICE COMPLETE

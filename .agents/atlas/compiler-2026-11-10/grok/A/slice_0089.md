# slice_0089

Digest: area_05 (`digests/area_05.md` preamble: `CO` → `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/current_ownership.rs`). Rows A0680–A0687.

## Summary

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0680 | CO:3156-3173 | REACHABLE | Linear owned root with no claims is dropped from `owned_places` when empty-claim frontier validates; `reject_live_linear_claim` only scans live claims, not owned places. |
| A0681 | CO:3490 | CLOSED | `saturating_sub(1)` on an empty block yields node ordinal 0, but `unit_validation` rejects empty blocks before ownership replay runs. |
| A0682 | CO:6461 | REACHABLE | Default `_ => {}` in `apply_operation` skips operation-specific access arms; e.g. `ElementViewWrite` has no arm and is absent from descriptor `operation_observations` explicit retain list at CO:1368-1406. |
| A0683 | CO:5824-5826 | REACHABLE | `discard_subtree` returns `Ok` when `structural_source_contract` is missing; `consume_edge_sources` calls `discard_owned` at CO:2518 before any contract requirement on the source. |
| A0684 | CO:4302-4308 | CLOSED | Early return passes all live references through, but sole caller `validate_normal_return` at CO:7393-7409 requires equality with filtered entry references. |
| A0685 | CO:6008-6019 | REACHABLE | Pin check runs only when `operation.psi_operation()` is `Some`; `StructuralCase` and other variants return `None` (`operations.rs:1171-1178`) yet still go through `apply_operation` at CO:7699. |
| A0686 | CO:6636-6669 | REACHABLE | Overlap loop’s first disjunct is false when both arguments are `CallRegion::OrdinaryStorage`, so two non-shared ordinary storage call operands are not compared for origin overlap. |
| A0687 | CO:7458-7460 | REACHABLE | `validate_atomic_access` returns `Ok` when `event.location()` is `None`; obligation replay at UV:1931-1937 also `continue`s when location is absent and no resident obligation. |

## A0680

**Trace (read).** `cleanup::validate_return_cleanup_actions` (`CO:3136-3191`) clones the frontier, runs edge cleanup, then `owned_places.retain` (`CO:3156-3173`). A place is removed when it is `Linear`, has no partial custody path, has no live claim with `input == Some(place)`, and `validate_owned_claim_frontier` succeeds with `std::iter::empty()` claims and `&|_, _| true` (`CO:3164-3170`, `terminal-psi/semantics/src/structural.rs:876+`). If nothing remains in `owned_places`, `partial_custody_paths`, or `claims`, the function returns `Ok` (`CO:3174-3182`). `reject_live_linear_claim` (`CO:3761-3777`) only errors on live **claims** with `Linear` multiplicity, not stray `owned_places`. **Caller search:** `validate_return_cleanup_actions` — ripgrep `\bvalidate_return_cleanup_actions\b` under `omega-rust/`, count 3 (definition `CO:3136`, import `CO:7414`, call `CO:8556` via `cleanup_frontier` on `O::Return` / `ReturnUnit` / `ReturnStructural`). `O::ReturnUnit` (`CO:8574-8578`) does not call `reject_live_linear_claim` (only `O::Return` at `CO:8572`). **Input:** a unit that reaches return cleanup replay with a `Linear` owned local/root whose type does not require nominal claims under an empty frontier (no `owns_linear_claim` obligation at the root per `structural.rs:931-935`), cleanup actions that clear other state, and `ReturnUnit` or cleanup that drops the place from `owned_places` without recording a claim. **Wrong program (plain words):** a function returns while still owning a linear place that owes nothing under an empty claim frontier and never records that ownership as a live claim.

## A0681

**Trace (read).** In `consume_affine_cleanup_subjects` (`CO:3488-3491`, cited digest also `CO:3582` in `consume_nominal_cleanup`), block node ordinal is `block.nodes.len().saturating_sub(1)`; for `len == 0` this is `0` instead of failure. **Closing check (read).** `unit_validation` builds control flow with `if block.nodes.is_empty() { return Err(EmptyBlock { ... }); }` at `UV:5680-5684` before `validate_current_ownership_frontier_collecting` (`UV:9660`). Ownership replay (`mod replay`, `CO:7711+`) runs under that validator. **Caller search:** ordinal computation is internal to cleanup consumption on return paths from `validate_return_cleanup_actions` (`CO:8556`). **Verdict CLOSED:** empty blocks cannot reach CO:3490 on validated units.

## A0682

**Trace (read).** `references::apply_operation` (`CO:5995-6680+`) matches many operation kinds, then `_ => {}` at `CO:6461`; only matched arms run access/reference checks before the shared argument/overlap/`call_effects.stage` path (`CO:6463-6680`). `operation_structural_arguments` (`CO:5295-5302`) returns `&[]` except calls and `StoreStructuralField`, so default-arm operations get no operation-specific access in the first `match`. **Descriptor parallel (read).** `descriptor_loans::operation_observations` explicitly retains storage for `ElementViewRead` at `CO:1368-1381` but has no corresponding arm for `ElementViewWrite`; unlisted ops fall through to `written_or_exclusive_regions` (`CO:1448-1456`). `ElementViewWrite` does not appear in `apply_operation`’s `match` (ripgrep in `current_ownership.rs`). **Caller search:** `apply_operation` — ripgrep `\bapply_operation\(` under `omega-rust/`, count 2 (`CO:5995` def, `CO:7699` production call in replay). **Wrong program (plain words):** a suspended referent is read or written through an element-view write (or similar unlisted structural operation) that hits the default arm, so neither the `apply_operation` access arms nor the descriptor retain list at CO:1368-1406 model the referent access.

## A0683

**Trace (read).** `discard_subtree` (`CO:5816-5859`): if `structural_source_contract(function, source)` is `None`, `return Ok(())` at `CO:5824-5825` without mutating `live`. `discard_owned` (`CO:5806-5814`) delegates with prefix `&[]`. **Caller (read).** `block_parameters::consume_edge_sources` (`CO:2452+`) for owned non-unrestricted parameters with empty argument path calls `discard_owned` at `CO:2518` when `live_references` is non-empty, **without** requiring `structural_source_contract` first (contrast unrestricted branch at `CO:2473`). Further checks at `CO:2519-2525` only fail when borrows remain after discard. **Caller search:** `consume_edge_sources` — ripgrep under `omega-rust/`, count 2 in `current_ownership.rs` (`CO:2452` def, `CO:8772` call from replay edge handling). **Wrong program (plain words):** an edge binds an owned affine or linear parameter from a source place that has no structural producer contract; discard is a no-op and the transfer can proceed if no live borrow is detected.

## A0684

**Trace (read).** `call_effects::ReferenceCallEffects::remaining_at_return` (`CO:4294-4368+`): when `self.functions.get(&function.machine).and_then(Option::as_ref)` is `None`, returns `Ok(live.iter().collect())` at `CO:4306-4307` (no effect subtraction). **Caller (read).** Sole call: `references::validate_normal_return` (`CO:7379-7410`) sets `expected` from `entry_references`, filters to non-owned parameters (`CO:7387-7391`), calls `remaining_at_return` (`CO:7392-7393`), sorts both sides, and returns `Err` unless lengths and pairwise equality match (`CO:7396-7409`). Also invoked from `validate_return_cleanup_actions` (`CO:3183-3189`). **Caller search:** `remaining_at_return` — ripgrep under `omega-rust/`, count 1 in semantics crate listing (definition plus call at `CO:7393` within same file). **Verdict CLOSED:** passing `Ok(())` requires `expected` and `ordered` to match exactly; uncaught “return all live” only occurs if both sides already agree.

## A0685

**Trace (read).** Start of `apply_operation` (`CO:6007-6019`): the “operation result remains pinned” error runs only inside `if let Some(operation) = operation.psi_operation()`. `AbstractOperation::psi_operation` (`operations.rs:1089-1179`) returns `None` for `Jump`, `Conditional`, `StructuralCase`, returns, etc. (`operations.rs:1171-1178`). Replay invokes `apply_operation` for every node (`CO:7699-7710`). **Wrong program (plain words):** at a `StructuralCase` (or other `psi_operation`-less) node, live reference custody still pins an operation-result place tied to a producer, and the pinned-result guard is skipped because `psi_operation()` is `None`.

## A0686

**Trace (read).** After normalizing call arguments (`CO:6463-6631`), pairwise overlap at `CO:6633-6669`. For two arguments both classified `CallRegion::OrdinaryStorage` (`CO:6624-6630`), the leading clause `(*reference_access != OrdinaryStorage || *other_reference_access != OrdinaryStorage)` is false, so origin-overlap between two ordinary non-reference storage operands is not rejected unless dependency clauses apply (non-shared borrow dependencies at `CO:6654-6663`). **Caller search:** same as A0682 — overlap runs inside `apply_operation` called from `CO:7699`. **Wrong program (plain words):** one call passes two mutable non-reference storage operands whose origins overlap; both are tagged ordinary storage and never compared for overlap here.

## A0687

**Trace (read).** `replay::validate_atomic_access` (`CO:7447-7492`): on `O::AtomicEvent`, if `event.location()` is `None`, `return Ok(())` at `CO:7458-7459` before authority / `owned_places` checks (`CO:7482-7490`). Called after `apply_operation` on every node (`CO:7711-7717`). **Parallel (read).** Obligation establishment in `unit_validation` for `O::AtomicEvent`: `let Some(location) = event.location() else { ... continue; }` at `UV:1931-1937` when no resident obligation. **`apply_operation`** still has an `AtomicEvent` arm that only accesses when `location` is `Some` (`CO:6021-6063`). **Caller search:** `validate_atomic_access` — ripgrep under `omega-rust/`, count 2 (`CO:7447` def, `CO:7711` call). **Wrong program (plain words):** an atomic event with no storage location is accepted by ownership replay even though frontier authority for a located atomic would have been checked.

## What I did not read

- `git show 457adf4a11f:...` line-by-line diff (current tree lines matched digest citations on read).
- Full bodies of `validate_owned_claim_frontier` beyond `structural.rs:876-949`, `written_or_exclusive_regions`, and producer-side rules that forbid atomics without locations.
- `unit_validation` phases ordering beyond `UV:5680-5684`, `UV:9660`, and `UV:1931-1937`.
- Non-semantics `discard_subtree` call sites in pipeline crates (`legalization_reference_custody.rs`, `control_flow.rs`, etc.).
- Corpus samples witnessing each wrong program.

SLICE COMPLETE

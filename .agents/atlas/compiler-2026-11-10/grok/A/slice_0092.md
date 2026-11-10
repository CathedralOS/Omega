# slice_0092

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0704 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:3773-3816 | REACHABLE | `ByteSequenceWrite` skips predicate validation when `byte_write_predicates` returns `None`. |
| A0705 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:3750-3771,3816 | REACHABLE | Fixed primitive and whole-sequence writes proceed when ASCII/predicate requirements are absent or not computable. |
| A0706 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:4479 | REACHABLE | `ElementViewWrite` is not treated as mutating an owned byte source in `may_change_owned_source`. |
| A0707 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:5455-5457 | REACHABLE | ASCII freshness can be satisfied by declared qualifications on a structural result without byte evidence. |
| A0708 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:5793 | REACHABLE | Control-flow cycle rejection is skipped when `admit_cycles` is true for that machine. |
| A0709 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:6724-6727 | REACHABLE | Place availability ignores input places with no entry in the producer map. |
| A0710 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:8506 | UNDECIDED | Default match arm skips per-operation checks; no cited failure mode and downstream closure not fully traced. |
| A0711 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:7517-7550 | REACHABLE | `ByteSequenceEqual` / `ByteSequencePredicate` validate only `Whole` operands, not `Field`. |

## A0704

**Verdict:** REACHABLE (read)

**Cited site:** `unit_validation.rs:3773-3816` in `byte_predicates::validate_indexed_byte_stores` (read; digest cited `UV:3773-3817`, line 3817 is `continue` in current tree).

For `Operation::ByteSequenceWrite`, validation runs only inside `if let Some(predicates) = byte_write_predicates(...)` (`3774-3776`). When `byte_write_predicates` returns `None`, the block is skipped and the arm ends with `continue` (`3816`) — no error.

**`byte_write_predicates`** (`3724-3734`, read): `arrival_parameter` → `byte_parameter_predicates_in`, else `owned_byte_predicates`. **`owned_byte_predicates`** (`3897-3935`, read) returns `None` unless the place is an unrestricted bounded owned-byte concat result with a matching producer node, etc.

**Callers:** `rg '\bvalidate_indexed_byte_stores\b'` under `omega-rust/` → **2** (definition `3736`, call `9638`). **Callers of `byte_write_predicates`:** `rg '\bbyte_write_predicates\b'` under `omega-rust/` → **4** (all in `unit_validation.rs`).

**Upward:** `validate_indexed_byte_stores` ← `validate_function` (`9638`) ← `index_and_validate_unit_catalogs` (`1233`) ← `validate_psi_optimization_unit_with_control_cycles` (`18265`) ← public validators (`18204-18210`, `18229-18237`). Failure returns `Err` from that chain; missing predicate check yields `Ok`.

**Input:** A unit with a `ByteSequenceWrite` to a destination where `byte_write_predicates` is `None` (place not an owned concat result with computable predicates), including UTF-8 scalar-splitting writes that would fail inside the `Some(predicates)` branch — accepted here.

Digest claim on predicate `None` matches the code (read).

## A0705

**Verdict:** REACHABLE (read)

**Cited sites:** `3750-3771`, `3773-3816` in `validate_indexed_byte_stores` (read).

1. **`WriteOnlyPrimitiveStore`** (`3746-3771`): ASCII freshness is enforced only when `requires_ascii(...) == Some(true)` (`3750-3761`). `requires_ascii` (`5388-5415`, read) returns `Option<bool>`; `primitive_store_byte_predicates(...)?` makes the whole function return `None` when predicates cannot be computed. `None` and `Some(false)` skip the error path and `continue` (`3771`).

2. **`ByteSequenceWrite`:** Same as A0704 — `byte_write_predicates` `None` → no check, `continue` (`3816`). Digest text about destinations whose byte predicates cannot be computed matches (read).

**Callers:** same as A0704 (`validate_indexed_byte_stores` → **2** sites).

**Input:** A unit with a fixed-byte `WriteOnlyPrimitiveStore` where `requires_ascii` is `None` or `Some(false)` despite non-ASCII content that should be rejected; or a `ByteSequenceWrite` with `None` predicates as in A0704.

## A0706

**Verdict:** REACHABLE (read)

**Cited site:** `may_change_owned_source` (`4451-4481`, read); digest `UV:4479` is the `_ => false` arm — line matches in current tree.

The match lists byte/structural stores and calls with owned arguments; **`Operation::ElementViewWrite` is absent**, so any `ElementViewWrite` yields `false` even when `destination.place` equals the tracked `source` (`302-310` in `abstract_operations/src/operations.rs`, read). By contrast, `fixed_byte_freshness::holds` invalidation treats `ElementViewWrite` as mutating (`5529`, read).

**Uses:** `rg '\bmay_change_owned_source\b'` → **3** in `unit_validation.rs` (`4166`, `4421`, definition `4451`); **3** in `omega-rust/psi/.../bytes.rs` (not read). In `derive_source_predicates` path (`4419-4423`, read), if no node `may_change_owned_source`, declared predicates on the operation result can be returned without seeing an intervening element view write.

**Input:** A unit where an owned byte place is logically changed by an `ElementViewWrite` with `destination.place` equal to that source, but predicate derivation still treats the source as unchanged — dependent byte checks can accept the unit.

Digest claim that `ElementViewWrite` is missing from the list is correct (read).

## A0707

**Verdict:** REACHABLE (read)

**Cited site:** `fixed_byte_freshness::holds` establishment closure (`5436-5458`, read). Catch-all arm `operation =>` (`5455-5457`) returns true when `operation.structural_result()` names `root` and `declared_ascii(...)` holds for `path`.

**`declared_ascii`** (`5552-5587`, read): checks qualification domains on the carrier at `path` via `byte_qualification_predicates_in`; does not inspect actual byte values.

**Callers of `holds`:** used from `validate_indexed_byte_stores` (`3752-3760`) and `fixed_byte_freshness`/`byte_field_freshness` (inferred from grep, not every caller read).

**Input:** A unit whose producer at the root is a structural-result operation with ASCII declared in `qualifications`/`projected_qualifications` but whose bytes are not ASCII-established, followed by a fixed-byte store that requires ASCII freshness — `holds` can return true via `declared_ascii` alone.

## A0708

**Verdict:** REACHABLE (read)

**Cited site:** `control_flow::validate_total_cfg` (`5745-5804`, read). At `5793`, `if visited != blocks.len() && !admit_cycles` is the only cycle error; when `admit_cycles` is true, the function returns `Ok(())` despite incomplete topological visit.

**`ControlCyclePolicy`** (`5614-5625`, read): default `admitted_machines` is empty; `admits(machine)` true only after `admit(machine)`.

**Call path:** `validate_function` passes `cycle_policy.admits(function.machine)` (`9595-9600`, read) ← `index_and_validate_unit_catalogs` (`1233`) ← `validate_psi_optimization_unit_with_control_cycles` (`18260-18269`).

**Public entry points (read):**
- `validate_psi_optimization_unit` (`18204-18210`) uses `ControlCyclePolicy::default()` → cycles **not** admitted.
- `validate_psi_optimization_unit_with_admitted_cycle_machines` (`18229-18237`) admits listed machines.

**Pipeline callers (read, digest paths re-located):**
- `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager.rs:1313-1315` passes `admitted_machines` from cycle components (`1297-1304`).
- `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/artifact_admission.rs:1126-1128` passes `&[]` (no admitted cycles).

**Callers of `validate_total_cfg`:** `rg '\bvalidate_total_cfg\b'` under `omega-rust/` → **2** (def `5745`, call `9595`).

**Input:** A machine with a cyclic CFG whose `MachineId` is in `admitted_machines` for `validate_psi_optimization_unit_with_admitted_cycle_machines` — cycle not reported at `5793`.

Whether every cyclic machine in production is admitted only via `pass_manager` — **not fully traced** (inferred from digest “not read” for other callers).

## A0709

**Verdict:** REACHABLE (read)

**Cited site:** `availability::validate_structural_place_availability` (`6681-6746`, read). At `6724-6727`, missing `producers.get(&place)` uses `continue` instead of error.

**Producer map** (`6686-6716`, read): block `structural_parameters`, plus operation results from `structural_operation_result_contract` and byte-literal establishment — **not** all `function.structural_parameters` (read at `6688-6690` vs `2408-2410` in `reconstruct_declared_places`).

**Callers:** `rg '\bvalidate_structural_place_availability\b'` under `omega-rust/` → **3** (def `6681`, `pub(crate) use` `7137`, call `9605`).

**Order in `validate_function`:** availability at `9605` before `validate_places_and_claims` at `9659` (read).

**Input:** An operation whose `operation_place_inputs` (`6917-6923`, read) references a place with no producer row (e.g. machine-level parameter place not duplicated in block parameters, or an unproduced place id still embedded in an operation) — availability passes.

Digest wrong-program wording matches (read).

## A0710

**Verdict:** UNDECIDED

**Cited site:** `structural_roots::validate_structural_root_operations` (`7190-8511`, read). Inner `match &node.operation` ends with `_ => {}` at `8506` after the `O::ReturnStructural` arm (`8367-8505`).

**Read:** Universal checks before the match (`7301-7323`: `view_memberships_match`, `reference_result_memberships_match`, `primitive_locals::validate`) (read). Many operation kinds have dedicated arms (`7325-7562+`, `ReturnStructural`); all others hit `_ => {}` with no further errors from this match (read).

**Could not determine:** Digest item 47 does not name a concrete rejecting condition or operation variant. Whether every variant that reaches `_ => {}` is validated elsewhere in `validate_function` or earlier phases was not fully traced.

**What would decide:** Per-variant audit of operations hitting `_ => {}` against other validators in `validate_function` (`9546-9673` region) and upstream Psi admission.

## A0711

**Verdict:** REACHABLE (read)

**Cited site:** `7517-7550` in `validate_structural_root_operations` (read). For `ByteSequenceEqual` and `ByteSequencePredicate`, loops only run `valid_whole` when the operand is `ByteSequenceOperand::Whole { .. }` (`7519-7522`, `7536-7539`). `Field` operands are not matched — no error from this loop.

**`valid_whole`** (`5131-5155`, read): shared-borrow whole-sequence shape and length witness — does not cover field projections.

**Contrast (read):** `byte_concatenation::validate` checks `ByteSequenceOperand::Field` for concatenation (`2817-2844`). `descriptor_sources::validate_whole_freshness` also filters to `Whole` only (`5306-5312`).

**Callers:** `rg '\bvalidate_structural_root_operations\b'` under `omega-rust/` → **2** (def `7190`, call `9610`).

**Input:** A unit with `ByteSequenceEqual` or `ByteSequencePredicate` using a `Field` operand whose field does not exist, is erased, or whose `length` value is not backed by a valid length operation — this match arm does not reject it.

Digest wrong-program wording matches (read).

## What I did not read

- `git show 457adf4a11f:.../unit_validation.rs` for line-by-line diff against digest commit (current paths used; only spot-checked).
- Full body of `validate_structural_root_operations` match arms between `7562` and `8367`.
- `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/bytes.rs` `may_change_owned_source` (3 name hits).
- Whether Psi lowering or unit construction prevents the synthetic units described above before they reach `validate_psi_optimization_unit`.
- Complete caller closure for every operation kind hitting `_ => {}` (A0710).

SLICE COMPLETE

# slice_0027

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0200 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission_cleanup.rs:2044 | REACHABLE | `complete_carriers` matches InvokeNoNormal/Crash/ExternalCompletion with `_ => {}`, so live managed places get no `complete_disposals` on those exits. |
| A0201 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission_cleanup.rs:1439-1459 | REACHABLE | Machine-wide `scheduled_deaths` drops a descriptor arrival from `pending` when any branch schedules that place, so other branches never receive `DiscardRoot` here. |
| A0202 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission_cleanup.rs:3089-3118,3223-3227 | REACHABLE | With no open window and `restoration_required` false, overlap checks can be skipped and the store still emits `StructuralStoreDisposition::Vacant`. |
| A0203 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission_cleanup.rs:2274-2276 | REACHABLE | Ambiguous loan captures stop `captured_window_storage` at the unresolved carrier, so `emit_move` overlap at 2852-2861 may miss one spelling. |
| A0204 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission_cleanup.rs:4903-4912 | UNDECIDED | Obligations with no instance remap are copied verbatim; whether that lets an ill-checked program complete lowering was not traced past `CompanionRows`. |
| A0205 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/expression_bindings.rs:2995 | REACHABLE | Invalid scalar-computation node id skips the segment and leaves `ComputedIndex` on the path while lowering continues. |
| A0206 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/expression_bindings.rs:1732-1736 | REACHABLE | `retain` drops all prior payload rows with the same source and path before extending, not keyed by case or field. |
| A0207 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/expression_bindings.rs:4032-4051 | REACHABLE | Only excess captured selectors error; leftover `RuntimeIndex` segments after captured places are consumed are left unchanged. |

## A0200

**Digest:** area_10 record 067 item 25 (`emission_cleanup.rs:2044`): no discards on InvokeNoNormal, Crash, or ExternalCompletion exits for live managed places.

**Cited site (read):** `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission_cleanup.rs:2044` is the `_ => {}` arm of the terminator `match` inside `complete_carriers` (same file, `1602-2048`), in module `emission_cleanup::affine_cleanup`. Line numbers match the digest at commit `457adf4a11f` on this checkout.

**Enclosing function (read):** `complete_carriers` builds managed-place custody via `custody_flow::solve` (`1900-1913`), defines closure `complete_disposals` (`1946-1967`) that adds filtered `DiscardRoot` actions via `replace_managed_discards`, then walks every block (`1968-2046`). Jump, Conditional, StructuralCase, and Return variants call `complete_disposals`; `_ => {}` (`2044-2045`) handles all other terminators without calling it. Earlier, when building the successor graph (`1807-1884`), `Terminator::InvokeNoNormal`, `Crash`, and `ExternalCompletion` yield empty successor lists (`1879-1880`) but remain valid exit blocks in the machine.

**Callers (search):** `rg '\bcomplete_carriers\(' omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi` → **1** match: `emission_cleanup.rs:1383` inside `affine_cleanup::complete`. `rg 'affine_cleanup::complete' omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi` → **2** matches: `attached_unit.rs:11530`, `unit_composed_control.rs:11206` (machine lowering after body emission).

**Upward trace (read):** `affine_cleanup::complete` (`1371-1403`) always runs `complete_carriers` then returns `Ok(())` on success; it does not revisit crash-style terminators. Separately, `emission.rs:2436-2466` (`cleanup_rosters_mut`, read) returns an empty roster list for the same three terminator kinds, so `install` (`2327-2377`, read) also never edits their cleanup lists. Digest area_10 preamble item 8 (read) states both sites leave those terminators untouched.

**Input that reaches the branch (read):** A lowered machine with non-empty `managed` (`1781-1783` early-returns only when empty) and at least one block whose terminator is InvokeNoNormal, Crash, or ExternalCompletion, with `exits[position]` from custody flow still listing live managed places that are not already on that terminator’s cleanup roster.

**Verdict rationale:** Lowering completes without adding affine discards on those exits even when custody flow still reports live managed places—consistent with a smallest program that ought to be rejected or fully cleaned but is accepted through stage-06 affine completion.

## A0201

**Digest:** area_10 record 067 item 26 (`1439-1459`): machine-wide `scheduled_deaths` means a descriptor death on one branch blocks `DiscardRoot` installation on others.

**Cited site (read):** `1439-1459` fills `scheduled_deaths` by scanning every block’s cleanup rosters, owned structural operands, `ReleaseReference`, and owned arguments on Jump/ExternalCompletion/Conditional/StructuralCase terminators (`1467-1497`); `_ => {}` at `1498` skips other terminators including InvokeNoNormal and Crash.

**Enclosing function (read):** `retire_borrowed_descriptor_arrivals` (`1405-1545`) collects borrowed-descriptor arrival places per parameter, then if `arrivals` non-empty builds global `scheduled_deaths`, filters to `pending` arrivals with `!scheduled_deaths.contains(place)` (`1501-1504`), and for each pending place reachable from its birth blocks pushes `DiscardRoot(place)` on return-style cleanup rosters only where missing (`1524-1541`). `return_cleanup` (`1556-1571`) supplies rosters only for Return variants, not for InvokeNoNormal/Crash/ExternalCompletion.

**Callers (search):** `rg '\bretire_borrowed_descriptor_arrivals\(' omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi` → **1** match: `emission_cleanup.rs:1402` from `affine_cleanup::complete`.

**Upward trace (read):** Same two `affine_cleanup::complete` call sites as A0200; success returns `Ok(())` without a second pass over descriptor arrivals.

**Input that reaches the branch (read):** An affine borrowed view-descriptor parameter that “arrives” on one control-flow branch while another branch (or block body) records that place in `scheduled_deaths` via an owned operand, cleanup action, or edge transfer. The arrival is removed from `pending` (`1503`), so branches that never scheduled death get no `DiscardRoot` from this function.

**Verdict rationale:** Machine-wide death set is intentional in code; a program where retirement is required on an arrival-only branch but death appears only elsewhere can complete lowering without `DiscardRoot` on that branch.

## A0202

**Digest:** area_10 record 067 item 27 (`3089-3118`, `3223-3227`): store without open window and without required restoration can emit `Vacant` without proving vacancy.

**Cited site (read):** `3089-3117` gates errors when `index.is_none()` and either restoration is required (non-discardable) or an open window overlaps storage; `3223-3227` sets `StructuralStoreDisposition::Vacant` whenever `index.is_none() && !place.restoration_required` (else `ReplaceDiscardable`).

**Enclosing function (read):** `BorrowedWindowLedger::emit_store` (`2976-3245` in `emission_cleanup::borrowed_window`, read). After `captured_window_storage` and optional open-window index resolution, multiplicity/`discardable` logic (`3056-3088`) feeds the guard at `3089`. If the guard does not fire and types align, `StoreStructuralField` is pushed with the disposition arm above (`3220-3237`).

**Callers (search):** `rg '\bemit_store\(' omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi` → **1** production call: `unit_operation_frame.rs:3689` (borrowed-window repair during operation lowering).

**Upward trace (read):** `emit_store` returns `Ok(())` on the Vacant path; `unit_operation_frame` propagates that as successful operation emission into the terminal machine, not a source rejection.

**Input that reaches the branch (read):** A checked store into borrowed storage with no matching open window (`index.is_none()`), `place.restoration_required == false`, no overlapping entry in `environment.windows.open` under `paths_overlap` (`3282-3314`, read), and a repair value of matching type—lowering emits `Vacant` without an explicit “slot is vacant” proof beyond those guards.

**Verdict rationale:** Digest claim matches control flow: the Vacant disposition path does not require a positive vacancy proof when restoration is not required and overlap checks pass vacuously.

## A0203

**Digest:** area_10 record 067 item 28 (`2274-2276`): ambiguous captures leave the unresolved carrier, blinding overlap detection at `2852-2861`.

**Cited site (read):** `2274-2276` in `captured_window_storage` (`2252-2290`): if more than one `loan_captures` row shares `carrier == source` with differing `capture.source`, the `while` loop `break`s without rewriting `source`/`path`.

**Enclosing function (read):** `captured_window_storage` peels `Referent` prefixes through loan captures. `BorrowedWindowLedger::emit_move` (`2783-2974`) calls it at `2844-2849` then runs overlap detection on `environment.windows.open` using `storage_source`/`storage_path`/`storage_field` at `2852-2869`. `emit_store` calls the same helper at `3025-3030` before its overlap guard at `3091-3100`.

**Callers (search):** `rg '\bcaptured_window_storage\(' omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/emission_cleanup.rs` → **3** matches: definition `2252`, uses `2844`, `3025` (both in `borrowed_window`).

**Upward trace (read):** `emit_move` is invoked from `unit_operation_frame.rs:3653`; errors from `2852-2869` abort that emission, but if ambiguity prevents resolving to the storage identity that already holds an open window, the `find` at `2852` may not see the overlap.

**Input that reaches the branch (read):** `OperationBuffer.loan_captures` lists two or more captures for the same carrier place with different `source` places, and a move/store path still uses the unresolved carrier spelling—overlap with an open window recorded under the resolved storage identity may not fire.

**Verdict rationale:** Early `break` on ambiguous captures is explicit; a program where two spellings alias one borrowed window could pass `emit_move` overlap checking for one spelling only.

## A0204

**Digest:** area_10 record 067 item 29 (`4903-4912`): source-checked obligations not renamed by any instance are kept verbatim.

**Cited site (read):** After per-instance remapping (`4885-4894`), `4903-4912` extends `rows.source_checked_obligations` with originals from `source_checked` that appear in no `instance.namespace.obligations` mapping, then sorts and dedups.

**Enclosing function (read):** `continuation_specialization::companions::instantiate` (`4818-4914+`, read through invariant loop start). It rewrites calls, returns, comparisons, completions, and mapped obligations per continuation instance, then applies the verbatim extension for unmapped obligation ids.

**Callers (search):** `rg 'companions::instantiate' omega-rust` → **1** match: `emission_cleanup.rs:5857` inside `continuation_specialization::complete` (`5709+`).

**Upward trace (read):** `continuation_specialization::complete` is called from `attached_unit.rs:11504` and `unit_composed_control.rs:11131`. `instantiate` is invoked after cloned blocks are specialized (`5850-5857`); lowering continues to rebuild the machine regardless of which obligations were remapped.

**Input that reaches the branch (read):** A `source_checked_obligations` entry present in the pre-specialization rows but absent from every instance’s `namespace.obligations` table after duplication—those ids are copied unchanged into the output rows.

**Verdict rationale (UNDECIDED):** The verbatim copy behavior is clear (read), but whether that allows a checked program that should fail lowering—or only produces a mismatched obligation table in the lowered machine—was not determined; tracing consumers of `source_checked_obligations` after `complete` was not done.

## A0205

**Digest:** area_10 record 068 item 30 (`2995`): invalid computation handle skipped; segment stays `ComputedIndex`.

**Cited site (read):** `2995-2997`: `if !computations.nodes.is_valid(computation) { continue; }` in `retain_proven_selector_coordinates`.

**Enclosing function (read):** `retain_proven_selector_coordinates` (`2986-3004`) scans path segments; only `PreparedStructuralPathSegment::ComputedIndex` is considered; invalid node ids skip replacement. Called from `prepare_structural_argument_projection` (`2982`) after `prepare_root_projection` and optional `retain_selector_evaluations`.

**Callers (search):** `rg 'retain_proven_selector_coordinates' omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi` → **2** matches: definition `2986`, call `2982`. `prepare_structural_argument_projection` is used by `prepare_structural_argument_path` (`2761`) and `prepare_call_argument_path` (`2783`); `prepare_call_argument_path` has **3** production calls (`scalar_computations.rs:6263`, `6304`, `attached_unit.rs:14327`). `prepare_structural_argument_path` is used from `ResolvedBorrowedArgument::prepare_path` (`4021`), which is used from `unit_operation_frame.rs:7259`, `unit_structural_values.rs:2807`, `expression_bindings.rs:4411`, `4573`, and `attached_unit.rs:16156` (direct `retain_captured_coordinates` only on the last).

**Upward trace (read):** `prepare_structural_argument_projection` returns `Ok((path, tip))` after `retain_proven_selector_coordinates` even when segments remain `ComputedIndex`; callers extend lowered paths without an extra validity gate on those segments.

**Input that reaches the branch (read):** A checked structural argument path containing `ComputedIndex(handle)` where `checked.facts.values.scalar_computations.nodes.is_valid(handle)` is false (stale or corrupt id relative to the checked graph).

**Verdict rationale:** Lowering does not fail on that segment; path preparation succeeds with a `ComputedIndex` that cannot be resolved to `EvaluatedFixedIndex`.

## A0206

**Digest:** area_10 record 068 item 31 (`1732-1736`): earlier payload rows with the same source and path are replaced.

**Cited site (read):** `1732-1736` in `establish_case_payloads` (`1728-1749`): `retain` removes any prior `established_payloads` row whose `source` and `path` match any row in the new `established` batch (case and field are not compared).

**Enclosing function (read):** `establish_case_payloads` in `expression_bindings::structural_fields` updates `StructuralFieldBindings`, then copies matching rows into each root’s `case_payloads` (`1738-1747`).

**Callers (search):** `rg 'establish_case_payloads\(' omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi` → **3** call sites in `unit_composed_control.rs:2527`, `2963`, `8602` (plus `use` lines at `2852`, `8535`).

**Upward trace (read):** Composed-state emission calls `establish_case_payloads` when case payloads are established during lowering; no error is returned from `establish_case_payloads` itself.

**Input that reaches the branch (read):** Two establishments at different times (or a second call with a batch containing one case) sharing the same `source` and `path` but different `case`/`field`—the `retain` before `extend` drops the earlier row keyed only by source+path.

**Verdict rationale:** Replacement semantics are explicit; a machine that had established payload metadata for one case variant can silently lose it when another case on the same storage path is established later.

## A0207

**Digest:** area_10 record 068 item 32 (`4032-4051`): fewer captured places than runtime indexes leaves remaining `RuntimeIndex` without error.

**Cited site (read):** `4032-4050`: `retain_captured_coordinates` pairs `RuntimeIndex` segments with `self.captured` in order; only surplus captures after the path loop trigger `unsupported` (`4047-4048`).

**Enclosing function (read):** `ResolvedBorrowedArgument::retain_captured_coordinates` (`4032-4051`), called from `prepare_path` (`4028`) and directly from `attached_unit.rs:16156`.

**Callers (search):** `rg 'retain_captured_coordinates' omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi` → **3** matches: definitions/call in `expression_bindings.rs:4028`, `4032`, and `attached_unit.rs:16156`. `prepare_path` call sites as in A0205.

**Upward trace (read):** `prepare_path` returns `Ok(path)` when fewer captures than runtime indexes; downstream path lowering in `unit_operation_frame` / `unit_structural_values` / range lowering continues with unresolved `RuntimeIndex` segments.

**Input that reaches the branch (read):** A borrowed argument path prepared with more `RuntimeIndex` selector segments than `resolve_borrowed_argument` collected in `captured` (e.g. incomplete loan selector capture at formation, but path still lowered).

**Verdict rationale:** Asymmetric bounds check (errors only on too many captures, not too few) matches digest; lowering accepts the path with leftover runtime indexes.

## What I did not read

- Bodies of `custody_flow::solve` and full `custody_flow::Frontiers` semantics (`emission_cleanup.rs` callee, not opened).
- Stage-05 guarantees that invalid scalar computation ids or stale obligation ids cannot reach stage 06 (`05_typed-trees-to-checked-trees`, not opened).
- Downstream consumers of `rows.source_checked_obligations` after specialization (retention/proofs/08 verification, not traced for A0204).
- Full call contexts at `unit_composed_control.rs` for each `establish_case_payloads` invocation (only callee and grep counts read).
- Whether checker or spec requires rejection when affine places remain live at crash/invoke exits (wiki/spec, not opened).

SLICE COMPLETE

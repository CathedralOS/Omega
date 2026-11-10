slice_0027

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0200 | emission_cleanup.rs:2044 | CLOSED | Abnormal terminators are intentionally excluded; ordinary-machine completion is gated on normal completion. |
| A0201 | emission_cleanup.rs:1439-1459 | REACHABLE | Machine-wide `scheduled_deaths` suppresses descriptor `DiscardRoot` on other branches that still need retirement. |
| A0202 | emission_cleanup.rs:3089-3118 ; 3223-3227 | CLOSED | `Vacant` follows overlap and restoration-required guards; entry overstates “no proof”. |
| A0203 | emission_cleanup.rs:2274-2276 ; 2852-2861 | REACHABLE | Ambiguous loan capture stops resolution; overlap check can miss one storage reached via two spellings. |
| A0204 | emission_cleanup.rs:4903-4912 | CLOSED | Verbatim retention only for obligations with no per-instance remap; not a missing-input accept. |
| A0205 | expression_bindings.rs:2995 | CLOSED | Invalid computation handles should not reach stage 06; remainder fails later on the prepared path. |
| A0206 | expression_bindings.rs:1732-1736 | CLOSED | Same source/path payload rows are internal dedup during lowering, not unchecked program acceptance. |
| A0207 | expression_bindings.rs:4032-4051 | CLOSED | Fewer captures than `RuntimeIndex` segments is allowed for non-loan indices; excess captures error at 4047-4048. |

### A0200

1. Read `complete_carriers` in full (`emission_cleanup.rs:1602-2047`); read `custody_flow::solve` through `Frontiers` return (`emission_cleanup.rs:245-567`); read terminator successor arms (`emission_cleanup.rs:1870-1885`, `1968-2045`).
2. Search `affine_cleanup::complete\(` in `omega-rust`: 2 hits (`attached_unit.rs:11530`, `unit_composed_control.rs:11206`).
3. Read `attached_unit.rs:10869-11539`: `completes_normally` becomes false when `take_invocation_completion` supplies a terminator (`10871-10872`); `affine_cleanup::complete` runs only when `completes_normally` (`11529-11539`). Composed emission always calls `complete` (`unit_composed_control.rs:11206-11218`).
4. Input on the accepting branch: blocks whose terminators are `InvokeNoNormal`, `Crash`, or `ExternalCompletion` after `custody_flow::solve` has computed `exits[position]` with live managed places (`emission_cleanup.rs:1900-1913`, `2044`).
5. **Verdict: CLOSED.** `attached_unit.rs:11529` limits carrier completion to normally completing machines; digest area_10 item 8 (read) documents leaving Crash, InvokeNoNormal, and ExternalCompletion untouched across roster editors. Line 2044 is `_ => {}`, so no `complete_disposals` on those kinds; that is intentional for non-continuing exits, not a path where a should-reject program is admitted at compile time.
6. Unsure: whether a normally completing emitted graph can still contain a reachable Crash block with live affine owners that ought to fail compilation; did not read composed-control roster edit sites (`unit_composed_control.rs` ~2335-2379).

### A0201

1. Read `retire_borrowed_descriptor_arrivals` in full (`emission_cleanup.rs:1405-1545`); read caller `complete` (`emission_cleanup.rs:1371-1403`).
2. Search `retire_borrowed_descriptor_arrivals\(` in `omega-rust`: 2 hits (definition + `complete` at `1402`).
3. Read `complete` callers via `affine_cleanup::complete` (2 production call sites above); both run after body emission on checked machines.
4. Input: borrowed descriptor arrival place `P` where some block schedules death/custody for `P` (cleanup roster, owned operand, or `ReleaseReference` at `1442-1458`, `1467-1497`), while another reachable branch returns without that death but still holds the descriptor.
5. **Verdict: REACHABLE.** `scheduled_deaths` is machine-wide (`1439-1500`); `pending` drops any place present anywhere (`1501-1504`). A death on one branch prevents `DiscardRoot` insertion on all return cleanup rosters (`1534-1540`) for that descriptor, including branches that never scheduled death. A program whose checker should require retirement on those returns could compile and emit IR missing those discards.
6. Unsure: stage-05 facts that force per-branch descriptor death before lowering; did not read stage-05 descriptor cleanup planner.

### A0202

1. Read `BorrowedWindowLedger` store emission through disposition (`emission_cleanup.rs:2988-3244`); read `paths_overlap` (`emission_cleanup.rs:3282-3314`).
2. No separate caller search beyond in-module `emit_move` / store paths using the same overlap helper (`2852-2869`, `3091-3117`).
3. Entry claim at 3089-3118 is partially wrong (read): overlap and restoration-required checks run before emission; only when `index.is_none()`, not `discardable` with `restoration_required`, and no overlapping open window does lowering proceed to `Vacant` (`3089-3118`, `3223-3227`).
4. Input: checked store with no open window, `place.restoration_required == false`, no overlapping open window per `paths_overlap`, and operand custody claims resolved (`3193-3218`).
5. **Verdict: CLOSED.** `Vacant` is emitted only after explicit overlap and restoration guards (`3089-3117`); vacuity of the slot is delegated to checked window state and prior `emit_move` bookkeeping, not silently assumed.
6. Unsure: none material for the cited lines.

### A0203

1. Read `captured_window_storage` in full (`emission_cleanup.rs:2252-2290`); read overlap gate in `emit_move` (`2844-2869`).
2. Search `captured_window_storage` in `omega-rust`: 3 hits (definition, two call sites at `2844`, `3025`).
3. `loan_captures` populated during emission (`operation_emission.rs:1231`, uses in `attached_unit.rs`); did not trace stage-05 capture formation.
4. Input: carrier place with multiple `loan_captures` rows disagreeing on `source` (`2271-2275`); loop breaks without error and leaves unresolved `(storage_source, storage_path)`.
5. **Verdict: REACHABLE.** Two spellings of the same storage can yield non-overlapping coordinates at `2852-2861`, so a second extraction overlapping an open window might not error when it should.
6. Unsure: how often stage-05 emits ambiguous captures; did not read capture insertion logic.

### A0204

1. Read `continuation_specialization::companions::instantiate` (`emission_cleanup.rs:4818-4914`); read `continuation_specialization::complete` call site (`attached_unit.rs:11504-11528`, `emission_cleanup.rs:5857`).
2. Search `source_checked_obligations` in stage-06 tree: many pushes during emission; specialization remap loop at `4885-4912`.
3. Obligations mapped inside an instance namespace get remapped targets (`4885-4893`); others are `extend`ed verbatim (`4903-4912`).
4. Input: obligation ids from pre-specialization emission that no continuation instance’s `namespace.obligations` lists.
5. **Verdict: CLOSED.** Verbatim retention keeps machine-global source-checked ids that specialization does not own; this is not accepting a missing or invalid obligation—ids still originate from checked emission (`operation_emission.rs` pushes).
6. Unsure: downstream consumers assuming all ids are remapped; did not read retention/proof join for those rows.

### A0205

1. Read `retain_proven_selector_coordinates` (`expression_bindings.rs:2986-3004`) and enclosing `prepare_root_projection` / `prepare_argument_projection` (partial `2986-3156`).
2. Search `retain_proven_selector_coordinates` in `omega-rust`: count 2 (definition + call at `2982` inside `expression_bindings.rs`).
3. Stage-05 rejects invalid computation in cleanup (`execution_terminal_unit_cleanup.rs:2343-2344`, read).
4. Input: `PreparedStructuralPathSegment::ComputedIndex` handle where `!computations.nodes.is_valid(computation)` (`2995-2996`).
5. **Verdict: CLOSED.** Invalid handles are skipped and the segment stays `ComputedIndex`; checked trees should not supply them, and later selector lowering requires evaluated operands (`3624-3635`) or errors.
6. Unsure: every path from stage 05 into `prepare_argument_projection`; did not exhaust all `ComputedIndex` producers.

### A0206

1. Read `establish_case_payloads` (`expression_bindings.rs:1728-1749`) and `EstablishedCasePayload` (`1714-1721`).
2. Search `establish_case_payloads\(` in `omega-rust`: count 4 (definition + 3 in `unit_composed_control.rs:2527`, `2963`, `8602`).
3. Read one caller context partially (`unit_composed_control.rs:2960` region, not full `emit`).
4. Input: new `established` batch sharing `(source, path)` with prior `fields.established_payloads` rows (`1732-1736`).
5. **Verdict: CLOSED.** Prior rows with matching source/path are removed and replaced by the new batch; this merges repeated establishment during lowering, not admission of an unchecked case payload.
6. Unsure: none for the cited behavior.

### A0207

1. Read `retain_captured_coordinates` (`expression_bindings.rs:4032-4051`); read `resolve_borrowed_argument` selector accounting (`expression_bindings.rs:4337-4350`, `4383-4388`).
2. Search `retain_captured_coordinates\(` in `omega-rust`: count 3 (`expression_bindings.rs:4028`, `attached_unit.rs:16156`).
3. Read `attached_unit.rs:16150-16156`: called on prepared path after selector retention.
4. Input: prepared path with more `RuntimeIndex` segments than entries in `self.captured`; only the first N matching segments become `CapturedIndex` (`4037-4045`); extras remain `RuntimeIndex`. Excess captures error (`4047-4048`).
5. **Verdict: CLOSED.** `resolve_borrowed_argument` requires `alias.selectors.len()` to match loan-path `RuntimeIndex` count (`4348-4349`); additional `RuntimeIndex` in the extended path are non-loan indices resolved at emit (`3589-3618`), so silence is not unchecked acceptance.
6. Unsure: paths that add `RuntimeIndex` after `resolve_borrowed_argument` without going through that check; did not read full `prepare_structural_argument_path`.

## What I did not read

- Composed-control cleanup roster edit arms (`unit_composed_control.rs` ~2335-2379, ~3336-3348, and related sites from digest item 8).
- Stage-05 borrowed-descriptor and loan-capture planners (beyond `execution_terminal_unit_cleanup.rs:2343`).
- Full `continuation_specialization::complete` and post-specialization obligation consumers in `retention.rs` / `proofs.rs`.
- Corpus or runtime validation behavior for missing crash-edge discards.

SLICE COMPLETE

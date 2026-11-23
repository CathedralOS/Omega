# Record: unit_047 (area 07-psi-semantics, crate terminal-semantics)

Paths below are relative to `source-rust/subsystems/compiler/psi/semantics/src/` (written `S/`) unless they start with
`source-rust/`. Everything cited was read. Counts come from the grep commands named inline. "Test" files were read in full too.

## Overview (what this unit is)

Three groups of files:
1. The descriptor replay engine (`S/descriptors/replay/*`, `selected_memberships.rs`): a data-flow replay over a per-machine
   "descriptor graph" that tracks, per storage location, which origins (loans/backings) a slot holds, which enum case is
   known, and what the contents of borrowed storage are. Callers ask it about storage identity, loans, case facts, and
   function summaries (result origins, written effects).
2. Judgments over the Terminal Psi module: membership facts (`membership_facts.rs`), inhabitation (`inhabitation.rs`),
   normal-return carrier checks (`normal_completion.rs`), numeric-primitive contracts (`numeric_primitives.rs`), the operation
   catalog (`operations.rs`), provider selection (`logical_providers*`), crash-free evaluation admission
   (`evaluation_crashes.rs`), plus small helpers (`entry_values.rs`, `evaluation_scope.rs`, `float_meaning_sources.rs`,
   `owned_sequence_bounds.rs`, `lib.rs`).

---

## 1. `S/lib.rs` (358 lines)

1. Purpose: crate root. Declares ~40 modules and re-exports ~300 names so consumers use `terminal_semantics::name`. (read)
2. Shape: module list `lib.rs:8-43`, one `pub(crate) mod exactly_one` with `exactly_one(rows, keep) -> Option<Row>` (`lib.rs:82-92`);
   `pub(crate) mod operations` (`lib.rs:147`); big re-export blocks (`lib.rs:46-353`). Last items `pub mod provider_candidates`, `logical_providers`,
   `program_storage_entry` (`lib.rs:355-358`).
3. IO: none beyond re-exports.
4. Decides: `exactly_one` returns `Some` only when exactly one row passes `keep` (`lib.rs:84-91`); keyed on a caller closure.
5. Cases by shape: none.
6. Same job elsewhere: `exactly_one` (unique-match lookup) is also hand-written as `Replay::machine` (`S/descriptors/replay/engine.rs:763-770`),
   as `TerminalContracts::declaration` (`S/inhabitation.rs:561-576`), and as `exactly_one_row_for_tag` (`S/operations.rs:318-331`, which returns
   a distinct error per case). Used via `crate::exactly_one` 81 times (`grep -rn -F "exactly_one(" src | grep -v "fn exactly_one"` = 81 lines, whole crate);
   `exactly_one_row_for_tag` has 3 lines of use in the crate (def + 2).
7. Unused code: not applicable (re-exports). Not traced re-export by re-export; the only re-exports I checked are the ones in section 5 below.
8. Stated limits: none.
9. Fail-open: none seen.

---

## 2. `S/descriptors/replay/case_flow.rs` (504 lines)

1. Purpose: tracks, in the replay state, which enum (sum/mixed) cases are known for each storage location, and which locations are
   known copies of each other (so a case fact about one holds for its copies). Answers "is this case possible / certain / impossible"
   at a flow point and moves the facts along flow edges.
2. Shape: `CurrentCases { facts: LocationMap<BTreeSet<StructuralCaseId>>, copies: values::Copies }` (`case_flow.rs:12-16`);
   `validate_graph` (`:18`), `valid_subject` (`:50`), `establish` (`:91`), private `Selection` enum + `selection` (`:113-159`),
   `select` (`:161`), `established_case` (`:184`), `membership` (`:219`), `established` (`:241`), `written_queries` (`:263`),
   `written_locations` (`:290`), `survives` (`:321`), `transport` (`:336`), `retire_location` (`:447`), `retire_query` (`:453`), `join` (`:491`).
3. IO: reads `Replay` (types), `DescriptorMachine` (flow, `case_constructors`, `case_successors`, `payload_invalidations`, `contents_transfers`),
   `State`. Mutates `State.cases` through `Arc::make_mut`.
4. Decides: case facts keyed on the canonical storage location (`replay.canonical_storage`, `:99`, `:134`, `:192`, `:231`, `:253`), i.e. an identity/place,
   not a name. A predicate is valid only if the subject's type is a `Sum`/`Mixed` containing that `StructuralCaseId` (`:83-88`). `survives` keeps a fact
   only if the writes are known and either empty or all disjoint from the subject and inside the owned `Storage` (`:321-334`). `join` keeps a location only if both
   sides have it, unions its cases (`:491-504`).
5. Cases by shape: `selection` "A nonsingular query validates the selected carrier but cannot name one cell whose case can be narrowed. Keep every branch feasible." (`:129-130`)
   - a contents query that is not a single exact location grants no fact. `written_locations` refuses (None) when a write target has `completed: Some` or is not storage (`:308-313`).
6. Same job elsewhere: `ResultCases` (`case_flow/result.rs`) does the case-fact capture/install for call results. `CurrentCases::join` and `ResultCases::join` (`result.rs:42`) both union-and-drop-missing.
7. Unused: `membership` (`:219`) one external call (`analysis.rs:338`); `established_case` has 11 lines in `src/` (`grep -F established_case`), one non-def caller seen at `analysis.rs:383`. All `pub(crate)`.
8. Stated limits: `4096` multiplier for the copy-equivalence limit `machine.flow.len().saturating_mul(4096).max(4096)` (`:103`, `:140`), no comment saying why.
9. Fail-open:
   - `select` returns `Some(true)` (branch feasible) when `selection` is `Selection::Unknown` (`:168-172`), which happens for a non-exact subject (`:131-133`) or an unresolved storage (`:134-136`). Stated: "Keep every branch feasible" (`:129-130`). Traced: this keeps a branch the compiler could not rule out; it is the conservative choice for reachability, not a way to accept a bad program. The matching `membership` returns `Unknown` not `Possible` (`:228-233`) and `established` returns `false` (`:250-255`), so no case is *claimed* known. (traced)
   - `transport` returns the base case facts unchanged when no derivations and writes are `Some(empty)` (`:356-358`); a `None` writes set (unlocated) goes to `survives` which then drops all facts (`:326-328`). Fail-closed.

---

## 3. `S/descriptors/replay/case_flow/result.rs` (110 lines)

1. Purpose: remembers, for a callee summary, which cases each returned value can have, then installs those facts on the caller's result location.
2. Shape: `ResultCases { root_type, facts: BTreeMap<Vec<DescriptorPathSegment>, BTreeSet<StructuralCaseId>> }` (`result.rs:8-11`); `capture` (`:14`), `join` (`:42`), `excludes_payload` (`:57`), `install` (`:75`).
3. IO: reads `State.cases.facts`; `install` writes into `state.cases` after `valid_subject` checks (`:93-100`).
4. Decides: `capture` keeps facts whose path does not cross a `Referent` (`:32-33`); `join` of results with different root types discards everything (`:43-47`); `excludes_payload` is true when some `Case(c)` segment's parent fact is non-empty and does not contain `c` (`:65-72`). Keyed on structural type id and case id.
5. Cases by shape: none.
6. Same job elsewhere: see section 2 (`case_flow.rs:491`).
7. Unused: `ResultCases` used outside `case_flow` only in `engine.rs` (`grep -F ResultCases src | grep -v case_flow` returned only engine references per my read of `engine.rs:228, 3981-4085, 2816-2853`); not otherwise public.
8. Stated limits: none.
9. Fail-open: `install` returns `Some(())` with nothing installed when `facts.is_empty()` (`:82-84`) - a callee that proved no case fact gives the caller none. Traced fail-closed (no fact claimed).

### `case_flow/result/tests.rs` (306 lines)
Two tests: an indexed `Contents` subject validates its type but grants no cell fact (`tests.rs:77-190`), and completed return tags union all exits while unknown or public returns grant nothing (`tests.rs:193-306`). Builds `DescriptorMachine` by hand (`tests.rs:14-74`) with every field listed.

---

## 4. `S/descriptors/replay/case_flow/values.rs` (401 lines)

1. Purpose: copy-equivalence classes ("these locations hold copies of the same enum value"), the per-machine set of storage roots eligible for case tracking, and the `Transfer` kind (operation vs. edge arrival).
2. Shape: `Copies { classes, member_classes }` with `equivalents` (`values.rs:27`), `insert_class` (`:68`), `retain` (`:93`), `meet` (`:104`); `Transfer` enum (`:120-130`) with `transports_value` (`:146`) and `copies_value` (`:167`); `Storage { roots, case_types, limit }` (`:184`) with `new` (`:191`), `contains` (`:238`), `copy_type` (`:243`); `transported_subject` (`:271`), `owned_arrival` (`:286`), `contains_case` (`:308`), `ordinary_type` (`:352`), `overlaps` (`:396`).
3. IO: reads `DescriptorMachine.places`, `payload_producers`, `flow` edges, `Replay.types`.
4. Decides: `Storage::new` puts a place in `roots` only if its type is "ordinary" (`ordinary_type`, `:199-203`) and it is owned: a parameter/arrival with `Owned` access, a local that is a payload producer root or an owned arrival, or a literal (`:210-223`). `copy_type` requires source type contains a case, destination same type, and `row.is_copyable()` (`:243-260`). Keyed on place id and structural type id.
5. Cases by shape: `contains_case`/`ordinary_type` walk `Record`, `Mixed`, `Sum`, `FixedArray` and treat every other shape (`_ => false` / `_ => true`) as having no case / being ordinary (`:330-346`, `:370-391`). `ordinary_type` excludes `AccessView`/`Accessor` records (`:374-378`) and anything not `plain_owned_shape` (`:357`).
6. Same job elsewhere: `Copies` is a union-find-like class table; `LocationMap` (section 9) is a separate keyed map. No duplicate seen.
7. Unused: `contains_case` is `pub(crate)` and re-exported from `case_flow.rs:8`; callers beyond this file not grepped.
8. Stated limits: `limit: machine.flow.len().saturating_mul(4096).max(4096)` (`:234`).
9. Fail-open: `contains_case` returns `false` on a cycle (`active.insert` fails, `:317-319`) and on a missing declaration (`:320-322`), so a recursive type yields "no case" (the cycle path alone). Traced: effect is fewer tracked case facts, not more. `ordinary_type` returns `false` on a missing declaration (`:362-364`). None seen that accept.

---

## 5. `S/descriptors/replay/contents_queries.rs` (1456 lines)

1. Purpose: `impl Replay` methods that answer questions about the *contents* behind borrowed storage: what a path through references and element views resolves to, which concrete storage a write through such a path can reach, and how a callee's write/return effects are substituted into the caller.
2. Shape: all `impl Replay<'_>`. Key entries: `location_capture` (`:6`), `returned_roster_origins` (`:64`), `resolved_location` (`:103`), `returned_packet_input` (`:211`), `validate_query` (`:264`), `target_state` (`:289`), `read_contents` (`:323`)/`read_contents_with_depth` (`:337`), `known_current_contents` (`:453`), `read_incoming_contents` (`:469`), `contents_targets_with_depth` (`:553`), `contents_address_hidden` (`:625`), `inline_steps` (`:655`), private `contents_targets_at` (`:663`), `reference_targets` (`:822`), `incoming_targets_at` (`:888`), `account_value` (`:991`), `stored_view_targets` (`:1021`), `view_element_targets` (`:1067`), `transfer_contents` (`:1197`), `contents_replacements_with_depth` (`:1286`), `install_contents` (`:1350`), `actual_contents` (`:1377`), `extend_actual_contents` (`:1390`).
3. IO: reads `Replay.{types, summaries, snapshots}`, `State.{slots, contents, contents_invalidated, arrival_backing}`, `DescriptorMachine`. Writes `State` only in `transfer_contents`/`install_contents`.
4. Decides: everything keyed on `DescriptorLocation` (root place id + path of `DescriptorPathSegment`), `StructuralTypeId`, `MachineId`/position pairs, and `Origin` variants. No string compares. `contents_targets_with_depth` returns the `Incoming` target for "a shared array with an open extent" or "an exact index into an exclusive input" (`:573-600`); otherwise it drops selectors (`broad`, `:601-602`) and walks. `strong` update (replace) only when exactly one target, and it is current storage or a symbolic incoming location (`:1310-1314`); else union with the previous value (`:1319-1335`).
5. Cases by shape:
   - Fixed arrays enumerate at most 4096 elements: `if length > 4096 { return None; }` (`:774-776`) and `count <= 4096` (`:951`).
   - `contents_targets_at` handles exactly: an `Address` prefix then either `Referent` crossing, `Element` over a `FixedArray` with literal length or an `ElementView`, or end of path; any other combination returns `None` (`:769-819`).
   - `reference_targets` handles origins `Storage`, `Incoming`, `IncomingContents` with an empty projection, `Backing(Argument)` that is exact/direct/windowless/uncaptured, and `CapturedStorage`; `_ => return None` (`:846-878`).
   - `view_element_targets` handles `Empty`, `Storage`, `Incoming`, `IncomingContents(empty projection)`, and `Backing(Argument)` non-indirect; `IncomingContents(..)|CapturedStorage` and `_` return `None` (`:1089-1187`).
6. Same job elsewhere: `read_incoming_contents` / `read_contents_with_depth` parallel the slot read `read_slot` in `engine.rs:1812`; both resolve a location through references (suspected deliberate split: slot vs. contents). `extend_actual_contents` selector substitution calls `inline_selected_index` (defined elsewhere in `replay.rs`, not in this unit).
7. Unused: `contents_targets` (`:539`, no depth) used by `engine.rs:504` and `snapshot_tests`; `contents_replacements` (no depth, `:1268`) has non-test callers at `analysis.rs:1447` and one test at `scalar_captures/tests.rs:965` (grep). `returned_packet_input` (`:211`): 2 lines in `src/` (definition + one call, `grep -F returned_packet_input`).
8. Stated limits: comments at `:573-575` ("A shared array with an open extent supplies its elements at the call"), `:613-624` (address-hidden rule), `:1231-1234`. The 4096 caps have no comment.
9. Fail-open:
   - `read_incoming_contents` reuses a current slot "only while every reference crossed still denotes that entry pointer" (`:501-504`, `:505-537`); stated. Wrong program: a replaced pointer is detected by `incoming_references_unchanged` returning false.
   - `contents_replacements_with_depth` refuses (`None`) on empty or `completed: Some` targets (`:1296-1309`); `transfer_contents` then sets `contents_invalidated = true` (`:1213`, `:1227`), i.e. fails toward "unknown". Traced fail-closed.
   - `install_contents` with `completed: Some` sets `contents_invalidated` (`:1365-1369`). Fail-closed.
   - `extend_actual_contents` with no selector copies steps verbatim (`:1397-1407`). Traced benign.
   None seen that accept a bad program.

---

## 6. `S/descriptors/replay/dispatch.rs` (122 lines)

1. Purpose: joins several callee summaries (one per possible realization of a public interface) into one summary.
2. Shape: `pub(crate) fn join(joined: Summary, summaries: &[Summary]) -> Summary` (`dispatch.rs:4`).
3. IO: consumes `Summary` values (defined `engine.rs:226`). 
4. Decides: result slot lists must have identical paths across summaries; any mismatch sets `slots = None` and clears contents (`:20-31`). If any summary has `EffectObservations::Unobserved`, the joined effects become `Unobserved(writes)`, with `None` (unplaceable ceiling) if any is `Unobserved(None)` (`:37-76`). Otherwise effects are unioned and a summary lacking an effect contributes `IncomingContents` origin (`:98-120`).
5. Cases by shape: none beyond the above.
6. Same job elsewhere: `engine.rs` `merge_state`/`merge_slots` (`:4312-4401`) join states, a different object.
7. Unused: `join` called at `engine.rs:3360` and `:3391` (read).
8. Stated limits: none.
9. Fail-open: `.expect("unobserved effects joined above")` (`:84`, `:103`) and `.expect("a preceding unavailable ceiling exits the join")` (`:52`, `:66`) are panics, not accepts. Result-slot mismatch drops to `None` (fail-closed). None seen.

---

## 7. `S/descriptors/replay/engine.rs` (4548 lines)

1. Purpose: the replay engine core. Computes a state (per flow position) for each machine by a worklist data-flow to a fixed point, then derives summaries: what a call returns (borrowed origins), what it writes, whether it invalidates contents, which storage it writes (write frame) and accesses (access frame).
2. Shape (types): `Origin` enum (`engine.rs:10-20`: `Provider`, `Backing(DescriptorCallOrigin)`, `Incoming`, `IncomingContents`, `ProducedContents`, `Storage`, `CapturedStorage(StorageResolution)`); `Value = Option<BTreeSet<Origin>>` (`:21`, `None` = unknown); `State { cases, slots, arrival_backing, contents, contents_invalidated }` (`:24-33`); `StorageResolution` (`:39-52`, including `Opaque*` variants carrying a `&'static str` reason); `StorageFrame` (`:134`); `ResultObservations` (`:149`); `EffectObservations` (`:204`); `Summary` (`:226`); `PreparedStates`/`PreparedStateCache` (`:274-294`); `Replay` (`:310`); `DescriptorReadDepth` (`:318`).
   Main functions: `storage_writes` (`:411`), `prepare_write_frame` (`:571`), `prepare_access_frame` (`:602`), `checked_storage_family` (`:797`), `resolve_storage_with_depth` (`:954`), `resolve_storage_branch` (`:1261`), `read_slot` (`:1812`), `initial` (`:1955`), `project` (`:2023`), `transfer_states` (`:2232`), `transfer_edge` (`:2391`), `apply_invocation` (`:2533`), `apply_cleanup_calls` (`:2859`), `states` (`:2938`), `prepare_state_rows` (`:2960`), `invocation_summary` (`:3336`), `substitute_actuals_with_depth` (`:3493`), `exported_result` (`:3604`), `exported_roster` (`:3651`), `exported` (`:3821`), `summary` (`:3914`), `symbolic`/`flatten`/`resolved` (`:4227-4298`), `merge_state` (`:4312`), `merge_slots` (`:4348`), `impl State` (`:4403-4545`).
3. IO: input is a slice of `DescriptorMachine` plus the structural type table; mutates `self.summaries` and `self.snapshots` caches. Output is `Summary`, `States`, resolved storage coordinates.
4. Decides, keyed on:
   - Place/path identity (`DescriptorLocation`), declared access (`Owned`/`SharedBorrow`/...), and `StructuralTypeId` shape (`carries_backing`, `engine.rs:298-308`).
   - `resolve_storage_branch` follows reference origins to a captured address; it returns `OpaqueLocation`/`Opaque*` with a reason string when the account is incomplete (`:1273-1619`). The reason strings are only diagnostics; code never matches on their text (`StorageResolution::unavailable` returns the string, `:120-127`, and `checked_storage_family` returns it as the error, `:803-805`).
   - Fixed-point: a successor state is merged and re-queued only if `merge_state` reports a change (`:3264-3274`).
   - Summary of a body with no normal exit is `NoNormalBodyObservation` with effects `Unobserved(writable)` (`:3926-3979`).
5. Cases by shape / bounds (quote):
   - Recursion depth `if depth >= 128 { return None; }` (`:355`, `:2972`); exported roster/results `depth > 64` (`:3621`, `:3658`, `:3759`).
   - Visit cap `visits > machine.flow.len().saturating_mul(4096).max(4096)` -> failure "did not stabilize within its bound" (`:3052-3054`).
   - Result slots: `if !location.path.is_empty() { return None; }` (`:3988-3990`) - returns must be whole places.
   - `exported` accepts a `Backing(Argument)` only if the parameter's access is a reading borrow pair, `WriteOnlyBorrow`, or an owned parameter with `external_backing` (`:3896-3911`).
   - Cleanup calls: each cleanup call must have ascending `action_ordinal` and no missing `argument_contents` else `None` (`:2893-2897`); a normal-exit position may carry cleanup for at most one edge (`:3189-3191`).
   - `invocation_state` has two lookup routes by owner kind (`:2924-2935`).
6. Same job elsewhere:
   - `Replay::machine` (`:763-770`) hand-rolls "exactly one" (see section 1).
   - `storage_origins_with_depth` (`:1686`) and the `aliases_backing` branch in `resolve_storage_branch` (`:1338-1367`) both enumerate the backing locations of an origin set.
   - `exported` (`:3821`) vs. `symbolic` (`:4227`): two conversions from `Origin` to a portable form (stored origin vs. source origin). Read; not identical.
   - `empty_interface_summary` (`summary_interface.rs`) short-circuits `summary` (`:3919`).
7. Unused / one-caller (greps in `S/`): `Replay::resolved` (`:4289`) has one non-test caller `analysis/call_origins.rs:66` (the other 3 `.resolved(` matches are tests, `snapshot_tests.rs:2073,2079,2080`; `grep -rn -E "\.resolved\("`); `substitute_result` (`:3397`) one caller `call_origins.rs:58`; `substitute` (`:3447`) callers `analysis.rs:1052,1445`; `EffectObservations::into_observed` (`:217`) one caller `analysis.rs:1380`; `cached_invocation_summary` (`:3366`) callers `call_origins.rs:44`, `analysis.rs:1271,1434,1843`.
8. Stated limits: none labelled "unsupported"; limits are the numeric bounds in item 5 with no comment. Comments name incomplete accounts: "A recursive dependency sees an unknown frame. It cannot turn incomplete inference into an empty set of writes." (`:589-590`).
9. Fail-open:
   - `read_slot` returns `state.slots.get(location)?.clone()` for an `UnknownContents` leaf (`:1829`), i.e. `None` (unknown) if absent. Not an accept; callers treat `None` as unknown.
   - `exported`'s `Origin::Storage(location)` arm goes through `storage_projection` and then `exported` again (`:3870-3873`); if the root is an owned local the next call fails at `parameter(...)?` (`:3897`). Traced fail-closed.
   - `merge_slots` unions two known sets, and any side being unknown/absent gives unknown (`:4380-4386`) - fail-closed.
   - `transfer_states` `UntrackedFamily` destination: "It can change a case, but cannot replace a loan." (`:56-58`) and is accepted when `slots.is_empty()` (`:2266`): a write to one of several captured addresses whose value has no descriptor slots is allowed to proceed while the case facts are dropped through `unlocated_write` (`:2295`, `:2359`). Stated (comment at `:56-58`); traced: only the value with no borrowed slots reaches it.
   - `apply_invocation` skips an effect when all its origins "keep the incoming slot" and one of them cannot be substituted (`:2632-2643`), and later `continue`s when `origins.is_empty() && dropped_retired_incoming` (`:2710-2712`). This drops a callee write from the caller state when the written value was a retired incoming slot. Open: no comment at the site says why dropping is sound beyond the `keeps_incoming_slot` name; traced only as far as `effect.keeps_incoming_slot` (defined outside this unit, not read).
   - `summary()` when `completed.is_empty()` (no normal exit) publishes `invalidates_contents` from any state (`:3975`) and writable slots, with `slots = None` -> unobserved result. Stated in the `ResultObservations::NoNormalBodyObservation` doc (`:160-162`).
   - `ResultObservations::into_slots` returns `None` when contents are non-empty (`:182`). Fail-closed.

### `S/descriptors/replay/engine/snapshot_tests.rs` (3176 lines)
Test module (`engine.rs:4547-4548`). ~24 `#[test]`s (read all). Covers: shared `Arc` ownership of completed state maps and failure caching (`:7-417`), install/replace snapshots (`:419-448`), join behavior (`:450-596`), returned storage borrows (`:676-783`), absent normal observations (`:785-1014`), access and write frames (`:1016-1153`, `:1337-1435`), late summary not frozen (`:1155-1335`), cleanup actions replayed in order (`:1437-1655`), backedge does not preserve replaced descriptor (`:1657-1781`), referent contents and incoming alias/family handling (`:1783-2243`), indexed arrival capture (`:2245-2612`), complete storage family (`:2614-3039`), returned element family (`:2820-2873`), captured element store (`:2875-2956`), branch-selected storage (`:3041-3176`). Each builds `DescriptorMachine` literals by hand; the same 25-field literal is repeated (`:97-158`, `:633-674`, `case_flow/result/tests.rs:14-74`).

---

## 8. `S/descriptors/replay/input_contents.rs` (69 lines)

1. Purpose: for an opaque call, lists the borrowed-input origins a call may retain for one formal parameter (so an opaque call keeps those inputs live).
2. Shape: `impl DescriptorSlotKind { fn input_contents_origins(self, types, formal, position) -> Option<Vec<DescriptorStoredOrigin>> }` (`input_contents.rs:10`).
3. IO: reads `slot_inventory` (outside unit) and type table; returns origins.
4. Decides: walks types depth-first, `ancestors.contains(&ty)` -> `None` on recursion (`:19-21`); a symbolic-array unknown-contents slot is accepted only for `SharedBorrow` formals (`:47-51`).
5. Cases by shape: `origins.len() + pending.len() > 4096 -> None` (`:63-65`). Non-`FixedArray` array shape -> `None` (`:53-57`).
6. Same job elsewhere: none seen.
7. Unused: 2 lines (`grep -F input_contents_origins`): def + one call.
8. Stated limits: module doc: "A shared array with a symbolic length contributes its element origins without claiming that any particular index exists." (`:1-3`).
9. Fail-open: none seen (every unsupported shape returns `None`, which callers treat as no summary).

---

## 9. `S/descriptors/replay/location_map.rs` (283 lines)

1. Purpose: a sparse map keyed by `DescriptorLocation`, split into 64-root pages behind `Arc` so saved states share unchanged pages (copy-on-write).
2. Shape: `LocationMap<Value> { pages: Vec<(u64, Arc<BTreeMap<..>>)> }` (`:13-18`), `ROOTS_PER_PAGE = 64` (`:10`); `get`, `get_mut`, `insert`, `retain`, `iter`, plus `Extend`, `FromIterator`, `From<[..;N]>`, `IntoIterator` impls. Two tests (`:184-283`).
3. IO: pure container.
4. Decides: page index `(location.root.get() - 1) / ROOTS_PER_PAGE` (`:32`, `:73`); keyed on place id.
5. Cases by shape: none.
6. Same job elsewhere: `copy_on_write` is also `Arc::make_mut` on `State.{cases,slots,...}` (`engine.rs:4446-4452`); `LocationMap` is the only paged table here.
7. Unused: used outside file in 7 lines (`grep -rn -F LocationMap source-rust | grep -v location_map.rs` = 7).
8. Stated limits: "Keeping only actual pages also bounds storage for sparse, untrusted root IDs." (`:15-16`).
9. Fail-open: `get` of an absent page returns `None` (`:37`). Place id 0 would underflow `location.root.get() - 1`; `PlaceId` is non-zero by type (`PlaceId::new(0)` -> `None` used in tests), so not reachable (inferred).

---

## 10. `S/descriptors/replay/provider.rs` (106 lines)

1. Purpose: substitutes a callee's provider origin and its lifetime dependencies into the caller, and converts a source-level origin into a replay `Origin`.
2. Shape: `DescriptorProviderOrigin::map_dependencies` (`:5`), `Replay::substitute_lifetime_dependencies` (`:29`), `substitute_provider_with_depth` (`:61`), `source_origin` (`:92`).
3. IO: reads `DescriptorInvocation`, `State`; returns `Origin::Provider`.
4. Decides: when a dependency substitutes to another provider or produced contents, its dependencies are flattened into the new set (`:43-48`); otherwise the dependency keeps its own access (`:49-54`). Adds the invoking `(machine, owner)` to `invocations` (`:76-80`).
5. Cases by shape: none.
6. Same job elsewhere: none seen.
7. Unused: `source_origin` is `pub(crate)`; not grepped.
8. Stated limits: none.
9. Fail-open: none seen.

---

## 11. `S/descriptors/replay/summary_interface.rs` (83 lines)

1. Purpose: gives a trivial summary to a machine that returns nothing, writes through no parameter, and calls only things whose effects are known.
2. Shape: `Replay::empty_interface_summary(machine) -> Option<Summary>` (`:5`).
3. IO: reads `machine.returns`, parameter rows, `known_invocations`; produces `Summary` with `slots: None`, empty effects, and `invalidates_contents` ORed over reachable bodies.
4. Decides: `None` if `machine.returns.is_some()` (`:9-11`) or any parameter has writable slots or may write indirect contents (`:21-35`). Then walks all reachable callee machines (`:38-70`), validating each graph (`case_flow::validate_graph`, `:46`).
5. Cases by shape: `Public` target adds `effects_machine` and `realizations` to the worklist (`:58-67`).
6. Same job elsewhere: `summary()` (`engine.rs:3914`) is the general path. This is the fast path for a subset.
7. Unused: 2 lines (def + `engine.rs:3919`).
8. Stated limits: none.
9. Fail-open: the summary returns `effects: Observed(Vec::new())` for the whole call graph if each machine passes `validate_graph`; the claim "no writes" rests on the root having no writable parameter slots (`:21-35`) and *not* on the callees' own writes. A callee that writes storage reachable through a parameter is only visible through the root's own parameters. Open: reasoning not stated at site; I did not trace whether a callee can write caller storage not reachable from the root's parameters.

---

## 12. `S/descriptors/selected_memberships.rs` (335 lines)

1. Purpose: when a reference (or view) is formed from a source path, decides which structural-domain memberships ("qualification obligations") of the source carry over to the reference's referent path.
2. Shape: `reference_carrier_memberships` (`:8`), `reference_carrier_membership_contract` (`:46`), `reference_source_memberships` (`:103`), `MembershipSuffix`, `SelectionWork`, `append_candidate` (`:263`), `partition_runtime_memberships` (`:285`).
3. IO: reads type iterator, `whole` domain ids, `projected` path qualifications; returns `Vec<StructuralPathQualification>` or `&'static str` error.
4. Decides: keyed on structural paths and domain ids. For a runtime index or all-elements step it partitions candidate memberships by concrete index, `AllElements` rows (universal), and a "remainder" for unlisted indices (`:285-334`); the result is the intersection of candidates for a shared borrow, and for a non-shared access every candidate must equal (`:231-237`).
5. Cases by shape: fixed array with `Literal(0)` is refused (`:129-134`, `:179-184`); non-literal fixed extent refused (`:185-189`); collection carrier must be `FixedArray`, `ElementView` or `ByteSequence` (first loop) / `FixedArray`, `ElementView` (second loop) (`:128-141`, `:178-192`). Caps: `MAX_PATH_DEPTH: usize = 128`, `MAX_SELECTION_WORK: usize = 1_048_576` (`:111-112`), `MAX_CANDIDATES: usize = 4096` (`:269`).
6. Same job elsewhere: not seen in this unit.
7. Unused: `reference_carrier_membership_contract` (`:46`): 2 lines in `source-rust`/`tools` (`grep -rn --include=*.rs -F`): definition and the `lib.rs:98` re-export. **No call site seen.** `reference_carrier_memberships`: 1 external user, `psi/artifact/wire/src/foundation_validation.rs:8668`. `reference_source_memberships`: external user `descriptor_results/contents.rs:1031`.
8. Stated limits: error strings "exceeds the path-depth budget", "work budget", "candidate budget" (`:114`, `:258`, `:271`).
9. Fail-open:
   - `reference_source_memberships` returns `Ok(Vec::new())` when `whole` and `projected` are both empty (`:116-145`) after only checking the runtime-index parents are collections; traced: with no memberships to carry, nothing is dropped.
   - `reference_carrier_memberships` returns `Ok(Vec::new())` (drops all) when the reference reinterprets its source and `access == SharedBorrow` (`:38-39`); for non-shared it errors (`:41`). Stated by the error text "reference reinterpretation cannot erase storage qualification obligations". Traced: a shared reinterpretation carries no obligation set forward, so a reader of the reference gets no memberships rather than a wrong one.

---

## 13. `S/entry_values.rs` (118 lines)

1. Purpose: value types a receiver supplies at a Psi entry (scalars with provenance, structural inputs with path and qualifications).
2. Shape: `TerminalScalarValue` enum (`:11-35`: `Boolean`, `Integer`, `IeeeFloat`, `Callable`, `HostAddress`, `UndeterminedNaN`) with `satisfies_field` (`:38`), `literal` (`:55`), `scalar_type` (`:68`); `TerminalStructuralValue` (`:81`) with `minted`, `absorb_qualifications`; `TerminalAccessViewEstablishment` (`:115`).
3. IO: data; `satisfies_field` calls `crate::scalar_field_accepts_literal`.
4. Decides: `Callable` satisfies a field only if the field is `Scalar(ScalarType::Callable(expected))` with equal contract id (`:39-41`); `HostAddress` only an integer field of identical `IntegerType` (`:42-44`); `UndeterminedNaN(format)` only matching float field (`:45-49`). `literal()` is `None` for those three (`:62-64`).
5. Cases by shape: none.
6. Same job elsewhere: none seen.
7. Unused: `TerminalScalarValue` 755 lines/68 files; `TerminalStructuralValue` 531/37; `TerminalAccessViewEstablishment` 27/9 (`grep -rn --include=*.rs -F`).
8. Stated limits: `HostAddress` "refuses arithmetic, casts and comparison against its own addresses (spec: ...host-result-validation)" (`:22-27`).
9. Fail-open: none seen.

---

## 14. `S/evaluation_crashes.rs` (285 lines) and `evaluation_crashes/tests.rs` (529 lines)

1. Purpose: admits a compile-time-evaluated entry only if the emitted Terminal Psi machine can be proved to have no crash route.
2. Shape: `require_crash_free_evaluation(module, root)` (`:14`), `exclude_failure_routes` (`:50`), `require_formed_operands` (`:111`), `require_formed_scalar` (`:141`), `require_formed_integer_math` (`:219`), `closed_guard` (`:253`), `evaluation_declaration` (`:263`).
3. IO: reads `module.scalars.scalar_applications.proof_scope`, `machine.contract.crash_routes`, entry proposition context; calls `closed_predicates::{ClosedIntegerEvaluator, check_predicate_evaluation_size, closed_guard}`.
4. Decides: each crash-route guard must evaluate to `Some(false)`; `Some(true)` -> "can occur", `None` -> "cannot be proved impossible" (`:83-107`). Requires exactly one emitted machine associated with the declaration (`:30-35`). Checks guards are canonical (sorted, non-empty, `Truth` alone, `:54-64`), within evaluation size, valid in the entry context (`:77-80`), and that closed subterms are defined (no zero divisor, valid exact shift count, defined closed scalar) even inside a false conjunction/implication (`:111-217`). Keyed on `ProofDeclarationId` and machine id.
5. Cases by shape: `require_formed_scalar` special-cases `ExactIntegerDivide/Remainder`, `Wrapping*`/`Saturating*` divide/remainder with a literal zero right operand and `ExactIntegerShiftLeft/Right` with literal count (`:147-187`); treats `Application`, `Value`, `StructuralRead`, field, `ViewExtent`, `ByteSequencePredicate` terms as open (`:194-201`). Closed non-Boolean non-Integer terms count as undefined (`:204-212`).
6. Same job elsewhere: `inhabitation::literal_predicate` (`S/inhabitation.rs:82`) also drives `closed_predicates::closed_guard_with` with its own admitted leaf list.
7. Unused: `require_crash_free_evaluation` 9 lines / 4 files (`grep -rn --include=*.rs -F`).
8. Stated limits: error text "constant evaluation requires proof that no crash route can occur before execution" (`:44`).
9. Fail-open: none seen. Unknown (`None`) guard rejects (`:99-105`). Tests assert: empty route list passes, `Falsehood`-guard passes, undefined sibling under a false guard still fails (`tests.rs:28-475`).

---

## 15. `S/evaluation_scope.rs` (75 lines)

1. Purpose: finds the proof declarations a compile-time evaluation entry demands for one target selection.
2. Shape: `CompilerInvocationScope { selected, closure, dependencies }` (`:4`), `compiler_invocation_scope(scope, target, providers, root)` (`:10`).
3. IO: reads `scope.plan`, `scope.applications()`; calls `crate::scalar_conditions::proof_provider_edges`.
4. Decides: picks callable by target (`callable_for_target`) or without a target (`:22-26`). With no target, any machine in the demanded set whose `authored_target`, `source_target` or `family_interface` is set is an error: "target-independent evaluation demands a target-selected body" (`:47-69`). Keyed on ids.
5. Cases by shape: none.
6. Same job elsewhere: none seen.
7. Unused: 5 lines, 3 files: callers `build/evaluation/authority/admission/admission.rs:112,252` and `psi/execution-inputs/scalar-applications/entry_checks.rs:82`.
8. Stated limits: "A tagged callee can occur below an untagged root. Traversal leaves it in the demanded call roster rather than silently selecting it." (`:48-49`).
9. Fail-open: none seen.

---

## 16. `S/float_meaning_sources.rs` (132 lines)

1. Purpose: maps each float-typed value in a machine to the kind of producer that made it (parameter, block parameter, operation result, call result), so a float meaning is bound to its actual producer.
2. Shape: `Sources { values: Vec<(ValueId, FloatMeaningSource)> }` (`:12`), `for_machine` (`:20`), `source` (`:91`), `validate_source` (`:122`).
3. IO: reads `TerminalMachine` parameters, blocks, operations; `crate::operation_semantic_row`.
4. Decides: a call result is `DirectCallResult` iff the operation's semantic custody is `CallComposition` (`:62-68`), else `DirectOperationResult`. Duplicate producers are an error (`:81-83`). The entry block's parameters are skipped when they repeat the machine's parameters (`:38`).
5. Cases by shape: `source` handles literal floats, `Value` of float type; `IeeeFloatField` needs "the exact captured storage observation" (`:115-117`); every other term -> error (`:118`).
6. Same job elsewhere: none seen.
7. Unused: 3 lines/2 files; used by source-to-psi stage 06 `places.rs:1046,1253`.
8. Stated limits: none labelled.
9. Fail-open: none seen.

---

## 17. `S/inhabitation.rs` (1400 lines)

1. Purpose: conservatively decides which declared types have no possible value ("proved empty"), shared by source and Terminal types. Also evaluates closed Boolean/integer predicates with known actuals.
2. Shape: trait `TypeContracts` (`:21`); `Inhabitation { ids, types, cases, links }` (`:250`) with `prepare` (`:258`), `extend` (`:268`), `status` (`:360`), `case_status` (`:375`), `declared_path_status` (`:403`); `TerminalContracts` (`:550`) implementing `TypeContracts` (`:579`); free functions `observed_storage_path` (`:40`), `predicates_are_false(_with)` (`:50`, `:58`), `known_predicate_with` (`:71`), `literal_predicate` (`:82`), `known_scalar_value_with` (`:127`), `external_result_is_erased` (`:529`); `KnownLiterals` binding (`:153-201`).
3. IO: reads type declarations (`requires`, shape, fields). Output `Status::{ProvedEmpty, Unknown}`.
4. Decides: a type is empty if its `requires` contain a clause that evaluates to false (`predicates_are_false`, `:329`), or a record has an empty owned field, or every case of a sum is excluded (`notify`, `:508-523`). Reverse-dependency worklist (`:313-355`). Keyed on `StructuralTypeId` ordering (binary search, `:283-288`, `:360-371`). `declared_path_status` walks a path of `Case`, `Field`, `FixedIndex`, `Referent` segments (`:413-493`).
5. Cases by shape: `visit_owned_fields` handles `Record`, `Sum`, `Mixed`, non-empty literal-length `FixedArray`; "Reference | ElementView | FixedArray{..} | Opaque | TypeParameter | PrimitiveScalar | ByteSequence | AtomicCell => {}" (`:609-638`). Path walker returns `Unknown` for any segment not in its list, e.g. ranges, representation views, dynamic selectors (doc `:400-402`, `:492`). `literal_predicate` admits only `Truth|Falsehood|Equal|LessThan|LessOrEqual|IntegerMath*` leaves (`:93-103`).
6. Same job elsewhere: another `TypeContracts` impl in `source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/inhabitation.rs` (grep `TypeContracts for`: 2 files); `Inhabitation` has users in 20 files (`grep -rln Inhabitation`, listing: wire `foundation_validation.rs`, `calls/closed_conformances.rs`, native `terminal_signature.rs`, source-to-psi 05 and 06 files). Doc says "shared by source and Terminal types" (`:1`).
7. Unused: `known_predicate_with` 1 external caller (`source-to-psi/pipeline/06.../prove/counterexample.rs:64`); `observed_storage_path` callers `plain_affine.rs:2572` and native `semantic_analyses.rs:974`; `Inhabitation::extend` (the `.extend(&` pattern is too generic; `extend` is called inside `prepare` `:260`); `external_result_is_erased` 12 lines/8 files; `declared_path_status` 12 lines/3 files.
8. Stated limits: "Unknown contracts and cycles alone prove nothing." (`:5`); "Zero cases supplies no exclusion evidence; a missing/unfinished shape must never become a vacuous proof of emptiness." (`:29-30`).
9. Fail-open: the function answers "proved empty" (which unlocks `validate_return` for an uninhabited return, `normal_completion.rs:195`). What reaches it: a type whose `requires` list contains a clause that is provably false, or all cases excluded. `Status` defaults to `Unknown` for missing/foreign ids (`:360-371`). Duplicate ids in the catalog -> `Error::UnorderedIdentity` (`:277-279`) and `TerminalContracts::declaration` returns `None` on duplicates (`:566-574`). None seen. `known_scalar_value_with` returns a known value only for valid closed Boolean/integer terms (`:132-150`).

---

## 18. `S/logical_providers.rs` (386 lines) and `logical_providers/fixed_image.rs` (281 lines)

1. Purpose: after source checking, picks for each reached service requirement the exact provider body (or sealed fixed-image native member) and records the choice so native planning cannot re-select.
2. Shape: `ClosedLogicalProviders { program, target, reached, providers }` (`logical_providers.rs:339`) with `select` (`:350`); `LogicalProvider`, `ChosenProviderRow`, `LogicalRealization::{Checked(Vec<usize>), External(usize)}` (`:162-185`); `ProviderSelectionError` (`:112`); `satisfies_plan_name` (`:187`); private `candidates` (`:32`), `chosen_candidates` (`:69`), `schema_is_demanded` (`:79`), `selected_logical_providers` (`:196`). `fixed_image::selected` (`fixed_image.rs:22`), `verify_fixed_image_member` (`:74`), `no_additional_obligations` (`:226`), `storage_transport` (`:245`).
3. IO: reads `module.interfaces.{provider_candidates, provider_selections, external_providers, boundary_machines}`, identity text table, `module.proofs.closed_conformance_applications`.
4. Decides:
   - Candidates are sorted and matched by `(provider identity index, owner identity index)` (`:58-76`) and by requirement **string** (`row.requirement < requirement`, `:231-232`), i.e. identity text compared as `&str` after `module.identities.text(index)` lookups (`:13-22`). A `satisfies_plan_name` string is built with `format!` and passed as `name` to `selected_candidate_positions` (`:187-194`, `:225-227`, `:289`).
   - A schema is demanded if a reached boundary's identity text equals a method's requirement identity text (`:85-104`).
   - Competing exact realizations (different `source_callable` or source package) are an error (`:264-270`); a selected provider both checked and sealed-external is an error (`:236-238`); cross-package adapter closure is refused (`:240-244`, `:275-279`); a "lifetime family" requirement is refused (`:271-273`).
   - `fixed_image::verify_fixed_image_member` compares many identity texts against `format!("toolchain::{service}")`, `exact(provider)`, `callable_declaration_path(..)` etc. (`fixed_image.rs:148-196`). It is the string-keyed check of one bundled declaration. The two admitted operations are `ProgramImageRetainedSuffixStart` and `ProgramStorageEntry` (`:82-93`); target must be `uefi_x86_64` (`:57-61`).
5. Cases by shape: only two fixed-image operations (`:82-93`, `:201-216`). `storage_transport` requires exactly two structural parameters and exactly two crash routes `Trap` and `Abort` with `Truth` guards (`:255-278`). `no_additional_obligations` requires 14 clause lists empty (`:226-243`).
6. Same job elsewhere: `satisfies_plan_name` appears in 4 lines/3 files (def, own use `:225`, and others, `grep -rn -F`). The identity-text rebuilds in `fixed_image.rs` and `numeric_primitives::coordinates` (`S/numeric_primitives.rs:612-727`) both rebuild a canonical declaration/callable identity from a path and compare against the module's text: same job, two copies (`exact(..)`/`callable(..)` closures `fixed_image.rs:108-134`; `exact`/`callable` `numeric_primitives.rs:641-698`).
7. Unused: `ClosedLogicalProviders` 16 lines/6 files, with `select` called from `psi/execution-inputs/providers/logical_reach.rs:129` and two tests (`build/tests/provider_publication/fixed_image.rs:61,316`); I read these three callers only as grep hits.
8. Stated limits: error texts "chosen map needs an unrepresented cross-package adapter closure" (`:242`, `:277`), "selected provider lifetime family needs its exact retained partition coordinate" (`:272`). Doc: "Labels are diagnostic only; matching continues to use these IDs." (`:109-110`) - but the matching in item 4 compares text, not ids, at `:231-232`, `:85-99`, `:314-319`; the ids used are `TerminalIdentityIndex` values that index identity text (`:13-22`).
9. Fail-open: `fixed_image::selected` returns `Ok(None)` (no fixed-image supply) when no external provider row passes its filters (`fixed_image.rs:27-72`), and the caller then falls back to the checked-body path which errors with `MissingCheckedRow` if none (`logical_providers.rs:254-263`). Traced fail-closed. `schema_is_demanded` returning `false` skips a provider choice silently (`:219-221`): a chosen provider whose schema is not demanded yields no row. Traced: only undemanded schemas are skipped.

---

## 19. `S/membership_facts.rs` (2542 lines)

1. Purpose: proves, per flow point, which runtime values/storage locations are known members of a scalar or structural domain ("qualification"), and checks that every published normal-result membership promise holds at normal completion. Also checks callable refinement of promises.
2. Shape: `Domain`, `Subject::{Value, Storage}`, `Fact { subject, domain }`, `Dependency::{ValueImage, Referent}`, `Facts(BTreeMap<Fact, Dependencies>)` (`:22-217`), methods `insert_with_requirements`, `transferred`, `accept_transfers`, `scalar_read`, `scalar_store`, `proves`, `invalidate`, `intersect` (`:227-438`); `ResultBinding` (`:457`); `prepare_promises` (`:861`); `normal_membership_refinement` (`:976`); `validate_boundary_normal_memberships` (`:1142`); `validate_predicates` (`:1272`); `validate_headers` (`:1346`); `MembershipFacts` (`:1443`) with `scalar_on_edge` (`:1484`) and `structural_on_edge` (`:1505`); `analyze` (`:1626`), `analyze_bodies` (`:1637`).
3. IO: reads `TerminalModule`, runs `DescriptorAnalysis::with_reference_slots` over `terminal_reference_graph_from(module, ..)` (`:1665-1692`). Produces `MembershipFacts` (per-machine before-state facts and edge positions) or `MembershipError`.
4. Decides: a fact is *kept* across a write only if its dependency set is `Some(empty)`, or all known writes are disjoint from its dependencies (`invalidate`, `:401-425`); `None` dependencies are dropped by any write that is not known-empty. Join is intersection (`intersect`, `:427-437`). `proves` asks whether the same subject has the queried domain or a domain whose required closure contains it (`:379-399`, `domain_supplies` `:54-57`). Keyed on `ValueId`, `DescriptorLocation`, domain ids; not on names. Result subjects are matched against the formal result place/value id (`:495`, `:527`).
5. Cases by shape: promise subject must be `ScalarTerm::Value`, or `BooleanField/IntegerField/IeeeFloatField` (`:535-554`), otherwise "membership promise does not name its exact normal result" (`:478-483`, `:554`). A fact with a `Case` projection is never accepted from a type declaration alone: `has_case_projection` filtered at `:1016-1019`, `:1033`. Projected rows with `AllElements` skipped (`:660-666`).
6. Same job elsewhere: `scalar_qualifications.rs:1093` and `structural_qualifications.rs:2873,2992` consume `analyze_bodies` / `analyze` (grep). `declared_scalar_domains` (`:61`) overlaps `scalar_qualifications::field_qualifications`/`primitive_qualifications` (calls them). Predicate-retention check `validate_predicates` uses `scalar_qualifications::retains_conjunct` (`:1333`).
7. Unused: `normal_membership_refinement` one non-test caller `calls/callable_refinement.rs:441`; `analyze` one caller `structural_qualifications.rs:2992`; `analyze_bodies` callers `scalar_qualifications.rs:1093`, `structural_qualifications.rs:2873`; `validate_boundary_normal_memberships` 9 lines/6 files (re-exported at `lib.rs:27`).
8. Stated limits: error strings (`:1245`, `:1667-1670`). Comment: "An all-elements default is not an observation of one concrete element. Its existing default-domain judgment remains responsible for indexed uses and restoration." (`:657-659`).
9. Fail-open:
   - `analyze_bodies` returns `Ok(MembershipFacts::default())` (empty) when no machine/header/boundary/callable contract has any `membership_ensures` (`:1643-1662`). The empty facts answer `scalar_on_edge`/`structural_on_edge` with `false` (`:1492-1503`, `:1512-1514`). Traced fail-closed: no fact claimed.
   - `observe` returns `Some((fact, Some(BTreeSet::new())))` (empty dependency set, "never invalidated by writes") for any fact whose subject is a `Value` or a non-`Current` storage version (`:90-96`). Traced: `Value` facts and `Entry`-version facts are meant to survive writes (`invalidate` keeps them, `:419-424`); a `Value` is an SSA-like value id.
   - `prepare_promises` omits promises about "pure predicate" domains unless the fact needs projection evidence (`:864-891`): "Pure predicates are still ordinary postconditions ... do not invent an additional issuer obligation for their bodies." Stated.
   - `invalidate(Some(&[]))` keeps every fact (`:408-410`) - correct for "no writes".

   Tests (`:1994-2541`, 8 `#[test]`s, read) cover copy/referent dependency, simultaneous referent write, field copies, scalar read/store, unused headers, refinement closure, return capture, unknown writes.

---

## 20. `S/normal_completion.rs` (804 lines, includes inline tests `:328-804`)

1. Purpose: checks that an actual `Return`/`ReturnStructural`/`ReturnUnit` matches the machine's declared result carrier, and that whole-root discards (affine cleanup) have an owned affine source.
2. Shape: `structural_return_carrier_matches` (`:25`); `ReturnSources { machine, scalars, places }` (`:49`) with `new` (`:67`), `scalar_type` (`:157`), `validate_return` (`:166`), `is_uninhabited_return` (`:208`), `validate_whole_discards` (`:225`), `source_multiplicity` (`:292`), `structural` (`:319`).
3. IO: reads `TerminalMachine` parameters/places/blocks/operations; `Inhabitation`; `plain_affine::{declared_multiplicity, required_parameter_multiplicity, required_result_multiplicity, validate_carried_permissions}`.
4. Decides: keyed on `ValueId` / `PlaceId` via binary search over sorted scratch (`:157-162`, `:319-325`); duplicates in scalar/structural/place ids are errors (`:79-81`, `:105-110`, `:145-147`). A structural return passes if result type id equals the actual source's type id, or the source is an owned, proved-empty type (`:187-197`, `:208-219`). A whole discard needs owned access, `Affine` multiplicity and no `owns_linear_claim` (`:280-285`).
5. Cases by shape: `StructuralPlaceKind::Result | ProviderAttachment` yield no source (`:138-140`). `validate_return` only inspects three terminator kinds; other terminators pass `_ => {}` (`:199`).
6. Same job elsewhere: `structural_return_carrier_matches` 9 lines/6 files (grep). "Both source checking and artifact reception ask this same typed question" (`:3`): `ReturnSources` users are 4 files: wire `foundation_validation.rs`, this file, source-to-psi 06 `minted.rs`, and its test `tests/uninhabited_results.rs` (`grep -rln ReturnSources`).
7. Unused: n/a.
8. Stated limits: "An owned empty source may fit any result type, but this supplies no availability, ownership or domain proof." (`:5-6`).
9. Fail-open: `validate_return` accepts a `ReturnStructural` whose type differs from the result when the source is `Owned` and `ProvedEmpty` (`:195-197`). Stated (`:204-207`: only the returned owned root can establish the exception; physical reachability unchanged). Wrong program that gets through: one returning a value of an empty type where another type is declared - which has no runtime value, so no value is produced. Also `_ => {}` for non-return terminators (`:199`): traced - those carry no return carrier.

---

## 21. `S/numeric_primitives.rs` (775 lines)

1. Purpose: for each supported numeric primitive (float ops, comparisons, float<->int conversions) gives its canonical declaration path, requires/ensures/crash routes, and verifies that a published boundary machine for it matches exactly.
2. Shape: `Comparison` (`:14`), `comparison_predicate` (`:69`), `declaration_path` (`:79`), `declaration_kind` (`:122`), `ensures` (`:141`), `requires` (`:294`), `crash_routes` (`:311`), `result_policy` (`:335`), `result_matches` (`:347`), `result_scalar_type` (`:382`), `validate_boundary` (`:406`), `crash_routes_match` (`:586`), `coordinates` (`:612`).
3. IO: reads `numerics::intrinsics::NumericIntrinsic`, `language_core` bundled catalog, `TerminalModule` identity text; produces `Proposition`s.
4. Decides: `declaration_path` builds a **string** like `Float::{operation.name()}` or `F32::from_i32` via `format!` and checks it is in `language_core::float_operations::CANONICAL_FLOAT_OPERATIONS` (`:79-117`). `validate_boundary` then compares ~30 identity strings from `coordinates` against `module.identities.text(..)` (`:439-581`). Keyed on text compare of canonical identity strings plus enum fields.
5. Cases by shape: `Comparison::from_meaning` handles `FloatBinary` operations Equal/NotEqual/Less/LessOrEqual/Greater/GreaterOrEqual, all others `Err("numeric primitive has no canonical result contract yet")` (`:23-38`). `declaration_path` supports float binary, float builtin/negation, and exact/saturating/trapping conversions with a float on one side; else "outside the supported catalog" (`:81-112`). `integer_conversion` handles only float -> integer with Exact/Saturating/Trapping (`:271-290`).
6. Same job elsewhere: `coordinates` vs. `fixed_image::verify_fixed_image_member` (see section 18).
7. Unused: `comparison_predicate` 2 lines/2 files: def + `source-to-psi/pipeline/05.../read/contracts.rs:1156`. `declaration_kind`, `validate_boundary`, `result_matches` not grepped.
8. Stated limits: "numeric primitive has no canonical result contract yet" (`:24`, `:37`).
9. Fail-open: none seen; unsupported meanings return `Err`.

---

## 22. `S/operations.rs` (386 lines)

1. Purpose: the catalog of Terminal Psi operation kinds: one row per `OperationKind` variant with a schema identity string, a custody (leaf denotation or call composition) and an optional goal-free scalar-leaf schema.
2. Shape: `OperationSemanticCustody` (`:13`), `OperationSemanticRow` (`:19`), `OperationSemanticError` (`:49`, ~65 variants), macro `operation_semantic_rows!` generating `OperationSemanticTag` (`:116-156`), the row table (`:162-294`, ~118 rows), `exact_operation_semantic_row_in` (`:296`), `operation_semantic_row` (`:309`), `exactly_one_row_for_tag` (`:318`), macro `operation_semantic_cohort!` (`:333-384`).
3. IO: static data; `OperationSemanticTag::for_operation` maps an `OperationKind` by variant (`:134-138`). The `match` is exhaustive over `OperationKind` (macro), so a new variant fails to compile until a row exists.
4. Decides: keyed on the operation variant (enum), looked up by linear scan with exactly-one check (`:318-331`). `identity` strings like `"schema:operation:integer-constant"` are static labels, never compared inside this file.
5. Cases by shape: 21 rows have a goal-free scalar-leaf schema, the rest `None` (`:226-284`). Call operations (`Call`, `CallUnit`, `CallStructural...`, `BoundaryCall`, `CallDynamic`, `CallMachine`) are `CallComposition` (`:214-222`, `:223`), all others `LeafDenotation`.
6. Same job elsewhere: `exactly_one_row_for_tag` duplicates `exactly_one` (`lib.rs:84`) with distinct errors; `operation_semantic_cohort!` is used by the schema cohorts outside this unit (not read).
7. Unused: `exact_operation_semantic_row_in`: 3 lines in `source-rust`/`tools` (`grep`): def, `lib.rs:235` re-export, own use `operations.rs:312`. No external call site seen. `float_meaning_sources.rs:59` uses `operation_semantic_row`.
8. Stated limits: none.
9. Fail-open: none seen (`MissingRow`/`DuplicateRow` errors).

---

## 23. `S/owned_sequence_bounds.rs` (55 lines)

1. Purpose: for each owned sequence type (an `ElementView` with `access: None`) constructed by included machines, the largest literal element count.
2. Shape: `closed_owned_sequence_bounds(module, included) -> Result<Vec<(StructuralTypeId, u64)>, &'static str>` (`:4`).
3. IO: reads `module.types.structural_types`, machine operations (`EstablishStructuralArray { elements }`, `:21-23`).
4. Decides: maximum `elements.len()` per result type, keyed on `StructuralTypeId` through `arena::IdTable` (`:8-14`, `:30-40`); machines filtered by caller closure `included` (`:18`).
5. Cases by shape: only owned `ElementView` results counted (`:33-36`); other results ignored.
6. Same job elsewhere: none seen.
7. Unused: 8 lines/5 files (`grep -rn --include=*.rs -F closed_owned_sequence_bounds`).
8. Stated limits: none.
9. Fail-open: a type constructed by no included machine has no row (`:43-52`); consumers decide meaning of a missing bound (not traced). Open.

---

## Coverage

All 27 files in `units/unit_047.tsv` were read in full, every range from line 1 to its last line, in order:
- case_flow.rs 504; case_flow/result.rs 110; case_flow/result/tests.rs 306; case_flow/values.rs 401; contents_queries.rs 1456; dispatch.rs 122; engine.rs 4548 (read in three chunks 1-1500, 1501-3020, 3021-4548); engine/snapshot_tests.rs 3176 (1-1600, 1601-3176); input_contents.rs 69; location_map.rs 283; provider.rs 106; summary_interface.rs 83; selected_memberships.rs 335; entry_values.rs 118; evaluation_crashes.rs 285; evaluation_crashes/tests.rs 529; evaluation_scope.rs 75; float_meaning_sources.rs 132; inhabitation.rs 1400; lib.rs 358; logical_providers.rs 386; logical_providers/fixed_image.rs 281; membership_facts.rs 2542 (1-1300, 1301-2542); normal_completion.rs 804; numeric_primitives.rs 775; operations.rs 386; owned_sequence_bounds.rs 55.
- Lines read in full: 19,625. Skimmed: 0. Not read: none.
- Test files (`case_flow/result/tests.rs`, `engine/snapshot_tests.rs`, `evaluation_crashes/tests.rs`, and the inline test modules in `inhabitation.rs`, `normal_completion.rs`, `membership_facts.rs`, `location_map.rs`, `numeric_primitives.rs`) were read through but described at test-title level, not assertion by assertion.
- I did not read the other files in the crate that these call (for example `replay.rs`, `replay/analysis.rs`, `descriptor_lifetimes`, `plain_affine.rs`, `scalar_qualifications.rs`); claims about them rest on grep only and are marked.

## Five things a maintainer most needs to know about this unit

1. The replay engine (`engine.rs`, 4548 lines, plus `contents_queries.rs`) is one worklist data-flow whose unit of identity is `DescriptorLocation` (place id + path). `Value = Option<BTreeSet<Origin>>`: `None` means unknown and every merge/join turns "unknown on either side" into unknown (`engine.rs:4380-4386`, `:4306-4310`). Failures are carried as `PreparedStateCache::Failed(DescriptorGraphError)` with a detail string chosen by a mutable `failure` variable assigned before each step (`engine.rs:2947-3275`).
2. Many hard-coded bounds with no stated reason: `4096` (array element enumeration `contents_queries.rs:774,951`; copy-class and visit limits `case_flow.rs:103,140`, `values.rs:234`, `engine.rs:3052`; origin count `input_contents.rs:63`; candidates `selected_memberships.rs:269`), `128` (read depth `engine.rs:355`, state-prep depth `:2972`, membership path depth `selected_memberships.rs:111`), `64` (export depth `engine.rs:3621,3658,3759`), `1_048_576` (selection work `selected_memberships.rs:112`).
3. Identity for provider/numeric/fixed-image checks is by identity *text*: `logical_providers.rs:231-232`, `:85-99`; `fixed_image.rs:148-196` compares ~25 strings built with `format!`; `numeric_primitives.rs:439-581` does the same. The doc at `logical_providers.rs:109-110` says matching uses IDs, but the code looks up text by index and compares the text.
4. Safe default everywhere is "unknown/refuse": `inhabitation` gives `Unknown` for missing ids; `membership_facts` keeps a fact across a write only with `Some(empty)` or provably disjoint dependencies; `evaluation_crashes` rejects a guard that evaluates to `None`. The few accept-on-miss sites are the `select` branch-feasible answer (`case_flow.rs:168-172`), the empty-interface summary (`summary_interface.rs`), and `apply_invocation` dropping a retired incoming write (`engine.rs:2632-2643`, `:2710-2712`).
5. Dead-looking public surface: `reference_carrier_membership_contract` has no caller anywhere in `source-rust`/`tools` (2 grep lines: definition and re-export); `exact_operation_semantic_row_in` has no caller outside `operations.rs`; `Replay::resolved` has one non-test caller (`call_origins.rs:66`). The `DescriptorMachine` 25-field literal is repeated in three test files.

## What you are unsure about

- Whether `effect.keeps_incoming_slot` (defined outside this unit) makes the drops at `engine.rs:2632-2643` and `:2710-2712` sound; I did not read it.
- Whether `summary_interface.rs` is sound when a callee writes storage not reachable from the root's own parameters (`summary_interface.rs:21-35`, `:38-70`).
- The consumers of a missing row in `closed_owned_sequence_bounds` (what an absent bound means to callers).
- Exact external callers of several `pub(crate)` items in `replay/` (`contains_case`, `source_origin`, `contents_targets`): not counted.
- Whether `LocationMap::page` is safe for root id 0 (I inferred `PlaceId` is non-zero from `PlaceId::new(..).unwrap()` use in tests and `get() - 1`; did not read the `PlaceId` definition).

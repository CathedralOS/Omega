# Unit 53 notes

Files covered (all under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/`, abbreviated `SRC/`):
1. `SRC/checks_multiplicity_claim_outcomes.rs` (1-5011)
2. `SRC/checks_multiplicity_cleanup_dispositions.rs` (1-1624)
3. `SRC/checks_multiplicity_linear_validation.rs` (1-4619)
4. `SRC/checks_multiplicity_permission_events.rs` (1-2083)
5. `SRC/checks_operators.rs` (1-2945)

All five files were read in full. Greps below used the Grep tool over `omega-rust` (or the narrower path named) unless said otherwise. "read" = I read the lines; "grepped" = count from a Grep; "inferred" = my reading, not traced to the end.

---

## 1. checks_multiplicity_claim_outcomes.rs (5011 lines)

**Purpose.** For each state whose return type contains linear "claims", it works out which claim each result path settles: an input parameter's claim, or a claim established inside the callee. It also binds a call site's local claim identity to the callee's returned claim (a "join receipt"), and rewrites claim origins once call results are resolved. About 160 lines (106-268) and about 820 lines (2531-3352) are `#[cfg(test)]` modules.

**Shape (read).**
- `mod outcome_tables` (12-269): `unique_entries` (26), `Entries` (38; slice of `CheckedClaimOutcomeEntry` plus two `PlaceBucketIndex` on output_path and source), `InactivePaths` (85). Hash only selects candidates; equality is rechecked (63-66, 100-104).
- `mod joins` (271-2199): `ExitFrontier` (484), `Alternative` (505), `Capture` (519), `ExcludedInput` (526), `Receipt` (533), `ConsumedCaptureReports` (593, memo per outgoing/argument), `CallClaimJoinFailure` (658; `detail_for` 669 builds a diagnostic string), `ReceivingClaim` (738), `PreparedCallClaimJoin` (749), `CallJoinPreparation` (761; `resolve` 832), `known_source` (874), `state_frontiers` (903; recursive over callee outgoing edges, `active` stack rejects recursion at 914), `returned_identity_source` (1100), `bound_sources_at_call` (1162), `expand_joined_frontier` (1302), `complete_frontier` / `validate_frontier` / `complete_declared_frontier` (1362 / 1378 / 1394), `InactiveClaimQueries` (1441), `InactivePlaceWrites` (1523; `prepare` 1529, `preserved` 1592), `derive_receipts` (1646), `join_provenance` (1873), `claim_join_rewrites` (1911), `publish_call_claim_joins` (1941; fixed-point loop `0..=events.len()` at 1953), `validate_call_claim_joins` (2064; re-derives receipts and compares to the stored ones).
- `mod joins::selected_storage` (309-480): `selection_routes_agree` (328), `expand_storage_frontier` (361; splits a frontier over the false/true arms of one selection decision).
- `mod event_queries` (2200-3353): `SortedRows` (2216), `GlobalClaimEvents` (2244), `OrderedOriginRewrites` (2279), `OriginRewrites` (2299), `PermissionEvents` (2343; five sorted associations: statements, origins, claim_identities, places, places_across_statements; `Cow` rows), `EventCandidates` (2511). Comment 2508-2510: "Hashes select candidates only"; callers keep full predicates.
- Top level (3354-5011): `ClaimOutcomePreparation` (3391), thread-local `MAP_READS` + `note_map_read` / `record_map_reads` (3550-3574), `derive_checked_claim_outcome_maps` (3576; fixed point, `state_count+1` rounds, then a support-pruning loop at 3637-3650), `derive_claim_outcome_hypothesis` (3666), `claim_paths_are_case_alternatives` (3817), `ClaimAbsence` (3880; `expression` 3890, `arguments` 3994), `state_result_expressions` (4042), `state_result_named_transitions` (4070), `claim_outcomes_for_named_transition` (4108), `claim_outcomes_for_expression` (4152), `output_path_type` (4384), `claim_outcomes_for_owned_selection` (4414), `claim_outcome_source_for_identity` (4497), `claim_origin_for_source` (4531), `bind_claim_outcome_source_at_call/_arguments/_arguments_raw` (4578/4603/4716), `resolve_claim_outcome_source` (4640), `call_result_origin_rewrite(s)` (4830/4807), `apply_claim_origin_rewrites` (4910), `rewrite_claim_origin` (4944), `publish_claim_outcome_maps` (4965).

**Inputs/outputs.** Reads `CheckingInput` program tables, `CheckFactsRef` flow/borrow/ownership facts, and the permission-event rows (`FlowPermissionEventFact`). Produces `CheckedClaimOutcomeMap`s (one per state with declared claims) and `Receipt`s. Writes into `ownership.claim_join_*` arenas, `claim_outcome_maps/entries` (1965-2038, 4965-5011) and mutates `provenance`/`claim_identity` of events and selection transfers (4910-4941).

**How it decides.**
- Mostly typed identity equality: `PermissionClaimIdentity` plus `PermissionProvenance`, symbol handles, place paths.
- Source-spelling keys exist. `ClaimAbsence::expression` pairs a struct-literal field with a declared field by `field.name.as_str() == field_name` where `field_name = data_field_name(...)` (3946-3955). `claim_outcomes_for_expression` pairs literal fields by `literal_field.name == field.name` (4273-4276) and `field.name.as_str() == literal_field.name.as_str()` (4311-4312). The comment at 4379-4383 says "display names cannot join those namespaces", yet these three sites compare field names as strings.
- A map hypothesis survives only if each declared claim path has one distinct source (3727-3744) or is "statically inactive", and `complete_declared_frontier` holds (3760). `all_returns_covered` is recomputed after unsupported maps are dropped (3638-3649).
- Absence is considered only for paths containing a `Case` segment (3852-3856, 4016-4018, 1475-1479).
- A join receipt needs exactly one entry per received path in each callee frontier (1771-1790), a bound source per argument (1792), and an `Established` receiving identity matching the event (1842-1856).

**Cases by shape.**
- `claim_outcomes_for_expression` (4239-4376) handles: owned selection (4179), a place expression with Transfer events (4193-4237), `ArrayLiteral`, case `StructLiteral`, plain `StructLiteral`, `Call`, value-preserving `Cast`. `_ => Vec::new()` at 4375 for everything else.
- `ClaimAbsence::expression` (3921-3991): `StructLiteral`, `ArrayLiteral`, `Call`; `_ => false` at 3990.
- `state_result_expressions` (4049-4065): the final statement's expression only when `statement_index + 1 == statements.len()`, plus transition target/continuation values.
- `known_source` (874-895): Input path segments only `Field`, `Case`, `FixedIndex`. Other segment kinds make the source unknown.
- `bound_sources_at_call` requires exactly one captured invocation (1196-1208).

**Same job elsewhere.**
- `claim_outcome_source_for_identity` (4497) and `returned_identity_source` (1100) both turn a claim identity into `Input`/`Established` by looking at StateEntry Establish events. The second returns `None` on ambiguity (1132-1146); the first falls back to `Established` (4525).
- The "call result has no map entry, so the claim is inactive" judgment appears three times: `ClaimOutcomePreparation::resolve` (3516-3525, sets `obligation_live=false`), `ClaimAbsence::arguments` (4015-4019), and `apply_statically_inactive_call_results` in file 4 (`permission_events.rs:2036-2082`). Read all three.
- Three hash-index flavours in this unit: `PlaceBucketIndex` (this file, 15-18), `SortedRows` (2216), and `CaseObservationTable.by_identity` (file 2, line 36).
- `receiving_value` is re-exported from `validation` (4805; defined `validation/src/value_custody_core.rs:8041`). A separate `fn receiving_value` exists at `06_checked-trees-to-lowered-psi/src/unit_arguments.rs:2926` (suspected duplicate, not read).

**Unused / one-caller (grepped, `omega-rust`).**
- `call_result_origin_rewrites` (4807): 1 outside call (`permission_events.rs:2021`).
- `claim_paths_are_case_alternatives` (3817): 1 in-file use (1431), plus `checks_multiplicity.rs` lines 6589 and 10746 and `checks_content.rs:2496` (3 call sites outside; also re-exported at `checks_multiplicity.rs:1638` and imported at 5755 and 9113).
- `record_map_reads`: 1 call (`permission_events.rs:695`). `publish_claim_outcome_maps`: 2 calls (`permission_events.rs:195, 215`). `publish_call_claim_joins`: 1 call (:196). `validate_call_claim_joins`: 1 call (`linear_validation.rs:4199`). `apply_claim_origin_rewrites`: 2 calls (`permission_events.rs:2022`, in-file 1956). `derive_checked_claim_outcome_maps`: 3 calls (`permission_events.rs:205, 2019`, in-file 1654).
- `complete_frontier` (1362, `pub(super)` in `joins`): 1 call site (1056). `validate_frontier` (1378): 1 caller (1369, from `complete_frontier`).

**Stated limits.** 899-901 "No exit may disappear through filter_map." 1522 "Unknown effects stay unknown at their original statement". Non-convergence errors: "claim outcome equations did not converge" (3653), "call-result claim joins did not converge" (1962), "call-result custody preparation did not converge" (3491), `revision_limit` at 3479-3481.

**Fail-open risks.**
- `claim_outcomes_for_expression` `_ => Vec::new()` (4375) and early returns (4256, 4301, 4331, 4337): an unsupported return expression yields no entries. Traced: the hypothesis then needs the path to be statically inactive (3734) or is dropped (`return None` 3731/3741). Without a `Case` segment it is dropped. With one, `ClaimAbsence` must prove it. Not fail-open.
- `ClaimAbsence::arguments` 4015-4019: callee map has no entry for `path` and `path` has a `Case` segment returns `true` (claim treated as absent). Traced to map completeness: maps come out of `complete_declared_frontier`, which requires each declared path to have an entry or an inactive mark (1415-1418). A callee map missing an entry for a path that was declared but dropped would be read as absent: "traced", relies on that completeness.
- `ClaimOutcomePreparation::resolve` 3516-3525: callee map exists with no entry for the received path, so the receiving event is marked `obligation_live=false`, identity Unknown (the obligation is dropped). Keyed on `output_path` presence. "traced": same completeness argument; if no map exists nothing is dropped.
- `validate_call_claim_joins` 2167-2187: an event whose provenance is neither `Joined` kind passes if no expected receipt has its identity. An event with Unknown provenance and no receipt passes. "open" whether other validators cover Unknown provenance.
- `derive_receipts` `continue`s on any missing lookup (1678-1686, 1704-1706, 1839, 1851-1855), so no receipt is made. Validation compares recorded versus derived, so a missing receipt is an absent join, not an accepted one. Whether the event's claim then stays live depends on `resolve`. "open".
- `claim_outcomes_for_owned_selection` 4468-4476: no transfers gives one entry with Unknown identity/provenance; `known_source` (874) rejects it. "traced", fail-closed.

---

## 2. checks_multiplicity_cleanup_dispositions.rs (1624 lines)

**Purpose.** Tracks the "remaining custody" of owned storage at each point of one state's permission walk: which regions were moved or never established, which sum-type cases are proven inactive, which locals are still under construction. It uses that to emit the contextual cleanup rows (what to dispose at each exit). Comment 1-7 says it is a projection of the permission producer's order, not a second control-flow analysis.

**Shape (read).**
- `mod case_exclusions` (9-457): `CaseExclusions` (40) = `Arc<CaseObservationTable>` (34: `observations: Vec<CaseObservation>`, `by_identity: SymbolKeyMap<u64, usize>`). Methods `insert` (62), `contains` (78), `contains_prefix_of` (87; pops segments one at a time and re-queries), `iter` (105), `retain` (112), `clear` (137). `Arc::make_mut` copy-on-write at 67 and 121. Tests 142-456.
- `RemainingCustody` (479): fields `excluded: Vec<ExcludedStorage>`, `definite_excluded: Vec<CanonicalPlace>`, `inactive_cases: CaseExclusions`, `storage_frontiers: Vec<(PlaceRoot, ConstructionFrontier)>`, `zero_images: Vec<PlaceRoot>`. Also `ExcludedStorage` (511), `StorageEstablishment` (522), `CustodyCut` (531), `StorageBindingChange` (541), `TransferCustody` (553).
- Entry methods: `for_state` (646), `join` (670), `reserve` (754), `observe_ordinary` (981), `observe_evaluation` (1008), `observe` (1055), `apply` (1194), `select_owned_case` (1340), `owned_case_paths` (1364), `invalidate_call_observations` (1435), `remove` (1454), `transfer_storage` (1524), `retire_storage` (1540), `establish_storage` (1581).

**Inputs/outputs.** Reads permission events (`FlowPermissionEventFact`), the program (types, locals, case variants via `closed_data_definition` 1389), `ConstructionFrontier` (from `construction_frontier`). Mutates itself and appends to `CleanupFacts` (`output.dispositions`, `output.exclusions`, `output.storage_establishments`, `output.selected_dispositions`; 1076-1188, 1556-1572).

**How it decides.**
- Place equality on `(PlaceRoot, Vec<PlaceSegment>)`, `starts_with` prefix tests (e.g. 826, 1225, 1294), and `canonical_place_segments_may_overlap` (1419, 1426) for invalidation. Cause is an enum `ExclusionCause` (`Unestablished`, `Consumed`, `Unknown`, `InactiveCase`).
- `observe` (1055-1192): emits a disposition for an `AffineDrop` that is not already a unique whole-owned disposition (1093-1114), or for a `Consume` from `StateExit` (or from a `Statement` when `selected`) on a present path (1115-1119). The "whole-owned" test counts existing rows matching machine/state/source/completion/exit/root with `.take(2).count() == 1` (1095-1113).
- `join` (670-751): `definite_excluded` and `inactive_cases` intersect; `excluded` unions, with `Unestablished` downgraded to `Unknown` unless both sides agree (681-716); `storage_frontiers` keep only roots in both and intersect `established` (722-749).
- `same_storage_custody` (580-629): `excluded` and `definite_excluded` compared as sets; `inactive_cases` and `storage_frontiers` compared by ordered `.eq` (612-627).

**Cases by shape.** `apply` (1200-1272) splits `PermissionEventKind` into Establish/Restore versus Transfer/Consume/AffineDrop. The zero-image early exit (1202-1222) needs: Linear, not live, statement source, local declaration, no initial value, no establishment required, and `zero_image_inactive_case` returning a case. `owned_case_paths` (1364-1408) accepts only a stable case subject whose every prefix is a non-reference place of a closed data definition (1372-1389); returns `None` otherwise.

**Same job elsewhere.** The hash-fingerprint-plus-first-position index with full-equality fallback (`identity_position` 51-60) is a third copy of the idea in `claim_outcomes.rs` (`PlaceBucketIndex` 15, `SortedRows` 2216). Exclusion bookkeeping by root+path also appears in `RemainingCustody::excluded` and `definite_excluded` (two parallel vectors, 484-489).

**Unused / one-caller (grepped, `omega-rust`).** Each method below has at least one caller outside this file except where noted: `is_construction_vacancy` 1 (`checks_multiplicity.rs:9076`); `excluded_paths` 1 (:9041); `removed_paths` 3 (`linear_validation.rs:245`, `checks_multiplicity.rs:9015`, plus its in-file wrapper user at 880); `removed_regions` 0 outside, 1 in-file caller (880); `inactive_case_paths` 2 (`linear_validation.rs:246`, `checks_multiplicity.rs:9091`); `retire_storage` 4 (`linear_validation.rs:2177, 2219, 2584, 3213`); `consumed_place` 1 (`checks_multiplicity.rs:8490`); `crosses_inactive_case` 1 (:8487); `established_paths` 3 (:7474, 7480, 7549); `same_storage_after_transfers` 1 (:7016); `completes_with` 1 (:7546; the hit at :1809 is a different definition); `same_storage_custody` 1 in-file (577). Type names `RemainingCustody|CustodyCut|TransferCustody|StorageEstablishment|StorageBindingChange` counted by line: `flow.rs` 1, `permission_events.rs` 4, `linear_validation.rs` 29, this file 12, `checks_multiplicity.rs` 41.

**Stated limits.** None of the "unsupported / remains" kind. Comment 1362-style statements are design notes only. Zero-image comment 496-501 cites `ownership.md`.

**Fail-open risks.**
- `is_present` (822-833): a root with no `storage_frontier` is treated as present everywhere except explicit exclusions (`is_none_or` at 832). By analogy `completes_with` (793-799) has the stated reason at 784-785 "A root that is not under construction is already whole." For `is_present` no comment: "open".
- `join` 722-727 drops the frontier of a root that exists on only one side; afterwards `is_present` treats all paths of that root as present. A path unestablished on one arm would be considered present at cleanup. Direction is over-disposal not acceptance of a wrong program. "open".
- `select_owned_case` returns without excluding anything when `owned_case_paths` is `None` (1349-1351), `invalidate_call_observations` clears all case evidence on unknown writes (1440-1447): fail-closed, "traced" by reading.
- `observe_selected_transfer` (1311-1318) returns silently when the expression has no ownership place: no case evidence recorded (fail-closed). "traced".
- Otherwise: none seen.

---

## 3. checks_multiplicity_linear_validation.rs (4619 lines)

**Purpose.** Header (1-10) says it validates linear permission events one statement at a time. Concretely: builds the initial list of tracked linear places per state, produces the permission events for each statement (transfers, establishes, cleanup), then replays the recorded events to diagnose use after transfer, overwrite of a live linear value, unconsumed values at exit, and inconsistent treatment across transition arms.

**Shape (read).**
- `mod affine_cleanup` (12-451): `append_owned_cleanup_permission_events` (37), `visit_owned_cleanup_events` (66; locals then parameters, reverse order 148-181), `append_owned_region_cleanup_permission_events` (190), `visit_owned_region_cleanup_events` (216), `apply_recorded_cleanup_events` (296), `exit_evaluation` (427).
- `mod applied_events` (452-881): `apply_recorded_state_entry_events` (479), `apply_recorded_statement_events` (503), `apply_recorded_consumption` (621), `apply_recorded_exit_dispositions` (653), `permission_event_statement_index` (688), `append_borrow_permission_events` (701), `append_borrow_permission_event` (792).
- `mod case_selection_diagnostics` (882-1047): `exit_detail` (901), `selected_subject_detail` (978); diagnostics text only.
- `mod claim_frontier` (1048-1324): `initial_linear_places` (1066), `linear_claim_frontier` (1219; thin wrapper over `validation::linear_claim_frontier` 1223), `reserve_owner_places` (1239), `claim_place_name` (1286).
- `mod claim_provenance` (1325-1781): `completed_statement_values` (1357), `completed_receiving_values` (1387), `written_linear_targets` (1417), `written_owner_targets` (1519), `expression_permission_claim_scan` (1603), `..._claim_identity_for_claim` (1712), `..._provenance_for_claim` (1742).
- `mod loan_completions` (1782-2009): `regions_for_calls` (1797), `restore` (1920).
- `mod permission_production` (2010-4145): submods `observed_completions` (2050; `complete_at` 2068, `append` 2230, `completion_cut` 2400), `projected_scopes` (2428; `ProjectedScopes` 2462). Types `ProducedStorage` (2599), `EvaluatedStorage` (2616), `ObservedCustody` (2633). Functions `evaluate_statement_permissions` (2643, about 770 lines, one loop over `occurrences` at 2900), `complete_statement_permissions` (3422), `complete_body_custody` (3620), `copy_source_completions` (3687), `establish_targets` (3845), `record_conditional_carrier_transfer` (3998), `move_selects_claim` (4017), `select_static_case_alternative` (4022), `established_provenance` (4059), `event_statement_index` (4071), `permission_source` (4080), `excluded_carrier_payload` (4102), `trace_transfer_cut` (4112).
- Top level: `unconsumed_linear_value_diagnostic` (4178), `validate_linear_permission_events` (4193), `append_unresolved_state_result_mapping_diagnostics` (4438).

**Inputs/outputs.** Reads `CheckFacts` (flow control/ownership, borrow, operators, contextual cleanup), the typed program, `RemainingCustody` (file 2), `ClaimOutcomePreparation` (file 1), `SelectionOccurrences`, `temporary_results`, `projected_affine`. Produces `FlowPermissionEventFact` rows pushed into `permission_events`, `ContextualCleanupFacts` rows, `ProducedStorage` (storage changes for selected cleanup), and `Diagnostic`s. `validate_linear_permission_events` produces only diagnostics (4435).

**How it decides.**
- `LinearPlace` rows are keyed on `(root, path)` equality: `place.root == event.root && place.path.as_slice() == event_path` (491, 576, 681).
- Consumption check (621-651): not `ever_established` → error; not `live` → error; claim identity or provenance differs from the event → error (641-647); else mark not live.
- Establish over a live Linear place errors (585-593).
- Expression claim identity/provenance: all matching live places must carry the same value (`common_permission_*` 1766-1780).
- Name spelling is used in `expression_permission_claim_scan` to pair a struct-literal field with a data field: `field.name.as_str() == field_name` (1672-1676). Diagnostic labels are built from names (`claim_place_name` 1286-1323).
- Mixed-exit check (4359-4415): for each Linear place, `live` must agree across all ordinary exit outcomes.

**Cases by shape.**
- `initial_linear_places` (1066-1211): parameters become Affine whole-places only when `type_multiplicity == Affine` and the type node is `Named | Generic | FixedArray` (1109-1120); locals likewise by `binding_multiplicity` and `declaration_established` (1184-1193).
- `written_linear_targets` (1425-1503): `LocalData`, `Assignment`, else the completed value of the statement; `Vec::new()` for any other statement (returns at 1429, 1472, 1479, 1488, 1496, 1499).
- `expression_permission_claim_scan` (1603-1710): peels `StructLiteral` with Case, `ArrayLiteral` with FixedIndex, `StructLiteral` with Field; returns `None` for a `Call` with non-empty path (1687-1694).
- `append_borrow_permission_event` (809-819): maps `Read` to (Unrestricted, Shared); `Mutable` and `WriteOnly` both to (Affine, Exclusive).
- `regions_for_calls` (1797-1918): normal-return terminal callables only (1820-1824); `Field | Case | FixedIndex` segments only, anything else truncates the region (1889-1894).
- `exit_evaluation` (427-450): `TransitionTarget` / `TransitionContinuation` versus everything else (`Statement`).

**Same job elsewhere.**
- "event source to statement index" is done in three places: `permission_event_statement_index` (688-699, on `PermissionEventSource`), `event_statement_index` (4071-4078, on `FlowOwnershipEventSource`), and `flow.rs:9585` `event_statement_index(program, state, source)` (a different 3-arg function, not read).
- `permission_source` (4080) converts `FlowOwnershipEventSource` to `PermissionEventSource`; `permission_source_from_invalidation` (863) does the same for `FlowInvalidationSource`.
- Linear-place construction occurs four times with the same 11-field `LinearPlace` literal: 1123-1139, 1142-1158, 1167-1179, 1195-1207, 1254-1266, 1269-1281.
- `visit_owned_region_cleanup_events` (216) is the "one eligibility for cleanup" shared by exit cleanup (121), observed completions (2107), and external users: `append_owned_region_cleanup_permission_events` has 3 outside call sites (`checks_multiplicity.rs:7609, 8898, 10594`) plus 1 in `observed_completions`.
- `OMEGA_TRACE_CLAIM_FRONTIER` env trace appears at 4119 and 4528 here and at `checks_multiplicity.rs:6711` (grepped).
- `validate_call_claim_joins` (file 1, 2175) calls `claim_origin` from `selected_evaluation::place_bindings`, imported at file 1 line 280 (not read).

**Unused / one-caller (grepped).**
- `select_static_case_alternative` (4022, `pub(crate)`): 0 callers outside this file; 1 in-file call (2993).
- `event_statement_index` (this file's version, 4071): about 14 outside lines in `checks_multiplicity.rs` and `permission_events.rs` (lines listed by the grep; `flow.rs` has its own different function of that name). `written_linear_targets`: 1 outside call (`checks_multiplicity.rs:4276`). `record_conditional_carrier_transfer`: 1 outside (`checks_multiplicity.rs:9439`). `reserve_owner_places`: 4 outside (`permission_events.rs:927, 949, 954` and the import). `append_owned_cleanup_permission_events`: 1 outside (`permission_events.rs:1255`). `regions_for_calls`: 1 (`permission_events.rs:895`). `initial_linear_places`: 2 outside (`permission_events.rs:798, 1610`). `linear_claim_frontier` (this file's wrapper): about 20 outside uses (checks_content.rs, checks_multiplicity.rs); `validation::linear_claim_frontier` is also called directly from 9 other sites (`execution_terminal_unit_*.rs`, `06_.../emission.rs:555`, `unit_structural_values.rs:1108`, `validation/...`), so the "shared walker" comment at 1213-1218 has multiple outside users.
- `permission_source` outside: used at `checks_multiplicity.rs:9116` import only (grep line); not further traced. `completed_statement_values`: 2 outside lines (`permission_events.rs:12, 936`).
- Count commands: Grep count of `permission_source|completed_statement_values|exit_evaluation|complete_statement_permissions|evaluate_statement_permissions|ObservedCustody|apply_recorded_state_entry_events|append_borrow_permission_events|validate_linear_permission_events` outside this file: `permission_events.rs` 9, `checks_multiplicity.rs` 12 (the 5 in `package-manager/.../resolution.rs` are an unrelated `permission_source` match by name).

**Stated limits.** 4111 `/* Temporary bounded source-producer diagnostic; remove before landing. */` on `trace_transfer_cut` (4112-4144, reads env `OMEGA_TRACE_CLAIM_FRONTIER`, emits at most 128 lines via a static counter 4125-4128). Second env-gated `eprintln!` at 4528-4577 (24 lines). 4617-4618 error text: "return a path-aligned source place or publish an explicit outcome mapping". 3636: `complete_body_custody` returns at once if `conservation_plans` is empty.

**Fail-open risks.**
- `initial_linear_places` 1074-1075: `let owned_receiver = false && !type_reference_is_reference(...)`: the owned-receiver branch (1076-1093, 1100-1103) can never run. Dead code, read.
- `apply_recorded_state_entry_events` 489-494, `apply_recorded_statement_events` 574-579, `apply_recorded_exit_dispositions` 679-684: a recorded event with no matching tracked `LinearPlace` is skipped without a diagnostic. A recorded Transfer/Consume on an untracked `(root, path)` is not checked. "open" (not traced who guarantees every event has a place).
- `complete_body_custody` 3654: `let _ = content_transformations::commit(...)` discards its result. "open".
- `validate_linear_permission_events` 4207-4209: states of machines whose `supply_mode != CheckedBody` are skipped; 4201-4206 skip states whose program row is missing. Reasonable for bodyless/foreign machines; "inferred".
- `append_unresolved_state_result_mapping_diagnostics` 4472-4525: a result is treated as mapped if any owned Transfer in the target state or the caller statement shares the claim identity (4475-4492), or a checked outcome entry or join receipt matches. This is lenient on identity-only match; "open" whether a wrong mapping can pass.
- `exit_evaluation` 438 `unreachable!()`; `.expect(...)` at 282, 285, 382, 385, 422: panics, not accepts.
- Otherwise none seen.

---

## 4. checks_multiplicity_permission_events.rs (2083 lines)

**Purpose.** Records the permission events for every state: runs the per-state walk (in parallel when there are 64 or more states), reconciles call-result claim origins to a fixed point, publishes claim-outcome maps and call-claim joins, and derives the crash-frontier lower bound. It also holds the positive-case-test helpers used to read `value == Type::Case` guards.

**Shape (read).**
- `owned_permission_events` (51), `prepare_permission_events` (76; 85-237), `PermissionWalkScratch` (246), `prepare_walks` (271), `walk_states_serially` (298), `MIN_STATES_APART = 64` (335), `StateWalkedApart` (339), `PermissionWalkMemory` (387) / `PreviousWalk` (391) / `WalkInputs` (413), `take_reusable` (422), `WalkWorker` (468), `walk_states_apart` (504), `walk_state_apart` (675), `walk_state` (776, about 510 lines), `PreparedStateWalk` (1294), `prepare_state_walks` (1354), `state_call_writes` (1514), `DerivedCrashFrontier` (1565), `derive_crash_frontier_lower_bounds` (1581), `record_crash_frontier_lower_bounds` (1704), `ProvenCaseMembership` (1736), `proven_conditional_entry_claims` (1759), `collect_positive_case_tests` (1830), `visit_positive_case_tests` (1847), `exact_positive_case_test` (1904), `case_transition_run_is_exhaustive` (1938), `reconcile_state_call_result_origins` (2010), `apply_statically_inactive_call_results` (2036).

**Inputs/outputs.** Reads `CheckFacts` (flow control, borrow, operators, qualifications content), program tables, `MutationSummaries`, `ClaimOutcomePreparation`. Writes: resets many ownership/content tables at 86-109 (about 24 `Default::default()` assignments), then rebuilds `facts.flow.ownership.permissions`, claim-outcome maps and joins (188-215), cleanup facts (220-232), and `storage_transfers` into the state control rows. Crash lower bounds go into `MachineContractPlans` (1704-1733).

**How it decides.**
- Reuse of a previous walk (`take_reusable` 434-444) is keyed on equality of `state_flows`, `first_page`, `contextual_cleanup`, `termination`, content `plans`, `conservation_plans`, and `inputs.restored`; per state also on `reads_unchanged` (455). Comment 366-384 states the guard.
- Parallel walk falls back to serial if any worker's `walk_guard.crossed()` (582, 740), a shifted append is refused (597-612), or the page range is exceeded (520-534).
- Crash frontier (1670-1681): a place counts when `live` and (not `conditional` or its identity is in `proven_conditional_claims`); identity must not be Unknown.
- `proven_conditional_entry_claims` (1759): proof is attached to claim identity; a `Case` segment is proven only if an incoming guard has a matching `(parameter, subject_path, variant)` (1802-1813).
- `case_transition_run_is_exhaustive` (1938-2002): the subject is compared by source label, `program.expression_table.display_name(subject)` (1961, 1972), string equality, then the covered variants must equal the non-impossible variants of one data definition (1985-2001).
- `apply_statically_inactive_call_results` (2036-2082): a receiving establishment whose callee outcome map has no entry for the path marks every Owned event with that claim identity `obligation_live=false`, identity Unknown (2074-2081).

**Cases by shape.**
- `visit_positive_case_tests` (1847-1902): `And` splits; `Equal | CaseMembership` only; unwraps `== true` (1881-1888); everything else reported as declined (`visit(expression, None)`). Comment 1826-1829: "deliberately not infer through negation or disjunction here."
- `derive_crash_frontier_lower_bounds` (1587-1607): only states that contain a transition with `TransitionExit::Crash`.
- `walk_state` 831-833: returns `None` for `has_bodyless_signature()` machines.
- Index selectors (865-887): `Index { expression }` segments are rewritten to `FixedIndex` only when `captured_storage_index` resolves them.

**Same job elsewhere.**
- Case-membership leaf reading appears here (`visit_positive_case_tests`) and in `linear_validation.rs:882-1047` (diagnostics reuse it via import at 890-891) and in `checks_operators.rs` `contexts_prove_case_membership` (2779-2827, uses `published::proof::exact_outcome_case_test`, a different function, not read).
- Conjunction splitting over `BinaryOperator::And`: here (1862), `checks_operators.rs:505` (`positive_expression_proves`), `checks_operators.rs:2278` (`contexts_prove_boolean_expression`), and `published::labels::boolean_expression_has_polarity` (called at `checks_operators.rs:1758`, not read).
- "target state of a call": `apply_statically_inactive_call_results` uses `program.state_by_symbol(call.target_symbol)` (2060) while `derive_receipts` (file 1, 1684) and `ClaimOutcomePreparation::resolve` (3511) use `validation::call_target_state(program, call.target_symbol)`. Two lookups for the same job; not traced whether they differ.
- The no-map-entry-means-inactive rule duplicated: this file 2062-2066, file 1 3516-3525 and 4015-4019.

**Unused / one-caller (grepped, outside this file).**
- `collect_positive_case_tests` (1830, `pub(crate)`): 0 outside callers (only 1770, 1931, 1956 in this file).
- `visit_positive_case_tests`: 1 outside (`linear_validation.rs:923`). `exact_positive_case_test`: 1 outside (`linear_validation.rs:942`). `case_transition_run_is_exhaustive`: 1 outside (`checks_multiplicity.rs:5486`). `derive_crash_frontier_lower_bounds` and `record_crash_frontier_lower_bounds`: 1 each (`checks_multiplicity.rs:2515, 2516`). `owned_permission_events`: 1 outside (`linear_validation.rs:4212`). `prepare_permission_events`: 1 call (`published_facts.rs:11101`). `PermissionWalkMemory`: 1 (`published_facts.rs:11057`); `WalkInputs`: 1 (:11069). Count command: Grep content over `omega-rust` excluding this file, for the named functions.
- Test-only/dead: none found.

**Stated limits.** 1575-1576 "The result is an underapproximation by design: a conditional sum payload is omitted until active-case proof can make its liveness definite." 335 "Below this many states the walk stays serial". 1826-1829 (above).

**Fail-open risks.**
- `prepare_permission_events` 178-186 drops events listed in `conditional_carrier_transfers` that are not live with Unknown identity (via `binary_search` on a vector filled in increasing order, 657-661 and per-state pushes). "inferred" sortedness; the dropped events are provisional carrier transfers (comment in `linear_validation.rs:3991-3997`).
- `case_transition_run_is_exhaustive` compares subjects by `display_name` string (1961, 1972). Two different places with the same spelling would pass as one subject; a non-exact guard only contributes a label check and never adds to `covered` (1955-1970). Caller (`checks_multiplicity.rs:5486`) not traced. "open".
- `derive_crash_frontier_lower_bounds` stated underapproximation (1575): "stated".
- `apply_statically_inactive_call_results`: `state_by_symbol` returning `None` for a non-state target skips the rewrite (stays live): fail-closed. "traced".
- Otherwise none seen.

---

## 5. checks_operators.rs (2945 lines)

**Purpose.** Reports operator-selection problems (inadmissible, ambiguous), checks that float operator results land in a destination of the same format, proves the authored `requires` clauses of selected operators at each use, and holds the float-semantic term algebra used to prove float predicates.

**Shape (read).**
- `mod destinations` (13-376): `check` (25), `positional_operator_parameters` (170), `check_destination` (181), `result_primitive` (281), `primitive` (367).
- `mod float_semantic` (377-1152): `FloatSemanticTerm` (410; Application, Projection, Format, Integer, IntegerSource, Literal), `FloatSourceValue` (445), `Frame` (459), `context_proves_float_semantic_term` (464), `positive_expression_proves` (498), `boolean_semantic_term` (526), `selected_float_boolean_requirement` (564), `float_boolean_term_value` (601), `ordered_meaning_image` (627), `closed_semantic_result` (662), `instantiated_float_semantic_term` (709), `finite_float_semantic_term` (742), `SemanticBindings` (774), `semantic_term` (789), `substituted_source` (919), `substituted_operand` (962), `source_value` (981), `selected_expression` (1017), `frame` (1117). Work budget `remaining_work = 1024` (544, 602, 722).
- `mod requires` (1157-2828): `mod invocation` (1230-1646) with `InvocationContexts` (1260) and `CapturedInvocation` (1266: Operator or Call); `operator_route_is_false` (1654), `named_operator_route_is_false` (1689), `expression_has_polarity` (1748), `leaf_has_polarity` (1777), `selected_binary_requires_diagnostics` (1825), `operator_definition` (2037), `operator_path_label` (2058), `check_requires_facts` (2086), `requires_fact_proven` (2127), `contexts_prove_boolean_expression` (2223), `contexts_prove_boolean_leaf` (2286), `context_proves_membership_label` (2319), `requires_clause_label` (2363), `ClosedScalar` (2411), `reflexive_comparison_holds` (2437), `closed_clause_value` (2488), `closed_scalar_value` (2548), `operand_for_parameter` (2574), `clause_comparison_format` (2608), `closed_compare` (2653), `closed_exact_float` (2699), `closed_operators_are_builtin` (2727), `operator_use_is_builtin_meaning` (2756), `contexts_prove_case_membership` (2779).
- Top level: `check_operator_resolution` (2835), `inadmissible_operator_diagnostic` (2888), `ambiguous_operator_diagnostic` (2913).

**Inputs/outputs.** Reads `CheckFacts` (`operators.uses`, `named_uses`, `flow.control` operand captures and invocations, `semantic` context facts), program tables (operators, proof facts, signature contracts). Produces `Vec<Diagnostic>`; `float_semantic` produces `FloatSemanticTerm` values consumed by `checks.rs` and `checks_contracts_calls.rs`. Nothing is mutated.

**How it decides.**
- Destination check (`check_destination` 181-279): after peeling `Constrained`, recurses through struct literals (field symbol match 207, 215) and array literals; for a float destination it peels `Atomic`/`Unary`, requires a `Binary`, and compares the primitive of the selected operator's return type against the destination (270-278).
- Proving `requires`: matches by rendered label strings. `leaf_has_polarity` negative branch compares `program.expression_table.display_name(expression) == required` (1813). `context_proves_membership_label` compares `semantic.place_label(...) == value_label` or `display_name(fact_value) == value_label` (2352-2354). `contexts_prove_boolean_leaf` uses `context_proves_boolean_label` with a `required_label` (2295-2310). Proposition proof uses `normalized_identity_label` (2200-2212). So much of this is keyed on spelling/labels, not identity.
- Case membership (2779-2827) is keyed on identity: canonical place root/segments plus variant symbol (2814-2824).
- Closed-literal evaluation requires every operator inside to resolve to builtin or a boundary machine token (`operator_use_is_builtin_meaning` 2756-2772).
- Invocation capture selection (1376-1463): picks exactly one spelled use or one named use or one call; otherwise no invocation (1446-1457). Operand expressions must equal the capture's operand expressions exactly (1619-1630).

**Cases by shape.**
- `destinations::check` statements (46-95): `Call`, `LocalData`, `Assignment`, `Expression`, `Transition`; `_ => {}`. Only F32/F64 destinations are checked for operator results (251-255); other destination types return silently.
- `result_primitive` (294-364): `Atomic`, `Unary`, `Binary` (Add/Subtract/Multiply/Divide/Modulo only, 317-322), `Float`, `Call`, else expression type; `_ => None` for other binary operators (332).
- `positional_operator_parameters` (170-179): receiver skipped only when parameters exceed arguments by exactly one, and it skips index 0.
- `frame` (1117-1151): fixed list of `ProgramPoint` variants, `_ => None`.
- `selected_expression` (1057-1088): `Call` and `Binary`; `Unary` and others return `None` (1086-1087). For a float-returning operator, operands must be literals or `Exact` arithmetic domain (1089-1113).
- `closed_clause_value` (2494-2539): Boolean, Borrow, LogicalNot, And/Or, six comparisons, scalar atoms; everything else `None`.
- `FloatSemanticTerm::Format` handled for `Binary32`/`Binary64` only (652-655, 683-686).

**Same job elsewhere.**
- `leaf_has_polarity` polarity=true (1787-1796) calls `contexts_prove_boolean_leaf` (2286), the same proof as `contexts_prove_boolean_expression`'s leaf (2250).
- Conjunction/disjunction decomposition: 505-521, 2278-2279, `permission_events.rs:1862` (see file 4).
- `check_destination`'s struct field typing (200-231) resembles `claim_outcomes_for_expression` field matching (file 1, 4287-4328), different representation.
- `named_call_operands` is defined at `published_facts.rs:8646` and used here (1047, 1071, 1948); `resolved_operator_statement_symbol` at `flow.rs:11784` (used 48, 2004). `positional_operator_parameters` (170) re-derives receiver skipping that `named_call_operands` also does (comment 1184-1186); suspected, `named_call_operands` not read.
- `ClosedScalarContractValue` exists in `representations/checked-trees/src/contract_and_obligation_facts.rs:3460` (suspected similar closed-scalar concept; not read). The names `closed_clause_value`, `closed_scalar_value`, `closed_compare`, `closed_exact_float`, `reflexive_comparison_holds`, `operand_for_parameter`, `clause_comparison_format` are defined only here (grep over `omega-rust`).

**Unused / one-caller (grepped, outside this file).**
- `closed_operators_are_builtin`: 8 outside call sites (`checks_contracts_calls.rs:339, 1653, 1775, 1776, 1786, 1787`; `checks.rs:1486, 1511`) plus 2 in-file uses (2141, 2259).
- `operator_route_is_false`: 1 (`published_facts.rs:8585`). `named_operator_route_is_false`: 1 (`published_facts.rs:7989`). `selected_float_boolean_requirement`: 1 (`checks_contracts_calls.rs:362`). `instantiated_float_semantic_term`: 1 (`checks.rs:9874`). `finite_float_semantic_term`: 1 (:9889). `float_boolean_term_value`: 1 (:9893) plus 1 in-file (593). `context_proves_float_semantic_term`: 1 (:9900). `check_operator_resolution`: 1 (`checks.rs:12162`).
- `InvocationContexts::from_call`: 1 (`checks_contracts_calls.rs:338`). `for_selected_expression`: 1 (`checks.rs:9867`). `for_selected_expression_in_flow`: 1 (`published_facts_crash_calls.rs:4327`). `entry_constraints`: 1 outside (:4336). `operand_at`: 3 outside lines (`checks.rs:1065`, `published_facts_crash_calls.rs:4337, 4341`). `InvocationContexts::new` (1307): used in-file (1884) only, 0 outside. `from_flow` and `from_named_use`: used in-file only (1664, 1700, 1958, 1425, 1428). `flow()` accessor (1529): not grepped.

**Stated limits.** 1168 "Spelled binary uses have no special-purpose prover" (context: obligation is instantiated over operands). 1819-1823 slice `[]`/`[..]` uses "deliberately excluded" from this pass (ranges seam). 2433-2435 "Floats are excluded because NaN is not equal to itself". 383-384 "adapted results contribute no unconditional raw equation" (`adapter != None` returns `None` at 535). Fixed work budget 1024 steps (544, 602, 722) with `checked_sub` bailing to `None`.

**Fail-open risks.**
- `selected_binary_requires_diagnostics` skips a use when: a Match-equality arm follows an earlier `Wildcard` arm (1835-1848); the spelling is `Index`/`Range` and the expression is not `Indexed` (`_ => true` at 1856) or `ranges_seam_owns` says so (1854); `selected_candidate` is `None` (1860-1862); there are no `requires` facts (1870). Stated for the Index/Range skip (1819-1823 "handled by the ranges seam"). The `_ => true` arm (any Index/Range-spelled use whose expression is not `Indexed` is skipped): "open", the ranges seam coverage was not traced.
- Named uses with Index/Range operator spelling are skipped (1921-1926): "stated" at 1819-1823.
- `requires_fact_proven` 2139-2144: `x == x`, `x <= x`, `x >= x` with one symbol on both sides is accepted when the declared type is a non-float primitive and the operators are builtin. Stated at 2428-2435. Wrong program that would pass: none for builtin non-float scalars; user-typed parameters fall to `false` (`primitive_type` returns `None` at 2479-2480).
- `InvocationContexts::for_expressions` 1522-1526: if none of the clause's parameters occurs in the expressions, it returns every context of the entry constraints (all facts at call entry). Stated reason: nearby comment 1352-1358 / 1244-1248 (clause leaf with no formals). "traced" by reading.
- `closed_clause_value` true result accepted only when `closed_operators_are_builtin` (2258-2259): fail-closed when operators are not builtin.
- `check_destination` and `result_primitive` return nothing for unsupported shapes (190-192, 251-255, 332): a float-format mismatch through an unsupported expression shape is not reported. This is "not diagnosed", cases listed under "Cases by shape". "open" whether other passes catch those.
- Otherwise none seen.

---

## Coverage
- `checks_multiplicity_claim_outcomes.rs` 1-5011 read in full (5011 lines).
- `checks_multiplicity_cleanup_dispositions.rs` 1-1624 read in full (1624).
- `checks_multiplicity_linear_validation.rs` 1-4619 read in full (4619).
- `checks_multiplicity_permission_events.rs` 1-2083 read in full (2083).
- `checks_operators.rs` 1-2945 read in full (2945).
- Skimmed: none. Not read: none. Total 16282 lines read. Test-module lines (claim_outcomes 106-268, 2531-3352; cleanup_dispositions 142-456) were read but described only briefly.

## Five things a maintainer most needs to know about this unit
1. The "callee map has no entry for this path, so the claim is inactive" rule is implemented three times with different lookups: `claim_outcomes.rs:3516-3525` (clears `obligation_live`), `claim_outcomes.rs:4015-4019` (absence proof, only for case paths), and `permission_events.rs:2036-2082` (clears every event with that identity). The last uses `state_by_symbol` where the others use `call_target_state`.
2. Everything in `claim_outcomes.rs` is a fixed-point computation with hard iteration caps (`state_count+1`, `events.len()+1`, `revision_limit`), and incremental reuse depends on a thread-local `MAP_READS` (3550) that every map lookup must call (`note_map_read`). A new lookup that forgets it silently breaks `reads_unchanged` reuse in `permission_events.rs:455`.
3. `permission_events.rs` runs each state's walk on worker copies of the ownership tables and merges by page generation; correctness rests on `WalkGuard` and a long guard comment (366-384, 474-502). Failure of any guard falls back to a serial walk, so behaviour is the same but timing is not.
4. `linear_validation.rs` is both producer (`evaluate_statement_permissions`, one ~770-line loop) and replay validator (`validate_linear_permission_events`) of the same events. Replay skips events that have no tracked `LinearPlace` (574-579, 489-494, 679-684) and `initial_linear_places` has a dead `owned_receiver = false && ...` branch (1074-1075). Env-gated debug traces (`OMEGA_TRACE_CLAIM_FRONTIER` 4119, 4528) are labelled "remove before landing" (4111).
5. `checks_operators.rs` proves `requires` clauses mostly by comparing rendered labels/display names (`display_name == required` 1813, 2354; `place_label` 2353), with identity-based proof only for case membership (2814-2824) and float semantic terms; closed-literal evaluation is accepted only when all operators are builtin (2258-2259, 2727-2772).

## What you are unsure about
- Whether other passes cover recorded events with no tracked `LinearPlace` (the skip sites in file 3) and Index/Range-spelled uses with non-`Indexed` expressions (file 5, 1856).
- Whether `case_transition_run_is_exhaustive` (file 4, 1938) is reachable with two distinct places sharing one display label; its caller at `checks_multiplicity.rs:5486` was not read.
- Whether `apply_statically_inactive_call_results` (`state_by_symbol`) and the join code (`call_target_state`) can disagree for a call target that is a machine entry symbol.
- Whether the over-disposal direction of `RemainingCustody::join` dropping one-sided frontiers (file 2, 722-727) is reachable in a program that currently compiles.
- Suspected duplicates not read: `receiving_value` in `unit_arguments.rs:2926`, `event_statement_index` in `flow.rs:9585`, `ClosedScalarContractValue` in `contract_and_obligation_facts.rs:3460`, `exact_outcome_case_test` in `published/proof`, `boolean_expression_has_polarity` in `published/labels`.
- `InvocationContexts::flow()` (checks_operators.rs:1529) was not grepped for users.

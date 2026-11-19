Second pass for unit 112

Ranges covered (from gap_030.tsv):
- `omega-rust/psi/interpreter/terminal-interpreter/src/engine_terminators.rs` 1-2610 (2610 lines, the whole file)

The range starts at the file header comment (line 1), not inside a function, and ends at the closing brace of the top-level `impl TerminalExecution<'_>` (line 2610). All lines were read in full.

Relation to the first-pass notes (`unit_112.md`): that note says this file was not read and makes no claims about it. Its only facts about the file came from grepping callers of custody functions, and those hold: `whole_value_frontier` is called at `engine_terminators.rs:511`, `:1444`, `:1864` (3 sites); `consume_projected_ownership` at `:221` and `:2294` (2 sites); `bind_boundary_arguments` at `:2537` (1 site). No contradiction.

---

## engine_terminators.rs (1-2610)

### 1. Purpose
This file runs the end of a block in the Terminal Psi interpreter. A block ends in a terminator (jump, conditional, structural-case switch, return, return-unit, return-structural, crash, external completion, invoke-no-normal). The file captures the outgoing edge's argument values, checks them against the target block's declared parameters, runs the nominal cleanup (disposal) actions scheduled on the edge, then installs the successor block's parameters all at once. On return it checks the return against the caller's expectation and restores the caller's tables. The header comment says the same: "We capture outgoing edge operands before running disposal schedules, install successor block parameters simultaneously, and validate return preflight before restoring caller frames" (`:5-7`).

### 2. Shape
Layout (read): three inline modules, then top-level imports and one `impl`.
- `mod block_bindings` (`:10-1118`): staged state for a successor block.
  - `struct BlockBindings` (`:37-50`): fields `scalars` (ValueId to scalar), `structural` (PlaceId to structural value), `aggregate_backings`, `byte_sequences`, `primitive_loans`, `element_views`, `owned_sources: BTreeSet<PlaceId>`, `claims: BTreeMap<ClaimId, LiveClaim>`, `projected_sources: BTreeSet<PlaceId>` (the only `pub(crate)` field), `affine_destinations: BTreeSet<StructuralAffineDiscard>`, `aggregate_payload: ConstructorStorage`, `identity_cursor: Option<u64>`. `Default` at `:52-69`.
  - `retain_storage` (`:72`), `reserve_identities` (`:94`), `commit(self, execution, target)` (`:100`), `prepare_cleanup_actions` (`:181`).
  - `impl TerminalExecution`: `prepare_block_bindings(target, scalar_arguments, structural_arguments) -> BlockBindings` (`:329-943`, about 615 lines, one function), `borrowed_argument_source` (`:945`), `block_parameter_is_declared` (`:997`), `owned_source_for` (`pub(crate)`, `:1019`), `install_presented_descriptor` (`:1032`), `owned_result_source` (`:1060`), `owned_block_source` (`pub(crate)`, `:1098`).
- `mod completion` (`:1119-1602`): `reject_no_normal_return` (`:1127`), `retire_home_companions` (`:1141`), `continue_cleanup_actions` (`:1148`), `start_nominal_cleanup` (`:1229`), `restore_return_caller` (`:1259`), `finish_return` (`:1277`), `install_return` (`:1342`). A `#[cfg(test)] mod tests` at `:1465-1601` with one test `refused_return_descriptor_retains_detached_backing` (`:1482`) and helper `check_refused_return(retain_header)` (`:1488`).
- `mod structural_return` (`:1603-2105`): `settle_return_structural` (`:1610-2056`, about 450 lines, one function) and `snapshot_scalar_case` (`:2058`).
- Top-level (`:2107-2610`): `struct SelectedSuccessor<'edge> { edge, target, arguments, structural_arguments, cleanup_actions }` (`:2130`). `impl TerminalExecution`: `settle_invoke_no_normal` (`:2139`), `live_claim_paths_at` (`:2172`), `stored_case_at_discard` (`:2182`), `consume_affine_projection` (method wrapper, `:2195`), `settle_jump` (`:2209`), `settle_successor` (`:2238`), `settle_successor_with_scalars` (`:2248`), `settle_conditional` (`:2310`), `settle_structural_case` (`:2341`), `settle_return` (`:2408`), `settle_return_unit` (`:2443`), `settle_crash` (`:2473`), `settle_external_completion` (`:2506`).

### 3. Inputs and outputs
- Reads: the verified `terminal_psi::Terminator`, the current machine's declarations (`self.program.verified_machine(self.current_machine)`, `:335`; `machine.structural_place(...)`, `:1069`, `:1103`), and the live tables on `TerminalExecution`: `structural_values`, `linear_homes`, `scalar_array_values`, `byte_sequence_values`, `element_view_values`, `primitive_loan_values`, `aggregate_argument_backings`, `live_claims`, `live_affine_frontier`, `structural_cases`, `structural_scalar_fields`, `reference_referents`, `call_stack`, `retired_structural_homes`.
- Delegates to `engine::custody` for claim and frontier work (imports `:14-18`, `:2114-2117`): `bind_arguments`, `bind_active_retained_arguments`, `bind_active_structural_arguments`, `bind_affine_frontier`, `exclude_linear_regions`, `projected_ownership_claims`, `resolve_single_structural_argument`, `validate_nominal_cleanup_action`, `verified_structural_place`, `whole_path_discard`, `whole_value_frontier`, `commit_cleanup_action`, `consume_projected_ownership`, `rebind_structural_result_claims`, `bind_boundary_arguments`, `validate_boundary_requirements`, `complete_nominal_cleanup`, `begin_nominal_cleanup`, `consume_affine_discard`, `retire_cleanup_home`, `remove_affine_root`, `place_is_claimed`. It also delegates to `terminal_semantics` (`view_descriptor_parameter`, `borrowed_storage_parameter`, `arrival_parameter`, `borrowed_referent_presentation`, `view_descriptor_projected_qualifications`).
- Outputs: every `settle_*` returns `Result<TerminatorFlow, TerminalInterpretError>` (`Continue` or `Yield(status)`). `prepare_block_bindings` builds a `BlockBindings` without touching execution tables; `BlockBindings::commit` (`:100`) applies it. State changes happen only in `commit`, in `continue_cleanup_actions` (cleanup actions mutate live tables as they go, `:1163-1177`), and at the tail of `settle_return_structural` (`:2031-2054`).

### 4. How it decides
Keyed on identities, places and paths, not on names. The one source-text-like key is the env var below.
- Block parameter vs argument match: by `parameter.position == index` and `argument.access == parameter.access` (`:370-374`), and `parameter.place` declared by `StructuralPlaceKind::BlockParameter { block, position }` in the current machine (`block_parameter_is_declared`, `:997-1017`).
- Which binding path a parameter takes is decided by access and type shape, tried in order (`:375-683`):
  1. non-Owned access plus `StructuralTypeShape::PrimitiveScalar` -> primitive loan (`:375-397`);
  2. `prepare_scalar_array_loan` returns `Some` -> array loan (`:398-419`);
  3. non-Owned plus `ByteSequence` shape and `prepare_byte_sequence_loan` returns `Some` -> byte loan (`:420-452`);
  4. the argument place is in `scalar_array_values` -> packed array argument (`:453-482`);
  5. otherwise a `match parameter.access` (`:485-683`): Owned (whole value or projected), `reading_borrow_pair_pattern!()` (= SharedBorrow | MutableBorrow, defined `terminal-psi/src/ownership.rs:24-28`, read) when `view_descriptor_parameter`, Shared/Mutable/WriteOnly borrow when `borrowed_storage_parameter`, plain SharedBorrow, plain MutableBorrow, and `_ => malformed()` (`:682`).
- Owned whole-value path (`:490-555`): taken when `argument.path.is_empty()` or the parameter is Unrestricted. It requires `owned_source_for(argument.place, value.structural_type, parameter.multiplicity)`, the live affine frontier entries at the place to equal `whole_value_frontier(...)` exactly, and (for non-Unrestricted) the place not already in `owned_sources`. Claims at the argument place are moved to `parameter.place` (`:535-549`); for Linear a count of 0 is refused (`:550-554`).
- Owned projected path (`:556-607`): needs Affine or Linear multiplicity, a view of the same type as the parameter, an owned source of the root place under `Affine`, and a claim match: Linear needs exactly one claim with empty path and Linear multiplicity; Affine needs every selected claim to have a non-empty path (`:585-595`).
- Active claim selection for case-guarded entry claims: `case_projection_with_checked_membership_is_active` (`:756`) and `case_projection_is_active` (`:812`), keyed on stored case at a projection path.
- `borrowed_argument_source` (`:945-995`): finds the strongest multiplicity the argument place qualifies as owned (Linear, then Affine, then Unrestricted, `:963-983`), then for Unrestricted requires no frontier entry at the place, and for Affine/Linear requires `reference_carrier_access` to cover the requested access.
- `owned_result_source` (`:1060-1096`): a place is an owned source if it is a `TrivialAffineLocal` of the type with Affine multiplicity (`:1072-1079`), or an `OperationResult` whose producer operation's declared result has the same place, type and multiplicity (`:1080-1095`).
- Cleanup preparation (`:181-325`): dry-runs each `TerminalAffineCleanupAction` on clones of values, frontier and claims. Refuses with `AffineFrontierMismatch` when the action's place is an owned source of the successor, or a projected source with empty path and not `RetireHome` (`:233-239`); `DiscardRoot` requires the place unclaimed and its frontier entries to equal exactly the whole-path discard (`:246-253`).
- `settle_structural_case` (`:2341-2406`): picks the successor whose `case` equals the observed active case (`:2365-2368`); path `Case` segments must each equal the observed case at that prefix (`:2354-2360`); payload scalars are read from `structural_scalar_fields` keyed by `case_payload(root, field)` and each scalar type must equal the target parameter's `scalar_type` (`:2378-2393`).
- `settle_crash` (`:2473`): refuses unless `frontier_lower_bound` equals the sorted live claim keys (`:2488-2490`).
- `settle_external_completion` (`:2506`): requires the boundary machine's `completion` to equal `External(kind)`, exactly one scalar argument, no structural arguments, `frontier_lower_bound` strictly increasing and all live (`:2524-2533`). If a target-family `completion_callee` exists it starts a call (`:2539-2556`); otherwise it requires an external binding admission `answers(...)` for the demand (`:2561-2575`), and an `i32` signed value (`:2576-2589`).
- Env var `OMEGA_STATE_REFERENCE_DIAGNOSTICS` (`:787`): when set, an error from `bind_active_retained_arguments` is printed with `eprintln!` (`:794-798`). Diagnostics only; it does not change the result. The same variable is read in three other places (grepped): `psi/compilation/validation/src/trace_flags.rs:56`, `06_checked-trees-to-lowered-psi/src/unit_composed_control.rs:8861`, `05_typed-trees-to-checked-trees/src/flow_call_phases.rs:1697`.

### 5. Cases by shape
- `prepare_block_bindings` match on `parameter.access` (`:485-683`): the arms listed above; the final arm is `_ => return Err(malformed())` (`:682`). A `WriteOnlyBorrow` parameter is accepted only through the `borrowed_storage_parameter` arm (`:630-651`); otherwise it reaches `_` and is refused.
- Plain `SharedBorrow` arm (`:652-669`): accepted only when the parameter type `is_borrowed_byte_carrier()`, the argument path is empty and no discard is live at the argument; any other shape is `malformed()`.
- Plain `MutableBorrow` arm (`:670-681`): accepted only for a borrowed byte carrier, with no frontier entry at the argument or parameter place, and `mutable_byte_sequence_storage(argument.place)` must succeed.
- `snapshot_scalar_case` (`:2058-2103`): returns a scalar-case snapshot only for a `Sum` type whose selected case has all fields non-erased and scalar-typed, and the value has no qualifications or projected qualifications; otherwise `Ok(None)` (`:2062-2064`, `:2066-2068`, `:2078-2082`). It is used only when `self.call_stack.is_empty()`, no signature qualifications, no projected qualifications and no returned claims (`:1997-2005`), i.e. at the entry machine's return.
- `settle_return_structural` has two top-level shapes: a scalar-array source (`:1682-1711`) and everything else (`:1711-2024`). The scalar-array shape refuses non-Unrestricted multiplicity, projected qualifications, returned claims, any live claim, or a destination with projected qualifications or claims (`:1691-1701`).
- `settle_return` requires the machine result to be `Scalar` (`:2423`); `settle_return_unit` requires `Unit` (`:2456`).
- Return-destination match (`:1644-1680`): the caller's `SuspendedCallResult` must be `Structural` with equal type and multiplicity, matching qualifications, and the result place free in four caller tables and the frontier; any other caller result kind is `malformed()` (`:1678`); empty call stack gives `None`.
- `rebound_claims` (`:1895-1925`): the match on `ResultClaimOrigin` lists three variants (`CalleeReturn`, `CheckedProvider`, `TargetFamilyReturn`), all handled identically.
- `install_return` (`:1342-1462`): match on `SuspendedCallResult`, with variants NoNormal, ExternalCompletion, NominalCleanups, Unit, Scalar, Structural. NoNormal and ExternalCompletion are checked twice, before (`:1348-1359`) and inside the match (`:1365-1376`).
- Return value kinds accepted into a structural result slot: `TerminalExecutionResult::Structural` or `ScalarArray`; others `malformed()` (`:1417-1443`).

### 6. Same job elsewhere
- "Remove a place from all runtime tables": open-coded at least three times in this file with slightly different table lists. `BlockBindings::commit` for owned sources removes from `structural_values`, `aggregate_argument_backings`, `byte_sequence_values`, `scalar_array_values`, `element_view_values`, `primitive_loan_values`, frontier, claims (`:105-128`); for destinations removes the same tables except claims and frontier (`:130-150`). `retire_home_companions` (`:1141-1146`) removes four tables (not `scalar_array_values`). `settle_return_structural` tail removes `structural_values`, `scalar_array_values`, `byte_sequence_values`, `element_view_values`, frontier (not `aggregate_argument_backings`, not `primitive_loan_values`) (`:2033-2047`). Read.
- "Retire a structural home": the sequence `get reference_referents origin -> push to retired_structural_homes` appears at `:109-119`, `:131-143`, `:1178-1189`, `:2033-2042` (four sites, read).
- "Dry-run cleanup actions" in `BlockBindings::prepare_cleanup_actions` (`:181`) repeats the cleanup logic of `commit_cleanup_action` (`:1163`, in custody; I did not re-read it here) on cloned tables; both call the same custody helpers (`begin_nominal_cleanup`, `complete_nominal_cleanup`, `retire_cleanup_home`, `consume_affine_discard`). `prepare_cleanup_actions` is called on a `BlockBindings::default()` by the three return settlers (`:2026`, `:2426`, `:2459`) and on real bindings by `settle_successor_with_scalars` (`:2283`).
- Case-guard filtered row selection ("keep the rows whose case projection is active, else drop") is written out at `:754-780`, `:810-821`, `:1742-1751`, `:1762-1774`, `:1969-1981`. Read.
- The three claim-aliasing/owned-source checks (`owned_source_for`) are called three times with the same shape in `prepare_block_bindings` (`:454`, `:503`, `:570`) and four times in `borrowed_argument_source` (`:963-979`).
- Suspected (not read): `storage_bytes_and_views.rs:789` also calls `owned_source_for` (grepped; not read).
- `consume_affine_projection` method at `:2195` wraps the custody free function of the same name (`engine_custody.rs:1712`); only outside caller is `storage_references.rs:3206` (grepped).

### 7. Unused or one-caller code (greps over `omega-rust`, `*.rs`, pattern `\.NAME\(`)
- `settle_invoke_no_normal`, `settle_jump`, `settle_conditional`, `settle_structural_case`, `settle_return`, `settle_return_unit`, `settle_return_structural`, `settle_crash`, `settle_external_completion`: one dispatch call site each, in `engine_execution.rs:2914-2928`. `settle_return` also has two test call sites in `boundary_values.rs:344`, `:350`.
- `reject_no_normal_return` (`pub(crate)`, `:1127`): 3 call sites, all in this file (`:1615`, `:2413`, `:2448`).
- `continue_cleanup_actions` (`pub(crate)`, `:1148`): 5 call sites in this file (`:1337`, `:2051`, `:2304`, `:2435`, `:2467`), 0 outside.
- `finish_return` (`pub(crate)`, `:1277`): 2 call sites in this file (`:1219`, `:1557` in the test). Other grep hits are `TerminalTrace...finish_return` in `terminal-psi/src/observation.rs` (a different method).
- `prepare_block_bindings` (`pub(crate)`, `:329`): 1 call site (`:2279`).
- `BlockBindings::reserve_identities` (`pub(crate)`, `:94`): 1 call site (`:2288`). `BlockBindings::retain_storage` (`pub(crate)`, `:72`): called from `storage_records_and_arrays.rs:5015` and `:5182` among the hits (those lines call `bindings.retain_storage`; other hits in that file are different types' methods). Counted by reading grep lines, not by type.
- `owned_source_for` (`pub(crate)`, `:1019`): 6 call sites in this file (`:454`, `:503`, `:570`, `:963`, `:969`, `:975`) plus 1 outside (`storage_bytes_and_views.rs:789`).
- `owned_block_source` (`pub(crate)`, `:1098`): 3 call sites in this file (`:954`, `:1002`, `:1025`) plus 2 outside (`engine_custody.rs:1080`, `storage_references.rs:2202`).
- `consume_affine_projection` method (`pub(crate)`, `:2195`): 1 outside call (`storage_references.rs:3206`).
- `live_claim_paths_at` (private, `:2172`): 3 call sites (`:516`, `:1449`, `:1873`). `stored_case_at_discard` (private, `:2182`): 4 call sites (`:518`, `:861`, `:1450`, `:1874`).
- `whole_value_frontier`: 3 call sites in this file (`:511`, `:1444`, `:1864`), 0 elsewhere (grep of `fn` count shows 1 definition, in custody). Custody documents no "shared" claim for it that I saw here.
- Comments claiming "the single/shared/canonical way": none in this file (read).
- `reading_borrow_pair_pattern!`: used at `:609` here; 70-odd uses across `omega-rust` (grepped; not counted exactly).

### 8. Stated limits
- Only comment inside the logic: `:761-764` "Conditions name the selected value, while a Referent crosses the original descriptor's captured backing. Resolve before cleanup or destination rebinding; this metadata query grants no payload read/write access."
- `UnsupportedSemanticVariant("family result projected qualification lacks transported member evidence")` at `:1949-1951`: an error for a target-family return whose result projected qualifications are missing from the returned value.
- Test comment-like assertion text "detached return stays with failed activation" (`:1567`).
- No TODO, "unsupported", or "migration" comments seen.

### 9. Fail-open risks
None seen that accept a wrong program on a missing or unknown input; the file consistently uses `malformed()` / specific errors on `None`. Items I checked specifically:
- `prepare_cleanup_actions` `_ => continue` (`:194`): applies to `DiscardRoot` and `RetireHome` which have no mutable path; so runtime-index projection selection is simply skipped for them. Traced within function: those variants carry no path to select. Not a fail-open for a bad input.
- `settle_external_completion` calls `validate_boundary_requirements(declaration, |_| None)` (`:2538`). Traced (read `engine_custody.rs:2414-2437`): the lookup returning `None` for every index makes any `requires` entry fail with `malformed()`; an empty `requires` passes. So a boundary with requirements is refused here (fail-closed), and a boundary with none is accepted. Traced.
- `settle_external_completion` `match kind { ProcessExit => {} }` (`:2590-2592`): single-variant match; adding a variant would force a compile error. Not an accept path.
- `borrowed_argument_source` (`:945`) returns `false` for a missing value or no source match; callers treat `false` as refusal (`:640`). Fail-closed.
- `snapshot_scalar_case` returning `Ok(None)` (`:2062-2082`): means "do not snapshot as a scalar case", then the value is returned as a `Structural` result (`:2010-2017`). Not an accept of a bad program; a representation choice. Traced.
- `settle_return_structural`, `destination` match `None => None` (`:1679`): empty call stack means the entry return, so no caller expectation. Then at `finish_return` a `destination.is_some()` with an empty stack is `malformed()` (`:1290-1297`). Traced.
- `prepare_block_bindings` redundant guard: `view_descriptor_parameter(parameter, ...)` is tested in the arm guard (`:610`) and again inside the arm (`:615`); the second test cannot fail. No effect on acceptance (read).
- `install_return` match on `NoNormal` / `ExternalCompletion` is unreachable after the early checks (`:1348-1359` return first), so `:1365-1376` are dead arms (inferred from reading).
- The `OMEGA_STATE_REFERENCE_DIAGNOSTICS` path only prints.

---

## Coverage
- Read in full: `engine_terminators.rs` 1-2610 (2610 lines). Read via 4 Read calls at offsets 1, 701, 1401, 2101.
- Skimmed: none.
- Not read: none. Unread ranges: none.
- Extra reads for context only (not counted): `terminal-psi/src/ownership.rs:20-31`, `engine_custody.rs:2410-2440`.

## Five things a maintainer most needs to know about this unit
1. `prepare_block_bindings` (`:329-943`) is one ~615-line function that builds the whole successor frame off to the side (no execution tables are changed) and returns a `BlockBindings`; the only commit point is `BlockBindings::commit` (`:100`), run at the end of `continue_cleanup_actions` (`:1220-1221`) after all cleanup actions.
2. Cleanup actions are applied twice: first as a dry run on clones in `prepare_cleanup_actions` (`:181-325`), then for real in `continue_cleanup_actions` via `commit_cleanup_action` (`:1163-1177`). A nominal cleanup suspends the current frame and runs the cleanup machine as a call (`start_nominal_cleanup`, `:1229-1257`; resumed in `finish_return` `:1331-1338`).
3. `settle_return_structural` (`:1610-2056`) is one ~450-line function. It checks claims returned versus claims live (`:1783-1810`), the frontier equals the expected whole-value frontier (`:1864-1894`), rebinds claims for the caller (`:1895-1925`), and special-cases target-family and checked-provider returns (`:1926-1981`). The "borrowed descriptor" condition (`:1811-1863`) is one long boolean built from about ten checks on a mutable-borrow parameter.
4. Which binding path a structural parameter takes depends on its access and type shape tried in a fixed order (primitive loan, array loan, byte loan, packed array, then access match). Any shape or access outside those paths ends in `malformed()` (`:682`).
5. Returns do not leave live state behind: `UnsettledReturnFrontier` is raised if the frontier or claims are non-empty after cleanup (`:1197-1203`); on a failed return the `CompletedReturn` is kept in `self.failed_return` so its storage stays accounted (`:1281-1308`, tested at `:1482-1600`).

## What I am unsure about
- I did not read `commit_cleanup_action`, `whole_value_frontier`, `present_structural_argument`, `bind_active_retained_arguments`'s callback contract or `complete_activation_storage`, so I cannot say whether every table removed in the four "remove a place" sites (see section 6) is the intended set; the lists differ (`:105-128`, `:130-150`, `:1141-1146`, `:2033-2047`) and I record them as different, not as wrong.
- The counts in section 7 were read off the lines of a Grep `\.(name)\(` over `omega-rust` with `glob=*.rs` (content mode, uncapped), not from count mode. `BlockBindings::retain_storage` outside-caller attribution (`storage_records_and_arrays.rs:5015`, `:5182`) is by line text `bindings.retain_storage`, not by type resolution.
- Whether `reading_borrow_pair_pattern!` use count (70-odd) matters; I did not count exactly.

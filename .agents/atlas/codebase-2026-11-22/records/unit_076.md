# Record unit_076 (area 14-interpreter), checkout 4f3c255d0559

All paths are under `source-rust/subsystems/interpreter/embedding/`. Claims are "read" unless marked. No grep counts were run (context ran out); every "other users" item is therefore "suspected / not grepped".

## 1. boundary_effects.rs (4504 lines, read in full)

Purpose: the host-boundary half of the interpreter. It defines the effect types and handler trait the embedder implements, validates a host's answer to a boundary call against the declared contract, stages byte/scalar buffers before a call and writes them back after it, and holds the runtime-supply and x86 flags types. One file holding about 10 inline modules.

Shape (inline modules, in order):
- `boundary_predicates` (10-786): `validate_input_requirements` (105), `validate_output_guarantees` (169), private `InputObservations` (247) that closes `StructuralRead`/`ViewExtent`/field terms against live storage (`close_scalar` 310, `read_extent` 381, `read` 471), `BoundScalar` (37) for undetermined-NaN. Tests 509-785 including `idle_boundary` (749).
- `byte_buffers` (787-2075): `writeback::{TerminalBoundaryByteBuffer (803), BoundaryByteBufferBinding (844: capture 852, validate 912, commit 969)}`; `BoundaryArguments` (1031) with `into_call_arguments` (1074), `validate_writeback` (1290), `commit` (1308); `TerminalExecution::resolve_boundary_arguments` (1342), `prepare_boundary_arguments` (1489), `prepare_byte_sequence_loan` (1591), `lend_declared_owner` (1820), `bounded_owner_snapshot` (1947).
- `crash_outcome` (2076-2218): `validate_boundary_crash_outcome` (2093), `BoundaryCrashOutcomeError` (2084), `validate_boundary_crash_routes` (2166).
- `normal_qualifications` (2219-2456): `capture_normal_qualifications` (2280), `normal_qualification_is_covered` (2322), `validate_normal_case_preservation` (2345).
- `original_storage` (2457-2638): data types only (`TerminalOriginalStorageRequest` 2511, `...Loan` 2576, `...Root` 2469, `...ReferenceValue` 2618).
- `results` (2639-3611): `TerminalEffectResult` (2682), `TerminalReferenceResult/Payload` (2709-2728), `admit_boundary_crash` (2731), `preflight_boundary_result` (2780), `commit_host_boundary_result` (3063), `commit_boundary_result` (3084), `requires_materialized_host_custody` (3003). Tests 3192-3610.
- `scalar_loans` (3612-4169): `TerminalBoundaryScalarBuffer` (3636), `ScalarLoanBinding` (3700), `BoundaryScalarLoans` (3731: capture 3741, validate_writeback 3848, commit 3903), `captured_scalar_restriction` (4054), `scalar_qualification_owner` (4150).
- Top level (4170-4504): `TerminalEffect` enum (4182: MemoryFence, BoundaryCall, ComponentCall, PortWrite), `TerminalEffectHandler` trait (4216) with default methods, `TerminalEffectRejection` (4308), `TerminalRuntimeSupplies` (4320), `AdmittedBoundarySupply` (4336), `TerminalRuntimeSupplyRequest` (4349), `require_compiler_evaluation` (4399), `with_component_provider` (4444), `TerminalX86FlagsEnvironment` (4484).

Inputs/outputs: reads execution state (`TerminalExecution` fields such as `structural_values`, `live_claims`, `call_stack`), `terminal_semantics` helpers, `closed_predicates`. Mutates execution only in `commit` paths (byte/scalar writeback, result publication).

How it decides:
- Input requires / output ensures are decided by `closed_guard*` evaluation, three-valued. `Some(false)` and `None` both refuse (`boundary_effects.rs:150-160`, `231-241`). Keyed on the declared proposition after substituting actual scalar values and observed storage reads.
- Crash admission keyed on the declared `CrashCause` bucket and its guards (`2126-2130`, `2142-2156`): any true alternative accepts; a bucket for the cause must exist.
- Byte writeback validated by `(argument_index, capacity, scope)` equality and by the live storage's current capacity (`917-965`).
- Scalar writeback validated by `Arc::ptr_eq` on a captured identity, argument index, field path, access, carrier, and that current storage still equals the original (`3857-3898`). Identity, not spelling.
- Result commit is a match on `(OperationResult, BoundaryMachineResult, TerminalEffectResult)` with guards on type id, multiplicity, qualifications (`3090-3186`); anything else is `malformed()`.
- `requires_materialized_host_custody` decides by walking the structural type shape (`3003-3060`).

Cases by shape (fixed lists): `commit_boundary_result` accepts only seven result pairings (3090-3186). `supports_aggregate_result` accepts Record, FixedArray, PrimitiveScalar, ByteSequence, ElementView via `AggregatePlan`, Sum only if every case field is a plain leaf, else false (2964-2980). `supports_captured_reference_result` requires every result region to be path-empty and `Parameter`-origin (2859-2865). `normal_qualification_subject` refuses `AllElements` paths with "needs an exact captured element" (2414-2418). `capture_byte_view` etc. in the next file have similar lists. Scalar loans require `PrimitiveScalar` and Shared/Mutable (`3749-3763`).

Same job elsewhere: `satisfies_source_restriction` (3715) vs scalar restriction checks in `boundary_storage_inputs.rs` (imported at its line 12, read). Two byte-snapshot paths: `bounded_owner_snapshot` (1947) and `resolve_shared_boundary_binding` (2003). Suspected duplicate of `case_projection_*` logic in `boundary_inputs.rs` (1726-2326) via `normal_qualification_subject` (2407): both walk `Case` segments against `structural_cases`; not compared line by line.

Unused/one-caller: not grepped. `TerminalEffectHandler::poll_host_computation` and `poll_effect_readiness` have default bodies returning `Ok(true)` (4223-4235); users not counted.

Stated limits (quoted): "does not support boundary storage loans" (4268); "handler does not supply structural boundary results" (4299); "handler does not supply original storage homes" (4254); "cannot yet transfer or remove opaque child storage" is in the storage_inputs file (191).

Fail-open risks:
- Default `poll_effect_readiness`/`poll_host_computation` return `Ok(true)` (4223-4235): a handler that does not override reports "ready" always. Classification: open (no comment; not traced).
- `normal_qualification_is_intrinsic` short-circuits capture and cover checks to "covered" when `scalar_projection_entails_membership` says the type entails it (2300, 2329). Traced only to the helper's name; class: open.
- `validate_boundary_crash_routes` returns Ok when `crash_routes` is empty (2189), then `validate_boundary_crash_outcome` fails with `UndeclaredCause` because no bucket is found (2126-2130). So empty accepts nothing; none seen.
- `TerminalX86FlagsEnvironment::new` allows any RFLAGS with bit 1 set and no bits outside mask `0x3f7fd7` (4489); stated by shape only.
- `validate_input_requirements` returns Ok immediately if `scalar_requires` is empty (113); non-scalar `requires` (the `requires` field in `idle_boundary`, 770) are not evaluated here. Whether another site covers them is not traced: open.

## 2. boundary_inputs.rs (3426 lines, read in full)

Purpose: host-supplied entry inputs and their binding into runtime storage before execution: access views, byte arrays, case membership, structural contents, initialization overlap checks, scalar fields. Ends with `TerminalStructuralInputs` (3414).

Shape:
- `access_views` (10-321): `establish_access_view_inputs` (46), `bind_call_access_views` (180), `AccessViewOccurrence` (21).
- `byte_arrays` (322-1725): inner `storage` module (323-972) with `StructuralByteArrayStorage` (340; Local bytes or Cells backing, 346), `ArrayStorage`/`ArrayStorageHome` (603-616), reads/writes on `TerminalExecution` (628-886); `byte_array_length` (1001), `array_path_type` (1030), `bind_byte_arrays` (1067), `establish_byte_array_binding` (1295), `prepare_array_view_argument_staged` (1441), `prepare_fixed_array_argument_staged` (1626).
- `case_membership` (1726-2326): `stored_case_at_projection` (1749), `active_field_declarations` (1769), `live_reference_storage_owner` (1837), `actual_storage_type_from_owner` (1932), `walk_actual_storage_path` (1985), `case_payload_projection` (2082), `case_projection_with_checked_membership_is_active` (2280).
- `contents` (2327-2958): `bind_structural_contents` (2359), `stage_structural_contents` (2418), `stage_immutable_contents` (2717).
- `initialization::validate` (2960-3056), `scalar_fields` (3057-3386: `entry::validate` 3086, `bind` 3284).

How it decides:
- Access view establishments: matched by `TerminalAccessViewInput` row equality and by overlap of runtime places (`opaque_identity` plus path prefix), 29-36, 101-111. Aliasing refused when either access is exclusive.
- Call-time access views match by the tuple of view/policy/schema/placement identities and access coverage (216-231, 248-267), not by names.
- Byte arrays: element type must be unsigned 8-bit non-address integer (1019-1027, 701).
- Case activity: a payload is active iff the stored case id at each `Case` prefix equals the segment; missing stored case is an error, never "inactive" (comment 2278-2279, code 2305).
- Contents: leaf count must equal declared complete leaf count (2436), paths must be strictly ordered (2428).

Cases by shape: `bind_byte_arrays` accepts paths of only `Field`/`FixedIndex` segments (1075-1082); parameters only Owned or borrowed, nonlinear multiplicity, no qualifications (1087-1097). `byte_array_length` handles only `BoundedView` byte sequence or fixed array of u8 with literal extent (1005-1027). `entry::validate_type` handles Record and FixedArray; Sum/Mixed/TypeParameter/Reference/AtomicCell return `malformed()` (3183-3188). `stage_immutable_sequence_contents` requires `ElementView` with None or SharedBorrow access (2660-2664).

Same job elsewhere: `case_projection_in_value_is_active` (2241) vs `normal_qualification_subject` Case walk in boundary_effects.rs (2419-2444) (read both). `byte_array_length` and the `scalar_array_leaf_shape` u8 test at 699-702 repeat the same "u8, unsigned, not address" predicate (also `scalar_byte` 898-901). `requires_contents` (3198) resembles `requires_materialized_host_custody` (boundary_effects.rs 3003): both walk type shapes for scalar-evidence fields; suspected duplicate, not compared.

Stated limits: "contents cannot establish erased bindings" (2452); "scalar/case contents cannot establish opaque or reference backing" (2471); "sequence contents have no complete immutable roster" (2682); "Retained rows can belong to an inactive case" (1866).

Fail-open risks: none seen that accept a wrong program. `case_projection_is_active` returns `true` when the path has no `Case` segment (2231-2236): stated by shape (nothing to check). `initialization::validate` returns Ok if no initializers are present; overlap check only compares argument roots present in `inputs.arguments` (3008-3020): an `other_index` out of range yields `false` (no overlap) via `is_some_and` (3012): open, not traced.

## 3. boundary_storage_inputs.rs (2698 lines; lines 1-1567 read; 1568-2698 NOT read)

Read part: `borrowed_completion::{stage (25), commit (273)}` verifies a host-completed structural loan against original access and restrictions and stages replacement; `byte_views` (297-585: `TerminalBoundaryByteViewBuffer` 314, `BoundaryByteViewBinding` 350, `capture_byte_view` 361, `validate_byte_views` 441, `commit_byte_views` 508); `original_residence` (586-...: `prepare_original_storage` 784, `capture_byte_view` 612, `original_scalar_owner` 1208, `original_storage_owner` 1251, `original_storage_request` 1299, `original_scalar` 1345, start of `capture_graph` 1456).
Decisions: shared loans must be byte-equal to the original (34-35, 471-475); mutable byte-view completion needs `acknowledge_original_write` (463, 336-347); changed case triggers `validate_normal_case_preservation` (115-133).
Stated limits (quoted): "case replacement cannot yet transfer or remove opaque child storage" (191); "...cannot establish new opaque child storage" (202); "...lacks a semantic codec for new byte fields" (210); "original byte-view capture does not support a replaceable bounded byte owner" (410); "native original reference graph must have a finite acyclic declaration" (1474).
Fail-open: none seen in read part. `original_storage_owner` ends with owner fallback via `live_reference_storage_owner` then errors if none (1273-1290): traced, errors.
Not read: 1568-2698 (rest of `capture_graph`, `BoundaryStorageInputs` struct and its capture/validate/commit methods, `TerminalBoundaryStorageBuffer`, `StructuralLoanBinding`).

## 4. boundary_values.rs (2764 lines; lines 1-1562 read; 1563-2764 NOT read)

Read part: `entry_results` (10-447: `request_entry_results` 85, `observe_completed_entry_result` 125, `require_entry_results_complete` 183, `observed_entry_result` 194; ties to `scalar_entry_checks` discharge 158-170), `errors` (449-818: the `TerminalInterpretError` enum 460-699, `malformed()` 721 which records `Location::caller()` so the refusal names its check line), `memory_ordering` (820-847), `native_code_layouts` (849-942, `linked_image` 875 validates sorted unique machines, non-overlapping regions, entry inside a region), `result_custody` (944-1190, `ResultCustody` counts cells and retained payload bytes and charges `charge_result_custody`), `results` (1192-1408: `TerminalExecutionResult` 1206, `TerminalExecutionStatus` 1356, `TerminalCrash` 1386, `TerminalScalarCaseResult::collect` 1280), start of `values`/`aggregates` (1410-1562: `TerminalAggregateValue` 1424, `AggregatePlan::new` 1527).
Decisions: entry-result observations require exact operation id and scalar carrier match (105-114). `AggregatePlan::new` refuses quotient carriers and recursive types (1539-1543).
Stated limits: "entry returned without completing every requested result occurrence" (188).
Fail-open: `observe_completed_entry_result` returns Ok without recording when called inside a nested call or for a different machine or unrequested result (129-141): by design (stated by test name 325, `entry_results_ignore_recursive_activation...`); the completeness check is at 183-191. `charge_completed_result` returns Ok when no account is present (1118-1120): stated by shape (no sponsor, no charge).
Not read: 1563-2764 (rest of `AggregatePlan`, other `values` items, remaining modules).

## 5. call_observations.rs (290 lines, read) and call_observations_tests.rs (344 lines, read)

Purpose: lets a compiler evaluation ask to see the argument passed at a chosen direct call site. `TerminalCallObservation` (14), `ObservedTerminalCall` (23), `CallObservations` (28), `select_call_observations` (71), `observe_direct_call` (135), `capture_call_argument` (166).
Decision: keyed on `(caller machine, InvocationSite::Operation id, callee, argument position)`, binary-searched in a sorted selector list (144-163). Duplicates refused (117-121). Only integer scalar or an Owned scalar-case structural argument with no opaque representation (178-208, 223-232). Cases-by-shape comment: "requires a direct call with a numeric or owned plain scalar-case argument" (49).
Storage is leased from the sponsor account; failed invocations drop the records (31-32); `finish` charges custody once (54-67). Requires an account, else `unsupported()` (81, 257).
Tests: two tests (3, 212). Second shows crashed invocations produce no records (306-308) and records follow execution order, not selector order (294).
Fail-open: `observe_direct_call` ignores a call with no matching selector (loop ends, 146-163); stated by design ("Runtime and component execution install no observer", doc line 2). `observe_direct_call` does `.ok_or_else(malformed)` when observations absent (142), so callers must gate (caller not read; open).

## 6. layout_target.rs (74 lines, read)

`IssuedLayoutTarget` (12) holds `layout_plans::LayoutTargetFacts` and the issued argument values; `snapshot` (18) answers a boundary call by returning profile bytes, pointer size, alignment, little-endian flag as a `TerminalEffectResult::Aggregate`. Decision: the call must be a `BoundaryCall` with exactly one structural argument that is `contains`-equal to an issued argument (25-39); field ids come from `terminal_semantics::calls::layout_target::snapshot_fields` (40). Uses `.expect("u64")` (42), which cannot fail for a valid width. Fail-open: none seen.

## 7. continuation.rs (955 lines, read)

Purpose: when a paused Build entry resumes under a new publication, check that every retained path, type, field, case and membership has a translation in the new module and rewrite them. Entry `relocate_continuation_storage` (107), `relocate_suspended_callers` (32), `HeldCoordinates` (320, the "can this be translated" checker), `Coordinates` (561, the translator; `relocate_frame` 819).
How it decides: refuses outright when the execution retains artifact-bound state: entry reference backings, original storage roots/cells, access-view occurrences, component lent atoms/addresses, erased fields, effects other than BoundaryCall/ComponentCall (120-133); any non-literal scalar, host address or resident backing (136-150); any element view or placeholder `placed`/`collection_home`/`array_loan` (204-227); representation-byte endpoints and unsupported byte descriptors (508, 536-538, 769). Otherwise it requires every type id, field identity, domain and case to be present in the `CheckedPrefix` map (`ty` 332, `path` 409, `memberships` 597). `RuntimeIndex` path segments always refuse (418, 637).
Cases by shape: `relocate_suspended_callers` pairs only Unit/Scalar/Structural results and requires `profile_completion.is_none()` (56-86); anything else "cannot transfer this caller's return obligations". `binding` supports only `Immutable` and `MutableArray` (751-770).
Same job elsewhere: `HeldCoordinates` (check) and `Coordinates` (rewrite) walk the same shapes twice; `relocate_checked_entry` methods elsewhere (e.g. `StructuralByteArrayStorage::relocate_checked_entry`, boundary_inputs.rs 369) are the per-type hooks (read). Other `relocate_checked_entry` implementations: suspected, not grepped.
Stated limits (quoted): "continuation retains an artifact-bound address, authority or fact" (134); "continuation cannot establish the declaration of a retained field" (285).
Fail-open: `covered_place`/`covered_field` return true when `complete_layout()` holds, i.e. every declared type translates (361, 386, 348-357): comment "Only that case requires a correspondence for every possible layout" (346-347). Stated. `typed_place` pushes roots without checking uniqueness; `root_type` returns None on ambiguous roots (570-574), which sends to the complete-layout fallback: traced to `covered_type`, then fallback; stated-ish. `field`/`key` fall to `unambiguous_field_id` when owner unknown (708-710): open (relies on `CheckedPrefix::unambiguous_field_id`, not read).

## 8. instance/storage_observation/tests.rs (1538 lines) and program.rs (4059 lines)

NOT READ. No claims made.

## Coverage

- Read in full: boundary_effects.rs 1-4504 (4504), boundary_inputs.rs 1-3426 (3426), call_observations.rs 1-290, call_observations_tests.rs 1-344, continuation.rs 1-955, layout_target.rs 1-74. Total 9,... = 4504+3426+290+344+955+74 = 9593.
- Read partly: boundary_storage_inputs.rs 1-1567 (1567), boundary_values.rs 1-1562 (1562). Subtotal 3129. Total read: 12,722.
- Skimmed: none.
- Not read: boundary_storage_inputs.rs 1568-2698 (1131); boundary_values.rs 1563-2764 (1202); instance/storage_observation/tests.rs 1-1538 (1538); program.rs 1-4059 (4059). Total not read: 7,930.

## Five things a maintainer most needs to know

1. Almost every refusal in the read code is `malformed()` (boundary_values.rs 721), which records the Rust source line of the check as the error; the reason is the line, not a message.
2. Host boundary answers are accepted only by a closed list of pairings in `commit_boundary_result` (boundary_effects.rs 3090-3186); a new result shape needs a new arm and a `supports_*` predicate.
3. Contract predicates are evaluated three-valued and "unknown" refuses (boundary_effects.rs 156-160, 237-241); storage predicates not observable are never assumed (comment 167-168).
4. Resume-after-new-publication (continuation.rs) refuses broadly any state tied to addresses, residents, element views, or original-storage homes; only literal, locally owned storage is translated.
5. The same shape walks recur: case-activity (boundary_inputs.rs 2241-2325 vs boundary_effects.rs 2407-2454), u8-array predicate (boundary_inputs.rs 699-702, 1019-1027, 898-901), type-shape "needs materialized custody" walks (boundary_effects.rs 3003 vs boundary_inputs.rs 3198).

## Unsure about

- The tail of boundary_storage_inputs.rs and boundary_values.rs, and all of program.rs, were not read, so the `BoundaryStorageInputs` struct, program loading and instance binding are not described.
- Whether non-scalar `requires` (boundary_effects.rs 770 field) are evaluated elsewhere.
- Whether default `Ok(true)` readiness polls in `TerminalEffectHandler` are overridden by every real handler (no grep).

# slice_0109

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0839 | omega-rust/omega/compilation/backend/artifacts/native-artifact/src/physical/derivation/internal_calls.rs:474-497 | REACHABLE | Missing provider/candidate identity text becomes `""` in settlement role/digest instead of failing derivation (read). |
| A0840 | omega-rust/omega/compilation/backend/artifacts/native-artifact/src/physical/derivation/fragment_custody.rs:401-404 | CLOSED | Out-of-range `register_operand` makes `scalar_immediate_span` return `None`, so custody rejoin fails closed (read). |
| A0841 | omega-rust/omega/compilation/backend/artifacts/native-artifact/src/runtime_provisions.rs:172-179 | REACHABLE | `selected_row_matches_requirement` fallback matches `""`/`""` without checking report identity (read). |
| A0842 | omega-rust/omega/compilation/backend/artifacts/native-artifact/src/runtime_provisions.rs:215 | CLOSED | Imports require a calling application; non-import mechanisms skip that check by design (read). |
| A0843 | omega-rust/omega/compilation/backend/calling-conventions/src/terminal_layout.rs:191-203 | CLOSED | Authored layout uses declared size here, but Psi rejects inconsistent field extents before this runs (read). |
| A0844 | omega-rust/omega/compilation/backend/calling-conventions/src/plans/call_plan_evaluation.rs:167-310 | REACHABLE | Structural plan validation never compares registers to policy `evaluate_*` output (read). |
| A0845 | omega-rust/omega/compilation/backend/calling-conventions/src/plans/call_plan_evaluation.rs:290-292 | REACHABLE | Result-placement registers are not added to `occupied_registers`, so param/result register overlap can pass (read). |
| A0846 | omega-rust/omega/compilation/backend/calling-conventions/src/host_operations.rs:380-394 | REACHABLE | Unknown Darwin import symbol names default to libSystem instead of rejection (read). |

## A0839

**Digest (read):** area_02 preamble (`digests/area_02.md:1-8`) — `NA/` = `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/`.

**Cited site (read):** `derive_boundary_internal_call_child` `InternalCallSelection::Provider` arm (`internal_calls.rs:462-510`). For `conformance.provider_identity` and `conformance.candidate_identity`, `module.identities.text(..).unwrap_or_default()` feeds `installed_provider_internal_call_identity` (`:476-481`) and copies the same into `BoundaryTraitSettlementRole::InstalledProviderInternalCall` (`:488-497`). No `Err` when text is absent.

**Enclosing function (read):** `derive_boundary_internal_call_child` (`internal_calls.rs:194-559`) returns `Ok(Some(NativePhysicalChild))` on success; byte equality is checked at `:441-443`; missing identity text does not abort.

**Callers (search):** `rg '\bderive_boundary_internal_call_child\b' omega-rust` — 4 matches: definition `internal_calls.rs:1`, calls `evidence.rs:3` (`:240`, `:341` inferred from unit_002).

**Upward (read):** `evidence.rs:240-259` pushes the child into physical evidence; no follow-up check of non-empty provider/candidate strings.

**Input (plain words):** A sealed native artifact replay where the Terminal module’s identity table lacks strings for an installed provider’s provider/candidate identity ids still derives an internal-call physical child with empty identity fields in the parent settlement digest.

**Verdict:** REACHABLE (read).

## A0840

**Cited site (read):** In `fragment_scalar_argument_custody_rejoin` (`fragment_custody.rs:379-404`), when `native_position` is not in `register_positions`, `register_operand = register_positions.iter().position(..).unwrap_or(register_arguments)` (`:401-404`), i.e. index `register_arguments` (one past register operands).

**Enclosing functions (read):** `fragment_scalar_argument_custody_rejoin` (`:317-493`); `fragment_immediate_argument_span_rejoin` (`:496-520`) calls `image_emission::scalar_immediate_span` with that index.

**`scalar_immediate_span` (read):** `object_artifact.rs:3221-3242` — `instruction.operands.get(register_operand)?` returns `None` when the index is out of range; no panic on the `unwrap_or(register_arguments)` path.

**Callers (search):** `fragment_scalar_argument_custody_rejoin` — caller `fragment_custody.rs:241` only (`fragment_normalized_foreign_rejoin`).

**Upward (read):** `:488-490` — failed span rejoin makes `fragment_scalar_argument_custody_rejoin` return `false`; foreign rejoin errors (`:249-250`).

**Verdict:** CLOSED (read).

## A0841

**Cited sites (read):** `TerminalRuntimeHostProvisions::capture_native_admission` (`runtime_provisions.rs:148-249`) passes `identities.text(boundary.identity).unwrap_or_default()` and `binding.requirement_identity(selected.identities()).unwrap_or_default()` into `selected.selected_row_matches_requirement` (`:172-178`, filter at `:186-190`); stores `unwrap_or_default()` strings in `RuntimeHostRequirement` (`:229-236`).

**`selected_row_matches_requirement` (read):** `provider_evidence.rs:738-760` — row lookups by requirement string first; if none match, final arm is `requirement == selected_requirement` (`:759`) with **no** `provider_plan_report_identity` test.

**Callers of `capture_native_admission` (search):** `rg '\bcapture_native_admission\b' omega-rust` — 7 matches: `runtime_provisions.rs:4`, `callable_objects.rs:2`, `realization.rs:1`.

**Input (plain words):** Runtime host provision capture where boundary identity text and selected requirement text are both missing from the identity table (both become `""`) can satisfy `selected_row_matches_requirement` via the equality fallback even when report identity should discriminate rows.

**Verdict:** REACHABLE (read).

## A0842

**Cited site (read):** `runtime_provisions.rs:200-219` — `calling_application` built from `external_bindings` only when `row.calling_application(&abi)` succeeds; if `calling_application.is_none() && binding.import().is_some()` → `Err` (`:215-218`); if `import()` is `None`, `calling_application` may stay `None` and capture continues (`:227-240`).

**Enclosing function (read):** Full `capture_native_admission` (`:148-249`).

**Input (plain words):** System-call, table, or component foreign bindings without an import locator proceed without an admitted calling-application row; that is explicit, not a silent accept of an import missing its application.

**Verdict:** CLOSED (read).

## A0843

**Cited site (read):** `resolve_structural_shape` (`terminal_layout.rs:146-203`) — for `Record` with `Authored` / `AccessView` / `Accessor` storage, returns `ValueShape::integer(storage.byte_size, storage.byte_alignment)` (`:191-202`) without walking `fields`.

**Upward (read):** `StructuralLayouts::new` (`terminal_layout.rs:46`) calls `resolve_structural_shape` on roots; used from pipeline 02, task-plans, native_execution_scope (grep: 4 production `StructuralLayouts::new` sites).

**Closing check (read):** `foundation_validation.rs:8138-8150` — authored record fields must have offsets/extents within `record.byte_size`; otherwise `malformed(...)`.

**Input (plain words):** A record whose authored byte size does not cover its fields should be rejected in Terminal Psi validation before backend `terminal_layout` sizing is consulted on normal compile paths.

**Verdict:** CLOSED (read).

## A0844

**Cited site (read):** `validate_call_plan_structure` (`call_plan_evaluation.rs:167-310`) checks parameter count, result shape, stack alignment, per-parameter `validate_value_placement`, architecture on clobbers/entry control — never invokes `evaluate_microsoft_x64` / `evaluate_system_v_amd64` / etc. Module header (`:1-8`) states validation of “any proposed `CallPlan`”.

**Enclosing validation (read):** `validate_call_plan` (`:153-165`) → `validate_boundary_entry_plan` (`plans.rs:405-411`) used on retained/admitted `BoundaryEntryPlan` values.

**Callers (search):** `rg '\bvalidate_boundary_entry_plan\b' omega-rust` — 19 files (provider-admission, provider-planning, pipelines 02/03, selected-instructions, etc.); production paths include `settlements.rs:592`, `selection_call_abi.rs:1257`, `descriptor_input.rs:167`.

**Input (plain words):** A boundary entry plan whose call subplan assigns arbitrary valid-architecture registers and stack slots that still satisfy coverage/disjointness can pass validation even if those registers differ from what the policy evaluator would assign.

**Verdict:** REACHABLE (read).

## A0845

**Cited site (read):** `validate_call_plan_structure` (`call_plan_evaluation.rs:189-292`) — parameters add registers to `occupied_registers` (`:205-217`, indirect pointers `:242-254`); result branch (`:290-292`) only calls `validate_value_placement` and does not union result registers into `occupied_registers`.

**Input (plain words):** A proposed call plan with an indirect-return pointer register reused for a parameter register can pass `validate_call_plan_structure` because the result placement never participates in the occupied-register set.

**Verdict:** REACHABLE (read). (Whether every admission path can emit such a plan was not traced; overlap is not rejected at this site.)

## A0846

**Cited site (read):** `darwin_import_library` (`host_operations.rs:380-394`) — prefix tests for libobjc and CoreGraphics; final `DARWIN_LIBSYSTEM_PATH` for all other symbols.

**Callers (search):** `rg '\bdarwin_import_library\b' omega-rust` — 6 matches: definition + re-export in `host_operations.rs:2`, `lib.rs:1`, production uses `dyld_linking.rs:3` (`:117`, `:1608`).

**Upward (read):** `dyld_linking.rs:115-125` — `StringBackedBootstrap` sets library bytes from `darwin_import_library(&symbol.name)`; symbol name non-empty required (`:118-122`) but spelling is not validated against a catalog here.

**Input (plain words):** A Mach-O bootstrap import whose symbol name is not a known Darwin prefix is bound to libSystem at image planning time instead of being rejected as an unknown symbol.

**Verdict:** REACHABLE (read). Pre-validation of symbol spellings before image emission was not traced.

## What I did not read

- `git show 457adf4a11f:` paths (current checkout lines matched citations).
- Whether broken identity tables can reach `derive_boundary_internal_call_child` on production compile paths (only derivation code and `evidence.rs` callers read).
- Every `validate_boundary_entry_plan` call site for hand-authored vs evaluator-produced plans.
- Mach-O / foreign-call paths that populate import symbol names before `dyld_linking.rs`.
- `installed_provider_internal_call_identity` hashing details beyond the empty-string inputs at `internal_calls.rs:467-484`.

SLICE COMPLETE

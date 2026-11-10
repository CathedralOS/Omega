# slice_0115

Digest preamble (area_03, lines 1–7, read): paths under `omega-rust/omega/compilation/backend/` unless noted; `ME/` = `machine-emission/src/`; layout crate under `layout/`; `backend-plan` under `plans/backend-plan/`.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0886 | layout/src/lib.rs:138-143 | UNDECIDED | `Ok(None)` follows `for_target` only on the natural-layout branch; whether an authored record can reach that shape was not traced in Terminal admission. |
| A0887 | layout/src/boundary_values.rs:215-217 | REACHABLE | Supplied record storage replaces computed size and alignment with no check that fields still fit inside. |
| A0888 | ME/exit_contract.rs:2584-2597 | REACHABLE | Catch-all `NoneV1` arm accepts any kind not on the manual exclusion list when stack and control checks pass. |
| A0889 | ME/exit_contract.rs:2332, 2376 | REACHABLE | `CopyBytes` and `ScanByteSequence` match arms return `true` without trap or operand-list checks unlike sibling kinds. |
| A0890 | object/object-file/src/names.rs:79-85 | UNDECIDED | Invalid handles yield `""`; production callers traced, but whether stale handles reach those calls was not shown. |
| A0891 | plans/backend-plan/src/direct_boundary_leaves.rs:89-91 | REACHABLE | `Intrinsic` execution returns `Ok(())` before foreign calling-roster and commitment comparisons run. |
| A0892 | plans/backend-plan/src/direct_boundary_leaves.rs:96, 116 | REACHABLE | Empty `calling_for_requirement` skips selected-roster comparison while still iterating binding applications. |
| A0893 | plans/backend-plan/src/synchronous_invocations.rs:200-205 | REACHABLE | No matching selected plan adds neither a cycle edge nor a diagnostic. |

## A0886

Cited site (read): `terminal_layout_identity` in `omega-rust/omega/compilation/backend/layout/src/lib.rs:129-182`. After resolving the native layout record (`:134-137`), it calls `module.native_layouts.for_target(native, target.target_name())?` (`:138-140`); on `None` it returns `Ok(None)` (`:141-143`), otherwise it continues identity and geometry checks (`:144-172`).

`NativeLayoutCatalog::for_target` (read, re-located in checkout): `omega-rust/psi/compilation/representations/terminal-psi/src/lib.rs:636-662`. With `policy_identity` absent, it returns `Ok(None)` only when `realizations` is empty (`:642-646`); absent policy with nonempty realizations is `Err` (`:643-644`); present policy with no matching realization is `Err` (`:655-656`).

Caller search: `rg '\bterminal_layout_identity\b' omega-rust --glob '*.rs'` → 6 matches in 4 files (read): 1 definition (`layout/src/lib.rs:129`), 1 production call (`layout/src/terminal_callback_layouts.rs:124`), 2 test calls (`compiler/tests/support/native_layout_controls.rs:49, 52`), 2 lines in `data_layouts.rs` name a local `terminal_layout_identity` variable (`provider-planning/.../data_layouts.rs:186, 207`), not this function.

Production upward trace (read): `TerminalCallbackLayouts::close_record` (`terminal_callback_layouts.rs:108+`) loads `authored = terminal_layout_identity(...)?` (`:124`); when `authored.is_none()` it takes the natural callback field path (`:160-168`) instead of plan-laid coordinates.

`for_target` caller search: `rg '\.for_target\(' omega-rust/psi/compilation/representations/terminal-psi --glob '*.rs'` → uses in `semantics/src/storage.rs` (e.g. `:2407`) and the definition in `src/lib.rs` (read).

Input (plain words): a Terminal module whose native layout catalog marks a callback-backing record as natural (absent policy, empty realizations) while the compile product still requires target-specific authored callback slots for that record on the selected target.

Verdict **UNDECIDED**: `for_target` and `terminal_layout_identity` close the authored-vs-natural split at `lib.rs:642-656`, and `close_record` intentionally follows natural geometry when `authored` is `None` (`terminal_callback_layouts.rs:160`). I did not read Terminal lowering or admission that would show whether a record that must be plan-laid can be admitted with absent policy and empty realizations; that would decide reachability.

## A0887

Enclosing function (read): `BoundaryValueLayout::record` in `boundary_values.rs:175-249` accumulates field offsets and `size`/`alignment` from recipes (`:182-214`). When `storage: Option<&BoundaryRecordStorage>` is `Some`, it assigns `size = storage.byte_size` and `alignment = storage.byte_alignment` (`:215-217`) without comparing against the maximum `field_offset + shape.byte_size` already computed.

Upstream (read): `TerminalBoundaryRecipes::record` (`boundary_values.rs:458-523`) builds `BoundaryRecordStorage` from `foreign_record_storage` (`:465`, `terminal-psi/semantics/src/storage.rs:2214-2248`), mapping `TargetLayout` and `Authored` storage into optional packed dimensions (`:518-521`). `foreign_record_storage` does not re-validate that storage extents cover fields (read).

Call path (read): `value_type` → `BoundaryValueRecipe::Record` arm (`boundary_values.rs:133`) → `record(...)`.

Input (plain words): a by-value boundary record whose Terminal target-layout realization advertises a packed byte size smaller than the sum of its field placements, while field recipes still parse.

Verdict **REACHABLE**: `record` emits a boundary shape using the undersized storage numbers; no branch in `:215-249` rejects field overflow against `storage.byte_size`.

## A0888

Enclosing function (read): `validate_non_return` in `ME/exit_contract.rs:2094-2607`. It derives `expected_control` (`:2106-2143`, default `FallThroughV1` at `:2142`), loads decoded effects from encoding/layout (`:2149-2178`), checks stack (`:2180-2183`), then `memory_matches` (`:2184-2599`).

Cited arm (read): `(_, MachineEncodedMemoryEffect::NoneV1, _) => !matches!(kind, ...)` (`:2584-2597`) returns true for any instruction kind not listed in the exclusion set when memory decodes as `NoneV1`. On mismatch, `NonReturnMemoryEffect` (`:2600-2601`); control still compared at `:2603-2605`.

Caller search: `rg '\bvalidate_non_return\b' omega-rust --glob '*.rs'` → 7 matches, all in `ME/exit_contract.rs` (read): production calls at `:638`, `:1130`, `:2827`, `:2901`, `:2926`; tests at `:2827+`.

Upward (read): non-return machine instructions in whole-function exit contract validation (`compute_inner` paths around `:638` and `:1130`) reach `validate_non_return` after stack-pointer checks; compilation does not accept the function unless this returns `Ok(())` (module header `:4-5`, read).

Input (plain words): a allocated non-return instruction whose selected kind performs memory but whose retained encoded footprint reports `MachineEncodedMemoryEffect::NoneV1`, where the kind is not yet on the manual exclusion list at `:2586-2597`.

Verdict **REACHABLE**: memory validation accepts the instruction whenever stack and default fall-through control match; adding kinds requires editing the exclusion list by hand.

## A0889

Same function `validate_non_return` (read). Cited arms:

- `SelectedInstructionKind::CopyBytes` with `MachineEncodedMemoryEffect::CopyBytesV1 { ... }` (`:2325-2332`) → `true` with no `effects.trap` or operand vector checks.
- `SelectedInstructionKind::ScanByteSequence { .. }` with `ScanByteSequenceV1` (`:2370-2376`) → `true` likewise.

Contrast (read): `ZeroBytes` arm at `:2334-2343` requires `MayArchitecturalFaultV1` trap and specific operand read/write lists; `LoadEncodedScalar` arms at `:2352-2367` also check operands and trap.

Caller trace: same five `validate_non_return` call sites in `exit_contract.rs` (read, search above).

Input (plain words): a `CopyBytes` or `ScanByteSequence` non-return site whose decoded footprint omits the architectural-fault trap behavior or lists wrong external operands, while memory and control variants still pair with the kind enum.

Verdict **REACHABLE**: the two arms trust the paired memory effect variant only; mismatching trap or operands still yields `memory_matches == true` and the function continues to control checking.

## A0890

Cited site (read): `object_symbol_name` in `omega-rust/omega/compilation/backend/object/object-file/src/names.rs:79-85` returns `object.layout.symbols.get(symbol).name` when `object.layout.symbols.is_valid(symbol)` (`:80-81`), else `""` (`:82-83`).

Caller search: `rg '\bobject_symbol_name\b' omega-rust --glob '*.rs'` → 5 matches (read): 1 definition, 1 re-export (`object-file/src/lib.rs:85`), 2 production uses in `images/image-emission/src/image.rs:945, 996`, 1 in `images/image-emission/src/entries.rs:3936`.

Upward (read): `image.rs:944-958` matches compiler regions where `region.symbol == symbol` for each `artifact.functions` entry; `image.rs:996` does the same for private helper symbols. Nearby path for private callbacks uses `object_function_symbol` (`:963-968`), which goes through `object_function_reference` and rejects invalid identities (`names.rs:100-101`, read).

Input (plain words): a machine function entry bound to an out-of-range or stale `ObjectSymbolHandle` in an otherwise emitted object plan, reaching image region binding without a prior `is_valid` check on that handle.

Verdict **UNDECIDED**: callers and the empty-string behavior are read; I did not trace object-plan construction to show whether invalid handles can appear on `artifact.functions[].symbol` at `image.rs:945`. That construction trace would decide reachability.

## A0891

Enclosing function (read): `DirectBoundaryLeaf::validate_calling_custody` in `plans/backend-plan/src/direct_boundary_leaves.rs:25-130`. Schema and plan consistency checks run for all executions (`:26-88`). At `:89-91`, `let DirectBoundaryExecution::Foreign(Ok(binding)) = &self.execution else { return Ok(()); };` — `Intrinsic(_)` and `Foreign(Err(_))` exit before foreign route and calling checks.

Skipped checks (read): requirement route match (`:92-94`), `calling_for_requirement` roster length (`:95-99`), per-application ABI duplication and commitment checks (`:101-127`).

Caller search: `rg '\bvalidate_calling_custody\b' omega-rust --glob '*.rs'` → 6 matches in 4 files (read): definition plus `validate_bound_calling_custody` call (`direct_boundary_leaves.rs:136`), production `provider_rows.rs:208`, pipeline `retained_terminal_codec.rs:249, 277`, test `filesystem_receiver.rs:174`.

Input (plain words): a direct boundary leaf classified as compiler-intrinsic whose satisfied schema and plan were rewritten earlier in the pipeline but whose foreign-style calling roster would fail the `Foreign(Ok(_))` comparisons if execution were foreign.

Verdict **REACHABLE**: intrinsic leaves never run roster or commitment validation at `:95-127`; admission depends on earlier passes only (`provider_rows.rs:208`, read).

## A0892

Same function (read). `expected = source.calling_for_requirement(original.requirement_identity)` at `:95` (`ServiceSchema::calling_for_requirement`, `plans/effects/.../service_schemas.rs:87-98`, read).

Length guard (read): `:96-99` errors only when `!expected.is_empty()` and lengths differ. The loop at `:101-127` always walks `binding.calling_applications`, but selected comparison at `:116-127` runs only when `!expected.is_empty()`.

Input (plain words): a direct foreign boundary leaf whose original service schema lists no selected calling applications for the requirement, while the retained foreign binding still carries one or more `calling_applications` entries.

Verdict **REACHABLE**: empty `expected` skips fingerprint and commitment equality checks; applications are only checked for internal duplication and self-consistency (`:101-115`).

## A0893

Enclosing function (read): `validate_selected_synchronous_invocation_cycles` in `synchronous_invocations.rs:134-239`. For each resolved invocation target (`:194-199`), `matching_targets` filters selected plans with `invoked.matches(target)` (`:195-198`).

Cited branch (read): `match matching_targets.as_slice() { [] => {}` (`:200-201`) — no edge, no diagnostic. Single-target arm pushes a cycle edge (`:202-204`); ambiguous multi-match errors (`:206-212`).

`matches` for `Application` (read): `package.is_none()` matches any package at `:108` (`:106-109`).

Caller search: `rg '\bvalidate_selected_synchronous_invocation_cycles\b' omega-rust --glob '*.rs'` → 4 matches (read): definition, call from `target_providers.rs:9` import path, and invocations within `synchronous_invocations.rs` and `target_providers.rs` (grep shows 2 files with 2 lines each).

Upward (read): `target_providers.rs` closes synchronous invocation cycles during target provider selection before publication (module header `:1-6`, read); empty `[]` branch allows the scan to finish and later cycle detection to run on a sparser graph.

Input (plain words): a selected synchronous provider plan that invokes a boundary trait or application name with no corresponding entry among the selected synchronous plans in the batch (including wildcard package matching via `package: None`).

Verdict **REACHABLE**: the invocation is ignored for cycle purposes and produces no error at `:200-201`.

## What I did not read

- Terminal Psi validation or lowering that populates `NativeLayoutCatalog` and `policy_identity` for plan-laid records.
- Full `foreign_record_storage` / Checked Psi proof that target-layout storage extents always cover fields before Terminal emission.
- ISA encoding pipelines that produce `SelectedFormEncodedRowStore` footprints (whether wrong `NoneV1` can be stored before exit contract).
- Complete object-plan and image-artifact construction proving validity of every `ObjectSymbolHandle` passed to `object_symbol_name`.
- `git show 457adf4a11f:...` at each cite (current checkout symbols matched the cited ranges after re-location where noted).

SLICE COMPLETE

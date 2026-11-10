# slice_0101

Area digest preamble (read): Area 5 covers Omega backend representation records, adjacent checkers, foundation crates (`F/`), and terminal-runtime / native-provider interpreter hooks (`I/`). Path prefixes `LO/`, `TO/`, `F/`, `I/` map under `omega-rust/omega/` per `digests/area_05.md:5-25`. Citations are pinned at commit `457adf4a11f`; cited line ranges for rows A0776–A0783 matched this checkout at the stated paths (read).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0776 | LO/identity/calling.rs:393-397 | CLOSED | Bare tags encode only zero-field realization structs; binding identity still includes boundary and execution bytes at :354-355. |
| A0777 | LO/validation.rs:226 | CLOSED | `validate_source` is permissive on `target`, but legalization matches `target` to module family at `legalization_scalar_graph.rs:2485-2501`. |
| A0778 | TO/descriptor_input.rs:35-48 | REACHABLE | Descriptor-shaped native read with non–three-parameter declaration yields `Ok(None)` and no `max_bytes` guard on the lowered call. |
| A0779 | TO/values.rs:185-189 | REACHABLE | `projected_family` skips suffix extension for non-`Projected` origins inside an `Alternatives` root used when lowering/legalizing references. |
| A0780 | F/optimization-core/.../baseline.rs:162-178 | UNDECIDED | `decode` omits roster/outcome checks, but no traced path from decoded manifest log to admitting a bad Omega source program was found. |
| A0781 | F/bounded-process/src/process_child.rs:251 | REACHABLE | On `#[cfg(not(unix))]`, any `InvalidInput` from `kill` marks the container closed like Unix `ESRCH`. |
| A0782 | I/terminal-runtime/src/execution.rs:103-115 | CLOSED | CLI maps `comparison_unavailable()` errors to `InterpreterComparison::Declined` at `application/cli/src/execution/mod.rs:544`. |
| A0783 | I/terminal-runtime/.../allocation.rs:360 | REACHABLE | After `free`, `backing_for_range` still finds the row; `memset` passes the handler liveness check though backing is retired. |

## A0776

**Cited site (read):** `omega-rust/omega/compilation/representations/target-operations/legalized-operations/src/identity/calling.rs:393-397` — `BoundaryRealization::LinuxWriteLine`, `HostedExitProcessI32`, `HostedReadByte`, `HostedWriteByteI32`, and `ClaimCompletionOnly` each `bytes.push` a discriminant only.

**Enclosing function (read):** `encode_binding` (`calling.rs:353-408`) writes `binding.boundary`, `encode_boundary_execution_identity`, then discriminates settlement realization; builtin realizations with fields (for example `MetadataOnlyPort` at :381-386, `NearestIeeeFloatArithmetic` at :398-405) encode payload bytes.

**Callers (search):** pattern `\bencode_binding\b` in `omega-rust` — 8 occurrences: 1 definition (`calling.rs:353`), 2 call sites in `identity/scalar_graph.rs:1532`, re-export path via `identity/calling.rs:8` comment; also `encode_bindings` in `legalized-operations/src/identity.rs:148` and package review encoders (read count).

**Code vs digest (read):** `target-operations/src/boundary/realizations.rs:22-23,31-32,40-41,79-83` — the five cited variants are unit structs (`Default`) with no fields; digest “bare tag” matches the type shapes (read).

**Closing check (read):** Settlement identity is not tag-only: `calling.rs:354-355` hashes boundary id and execution binding before the realization tag. Distinct realizations with empty payloads are intentionally indistinguishable in the identity bytes because the enum carries no data.

**Why CLOSED:** No program acceptance branch consumes these tags without the preceding boundary/execution bytes; there is no extra payload for these five variants that identity encoding could drop.

## A0777

**Cited site (read):** `legalized-operations/src/validation.rs:219-229` — `NativeCallOrigin::TargetFamily` accepts when `call.target.machine() == Some(*member)`, `!target.is_empty()`, then runs `validate_completion`; the `target` string is not compared to module family metadata here.

**Enclosing function (read):** `validate_call_source` (`validation.rs:142-231`); `LegalizedScalarCall::validate_source` (`:38-43`) delegates here.

**Callers (search):** pattern `\bvalidate_call_source\b|\bvalidate_source\b` on `LegalizedScalarCall` — production uses in `pipeline/03_.../selection_scalar_graph.rs:1928,4685,5814,6841` and extensive tests in `legalized-operations/src/tests.rs` (read count).

**Closing check (read):** `pipeline/03_target-operations-to-selected-instructions/src/legalization_scalar_graph.rs:2474-2503` — for `TargetFamily` calls, requires `selections.target() == target`, exactly one matching `family.members` row for `(target, callable, supply)`, and matching receipts/sources. Mismatch returns `LegalizationError::custody()`.

**Why CLOSED:** User programs reach legalized calls through pipeline 03 legalization, which binds `target` to the compiled module’s target-family table before `validate_source` runs at selection; a mismatched non-empty `target` string does not survive legalization.

## A0778

**Cited site (read):** `target-operations/src/descriptor_input.rs:35-48` — `descriptor_input_guard` returns `Ok(None)` when structural parameter count ≠ 1, `content_guarantees` non-empty, completion ≠ `NormalReturn`, or `parameter_order` is not scalar–structural–scalar.

**Enclosing function (read):** full file `descriptor_input.rs:29-363`; `derive` (`:123-185`) returns `Ok(None)` for additional guard prerequisites (access, layout, ABI validation failures as `Err`).

**Callers (search):** pattern `\bdescriptor_input_guard\b` in `omega-rust` — 7 identifier hits: 2 definitions (`descriptor_input.rs:29,65`), 1 re-export (`target-operations/src/lib.rs:83`), 4 pipeline calls (`02_.../references.rs:1569`, `02_.../unit.rs:928` uses `descriptor_input_guard_for_call`, `03_.../legalization_descriptor_inputs.rs:142`, `03_.../legalization_target.rs:3110`) (read count).

**Upward trace (read):** `02_.../unit.rs:928-941` stores `descriptor_input` from `descriptor_input_guard_for_call` (may be `None`) on `TargetUnitOperation::NormalizedForeignCall`. `03_.../legalization_target.rs:3110-3125` requires `required == *descriptor_input` but both sides can be `None`.

**Input that reaches the branch:** A normalized foreign descriptor **read** whose boundary declaration is not exactly one mutable structural parameter with two scalar parameters in the prescribed order (for example wrong parameter count or order) still lowers with `descriptor_input: None`.

**Smallest program (plain words):** A program that performs a Windows/Linux-style descriptor read through a foreign boundary whose declaration does not match the exact three-parameter read shape; the compiler attaches no `DescriptorInputGuard.max_bytes` limit because the guard helper returns “no guard” instead of rejecting the call.

## A0779

**Cited site (read):** `target-operations/src/values.rs:185-189` — in `projected_family`, only origins with `extent == ReferenceOriginExtent::Projected` receive `path.extend_from_slice(suffix)` and updated `extent`; other extents are left unchanged.

**Enclosing function (read):** `ReferenceRoot::projected_family` (`values.rs:175-200`); callers at `values.rs:165` (`projected`) and pipeline `02_.../references.rs:2546`, `03_.../legalization_reference_custody.rs:5620` (read).

**Callers (search):** pattern `\bprojected_family\b` in `omega-rust` — 4 hits: definition plus three pipeline projection sites (read count).

**Upward trace (read):** `03_.../legalization_reference_custody.rs:5618-5620` builds reference leaves with `root.projected_family(&binding.suffix, binding.extent)` for non–transfer-existing sources. `ReferenceRoot::Alternatives` can hold multiple origins (`values.rs:196-198`). Origins with `WithinRegion` or other non-`Projected` extent keep stale paths after projection.

**Input that reaches the branch:** A reference result whose `ReferenceRoot::Alternatives` mixes `Projected` and non-`Projected` origins and a binding supplies a non-empty suffix with `Projected` extent.

**Smallest program (plain words):** A function returns a reference formed from alternative borrow paths where one alternative is scoped `WithinRegion` and another is `Projected`; applying a field suffix updates only the projected alternative, and legalization can accept a combined reference leaf whose non-projected alternative omits the required suffix.

## A0780

**Cited site (read):** `foundation/optimization-core/src/optimization_core/decisions/baseline.rs:162-178` — `BaselineDecisionLog::decode` rebuilds each record via `BaselineDecisionRecord::record(input, considered, outcome)` without verifying that `Choose` lies in `considered` or that `considered` is canonical order; it only checks `canonical.identity == encoded_identity`.

**Enclosing function (read):** `decode` (`:140-184`); contrast `BaselineDecisionLogBuilder::record_validated_outcome` (`:193-219`) which rejects unknown `Choose` at `:204-208` and canonicalizes roster at `:237-243`.

**Callers (search):** pattern `\bBaselineDecisionLog::decode\b` in `omega-rust` — 6 hits: definition, tests in `baseline.rs`, and `optimization-unit/src/manifest.rs:157` inside `PrePhysicalOptimizationManifest::decode` (read count).

**Upward trace (read):** Pipeline 01 records decisions via `record_validated_outcome` (`pass_manager.rs:631-636`, `:808-817`) when optimizing from source. `manifest.rs:157-158` decodes embedded logs without a second roster check. Grep for `PrePhysicalOptimizationManifest::decode` in `omega-rust/omega` returned no compilation-pipeline consumers beyond manifest codec (read).

**UNDECIDED because:** A self-consistent tampered `OMGBDL` log with `Choose` outside `considered` is accepted by `decode` (read), but I did not trace a path where that decoded log causes the compiler to accept an Omega source program that should be rejected (fresh optimization uses the builder path).

**What would decide:** Reading every consumer that replays `manifest.decision_log` after `PrePhysicalOptimizationManifest::decode` and checking whether roster/outcome validation runs before applied optimizations affect program acceptance.

## A0781

**Cited site (read):** `foundation/bounded-process/src/process_child.rs:244-252` — `native_container_already_absent` maps Unix `raw_os_error() == 3` (`ESRCH`) to true; on `#[cfg(not(unix))]` it maps `error.kind() == io::ErrorKind::InvalidInput` to true.

**Enclosing function (read):** `terminate` (`:124-149`) treats `native_container_already_absent` like success, setting `container_closed = true` (`:130-133`, `:141-143`).

**Callers (search):** pattern `\bnative_container_already_absent\b` — 4 hits, all in `process_child.rs` (read count).

**Input that reaches the branch:** On a non-Unix hosted target (for example the `#[cfg(not(any(unix, windows)))]` child path at `:47-48,88-91`), `child.kill()` returns `ErrorKind::InvalidInput` while processes may still be running.

**Smallest program (plain words):** A bounded worker spawn on a non-Unix platform where process-group kill surfaces `InvalidInput`; termination reports the container already closed and proceeds as if the child were reaped.

## A0782

**Cited site (read):** `interpreter/terminal-runtime/src/execution.rs:101-115` — `comparison_unavailable` returns true for `MissingProvision`, and for `Requirements` / nested execution errors matching four `TerminalInterpretError` variants (unsupported capability/variant, scalar entry qualification, affine projection).

**Enclosing function (read):** `TerminalRuntimeError::comparison_unavailable` and `unavailable_before_guest_effects` (`:118-127`).

**Callers (search):** pattern `\bcomparison_unavailable\b` in `omega-rust` — 4 hits: definition, two unit tests in `execution.rs:478,481`, and CLI `application/cli/src/execution/mod.rs:544` (read count).

**Closing check (read):** `compare_interpretation` (`execution/mod.rs:542-549`) — `Some(Err(error)) if error.comparison_unavailable()` becomes `InterpreterComparison::Declined(error.to_string())`, not `Failed`. Native/interpreter disagreement is intentionally declined for these error classes.

**Why CLOSED:** The “unavailable instead of failed” behavior is the contracted CLI path for listed interpreter limitations; it does not accept an ill-typed or unsound Omega program through compilation.

## A0783

**Cited site (read):** `interpreter/terminal-runtime/src/execution/host_imports/allocation.rs:360-362` — `Initialize` (`memset`) rejects only when `values.backing_for_range(address, length).is_none()`.

**Enclosing function (read):** `AllocationImport::handle_effect_with_buffers` `Initialize` arm (`:299-368`); `AllocationValues::backing_for_range` (`:49-60`) matches address/length only, not retirement.

**Callers (search):** `backing_for_range` — 7 hits in `allocation.rs` (definition, handler, tests) (read count). `release` (`:93-107`) calls `allocation.backing.retire()` but keeps the `Allocation` row in `allocations`.

**Upward trace (read):** `TerminalProviderBacking::retire` (`terminal-interpreter/src/storage_providers.rs:117-128`) sets `retired = true` and empties bytes. Handler still finds the allocation at `:360`. Effect returns `TerminalReferenceResult` with `Zeroed` payload (`:363-367`) without re-locking backing. Later `AllocationStorage::resolve_range` (`allocation/backing.rs:51-52`) can still find the allocation; `byte_count` on retired backing errors at `:54-57` (read).

**Input that reaches the branch:** Reference execution sequence: allocate, `free` the address, then `memset`/`Initialize` the same address with zero fill and positive length.

**Smallest program (plain words):** A Terminal program that frees a heap address and then invokes the admitted zero-fill initializer on that same address; the host import treats the address as live because the allocation table still contains it after retirement.

## What I did not read

- Full `legalization_reference_custody.rs` beyond cited reference-projection sites for every `WithinRegion` combination.
- Runtime behavior of `child.kill()` on concrete non-Unix targets beyond the `cfg` branch in `process_child.rs`.
- All downstream uses of decoded `PrePhysicalOptimizationManifest.decision_log` outside `manifest.rs` and identity-bundle encoding.
- Remainder of `terminal-runtime/src/execution.rs` after line 170.

SLICE COMPLETE

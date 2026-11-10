# slice_0144

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1105 | omega-rust/psi/interpreter/terminal-interpreter/src/embedding.rs:392-397 | CLOSED | Provider-missing boundaries never enter unbound imports; invoke rejects `missing_providers` before execution. |
| A1106 | omega-rust/psi/interpreter/terminal-interpreter/src/embedding.rs:2908-2914 | REACHABLE | Missing inner `declaration` makes `compiler_owns_boundary_execution` false, so host external supply and import preflight treat the boundary as replaceable. |
| A1107 | omega-rust/psi/interpreter/terminal-interpreter/src/embedding.rs:2114-2128 | REACHABLE | With no installation layouts and no catalog layout applications, validation returns `Ok` even when other sub-claims (resolver `Ok(None)`) defer failure downstream. |
| A1108 | omega-rust/psi/interpreter/terminal-interpreter/src/engine_calls.rs:634-658 | REACHABLE | `begin_no_normal_call` and companions skip callee-result and operand checks named in the digest (table id in `u8`, skipped params, namespace compare, port forward). |
| A1109 | omega-rust/psi/interpreter/terminal-interpreter/src/engine_custody.rs:556-604 | REACHABLE | `require_owned_value_claims` returns `Ok` when the owned path leaves inline storage, so no live-claim validation runs on that operand. |
| A1110 | omega-rust/psi/interpreter/terminal-interpreter/src/engine_structural.rs:1322-1324 | REACHABLE | Aggregate-view windows skip membership repair; leaf copy accepts quotient adaptation and shared-descriptor match instead of exact type equality. |
| A1111 | omega-rust/psi/interpreter/terminal-interpreter/src/engine_execution.rs:44 | REACHABLE | Atomic store/swap allows `SharedBorrow` arrival; preflight skips compiler-owned boundaries; `AtomicFence` and partial Rflags/return qualification paths advance without full checks. |
| A1112 | omega-rust/psi/interpreter/terminal-interpreter/src/storage_bytes_and_views.rs:3009-3016 | REACHABLE | Home-less descriptor matching and empty-view subslice gates let distinct empty-path values pair with detached views. |

## A1105

**read** `embedding.rs:378-410` (`TerminalComponentBinding::bind_import` in full): supply guard at `392-397` allows host adapter when `supplies.get(&boundary)` is `None` or `HostImport` only (read).

**read** `embedding.rs:320-342`: component construction runs `execution_requirements` and rejects when `missing_providers` or `external_completions` non-empty (read).

**read** `engine_execution.rs:510-531` (capability preflight inside `check`): for `BoundaryCall`, `None` supply with `program.has_provider(boundary)` inserts `missing_providers`, not `imports`; otherwise `imports` (read).

**read** `embedding.rs:2745-2748`: `TerminalProgram::invoke` returns `ProviderInstallationMissing` when `requirements.missing_providers` is non-empty (read).

**Caller search** `\bbind_import\b` in `omega-rust`: 29 matches (read count); production definitions at `embedding.rs:378`, `embedding.rs:1413`; host/tests call the public `TerminalInstance::bind_import` (read listing).

**read** `embedding.rs:1413-1480` (`TerminalInstance::bind_import`): does not use the `392-397` supply test; still subject to invoke-time `missing_providers` (read).

**Input that reaches `392-397`:** nested component binding an import slot listed in `required_imports` where `supplies.supplies` has no entry for that boundary (read).

**Verdict CLOSED** for the digest wrong program (provider-required slot, no supply, host adapter): such a boundary is in `missing_providers`, so component `new` fails (`328-334`) and `invoke` fails (`2745-2748`) before host execution (read); host bind at `392-397` is only on the intentional import path when the artifact does not list `provider_candidates` for that boundary (`2196-2200`, read).

## A1106

**read** `embedding.rs:2908-2914` (`compiler_owns_boundary_execution` in full): true only when inner `declaration` exists and `execution != Provider` (read); `None` inner declaration yields false (read).

**read** `embedding.rs:2491-2544` (`select_external_supply` in full): rejects when `compiler_owns_boundary_execution(declaration)` at `2511-2515` (read); otherwise may `insert` external supply (read).

**read** `embedding.rs:2217-2227` (`import_declaration`): omits boundaries where `compiler_owns_boundary_execution` is true (read).

**read** `engine_execution.rs:478-532` (preflight `BoundaryCall` arm): `compiler_owns_boundary_execution` true → `continue` without import demand; false → import/host supply routing (read).

**Caller search** `\bcompiler_owns_boundary_execution\b` in `omega-rust`: 6 matches (`embedding.rs`×5, `engine_execution.rs:479`; read).

**Input that reaches the branch:** admitted program whose boundary row has `declaration: None` or provider execution, installation without a checked-provider supply entry, host selects `HostImport`/`DeferredComponent` via `select_external_supply` (read).

**Verdict REACHABLE:** any admitted callable boundary classified not compiler-owned (missing inner checked declaration) can be replaced by external supply and preflight import collection (read); whether admission can emit that shape for a boundary that must not be host-replaced was not traced outside `omega-rust/psi/interpreter/terminal-interpreter` (not read).

## A1107

**read** `embedding.rs:2099-2128` (`validate_layout_selection` in full): without `installation.target_layouts`, returns `Ok(())` when catalogs lack `target_layout_applications` and no `access_lifecycle_plans` row has `target.is_some()` (read); otherwise errors at `2114-2126` (read).

**read** `embedding.rs:2305-2314`, `2272-2277`: entry paths call `validate_layout_selection` before capability `check` (read).

**read** `embedding.rs:1778-1786` (`ImportDispatch::resolve_provider_storage`): `None` resolver returns `Ok(None)` (read).

**read** `embedding.rs:1916-1931` (`ProgramDeclarations::get`): `binary_search_by` on `declarations` and `replacements` (read); assumes sorted-by-identity catalogs (inferred from API; admission sort not read).

**read** `storage_providers.rs:2846-2858`: `resolve_provider_storage` `None` still yields `Ok(Some(PendingResourceProfile { backing: None, .. }))` (read).

**Caller search** `\bvalidate_layout_selection\b` in `omega-rust/psi/interpreter/terminal-interpreter`: 5 matches, all `embedding.rs` call sites (read).

**Verdict REACHABLE** for the layout sub-claim: program with no layout applications and no targeted lifecycle plans passes validation with `installation` lacking layouts (read). Resolver `Ok(None)` sub-claim is REACHABLE to a pending profile without host backing (`2853`, read). Unsorted-catalog mis-lookup via `binary_search` is **UNDECIDED** (admission ordering invariant not read).

## A1108

**read** `engine_calls.rs:634-657` (`begin_no_normal_call` in full): loads callee via `verified_machine` only; no callee `result` contract check unlike `begin_structural_scalar_call` at `669-672` (read).

**read** `engine_terminators.rs:2162-2168`: sole production caller of `begin_no_normal_call` from `InvokeNoNormal` (read); `reject_no_normal_return` at `1127-1137` errors if a normal return occurs while `NoNormal` suspended (read).

**read** `engine_calls.rs:271-285`, `363-376`: `continue` skips byte-sequence and element-view parameter binding when borrowed-storage ceiling is `None` (read).

**read** `engine_calls.rs:1042-1062` (`completion_callee`): non-target-family paths return `Ok(None)` except checked provider (read).

**read** `engine_calls.rs:1717-1721`: component completion rejects when `Some(completed.instance_namespace()) == self.local_structural_identities.exposure_namespace`; both `None` passes (read).

**read** `engine_calls.rs:2886-2906` (`execute_select_implementation`): stores `IntegerValue::Unsigned(u128::from(table))` without width check against result integer type (read).

**read** `engine_calls.rs:1748`, `1078-1081`: scalar component completion and `validate_result_transport` compare scalar types only (read).

**read** `engine_calls.rs:3081-3088` with `storage_providers.rs:818-859` (`release_empty_run`): `EmptyRunRelease` boundary calls validate in callee helper, not in the early gate alone (read).

**read** `engine_calls.rs:3627-3649` (`execute_port_write`): builds `TerminalEffect::PortWrite` and forwards to `handler.handle_effect` without local service/port validation (read).

**Caller search** `\bbegin_no_normal_call\b` in `omega-rust`: 2 matches (`engine_calls.rs:634`, `engine_terminators.rs:2162`; read).

**Input examples (plain words):** terminal program using `SelectImplementation` with a `u8` integer result place and dynamic table id above 255; or structural call whose byte-sequence parameter is skipped by `continue` but callee still runs (read).

**Verdict REACHABLE** per digest scenarios (read).

## A1109

**read** `engine_custody.rs:513-554` (`require_owned_value_claims` free function in full) and `556-604` (method in full): early `Ok(())` when path does not stay in inline storage (`577-586`, read) or access not owned (`561-562`, read).

**read** `engine_custody.rs:69-70`, `97-103`: `restore_store_frontier` skips `Referent` paths and returns `Ok` when `owned_window_projection` is `None` (read).

**read** `engine_custody.rs:1555-1558`: `exclude_linear_regions` `continue`s when claim place not in affine frontier roots (read).

**read** `engine_custody.rs:2067-2075`: `content_entry_claims` may `or_insert` `LiveClaim { place: None, .. }` when parameter lookup fails (read).

**Caller search** `\brequire_owned_value_claims\b` in `omega-rust/psi/interpreter/terminal-interpreter`: 7 matches (1 free fn, 1 method, call sites in `engine_terminators.rs`, `boundary_effects.rs`, `storage_records_and_arrays.rs`, `engine_custody.rs`; read).

**Input that reaches `577-586`:** owned structural operand whose projection leaves inline storage (e.g. through a reference path segment) passed into call/terminator preparation that calls `require_owned_value_claims` (read).

**Verdict REACHABLE:** claims are not required on that operand though access is `Owned` (read).

## A1110

**read** `engine_structural.rs:1300-1324` (membership repair loop): `if window.aggregate_view { continue; }` skips repair for aggregate-view windows (read).

**read** `engine_structural.rs:1815-1845` (`execute_structural_leaf_copy` segment): `quotient_copy_adaptation` or `shared_descriptor_copy_matches` can allow `leaf.structural_type != result.structural_type` without immediate error (read).

**Caller search** aggregate-view `continue` enclosing fn: `preserve_enclosing_memberships` path in structural store preparation (read enclosing block from `1280+`, not every line below `1360`).

**Verdict REACHABLE** for both digest points (read).

## A1111

**read** `engine_execution.rs:22-46` (`execute_atomic_access` entry): only `WriteOnlyBorrow` and illegal ordering rejected; `SharedBorrow` allowed for store/swap path (`44-46`, read).

**read** `engine_execution.rs:478-482` (preflight): `compiler_owns_boundary_execution` true → skip boundary host demand (read); contrast `479-483` in digest maps here (read).

**read** `engine_execution.rs:534-535`: non-matching operation kinds fall through to `direct_callee()` without capability arm (read).

**read** `engine_execution.rs:2744-2754`: Rflags write skips store when operand missing or not unsigned integer; oversized unsigned becomes `u64::MAX` via `unwrap_or` (read).

**read** `engine_execution.rs:2810-2812`: `AtomicFence` → `OperationFlow::Advance` with no capability or effect (read); compare register fence at `2726-2736` (read).

**read** `engine_execution.rs:1926-1974` (`ResultClaimOrigin` qualification helpers): `TargetFamilyReturn` and `CheckedProvider` use subset/`all` checks, not equality to full declared lists (read).

**read** `engine_execution.rs:3826-3831` (`terminal_scalar_belongs_to_type`): `IeeeFloat` arm returns true without format comparison (read).

**Verdict REACHABLE** for digest atomic, preflight, fence, Rflags, return-qualification, and float-format points (read).

## A1112

**read** `storage_bytes_and_views.rs:3009-3016`, `3571-3576` (`matches_descriptor_home` on byte and element views): `descriptor_home` `None` accepts any value with `path.is_empty()` (read).

**read** `storage_bytes_and_views.rs:3102-3118` (`subslice`): sets `descriptor_home: None` on narrowed view (read).

**read** `storage_bytes_and_views.rs:865-891` (`prepare_byte_descriptor_argument`): multiple shapes return `Ok(None)` without error (read).

**read** `storage_bytes_and_views.rs:649-673`: fallback picks first call-stack place matching identity/path/type (read).

**read** `storage_bytes_and_views.rs:1587-1593`: element-view root mutation check applies only when `parameter_root` (read).

**read** `storage_bytes_and_views.rs:4868-4872`: empty view (`view.len() == 0`) bypasses projected-qualification containment test (read).

**read** `storage_bytes_and_views.rs:4127-4158` (`WholeRepresentation` branch): inserts result and returns without `track_affine_result` (read; other branches at `2405`, `4268` call it — read grep).

**Input (plain words):** two different structural values at empty path bound to views whose `descriptor_home` was cleared by subslice/copy-on-write, accepted as the same home (read).

**Verdict REACHABLE** (read).

## What I did not read

- Terminal artifact admission rules for boundary `declaration` presence (`omega-rust/psi` outside terminal-interpreter).
- Full `release_empty_run` below `storage_providers.rs:859`.
- Full capability `check` after `engine_execution.rs:668` and all `TerminalInstance::invoke` paths beyond `2745-2768`.
- Corpus or sample programs exercising each REACHABLE shape.
- `git show 457adf4a11f` line drift check (current checkout read; digest commit `457adf4a11f` per area_15 preamble).

SLICE COMPLETE

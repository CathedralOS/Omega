slice_0102

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0784 | omega-rust/omega/compilation/compiler/src/product_execution.rs:463-491 | REACHABLE | `runtime_product=false` lets `permissions: None` skip `validate` on Check/Terminal paths even with package identity and assumed bindings. |
| A0785 | omega-rust/omega/compilation/compiler/src/terminal/behavior_exclusions.rs:52-57 | CLOSED | Empty resolved exclusion set early-`Ok` only after Build-time `collect_behavior_exclusions` already admitted exclusions in Psi. |
| A0786 | omega-rust/omega/compilation/compiler/src/checked/checking/source_checking.rs:202-251 | CLOSED | Reusable scopes use `entry_declarations: None` by design; layout work still runs via `evaluate_fixed_layout_applications` and `source_checking_applications`. |
| A0787 | omega-rust/omega/compilation/backend/target/src/target_profile.rs:250-259,430-430 | CLOSED | No address model passes `admit_address_representation`, but native lowering requires `layout_target_facts()` and fails when absent. |
| A0788 | omega-rust/omega/build-system/build-settings/src/host/bindings.rs:272-292 | CLOSED | `Ok(true)` for unregistered queries only after `poll_effect_readiness`; non-`AwaitProvider` calls are already treated as synchronously ready. |
| A0789 | omega-rust/omega/build-system/build-settings/src/admission/checking.rs:118,143-147,161 | CLOSED | Missing execution profile defaults to host for defaults/folds; provider plan selection still errors when a concrete profile is required. |
| A07810 | omega-rust/omega/build-system/build-settings/src/admission/selected_constants.rs:79-117 | UNDECIDED | Unplanned operator uses are skipped before validation; did not prove every such use is caught by `CheckRequest` or pending evaluation. |
| A0791 | omega-rust/omega/build-system/orchestration/src/project/build_directory.rs:217-218,254,271-273 | REACHABLE | Existing `.omega-owner` with no `root ` line is accepted (`None` → `Ok(())`). |

### A0784

1. Read `admit_permissions` in full at `product_execution.rs:463-491` (read): when `permissions` is `None` and the `runtime_product && permissions.is_none() && package_identity && assumed_bindings` conjunction is false, `evaluated_rows` stays `None`, `permissions` stays `None`, and the function returns `Ok(())` without calling `validate`.
2. Search `rg 'admit_permissions\(' omega-rust` → **4 hits**, all in `product_execution.rs:249`, `:316`, `:379`, `:463` (read).
3. Read callers (read): `:249` and `:316` pass `runtime_product=false`; `:379` passes `true` for executable terminal finish.
4. Input reaching the accepting branch: Check or non-runtime Terminal report completion with `permissions: None`, including when `checked.execution().plans().assumed_bindings()` is non-empty and `package_identity()` is `Some` (e.g. `compilation_session.rs:137` sets `permissions: None`, read).
5. **Verdict: REACHABLE** — A Check/Terminal compile on a packaged root with assumed bindings but no `TerminalPermissionAcceptance` should still be refused if checked policy rows disagree with required consent; here it is accepted because validation is skipped.
6. Unsure: whether CLI always attaches lock permissions before Check in production flows (did not read all orchestration compile paths).

### A0785

1. Read `TerminalBehaviorExclusions::admit` in full at `behavior_exclusions.rs:41-85` (read): `authored_behavior_exclusion_set_in` at `:52`; if `exclusions.is_empty()`, return `Ok(())` at `:53-57` without `establish_behavior_exclusions`.
2. Read `authored_behavior_exclusion_set_in` at `effects/.../executable_scopes.rs:811-819` (read): filters through `resolve_authored_behavior_exclusion`.
3. Search callers of `TerminalBehaviorExclusions::admit`: `terminal_artifact.rs:440` only in production attach path (read); policy is `None` for non-Application products at `:247-255` (read).
4. Input: authored exclusions present in Build config, but none resolve to module members at Terminal (empty set).
5. Read upward (read): Build admission runs `collect_behavior_exclusions` (`psi-compiler/source_compilation.rs:464-470`, `build-time-evaluation/product.rs:578-633`) and errors on invalid service exclusion declarations before Terminal.
6. **Verdict: CLOSED** — check at `product.rs:585-633` (service/enum resolution with diagnostics); Terminal empty-set `Ok` is harmless for exclusions already rejected or fully resolved at Build; executable path only runs policy when `TerminalProductUse::Application` (`terminal_artifact.rs:247-266`, read).

### A0786

1. Read `layout_target_scopes` in full at `source_checking.rs:170-255` (read): `reusable = profiles.is_empty()` at `:202-208`; when reusable, all layout-capable profiles are added; `entry_declarations: (!reusable).then_some(entries)` at `:251`.
2. Search `rg 'layout_target_scopes' omega-rust` → hits only `source_checking.rs:34` caller (read).
3. Read consumer (read): `psi-compiler/source_checking.rs:733-738` calls `evaluate_fixed_layout_applications` with full `layout_scopes`; `entry_declarations: None` is encoded in digest at `:144-157` without treating `None` as “skip all layout”.
4. Read `layouts.rs:6095-6160` and `product.rs:1674-1754` (read): layout checking uses `scope.facts` and `source_checking_applications` declaration discovery, not only explicit entry roots.
5. Input: Product with no Build root bindings / empty initial profile list → reusable scopes with `entry_declarations: None` for every layout profile.
6. **Verdict: CLOSED** — `None` means “no Build-pinned entry roots for this scope,” not “no layout check”; `evaluate_fixed_layout_applications` still runs when fixed-layout applications exist (`layouts.rs:6100-6101`, read).

### A0787

1. Read `TargetProfile::admit_address_representation` at `target_profile.rs:250-259` (read): returns `Ok(())` when `address_model()` is `None` (CrossPlatformCli, LocalUnchecked, AlphaBootstrap at `:270-271`, read).
2. Read `native_target` at `:422-434` (read): CrossPlatformCli and LocalUnchecked map to `NativeTarget::host()`.
3. Read `from_omega_target_name` at `:320-322` (read): `None` target name → `Ok(Self::host())`.
4. Search `rg 'admit_address_representation' omega-rust` → **2 hits**: definition and `artifact_admission.rs:177` (read).
5. Read caller `artifact_admission.rs:176-191` (read): after `admit_address_representation`, `layout_target_facts().ok_or_else(...)` errors for profiles without facts.
6. Input: `cross_platform_cli` / `local_unchecked` or omitted target name on paths that only call `admit_address_representation` without later layout admission.
7. **Verdict: CLOSED** — permissive address admission is gated for native artifact preparation by `layout_target_facts()` at `artifact_admission.rs:187-190` (read); did not read every `native_target()` caller for CrossPlatformCli beyond this trace.

### A0788

1. Read `poll_host_computation` in full at `bindings.rs:272-292` (read): non-`BoundaryCall` → error; no matching registered query → `Ok(true)` at `:291`.
2. Search `rg 'poll_host_computation' omega-rust` → **7 hits** (1 production: `bindings.rs`; others tests/interpreter trait/default, read).
3. Read caller chain (read): `engine_calls.rs:3475-3500` calls `poll_effect_readiness` first, then `poll_host_computation`; registered queries use `poll_query` at `bindings.rs:290`.
4. Read `poll_effect_readiness` at `bindings.rs:295-353` (read): registered queries return `Ok(true)` at `:316`; non-`AwaitProvider` calls return `Ok(true)` at `:325-326`.
5. Input: Build binding `BoundaryCall` that is not in `host.queries`, after readiness already returned true.
6. **Verdict: CLOSED** — check at `bindings.rs:325-326` (non-awaiting calls deemed ready); unregistered effects treated as not needing host-computation polling; registered queries go through `poll_query` at `:290`.

### A0789

1. Read `prepare_checked_build` at `checking.rs:43-167` (read): `execution_profile.unwrap_or_else(TargetProfile::host)` at `:118` and `:161`; `execution_profile.ok_or_else(...)` error at `:143-147` when provider planning requires a profile.
2. Search callers: `build_evaluation.rs:280` passes `execution_profile` from `BuildExecutionInputs.profile` (read `build_evaluation.rs:226-228`).
3. Input: `execution_profile: None` during Build semantic admission.
4. **Verdict: CLOSED** — host default at `:118`/`:161` is intentional for defaults/bindings; fail-closed error at `:143-147` when derived provider candidates require an admitted profile; did not read every activation path that sets `execution_profile`.

### A0790

1. Read `SelectedConstantEvaluation::prepare` in full at `selected_constants.rs:61-118` (read): `continue` at `:83` when no selected plan; only provider-body or primitive-float paths recorded at `:85-113`; `validate_selected_operators` at `:115-116` on recorded operators only.
2. Read caller `checking.rs:159-166` (read): runs before `CheckRequest` at `:179`.
3. Input: resolved operator use in `facts.resolved_expression_selections()` with no matching selected requirement plan, or plan that is neither provider body nor primitive float operator.
4. **Verdict: UNDECIDED** — accepting branch is real (read `:79-84`), but I did not read `SelectedConstantEvaluationRequest::evaluate` or all `PreCheckEvaluation` deferral rules to prove an unrecorded use with a pending fold always fails later at `CheckRequest` (`checking.rs:179`, unread in depth).

### A0791

1. Read `directory_identity`, `check_owner`, `canonicalize_or_literal` at `build_directory.rs:210-273` (read).
2. `directory_object` at `:216-217` (read): `filesystem_object_identity(path).ok()` → `None` on error.
3. `check_owner` at `:253-254` (read): `recorded` is `None` when owner file exists but has no `root ` line → `Ok(())`.
4. `canonicalize_or_literal` at `:271-272` (read): failed canonicalize keeps literal spelling.
5. Input: pre-existing `.omega-owner` without a `root ` line; paths where object identity is unavailable (`object: None` for both sides).
6. **Verdict: REACHABLE** — two different source roots can share a build directory if the owner marker is present but lacks a parseable `root ` line (should be `AliasedRoot` but is accepted).
7. Unsure: frequency of `filesystem_object_identity` failure in supported host matrix (read-only, not run).

## What I did not read

- Full orchestration compile path that sets `TerminalPermissionAcceptance` vs `None` for Check.
- `SelectedConstantEvaluationRequest::evaluate` implementation body.
- All production callers of `TargetProfile::native_target()` for CrossPlatformCli beyond `artifact_admission.rs`.
- `BuildExecutionInputs.profile` population in every activation variant.
- Deeper `effects` / `establish_behavior_exclusions` internals beyond cited sites.

SLICE COMPLETE

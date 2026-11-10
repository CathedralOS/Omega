# slice_0102

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0784 | omega-rust/omega/compilation/compiler/src/product_execution.rs:473-491 | REACHABLE | Check and Checking stops call `admit_permissions` with `runtime_product=false`, so `permissions: None` skips all `validate`. |
| A0785 | omega-rust/omega/compilation/compiler/src/terminal/behavior_exclusions.rs:53-58 | REACHABLE | Authored exclusions that resolve to an empty module set return `Ok` without product exclusion verification. |
| A0786 | omega-rust/omega/compilation/compiler/src/checked/checking/source_checking.rs:251 | REACHABLE | Reusable layout profiles set `entry_declarations: None`, widening fixed-layout reachability roots. |
| A0787 | omega-rust/omega/compilation/backend/target/src/target_profile.rs:250 | REACHABLE | `CrossPlatformCli` and `LocalUnchecked` pass `admit_address_representation` and realize as build host. |
| A0788 | omega-rust/omega/build-system/build-settings/src/host/bindings.rs:290-291 | REACHABLE | Unregistered query boundary effects report host computation finished via `Ok(true)`. |
| A0789 | omega-rust/omega/build-system/build-settings/src/admission/checking.rs:118 | REACHABLE | Missing `execution_profile` substitutes `TargetProfile::host` for defaults and pending constants. |
| A0790 | omega-rust/omega/build-system/build-settings/src/admission/selected_constants.rs:80-84 | REACHABLE | Operator uses without a selected plan or non-recorded plan kinds skip validation. |
| A0791 | omega-rust/omega/build-system/orchestration/src/project/build_directory.rs:254 | REACHABLE | Owner file without a `root ` line admits occupancy; failed canonicalize or inode identity weakens equality. |

## A0784

Digest (read): area_01.md:158 — `comp/product_execution.rs:473-491` skips permission validation when `permissions` is `None` and the runtime-product conjunction is false; callers `:249`, `:316` pass `runtime_product=false`.

Opened `admit_permissions` in full at `product_execution.rs:463-492` (read). When `permissions` is `None` and the `if` at `:473-477` is false (needs `runtime_product && permissions.is_none() && package_identity && !assumed_bindings.empty()`), `evaluated_rows` stays `None`, `:485` leaves `permissions` as `None`, and the `if let Some(permissions)` at `:486-490` is skipped — function returns `Ok(())` at `:491`.

Caller search: `rg '\badmit_permissions\(' omega-rust` → 4 matches (read): definition `:463`, calls `:249`, `:316`, `:379`.

Traced callers (read): `CheckingCompilation::finish` at `:249` passes `false` for `runtime_product`; `ProductRequest::plan` non-terminal branch at `:316` passes `false`; `RequestedTerminalCompilation::finish` at `:379` passes `true` (only path that can synthesize permissions via `from_public_build` at `:478-480`).

Input in plain words: A Check or Checking (terminal-stop) product compilation with assumed provider bindings, a package identity, and no supplied `TerminalPermissionAcceptance` — permission rows are never validated before trust admission and report preparation.

Verdict **REACHABLE**: such a program should be rejected for missing or mismatched terminal authority permissions on the stop path but is accepted because callers pass `runtime_product=false`.

## A0785

Digest (read): area_01.md:159 — authored exclusions resolving to an empty set against the module → `Ok`; cites unit_012 wording that an unresolving exclusion imposes no check; `effects` not traced in digest.

Opened `TerminalBehaviorExclusions::admit` at `behavior_exclusions.rs:41-85` (read). After first root at `:51-52`, `exclusions = authored_behavior_exclusion_set_in(self.authored, module)` at `:52`. Empty set at `:53-57` drains remaining roots with `root?` only and returns `Ok(())` without calling `establish_behavior_exclusions` (contrast `:68-82`).

Opened `authored_behavior_exclusion_set_in` at `effects/src/provider_evidence/executable_scopes.rs:811-820` (read): `filter_map(|row| resolve_authored_behavior_exclusion(row, module))` — unresolved rows drop out.

Caller search: `rg '\bTerminalBehaviorExclusions\b' omega-rust/omega/compilation/compiler` → 4 matches (read): struct/impl in `behavior_exclusions.rs`, construction at `terminal_artifact.rs:261-266`, use in terminal production admission (not opened in full).

Spec (read): `wiki/spec/build/behavior_exclusions.md:51-53` — “Service names must resolve to exact authorized declarations… Unrecognized… reject.” Publication-time check is described at `:141-186` as mandatory product admission.

Input in plain words: A Terminal product whose Build configuration still lists behavior exclusions, but every exclusion fails to resolve against the published Terminal module (stale identity, wrong artifact, or renamed service), so the resolved set is empty.

Verdict **REACHABLE**: exclusions that should fail product admission are treated as imposing no check and roots pass when only resolution failure distinguishes them from an intentional empty policy.

## A0786

Digest (read): area_01.md cites `source_checking.rs:202-207`, `:251` — reusable scopes carry `entry_declarations: None`; layout checker handling of `None` not traced.

Opened `layout_target_scopes` at `source_checking.rs:170-255` (read). When `build.root_bindings` and machine targets yield no profiles, `reusable = profiles.is_empty()` at `:202`; `:204-208` extends all `TargetProfile::ALL` with `layout_target_facts()`. Each scope gets `entry_declarations: (!reusable).then_some(entries)` at `:251` — reusable mode forces `None` for every scope.

Downstream (read): `build-time-evaluation/src/product.rs:1820-1852` — `CheckingScope::prepare` clones `scope.entry_declarations`; when `None`, `work` calls `applications.reachability(..., self.roots.is_none(), ...)` at `:1848-1852`, passing `true` for the “unrestricted roots” flag.

Admission path (read): `psi-compiler/src/source_checking.rs:733-738` runs `evaluate_fixed_layout_applications` over `layout_scopes` before lowering; digest encoding at `:144-157` records `entry_declarations: None` as an absent roster byte.

Input in plain words: A package Build with no target-specific root bindings (reusable layout mode) still runs fixed-layout evaluation across all layout-capable targets without pinning Build root entry symbols per target.

Verdict **REACHABLE**: layout checking uses unrestricted reachability instead of the Build-bound entry set the non-reusable branch would supply.

## A0787

Digest (read): area_01.md — `tgt/target_profile.rs:430`, `:250`; CrossPlatformCli/LocalUnchecked realize as build host; `admit_address_representation` passes without address model; callers not traced in digest.

Opened `admit_address_representation` at `target_profile.rs:250-259` (read): errors only when `address_model()` is `Some` and differs from supported representation; otherwise `Ok(())`.

Opened `address_model` at `:262-271` (read): `CrossPlatformCli | LocalUnchecked | AlphaBootstrap => None`.

Opened `native_target` at `:422-434` (read): `CrossPlatformCli | LocalUnchecked => NativeTarget::host()`.

Caller search: `rg '\badmit_address_representation\(' omega-rust` → 2 matches (read): definition and `pipeline/00_terminal-psi-to-abstract-operations/src/artifact_admission.rs:177` inside `NativeScopePreparation::prepare`. That function then requires `layout_target_facts()` at `:187-191`, which returns `None` when `address_model()` is `None` (`:409-410`) and errors — native artifact path rejects.

Separate uses of `native_target()` without `admit_address_representation` are widespread (grep count > 35 in `omega-rust/omega`; read samples in `provider-planning`, `terminal-runtime`).

Input in plain words: A build or check pipeline selects `cross_platform_cli` or `local_unchecked` — address-representation admission is vacuous, but native planning and provider layout still use the compile host’s `NativeTarget::host()`.

Verdict **REACHABLE**: target profiles without an address model pass the only dedicated address-representation gate and still participate in host-shaped native planning; native artifact preparation adds a separate `layout_target_facts` refusal not traced through every caller of `native_target()`.

## A0788

Digest (read): area_01.md — `bs/host/bindings.rs:290-291`; `poll_host_computation` returns `Ok(true)` when the effect is not a registered query; reachability inferred only for classified host computations.

Opened `poll_host_computation` at `bindings.rs:272-293` (read): for a `BoundaryCall`, if no `host.queries` row matches `operation` and `boundary`, `:291` returns `Ok(true)` (computation treated as complete).

Interpreter use (read): `terminal-interpreter/src/engine_calls.rs:3488-3500` — after `poll_effect_readiness` reports ready, `poll_host_computation` returning `true` means no `HostComputationPending`.

Related (read): `poll_effect_readiness` at `bindings.rs:295-353` returns `Ok(true)` immediately when a query row exists (`:310-316`); otherwise enforces provider-installation state for registered calls.

`handle_effect_result` at `:361-390` still requires a matching `host.calls` row (`:389-390`).

Input in plain words: A Build activation boundary call that is listed in `host.calls` but not in `host.queries`, once readiness polling returns ready, is treated as having finished its host computation without executing a registered query.

Verdict **REACHABLE**: `Ok(true)` on the non-query branch allows the interpreter to proceed as if host work completed when no query was ever registered for that operation/boundary pair.

## A0789

Digest (read): area_01.md — `bs/admission/checking.rs:118`, `:161`; missing execution profile replaced by host profile.

Opened `prepare_checked_build` at `checking.rs:43-167` (read). Parameter `execution_profile: Option<TargetProfile>` at `:49`. `settle_provider_defaults` at `:112-118` uses `execution_profile.unwrap_or_else(TargetProfile::host)`. Pending constant fold at `:159-161` uses the same fallback. When `candidates` is non-empty, `:143-147` requires `execution_profile` be `Some` or errors — partial strictness.

Caller (read): `build_evaluation.rs:280-290` passes through `execution_profile` unchanged into `prepare_checked_build`.

Input in plain words: Build semantic admission invoked with `execution_profile: None` (profile not fixed before check) still settles provider defaults and evaluates pending build-time constants against the compile host target.

Verdict **REACHABLE**: missing profile should be an explicit admission failure for host-sensitive planning but is silently replaced by `TargetProfile::host()` at `:118` and `:161`.

## A0790

Digest (read): area_01.md — `selected_constants.rs:80-84`, `:96-113`; operator use without selected plan skipped; other plan kinds not recorded; validation runs on recorded set only.

Opened `SelectedConstantEvaluation::prepare` at `selected_constants.rs:61-119` (read). Loop at `:79-114`: `selected_requirement_plan` `None` → `continue` at `:82-83`. When `prepare_selected_build_time_provider_body` returns `None` and `derive_selected_primitive_float_binary_execution` is `None`, nothing is pushed. Validation at `:115-117` runs only on `selected.operators` and `selected.provider_bodies`.

Caller (read): `checking.rs:158-166` invokes `prepare(...).evaluate_pending(...)` when `had_pending`.

Input in plain words: A pending Build pre-check operator requirement whose provider plan is missing, or whose selected plan is not a build-time provider body nor a primitive float binary operator, is omitted from the batch that `validate_selected_operators` and `validate_selected_provider_bodies` inspect.

Verdict **REACHABLE**: deferred constant evaluation can proceed without validating skipped operator requirements.

## A0791

Digest (read): area_01.md — `orch/project/build_directory.rs:254`, `:271-273`, `:217`; owner file with no `root ` line accepted; canonicalize failure falls back; identity lookup error becomes `None`.

Opened `check_owner` at `build_directory.rs:231-261` (read). Existing owner file: `recorded` is `None` when no line has prefix `root ` (`:234-237`, `:253-254`) → match arm `None => Ok(())` admits without comparing roots.

Opened `canonicalize_or_literal` at `:271-273` (read): failed `fs::canonicalize` uses the literal path.

Opened `directory_object` at `:216-218` (read): `platform_custody::filesystem_object_identity(path).ok()` drops errors to `None`; `directory_identity` at `:210-213` then compares canonical paths with optional inode pair.

Caller (read): `acquire` at `:167-173` calls `check_owner` then creates occupant file.

Input in plain words: A build directory whose `.omega-owner` (owner file) exists but contains only magic lines without `root <path>`, or two different directory paths that fail canonicalize the same way, or paths whose object identity cannot be read — occupancy is still admitted and conflict detection may compare only spelling.

Verdict **REACHABLE**: missing `root` line does not refuse admission; weak path equality can treat distinct directories as compatible when canonicalize or inode identity is unavailable.

## What I did not read

- Full terminal artifact admission chain calling `TerminalBehaviorExclusions::admit` beyond construction at `terminal_artifact.rs:261-266`.
- `effects::establish_behavior_exclusions` and full `resolve_authored_behavior_exclusion` arms in `executable_scopes.rs` above `:811`.
- Every production caller of `TargetProfile::native_target()` outside the `admit_address_representation` grep sample.
- `platform_custody::filesystem_object_identity` implementation.
- `package_evidence::permission_acceptance::TerminalPermissionAcceptance::validate` rules (only the skip path in `admit_permissions` was traced).

SLICE COMPLETE

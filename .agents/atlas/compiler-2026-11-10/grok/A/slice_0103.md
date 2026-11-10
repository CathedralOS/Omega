# slice_0103

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0792 | orch/package/package_review.rs:205-207 | REACHABLE | Package review can finish without a description for a required component when `round.component` is absent; `with_independent_component_descriptions` does not require completeness. |
| A0793 | ba/lib.rs:201-203 | CLOSED | Float comparison commitment is filled from the same plan index binding as integer at settlement time; no other production gate reads float `provider_plan_commitment`. |
| A0794 | cd/component_description.rs:334-336 | UNDECIDED | `unwrap_or_default()` yields `""` for bad indices in derive/verify paths; no explicit emptiness rejection on those derived identity strings was found. |
| A0795 | nc/realization/callback_thunks.rs:506-507 | REACHABLE | `(None, None)` returns `Ok(None)` and `lower_callback_thunk` omits installed entry execution, emitting an ordinary private callback body. |
| A0796 | whr/windows_handle_origins.rs:87 | CLOSED | `ExternalInput` and `OpaqueObservation` edges are classified as `ArgumentValues::Unknown` in argument propagation. |
| A0797 | pa/provider_installation.rs:219-229 | REACHABLE | Pre-filter drops unselected catalog candidates so `select_provider_candidates` never errors for them; Build `admit` does not reject nonempty `missing_providers`. |
| A0798 | pa/provider_installation/replay.rs:66-68 | CLOSED | Skipped replays omit calls from `installed_calls`; `access_lifecycles` requires an exact row via `selected_profile_call` and errors if absent. |
| A0799 | pp/intrinsic_declarations.rs:314-319 | REACHABLE | Hosted settle treats `binding.declaration_identity == None` as matching any prepared `declaration_identity` when `subject.matches` succeeds. |

## A0792

**Verdict:** REACHABLE

**Cited site (re-located at current checkout):** `omega-rust/omega/build-system/orchestration/src/package/package_review.rs:205-207` (digest cited `orch/package/package_review.rs:205-207`).

**Trace (read):**

1. Opened `component_descriptions` (`package_review.rs:185-217`). For each `build.required_component_packages()` entry, if no description is already present and `round.component(...)?` returns `None`, the loop `continue`s without pushing a description (`:205-207`).
2. Read `PackageBuild::with_independent_component_descriptions` in `omega-rust/omega/packages/package-compilation/src/package_compilation.rs:443-478`. It only rejects root/foreign/duplicate descriptions; it does **not** require every required component package to appear in the vector.
3. Read `review_package_build` (`omega-rust/omega/compilation/compiler/src/checked/package_review.rs:75-112`). It runs `check_package_build` and evidence projection; no step compares `required_component_packages()` to attached descriptions.
4. **Closing path for full product compile (read):** `required_components` in `omega-rust/omega/build-system/orchestration/src/compile/build_execution.rs:404-428` errors with `"independent component has no prepared Build for this purpose"` when a required component package lacks a matching `OccurrenceProduct`. That path does not use `with_independent_component_descriptions` completeness either, but blocks compile when the component was never built.

**Input that reaches the branch:** A package review round where the build lists a required independent component package, the inputs have no pre-attached description for it, and `round.component` yields no candidate—the loop skips it and review proceeds with a shortened description list.

**Plain-words REACHABLE case:** A project whose Build declares a required independent component, but review cannot obtain a component description for that package in this round, still passes `review_package_build` because neither the skip nor `with_independent_component_descriptions` rejects the missing entry.

**Caller search:** `with_independent_component_descriptions` — ripgrep `\bwith_independent_component_descriptions\b` under `omega-rust`: 6 listing sites (4 production: `package_compilation.rs:375`, `443`; `package_build.rs:185`; `package_compilation.rs` compiler wrapper `:229`; `build_execution.rs:269`; `package_review.rs:169`; plus `product_boundary.rs:255` string guard).

## A0793

**Verdict:** CLOSED

**Cited site:** `omega-rust/omega/compilation/backend/artifacts/boundary-applications/src/lib.rs:201-203` (float `validate_roster`); integer sibling check at `:304-305` (read).

**Trace (read):**

1. Read full `TerminalIeeeFloatComparisonOccurrenceProposal::validate_roster` (`lib.rs:171-251`). At `:201-203` it only checks `plans.get(occurrence.provider_plan_index)`; it does not compare `occurrence.provider_plan_commitment` to `plan.identity_digest()` (integer does at `:304-305`).
2. Read proposal construction in `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/settlement.rs:139-154, `:185-191`, `:226-230`. Plan index is resolved from `row.selected_plan_digest()` with uniqueness checks (`:144-151`); `provider_plan_commitment` is set from `plans[provider_plan_index].identity_digest()` at `:154` in the same loop iteration.
3. **Single production caller of `validate_roster` (search):** ripgrep `::validate_roster\(` under `omega-rust` → 2 files: `settlement.rs:226-232` (production) and `boundary-applications/src/lib.rs` (tests only). Count: 1 production call site.
4. **Downstream use of float proposals (search):** ripgrep `ieee_float_comparisons` under `omega-rust` → stored on `TerminalNativeOccurrences` in `retained_terminal.rs:1194-1195`; no further production reads of `provider_plan_commitment` on float proposals found (integer field is checked only inside `validate_roster`).

**Why CLOSED:** On the normal compile path, proposals are built and validated in one settlement pass; commitment is derived from the bound index. The float roster check is roster/operation shape integrity; the integer commitment check is extra hardening, not the only plan binding gate.

## A0794

**Verdict:** UNDECIDED

**Cited site:** `omega-rust/omega/compilation/backend/artifacts/component-description/src/component_description.rs:334-336`.

**Trace (read):**

1. `identity_text` returns `module.identities.text(identity).unwrap_or_default()` (`:334-336`). For a dangling `TerminalIdentityIndex`, `terminal_psi::TerminalIdentityTable::text` returns `None` (`omega-rust/psi/compilation/representations/terminal-psi/src/identity.rs:354` area).
2. Read `derive_component_inventory` uses (`component_description.rs:572-689`): pushes `identity_text(...).to_string()` into `called_requirement_identities`, custody identity strings, `service_bounds`, and `export_contracts` without an `is_empty()` guard on those texts.
3. Read `component_verification.rs` consumers of `identity_text` (`:322-353`, `check_providers` at `:775-855`): checks provider roster consistency and nonempty `provider.requirement_identities` (`:799-802`), but no rule rejecting empty requirement identity text derived from a bad module index.

**UNDECIDED because:** I did not trace whether a **compiler-produced** Terminal module can carry a used identity index absent from the table (vs. only corrupt hand-edited bytes). I did not read every `verify_component` path for collision/smuggling when multiple indices map to `""`.

**Search:** `identity_text` under `component-description` crate — 16 matches across `component_description.rs` and `component_verification.rs`.

## A0795

**Verdict:** REACHABLE

**Cited site:** `omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs:506-507` (digest `:507`; match arm spans `:505-506`).

**Trace (read):**

1. Read full `PreparedEntryPlans::for_body` (`callback_thunks.rs:479-525`). `(None, None) => Ok(None)` at `:505-506` when cohort has no entry for the placement and retained plan slot is empty.
2. Read sole caller `lower_callback_thunk` (`:300-320`): if `installed_plan` is `None`, it skips `with_installed_entry_execution` and continues to `prepare_installed_callbacks` / `emit_prepared_callable_object`, emitting a normal private callback object.
3. Mismatched partial states error at `:522-524`.

**Caller search:** `\bfor_body\b` under `omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs` — 1 production call at `:307`.

**Plain-words REACHABLE case:** A callback thunk settlement that should have installed-interrupt evidence for a placement, but admission produced an empty cohort entry and no retained `PreparedEntry` for that index, is lowered as an ordinary callback body without installed entry execution.

## A0796

**Verdict:** CLOSED

**Cited site:** `omega-rust/omega/compilation/realization/provider-admission/src/provider_admission/terminal_authority_review/windows_handle_origins.rs:87-89` (`ExternalInput`, `OpaqueObservation` with comment that absent edges are not empty reach).

**Trace (read):**

1. Origins built with empty `sources` for these kinds in `windows_handle_origins.rs` (e.g. `:185`, `:207`, `:229`, `:455`).
2. Read consumer `windows_handle_arguments.rs:250-254` (re-located; digest area `whr/windows_handle_arguments.rs:87` was the origins enum): `OriginKind::ExternalInput | OpaqueObservation | ... => ArgumentValues::Unknown`.

**Search:** ripgrep `ExternalInput|OpaqueObservation` under `omega-rust/omega/compilation/realization/provider-admission` — origins definitions plus classification in `windows_handle_arguments.rs:252-253`, `windows_handle_activations.rs`, etc.; argument propagation path read treats them as unknown, not as empty/definite.

## A0797

**Verdict:** REACHABLE

**Cited site:** `omega-rust/omega/compilation/realization/provider-admission/src/provider_installation.rs:219-229`.

**Trace (read):**

1. `admit_terminal_provider_installation` filters `module.provider_candidates` to those whose `requirement_identity` text matches some `SelectedProviderAdapter` row (`:219-228`), then calls `select_provider_candidates` on the filtered list only (`:230`).
2. Read `select_provider_candidates` (`:235-301`). It iterates boundaries present in the **filtered** candidate map; `MissingSelectedProvider` (`:280`) only fires for a requirement identity that still has candidates but no selected row match—not for requirements removed entirely by the pre-filter.
3. **Callers of `admit_terminal_provider_installation` (search):** ripgrep under `omega-rust` → `provider_installation.rs:214` (definition) and `omega-rust/omega/build-system/build-settings/src/admission/provider_execution.rs:64` (Build admission).
4. Read Build closure after admit (`provider_execution.rs:83-89`): `execution_requirements` is called; result is **not** checked for `missing_providers` (contrast `terminal-runtime/src/execution.rs:314-321`, which refuses nonempty `missing_providers`).

**Plain-words REACHABLE case:** Build-time terminal admission with a callable boundary that still has catalog candidates for an unselected requirement identity: those candidates are dropped before selection, admission succeeds, and Build does not fail on uncovered provider slots reported in `execution_requirements.missing_providers`.

**Caller search:** `select_provider_candidates` — only `provider_installation.rs:196` and `:230` (internal).

## A0798

**Verdict:** CLOSED

**Cited sites:** `pa/provider_installation/replay.rs:66-68`, `:151-159` (read); digest also cited `:33-52`, `:151-160`.

**Trace (read):**

1. `replay_installed_provider_calls` — if no installed provider for a boundary, `ExternalCompletion` and `BoundaryCall` arms `continue` without recording a call (`:66-67`, `:151-159`). Duplicate boundary keys in `installed_by_boundary` are first-wins (`:33-35`).
2. **Single caller (search):** ripgrep `replay_installed_provider_calls` → only `provider_installation.rs:200`.
3. Read consumer `provider_admission/access_lifecycles.rs:421-427` → `selected_profile_call(installation.installed_calls(), ...)`; helper at `:476-491` returns `Err("profile was not produced by an installed selected-provider call")` when no matching call exists (`:585-586` test asserts empty slice errors).

**Input:** Callable boundary use omitted from replay because `installed_by_boundary` lacked that boundary; any profile/access lifecycle proof that needs the corresponding installed call fails closed.

## A0799

**Verdict:** REACHABLE

**Cited site:** `omega-rust/omega/compilation/realization/provider-planning/src/intrinsic_declarations.rs:314-319`.

**Trace (read):**

1. Read full `IntrinsicDeclaration::settle` hosted branch (`:297-330`). When `binding.declaration_identity` is `None`, the inner match uses `None => true` at `:318-319`, skipping comparison to `declaration_identity` from `prepare`.
2. **Production callers of `settle` (search):** ripgrep `\.settle\(` under `omega-rust/omega/compilation/realization` → `selected-dispatch/src/compiler_intrinsic.rs:135` and `intrinsic_review.rs:389`. Read `intrinsic_review.rs:220-223`: review builds `AcceptedRowBinding { declaration_identity: None, ... }` for unresolved accepted paths.
3. Read `compiler_intrinsic.rs:47`: normal dispatch can pass `declaration_identity: Some(binding.declaration().nominal_identity.as_str())`.

**Plain-words REACHABLE case:** A hosted intrinsic row is settled as consumed when the accepted binding carries no declaration identity, even if the prepared hosted declaration identity would not equal a specific expected string, as long as `subject.matches` and shape/bundled/consumed flags succeed.

## What I did not read

- Full `terminal_psi::engine::capabilities::check` reachability beyond the `missing_providers` collection logic (`engine_execution.rs:244-667` partially read).
- Entire Windows handle origin graph beyond `windows_handle_arguments.rs` classification for `ExternalInput` / `OpaqueObservation`.
- Whether tampered or non-settlement-produced `TerminalNativeOccurrences` can enter native compile without passing `settlement.rs` validate_roster.
- Complete `verify_component` surface for empty-string requirement collisions (A0794).
- Static interrupt cohort admission that might prevent `(None, None)` on true installed-handler thunks (upstream of `callback_thunks.rs`).

SLICE COMPLETE

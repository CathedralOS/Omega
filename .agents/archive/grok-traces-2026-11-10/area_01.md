# Area 01 — accepts when it does not know

Read-only trace of every entry classed `open` in `.agents/atlas/compiler-2026-11-10/digests/area_01.md`. Citations are `file:line` at `457adf4a11f`. `git diff --stat 457adf4a11f` on `omega-rust/` was empty, so the line numbers still match. Each claim is from lines I read unless it says inferred.

Paths below are under `omega-rust/`. `comp/` is `omega/compilation/compiler/src/`. `bs/` is `omega/build-system/build-settings/src/`. `tgt/` is `omega/compilation/backend/target/src/`.

| # | file:line | verdict | reason |
| --- | --- | --- | --- |
| 1 | comp/product_execution.rs:473-491 | REACHABLE | A native or terminal product with assumed bindings, no package identity, and no permission acceptance skips the package requirement. |
| 2 | comp/terminal/behavior_exclusions.rs:53-58 | UNDECIDED | An authored service exclusion that matches no published service returns success. Whether a present service can miss that text compare was not settled. |
| 3 | comp/checked/checking/source_checking.rs:202-207, :251 | UNDECIDED | Reusable layout scopes carry no entry list. Whether a machine outside the product-selected seed can hide a layout error was not settled. |
| 4 | tgt/target_profile.rs:250, :430 | CLOSED | The only caller of the address-model pass then rejects a profile with no layout facts. The host arm is an explicit realization of two named profiles. |
| 5 | bs/host/bindings.rs:290-291 | CLOSED | `Ok(true)` only means the effect is not a pending query. The commit that follows rejects a call that is not in the host's call list. |
| 6 | bs/admission/checking.rs:118, :161 | CLOSED | A missing profile is filled with the host before this function, or `TargetProfile::host` panics. The substitution does not accept a program. |
| 7 | bs/admission/selected_constants.rs:80-84, :96-113 | UNDECIDED | A use with no plan, or a plan that is neither a provider body nor a float operator, is not recorded. Whether that use is later rejected was not settled. |
| 8 | orch/project/build_directory.rs:254, :271-273, :217 | REACHABLE | An owner file with no `root ` line is treated as unowned, so another project root is admitted into that build directory. |
| 9 | orch/package/package_review.rs:205-207 | CLOSED | `Ok(None)` means the required component is the package being reviewed. A missing candidate is an error. |
| 10 | ba/lib.rs:201-203 | CLOSED | The only producer copies the plan digest into the occurrence immediately before this check. A mismatched digest is not supplied. |

`orch/` is `omega/build-system/orchestration/src/`. `ba/` is `omega/compilation/backend/artifacts/boundary-applications/src/`.

Entries 11 onward are not written yet.

## 1. `comp/product_execution.rs:473-491`

`admit_permissions` (`product_execution.rs:463-492`, read in full). Search `admit_permissions\(`: 4 hits, all in this file (definition and three calls).

The function builds `RuntimePermissionInput`, then calls `TerminalPermissionAcceptance::from_public_build` only when all of these hold: `runtime_product`, `permissions` is `None`, `package_identity()` is `Some`, and `assumed_bindings()` is nonempty (`:473-477`). Otherwise that value stays `None`. `validate` runs only when a permission value is present (`:485-489`). The function then returns `Ok(())`.

`from_public_build` (`omega/packages/review-evidence/src/permission_acceptance.rs:51-66`, read) rejects a missing package with "assumed binding risk rows require the consuming root package", and rejects a count mismatch with the public Build. That function is not called when the package identity is absent, because of the `is_some()` guard.

Callers, all read:

- `CheckingCompilation::finish` (`product_execution.rs:249`) passes `runtime_product = false`. This arm is the check path that still has proof obligations (`:309-313`).
- `ProductRequest::plan` (`:316`) passes `false` when `needs_terminal` is false and there are no proof obligations (`:300-316`). `needs_terminal` is false for a check, and also when the admission is artifact-only (`omega/compilation/products/src/checked_compilation/report.rs:84-86`, read).
- `RequestedTerminalCompilation::finish` (`:379`) passes `true`. That is the path that produces a terminal or native application.

Permission values arrive as `None` from `complete_source_product` (`comp/checked/compilation_session.rs:137`, read) and from `select_target_product` (`comp/target_batch.rs:164`, read). `admit_accepted_contexts` (`omega/packages/package-manager/src/operations/accepted_compilation.rs:387-401`, read) loads lock permissions only for `Native` or `Terminal`, and only when a lock exists. A check never supplies them.

The check-only skip is the caller's choice: a check is not given lock rows. The hole is the runtime path. A native or terminal product whose source has no package identity, whose plans carry assumed bindings, and which has no supplied permission acceptance fails the conjunction at `:475`, so neither `from_public_build` nor `validate` runs.

Digest: right about the conjunction and about `:249` and `:316` passing `false`. It does not mention that `:379` still skips when the package identity is missing.

Read in full: `admit_permissions`, `from_public_build`, `validate`, `ProductRequest::plan`, `CheckingCompilation::finish`, `admit_accepted_contexts`. Read `RequestedTerminalCompilation::finish` through the permission call and the following product match (`:372-453`). Not read: the producer that fills `assumed_bindings` on a non-package source (`product_selections.rs:82` was seen as an assignment only).

## 2. `comp/terminal/behavior_exclusions.rs:53-58`

`TerminalBehaviorExclusions::admit` (`behavior_exclusions.rs:40-86`, read in full). `new` (`:22-37`, read) returns `None` when the authored list is empty, so `admit` is only called for a nonempty list. The caller is `produce_retained_terminal_artifacts` (`comp/terminal/terminal_artifact.rs:261-266` and `:439-443`, read). Search `TerminalBehaviorExclusions::`: 1 hit, the `new` call. Search `authored_behavior_exclusion_set_in\(`: 4 hits. One is a source-text guard in `product_boundary.rs`. The function is `executable_scopes.rs:811-819` (read). The other call is `retained_terminal.rs:1201` (read the surrounding proposal build `:1176-1204`).

`authored_behavior_exclusion_set_in` keeps only rows that `resolve_authored_behavior_exclusion` (`executable_scopes.rs:792-808`, read) turns into a `BehaviorExclusion`. Crash-cause and physical-authority rows always resolve. A service row resolves only when some published service's identity text equals the authored identity. If every authored row is a service that misses, the set is empty and `admit` drains root errors and returns `Ok(())` (`:53-57`) without calling `establish_behavior_exclusions`.

Service rows are built from `normalized_hermetic_symbol_identity` (`psi/compilation/build-time-evaluation/src/product.rs:691-719`, read). I did not read the producer of `module.services[].identity`, so I cannot say whether a service that is present in the module can fail that text compare.

Verdict UNDECIDED. What would decide it: whether a published service's identity text is always the hermetic symbol identity stored on the authored exclusion. If it can differ, a Build that excludes that service is accepted with the exclusion dropped. If it cannot, the empty set means the excluded service is absent, and the skip does not admit a present service.

Digest: right that an unresolved authored exclusion imposes no check on this path. Incomplete: crash and authority exclusions cannot take the empty path.

Not read: `establish_behavior_exclusions`, and `validate_physical_authority_closure` past the call at `native_execution_scope.rs:752-754`.

## 3. `comp/checked/checking/source_checking.rs:202-207, :251`

`layout_target_scopes` (`source_checking.rs:170-256`, read in full). Search `layout_target_scopes\(`: 2 hits, the call at `:34` and the definition.

When no root binding and no provider choice names a target, `reusable` is true (`:202`) and the scopes are every profile whose `layout_target_facts()` is `Some` (`:203-208`). That is the six profiles with an address model (`tgt/target_profile.rs:262-271` and `:409-410`, read). Each of those scopes sets `entry_declarations: None` (`:251`). `CrossPlatformCli`, `LocalUnchecked`, and `AlphaBootstrap` are not in this list.

The consumer is `CheckingScope::work` (`psi/compilation/build-time-evaluation/src/product.rs:1836-1918`, read). It passes `roots.is_none()` as `selected_product` into `ProofApplications::reachability` (`psi/foundation/semantic-vocabulary/src/proof_applications.rs:386-414`, read). With `roots == None` and `selected_product == true`, the walk starts from product-selected machines whose authored target matches, and from product-selected machines with no authored target (`:404-406`). It does not start from a machine that is not product-selected.

I did not read the code that turns that declaration selection into a layout diagnostic. A machine that is not product-selected and is not added later by the unscoped pass (`product.rs:1698-1718`, read) might never be layout-checked.

Verdict UNDECIDED. What would decide it: whether layout errors are reported only for symbols in the `DeclarationSelection` returned by `work`. The `:515` `entry_declarations: None` in `psi/compilation/psi-compiler/src/source_checking.rs` is a test helper (`:504-518`, read), not this production path. The digest's second cite is `source_checking.rs:251` in the compiler crate.

Not read: the body of `walk_applications` after `:479`, and the layout-error reporter.

## 4. `tgt/target_profile.rs:250` and `:430`

Two sites in one entry.

`admit_address_representation` (`target_profile.rs:250-260`, read). It returns `Ok(())` when `address_model()` is `None` (`:262-271`: `CrossPlatformCli`, `LocalUnchecked`, `AlphaBootstrap`) or when the model equals the supported address representation. Search `admit_address_representation\(`: 2 hits. The only caller is `NativeScopePreparation::prepare` (`omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/artifact_admission.rs:176-190`, read). The next statement requires `layout_target_facts()`. That returns `None` when `address_model()` is `None` (`target_profile.rs:409-410`, read), and `prepare` then returns "selected native target has no representation facts". Every path through the accepting `Ok(())` in this caller hits that error when there is no address model. A profile whose model is present but not the supported representation is already `Err` inside `admit_address_representation`.

`native_target` (`:422-435`, read) maps `CrossPlatformCli` and `LocalUnchecked` to `NativeTarget::host()`, and `AlphaBootstrap` to `unreachable!`. The file header (`:5-8`, read) says profiles realized outside this compiler, with `alpha_bootstrap` as the example, stay valid catalog identities and refuse native realization. The host arm is that written rule for the other two profiles, not a missing-input fallthrough. I grepped `.native_target()` and the result was truncated past 120 hits. I did not read those callers. I am not claiming they reject a program. I am claiming this arm is not an unknown accept: it names the two profiles.

Digest: the address-model sentence is right about this function and wrong about the outcome, because the only caller rejects it. The host-realization sentence describes the arm accurately and overstates it as acceptance of an unknown target.

Not read: the callers of `native_target`.

## 5. `bs/host/bindings.rs:290-291`

`TerminalBuildBindingHost::poll_host_computation` (`bindings.rs:272-293`, read). A non-boundary effect is `Err`. A boundary that matches a registered query calls `poll_query`. Any other boundary returns `Ok(true)`.

Search `poll_host_computation\(`: 7 hits. The production call is `engine_calls.rs:3491` inside `boundary_effect_wait` (`:3459-3501`, read). `Ok(true)` means computed, so the function returns `Ok(None)`. The caller (`:3448-3456`, read) then calls `complete_boundary_effect` (`:3504-3528`, read), which calls `handle_effect_with_buffers`.

`handle_effect_with_buffers` (`:248-270`, read) handles a `BindNative` call or rejects unexpected buffers. Otherwise `handle_effect_result` (`:361-390`, read) handles a registered query or a registered call, and otherwise returns `Err` "Build binding call has no source-issued designated operands". An effect that took the `None => Ok(true)` arm and is not in `host.calls` is rejected there. A native bind and an ordinary selection are registered calls, so they are handled, not accepted as unknown.

The trait default (`psi/interpreter/terminal-interpreter/src/boundary_effects.rs:3562-3566`) also returns `Ok(true)`. That is a different function. This entry is the build-settings impl. The embedding forwarder (`embedding.rs:1788-1792`) was not read past the signature; the engine path above is the one that both polls and completes.

Digest: right about the `Ok(true)` arm. The following commit rejects an unregistered effect.

Not read: `poll_query` and `commit_native`.

## 6. `bs/admission/checking.rs:118` and `:161`

`prepare_checked_build` (`checking.rs:43-198`, read through the host substitutions and the candidate error). Search `prepare_checked_build\(`: 2 hits. The caller is `prepare_build_evaluation` (`build_evaluation.rs:280`, and the profile destructure at `:226-227`, read). Search `prepare_build_evaluation\(`: 3 hits. Callers are `build_continuation.rs:147` (read `:116-170`) and the example `restricted_consent.rs:102` (read `:116-117`).

Both compiler entry points fill the profile first. `begin_package_build_worker` (`comp/checked/checking.rs:348-351`, read) and `begin_source_compilation` (`comp/checked/compilation_session.rs:91-94`, read) call `resolve_compilation_defaults` (`omega/packages/package-compilation/src/build_execution/execution_inputs.rs:36-45`, read). That sets `profile` to `host_if_supported()` when it was `None`. The field comment at `:20` says compilation requests default to the host. The example passes `Some(host)`.

`unwrap_or_else(TargetProfile::host)` at `:118` and `:161` therefore runs only when the profile is still `None`, which is an unsupported host. `TargetProfile::host` (`tgt/target_profile.rs:300-301`, read) panics with "unsupported host profile for Omega native planning". It does not accept the build. When candidates exist and a profile is present, planning uses that profile. The error at `checking.rs:143-147` (read) is the nonempty-candidate case with a missing profile; on a supported host the profile is already `Some`, so that error is not the path that accepts.

Orchestration also fills a profile or errors before evaluation (`orch/compile/build_execution/source_resolution.rs:162-169`, read), and `source_build_request` forces `Some` (`omega/packages/package-manager/src/build_session.rs:165-173`, read).

Digest: right that these two lines substitute the host. Wrong about the outcome for a program: the substitution is not reached on a supported host, and it panics on an unsupported one.

Not read: `settle_provider_defaults`. It is not needed. The branch that would call it with a substituted host does not accept.

## 7. `bs/admission/selected_constants.rs:80-84` and `:96-113`

`SelectedConstantEvaluation::prepare` (`selected_constants.rs:61-119`, read in full). Search `SelectedConstantEvaluation::prepare\(`: 1 hit, `checking.rs:159`. It runs only when `had_pending` (`checking.rs:156-166`, read).

For each resolved operator use, `selected_requirement_plan` returning `None` continues (`:80-83`). A plan that yields no provider body and no primitive float execution is not pushed (`:85-113`). There is no `else`. `validate_selected_operators` and `validate_selected_provider_bodies` then run on the recorded lists only (`:115-117`).

`evaluate` (`psi/compilation/build-time-evaluation/src/const_evaluation.rs:5151-5178`, read) folds layout queries and calls `evaluate_selected_operators` with those recorded lists. `prepare_selected_execution` (`machine_execution.rs:5444-5461`, read) returns `Ok(None)` when both lists are empty, and otherwise rewrites only the expressions that are in the lists (`:5464-5514`, read).

A constant binary with no builtin meaning is rejected (`const_generic_expressions.rs:4461-4466`, read). `has_builtin` (`:2644-2657`, read) is true for a closed context, and otherwise calls `has_builtin_binary_expression_meaning`. That check is not "was this use recorded in `SelectedConstantEvaluation`". I did not read `has_builtin_binary_expression_meaning`, and I did not read the path that executes a use whose selected plan was skipped.

Verdict UNDECIDED. What would decide it: whether a pending continuation whose requirement had a selected plan, and whose plan was neither a provider body nor a float operator, fails inside `evaluate_selected_operators` or is folded without that plan.

Digest: right about the skip and the missing `else`.

Not read: `derive_selected_primitive_float_binary_execution` and `has_builtin_binary_expression_meaning`.

## 8. `orch/project/build_directory.rs:254`, `:271-273`, `:217`

`check_owner` (`build_directory.rs:231-262`, read in full). Search `check_owner\(`: 3 hits, all in this file: the definition, the call from `acquire` (`:173`), and the recursive call after `AlreadyExists` (`:246`).

A missing owner file is created with a `root ` line (`:238-243`). A read error other than not-found is `occupied_io` (`:251`). A recorded root that differs is `AliasedRoot` (`:255-260`). A file that exists and has no line starting with `root ` makes `recorded` `None`, and that arm returns `Ok(())` (`:253-254`). `acquire` (`:167-189`, read) then creates the occupant and returns the directory. A second project's build is admitted into a directory whose owner file has no root line.

`canonicalize_or_literal` (`:271-273`, read) uses the spelled path when `canonicalize` fails. `directory_object` (`:216-218`, read) turns an identity-lookup error into `None`, so `DirectoryIdentity` equality (`:220-228`, read) compares paths only when both lookups fail. I did not find a later check that restores the object identity. Those two accepts are real. The owner-file arm is the one that admits a different project.

Digest: right about all three.

Not read: `filesystem_object_identity`. The `.ok()` at `:217` is what drops its error.

## 9. `orch/package/package_review.rs:205-207`

`component_descriptions` (`package_review.rs:185-216`, read). The header (`:9-30`, read) says a cached dependency that lacks its description returns `ComponentDescriptionNotRetained`. The `?` on `round.component` propagates that error. The `continue` is only the `Ok(None)` arm.

`PackageReviewRound::component` (`omega/packages/package-manager/src/review/candidate/passes/occurrences.rs:620-655`, read in full). `Ok(None)` is returned only when `component == occurrence.key.identity()` (`:627-628`), the package currently being reviewed. A live or cached candidate returns `Some`. No candidate returns `Err` "which this closure never compiled" (`:647-653`).

`with_independent_component_descriptions` (`omega/packages/package-compilation/src/package_compilation.rs:443-478`, read) checks the descriptions it is given. It does not look for missing required components. It does not need to: a foreign required component with no candidate already failed in `component`. The self-package skip matches `required_components`, which also skips `component == inputs.root()` (`orch/compile/build_execution.rs:410-412`, read).

Digest: wrong. The skipped case is not "no candidate description". No candidate is an error. The skip is the package itself.

Not read: other functions named `component` in unrelated files. The call here is this method.

## 10. `ba/lib.rs:201-203`

`TerminalIeeeFloatComparisonOccurrenceProposal::validate_roster` (`lib.rs:171-251`, read). It checks that the plan index is in range (`:201-203`) and does not compare `provider_plan_commitment`. The integer roster does (`:301-306`, read).

Search `TerminalIeeeFloatComparisonOccurrenceProposal {`: 3 hits. Two are the struct and its impl (`lib.rs:138`, `:147`). The only construction is `settlement.rs:186-201` (read `:140-238`). That function sets `provider_plan_commitment` from `plans[provider_plan_index].identity_digest()` (`:154`) and then calls `validate_roster` (`:226-238`). The index was just taken from that same `plans` slice (`:141-153`). A digest that disagrees with the plan cannot be passed in by this caller, and there is no other caller.

Digest: right that this roster does not compare the commitment. The commitment cannot disagree on the only path that reaches the check.

Not read: later consumers of the stored commitment field. They receive a value copied from the plan this function just checked by index.


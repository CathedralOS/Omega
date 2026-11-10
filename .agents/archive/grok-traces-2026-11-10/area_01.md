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
| 11 | cd/component_description.rs:334-336 | UNDECIDED | A missing identity becomes `""` and is stored in the called-requirement list. No emptiness rejection was found in the provider-closure check that was read. |
| 12 | nc/realization/callback_thunks.rs:507 | UNDECIDED | `(None, None)` emits an ordinary callback. Whether a real handler is dropped from its own interrupt cohort before `retain` was not settled. |
| 13 | whr/windows_handle_origins.rs:87 | CLOSED | Those origins are marked unknown, and the argument solver rejects an unknown word. The site comment already says they are unknowns. |
| 14 | pa/provider_installation.rs:219-229 | UNDECIDED | An unselected catalog row is not installed. Whether a call to a boundary whose candidates were all filtered out is rejected later was not settled. |
| 15 | pa/provider_installation/replay.rs:66-68, :151-160, :33-52 | CLOSED | A call with no replay row is left out of the selected-call map, and the settlement roster then requires a settlement for it. |
| 16 | pp/intrinsic_declarations.rs:314-319 | REACHABLE | Review settles a hosted row with no declaration identity, so a failed declaration identity is never compared. |
| 17 | pp/data_layout_queries/builder/data_layouts.rs:170-185 | UNDECIDED | A plan-laid record with a failed identity and no callback demand is still returned. Whether the layout-query caller enters that arm was not settled. |
| 18 | pp/data_layout_queries/builder/private_callback_closure.rs:69-74, :184-186 | CLOSED | The record that owns the callback demand is visited on its own, and the demand match happens before the enum skip. |
| 19 | pp/program_entry/source_contract.rs:121-124, :128 | CLOSED | The only consumer passes those strings into a constructor that rejects an empty parameter or result identity. |
| 20 | pp/provider_identity.rs:22 | UNDECIDED | A plan with an application identity skips the no-type-parameter test. Whether that plan is supposed to be rejected was not settled. |
| 21 | pp/provider_instances.rs:229, :256 | UNDECIDED | Import, vtable, and table rows get an empty instance list. Whether those rows can carry closed requirement instances was not settled. |
| 22 | pp/provider_planning/external_binding_rows.rs:167-194 | CLOSED | A syscall on a target with no kernel policy is rejected when the call is realized, before the empty application list is used. |
| 23 | pp/provider_planning/provenance_replay.rs:357-391 | CLOSED | A demanded boundary whose intrinsic execution is missing is rejected. `settle` does not itself reject the unknown machine. |
| 24 | pp/provider_planning/provenance_replay.rs:175-176 | UNDECIDED | A missing trait or signature text is interned as `""` with no check in `mint_plan_row_identity`. Whether a demand can match that empty text was not settled. |
| 25 | pp/provider_planning/provenance_replay/adapter_conformance.rs:232 | UNDECIDED | The provider-row join is skipped when `provider_type` is empty. One caller rejects that emptiness first. The selected-facts caller does not, and I did not read its admission check. |

`orch/` is `omega/build-system/orchestration/src/`. `ba/` is `omega/compilation/backend/artifacts/boundary-applications/src/`. `cd/` is `omega/compilation/backend/artifacts/component-description/src/`. `nc/` is `omega/compilation/native-compiler/src/`. `pa/` is `omega/compilation/realization/provider-admission/src/`. `whr/` is `pa/provider_admission/terminal_authority_review/`.

`pp/` is `omega/compilation/realization/provider-planning/src/`.

Entries 26 onward are not written yet.

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

## 11. `cd/component_description.rs:334-336`

`identity_text` (`component_description.rs:334-336`, read). `module.identities.text(identity)` missing becomes `""`. Search `identity_text(` in this crate: the definition, eight call sites in `component_description.rs`, and uses in `component_verification.rs`.

`derive_component_inventory` pushes that string into `called_requirement_identities` with no emptiness test (`:567-572`, read). The provider-closure check (`component_verification.rs:780-829`, read) rejects an empty `requirement_identities` list on a retained provider (`:799-803`) and a zero plan digest. It does not reject a called identity whose text is `""`. I stopped at `:829`.

Verdict UNDECIDED. What would decide it: the rest of that verification function, and whether a missing identity index can occur for a module that still verifies. If `""` is compared with a plan row whose requirement text is also empty, two different missing identities would compare equal.

Digest: right that a missing identity becomes `""`.

Not read: `component_verification.rs` after `:829`.

## 12. `nc/realization/callback_thunks.rs:507`

`PreparedEntryPlans::for_body` (`callback_thunks.rs:479-526`, read). `(None, None)` returns `Ok(None)`. Any other mismatch is `Err`. The caller is `lower_callback_thunk` (`:305-320`, read). `Ok(None)` skips `with_installed_entry_execution`, so the body is prepared as an ordinary callable.

`for_body` runs before `prepare_installed_callbacks`, which is the function that calls `retain` (`:153`, read). `realize` (`:75-114`, read) stores the first object and later returns it without lowering again (`:87-95`).

The cohort `for_body` reads is `admitted_installed_interrupts()` on the body just lowered. `admit_prepared_native_realization` keeps a parent interrupt when `in_scope` is true or a callback occurrence matches (`native_execution_scope.rs:707-727`, read). `in_scope` (`:681-684`, read) is true when the operation is in the body's plan, or when the body's module does not contain that operation as a boundary call. If the handler's own admission still contains the placement, `for_body` sees `Some` against a `None` slot and rejects. If that admission drops the placement, `for_body` returns `Ok(None)` and the ordinary object is reused.

Verdict UNDECIDED. What would decide it: for one installed-handler body, whether the filtered `installed_interrupts` in `admit_prepared_native_realization` still contain that placement. I did not execute a handler and I did not read `lower_callable` past `retained_terminal.rs:374-401`.

Digest: right about the `(None, None)` result. The producer half is the filter above, which I read, and the body's own operation set, which I did not.

Not read: `InstalledInterruptEntryPlan` consumers after a successful `Some` arm.

## 13. `whr/windows_handle_origins.rs:87`

`OriginKind::ExternalInput` and `OpaqueObservation` (`windows_handle_origins.rs:86-89`, read). The comment on the site says inputs or observations with no derived identity remain explicit unknowns, and no consumer may treat absent edges as empty reach. The digest's "no reason at the site" is wrong. The comment is the reason.

`HandleArgumentValues::from_origins` maps both, plus `NativeOutput`, `IntrinsicReturn`, and `InactiveCase`, to `ArgumentValues::Unknown` (`windows_handle_arguments.rs:250-254`, read). `actual` (`:290-307`, read) returns `Err` "has no complete closed value derivation" unless the value is a nonempty `Finite` set. `words` and `exact` (`:310-344`, read) call `actual`. `prepare_handle_arguments` (`:151-181`, read) sends `CreateFile`, `DuplicateHandle`, `QueryFileMode`, `ReadFile`, `WriteFile`, and `QueryFinalPath` through those solvers. Search `OriginKind::ExternalInput`: I read the match arm; I did not grep a count beyond this file and `windows_handle_arguments.rs`.

An external parameter or an opaque observation used as one of those native arguments fails the word check. It is not treated as an empty handle list.

Digest: the "no sources" description matches the variants. The open question is closed by `actual`.

Not read: `windows_handle_replay.rs` outside this argument path. `NoHandleIntervention`, `CurrentProcess`, and `CloseHandle` take `CurrentValues` and do not call `actual` (`:175-177`). I did not read whether those contracts can consume an `ExternalInput` origin.

## 14. `pa/provider_installation.rs:219-229`

`admit_terminal_provider_installation` (`provider_installation.rs:214-232`, read). The filter keeps a catalog candidate only when some selected row's requirement text equals `identities.text(candidate.requirement_identity)`. Search `admit_terminal_provider_installation`: I used the definition and did not record a second caller in the earlier grep of this name. `select_provider_candidates` (`:234-316`, read) groups the already filtered list. `MissingSelectedProvider` (`:278-281`) fires only for a boundary that survived the filter and whose text is then absent from `selected_by_req`. An unselected catalog row never enters that list, so it is not installed and it does not raise that error.

That drop is the selection. It is a wrong accept only if a program can call a boundary whose every catalog candidate was filtered out and a later stage still accepts the call. I did not read the interpreter's missing-provider check.

Verdict UNDECIDED. What would decide it: the consumer of `AdmittedProviderSelections` when a called boundary has no installed candidate because the filter removed it.

Digest: right that unselected rows are dropped before the missing-provider check. That check cannot see them.

Not read: `embedding.rs` and `engine_execution.rs`.

## 15. `pa/provider_installation/replay.rs:66-68`, `:151-160`, `:33-52`

`replay_installed_provider_calls` (`replay.rs:24-160`, read through the second `continue`). Search `replay_installed_provider_calls\(`: 2 hits, the definition and `provider_installation.rs:200`. An empty `installed` slice returns no calls (`:30-32`). `or_insert` keeps the first boundary, assumption, function, and machine (`:33-52`). An external completion or boundary call with no installed provider `continue`s (`:66-68`, `:151-160`).

The caller stores those calls on the installation (`provider_installation.rs:185-205`, read). `index_installed_provider_calls` (`coordination.rs:43-71`, read) inserts them and rejects a duplicate key (`:63-68`). `selected_boundary_calls::index` (`omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lib.rs:538-675`, read) leaves a call with no installed row and no target family out of the selected map (`:674`, `(None, None) => continue`). `validate_settlement_roster` (`coordination.rs:430-476`, read) then requires a settlement for every boundary call absent from that map (`:460-467`) and rejects a boundary that was only partly installed (`:446-458`). The call at `coordination.rs:534-546` (read) runs this after the index.

A call with no installed provider is not replayed, and it is not accepted unless a settlement covers it. Two installed candidates for one boundary cannot reach `or_insert` from `select_provider_candidates`, which requires exactly one match (`provider_installation.rs:295-301`, read).

Digest: right about the skip and first-wins. The skip does not accept the call.

Not read: the external-completion arm of `index_boundary_calls` past `:89`. Boundary calls are indexed. External completions are handled in `replay` and in `selected_boundary_calls::index`, which I did read.

## 16. `pp/intrinsic_declarations.rs:314-319`

`IntrinsicDeclaration::settle` (`intrinsic_declarations.rs:274-333`, read). For a hosted row, `subject.matches` (`:84-112`, read) requires the role, package owners, declaration path, schema digest, and plan digest. If that passes and `binding.declaration_identity` is `None`, the arm returns `true` (`:314-318`) and does not read `declaration_identity` on the declaration. An `Err` stored there is ignored.

`retain_selected_compiler_intrinsic_review_identities` builds that binding with `declaration_identity: None` (`intrinsic_review.rs:220-223`, read) and passes it to `settle_declaration` (`:246-252`, read). `settle_declaration` (`compiler_intrinsic.rs:127-139`, read) returns `Closed` when `settle` returns `Some`. Search `settle_declaration`: the review call above and the definition. I did not read a second constructor that passes `Some`.

A hosted console or process-exit plan whose package, path, schema digest, and selected plan digest match the accepted binding is closed even when the declaration's own identity result failed.

Digest: right.

Not read: `compiler_intrinsic.rs` above `:120`. The numeric arm returns before this settle when a numeric identity is present (`intrinsic_review.rs:244-245`).

## 17. `pp/data_layout_queries/builder/data_layouts.rs:170-185`

The plan-laid record arm (read `:170-243`) turns `native_policy_identity` and `native_data_identity` errors into `None` with `.ok()`. The identity row is pushed only when both are `Some` (`:211-236`). If callback demands are nonempty, a missing subject is `Err` (`:191-200`). If they are empty, the function still returns `Ok(DataLayout ...)` (`:238-243`). `LayoutBuilder::finish` (`builder.rs:255-266`, read) keeps `data_layouts` after closing callback layouts.

Search `build_layout_plan\(`: 7 hits, 6 in `builder/tests.rs` and the definition. `try_layout_type_reference` is called from `data_layout_queries.rs:128`. I did not read `layout_type_reference_handle`, so I do not know whether that call enters this plan-laid arm.

Verdict UNDECIDED. What would decide it: whether `measure` in `data_layout_queries.rs` reaches `layout_data_definition`'s plan-laid arm. If it does, a record with no callback slot and a failing identity derivation is laid out with no identity row. If it does not, the arm is test-only.

Digest: right about the `.ok()` and the missing row.

Not read: `native_policy_identity` and `data_layout_queries.rs:128` past the call.

## 18. `pp/data_layout_queries/builder/private_callback_closure.rs:69-74` and `:184-186`

`close_private_callback_layouts` (`private_callback_closure.rs:22-74`, read) walks every `DataLayout`. A non-record shape `continue`s (`:69-74`) and gets no natural-layout identity. `reaches_slot` (`:166-196`, read) returns true when `demand.data_symbol == symbol` (`:173-174`) before it looks at the shape. A non-record then returns false (`:184-185`) and is not descended.

The loop visits the record that owns the demand as its own layout. That record hits the demand match and is not skipped by the enum arm. An enum root is not used as a path through a case payload. The slot's record does not need that path.

Digest: the skip of enum and mixed shapes is what the code does. A callback slot on a record is not missed because the slot sits in a case.

Not read: `close_path` and `install_geometry`. They run after a record has already been selected.

## 19. `pp/program_entry/source_contract.rs:121-124` and `:128`

`prepare` (`source_contract.rs:106-131`, read) stores `type_identity_text(...).unwrap_or_default()`, so a missing text is `""`, for each parameter and for the result. Search `PreparedEntryCallingContract {`: the struct and this constructor. The only consumer is `program_entry.rs:302`, which calls `calling_plans::settle` (`:598-617`, read). `settle` rejects a missing result (`:716-719`, read) and passes the strings to `ProgramEntryPhysicalContractPlan::new` (`:721-729`, read). Search `ProgramEntryPhysicalContractPlan::new\(`: 1 hit, that call. `new` (`program-entry-plan/src/program_entry_physical.rs:154-159`, read) returns `Err` when any parameter identity is empty or the result identity is empty.

Digest: right that the string becomes `""`. Empty does fail, in the only consumer.

Not read: `physical_source.validate` before the `new` call. It does not replace these strings. `settle` passes the contract's strings through.

## 20. `pp/provider_identity.rs:22`

`named_provider` (`provider_identity.rs:16-30`, read). The filter is `(plan.application_identity().is_some() || eligible(definition))` plus path and package equality. `eligible` is `type_parameters.is_empty()` (`psi/compilation/build-time-evaluation/src/product.rs:1938-1940`, read). A conformance with type parameters is returned when the plan's application identity is `Some` and it is the unique path and package match.

Search `named_provider\(`: 5 hits, three in `provider_identity.rs`, `provider_instances.rs:171`, and `adapter_conformance.rs:95`. Both outside callers use the symbol with no second `eligible` call (`adapter_conformance.rs:95-103` and `provider_instances.rs:170-171`, read).

I did not read `application_identity()`. The disjunction may be the captured-application case, where a generic declaration is the intended owner. I cannot say the generic conformance should be rejected.

Verdict UNDECIDED. What would decide it: the body of `ProviderPlan::application_identity` and whether a nonempty application is allowed to name a generic conformance.

Digest: right that `eligible` is skipped when an application identity is present.

Not read: `provider_plan.rs` around the application-identity accessor.

## 21. `pp/provider_instances.rs:229` and `:256`

`prepare_instance_declarations` (read `:154-259`). `CompilerIntrinsic` and `Syscall` inherit leaf applications (`:175-196`). `CheckedAdapter` inherits through `generic_instances` (`:241-255`). Every other binding, including `Import`, `VtableField`, `TableFunction`, and `VtableSlot`, takes `_ => Ok(Vec::new())` (`:229` and `:256`).

I did not read `inherited_instances` or `closed_requirement_applications`. An empty list drops whatever those functions would have found for a checked adapter. I cannot say an import or vtable row has such instances.

Verdict UNDECIDED. What would decide it: whether `closed_requirement_applications` returns a row for one of those four bindings.

Digest: right about the empty list.

Not read: `providers_and_trust.rs`.

## 22. `pp/provider_planning/external_binding_rows.rs:167-194`

`ExternalBindingContext::row` (read `:165-205`). For a syscall, `compatibility_policy` is `CallingPolicy::native_syscall_for_target`. That is `None` for Mach-O and COFF (`calling-conventions/src/plans/vocabulary.rs:498-510`, read). The default-application fill runs only when the policy is `Some` (`:172-174`), so the row is returned with the applications it already had, including none (`:195`).

`rejoin_system_call` is the realization of `NativeBoundaryRealization::SystemCall` (`settlements.rs:125-134`, read). Search `rejoin_system_call\(`: 2 hits, the definition and that call. It rejects a target with no kernel policy (`:518-519`, read) before it reads applications. When a policy exists, `external_calling_application` (`:271-279`, read) rejects a row whose ABI application is missing.

Digest: right that the row can be emitted with an empty application list. Using that row does not succeed.

Not read: `evaluate_default_calling_applications`.

## 23. `pp/provider_planning/provenance_replay.rs:357-391`

`inferred_selected_target_compiler_leaf` (read in full, `:357-391`). On a non-hosted target it returns a `CompilerIntrinsic` binding for a bodiless boundary machine with attached data, no parameters, no external binding, one state, and one target origin. It does not check the signature or the machine name.

`settle` returns `Ok(None)` when the target does not match (`intrinsic_declarations.rs:290-292`, read) and for `Unsupported` (`:294`, read). It does not reject an unknown machine by name.

`select_boundary_applications` (`ps/boundary.rs:57-111`, read) zips each row with `row_executions` and, for a `CompilerIntrinsic` binding, requires `execution.ok_or` "selected intrinsic has no canonical or admitted execution identity" (`:109-111`). A demanded boundary whose execution slot is `None` is rejected there.

Digest: right about the leaf. `settle` does not reject it. The demand check does.

Not read: the assignment that copies `settle`'s `None` into `row_executions`. The boundary function rejects a `None` slot whatever wrote it.

## 24. `pp/provider_planning/provenance_replay.rs:175-176`

`exact_satisfied_requirement_identity` (read `:158-181`) returns `""` when the trait symbol has no definition (`:174-175`) and when the signature or callable-identity text is missing (`:177-180`). `plan_derivation.rs:112-116` (read) stores that string, and `:139-152` (read) pushes a row whose identity is `mint_plan_row_identity` (`:550-555`, read). `mint` is `plan.identities.intern` with no emptiness test. The push sits inside `provider_plan_schema_target` returning `Some` (`:118-120`). A trait that has a schema and no callable-identity text still gets the row.

`select_boundary_applications` matches `plan.identities.text(row.requirement_identity)` to `requirement.overload()` (`boundary.rs:74`, read). An empty identity does not match a nonempty overload, so that demand fails the unique-row error (`:78-84`). I did not read `overload()`. If it can be `""`, the empty identity matches it.

Verdict UNDECIDED. What would decide it: whether a boundary demand's overload text can be empty.

Digest: half right. The empty string is interned. The `plan_derivation.rs:112` call does not by itself push a row when the schema lookup is `None`.

Not read: `provider_plan_schema_target` and `named_provider_plans.rs:154`.

## 25. `pp/provider_planning/provenance_replay/adapter_conformance.rs:232`

`exact_checked_adapter` (read `:224-239`) runs `provider_row_matches` only when `plan.provider_type` is nonempty. Search `exact_checked_adapter\(`: 5 hits. The build-time caller rejects an empty provider type before the call (`adapter_conformance.rs:88-93`, read). `provider_instances.rs:115` (read `:103-119`) does not. `validate_candidate_against_schema` rejects an empty provider type on a checked-adapter row (`provider_plan.rs:883-901`, read). I did not read the function that inserts a plan into `SelectedProviderPlanFacts`, so I cannot show that every plan reaching `:115` passed that validation.

Verdict UNDECIDED. What would decide it: the constructor of `SelectedProviderPlanFacts` and whether it calls `validate_candidate_against_schema`.

Digest: right about the local skip.

Not read: `from_selected_plans`. `product_boundary.rs` and the two test hits were not read past the search count.


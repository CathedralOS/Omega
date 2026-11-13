# worker-15 handoff, second session (stopped by owner order): supersedes "State" and "NOT verified" below

## State
Nothing landed. Draft PR #2805 (https://github.com/CathedralOS/Omega/pull/2805), branch `work/compiler-route-application-artifact` @ ab152a210398. Slot worker-15 clean at that commit, nothing running, all background tasks stopped.

## Measured at 1365094ea450 (branch + origin/main 587f1f7265af), host windows_x86_64
- `mbx build -p omega`: ok.
- `omega run --both tests\omegaun\calls\scalar_contract_route_exit\main.omg`: native exit 0, interp exit 0 (no trace this time; the test below pins that the route's artifact is the one attached).
- `mbx test -p compiler --test lowered_route_publication`: 3 passed (65 s). `--lib lowered_route`: 5 passed.
- clippy on the six crates, `fmt --check`, `check-boundary`, `check-rules` (Gate must be spelled `RC-SOURCE-SEMANTICS`): clean.
- `mbx xtask corpus --filter run/calls/scalar_contract_route_exit`: exit 3. Witness row unchanged (checked). Two neighbours `run/calls/saved_scalar_dispatch_guard_arguments_exit` and `run/calls/scalar_guard_after_borrow_parameters_exit` show checked -> rejected: both are router refusals "not supported yet: type float" / "type data with custody" (seen with `omega --check`), i.e. main's #2792, not this change. Not re-run at base.

## NOT verified
Head ab152a210398 merged origin/main d892667b7d59 (changes in lowered-discharge incl. verdict.rs, structure-elaboration, typed-trees service_declarations). Its rebuild was stopped mid-compile: not built, not run. origin/main has since moved to f4a966d6d9b0 (#2804, deletes the tree interpreter) and the branch conflicts with it in one docs hunk (OWNER_CODE_CONCERNS.md or TASKS.md; keep both sides, as done for the earlier OWNER_CODE_CONCERNS.md conflict).

## Exact next step
In worker-15: `git merge origin/main` (resolve the docs hunk), build, witness, the two test commands, clippy on the six crates, fmt, check-boundary, check-rules with `scratchpad/w15r/pr_body.md` (rewrite its "State:" paragraph and validation commit ids, regenerate the provenance line), push, `gh pr ready 2805`, squash-merge on `admission`.

## Item 2 (next consumers): read only, nothing written
- `BuildSourceCustody::seal` (`package-compilation/src/source/source_consumption/checked_sources.rs:105`) reads only `program.symbols.source_files()`. Giving it `&symbols` removes package-compilation's only use of `checked-trees` (drop the Cargo dependency). Smallest first PR.
- `PreparedProviderInventory` (frozen crate `checked-trees/src/providers_and_trust.rs:646`): services, direct calls and origins read only `checked.typed`; only `trust_declarations` (one caller, `selected-dispatch/src/provider_binding.rs:94`) and `uses_for` (no caller outside the crate) read Psi 05 facts. Plan: a typed-trees-owned inventory (services + direct calls + origins) that owns nothing and checks its owner the way `PreparedServiceCatalog::bind` does (identity + address), methods taking `&FrozenTypedTrees`; delete those halves from the checked-trees type. Call sites: `target_preparation.rs` (7), `terminal.rs` (3), `terminal/product_code.rs`, `source_checking.rs`, `provider_binding.rs:101`, `review-evidence/src/capture/runtime_permissions.rs:57`, `psi-compiler/terminal-production/src/lib.rs:48,407,454`.
- `CheckedTrees` owns the typed trees by value, so no `Arc<FrozenTypedTrees>` exists to hand consumers until Psi 05 stops taking them; 117 files under `omega/` name `CheckedTrees`.
- Draft #2796 (delete Psi 06) and worker-3's Build-program work both touch `terminal-production`; check them before editing it.

---
(first session's note follows)

# worker-15 handoff, 2026-11-13: LOWERED-FORM-CHECKER step 1, product route

## State
Nothing landed. One capability is pushed, not yet a PR:
`origin/work/compiler-route-application-artifact` @ 553abd00cdcc (commit 1d6cb9b1644f + merge of origin/main 7f351ed23dba).
Slot worker-15 is clean at that commit, nothing running.

## What the branch does
The default compile builds the executable application from the module the lowered-form checker published, and carries its evidence seals to Omega 00.
- `compiler/src/checked/checking/lowered_route.rs` `RouteScope::judge` now returns `(RouteJudgment, Option<RouteTerminalProgram>)`: it calls `AcceptedClosure::publish` on the first verdict (the closure of the roots) when the whole compilation is accepted. `publish` error => `None` (application stays on Psi 06's program).
- `products/src/terminal_publication.rs`: `RouteTerminalProgram { artifact, seals }`, `ProductTerminalPublication::with_route` / `route()`; `for_other_roots` (test roots) drops it.
- `compiler/src/checked/checking/source_checking.rs`: threads it into the publication.
- `compiler/src/terminal/terminal_artifact.rs` (Application branch of `produce_retained_terminal_artifacts`): attaches to the route artifact via new `psi_compiler::TerminalApplicationCustody::attach_published` (`psi-compiler/terminal-production/src/lib.rs`); receipt, roots, `provider_publication.publish(artifact.view(), ..)` all use it.
- Seals: `native_compiler::TerminalApplication::evidence` -> `RetainedTerminalArtifact::with_evidence/evidence` (`verified-input/src/native_proposal.rs`) -> `native-compiler/src/compile.rs` after `work.lower` calls new `VerifiedNativeArtifactInput::attach_evidence` (`00_terminal-psi-to-abstract-operations/src/artifact_admission.rs`), which refuses seals naming another module.
- Test: `compiler/tests/lowered_route_publication.rs` new `the_default_compile_builds_the_application_from_the_routes_module`; the identity test now reads the old program from `checked.providers().calling_program.artifact`.
- Docs: TASKS.md step 1, `wiki/drafts/designs/check_on_lowered_form.md` "The gate" item 1, one entry in OWNER_CODE_CONCERNS.md.

## Measured (before merging main; base b57f2b5ac603)
- `omega run --both tests\omega\run\calls\scalar_contract_route_exit\main.omg`: native exit 0, Psi exit 0; a temporary trace (removed) showed `publish: Ok` and the 6-machine route artifact attached (old program: 1,268 machines, 612 boundary declarations, 146 float conformance rows).
- `omega --target linux_x86_64 --build-dir <tmp> ...`: image produced from the same artifact (not executed).
- `mbx test -p compiler --test lowered_route_publication`: 3 passed (689 s). `--lib lowered_route`: 5 passed.
- `mbx xtask corpus --filter run/calls/scalar_contract_route_exit`: blast clean, 3 fixtures (check mode).
- clippy on the six touched crates, check-boundary: clean. fmt: one file reformatted afterwards.
- CLI probe of 13 other pass/arithmetic and run/calls fixtures: none accepted whole by the route, so none changed path.

## NOT verified
After merging main (which brought #2786.. and the fall-back removal: the router now refuses with "not supported yet" and `judge(...).verdict()`), I re-applied the hand-off on the new `lowered_route.rs`/`source_checking.rs` by hand. That merged tree has NOT been built or tested: the revalidation run was stopped at the coordinator's stop order. No PR opened, no provenance line, check-rules not run.

## Exact next step to land it
In worker-15 (or any slot on that branch): `mbx build -p omega`; `omega run --both` on the witness; `mbx test -p compiler --test lowered_route_publication` and `--lib lowered_route`; `mbx xtask fmt --check`; clippy `-p compiler -p native-compiler -p terminal-psi-to-abstract-operations -p compilation-products -p verified-optimization-input -p psi-terminal-production`; `mbx xtask corpus --filter run/calls/scalar_contract_route_exit` (do not edit files while it runs: it fails with "source changed during producer preparation"). PR body draft: scratchpad `w15r/pr_body.md` (fill @BASE@, @UNIT@, add provenance). Title: `compiler: build the application from the module the lowered-form checker published`.

## What I learned about the hand-off
- The board's "route must publish boundary machines and ExternalProviderConformance rows" was not needed by the default compile for a no-float program on windows_x86_64 or linux_x86_64. The "NativeFloats has no retained external implementation" refusal comes from `validate_external_provider_owners` (`provider-admission/.../compiler_builtins/retained.rs:287`, called at `verified-input/src/native_proposal.rs:338`) when the proposal request carries provider declarations prepared for the OLD program (as the trial and the Codex test do via `checked.terminal_native_proposal_request`). A scoped twin `validate_retained_external_provider_owners` exists. Codex has rows in flight on `origin/work/compiler-lowered-provider-publication` (no PR; rows only, no boundary machines).
- The published "product code universe" (`compiler/src/terminal/product_code.rs` `ProductCodeSource::publish`) is not just the application: it holds calling-policy bodies (`LinuxX86_64CallingPolicy::plan` etc.), constant recipe providers and boundary catalogs. `terminal.rs` `retain_calling_program` executes policy bodies from that artifact during execution settlement. So "no old-stage code on the path" needs the route to judge and publish those std library machines.
- Everything after `govern` in `source_checking.rs` needs `Arc<CheckedTrees>`: `selections.select`, `build_sources.seal(&program,..)`, `PreparedProviderInventory::new`, `PreparedNativeBuildBindings::capture`, then `execution_settlement.rs`. That is the next native-side consumer: build `CheckedProductSource` from typed trees + the published module.
- `PortableTerminalProgram` (psi-compiler terminal-production) depends on checked-trees and Psi 06/08; the branch avoids constructing one for the route (only `attach_published`).
- Each `lowered_route_*` integration test compile takes 3 to 8 minutes in the dev profile; `omega run --both` on a small fixture is seconds; a linux cross build about 2.5 minutes.

## New fact from the coordinator (not verified by me)
Every project's Build program (`build.omg`) is checked by Psi 05, lowered by Psi 06 and run in the terminal interpreter before the product check starts (`build-settings/src/activation/preparation.rs` `PreparedBuildExecution::prepare` -> `TerminalProductionRequest::produce`; `build-time-evaluation/src/machine_execution.rs`). So the first consumer the route must feed is Build execution, ahead of the native product: the route must lower and publish the Build program (it roots neither the Build program nor the entry-contract source today), and `PreparedBuildExecution::prepare` must take that published artifact. The branch above is still needed after that (it is the application's path), but it is not the first blocker once the old stages are gone.

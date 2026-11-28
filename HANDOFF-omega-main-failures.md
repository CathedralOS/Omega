# Handoff: omega tests broken on main

This note covers fixing the omega application tests (`source-rust/application/tests/*.rs`) that failed on main at a78c1b68. Everything listed as done is merged. No code is in flight. This branch holds only this note, so delete it rather than merge it.

## Done (all squash-merged)

- #6495: psi `find_by_identity` fix.
- #6619: Stage 02 no longer refuses absent operands as stale (subjectless `match`, positional `_` and `..`). Also fixes stale test sources (`host` alias, `use x;` message, `addr`, and Application changed to Package for check-only consumers).
- #6630: a check-only Build resolves its provider backing from the rejoined source.
- #6634: three tests whose sources had drifted (named_build_entry, entry_rows_every_target, nominal_const_extent).
- #6635: a consumer's Product now receives a package's Build-scope provider choice (`compile_product/retained_application.rs::receiving_file`). Diagnostics now print file-qualified names such as `|app|::Reading`, and the tests assert them.
- #6636: the saved .psi fixtures were republished under semantic format 303, after #6594 bumped it. This covers `tests/fixtures/calls/run/scalar_contract_route_exit/linux_x86_64.psi` and its README and digests, plus the six `tests/fixtures/providers/pass/source_provider_choices/*.psi`.
- #6639: the `selection admission` timing row is recorded in `SourcePublication::publish` (the `--check` path). The runtime_boundary test now checks only the CLI's production dependencies.
- #6642: a check-only Build subject's refusals are located against its own source copies, so they print a file and line instead of `source-unit N`. Also updates the audit test to use `BuildProduct::InterpretedBuild`, and fixes the 16:22 position.

## What's left, by cause

The full table is in `/mnt/project-files/omega-main-failures/remaining.md`.

1. **theorem_proof_parameters (2 tests): "runtime callable lost its supplying type-parameter scope". Partly traced.**
   - To reproduce, run `omega --check main.omg` on `source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures/proofs/positive/named_implication/main.omg`.
   - The fixture declares `data NonzeroIsPositive = machine(n: u8) requires ... ensures ...` and calls `Step(n)` through `use_step<Step: NonzeroIsPositive>`.
   - In `names/runtime_callable_types.rs::lexical_parameters`, the contract's `lexical_owner` is a TypeAlias symbol (258) named NonzeroIsPositive. `program.type_aliases` holds that alias at a *different* symbol (2605).
   - 258 is the seeded root symbol: `install_root_family!(type_aliases, ...)` in `names/symbol_table.rs`. Something later reallocates the alias symbol, possibly the append, extension or alias-rewrite path, and `runtime_callable_scopes::bind_program` has already bound the contract to the old one.
   - Next step: find who writes the 2605 symbol, then either rebind the contracts or resolve the alias by identity.
2. **Selection-context refusal (6 tests).** Traced; see remaining.md. A Build that designates a generic product application forms a retained trait frame whose `argument_owner` is the Build. That records a Build-to-Product Description dependency, which selection admission correctly refuses. This is a design or owner question, so file it in OWNER_QUESTIONS through the owner-questions skill. Do not weaken admission.
3. **build_helpers_are_checked_at_the_call_site_in_both_file_layouts, imported layout.** An unknown name or a possible overflow in a private, unreached machine of a Build-imported `support/support.omg` is accepted, so that file is not checked whole.
4. **Checker and lowering "not supported yet" gaps:**
   - whole-trait supply with parameters (build_applied_provider_choices, 2);
   - "machine no complete body" (build_staged_dependency_calls, build_selected_providers);
   - atomic non-once `compare_exchange` (atomic_acquisition, 6);
   - array_reference_loop, proof_only_contract_identity, retained_cast_membership.

   These are features, not test fixes.
5. **Reach ceiling refusals:** independent_component, psi_runtime_admission, reflected_field_inspection. Zac's #6597 may cover them.
6. **One-offs, not yet traced:**
   - domain_access_field_observations: "domain `Restricted` does not qualify carrier `|layout|::LayoutStorage<..>`".
   - registered_private_filesystem: "object semantic interval differs from its physical spans".
   - staged_build_interpreter_providers: "no admitted logical file `clock.omg`".
   - target_check: windows row "boundary requirement has no provider".
   - build_prelude_operation_calls: darwin poll exclusions.
   - installed_build_files.
   - target_selected_implementations (2).
7. **PROOF-KERNEL-CORE:** rerun the closure check once the identity, families and Real threads land.

## Commands

- Build the CLI with `mbx build -p omega --bin omega`. The binary is `target/debug/omega`.
- Run one test binary with `mbx test -p omega --no-fail-fast --test <name>`. A cold build takes about 10 minutes.
- Other crates: terminal-codec, native-compiler, retained-image-runtime, source-compiler (`--lib provider_choices::tests`), terminal-runtime, project-build.
- Admission, before each PR:
  - `mbx xtask fmt --check --base origin/main`
  - `mbx xtask check-rules --base origin/main --title "<title>" --body-file <body>`

## Gotchas

- Use `mbx` only, never raw cargo. Never run plain rustfmt on module-root files.
- PR bodies need the `Gate:`, `Spec:`, `Proof:` and `Agent:` lines. `Proof:` must read `<tests> | <result>`. Titles are at most 85 characters. Each PR is squash-merged after admission passes.
- To republish a saved Psi fixture:
  - For the provider fixtures, set `OMEGA_PROVIDER_TEST_OUTPUT=<dir>` and run the provider_choices tests. They write `*.new.psi`.
  - For the Linux fixture, run the command in its README.
  - Run twice and compare the bytes.
- Never weaken the lowered-form checker or selection admission. Pair every positive test with a negative twin.
- Only Jarod and Zac edit `wiki/spec/`. Design and owner questions go to OWNER_QUESTIONS through the skill.

# Old-checker removal: handoff from worker-14 (second session, 2026-11-12/13)

## The removal is blocked at its first step: Psi 06 executes every Build program

Deleting Psi 06 stops every compilation, `omega --check` included, before the
lowered-form checker is called. Measured, not inferred:

- `omega --check tests/omega/pass/arithmetic/anonymous_integer_arith/main.omg`
  built from `0b1cec8cc37`: 61.6 s, reaches the route (which refuses this fixture
  with `not supported yet: machine no bound entry`).
- Same command built from the Psi 06 deletion branch: 0.49 s,
  `project source discovery failed: no product route yet: the checked program has
  no Terminal program producer`. The route never runs.

Why: Build activation lowers the checked Build program through Psi 06 into
Terminal Psi and runs it in the terminal interpreter:
`omega-rust/omega/build-system/build-settings/src/activation/preparation.rs`
(`PreparedBuildExecution::prepare`, the `TerminalProductionRequest::produce` call),
reached from `build_evaluation.rs:362` whenever no completed pure Build is
reusable. Build-time machine evaluation does the same at
`omega-rust/psi/compilation/build-time-evaluation/src/machine_execution.rs:5082`
and `:5457`. The first handoff's order ("Psi 06 is the leaf") missed this; its own
note "the Build stops here too" was the warning.

The owner's ruling accepts that programs stop reaching native output. It does not
say checking stops, and the coordinator's brief protects Build execution (the
`checked-interpreter` caveat). So the deletion was NOT merged.

## State

- **#2796, DRAFT, not merged** (`work/psi-delete-checked-tree-lowering`, head
  `9ce7b73e0d6`): the complete Psi 06 deletion, 130,032 lines deleted (129,570
  Rust), based on `0b1cec8cc37`. `mbx check --workspace --all-targets`,
  `mbx build -p omega`, fmt, check-boundary, check-rules all pass. It is ready to
  land the moment Build programs no longer need Psi 06, or if the owner rules that
  checking may stop too. What it does:
  - `psi-terminal-production`: `TerminalProductionRequest::produce`,
    `BuildProviderProductionRequest::produce`,
    `PreparedExternalProviderDeclarations::prepare` and
    `project_external_provider_declarations` refuse with `no product route yet`
    (`TerminalArtifactProductionError::NoProductRoute`, `NoProductRoute`).
  - Moved in as plain data: `TerminalMachineSelection`, `TerminalProgramRoots`,
    `ExternalProviderDeclarations`, `PreparedExternalProviderDeclarations`
    (new `terminal-production/src/external_providers.rs`).
  - Deleted with the stage: calling-signature capture, the Build companion
    captures, `lower_task_activation_demands`, compiler tests
    `terminal_{entry_writeback,behavior_exclusions,effect_readiness}.rs`,
    `tests/support/task_activation_controls.rs`, examples
    `host_reference_result.rs` and `terminal_loans.rs`, the Psi 06 ratchets in
    `tools/xtask/src/rules.rs` with `tools/stage06_*.txt`.
  - `advance` and `coordinator-review` skill wording, pipeline and README tables,
    two OWNER_CODE_CONCERNS.md entries.
- **#2801, MERGED** (merge `587f1f7265a`): terminal-installation move. Pure `git mv` to
  `omega-rust/psi/compilation/terminal-installation` plus path strings.

## What unblocks the rest, in order

1. **Build programs run without Psi 06.** Either the product hand-off from the
   lowered route's published module also carries Build programs (then
   `preparation.rs` and `machine_execution.rs:5082/5457` call it instead of
   `TerminalProductionRequest::produce`), or the owner rules that checking may stop.
   Then rebase and land #2796 (expect conflicts only in `Cargo.lock`, the
   `tools/*.txt` baselines, OWNER_CODE_CONCERNS.md).
2. **Psi 05** has the same Build dependency and two more:
   - Build admission checks the Build program with it:
     `build-settings/src/admission/checking.rs:323-338` (`CheckRequest::check()`),
     then builds `PreparedBuildMachineProgram::from_checked`.
   - `build-time-evaluation` checks const helper bodies with
     `typed_trees_to_checked_trees::lower_typed_trees` before evaluating them:
     `const_initializers.rs:137`, `const_evaluation.rs` (4 sites),
     `machine_execution.rs` (5 sites).
   - The compiler's check outcome carries the checked trees even for
     `--check`: `compiler/src/checked/checking/source_checking.rs:300-339`
     (`selections.select`, `CheckedProductSource { program: Arc<CheckedTrees>, .. }`),
     so `CheckRequest` cannot simply end at the route's verdict; the check-only
     report needs a product source without checked trees first.
   - `review-evidence` calls seven `derive_*` functions of Psi 05
     (`capture/input.rs`, `capture/behavior/policy/crash.rs`, and others).
3. **validation**, then **checked-trees / facts / representation-***, then
   **checked-interpreter**: unchanged from the first handoff. `representation-planning`
   still lives under `validation/`.

## Notes

- The slot's `target\debug\xtask.exe` was rebuilt this session; it matches main.
- `check-rules` baselines are exact, not ceilings: after deleting files run
  `xtask check-rules --update-dense-id-maps --update-place-transport
  --update-cache-state --update-source-reads`.
- `mbx build -p omega` after touching `psi-terminal-production`: 9 minutes warm.
  `mbx check --workspace --all-targets`: 2.5 minutes warm.
- Bash heredocs containing Rust with apostrophes fail in this tool; write the
  script with the Write tool and run it.

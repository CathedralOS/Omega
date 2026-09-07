# CLI MVP: work from a should-be-working example

The customer is this unchanged program: print two lines, read into `Main.pause`,
then exit 0. Success means a published native executable produces:

```text
Hello, Omega.
[press Enter to close]
```

Interactive execution accepts Enter. The existing automated sample test closes
stdin, so input reaches EOF; it checks exit 0 and the `Hello, Omega.` substring.
Those are different input cases, and both belong to completion. The current
first failure and next assignment live in [SAMPLE-CORPUS](../../../../TASKS.md).

## Use the real command as the outer loop

Run from the repository root. These commands build the shipped `omega` CLI too;
compiling or testing the `compiler` library alone does not establish that it builds.

Windows PowerShell:

```powershell
mbx run -p omega -- --target windows_x86_64 --build-dir build/cli-mvp-route samples/cli/basics/cli_mvp/main.omg
if ($LASTEXITCODE -ne 0) { throw 'cli_mvp compilation failed; do not run an old executable' }
& ./build/cli-mvp-route/omega-program.exe
if ($LASTEXITCODE -ne 0) { throw 'cli_mvp returned a nonzero exit' }
```

macOS ARM64 shell:

```sh
mbx run -p omega -- --target macos_arm64 --build-dir build/cli-mvp-route samples/cli/basics/cli_mvp/main.omg &&
  ./build/cli-mvp-route/omega-program
```

Observe both lines and press Enter. Use a fresh ignored build directory when
comparing revisions. Failed compilation is not permission to run a stale image.
Do not remove `read_line`, rewrite the writer as synthetic machines, or substitute
a special Console intrinsic for its checked source body.

The faster Windows compiler-library probe is:

```powershell
$env:OMEGA_SAMPLE_RUNTIME_FILTER='cli_mvp'
mbx nextest run -p compiler --test samples_compile --no-fail-fast -E 'test(=samples_with_documented_exit_run_correctly)'
Remove-Item Env:OMEGA_SAMPLE_RUNTIME_FILTER
```

On macOS use the same test command with the inline environment assignment
`OMEGA_SAMPLE_RUNTIME_FILTER=cli_mvp`. This test exercises compilation,
publication, output, and exit through the compiler library; it bypasses the CLI's
local-project package review route. Keep its result separate from the outer command.

## Trace the actual route

The Windows outer CLI probe completes fresh checked package review, then stops with
`fresh package review has blocking rows but no explicit --package-root-policy`.
This is a policy boundary before accepted native production. The compiler-library
sample test bypasses it and stops later at the missing Terminal writer body.
Neither result establishes native execution.

Inspect the exact blocking rows before choosing policy. The existing
[root-policy contract](../../../../wiki/design_briefs/build_and_package_model.md)
requires an explicit file bound to the current reconstructed conflicts; std has
no implicit authority. Do not supply blanket acceptance merely to advance the
example. The next assignment and tested revision remain on the execution board.

The existing focused review probe is
`mbx nextest run -p package-manager --test standard_library_package_resolution --no-fail-fast -E 'test(=real_standard_library_has_a_complete_ordinary_review_entry)'`.
It selects the Linux x64 source profile even on Windows, so it is an inner
comparison rather than Windows acceptance. The actual CLI command remains the
outer check through package review, policy, native production, and execution.

| Step | Owning code and required result |
| --- | --- |
| CLI and package closure | [`command.rs`](../../../../omega-rust/omega/src/command.rs) prepares `build.omg` projects through [`prepare_project.rs`](../../../../omega-rust/omega/packages/manager/src/operations/prepare_project.rs). [`compile_project.rs`](../../../../omega-rust/omega/packages/manager/src/operations/compile_project.rs) reviews the dependency closure and retains its checked root before native production. An error here precedes the sample's Terminal failure. |
| Source and selection | [`main.omg`](main.omg) imports ordinary std Console. [`build.omg`](build.omg) declares the std path dependency and binds each target's `ProgramEntry` to `Main::main`. The checked frontend resolves types, text/borrow/termination facts, and selected provider calls. |
| Checked writer body | [`std/console.omg`](../../../../source/library/std/console.omg) implements `ConsoleNativeProvider::write_line(text)` by calling `console_write_bytes(text, true)`. That helper is one five-state slice-ranked machine. Its `emit` state writes a byte and transfers the guarded tail; completion optionally emits newline and returns. |
| Complete callable closure | [`terminal_unit.rs`](../../../../omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/flow/terminal_unit.rs) builds ordinary/composed bodies and prunes callers whose targets are missing. [`call_closure.rs`](../../../../omega-rust/psi/pipeline/checked-trees-to-lowered-psi/src/attached_unit/call_closure.rs) requires every reached body before lowering. Preserve exact view/scalar state transfers, effect order, and slice-decrease evidence. |
| Portable execution | [`terminal-production`](../../../../omega-rust/psi/compiler/terminal-production/src/lib.rs) produces the canonical Terminal artifact with source-entry evidence. Codec replay, independent verification, and interpretation must agree on the writer's bytes and continuation. |
| Native operations | [`operation/routing.rs`](../../../../omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/routing.rs) must lower the writer's byte length, indexed read, and subslice. At the traced revision these operations explicitly reject. Descriptor layout alone is not operation support. |
| Entry storage and providers | [`native_artifact.rs`](../../../../omega-rust/omega/compiler/native-realization/src/realization/native_artifact.rs) rejects an executable entry retaining unprovisioned `self`. `Main` needs real storage, including its 256-byte buffer, and a loan from the entry bridge. [`compiler_intrinsic.rs`](../../../../omega-rust/omega/build/selected-dispatch/src/compiler_intrinsic.rs) must supply closed identities for the selected Windows output, input, and exit leaves; declarations alone are not native implementations. |
| Native image and publication | [`object.rs`](../../../../omega-rust/omega/compiler/native-realization/src/realization/object.rs) sequences physical lowering and emission. PE image support exists. [`compilation-report`](../../../../omega-rust/omega/compiler/compilation-report/src/lib.rs) validates the retained artifact and requires compiler-text/function evidence before publishing exact bytes. Preserve these gates. |

The observed compiler-library failure names `ConsoleNativeProvider::write_line`,
but its missing dependency is the five-state `console_write_bytes` body. The
ordinary planner handles one-state bodies, and the shared composed call catalog
now traverses acyclic free and attached graphs with unchanged borrowed views.
The shared route retains scalar declaration/assignment prefixes and computed
scalar successor operands, including guarded byte-head reads. The writer still
needs derived tail views and cyclic execution with retained slice ranking.
Caller pruning exposes that missing transitive body at the adapter.

The next writer acceptance is the actual authored closure through verified
Terminal execution: empty/nonempty bytes, both newline choices, exact output
order, and caller continuation. Unguarded head reads and unchanged tails reject.
The governing contract is [borrowed-byte writer composition](../../../../wiki/architecture/pipeline/terminal_psi.md#borrowed-byte-writer-composition).
Native byte operations, Windows leaf settlement, receiver provisioning, and
`read_line` capacity/live-length writeback remain downstream dependencies. These
are code-inspected gaps, not claims that this sample has reached each failure.

## Read the evidence at the boundary reached

Capture the command's exit and diagnostic first. Package preparation/review can
fail before checked phase reports are emitted. Once checked observation emission
runs, inspect `00_timings.html`, `05_machine_contracts.json`,
`05_capability_manifest.json`, and `05_executable_tcb_manifest.json` in the chosen
build directory. See [`checked_observations.rs`](../../../../omega-rust/omega/compiler/compiler/src/pipeline/reporting/checked_observations.rs)
and [`artifacts.rs`](../../../../omega-rust/omega/compiler/compiler/src/pipeline/artifacts.rs)
for the actual producers. Do not infer a passed stage from an old file or assume
every numbered report mentioned elsewhere is produced on this route.

After a change, report the old and new first failure under the same outer
command. An unchanged failure with a passing helper test is dependency progress;
it is not a working Hello World. No Windows observation establishes a macOS run.

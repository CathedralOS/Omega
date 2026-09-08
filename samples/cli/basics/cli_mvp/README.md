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
mbx run --release -p omega -- --target windows_x86_64 --build-dir build/cli-mvp-route samples/cli/basics/cli_mvp/main.omg
if ($LASTEXITCODE -ne 0) { throw 'cli_mvp compilation failed; do not run an old executable' }
& ./build/cli-mvp-route/omega-program.exe
if ($LASTEXITCODE -ne 0) { throw 'cli_mvp returned a nonzero exit' }
```

macOS ARM64 shell:

```sh
mbx run --release -p omega -- --target macos_arm64 --build-dir build/cli-mvp-route samples/cli/basics/cli_mvp/main.omg &&
  ./build/cli-mvp-route/omega-program
```

Observe both lines and press Enter. Use a fresh ignored build directory when
comparing revisions. Failed compilation is not permission to run a stale image.
Do not remove `read_line`, rewrite the writer as synthetic machines, or substitute
a special Console intrinsic for its checked source body.

Use the release compiler for customer-facing latency measurements. The first
invocation also builds the Rust compiler; measure subsequent direct invocations
of `target/release/omega` (`omega.exe` on Windows) separately from that build.
Cargo's `--release` optimizes the host compiler; it does not disable Omega's
semantic checking, package review, or acceptance requirements. Development-build
timings are useful for developer iteration but are not release performance.

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

The outer CLI checks current package requirements against the project's accepted
`omega.lock` policy. Missing acceptance or changed requirements stop before
native production with ordinary `omega update` guidance and compiler-rendered
findings. The [single package-acceptance rule](../../../../wiki/spec/packages/acceptance.md#authority-boundaries)
means an unchanged accepted policy needs no second native approval file.
The macOS release outer command reaches missing package acceptance. Remaining
package latency, current measurements, and the next performance investigation
belong to the owning task; Windows release timing has not been measured.

On Windows, std contributes `FilesystemHost` authority and four external Console
leaves: `read_line`, `read_byte`, `write_byte`, and `exit_process`. These are review
findings, not implicit grants. Do not supply blanket acceptance merely to advance
the example. Native proof, provider, and receiving-permission checks remain
independent requirements.

The compiler-library sample test bypasses project acceptance and stops later at
native ranked-module admission after Terminal production. Neither package
acceptance nor that failing probe establishes native execution. Current commands,
tested revisions, and the next implementation step remain on the execution board.

Use `omega audit packages --project samples/cli/basics/cli_mvp
--target macos_arm64 --details` to inspect the current macOS package findings;
select `windows_x86_64` for the Windows closure. This displays ordinary package
findings without changing acceptance. Use `omega update --project
samples/cli/basics/cli_mvp --target macos_arm64` to start ordinary review; edit any
required decisions and use `omega update --resume --project
samples/cli/basics/cli_mvp` to finish it. An audit report alone never accepts a package.

The existing focused review probe is
`mbx nextest run -p package-manager --test standard_library_package_resolution --no-fail-fast -E 'test(=real_standard_library_has_a_complete_ordinary_review_entry)'`.
It selects the Linux x64 source profile on every host. Its macOS pass is a source
review check, not native Linux execution or another target's acceptance. The
actual CLI command remains the outer check through package review, policy,
native production, and execution.

| Step | Owning code and required result |
| --- | --- |
| CLI and package closure | [`command.rs`](../../../../omega-rust/omega/src/command.rs) prepares `build.omg` projects through [`prepare_project.rs`](../../../../omega-rust/omega/packages/manager/src/operations/prepare_project.rs). [`compile_project.rs`](../../../../omega-rust/omega/packages/manager/src/operations/compile_project.rs) reviews the dependency closure and retains its checked root before native production. An error here precedes the sample's Terminal failure. |
| Source and selection | [`main.omg`](main.omg) imports ordinary std Console. [`build.omg`](build.omg) declares the std path dependency and binds each target's `ProgramEntry` to `Main::main`. The checked frontend resolves types, text/borrow/termination facts, and selected provider calls. |
| Checked writer body | [`std/console.omg`](../../../../source/library/std/console.omg) implements `ConsoleNativeProvider::write_line(text)` by calling `console_write_bytes(text, true)`. That helper is one five-state slice-ranked machine. Its `emit` state writes a byte and transfers the guarded tail; completion optionally emits newline and returns. |
| Complete callable closure | [`terminal_unit.rs`](../../../../omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/flow/terminal_unit.rs) builds ordinary/composed bodies and prunes callers whose targets are missing. [`call_closure.rs`](../../../../omega-rust/psi/pipeline/checked-trees-to-lowered-psi/src/attached_unit/call_closure.rs) requires every reached body before lowering. Preserve exact view/scalar state transfers, effect order, and slice-decrease evidence. |
| Portable execution | [`terminal-production`](../../../../omega-rust/psi/compiler/terminal-production/src/lib.rs) produces the canonical Terminal artifact with source-entry evidence. Codec replay, independent verification, and interpretation must agree on the writer's bytes and continuation. |
| Native operations | [`operation/routing.rs`](../../../../omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/lowering/machine/operation/routing.rs) retains byte length, proof-bearing indexed reads, and checked subslices. Scalar functions can observe derived views and forward whole descriptor parameters with runtime `u64` arguments through ordinary native calls. Literal descriptor creation, derived-view calls/block transfers, and Unit writer calls remain missing; layout alone is not operation support. |
| Entry storage and providers | [`native_artifact.rs`](../../../../omega-rust/omega/compiler/native-realization/src/realization/native_artifact.rs) rejects an executable entry retaining unprovisioned `self`. `Main` needs real storage, including its 256-byte buffer, and a loan from the entry bridge. [`compiler_intrinsic.rs`](../../../../omega-rust/omega/build/selected-dispatch/src/compiler_intrinsic.rs) must supply closed identities for the selected Windows output, input, and exit leaves; declarations alone are not native implementations. |
| Native image and publication | [`object.rs`](../../../../omega-rust/omega/compiler/native-realization/src/realization/object.rs) sequences physical lowering and emission. PE image support exists. [`compilation-report`](../../../../omega-rust/omega/compiler/compilation-report/src/lib.rs) validates the retained artifact and requires compiler-text/function evidence before publishing exact bytes. Preserve these gates. |

The observed compiler-library failure with production checkpoint `cbad71e423`
on macOS ARM64 is `Verification(Module(NonExecutableRankedScc(MachineId(3))))`.
Terminal transport retains the exact `pause` field and inline capacity when
presented to the boundary's mutable byte parameter. Native admission currently
routes every ranked module to the restricted unsigned-countdown implementation;
the writer needs natural slice-decrease control and native byte-view realization.
Do not replace its evidence with a countdown or silently fall back to ordinary
admission. Terminal production uses the validated
boundary-call source view; direct selected-adapter calls belong to interpreter
dispatch, while native adapter selection remains on the Omega side. Source
custody and structural validation must not be bypassed.

Measure the complete package route separately from the compiler-library probe.
Source checking repeats for preliminary and settled package inputs; permission
comparison follows those checks. Mutation summaries depend on immutable source
and borrow facts, so range-state passes and branch snapshots share their check's
existing table. Local bounds and invalidations still evolve independently.
No summary cache spans source revisions or substitutes for package acceptance.

The shared free/attached Unit graph retains scalar prefixes, guarded head/tail
operands, repeated descriptor bindings, and authored `Slice::Length` ranking.
A writer-shaped source regression passes serialized Terminal verification and
interpretation with raw bytes, both newline choices, caller continuation, and
fuel suspension. This does not establish that the unchanged std provider closure
or this native sample executes.

The next writer acceptance is the actual authored closure through verified
Terminal execution: empty/nonempty bytes, both newline choices, exact output
order, and caller continuation. Unguarded head reads and unchanged tails reject.
The producer's [writer composition notes](../../../../omega-rust/psi/compiler/terminal-production/README.md#borrowed-byte-writer-composition)
describe support and acceptance; the [byte-view specification](../../../../wiki/spec/terminal-psi/byte_views.md)
owns the portable view rules.
Native byte operations, Windows leaf settlement, receiver provisioning, and
`read_line` capacity/live-length writeback remain downstream dependencies. These
are code-inspected gaps, not claims that this sample has reached each failure.

The [native byte-observation regressions](../../../../tests/native-differential/tests/terminal_byte_views.rs)
execute encoded, verified Terminal scalar functions against empty, nonempty,
non-UTF-8, and rebound caller descriptors on macOS ARM64, and cross-lower
Linux x64/ARM64 and Windows x64. Runtime `u64` indices guarded by the exact
view's measured length read and widen the selected `u8`; empty and out-of-range
inputs take the non-reading branch. Missing, changed, or wrong-view bounds
evidence rejects. Checked nested subslices and framed helper calls also execute;
repeated whole-view calls preserve the caller's descriptor pointer and runtime
index across both invocations. These regressions start at Terminal, not source
helper lowering. Literal descriptor creation, derived-view calls/block transfers,
general Unit calls, and the ranked writer remain separate dependencies.

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

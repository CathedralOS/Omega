# CLI MVP

Print two lines, read into the existing `Main.pause` array, and exit 0:

```text
Hello, Omega.
[press Enter to close]
```

Ordinary package review, CLI compilation, and native execution pass on macOS
ARM64: EOF and Enter produce exactly those bytes (including both newlines),
empty stderr, and exit 0. Both lines appear before input; with stdin open the
program waits until Enter. Windows and Linux execution remain open in
[SAMPLE-CORPUS](../../../../TASKS.md). Cross-compilation is not host execution.

## Review the project

Run from the repository root, in the checkout where you intend to compile:

```text
mbx run --release -p omega -- update --project samples/cli/basics/cli_mvp --target macos_arm64
```

Select `windows_x86_64`, `linux_x86_64`, or `linux_arm64` for those hosts.
Use Cargo if `mbx` is unavailable. Exit 3 means decisions are pending, not that
acceptance was published. Inspect the reported
`build/package-manager/review-<target>.txt` and change each required decision
from `pending` to `accept` or `reject`, then resume:

```text
mbx run --release -p omega -- update --resume --project samples/cli/basics/cli_mvp
```

Resume must succeed before compilation. The observed fresh macOS review required
six decisions: the application's Console output, input, and termination
permissions; std's external byte-input declaration and intrinsic binding; and
std's public filesystem capability. Inspect the actual findings rather than
assuming that list is unchanged. The source-diff renderer can report incomplete
output; accepted decisions are not evidence of a complete source audit.

Local-source identities bind the checkout. Do not copy another worktree's lock
as reusable approval. The [package acceptance contract](../../../../wiki/spec/packages/acceptance.md)
requires fresh checking against the accepted project policy. Ordinary artifact
production needs no receiving-policy input and makes no receiver-admission claim.
Acceptance does not bypass proof, provider, or entry checks.

## Compile and execute

macOS ARM64 shell:

```sh
mbx run --release -p omega -- --target macos_arm64 --build-dir build/cli-mvp-route samples/cli/basics/cli_mvp/main.omg &&
  ./build/cli-mvp-route/omega-program
```

Windows PowerShell:

```powershell
mbx run --release -p omega -- --target windows_x86_64 --build-dir build/cli-mvp-route samples/cli/basics/cli_mvp/main.omg
if ($LASTEXITCODE -ne 0) { throw 'cli_mvp compilation failed; do not run an old executable' }
& ./build/cli-mvp-route/omega-program.exe
if ($LASTEXITCODE -ne 0) { throw 'cli_mvp returned a nonzero exit' }
```

On Linux use the shell command with its matching target. These are host routes,
not claims that the untested hosts pass. Confirm the CLI's reported publication
path before execution. Use a fresh ignored build directory when comparing
revisions, and never execute a stale image after failed compilation.

Observe both lines and press Enter. Executing the emitted image directly keeps
stdin available for this interactive check. EOF also completes the program.
The macOS validation used `RUST_MIN_STACK=67108864` for compiler invocations:
`export RUST_MIN_STACK=67108864` in a shell, or
`$env:RUST_MIN_STACK='67108864'` in PowerShell sets the same environment option.

Cargo's `--release` optimizes the host compiler, not Omega's checking policy.
Measure direct invocations of `target/release/omega` separately from the Rust
build. Add `--timings` to compilation for command-stage durations on stderr;
builds do not emit debug manifests.

## Focused regression and ownership

The existing compiler-library regression compiles once and runs EOF and Enter,
checking exact stdout, empty stderr, and exit 0:

```text
mbx nextest run -p compiler --test samples_compile --no-fail-fast --no-tests fail -E 'test(=cli_mvp_preserves_both_lines_with_eof_and_enter)'
```

It executes the report's checked executable path under test-owned package
acceptance. It does not perform the local project's CLI review or accept its
`omega.lock`; keep those checks separate.

[`main.omg`](main.omg) uses ordinary std Console and intrinsic
`Service<Console>` establishment. [`build.omg`](build.omg) binds each hosted
`ProgramEntry` to the same machine. Preserve the two writes, mutable pause view,
blocking line read, and exit; do not replace the library bodies with sample-only
compiler paths. [Bounded input](../../../../wiki/spec/resources/bounded_input.md)
owns byte preservation, destination bounds, and LF/EOF/Full behavior; this
pause-only sample intentionally discards the line result and does not exercise
that entire contract.

For a new failure, start with the actual CLI diagnostic. The
[Terminal production owner](../../../../omega-rust/psi/compiler/terminal-production/README.md)
documents source-to-Terminal composition; `ENTRY-CONTENT-ROOTS` owns entry and
service establishment, and `TWO-AXIS-TERMINAL-AUTHORITY-REVIEW` owns any remaining
production/admission coupling. Their current obligations live on the board,
not in a historical progress log here.

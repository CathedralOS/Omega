# Application

This directory (`application/`) is the Cargo package and binary `omega`, a
router. It parses a command, hands it to that command's one owner, and turns
the result into output and an exit status:

| Command | Owner |
| --- | --- |
| `omega <root.omg>` (build), `--check`, `omega psi inspect` | the compiler |
| `omega run` | the interpreter runtime |
| `omega packages install`, `omega packages update`, `omega packages audit` | the package manager |

Each owner calls what it needs; the router has no compile, run, build or
package workflow. See the
[destination](../../pipeline.md#destination).

[main.rs](src/main.rs) parses and dispatches one invocation.
[arguments/](src/arguments/mod.rs) owns argument spelling, and
[commands/](src/commands/mod.rs) owns console output and shell exit codes.
They call complete operations, never individual compiler stages.

The [compiler's Build coordinator](../build-system/orchestration/README.md) owns
project builds. This package also holds run and inspect:

| Operation | Request and result |
| --- | --- |
| [Build a project](../build-system/orchestration/src/build.rs) | Typed target/product request → ordered reports and published products |
| [Run a project](src/execution/mod.rs) | Execution/comparison options → report, captured process output, and interpreter comparison |
| [Inspect a machine](src/inspection/mod.rs) | Source, target, and machine → verified module and fixed-work analysis |

These functions return errors; they never print or exit. Each includes package
preparation, accepted policy, trust admission and artifact publication, so
another frontend need not rebuild that sequence. Install, update and package
inspection have complete operations in
[package-manager](../packages/package-manager/src/operations/mod.rs), with no
second wrapper.

A grant in the root's `build.omg` accepts that claim or provider plan; a
dependency's claims are accepted through `omega.lock`. There is no second
admissions file.

`run_project` creates a temporary directory per invocation. It deletes
disposable output on success and failure, and keeps requested output, including
from an interpreter disagreement. A native run with
only `--target` compiles without running the image. The Psi engine keeps the
target but runs through its admitted host adapters. Host
execution returns raw stdout/stderr bytes and process status; a program the
interpreter cannot run is a decline, not an agreement.

Comparisons accept `--both --psi-work-budget <positive-u64>`. Each interpreter
invocation, including each input case and stdin replay, gets the full budget
(default 10,000,000 logical work units). The flag needs
`--both` without `--target`, or `--engine psi`. Running out declines the
comparison and stops a Psi-only run with status 200. Native execution has
no work counter. Kept single-run artifacts and case outputs include
`psi.work.tsv` (case-prefixed in batch mode), recording the budget, units
charged and schedule identity.

`run --keep` keeps the published native image, its exact `program.oterm`,
`native.identity.txt`, and requested comparison streams. It does not
write a text dump of the native object's internal evidence graph.

An ordinary host run passes the caller's stdin through without first reading to
EOF. When a program compared with `--both` imports standard input, the command
reads stdin to EOF so the interpreter and native program replay the same bytes;
a program without a stdin import never reads it. Compare
stdin-reading programs through a finite pipe or file, and end interactive
input with EOF. The saved input is deleted even with `--keep`.
`--inputs <dir> --outputs <dir>` runs many cases against one compilation.
Every `*.stdin` file in the directory, in name order (or each repeated
`--input <file>`, in the given order and named by file stem), is fed to the
native program and, with `--both`, replayed through the interpreter before
publication. Each case writes `<case>.stdout` and `<case>.stderr`, plus
`<case>.terminal.stdout` and `<case>.terminal.stderr` when interpreted, and
`results.tsv` lists each case's native status, comparison verdict and
interpreter status. The command exits 201 when any case diverges or fails to
interpret, otherwise 0; the programs' own statuses are in `results.tsv`. Validations that compiled once per
input, or linked their own capture program, use this.
A run with `--target` does not read stdin. The command holds stdout/stderr until
native execution ends, then forwards the original bytes without UTF-8 decoding,
before its own diagnostics on stderr. If writing or flushing a stream fails, the
command reports an error (status 200), not a successful program result.

A comparison runs the shared [hosted Terminal runtime](../interpreter/terminal-runtime/README.md)
on the exact artifact the native build produced, with its retained entry and
provider provisions, before publication moves that payload. It never interprets
an older program, recompiles or reruns Build. The runtime gives the
program an isolated hosted root and buffered output. The standalone CLI
hands ProcessExit to that fresh private domain; an embedding request hands
over no exit by default. Entry Binding fields, reach and selected
providers grant no authority. The core ties each handover to the actual
instance and exact requirement, never the CLI host or another instance. Selected byte input reads the captured finite replay stream.

Time: exact Darwin clock imports and Linux clock and wait syscalls share one
modeled clock private to the invocation, started from a snapshot of host time;
checked library calibration and retry code still run. Timespec writes and
read-only wait requests use the original authenticated loans and their retained
native placement. This signal-free model is no evidence of native latency or
interruption correctness.

Streams: exact MSVCRT and Darwin descriptor imports and Linux read/write
syscalls share the finite input and separate private stdout and stderr.
Captured loans use retained native placement; Darwin also accepts the admitted
pointer-and-length byte-view ABI. Retained and discarded counts go through the
same transfer. An explicit count stays inside the original borrowed extent; a
pointer to one array element does not grant the rest of its allocation. An
unknown kernel descriptor or a replay failure cannot produce an invented kernel
status. A large transfer is limited to the selected kernel page size, not an
impossible full-count write.

Descriptor-backed or unbounded waits, CPU clocks and other missing imports
decline; there is no tree fallback. The CLI compares only the
termination the host presents, not stdout/stderr or full semantic traces. A
typed `Trap` is compared with the executing host's architectural trap
presentation, never with an arbitrary signal death or a numeric exit code.
Other unmapped crashes and running out of the sponsor's work budget decline
explicitly. A Windows exception status can share its bits with an authored exit
code, so matching presentation does not prove the same cause.

The library is synchronous and has the compiler's stack requirements. The CLI
sets up its compiler worker stack; embedding callers must provide the same
stack size. This is not a scheduler, command bus
or new pipeline protocol. [Compiler orchestration](../compilation/compiler/src/target_batch.rs)
still passes representations through ordinary functions.

## Command surface

The [command-line contract](../../../wiki/spec/build/command_line.md) owns
ratified subsystem routing and public **Psi** naming; the spellings below follow
it. Flat `install`/`update`, action-first `audit`, and
`inspect-terminal` are rejected, not aliases.
`omega source rewrite` is specified but not implemented, so the parser rejects
it. Kept file names such as
`program.oterm` are internal artifact names and do not change.

The CLI package is **`omega`** (binary `omega`); there is no `omega-cli`.

The [standalone engine contract](../../../wiki/spec/build/embedding.md#standalone-execution-selection)
chooses the engine with `run --engine native|psi`, defaulting to native.
A Psi-only run uses the same admitted runtime as a comparison, without emitting
or launching a native image. `--both` is a separate comparison request and
cannot be combined with `--engine`. Target and provider choice are
independent; a missing interpreter adapter never falls back to native.
`UNIVERSAL-INTERPRETER-CLOSURE` in
[TASKS.md](../../../TASKS.md#universal-interpreter-cutover) tracks the remaining
implementation.

```bash
mbx run -p omega -- packages update --project samples/cli/basics/cli_mvp --target macos_arm64
mbx run -p omega -- --check samples/cli/basics/cli_mvp/main.omg
```

A hosted sample gets its target entry contract from std, an ordinary
dependency, so its first check or run needs the accepted program-entry
binding that `omega packages update` reviews (accept the pending decisions, then
rerun with `--resume`). Without it, the entry is rejected with a message naming
that review.

`--entry-ram` builds fresh private initialized scalar-record RAM for one exact borrowed placed entry position. It grants no host address or external allocation. The physical lender works on Linux x86-64 and macOS ARM64 and refuses unsupported predicates, qualifications, compound storage and cleanup. With `--both --keep`, observations of the original backing and the actual interpreter effect receipts are kept separately from the program's streams; see the [placed entry RAM sample](../../../samples/cli/probes/placed_entry_ram/README.md).

Startup and dispatch live in `src/main.rs`; typed invocations are parsed in
`src/arguments/` and run in `src/commands/`. Full surface:

```text
omega [--check] [--offline] [--timings]
      [--report-file <path>]
      [--build-input <path>]... [--optional-build-input <path>]...
      [--build-dir <dir>] [--target <name>]... [--disable-optimization <ExactName>]... <root.omg>
omega run [--engine <native|psi> | --both] [--psi-work-budget <positive-u64>] [--keep] [--entry-ram <position>=<hex-bytes>]... [--target <name> | (--inputs <dir> | --input <file>...) --outputs <dir>] <root.omg>
omega psi inspect --machine <qualified> [--target <name>] <root.omg>
omega source rewrite <ExactRule>... [--check] <root.omg>
omega packages audit --kind <local|git> <locator> [--rev <rev>]
omega packages audit [--project <dir>] [--target <name>]... [--details] [--offline] [--build-input <path>]... [--optional-build-input <path>]...
omega packages install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]... [--dangerously-accept-all]
omega packages update [package-or-alias...] [--to <revision>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]... [--dangerously-accept-all]
omega packages <install|update> <--resume|--discard-review> [--project <dir>] [--offline]
```

`--dangerously-accept-all` accepts every pending review decision unread,
resumes and publishes. `omega.lock` records each acceptance exactly
as a hand edit of the review document would. Automatic resume uses the saved
proposal's targets and build inputs, not new overrides.

`--offline` limits package acquisition to local sources and cached, recorded
Git pins. It does not refresh selectors or sandbox later program execution.
`run` and `psi inspect` do not accept this flag.

`--target` names a target and may repeat; duplicates are removed. Each named
target is compiled and published separately, into
`<build dir>/<target>/` when more than one is named, with one result per target.
The settled design compiles every provided target when none is named, and
checks every target's bodies either way
([multi-target compilation](../../../wiki/spec/build/configuration.md#multi-target-compilation)).
Today a missing `--target` checks target-neutrally and compiles for the host;
the pipeline route items on the task board close that gap.

The compile command's exit status is the
[compiler request](../../../wiki/spec/build/compiler_request.md#validation-and-failures)
halt value: 0 when the requested product is complete, 1 for a rejection, and 2
(`Incomplete`) when compilation stopped at a named private provision, such as
a proof-search limit, with no verdict on the source. With several targets it
exits 2 only when every refused target is incomplete; any rejection exits 1.
Argument errors exit 64 (`EX_USAGE`) before compilation starts. `omega run`
has its own statuses: the program's exit status; 200 when compilation or launch
fails; 202 when compilation stopped at a named private provision
(`Incomplete`); and 201 when the `--both` interpreter run disagrees or hits a
runtime or artifact defect; and 204 when `--both` could not compare the two
runs. That covers a Psi run that ran out of work, waited on the host, returned
no process result or hit an unsupported construct, and a Psi exit status
outside 0..=255 that a Unix host shows only as its low byte. Such a run reports
`DECLINED`; it never reports agreement or reuses the native status. Case runs
exit 201 if any case diverged, else 204 if any case was declined.

`--engine psi` exits with the interpreted program's own status; 203 when the
program crashes (the cause is reported, since a process cannot present the
native trap signal without raising it); and 200 otherwise: `UNAVAILABLE` when a
provider, host adapter or interpreter capability is missing and refused before
the program's first instruction, `FAILED` for an execution error after it (its
buffered output is lost), or execution that stops without a process result.
The Psi engine requests the ordinary native product, which passes the same
Build, package, trust, provider and entry admission, then stops before
emitting an object or image. `--keep` keeps `program.oterm` and the
interpreter's streams, work and effect receipts, never a native image. Case runs
(`--inputs`/`--input`) do not yet accept `--engine psi`. A program that binds
separately produced components is refused during compilation on this engine
(`Psi compile FAILED`, status 200), since those components only run as
native images. Streams are forwarded afterwards, not live.

Compilation emits requested products and diagnostics, not debug dumps.
`--timings` prints command-stage durations and total elapsed time to stderr;
otherwise no timings are collected.
`--report-file <path>` writes the compile report (product, target, summary and
publication lines) to a plain-text file; there is no `--output-only` switch.
The requested product decides the required proof and installation records. See
the
[compiler product contract](../compilation/compiler/README.md#product-boundaries-and-observations).

`omega --version` reports the compiler version, source commit (when the
packager supplies it) and Rust host target. Release bundles carry their
library beside the executable; Cargo development builds fall back to the
checkout. See [release packaging](../../../tools/release/README.md#build-a-relocatable-bundle)
for installation layout, explicit library selection and relocation tests.

`omega psi inspect --artifact <file>` reads an existing portable Psi artifact
file (up to 64 MiB), decodes it canonically, validates its meaning, and prints
its module and placed-input list. It takes no source, machine or target flags
and neither prepares packages nor executes. An invalid or noncanonical
artifact returns status 1; inspection grants no runtime authority.

Inspecting from source also accepts `--output <file>` to save the canonical
artifact for later inspection. The caller chooses that host path; artifact
inspection cannot write an output file.

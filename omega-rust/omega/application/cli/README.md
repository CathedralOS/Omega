# Omega application

[main.rs](src/main.rs) parses and dispatches one invocation.
[arguments/](src/arguments/mod.rs) owns argument spelling, and
[commands/](src/commands/mod.rs) owns console rendering and shell exit codes.
It calls complete operations, not individual compiler stages.

Project Build is owned by the [compiler's Build coordinator](../../build-system/orchestration/README.md).
The CLI package also houses the execution and inspection operations:

| Operation | Request and result |
| --- | --- |
| [Build a project](../../build-system/orchestration/src/build.rs) | Typed target/product request → ordered reports and published products |
| [Run a project](src/execution/mod.rs) | Execution/comparison options → report, captured process output, and interpreter comparison |
| [Inspect a machine](src/inspection/mod.rs) | Source, target, and machine → verified module and fixed-work analysis |

These functions return errors instead of printing or terminating the caller.
Package preparation, accepted policy, trust admission, and artifact publication
remain part of the operation; another frontend need not reconstruct that sequence.
Install/update and package inspection already have their own complete operations
in [package-manager](../../packages/package-manager/src/operations/mod.rs), with no second wrapper.

A root grant in `build.omg` is the root's acceptance of that claim or provider
plan; dependency claims are accepted through `omega.lock`. There is no second
admissions file.

`run_project` owns a distinct temporary directory per invocation. Disposable
output is cleaned up on success and failure; requested retained output survives,
including interpreter disagreement. Native target-only invocations compile without
running the image. Terminal engine selection retains the target but executes
through its admitted host adapters. Host execution returns raw stdout/stderr bytes and process status;
unsupported interpretation is a decline, not an agreement.

Host comparisons accept `--both --psi-work-budget <positive-u64>`.
The budget supplies each Terminal invocation independently, including each
input case and stdin replay; it defaults to 10,000,000 logical work units.
It requires `--both` without `--target`, or `--engine psi`. Exhaustion remains
a declined comparison and stops a Terminal-only run with status 200.
Native execution receives no work counter. Kept single-run artifacts and case
outputs include `psi.work.tsv` (case-prefixed in batch mode), recording the
supplied budget, actual charged units and schedule identity.

`run --keep` retains the published native image, its exact `program.oterm`,
`native.identity.txt`, and comparison streams when requested. It does not
pretty-print the native object's internal evidence graph into a text dump.

Ordinary host runs inherit the caller's stdin without waiting for EOF upfront.
When a compared program (`--both`) demands a standard-input host import,
comparison snapshots stdin to EOF for independent Terminal and native replay;
programs without stdin imports run without reading the caller's stdin. Use a
finite pipe or redirected file when comparing stdin-reading programs;
interactive input must be ended with EOF. Temporary input storage is deleted
even with `--keep`.
`--inputs <dir> --outputs <dir>` runs many cases against one compilation:
every `*.stdin` file in the input directory, in name order (or each
repeated `--input <file>`, in the given order, named by file stem), is fed to the
native program and, with `--both`, replayed through the Terminal reference
before publication. Each case writes `<case>.stdout` and `<case>.stderr`, plus
`<case>.terminal.stdout` and `<case>.terminal.stderr` when interpreted, and
`results.tsv` lists every case's native status, comparison verdict and
interpreter status. The command exits 201 when any case diverges or fails
interpretation, otherwise 0; program statuses are in `results.tsv`. A
validation that previously compiled once per input, or linked its own
Terminal capture program, uses this instead.
Target-only invocations do not read stdin. Stdout/stderr remain captured until
native execution ends. The command forwards those original bytes without
UTF-8 decoding, before its own diagnostics on stderr. A failed stream write or
flush reports a command error (status 200), not a successful program result.

Comparison runs the shared [hosted Terminal runtime](../../interpreter/terminal-runtime/README.md)
on the native report's exact produced
artifact and retained entry/provider provisions before publication transfers that
payload. It never interprets checked trees, recompiles source or repeats Build.
The current runtime establishes an isolated hosted root and buffered output.
The standalone CLI explicitly delegates ProcessExit to that fresh private domain;
the embedding runtime request defaults to no exit delegation. Entry Binding
fields, reach and selected providers confer no authority. The core seals each
delegation to the actual instance and exact requirement, never the CLI host or
another instance. Selected byte input uses the captured finite replay
stream. Exact Darwin clock imports and Linux clock/wait syscalls share
invocation-private modeled time seeded from a host epoch snapshot; checked
library calibration and retry code still run. Timespec writes and read-only
wait requests use authenticated original loans and retained native placement.
This signal-free time model is not evidence of native latency or interruption
correctness.
Exact MSVCRT/Darwin descriptor imports and Linux read/write syscalls share finite
input and separate private stdout/stderr. Captured loans use retained native
placement; Darwin also accepts the admitted pointer/length byte-view ABI.
Retained or discarded counts use the same transfer. Explicit counts stay within
the original borrowed extent; a pointer to one array element does not grant its
allocation's tail. Unknown kernel descriptors or replay failures cannot supply
an invented kernel status. Large transfers need the selected kernel page-size
limit, rather than an impossible full-count write.
Descriptor-backed or
indefinite waits, CPU clocks and other missing imports decline explicitly;
there is no tree fallback. The CLI compares host-presented termination only, not
stdout/stderr or full semantic traces. Typed `Trap` compares against the executing
host's architectural trap presentation, never against arbitrary signal death or
a numeric exit fallback. Other unmapped crashes and sponsor-work-budget exhaustion
decline explicitly. Windows exception status can share its bits with an authored
exit code; presentation agreement does not prove the underlying cause.

The library is synchronous and inherits the compiler's current stack requirements.
The CLI provisions its existing compiler worker stack; embedded callers must
provide equivalent stack capacity where needed. This is not a scheduler, command
bus, or new pipeline protocol. [Compiler orchestration](../../compilation/compiler/src/target_batch.rs)
continues to pass representations through ordinary functions.

## Command surface

The [command-line contract](../../../../wiki/spec/build/command_line.md) owns
ratified subsystem routing and public **Psi** naming. The spellings below follow
that contract. Flat `install`/`update`, action-first `audit`, and
`inspect-terminal` are rejected; they are not aliases.
`omega source rewrite` is specified and not implemented, so the parser rejects
it instead of accepting a rewrite it cannot perform. Retained filenames such as
`program.oterm` are internal artifact names and stay as they are.

The CLI package is **`omega`** (binary `omega`); there is no `omega-cli`
package.

The [standalone engine contract](../../../../wiki/spec/build/embedding.md#standalone-execution-selection)
selects execution with `run --engine native|psi`, defaulting to native.
Terminal-only execution uses the same admitted Terminal runtime as comparison,
without emitting or launching a native image. `--both` remains an explicit
comparison and cannot be combined with `--engine`. Target/provider selection
remains independent; missing interpreter adapters never trigger native fallback.
The implementation work is tracked by `UNIVERSAL-INTERPRETER-CLOSURE` in
[TASKS.md](../../../../TASKS.md#universal-interpreter-cutover).

```bash
mbx run -p omega -- packages update --project samples/cli/basics/cli_mvp --target macos_arm64
mbx run -p omega -- --check samples/cli/basics/cli_mvp/main.omg
```

A hosted sample takes its target entry contract from std, an ordinary
dependency, so its first check or run needs the accepted program-entry binding
that `omega packages update` reviews (accept the pending decisions, then rerun with
`--resume`). Without it the entry slot rejects and names that review.

`--entry-ram` explicitly constructs fresh private initialized scalar-record RAM for an exact borrowed placed entry position. It grants no host address or external allocation. The physical lender supports Linux x86-64 and macOS ARM64 and refuses unsupported predicates, qualifications, compound storage and cleanup. With `--both --keep`, original backing observations and actual Terminal effect receipts are retained separately from guest streams; see [placed entry RAM sample](../../../../samples/cli/probes/placed_entry_ram/README.md).

CLI startup and dispatch live in `src/main.rs`; typed invocations
are parsed in `src/arguments/` and executed in `src/commands/`. Full surface:

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

`--dangerously-accept-all` accepts every pending review decision without
reading it, resumes, and publishes. `omega.lock` records each acceptance exactly
as a hand edit of the review document would. Automatic resume uses the saved
proposal's target and build-input inventory, not new invocation overrides.

`--offline` restricts package acquisition to local sources and cached recorded
Git pins. It does not refresh selectors or sandbox later program execution.
`run` and `psi inspect` do not support this flag.

`--target` names a realization target and may repeat; repeats are removed
and each named target is compiled and published on its own, into
`<build dir>/<target>/` when more than one is named, with one outcome per
target. The settled model realizes every provided target when none is named
and checks every target's bodies either way
([multi-target compilation](../../../../wiki/spec/build/configuration.md#multi-target-compilation));
today an absent `--target` checks target-neutrally and realizes the host, and
the board's pipeline route items close that gap.

The compile command's exit status is the
[compiler request](../../../../wiki/spec/build/compiler_request.md#validation-and-failures)
halt value: 0 when the requested product is complete, 1 for a rejection, and 2
(`Incomplete`) when compilation stopped at a named private provision — such as
a proof-search capacity — without a verdict on the source. Several targets exit
2 only when every refused target is incomplete; any rejection exits 1.
Argument errors exit 64 (`EX_USAGE`) before any compilation starts, so they
never read as a compile outcome. `omega run`
keeps its own statuses: the program's exit status, 200 when compilation or
launch fails, 202 when compilation stopped at a named private provision
(`Incomplete`), and 201 when `--both` interpretation disagrees or encounters a
runtime/artifact defect. An explicitly unavailable comparison reports `DECLINED`
and retains the native program's status; it never reports agreement.
`--engine psi` exits with the Terminal program's own status, 203 when the
Terminal program crashes (the cause is reported; a process cannot present the
native trap signal without raising it), and 200 otherwise: `UNAVAILABLE` for a
missing provider, host adapter or interpreter capability refused before the
guest's first instruction, `FAILED` for an execution error after it (its
buffered output is lost), or execution that stops without a process result.
The Terminal engine requests the ordinary native product, which passes the same
Build, package, trust, provider and entry admission and then stops before any
object or image is emitted; `--keep` retains `program.oterm` and the Terminal
streams, work and effect receipts, never a native image. Case runs
(`--inputs`/`--input`) do not yet accept `--engine psi`. A program that
binds independently produced components is refused during compilation on this
engine (`terminal compile FAILED`, status 200), since those components execute
only as native images. Program streams are forwarded after the Terminal program
finishes, not interleaved live.

Compilation emits requested products and diagnostics, not debug dumps.
`--timings` prints command-stage durations and total elapsed time to stderr;
normal invocations do not collect optional timing measurements.
`--report-file <path>` writes the produced compile report (product, target,
summary, and publication lines) as a plain-text observation file; there is no
`--output-only` switch. Required proof and installation
records remain governed by the requested product. See the
[compiler product contract](../../compilation/compiler/README.md#product-boundaries-and-observations).

`omega --version` reports the compiler version, source commit (when supplied by
the packager), and Rust host target. Release bundles carry their matching library
beside the executable; ordinary Cargo development builds retain a checkout
fallback. See [release packaging](../../../../tools/release/README.md#build-a-relocatable-bundle)
for installation layout, explicit library selection, and relocation tests.

`omega psi inspect --artifact <file>` reads an existing portable Terminal
artifact envelope (up to 64 MiB), applies canonical decoding and semantic
validation, and prints its module and placed-input roster. It accepts no source,
machine or target flags and performs no package preparation or execution. Invalid
or noncanonical artifacts return status 1; inspection supplies no runtime authority.

The source form also accepts `--output <file>` to preserve its canonical
artifact envelope for later inspection. The destination is a caller-supplied
host path; artifact inspection cannot request an output file.

# Omega application

[main.rs](src/main.rs) parses and dispatches one invocation. The
[CLI](src/cli/mod.rs) owns argument spelling, console rendering, and shell exit
codes. It calls complete operations, not individual compiler stages.

The same package also exposes a [library](src/lib.rs):

| Operation | Request and result |
| --- | --- |
| [Compile a project](src/compilation/mod.rs) | Product and policy options → compile report and optional published executable |
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
including interpreter disagreement. An explicit target compiles without running
the image. Host execution returns raw stdout/stderr bytes and process status;
unsupported interpretation is a decline, not an agreement.

Ordinary host runs inherit the caller's stdin without waiting for EOF upfront.
Interpreter comparison (`--both`) instead snapshots stdin to EOF before compilation
for independent native and Terminal replay. Use a finite pipe or redirected file
for comparison; interactive input must be ended with EOF. Temporary input storage
is deleted even with `--keep`.
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

Comparison runs the shared Terminal runtime on the native report's exact produced
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
a numeric exit fallback. Other unmapped crashes and sponsor-fuel exhaustion
decline explicitly. Windows exception status can share its bits with an authored
exit code; presentation agreement does not prove the underlying cause.

The library is synchronous and inherits the compiler's current stack requirements.
The CLI provisions its existing compiler worker stack; embedded callers must
provide equivalent stack capacity where needed. This is not a scheduler, command
bus, or new pipeline protocol. [Compiler orchestration](../../compilation/driver/src/compiler.rs)
continues to pass representations through ordinary functions.

## Command surface

The CLI package is **`omega`** (binary `omega`); there is no `omega-cli`
package.

```bash
mbx run -p omega -- update --project samples/cli/basics/cli_mvp --target macos_arm64
mbx run -p omega -- --check samples/cli/basics/cli_mvp/main.omg
```

A hosted sample takes its target entry contract from std, an ordinary
dependency, so its first check or run needs the accepted program-entry binding
that `omega update` reviews (accept the pending decisions, then rerun with
`--resume`). Without it the entry slot rejects and names that review.

`--entry-ram` explicitly constructs fresh private initialized scalar-record RAM for an exact borrowed placed entry position. It grants no host address or external allocation. The initial physical lender supports Linux x86-64 and refuses unsupported predicates, qualifications, compound storage and cleanup. With `--both --keep`, original backing observations and actual Terminal effect receipts are retained separately from guest streams; see [placed entry RAM sample](../../../../samples/cli/systems/placed_entry_ram/README.md).

CLI startup and dispatch live in `src/main.rs`; typed invocations
are parsed in `src/cli/arguments/`. Full surface:

```text
omega [--check] [--offline] [--timings]
      [--report-file <path>]
      [--build-input <path>]... [--optional-build-input <path>]...
      [--build-dir <dir>] [--target <name>]... [--disable-optimization <ExactName>]... <root.omg>
omega run [--both] [--keep] [--entry-ram <position>=<hex-bytes>]... [--target <name> | (--inputs <dir> | --input <file>...) --outputs <dir>] <root.omg>
omega inspect-terminal --machine <qualified> [--target <name>] <root.omg>
omega audit source --kind <local|git> <locator> [--rev <rev>]
omega audit packages [--project <dir>] [--target <name>]... [--details] [--offline] [--build-input <path>]... [--optional-build-input <path>]...
omega install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]... [--dangerously-accept-all]
omega update [package-or-alias...] [--to <revision>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]... [--dangerously-accept-all]
omega <install|update> <--resume|--discard-review> [--project <dir>] [--offline]
```

`--dangerously-accept-all` accepts every pending review decision without
reading it, resumes, and publishes. `omega.lock` records each acceptance exactly
as a hand edit of the review document would. Automatic resume uses the saved
proposal's target and build-input inventory, not new invocation overrides.

`--offline` restricts package acquisition to local sources and cached recorded
Git pins. It does not refresh selectors or sandbox later program execution.
`run` and `inspect-terminal` do not support this flag.

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

Compilation emits requested products and diagnostics, not debug dumps.
`--timings` prints command-stage durations and total elapsed time to stderr;
normal invocations do not collect optional timing measurements.
`--report-file <path>` writes the produced compile report (product, target,
summary, and publication lines) as a plain-text observation file; there is no
`--output-only` switch. Required proof and installation
records remain governed by the requested product. See the
[compiler product contract](../../compilation/driver/README.md#product-boundaries-and-observations).

`omega --version` reports the compiler version, source commit (when supplied by
the packager), and Rust host target. Release bundles carry their matching library
beside the executable; ordinary Cargo development builds retain a checkout
fallback. See [release packaging](../../../../tools/release/README.md#build-a-relocatable-bundle)
for installation layout, explicit library selection, and relocation tests.

`omega inspect-terminal --artifact <file>` reads an existing portable Terminal
artifact envelope (up to 64 MiB), applies canonical decoding and semantic
validation, and prints its module and placed-input roster. It accepts no source,
machine or target flags and performs no package preparation or execution. Invalid
or noncanonical artifacts return status 1; inspection supplies no runtime authority.

The source form also accepts `--output <file>` to preserve its canonical
artifact envelope for later inspection. The destination is a caller-supplied
host path; artifact inspection cannot request an output file.

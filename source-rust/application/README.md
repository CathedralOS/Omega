# Application

This directory (`application/`) is the Cargo package and binary `omega`, a
router. It parses a command, hands it to that command's one owner, and turns
the result into output and an exit status:

| Command | Owner |
| --- | --- |
| `omega <root.omg>` (build), `--check`, `omega psi inspect` | the compiler |
| `omega run` | the interpreter runtime |
| `omega source parse` | the [source subsystem](../subsystems/source/lib.rs) |
| `omega packages install`, `omega packages update`, `omega packages audit` | the package manager |

[main.rs](src/main.rs) dispatches an invocation. [Arguments](src/arguments/mod.rs) owns command spelling. [Commands](src/commands/mod.rs) presents results and sets the shell exit status.

For source inputs, the CLI asks the package manager to prepare the project and resolve the dependencies its Build requests. Project planning opens the Product and hands its compiler request to the source pipeline. The saved-Psi reader decodes and authorizes an existing artifact for the native or interpreter entry. [Project preparation](../subsystems/package-manager/project/mod.rs) · [Build planning](../subsystems/compiler/build/project_build.rs) · [Saved Psi](src/saved_program.rs)

| Operation | Request and result |
| --- | --- |
| [Compile](src/commands/compile.rs) | Product and targets to diagnostics, ordered reports and published files |
| [Run](src/execution/mod.rs) | Run options to interpreted Psi termination and output |
| [Inspect](src/inspection/mod.rs) | Source and machine, or a saved artifact, to a Psi inspection report |
| [Package operations](src/commands/packages.rs) | Package selection and policy decisions to package-manager operations |

A grant in the root's `build.omg` accepts that claim or provider plan; a
dependency's claims are accepted through `omega.lock`. There is no second
admissions file.

`omega run app.omg` lowers the admitted source program to Psi and interprets it.
`--target` selects the target environment for interpretation. A run uses a
fresh temporary directory. `--keep` retains its Psi artifacts. To produce
a native executable, compile with `omega app.omg` and launch the published
image on a matching host.

`omega build --compare-tests build.omg` executes each enabled, registered test
on both engines. The installed adapter supplies arguments and may check
returned fields in ordinary Omega. Each invocation receives its own configured
stdin and private filesystem. Build checks termination, stdout, stderr and
declared file outcomes. Matching traps or matching incorrect output fail the
test. A project with no selected tests reports zero comparisons.
[Build test comparison](../../wiki/spec/build/testing.md#comparing-native-and-psi-tests)

For repository samples with application entry points, run
`mbx xtask sample-gate --filter samples/cli/basics/cli_mvp`. To inspect an
individual interpreted program, use `omega run --outputs <new-dir> app.omg`.
The directory contains `termination.tsv`, guest stdout and stderr, and any
requested private entry RAM.

Each registered test receives 4 MiB for stdout, 4 MiB for stderr and 8 MiB
in total per engine. Native runs have 60 seconds for execution and cleanup.
When a test exhausts its work, time or output allowance, Build reports the
invocation incomplete and runs the remaining selected tests.

A registered test that calls `ProcessExit::exit_process` fails its requirement
to return. On Unix the native wait status exposes only the low eight bits. A
different visible byte proves disagreement. A matching byte leaves the exact
exit comparison unavailable. A zero status can also come from a normal return
when the test has both paths.

`run --psi-work-budget <positive-u64>` supplies the full budget
to each invocation (default 10,000,000 logical work units). Exhaustion exits
200. Kept Psi artifacts and case outputs include `psi.work.tsv` (case-prefixed
in batch mode), recording the budget, units charged and schedule identity.

For a single invocation, `omega run` reads guest input from stdin.

`--inputs <dir> --outputs <dir>` runs many cases against one compilation.
Every `*.stdin` file in the directory, in name order (or each repeated
`--input <file>`, in the given order and named by file stem), is fed to the
interpreter. `--target` selects the target environment for every case. Each
input has its own invocation and work allowance. A batch creates a fresh output directory, writes the guest bytes to `<case>.stdout` and `<case>.stderr`, retains
`<case>.psi.work.tsv`, and records the termination and its detail in
`results.tsv`. A recorded return, exit or crash completes that case. An
incomplete, unavailable or failed execution makes the batch exit 200. Case
names must be distinct file stems and fit one TSV field.

The single-run command forwards buffered guest bytes before its diagnostics.

Psi executes the selected program through the shared [hosted Terminal
runtime](../subsystems/interpreter/host/README.md). The runtime gives the
program an isolated hosted root and buffered output. The standalone CLI
hands ProcessExit to that fresh private domain; an embedding request hands
over no exit by default. The core ties each handover to the actual instance
and exact requirement.

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
can prevent execution. The Psi run reports the unavailable capability or
incomplete result.

The library is synchronous and has the compiler's stack requirements. The CLI
sets up its compiler worker stack; embedding callers must provide the same
stack size. This is not a scheduler, command bus
or new pipeline protocol. [The compiler](../subsystems/compiler/compiler.rs)
still passes representations through ordinary functions.

## Command surface

The [command-line contract](../../wiki/spec/build/command_line.md) owns
ratified subsystem routing and public **Psi** naming; the spellings below follow
it. Flat `install`/`update`, action-first `audit`, and
`inspect-terminal` are rejected, not aliases.
The CLI passes the named path to the source subsystem. Its parse action checks the file with the compiler’s lexer and parser. Imported files may be absent.
`omega source rewrite` is specified but not implemented, so the parser rejects
it. Kept file names such as
`program.oterm` are internal artifact names and do not change.

The CLI package is **`omega`** (binary `omega`); there is no `omega-cli`.

The standalone interpreter uses the admitted runtime for the selected target
and providers. A missing adapter refuses execution.

**Implementation status.** The [standalone contract](../../wiki/spec/build/embedding.md#standalone-interpretation)
specifies bare `omega run app.psi` and confines both source and saved Products
to the caller’s working directory by default. Without `--trusted-artifact`,
the Rust CLI interprets saved Psi contained. It gets the host's reviewed
package bindings, standard streams and the working directory read/write at
`/`; explicit run directory mappings replace that mount. It gets no native
provider, network, program launch or native compilation.
<<<<<<< HEAD
`--trusted-artifact <manifest-id>` runs saved Psi on the trusted route. A
source or trusted run of a Linux program on its matching Linux host, with no
native provider, gets the same working-directory default. Any other run with
no explicit mapping keeps the host filesystem namespace and matching-host
adapters; that case is tracked in [PSI-UNTRUSTED-INTERPRETATION](../../TASKS.md#psi-untrusted-interpretation).
=======
`--trusted-artifact <manifest-id>` runs saved Psi on the trusted route. For a
trusted run with no explicit run directory mappings, the CLI supplies the host
filesystem namespace and enables matching-host adapters for admitted
providers. Giving source runs the working-directory default is tracked in [PSI-UNTRUSTED-INTERPRETATION](../../TASKS.md#psi-untrusted-interpretation).
>>>>>>> b84beb8852d955865ae5e2209793554e8aa7eedc

```bash
mbx run -p omega -- packages update --project samples/cli/basics/cli_mvp --target macos_arm64
mbx run -p omega -- --check samples/cli/basics/cli_mvp/main.omg
```

A hosted sample gets its target entry contract from std, an ordinary
dependency, so its first check or run needs the accepted program-entry
binding that `omega packages update` reviews (accept the pending decisions, then
rerun with `--resume`). Without it, the entry is rejected with a message naming
that review.

`--entry-ram <position>=<hex-bytes>` supplies initial bytes for a borrowed scalar record at a position declared by the installed entry contract. The standard hosted `ProgramEntry` has zero visible arguments. Passing `--entry-ram 0=29000000` to a parameterless hosted program therefore fails with “private entry RAM initializer answers no declared entry position”. Adding a parameter to the source machine also fails the hosted signature check. The [entry contract](../../wiki/spec/build/entry_roots.md#entry-shape-and-arrival-bridge) describes when a platform or embedding caller can supply arguments. The [placed entry RAM probe](../../samples/cli/probes/placed_entry_ram/README.md) records the expected bytes for an explicitly lent allocation.

Startup and dispatch live in `src/main.rs`; typed invocations are parsed in
`src/arguments/` and run in `src/commands/`. Full surface:

```text
omega --check [<root.omg>]
omega [--check] [--offline] [--timings]
      [--report-file <path>]
      [--build-input <path>]... [--optional-build-input <path>]...
      [--build-dir <dir>] [--target <name>]... [--disable-optimization <ExactName>]... <root.omg>
omega build [--compare-tests] <root.omg>
omega run [--psi-work-budget <positive-u64>] [--keep] [--entry-ram <position>=<hex-bytes>]... [--target <name>] [--inputs <dir> | --input <file>...] [--outputs <dir>] <root.omg|program.psi>
omega psi inspect --machine <qualified> [--target <name>] <root.omg>
omega source parse [--] <file>
omega source rewrite <ExactRule>... [--check] <root.omg>
omega packages audit --kind <local|git> <locator> [--rev <rev>]
omega packages audit [--project <dir>] [--target <name>]... [--details] [--offline] [--build-input <path>]... [--optional-build-input <path>]...
omega packages install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]... [--dangerously-accept-all]
omega packages update [package-or-alias...] [--to <revision>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]... [--dangerously-accept-all]
omega packages <install|update> <--resume|--discard-review> [--project <dir>] [--offline]
```

Use `omega --check build.omg` to check every member returned by a workspace
Build. An unnamed check discovers a single enclosing `build.omg`. If a member
and its enclosing workspace each provide one, name the entry to check.

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
([multi-target compilation](../../wiki/spec/build/configuration.md#multi-target-compilation)).
Today a missing `--target` checks target-neutrally and compiles for the host;
the pipeline route items on the task board close that gap.

The compile command's exit status is the
[compiler request](../../wiki/spec/build/compiler_request.md#validation-and-failures)
halt value: 0 when the requested product is complete, 1 for a rejection, and 2
(`Incomplete`) when compilation stopped at a named private provision, such as
a proof-search limit, with no verdict on the source. With several targets it
exits 2 only when every refused target is incomplete; any rejection exits 1.
Argument errors exit 64 (`EX_USAGE`) before compilation starts. `omega run`
returns the program's exit status on completion, 200 on compilation or execution
failure, and 202 when compilation stops at a named private provision.

A single `omega run` without `--outputs` exits with the interpreted program's own status; 203 when the
program crashes (the cause is reported, since a process cannot present the
native trap signal without raising it); and 200 otherwise: `UNAVAILABLE` when a
provider, host adapter or interpreter capability is missing and refused before
the program's first instruction, `FAILED` for an execution error after it (its
buffered output is lost), or execution that stops without a process result.
Source interpretation uses Build, package, trust, provider and entry admission
and lowers the program to Psi. `--keep` keeps `program.oterm` and the
interpreter's streams, work and effect receipts, never a native image. A program that binds
separately produced components is refused during compilation for interpretation
(`Psi compile FAILED`, status 200), since those components only run as
native images. Streams are forwarded afterwards, not live.

Compilation emits requested products and diagnostics, not debug dumps.
`--timings` prints command-stage durations and total elapsed time to stderr;
otherwise no timings are collected.
`--report-file <path>` writes the compile report (product, target, summary and
publication lines) to a plain-text file. For a native publication, it includes
the launchable executable path, the program’s process-exit capability and the
target’s trap presentation. A verifier can launch that path and use the two
observations to interpret the process status.
The requested product decides the required proof and installation records. See
the
[compiler product contract](../subsystems/compiler/README.md).

`omega --version` reports the compiler version, source commit (when the
packager supplies it) and Rust host target. Release bundles carry their
library beside the executable; Cargo development builds fall back to the
checkout. See [release packaging](../../_recycling/tools/release/README.md#build-a-relocatable-bundle)
for installation layout, explicit library selection and relocation tests.

`omega psi inspect --artifact <file>` reads an existing portable Psi artifact
file, decodes it canonically, validates its meaning, and prints
its module and placed-input list. It takes no source, machine or target flags
and neither prepares packages nor executes. An invalid or noncanonical
artifact returns status 1; inspection grants no runtime authority.

A host can pass `--max-psi-input-bytes <bytes>` to inspect, run, or compile
a saved artifact. The CLI refuses an oversized file before decoding it.
Compiling saved Psi to native code requires the trusted producer’s exact
manifest identity through `--trusted-artifact`. For interpretation, see the
[CLI implementation status](#command-surface).
With `--emit-psi`, `--max-psi-output-bytes <bytes>` limits the new artifact.
If it exceeds the quota, an existing destination keeps its previous contents.
Omitting either option leaves the encoded byte count uncapped for that operation.

Inspecting from source also accepts `--output <file>` to save the canonical
artifact for later inspection. The caller chooses that host path; artifact
inspection cannot write an output file.

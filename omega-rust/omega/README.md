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
in [package-manager](src/package_manager/operations/mod.rs), with no second wrapper.

`run_project` owns a distinct temporary directory per invocation. Disposable
output is cleaned up on success and failure; requested retained output survives,
including interpreter disagreement. An explicit target compiles without running
the image. Host execution returns raw stdout/stderr bytes and process status;
unsupported interpretation is a decline, not an agreement.

The library is synchronous and inherits the compiler's current stack requirements.
The CLI provisions its existing compiler worker stack; embedded callers must
provide equivalent stack capacity where needed. This is not a scheduler, command
bus, or new pipeline protocol. [Compiler orchestration](src/compiler/compiler.rs)
continues to pass representations through ordinary functions.

## Command surface

The CLI package is **`omega`** (binary `omega`); there is no `omega-cli`
package.

```bash
mbx run -p omega -- --check samples/cli/basics/cli_mvp/main.omg
```

CLI startup and dispatch live in `src/main.rs`; typed invocations
are parsed in `src/cli/arguments/`. Full surface:

```text
omega [--check] [--offline] [--accept-admissions] [--timings]
      [--report-file <path>]
      [--build-input <path>]... [--optional-build-input <path>]...
      [--build-dir <dir>] [--target <name>]... [--disable-optimization <ExactName>]... <root.omg>
omega run [--both] [--keep] [--target <name>] <root.omg>
omega inspect-terminal --machine <qualified> [--target <name>] <root.omg>
omega audit source --kind <local|git> <locator> [--rev <rev>]
omega audit packages [--project <dir>] [--target <name>]... [--details] [--offline] [--build-input <path>]... [--optional-build-input <path>]...
omega install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]...
omega update [package-or-alias...] [--to <revision>] [--target <name>]... [--project <dir>] [--offline] [--build-input <path>]... [--optional-build-input <path>]...
omega <install|update> <--resume|--discard-review> [--project <dir>] [--offline]
omega refresh-samples [samples-dir]
```

`--offline` restricts package acquisition to local sources and cached recorded
Git pins. It does not refresh selectors or sandbox later program execution.
`run` and `inspect-terminal` do not support this flag.

`--target` names a realization target and may repeat; repeats are removed
and each named target is compiled and published on its own, into
`<build dir>/<target>/` when more than one is named, with one outcome per
target. The settled model realizes every provided target when none is named
and checks every target's bodies either way
([multi-target compilation](../../wiki/spec/build/configuration.md#multi-target-compilation));
today an absent `--target` checks target-neutrally and realizes the host, and
the board's pipeline route items close that gap.

Compilation emits requested products and diagnostics, not debug dumps.
`--timings` prints command-stage durations and total elapsed time to stderr;
normal invocations do not collect optional timing measurements.
`--report-file <path>` writes the produced compile report (product, target,
summary, and publication lines) as a plain-text observation file; there is no
`--output-only` switch. Required proof and installation
records remain governed by the requested product. See the
[compiler product contract](docs/compiler/README.md#product-boundaries-and-observations).

The bundled `source/library/` location is derived from the compiler checkout
captured at build time. Rebuild the binary from the retained checkout before
removing the checkout used to build it.


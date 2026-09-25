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
in [package-manager](packages/manager/src/operations/mod.rs), with no second wrapper.

`run_project` owns a distinct temporary directory per invocation. Disposable
output is cleaned up on success and failure; requested retained output survives,
including interpreter disagreement. An explicit target compiles without running
the image. Host execution returns raw stdout/stderr bytes and process status;
unsupported interpretation is a decline, not an agreement.

The library is synchronous and inherits the compiler's current stack requirements.
The CLI provisions its existing compiler worker stack; embedded callers must
provide equivalent stack capacity where needed. This is not a scheduler, command
bus, or new pipeline protocol. [Compiler orchestration](compiler/src/compiler.rs)
continues to pass representations through ordinary functions.

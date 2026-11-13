# Psi interpreters

This engine executes the target-neutral representation:

- [Terminal interpreter](terminal-interpreter/README.md): execution of a verified
  Terminal Psi artifact.

Compilation precedes execution in the calling product workflow. The engine depends
on its input representation and semantic checks, not on the product compiler
coordinator. This keeps compiler use of evaluation from creating a dependency
cycle through an interpreter command workflow.

[Compile-time admission and elaboration](../compilation/build-time-evaluation/README.md)
belong to compilation. That service invokes the Terminal engine and installs its
results into compiler representations; the engine does not own those
transformations. A build-time machine that would have to run before the program
is checked has no engine and is refused.

Native realization remains Omega-owned; interpretation is not a fallback for
unsupported native lowering. See the [pipeline contract](../../pipeline.md).

The interpreter installs supplied bindings and dispatches calls; target-aware
host adapters live outside Psi in Omega-owned host integration. They reuse
independent backend ABI/mechanism libraries and retain exact foreign-binding
checks. Psi acquires no Omega dependency. Ordinary Omega provider bodies can
run interpreted; only external mechanisms require host adapters. See
[target environments and engines](../../../wiki/spec/build/embedding.md#target-environments-and-execution-engines).

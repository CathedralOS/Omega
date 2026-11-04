# Build system

Everything between the `omega` router and the compilers: running a project,
executing its `build.omg`, calling the compilers, and publishing what they
produce.

| Directory | Responsibility |
| --- | --- |
| [orchestration/](orchestration/README.md) | Runs a whole build in order: prepares inputs, runs `build.omg`, calls the compiler, writes outputs |
| [build-settings/](build-settings/README.md) | Runs `build.omg` and returns the settings it sets |

`omega build` reaches `orchestration` from the router. `orchestration` begins a
compilation, runs the project's Build through `build-settings`, completes the
compilation, and finishes native products. The compiler never runs Build
itself; see [compile/](orchestration/src/compile.rs) for that sequence.

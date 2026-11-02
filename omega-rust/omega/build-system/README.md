# Build system

Everything between the `omega` router and the compilers: running a project,
executing its `build.omg`, calling the compilers, and publishing what they
produce.

| Directory | Responsibility |
| --- | --- |
| [omega-build/](omega-build/README.md) | Runs a project or package batch: prepares it, calls the compiler, publishes outputs; Cargo package `omega-build` |
| [build-evaluation/](build-evaluation/README.md) | Executes `build.omg`: Build admission, activation and its host |

`omega build` reaches `omega-build` from the router. `omega-build` begins a
compilation, runs the project's Build through `build-evaluation`, completes the
compilation, and finishes native products. The compiler never runs Build
itself; see [compile/](omega-build/src/compile.rs) for that sequence.

# Bootstrap by language

Every tested route starts with the [Alpha VM](alpha/README.md) and the
separately admitted [Beta compiler tape](beta/AUDIT.md). Beta reproduces its
own tape and compiles the Beta-written Gamma interpreter. From Gamma onward,
choose how to run a program at each rung.

| Language | Interpret | Compile to Alpha tape | Compile to native Linux x86-64 |
| --- | --- | --- | --- |
| [Gamma](gamma/README.md) | Beta-written interpreter on Alpha | Shared Gamma compiler, Alpha backend | Same compiler, Linux backend |
| [Delta](delta/README.md) | Gamma-written interpreter executes checked Delta | Emit Gamma and compile to Alpha | Emit Gamma and compile to Linux |
| [Epsilon](epsilon/README.md) | Delta-written evaluator | Emit Delta, then Gamma, then Alpha | Emit Delta, then Gamma, then Linux |

Delta also has a route that translates the program to Gamma and runs the
Gamma interpreter on it. That route exercises the Delta compiler. The direct
Delta interpreter reads the checked Delta expressions itself.

Each row describes the **program being run**. The compiler's own host is a
separate choice. For example, a Gamma compiler running on Linux can emit Alpha
tape. A language's directory holds its implementations once, including those
shared by multiple outcomes.

The next rung is [Omega](omega/README.md). The bootstrap compiler must build the Omega compiler from source. The resulting
Omega compiler must then reproduce itself. Full
self-construction remains open. [Construction contract](CONTRACT.md) · [Bootstrap mandate](MINIMIZATION.md)

The [Linux cold-build harness](../tests/bootstrap/linux_x86_64/README.md) drives the seed and compilers on Linux x86-64. The [tested routes](../wiki/development/bootstrap/README.md) record which combinations have run.

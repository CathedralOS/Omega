# Bootstrap by language

Every tested route starts with the [Alpha VM](alpha/README.md) and the
separately admitted [Beta compiler tape](beta/AUDIT.md). Beta reproduces its
own tape and compiles the Beta-written Gamma interpreter. From Gamma onward,
choose how to run a program at each rung.

| Language | Interpret | Compile to Alpha tape | Compile to native Linux x86-64 |
| --- | --- | --- | --- |
| [Gamma](gamma/README.md) | Beta-written interpreter on Alpha | Shared Gamma compiler, Alpha backend | Same compiler, Linux backend |
| [Delta](delta/README.md) | Emit Gamma and interpret it | Emit Gamma and compile to Alpha | Emit Gamma and compile to Linux |
| [Epsilon](epsilon/README.md) | Delta-written evaluator | Emit Delta, then Gamma, then Alpha | Emit Delta, then Gamma, then Linux |

Each row describes the **program being run**. The compiler's own host is a
separate choice. For example, a Gamma compiler running on Linux can emit Alpha
tape. A language's directory holds its implementations once, including those
shared by multiple outcomes.

## Tested threads through the matrix

| Gamma → Delta → Epsilon | Furthest shared workload measured |
| --- | --- |
| Interpret → interpret generated Gamma → interpret Epsilon | Rebuilt the Epsilon evaluator and ran four lexer predicates on six inputs. The portable Omega compilation reached its 300 s allowance. [Interpreted run](../tests/bootstrap/interpreted/epsilon/README.md) |
| Alpha → Alpha → interpret Epsilon on Alpha | Compiled the product wire decoder and `Request.clear` to Alpha. [Alpha run](routes/compiled-alpha.md#compare-a-production-request-operation-across-evaluator-hosts) |
| Linux → Linux → interpret Epsilon on Linux | Produced both of those Alpha programs byte for byte. [Wire decoder comparison](#gamma-host-and-target-comparison) |
| Emit Alpha → emit Gamma for Alpha → compile Epsilon via Delta to Alpha | The string scanner passed 17 cases. The full scanner exceeded Alpha's tape limit. [Scanner measurements](routes/native.md#public-omega-lexer-observations) |
| Emit Linux → emit Gamma for Linux → compile Epsilon via Delta to Linux | The string scanner passed the same 17 cases. The complete public lexer passed 23. [Scanner measurements](routes/native.md#public-omega-lexer-observations) |

The Alpha and Linux scanner executables came from the same Gamma for each
customer. The measured scanner runs reused source-built predecessors. Timing
out, exceeding a tape limit and completing an execution are recorded separately
in the route reports.

The next rung is [Omega](omega/README.md). A winning thread must construct
the Omega compiler from the declared seed and use it to reproduce itself.
That remains open. [Bootstrap mandate](MINIMIZATION.md)

## Gamma host and target comparison

Both Gamma compiler hosts independently produced the same Delta compiler and
Epsilon evaluator for each target. The shared Delta program also ran under the
Gamma interpreter. [Four-cell experiment, commands, sizes and timings](routes/gamma-hosts.md)

## Building the compiler

From the repository root on Linux x86-64, run
`bootstrap/hosts/linux_x86_64/bootstrap . /tmp/omega-gamma` with a new output
directory to build the native Gamma compiler offline. Put `--alpha` before
`.` to build the Alpha-tape compiler. Both commands rebuild the Gamma compiler
twice with an empty `PATH`, using the embedded seeds and the supplied source files.
The same entry accepts the Delta compiler source and an Epsilon evaluator
compilation request. [Offline commands and inputs](hosts/linux_x86_64/README.md)

The bootstrap compiler's job is to build the **Omega compiler** (C in older
records), the product written in Omega.
Construction of the full product and its reproducible self-build remain open.
The product uses a deliberate subset of Omega while preserving the language's
semantics and explicit refusals. [Omega compiler and libraries](../source)

The complete construction input used here contains 304 Omega source units
in seven packages. Its 3,955,617 bytes fit the constructor's source-input
provision. Packaging it with the Epsilon-written bootstrap compiler for the
evaluator takes 4,807,765 bytes. Both stock targets returned outer status
253 with empty output at the Delta adapter's 4 MiB input check. A native
evaluator rebuilt with an 8 MiB input limit reached the constructor on a
same-size control. The real request ran for 480 s on that evaluator and
published neither an artifact nor a compiler diagnostic before the watchdog.
[Product input](../source/application/construction/construct.py) · [Evaluator experiment](../tests/bootstrap/independent/WHOLE_REQUEST.md)

A host Rust toolchain builds the Rust comparator, which can compile Omega
sources during development. Record that dependency when reporting a
Rust-assisted build. [Rust comparator](../source-rust)

Host-side tests, reference interpreters and development accelerators live under [tests/bootstrap](../tests/bootstrap/). When promoting a chain, take its candidate tests and the [portable conformance checks](../tests/bootstrap/conformance/README.md). The compiled routes share their comparison helpers in [compiled.py](../tests/bootstrap/compiled.py).

Record seed and trust assumptions, actual cold runs, development acceleration
and failed experiments.
The [edge contracts](CONTRACT.md) describe the retained language edges.
Scripts locate their sources through [`tools/bootstrap/paths.sh`](../tools/bootstrap/paths.sh).
[Proof research](proofs/README.md) is optional for construction.

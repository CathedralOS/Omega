# Bootstrap construction paths

[The bootstrap mandate](MINIMIZATION.md) governs this work. Each viable path must
construct the Omega compiler from its declared auditable seed and then use that
compiler to rebuild itself reproducibly. No path is selected during exploration.

All routes start with the audited Alpha VM and a separately admitted Beta
compiler tape. Running it on Beta's own source reproduces the tape byte for
byte. Rebuilt Beta then compiles the Beta-written Gamma evaluator to Alpha. On compiled routes this evaluator constructs the
first Gamma compiler. [Alpha seed](interpreted/0_alpha/README.md) · [Beta tape audit](interpreted/1_beta/AUDIT.md)

**Alpha** means tape executed by the seed VM. **Native** means a Linux
x86-64 executable. The Delta compiler is written in Gamma and emits Gamma
on every route.

| Route | Delta compiler runs on | Epsilon implementation | Demonstrated workload |
| --- | --- | --- | --- |
| [Original interpreter](interpreted/README.md) | Gamma evaluator on Alpha | Epsilon evaluator, interpreted by Gamma | The bootstrap compiler emits Alpha tape for supported Omega programs. |
| [Gamma compiled to Alpha](compiled_vm/README.md) | Alpha | Epsilon evaluator on Alpha | Same Omega programs. |
| [Gamma compiled to native](independent/README.md) | Native | Native Epsilon evaluator | Same Omega programs. |
| [Epsilon compiled to Delta, Alpha target](independent/README.md#calling-the-numeric-lexer-with-a-stored-enum) | Alpha | Epsilon-to-Delta compiler on Alpha | Numeric lexer on Alpha. |
| [Epsilon compiled to Delta, native target](independent/README.md#checked-epsilon-compilation) | Native | Native Epsilon-to-Delta compiler | Numeric lexer on Linux. |

The first three use one Delta-written evaluator to interpret the Epsilon-written
[bootstrap compiler](interpreted/5_omega/README.md)
(D in older records). The last two use a Delta-written compiler to translate Epsilon
into Delta modules. Adding a runtime and entry point lets the Delta and Gamma
compilers build an executable.

Native can lead back to Alpha. Compile the Gamma-to-Alpha compiler for Linux
and it emits tape for the original VM. Native-hosted and Alpha-hosted compilers
produced identical numeric lexer tapes. The tape and Linux executable agreed
on all thirteen inputs. [Cross-compilation](independent/README.md#construction-and-acceptance) · [Lexer comparison](independent/README.md#calling-the-numeric-lexer-with-a-stored-enum)

From the repository root on Linux x86-64, run
`bootstrap/linux_x86_64/bootstrap . /tmp/omega-gamma` with a new output
directory to build the native Gamma compiler offline. Put `--alpha` before
`.` to build the Alpha-tape compiler. Both commands rebuild the Gamma compiler
twice. The same entry accepts the Delta compiler source and an Epsilon evaluator
compilation request. [Offline commands and inputs](linux_x86_64/README.md)

The bootstrap compiler's job is to build the **Omega compiler** (C in older
records), the product written in Omega.
Construction of the full product and its reproducible self-build remain open.
The product uses a deliberate subset of Omega while preserving the language's
semantics and explicit refusals. [Product Psi](../source/psi) · [Product Omega](../source/omega)

A host Rust toolchain builds the Rust comparator, which can compile Omega
sources during development. Record that dependency when reporting a
Rust-assisted build. [Rust comparator](../omega-rust)

Each path owns its source, invocation and construction evidence. Retain only
machinery needed by real successors or necessary validation. Record seed/trust
assumptions, actual cold runs, development acceleration and failed experiments.
The [edge contracts](CONTRACT.md) describe the retained language edges.
Scripts locate their sources through [`tools/bootstrap/paths.sh`](../tools/bootstrap/paths.sh).
[Proof research](proofs/README.md) is optional for construction.

# Bootstrap construction paths

[The bootstrap mandate](MINIMIZATION.md) governs this work. Each viable path must
construct the Omega compiler from its declared auditable seed and then use that
compiler to rebuild itself reproducibly. No path is selected during exploration.

The four routes below start with the audited Alpha VM and a separately admitted
Beta compiler tape. Running that tape on the Beta compiler's own source
reproduces the tape byte for byte. The rebuilt Beta compiler then turns the
Beta-written Gamma evaluator into an Alpha tape. [Alpha seed](interpreted/0_alpha/README.md) · [Beta tape audit](interpreted/1_beta/AUDIT.md)

The first three routes use the same Delta compiler and Epsilon evaluator
sources. All three run the Epsilon-written **bootstrap compiler** (D in older
records), which emits Alpha tapes for supported Omega programs. The fourth
route compiles Epsilon itself. [Bootstrap compiler](interpreted/5_omega/README.md)

| Route | Authored sources | What executes the program | Result |
| --- | --- | --- | --- |
| [Original interpreted chain](interpreted/README.md) | Gamma-written Delta compiler and Delta-written Epsilon evaluator. | The Gamma evaluator runs the Delta compiler, then the Gamma source it emits for the Epsilon evaluator. | The Epsilon evaluator interprets the bootstrap compiler on Omega input. |
| [Gamma to Alpha](compiled_vm/README.md) | Gamma-written Gamma compiler with an Alpha backend. | The Gamma evaluator constructs the first compiler. Alpha executes each compiled tape. | Separate Gamma compiler, Delta compiler and Epsilon evaluator tapes. |
| [Native Gamma with Epsilon evaluation](independent/README.md) | The same Gamma compiler with a Linux x86-64 backend. | The Gamma evaluator constructs the first native compiler. Linux runs the compiled stages. | Standalone Gamma compiler, Delta compiler and Epsilon evaluator executables. |
| [Native Epsilon-to-Delta compiler](independent/README.md#checked-epsilon-compilation) | A compiler written in Delta translates supported Epsilon modules, including parts of the bootstrap compiler's lexer. | The native Delta and Gamma compilers build the Epsilon-to-Delta compiler. Linux runs it. | A Delta module. With a runtime and an entry point, the native Delta and Gamma compilers turn that module into a Linux executable. |

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

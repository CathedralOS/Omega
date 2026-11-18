# Bootstrap construction paths

[The bootstrap mandate](MINIMIZATION.md) governs this work. Each viable path must
construct the Omega compiler from its declared auditable seed and then use that
compiler to rebuild itself reproducibly. No path is selected during exploration.

```text
bootstrap/
  interpreted/   existing interpreter-based chain and its language sources
  compiled_vm/   source-compiled Gamma route to independent Alpha tapes
  independent/   native Linux compiler construction
  linux_x86_64/  offline binary construction of native Gamma, Delta and their successors
  proofs/        optional research outside construction acceptance
```

The [interpreted path](interpreted/README.md) contains the existing numbered
rungs. The [compiled VM path](compiled_vm/README.md) currently reuses their seed
and successor sources and owns its Gamma compiler. The native path shares that
frontend and emits Linux executables. One Linux x86-64 command constructs
the native Gamma compiler from its predecessors, rebuilds it twice, and
can deliver the native Delta compiler from the distributed Gamma source.
Given the Epsilon evaluator's Delta compilation request, the same command
produces a native Epsilon evaluator.
[Native path](independent/README.md) · [Offline command](linux_x86_64/README.md)

The Omega-written product lives in [`source/psi`](../source/psi) and
[`source/omega`](../source/omega), outside any bootstrap variant. Its construction
can advance through either bootstrap or the [Rust comparator](../omega-rust).
Keep its implementation within a deliberate subset of the language it implements
where that simplifies the complete bootstrap, while preserving specified source
semantics and explicit refusals. Rust-assisted results remain identified as such.

Construction and reproducible self-compilation of the full Omega compiler remain
open. The working stages have executed Omega programs and rebuilt the Gamma
compiler.

Each path owns its source, invocation and construction evidence. Retain only
machinery needed by real successors or necessary validation. Record seed/trust
assumptions, actual cold runs, development acceleration and failed experiments.
The existing [edge contracts](CONTRACT.md) describe the retained language edges;
they do not fix the architecture of new paths. Scripts locate retained sources
through [`tools/bootstrap/paths.sh`](../tools/bootstrap/paths.sh).

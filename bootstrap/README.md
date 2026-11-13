# Bootstrap construction paths

[The bootstrap mandate](MINIMIZATION.md) governs this work. Each viable path must
construct the Omega compiler from its declared auditable seed and then use that
compiler to rebuild itself reproducibly. No path is selected during exploration.

```text
bootstrap/
  interpreted/   existing interpreter-based chain and its language sources
  compiled_vm/   source-compiled Gamma route to independent Alpha tapes
  independent/   standalone-language construction experiment
  proofs/        optional research outside construction acceptance
```

The [interpreted path](interpreted/README.md) contains the existing numbered
rungs. The [compiled VM path](compiled_vm/README.md) currently reuses their seed
and successor sources and owns its Gamma compiler. The
[independent path](independent/README.md) has its first construction target
specified; its implementation has not started. Shared source reuse is explicit,
and does not require other experiments to use the same stages or implementation.

The Omega-written product lives in [`source/psi`](../source/psi) and
[`source/omega`](../source/omega), outside any bootstrap variant. Its construction
can advance through either bootstrap or the [Rust comparator](../omega-rust).
Keep its implementation within a deliberate subset of the language it implements
where that simplifies the complete bootstrap, while preserving specified source
semantics and explicit refusals. Rust-assisted results remain identified as such.

Neither existing path has constructed and reproducibly self-rebuilt the full
Omega compiler. Executed Omega programs and the Gamma compiler's self-rebuilds
are intermediate evidence, not that acceptance.

Each path owns its source, invocation and construction evidence. Retain only
machinery needed by real successors or necessary validation. Record seed/trust
assumptions, actual cold runs, development acceleration and failed experiments.
The existing [edge contracts](CONTRACT.md) describe the retained language edges;
they do not fix the architecture of new paths. Scripts locate retained sources
through [`tools/bootstrap/paths.sh`](../tools/bootstrap/paths.sh).

# Interpreted construction path

This directory contains the existing chain as one peer architecture:

```text
0_alpha/    audited native Alpha execution floor
1_beta/     Beta compiler source and separately admitted seed tape
2_gamma/    Gamma evaluator written in Beta
3_delta/    Delta compiler written in Gamma
4_epsilon/  Epsilon evaluator written in Delta
5_omega/    bootstrap compiler written in Epsilon
```

The construction target is:

```text
Alpha -> Beta -> Gamma -> Delta -> Epsilon -> bootstrap compiler
      -> Omega compiler -> reproducibly self-rebuilt Omega compiler
```

The seed includes the audited [Alpha implementation](0_alpha/README.md) and
separately admitted [1,773-byte Beta tape](1_beta/AUDIT.md). Reconstructing that
tape from source establishes identity; it does not remove the tape as a trust
input. Source and disassembly annotations retain historical paths so relocation
preserves their bytes.

The executable upper edge constructs exercised Omega programs. Full construction
of the product compiler and its self-rebuild remain unfinished. The product is
[`source/{psi,omega}`](../../source/omega), rather than another implementation
owned by this variant. Its implementation subset should keep bootstrap demands
small without changing the semantics of supported Omega facilities.

Cross-rung invocation resolves paths through
[`tools/bootstrap/paths.sh`](../../tools/bootstrap/paths.sh). Existing identity,
source-closure and executable gates remain under `tests/bootstrap/` and the
language-specific test directories. Reused artifacts and development accelerators
must remain distinguished from fresh reference-chain execution.

[The compiled VM peer](../compiled_vm/README.md) explicitly reuses these retained
sources. That reuse grants this architecture no preferred or selected status.
[Optional proof research](../proofs/README.md) does not gate construction.

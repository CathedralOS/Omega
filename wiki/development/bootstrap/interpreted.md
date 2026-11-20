# Interpreted construction path

This route uses the shared seed and one implementation from each language.

```text
alpha/                 audited VM
beta/                  self-reproducing compiler tape
gamma/interpreter/     Beta-written Gamma interpreter
delta/to-gamma/        Gamma-written Delta compiler
epsilon/interpreter/   Delta-written Epsilon evaluator
omega/compiler/        Epsilon-written bootstrap compiler
```

The construction target is:

```text
Alpha -> Beta -> Gamma -> Delta -> Epsilon -> bootstrap compiler
      -> Omega compiler -> reproducibly self-rebuilt Omega compiler
```

The seed includes the audited [Alpha implementation](../../../bootstrap/alpha/README.md) and
separately admitted [1,773-byte Beta tape](../../../bootstrap/beta/AUDIT.md). Reconstructing that
tape from source establishes identity; it does not remove the tape as a trust
input. Source and disassembly annotations retain historical paths so relocation
preserves their bytes.

The executable upper edge constructs exercised Omega programs. Full construction
of the product compiler and its self-rebuild remain unfinished. The product is
[the Omega product sources](../../../source), rather than another implementation
owned by this variant. Its implementation subset should keep bootstrap demands
small without changing the semantics of supported Omega facilities.

Cross-rung invocation resolves paths through
[`tools/bootstrap/paths.sh`](../../../tools/bootstrap/paths.sh). Existing identity,
source-closure and executable gates remain under `tests/bootstrap/interpreted/` and the
language-specific test directories. Reused artifacts and development accelerators
must remain distinguished from fresh reference-chain execution.

[The compiled VM peer](compiled-alpha.md) explicitly reuses these retained
sources. That reuse grants this architecture no preferred or selected status.
[Optional proof research](../../../tests/bootstrap/proofs/README.md) does not gate construction.

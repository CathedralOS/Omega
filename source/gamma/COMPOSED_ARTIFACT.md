# Gamma composed artifact

A Gamma executable is not a new Alpha tape for each program. `GammaComposedV1`
is the exact pair:

```text
(evaluator Alpha tape identity, Gamma source identity)
```

A manifest records:

```text
GammaComposedV1
evaluator-sha256 HEX
source-sha256 HEX
source-length DECIMAL
```

The evaluator identity must name the selected
`evaluator/gamma_evaluator_bytecode.tape`. The source identity covers the exact
Gamma source bytes, including comments and whitespace. The manifest does not
contain source, select semantics, or authorize a different evaluator.

Invocation constructs the evaluator request already fixed by
`EVALUATOR_PROFILE.md`:

```text
u32-le(source length) + exact source + sealed input
```

Framing is replaceable host plumbing. It may read the two identified files,
construct this byte sequence, execute the selected evaluator container, buffer
stdout, and atomically publish that buffer when status is zero or stdout is
nonempty. Empty nonzero stdout leaves an existing destination unchanged. The
evaluator's validated application-result convention makes this predicate exact:
published nonzero results are nonempty, while evaluator failures and discarded
application outcomes expose no stdout. Plumbing may not parse Gamma, change
source bytes, decode application statuses or output, recover partial output, or
select an alternate evaluator.

This composition avoids rebuilding a Gamma-to-Alpha compiler while preserving
three separately auditable facts: the fixed evaluator tape, readable program
source, and exact request/observation contract.

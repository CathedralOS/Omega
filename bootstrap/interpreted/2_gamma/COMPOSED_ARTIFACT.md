# Gamma composed artifact

A Gamma executable is not a new Alpha tape for each program. `GammaComposedV2`
is the exact triple:

```text
(evaluator Alpha tape identity, Gamma source identity, support identity)
```

A manifest records:

```text
GammaComposedV2
evaluator-sha256 HEX
source-sha256 HEX
source-length DECIMAL
support-sha256 HEX
support-length DECIMAL
```

The evaluator identity must name the selected
`gamma_evaluator_bytecode.tape`. The source identity covers the exact
Gamma source bytes, including comments and whitespace. The support identity
covers the exact packed bytes of the artifact's bound support-member
closure - an ordered source-closure manifest whose members the program may
consult or copy at run time, such as the Delta compiler's emitted runtime
and profile adapter. An artifact with no bound support members binds the
empty section (`support-length 0` with the empty-input digest). The manifest
does not contain source, select semantics, or authorize a different
evaluator.

Invocation constructs the evaluator request already fixed by
`EVALUATOR_PROFILE.md`:

```text
u32-le(source length) + exact source + sealed input
```

where the sealed input is the edge's payload bytes followed by the packed
support members in manifest order. The payload format and the program's use
of the support section remain the edge's own contract; the composition
guarantees only that the program observes payload then members, in order,
and that no other file or name participates.

Framing is replaceable host plumbing. It may read the identified files -
evaluator tape, source, and the bound member files - construct this byte
sequence, execute the selected evaluator container, buffer
stdout, and atomically publish that buffer when status is zero or stdout is
nonempty. Empty nonzero stdout leaves an existing destination unchanged. The
evaluator's validated application-result convention makes this predicate exact:
published nonzero results are nonempty, while evaluator failures and discarded
application outcomes expose no stdout. Plumbing may not parse Gamma, change
source or member bytes, select, reorder, or discover members by name, decode
application statuses or output, recover partial output, or
select an alternate evaluator.

This composition avoids rebuilding a Gamma-to-Alpha compiler while preserving
three separately auditable facts: the fixed evaluator tape, readable program
source, and exact request/observation contract. Bound support members add a
fourth: the exact readable bytes a program depends on beyond its own source.

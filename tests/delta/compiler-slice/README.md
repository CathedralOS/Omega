# Delta compiler scalar slice

This gate exercises the selected in-progress
`source/delta/compiler/delta_compiler.gamma` and preserves the earlier
`schema_elaborator.gamma` proof as supporting evidence. The selected compiler
emits canonical Gamma source instead of owning a direct Alpha backend.

The 239-line Gamma elaborator validates one typed accumulator-recursion schema
with arbitrary binder spellings and emits `scalar_recursive.gamma` byte for
byte. The expansion is the same workload used by the direct Functional and
State Delta compiler experiments. It uses only Gamma's existing value stack,
ordinary calls, branches, and tail `jump`; no Gamma or Alpha primitive was
added, and the elaborator contains no Alpha encoder.

The generated Gamma application writes its scalar result as one byte because
Gamma's application contract completes `main` with status zero. The direct
scalar Functional Delta experiment currently exposes its result as an Alpha
halt status. Both observations carry the value 15, but this first gate does not
claim identical application profiles. A renamed 1,000-step input additionally
proves structural binder checking and constant-space Gamma recursion; malformed
schema variants reject before publishing any output.

The measured path is:

```text
9-line Functional Delta source
	-> 239-line / 9,526-byte Delta-to-Gamma elaborator
	-> 5-line / 223-byte canonical Gamma source
	-> existing Gamma compiler
	-> 1,366-byte Alpha tape
```

The generalized path additionally compares interpreted and native elaborator
output byte for byte for a recursive program and a full-surface program. The
latter covers all scalar operators, `let`, `if`, 0/1/2/13 arguments, forward
calls, nested calls, renamed binders/functions, and direct recursion. Both
expansions compile through the existing Gamma compiler and execute as Alpha.
Malformed definitions, names, types, calls, arities, and expressions must reject
with no output.

```text
550-line / 21,336-byte selected compiler
	-> 19,238-byte elaborator tape

9-line / 198-byte recursive Delta
	-> 25-line / 1,267-byte canonical Gamma
	-> 2,498-byte Alpha tape -> byte 15

32-line / 947-byte full-surface Delta
	-> 77-line / 4,324-byte canonical Gamma
	-> 5,884-byte Alpha tape -> byte 21
```

The schema proof and its measurements remain independent evidence. The selected
compiler is not yet admissible as the complete Delta edge. Its current
acceptance limit includes one aggregate 15-level list-expression bound, and it
still lacks nominal data, exhaustive `match`, `Bytes`, complete checking,
proper tail calls, application profiles, and exact resource outcomes.
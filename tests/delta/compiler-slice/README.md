# Delta compiler scalar slice

This gate exercises the downgraded compiler at
`bootstrap/delta/bootstrap/concatenative-compiler/delta_compiler.gamma` and preserves the earlier
`schema_elaborator.gamma` proof as supporting evidence. The selected compiler
is now open; this retained compiler emits former concatenative Gamma source.

The 240-line Gamma elaborator validates one typed accumulator-recursion schema
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
9-line / 202-byte Functional Delta source
	-> 240-line / 9,629-byte Delta-to-Gamma elaborator
	-> 9,535-byte elaborator tape
	-> 5-line / 223-byte canonical Gamma source
	-> existing Gamma compiler
	-> 1,366-byte Alpha tape
```

The generalized path additionally compares interpreted and native elaborator
output byte for byte across scalar, nominal-data, Bytes, and tail-call programs. The
scalar surface covers all scalar operators, `let`, `if`, 0/1/2/13 arguments,
forward calls, nested calls, renamed binders/functions, and direct recursion.
The ADT fixtures cover `(data TYPE (CTOR TYPE*)+)`, nominal parameter/return
annotations, nullary and payload constructors, exhaustive constructor-pattern
`match`, nested `match` evaluation inside arm bodies, and recursive self
references in constructor fields. Patterns may be `CTOR` or `(CTOR binder*)`.
Nested matches use
row-owned runtime scratch cells so inner evaluation cannot overwrite an outer
scrutinee. All expansions compile
through the existing Gamma compiler and execute as Alpha. Malformed definitions,
names, types, calls, constructor/match ownership, duplicate arms,
non-exhaustive matches, constructor arities, and binder-scope misuse must reject
with no output.

```text
1,690-line / 66,682-byte downgraded compiler
	-> 51,239-byte compiler tape

9-line / 202-byte recursive Delta
	-> 42-line / 1,977-byte canonical Gamma
	-> 3,660-byte Alpha tape -> byte 15

32-line / 951-byte full-surface Delta
	-> 100-line / 5,232-byte canonical Gamma
	-> 7,214-byte Alpha tape -> byte 21

11-line / 244-byte Option Delta
	-> 21-line / 1,614-byte canonical Gamma
	-> 2,905-byte Alpha tape -> byte 9

12-line / 301-byte List Delta
	-> 45-line / 3,367-byte canonical Gamma
	-> 5,332-byte Alpha tape -> byte 9

11-line / 254-byte nested-match Delta
	-> 40-line / 2,603-byte canonical Gamma
	-> 4,381-byte Alpha tape -> byte 9

7-line / 213-byte five-operation Bytes Delta
	-> 53-line / 2,510-byte canonical Gamma
	-> 5,305-byte Alpha tape -> byte 66

14-line / 290-byte tail-call Delta
	-> 47-line / 2,171-byte canonical Gamma
	-> 3,858-byte Alpha tape -> byte 1
```

The packed five-operation Bytes runtime is checked byte for byte against a
20-line / 1,095-byte readable Gamma source and adds no Gamma primitive.

The schema proof and its measurements remain independent evidence. The selected
compiler is not yet admissible as the complete Delta edge. It still lacks
sealed application profiles, exact resource outcomes, and complete Epsilon
workflow behavior. The fully annotated current Epsilon compiler source, plus a
synthetic `main`, parses, resolves, checks, and emits 18,505 lines / 1,662,077
bytes of canonical Gamma; this is sufficiency evidence, not admission.
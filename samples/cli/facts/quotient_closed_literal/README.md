# Closed literal quotient lifting

The fact is that an explicit quotient lift may pass a closed `3i32` alongside
an opaque quotient value, preserving the representative's exact precondition and
congruence theorem. The program observes the returned `3` and exits zero; another
value takes a `Trap` route. Each application binds all four hosted targets.

`main.omg` uses the complete automatic forward-precondition proof.
`transport/` selects the complete transport theorem instead. `scalar_first/`
places the scalar before the quotient, testing the ordinary scalar/structural
argument split and structural nominal-license indexing. These are distinct
mechanism consumers, not a transport-place matrix.

Each negative changes one obligation:

- `invalid/`: pass `4i32` to the representative requiring `3i32`.
- `invalid_theorem/`: replace one shared theorem argument with `3i32`; equality
  under a premise does not satisfy the exact congruence schema.
- `invalid_landing/`: give the closed literal the wrong exact `u32` carrier.
- `invalid_missing_legality/`: omit the independently prior premises needed to
  form both representative applications in the theorem conclusion.

The frozen baseline at `47220be5d93deb294218fc143a472cb74362358d` rejected the
unchanged automatic and selected-transport roots when forming the representative
calls in the congruence theorem, despite the preceding `amount == 3i32` premises.
The first repaired producer passed formation and then refused the theorem's exit
equality. These are measured source failures; the old literal-source bridge fence
was inspected in code, not reached by that baseline.

The governing rules are [quotient lifting and correspondence](../../../../wiki/spec/proofs/quotients.md)
and [contract formation and call denotation](../../../../wiki/spec/proofs/contracts.md).
The checking judgment retains exact literals, types, theorem roles and legality.
Terminal Psi retains nominal quotient adaptations on ordinary calls, not source
proof schemas. The runtime oracle requires both Terminal interpretation and native
execution; hosted target declarations alone supply no foreign-host execution evidence.

This witness does not close preconditioned `define`, computed quotient results,
generic applications, review projection or transitive assumption closure.

The repaired producer at `624d97cbd7f2276ce21883ec6551be067e722e73`
passed all three programs with native exit zero and Terminal exit zero on Linux
x86_64. All four controls rejected for their authored obligation. Replacing only
the expected returned `3` with `4` produced a native illegal-instruction signal
and a matching Terminal `Trap`; the existing copy-carrier program retained exit
70 in both engines. These observations precede integration of later main changes
and are retained as historical evidence, requiring affected requalification.
The CLI retains the native ELF product; its in-memory Terminal artifact is the
interpreter input, but this path does not persist separate Terminal bytes.

The root regression also selects `lifted` for separate canonical-artifact
inspection. Its external byte ingress must preserve the exact semantic bytes
and manifest. This transport oracle is distinct from the runtime artifact.

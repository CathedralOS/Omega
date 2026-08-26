# OMGCOMP lower-rooted refinement witness, version 10

[`OMGRFN9`](OMGCOMP_REFINEMENT_WITNESS_V9.md) |
[`OMGRSW1`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION.md) |
[`OMGRSW2`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V2.md) |
[`OMGRSW3`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V3.md) |
[`CKIR8`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V8.md) |
[`CKIR8 backend`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V8_BACKEND.md)

`OMGRFN10` is the private lower-rooted carrier for pure, total, nontrapping
same-carrier scalar equality. It freezes all earlier identities and adds no
resolution identity: exact source still selects the least canonical OMGRSW1,
OMGRSW2, or OMGRSW3.

## Exact carrier

The 40-byte little-endian header uses magic `OMGRFNA\0` and version 10. It
carries exact OMGCOMP, selected OMGRSW1/2/3, CKIR8, and ELF bytes under the
inherited ceilings, EOF, entry/library, full-result, and exit-projection rules.
CKIR8 has schema major 8, minor 0, target 1 and must contain opcode 18
`ScalarEqual`. The operation has one canonical Boolean result, two visible
operands of the same exact scalar carrier (`bool`, `u8`, or `u32`), and zero
immediates. Inherited opcode 15/16/17 rows remain valid but are optional.

## Source, meaning, and lowering

The admitted source token is `==`; assignment's single `=` is not equality.
Both equality operands must have the same exact scalar carrier and be
independently pure, total, and
nontrapping. Calls, indexing, arithmetic, structural values, mixed scalar
carriers, `!=`, and missing operands are outside this tranche.

R4 reparses the admitted equality expressions, evaluates `bool`/`u8`/`u32`
equality, proves the exact full source result, and independently rebuilds the filtered
source-to-operation stream. Every authored `==` has one distinct opcode-18 row;
folding, dropping, or reordering an equality row rejects.

The broader `<`/`==`/`&&`/`||` ordering and equality-chain controls remain the
CKIR8 producer gate's responsibility. OMGRFN10 does not claim a second
independent proof of those mixed-expression precedence cases.

The pinned x86-64 operation template for result `r` and operands `a,b` is:

```text
8b 85 <disp32(a)> 3b 85 <disp32(b)> 0f 94 c0 0f b6 c0 89 85 <disp32(r)>
```

## Independent responsibilities

1. R1 owns exact OMGRFN10 framing and complete OMGCOMP custody.
2. R2 owns least-resolution selection and requires exact paired `==` syntax.
3. R3 joins the witness to CKIR8 and owns opcode-18 row shape.
4. R4 owns exact equality token correspondence, typing, purity, lowering,
   equality meaning, and the complete source result without ELF access.
5. R5 owns complete CKIR8 validation/evaluation and exact ELF reconstruction
   without source-body or witness-identity access.

Acceptance is their conjunction over one immutable frame. Version, component
cross-pair, result/exit, opcode shape/order/type, equality truth table, exact
ELF bytes, trailing-byte, and inherited resource mutations reject at their
owning responsibility. OMGRFN5 through OMGRFN9 remain live.

This tranche adds no structural equality, coercion, integer truthiness,
effectful eager evaluation, new resolver schema, public ABI, proof authority,
package authority, physical-target assurance, or build-lattice rung.

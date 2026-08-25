# OMGCOMP lower-rooted refinement witness, version 11

[`OMGRFN10`](OMGCOMP_REFINEMENT_WITNESS_V10.md) |
[`OMGRSW1`](../../../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION.md) |
[`OMGRSW2`](../../../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V2.md) |
[`OMGRSW3`](../../../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V3.md) |
[`CKIR9`](../../../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V9.md) |
[`CKIR9 backend`](../../../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V9_BACKEND.md)

`OMGRFN11` is the private lower-rooted carrier for pure, total, nontrapping
same-carrier unsigned ordered comparison. Exact source still selects the least
canonical OMGRSW1, OMGRSW2, or OMGRSW3; comparison syntax creates no new
resolution identity.

## Exact carrier

The 40-byte little-endian header uses magic `OMGRFNB\0` and version 11. It
carries exact OMGCOMP, selected OMGRSW1/2/3, CKIR9, and ELF bytes under the
inherited ceilings, EOF, entry/library, full-result, and exit-projection rules.
CKIR9 has schema major 9, minor 0, target 1 and requires opcode 19 `Greater`
and opcode 20 `GreaterEqual`. Each has a canonical Boolean result, two visible
operands of the same exact `u8` or `u32` carrier, and zero immediates. Boolean,
u64, structural, and cross-carrier operands reject. Inherited opcodes 15–18
remain valid but optional; all earlier schema/version pairings stay exact.

## Source, lowering, and meaning

The admitted tokens are standalone `>` and the exact pair `>=`. Transition
`->` is not a comparison, and `>=` is never double-counted as `>` plus `=`.
The refinement carrier uses a direct self `u8`/`u32` field on the authored left
and an in-range scalar literal on the right, making operand order independently
observable. Both operands must be pure, total, and nontrapping. Calls, indexing,
arithmetic, effects, and structural values are outside this tranche.

R4 reparses isolated ordered comparisons, preserves authored left/right order,
checks exact `>`/opcode-19 and `>=`/opcode-20 correspondence, evaluates all
u8/u32 true/false cases, and derives the full source result without ELF access.
Mixed comparison/equality/logical precedence remains primarily owned by the
CKIR9 producer gate rather than being claimed again here.

The conservative x86-64 templates for result `r` and operands `a,b` are:

```text
Greater:      8b 85 <disp32(a)> 3b 85 <disp32(b)> 0f 97 c0 0f b6 c0 89 85 <disp32(r)>
GreaterEqual: 8b 85 <disp32(a)> 3b 85 <disp32(b)> 0f 93 c0 0f b6 c0 89 85 <disp32(r)>
```

These use unsigned `SETA` and `SETAE`; signed `SETG`/`SETGE`, swapped operands,
or any alternate condition byte rejects.

## Independent responsibilities

1. R1 owns exact OMGRFN11 framing and complete OMGCOMP custody.
2. R2 owns least-resolution selection and exact non-arrow operator presence.
3. R3 joins the witness to CKIR9 and owns opcode-19/20 result-row envelopes.
4. R4 owns source token correspondence, operand order, numeric typing, purity,
   ordered-comparison meaning, lowering, and the complete source result.
5. R5 owns full CKIR9 validation/evaluation and exact ELF reconstruction.

Acceptance is the conjunction over one immutable frame. The primary carrier
must reach true and false results for both operators on both u8 and u32 before
continuing to exact result 70. Version, token/operator, type/arity/immediate,
operand order, condition-byte, component cross-pair, claim, trailing-byte, and
resource mutations reject at their owning responsibility.

This tranche adds no signed comparison, u64 comparison, coercion, structural
ordering, general effectful evaluation, new resolver schema, public ABI, proof
authority, package authority, physical-target assurance, or lattice rung.

# OMGCOMP lower-rooted refinement witness, version 9

[`OMGRFN8`](OMGCOMP_REFINEMENT_WITNESS_V8.md) |
[`OMGRSW1`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION.md) |
[`OMGRSW2`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V2.md) |
[`OMGRSW3`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V3.md) |
[`CKIR7`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V7.md) |
[`CKIR7 backend`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V7_BACKEND.md)

`OMGRFN9` is the private lower-rooted carrier for pure, total, nontrapping
Boolean `&&` and `||`. It freezes all earlier identities and responsibility
boundaries. It adds no resolution identity: exact source still selects the
least canonical OMGRSW1, OMGRSW2, or OMGRSW3 by the inherited direct-field-call
and pure-sum rule.

## Exact carrier

The outer 40-byte little-endian header is unchanged except for magic
`OMGRFN9\0` and version 9. It carries, in order, exact OMGCOMP, selected
OMGRSW1/2/3, CKIR7, and ELF bytes. Entry frames have flags 1, nonempty ELF, a
full u32 result, and `result & 255`; library frames have flags 0, empty ELF,
and `0xffffffff` in both result fields. Exact EOF, checked extent arithmetic,
and the inherited component and whole-frame ceilings are mandatory. Status 251
denotes malformed or relational rejection and 252 denotes resource rejection;
neither may publish output.

CKIR7 has schema major 7, minor 0, target 1. It inherits CKIR6 opcode 15
`LogicalNot` as optional and adds opcode 16 `LogicalAnd` and opcode 17
`LogicalOr`. Every CKIR7 admitted here contains at least one opcode-16 or
opcode-17 row. Both are binary, bool-typed operations over canonical zero/one
operands with zero immediates.

## Source and lowering relation

The admitted source syntax is ordinary infix Boolean syntax: `&&` binds tighter
than `||`, and each level associates left. Every operand has exact type `bool`.
For this tranche, each admitted operand is also independently proved pure,
total, and nontrapping. Calls and other potentially effectful, partial, or
trapping expressions are outside the boundary.

Source semantics remains short-circuit semantics. CKIR7 evaluates the selected
truth function eagerly. Those observations are equivalent only because the
checker proves that evaluating an otherwise skipped operand cannot introduce
an effect, divergence, or trap. This is a bounded admission rule, not a general
license to lower arbitrary short-circuit expressions eagerly.

The lowering owner reparses exact token pairs, independently rebuilds the
precedence/association opcode stream, and requires one distinct operation per
authored operator. Folding, swapping `&&`/`||`, reordering, or changing the
precedence tree rejects. The source-result owner independently checks types,
purity/totality/nontrapping, short-circuit truth meaning, and the exact full
source result.

The pinned target templates for operation result `r` and operands `a,b` are:

```text
LogicalAnd: 8b 85 <disp32(a)> 23 85 <disp32(b)> 89 85 <disp32(r)>
LogicalOr:  8b 85 <disp32(a)> 0b 85 <disp32(b)> 89 85 <disp32(r)>
```

Displacements are signed little-endian scalar-slot displacements. Canonical
Boolean zero/one makes bitwise AND/OR exactly the required truth functions.

## Independent responsibilities

1. **R1 custody** checks exact OMGRFN9 framing, bounds, mode/result shape, EOF,
   and complete OMGCOMP custody while treating later components as opaque.
2. **R2 resolution** independently selects and validates the least exact
   OMGRSW1/2/3 from source. Logical operators do not affect selection, and at
   least one exact `&&` or `||` token pair is required.
3. **R3 declarations/CKIR envelope** joins exact witness and CKIR7 identities,
   all inherited tables, and the opcode 15/16/17 structural envelopes. It
   requires at least one 16/17 but does not claim token correspondence or
   meaning.
4. **R4 source lowering/meaning** independently reparses bodies, types and
   purity, precedence and association, token-to-op no-fold correspondence,
   source result, and exact source-to-CKIR7 lowering. It has no ELF access.
5. **R5 CKIR meaning/artifact** fully validates and evaluates CKIR7, derives
   the exact result, and reconstructs the exact ELF from pinned templates. It
   has no source-body or witness-identity access.

Acceptance is the conjunction of all owners over the same immutable bytes.
Claim/result opacity, exact component identity, least-resolution cross-pairs,
source/witness, witness/CKIR7, CKIR7/ELF, and full-result joins remain mandatory.

## Required evidence and limits

The primary carrier is
`gates/fixtures/ckir7-logical-binary/general.omg`. It preserves the complete
OMGRFN7/8 payload-sum and OMGRSW3 composition surface, produces exact reachable
result 70, and makes the selected computation use both `&&` and `||`. Compact
OMGRSW1 and OMGRSW2 positives demonstrate that resolution remains generic.

Mutation teeth cover outer and CKIR versions, missing operators, operator swap,
operation order/precedence, purity escape, type/arity/immediate corruption,
ELF AND/OR bytes, component cross-pairs, result versus exit projection,
trailing bytes, and inherited resource boundaries. OMGRFN5 through OMGRFN8
positives remain live.

The carrier does not add integer truthiness, bitwise source operators, general
effects under eager lowering, a new resolver schema, public ABI, proof
authority, package authority, physical-target assurance, or a build-lattice
rung.

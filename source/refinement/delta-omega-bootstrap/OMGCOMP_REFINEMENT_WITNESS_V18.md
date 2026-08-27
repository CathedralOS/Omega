# OMGCOMP refinement witness v18

Status: private bootstrap-assurance contract for the bounded direct pure
same-carrier `u64 < u64` milestone. Product-source admission remains separate.

[`OMGRSW8`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V8.md) |
[`OMGLOWH`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_LOWERING_V17.md) |
[`CKIR16`](../../on-ramp/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V16.md)

OMGRFN18 is the fresh lower-rooted carrier for exact full-width unsigned `u64`
Less and its true-edge range custody. Its exact 40-byte little-endian header
uses magic `OMGRFNI\0`, outer version 18, flags 1, component extents, one
complete `u32` result, and its low-byte exit projection. The components are
exact OMGCOMP1, canonical OMGRSW8, CKIR16, and conservative Linux x86-64 ELF,
in that order. Every component is nonempty; component extents, inherited
ceilings, whole-frame ceiling, and EOF are exact.

OMGRSW8 normalized kind 10 with flags zero maps only to CKIR16 kind 8 with
flags zero. Both rows store inclusive lower and upper endpoints positionally as
`lower.lo32`, `lower.hi32`, `upper.lo32`, `upper.hi32`. Opcode 1 stores a `u64`
constant as `imm0 = lo32`, `imm1 = hi32`; opcode 9 remains Less. Semantic words
are never signed host integers or structural IDs.

## Admitted source and fact relation

The selected relation contains one or more pure direct same-carrier `u64 <
u64` expressions. Operands are direct typed machine/state binders, direct
typed field loads, or contextual decimal literals. Both operand values and all
interval endpoints may use all 64 bits. No arithmetic, mixed carrier,
coercion, computed/effectful operand, call operand, dynamic indexing, alternate
comparison, or `u64 in Trapping` spelling is admitted. The inherited rule that
multiple observable trapping arguments require an explicit order remains
normative; this pure relation does not weaken it.

For a direct left subject `x` and right interval `[rlo, rhi]`, the true edge
owns `x <= predecessor(rhi)`, intersected with the incoming interval for `x`.
The literal-focused profile has an exact right value, so `rlo = rhi`.
Predecessor is exact two-word unsigned subtraction, including borrow:
`0x00000002_00000000 - 1 = 0x00000001_ffffffff`. A fact exists only on the
true edge, only before its target arguments are checked, and only for the
direct subject identity. It may authorize a constrained same-carrier target
parameter and may then flow under that parameter's new identity. The false
edge receives no such fact. Mutation, a different binder, or an effectful or
computed operand invalidates custody.

The canonical focused join stores `0x00000001_ffffffff`, compares it with
`0x00000002_00000000`, carries the true-edge fact into a parameter bounded by
`0..=0x00000001_ffffffff`, transports that value through a direct call and
result, stores it, and returns 70. Boundary profiles additionally cover equal
high-word low-word ordering, bit 63, `MAX-1 < MAX`, equality at MAX, and
reversed high-word order. These are relation coverage, not a value allowlist.

## Responsibility ownership

- R1 owns OMGRFNI/version-18 identity, flags, component extents and ceilings,
  complete OMGCOMP1 custody, result/exit projection, and EOF.
- R2 owns OMGCOMP1 source closure to canonical OMGRSW8: exact identity,
  framing, dense tables, authored spans, selected root/owner, unqualified kind
  10, four endpoint words, named field/parameter links, and least selection.
  It reads neither CKIR nor ELF.
- R3 reconstructs CKIR16's producer-facing complete structure: kind 8,
  four-word intervals, two-word constants, opcode 9 operand/result types,
  dense values and visibility, 8-byte storage, ordered Call and edge vectors,
  inherited resources, and exclusion of sibling operations from this slice.
- R4-lowering independently reparses the selected source, joins operand order,
  literals, loads, storage, calls, and edges to CKIR16, and owns the source
  true-edge predecessor/intersection fixed point. Facts are deliberately absent
  from CKIR16 and cannot be inferred by R3 or R5. R4-source-result executes the
  source relation without reading CKIR or ELF and owns the exact result.
- R5-structure invokes the frozen independent CKIR16 semantics. R5-result owns
  independent CKIR execution and publication. R5-ELF owns exact conservative
  artifact reconstruction without invoking the production backend, including
  8-byte layout, `movabs` constants and bounds, 64-bit loads/stores, unsigned
  `cmp`/`setb`, call/edge scratch transport, branches, segments, and EOF.

Acceptance is the conjunction over one immutable frame. No owner imports
another owner's verdict. In particular, successful CKIR execution does not
establish the erased source fact relation, and exact artifact bytes do not
establish source or witness custody.

## Controls, resources, and lower-root evidence

Responsibility-local controls cover outer magic/version/flags/extents/EOF;
OMGRSW identity, policy, endpoint order and limb drift, stale source spans,
root drift, and cross-pairs; CKIR kind, flags, truncated/swapped limbs, signed
laundering, wrong constant word, mixed carrier, non-bool result, alternate
opcode, wrong storage/call/edge type, and retired major; source operand order,
effectful siblings, false-edge facts, predecessor borrow, off-by-one target,
wrong forwarded identity, mutation invalidation, omitted/folded/reordered rows,
and result drift; and ELF width, immediate, condition-code, slot, scratch,
range-check, branch, segment, truncation, and trailing-byte drift.

Each kind-8 value consumes one aligned 8-byte slot. Each constant consumes both
immediate words; each Less consumes one operation, two operands, one bool
value, and one expression-depth unit. Fact endpoint storage is bounded by the
inherited state count and uses packed existing arenas in the lowerer. Adjacent
frame, active-frame, table, operand, state, text, CKIR, ELF, and whole-frame
exhaustion select 252 without publication; malformed relations select 251.

The cheap oracle is split across eight Python entrypoints. Ten representative
persisted-Beta semantic-field projections split R4 source from CKIR and R5
CKIR from ELF. Each compiles under both persisted `bc` and self-produced `bc`,
requires identical assembly, accepts its assigned frame, rejects one local
mutation, and remains below the 262,140-byte tape ceiling; the observed maximum
is 145,462 bytes. These finite projections are lower-root lineage, not
whole-frame permutations or a replacement for the general Python relation.
The separate same-frame producer composite passes the canonical borrow profile
through native and self-compiled resolver/lowerer tools with exact OMGRSW8 and
CKIR16 parity, feeds those actual CKIR16 bytes to the production backend, and
passes the resulting immutable frame through all eight responsibility owners.
R5-ELF independently reconstructs the production artifact byte for byte. This
producer obligation remains distinct from the reference/Beta join; no
handcrafted carrier is presented as producer evidence.

This carrier does not widen Omega into general u64 arithmetic, mixed-width
comparisons, dynamic indexing, computed operands, additional order relations,
or a public integer ABI.

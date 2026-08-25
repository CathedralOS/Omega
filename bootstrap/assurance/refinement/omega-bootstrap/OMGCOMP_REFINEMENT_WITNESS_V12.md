# OMGCOMP refinement witness v12

Status: frozen private bootstrap-assurance contract.

OMGRFN12 is the independent refinement carrier for the CKIR10 `IntegerWiden`
slice. It is not a public Omega artifact. Its exact 40-byte little-endian header
is the existing refinement header with magic `OMGRFNC\0`, outer version `12`,
and the established flags/extents/result/exit fields. The frame contains, in
order, one exact OMGCOMP envelope, the least valid `OMGRSW1`, `OMGRSW2`, or
`OMGRSW3` witness selected by that source, exact CKIR schema `10.0`, and the
exact conservative Linux x86-64 ELF. Existing component and whole-frame bounds
apply; every component extent and EOF are exact.

The admitted increment is opcode 21 `IntegerWiden`: one visible exact-u8 value
(kind 1, flags/payload/low zero, high 255), no immediates, and one canonical
`u32 in Trapping` result (kind 2, flags bit 0 set, payload/low zero, high
2147483647). It corresponds one-for-one and in source order to authored
`as u32 in Trapping` after a pure, terminating, nontrapping exact-u8 field or
parameter leaf. Its value is unchanged. Narrowing, other source or target
carriers, other domains, literals, calls, indexing, mutation/effects, user
dispatch, implicit conversion, and broader casts are outside this tranche.

The selected immutable entry frame contains exactly three widenings carrying
0, 70, and 255, retains inherited scalar equality and payload-sum execution,
and claims final result 70. The backend sequence for each widening is the
ordinary source-value load, unsigned `MOVZX EAX, AL` (`0f b6 c0`), and the
ordinary result store. It must not sign-extend or elide that explicit
zero-extension.

Responsibility ownership remains conjunctive:

- R1 owns exact outer identity, component extents, bounds, and EOF.
- R2 independently reparses OMGCOMP, selects the least OMGRSW1/2/3 projection,
  and owns the exact cast target/domain tokens and canonical trapping-u32 type.
- R3 independently owns CKIR10 identity/table envelopes, opcode 21 result shape,
  and the three selected operations while preserving inherited table checks.
- R4 independently joins authored tokens to opcode-21 rows, exact operand/result
  carriers, and evaluates value preservation at 0/70/255 with final result 70.
- R5 independently validates opcode/type/visibility semantics, evaluates the
  unchanged values and result claim, and reconstructs the exact ELF including
  unsigned zero-extension.

Acceptance requires native and self-compiled Beta checkers to accept the same
immutable frame. Controls cover outer/schema/token/domain/type/opcode/operand/
result/instruction/claim/EOF/resource mutations. OMGRFN11 remains a required
regression. These conjuncts establish only the frozen CKIR10 widening slice;
they do not establish general casts, conversion policy, narrowing, arithmetic,
or full Omega correctness.

# OMGCOMP refinement witness v13

Status: frozen private bootstrap-assurance contract.

OMGRFN13 is the independent refinement carrier for the CKIR11 selected
canonical `u32 in Trapping` leaf-plus-literal addition relation. It is not a
public Omega artifact. Its exact 40-byte little-endian header uses magic
`OMGRFND\0`, outer version `13`, and the established flags, component extents,
result, and exit fields. The frame contains, in order, one exact OMGCOMP
envelope, the least valid OMGRSW1, OMGRSW2, or OMGRSW3 witness selected by that
source, exact CKIR schema 11.0, and the exact conservative Linux x86-64 ELF.
Existing component and whole-frame bounds apply; every component extent and EOF
is exact.

The admitted increment reuses opcode 8 `Add`: one direct field or parameter
leaf of the unique canonical `(u32, Trapping, 0..2147483647)` type, one authored
`+` token, and one anonymous nonnegative literal representable by that bridge
carrier. The result has the same exact canonical type. The carrier retains the
three inherited CKIR10 `IntegerWiden` operations and contains exactly four
selected additions: `2147483000 + 646`, the resulting near-limit value `+ 1`,
`0 + 70`, and `69 + 1`. It claims final result 70.

Literal-left, typed-right, nested, other-carrier, other-policy, domain-qualified,
and user-dispatched additions remain outside this tranche. Calls may carry at
most one potentially trapping argument, with pure, total, nontrapping siblings.
The bounded signed bridge carrier stops at 2147483647; this contract does not
claim the complete public Omega `u32` range.

Responsibility ownership remains conjunctive:

- R1 owns exact outer identity, component extents, bounds, and EOF.
- R2 independently reparses OMGCOMP, selects the least OMGRSW1/2/3 projection,
  and owns exactly four authored `+`-then-anonymous-literal token relations.
- R3 independently owns CKIR11 identity and table envelopes, the inherited
  CKIR10 widening relation, and exactly four candidate canonical-result
  opcode-8 envelopes.
- R4 independently joins authored tokens to those rows in source order,
  preserves the inherited widening joins, and evaluates source meaning at
  `0 + 70`, `69 + 1`, and near-limit `2147483646 + 1` with final result 70.
- R5 independently validates CKIR structure and trapping-add meaning, including
  operand order, carry and declared-range traps, result stores, final result,
  and exact conservative ELF reconstruction.

Acceptance requires native and self-compiled Beta checkers to accept the same
immutable frame. Controls cover outer-version and CKIR10 cross-pairs, authored
operator and near-limit mutations, opcode, operand, canonical type, ELF carry
branch, result claim, trailing bytes, and resource exhaustion. OMGRFN12 remains
a required regression.

These conjuncts establish only the frozen CKIR11 selected addition slice. They
do not admit general arithmetic, full-width `u32`, a public ABI, Delta syntax,
or any source facility to final `Ωself`.

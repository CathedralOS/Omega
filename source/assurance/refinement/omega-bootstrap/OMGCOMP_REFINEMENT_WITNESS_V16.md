# OMGCOMP refinement witness v16

Status: private bootstrap-assurance contract, frozen with the CKIR14 recursive
arithmetic producer and persisted-Beta lower-rooted milestone.

[`OMGRSW7`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V7.md) |
[`OMGLOWF`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_LOWERING_V15.md) |
[`CKIR14`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V14.md)

OMGRFN16 specifies the independent lower-rooted refinement carrier for recursive
full-width `u32 in Trapping` Add, Subtract, and Multiply. Its exact 40-byte
little-endian header uses magic `OMGRFNG\0`, outer version 16, and the
established flags, component extents, claimed result, and exit fields. The
frame contains, in order, exact OMGCOMP1, its canonical OMGRSW7, exact CKIR14,
and the exact conservative Linux x86-64 ELF. Every extent and EOF is exact;
the inherited component and whole-frame ceilings remain normative.

Flags are exact and version-local. Bit 0 means that the frame carries a
selected machine proposition. Bit 1 distinguishes a trapping proposition.
A successful proposition therefore uses flags 1, carries its complete `u32`
result (including the valid result `0xffffffff`), and carries `result & 255`
as its exit field. A trapping proposition uses flags 3 and carries
`0xffffffff` in both the result and exit fields as no-result sentinels. Flags
0, 2, unknown bits, and every other flag/result/exit combination are rejected.
This is private OMGRFN16 wire engineering, not a new Omega semantic rule.

OMGRFN15 and magic `OMGRFNF\0` are retired. They are not aliases, compatible
predecessors, alternate spellings, or decodable inputs to OMGRFN16. Outer
version, magic, OMGRSW selector, CKIR major, component bytes, claimed result,
and ELF must all agree; changing any subset never creates a valid cross-pair.
OMGRFN14 remains a separate frozen CKIR12 regression.

## 1. Admitted proposition

The source proposition contains at least one pure recursive expression over
the single exact full-range `u32 in Trapping` carrier. Leaves are direct typed
loads, contextual decimal literals, or the settled exact widening of a pure
direct `u8` leaf, and internal nodes are authored `+`, `-`, or `*` with the
OMGLOWF precedence, association, purity, context, and argument rules. Either
operand may recursively contain any selected operator. At least one arithmetic
node is required; a particular frame need not contain all three operators or a
widening.

The refinement family includes successful and trapping propositions for every
operator and at least one genuinely mixed nested tree. It exercises values on
both sides of the signed boundary and at `0xffffffff`. These are finite
coverage instances of the general typed expression/operation relation, not an
enumeration of accepted source files, exact operator counts, or compiler-text
permutations.

A successful proposition claims the exact source mathematical result and the
same CKIR and ELF result. For each internal node, Add traps when the
mathematical sum exceeds `0xffffffff`, Subtract traps when the left operand is
smaller, and Multiply traps when the product exceeds `0xffffffff`. A trapping
proposition claims the exact trap path and no result publication; it cannot be
converted into a successful proposition by choosing an arbitrary result
field. Parent nodes and dependent stores, calls, transitions, or returns are
unexecuted after a child trap.

The CKIR12 static-view family is optional. When source and CKIR include it,
OMGRFN16 proves its complete inherited literal, type, opcode, synthetic-edge,
partial-operation safety, source-result, and ELF relations alongside
arithmetic. A frame without view operations is equally valid and makes no
view proposition.

## 2. Conjunctive responsibility ownership

- R1 owns OMGRFNG identity, outer version 16, flags, checked component
  extents and ceilings, result/trap framing, complete OMGCOMP custody, and
  exact EOF.
- R2 independently reparses the source closure, reconstructs canonical
  OMGRSW7, validates the exact full-width normalized type and named leaves,
  and recognizes the general recursive pure same-carrier expression. CKIR,
  ELF, and the claimed result are not R2 proposition inputs.
- R3 independently validates the complete CKIR14 structure: full-width
  semantic words, every candidate opcode-8/26/27 row, dense values,
  visibility, exact opcode-21 widening custody where present, recursive
  dependencies, resources, and any optional complete CKIR12 view closure.
- R4-lowering joins every authored operator token to exactly one CKIR row in
  canonical postorder, joins every authored exact widening to opcode 21, retains
  left/right order, joins leaves and literals to their values, joins complete
  ordered Call and CaseDispatch argument vectors including pure siblings, and
  rejects folding, reassociation, omitted or invented rows, sibling drift, and
  source/witness/CKIR cross-pairs. R4-source-result evaluates the source tree
  without reading CKIR or ELF and owns its exact result or first trap.
- R5-structure independently validates CKIR14 against its frozen reference
  semantics. R5-result owns CKIR execution, per-node traps, nonpublication,
  and the selected machine claim. R5-ELF reconstructs the complete
  conservative artifact, including unsigned carry, borrow, high-product,
  branch-to-trap, result-store ordering, inherited view templates when
  present, headers, segments, and zero fill.

Acceptance is conjunctive. No responsibility may infer its proposition merely
because another checker accepted the same bytes, and the artifact is
reconstructed without invoking the production backend.

## 3. Controls and resource discipline

Controls include outer/OMGRSW/CKIR retired and cross-version pairs; high
semantic-word versus structural-ID confusion; source operator, precedence,
association, purity, leaf, literal, and carrier mutations; missing, extra,
reordered, reassociated, folded, or type-drifted CKIR operations; Add carry,
Subtract borrow, Multiply high-half, parent-after-child-trap, premature store,
and claimed-result mutations; optional-view closure and synthetic-edge
mutations; ELF instruction, branch, store, segment, and trailing-byte drift;
and inherited table, expression-depth, text, and whole-frame exhaustion.

Malformed framing or any failed source, resolution, CKIR, refinement, result,
or artifact relation selects 251 without publication. Declared resource
exhaustion selects 252 without publication. Runtime traps are validated
semantic outcomes, distinct from malformed-frame status 251, and expose no
result. All checkers size their bounded work before publication and do not
construct a whole compiler-file-shaped expected-output permutation table.

## 4. Closure evidence

The same-frame composite passes twelve producer-backed successful/trapping
profiles through all eight independent Python owners. The profiles cover Add,
Subtract, and Multiply; mixed and depth-eight trees; signed-boundary neighbors
and `0xffffffff`; exact widening; assignment, guard, Call, and CaseDispatch
contexts; inherited CKIR12 view composition; and first-trap outcomes. Local
Python controls cover complete OMGCOMP custody, stale source spans, equal-length
leaf substitution, ordered transition siblings, retired/cross-paired
components, claim opacity, exact ELF reconstruction, adjacent component
exhaustion, and whole-frame exhaustion.

Native and self-compiled owner assembly is identical. The default lattice mode
joins representative recursive, inherited-view, and trapping frames through
every persisted-Beta owner and gives each owner one rejection belonging to its
own responsibility. `OMGRFN16_MATRIX=exhaustive` additionally replays the
historical Cartesian native/self matrix for an intentional audit; it is not a
second closure obligation. This split retains the complete independent Python
relation and the persisted-Beta accept/reject joins without treating redundant
checker launches as new semantic coverage. The gate reports materialization,
producer, positive, Python-control, and persisted-Beta-control timings. Every
persisted-Beta owner remains within the 262,140-byte tape ceiling.

These conjuncts establish only the general compositional arithmetic bridge at
the CKIR14 frontier. They do not admit excluded source effects or carriers,
general optimization, a public ABI, or the facility to final `Ωself`.

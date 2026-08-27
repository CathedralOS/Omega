# OMGCOMP refinement witness v21

Status: private lower-rooted closure for the focused fixed-buffer capability
projection. The reference/Beta gate and the separate actual-producer
native/self same-frame gate pass. Product and package authority remain
separate.

[`OMGRSWA`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V10.md) |
[`CKIR18`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V18.md)

OMGRFN21 is the focused lower-rooted carrier for the `SourceUnit`-like
`append`/`byte_or_nul` capability needed by the next compiler-source checkpoint.
It is not a claim that the exact product `SourceUnit` is accepted: the selected
fixture deliberately omits the unrelated `SourceId` field and `clear(id)`
composition. It also grants no package, compilation, provider, or final
Omega-self authority.

The exact 40-byte little-endian outer header uses magic `OMGRFNL\0`, major 21,
flags 1, four component lengths, result 70, and exit projection 70. Components
are exact OMGCOMP1, OMGRSWA, CKIR18, and conservative Linux x86-64 ELF in that
order. Every component is nonempty and all component ceilings, extents,
whole-frame ceiling, and EOF are exact.

The canonical identities are:

- OMGCOMP1: 1,310 bytes, SHA-256
  `b7194afc8dfbd1b49c01524ac951b8146a7c67abe85913e8b190cd0bf0d1e089`;
- OMGRSWA: 1,376 bytes, SHA-256
  `7ee027659ff1da971055f3c659dc298f1cc5417048a3a89000872ec4ad568ae5`;
- CKIR18: 3,924 bytes, SHA-256
  `fd468683d3429eebccd700723f5f554ae586b245b7b6a9570caa5b57ed84a9bb`;
  and
- ELF: 8,192 bytes, SHA-256
  `83d5c09e1da6543a59514d0b1cff13e087032e3caafba39452268993a92ad0ce`.

These digests identify the canonical focused evidence. The relation itself is
semantic and admits the exact bounded reorder/rename family defined by
OMGRSWA; it is not a digest allowlist.

## Admitted relation and erased proof

The admitted projection owns one trapping fixed `[u8; N]`, with
`1 <= N <= 65,536`, one flags-zero retained length `u64 [0..=N]`, and one
Boolean status. Its lookup parameter is explicitly authored
`u64 in Trapping`, retained as OMGRSWA kind 10 flags 1 with the exact full-u64
four-word range. The retained length is a distinct kind 10 flags-zero row.
OMGLOWJ validates that source-policy distinction before both map to CKIR18
kind 8 flags zero: CKIR policies are operationally owned by IndexPlace and Add,
not copied source qualifiers.

Append first tests the direct retained-length leaf against literal `N`. Only
its true edge may store at `bytes[length]` and evaluate the authored Exact
expression `length + 1`. That edge proves `length <= N-1`; therefore the index
is in bounds, the addition has no carrier carry, and its result remains in
`0..=N`. Lookup first tests its direct index parameter against retained length.
Only its true edge loads `bytes[index]`; because `index < length <= N`, the
index is strictly below `N`. CKIR18 deliberately retains defensive unsigned
IndexPlace bounds checks, Add carry checks, and Add result-interval checks, but
all three are unreachable for admitted source execution.

The exact canonical exercise clears the buffer, appends byte 70, observes it
at index zero, forces length `N`, observes that append 71 takes the full path,
observes lookup `N` takes the absent path, and returns 70.

Computed or effectful indexes, mutable slices/views, indirect calls, recursive
calls, multiple observable trap sites, unrelated u64 arithmetic or relations,
cross-carrier operands, unguarded indexing, `N=65,537`, and additional
allocation are outside this relation.

## Responsibility owners

- R1 owns OMGRFNL/21 identity, flags, exact component identities and extents,
  inherited ceilings, complete OMGCOMP1 custody, result/exit 70, and EOF.
- R2 independently reconstructs the exact OMGCOMP1-to-OMGRSWA source relation:
  dense tables and authored spans, fixed array and record layout, distinct
  lookup-index/length policies and four-word endpoints, machine/state/call
  identity, guards, direct indexed Store/Load, Exact leaf-plus-literal Add,
  exclusions, selected root, and cross-pair rejection. It reads no CKIR/ELF.
- R3 reconstructs complete CKIR18 producer-facing structure. It owns the exact
  two kind-8 IndexPlace rows, one kind-8 Add, two kind-8 Less rows, fixed-u8
  array layout, values/places, operation/operand/terminator visibility,
  Store/Load and call typing, resources, and exclusion of sibling u64 ops.
- R4-lowering joins the exact source and witness identities to CKIR18 field,
  machine, operand-order, guard/true-target, indexed access, Add, interval, and
  policy-erasure identities. It owns the erased true-edge safety proof above.
  R4-source-result executes the selected source relation without CKIR or ELF
  and owns result 70.
- R5-structure invokes the frozen independent CKIR18 meaning. R5-result owns
  CKIR execution/result 70. R5-ELF independently reconstructs every artifact
  byte without invoking the production backend. It explicitly imports the
  modular OMGRFN18 qword owner for constants, loads/stores, Less, calls, edge
  transport, layouts, ranges, and ELF orchestration, then adds only qword
  IndexPlace and Add templates: imm64 unsigned JAE bounds, qword address add,
  unsigned carry JB, full-u64 result range, and qword result storage.

Acceptance is the conjunction over one immutable frame. No responsibility
imports another owner's verdict. In particular, R3 structure does not prove
authored operand order or erased range facts, and successful CKIR execution
does not prove exact ELF bytes.

## Controls, resources, and lower-root evidence

Responsibility-local controls cover outer/component identity, extents, EOF,
result and all declared ceilings; lookup-index and retained-length policy
drift, endpoint/array-length drift, stale spans and valid-envelope cross-pairs;
IndexPlace/Add/Less opcode, immediate, result, carrier, visibility and resource
drift; leaf-plus-literal operand reordering that remains structurally valid;
source/result drift; runtime full-u64 carry, result-interval and high-half index
traps; and ELF immediate, unsigned condition, qword Add/Less, truncation and
trailing-byte drift. `N=65,537` is semantic malformed 251, not exhaustion 252.

The cheap reference gate runs eight modular Python owners and five
representative persisted-Beta projections. Each projection compiles under both
persisted `bc` and self-produced `bc`, requires byte-identical assembly,
accepts its assigned canonical frame, rejects one responsibility-local byte,
publishes nothing on rejection, and remains under the 262,140-byte tape
ceiling. The observed tapes are R1 45,663; R2 205,730; R3 220,115; R4 243,269;
and R5 253,379 bytes. These finite projections establish persisted lower-root
lineage for representative semantic fields; they do not replace the complete
Python relation or serve as producer evidence.

The separate actual-producer same-frame gate compiles the focused resolver,
OMGLOWJ lowerer, and CKIR18 backend through both native Delta and the
self-produced lowermachine path. It checks the five accepted source variants,
exact native/self OMGRSWA, CKIR18, and ELF identity, an actual accepted-witness
cross-pair, one no-publication resource tooth per producer phase, an artifact
byte tooth, and passes the unmodified canonical components through every
R1-R5 owner. The handcrafted reference remains separately useful for local
permutation coverage; it is not substituted for this producer evidence.

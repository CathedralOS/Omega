# OMGCOMP refinement witness v14

Status: private bootstrap-assurance contract, frozen with the CKIR12 producer
milestone.

OMGRFN14 is the independent lower-rooted refinement carrier for CKIR12's
program-static shared `&[u8]` relation. Its exact 40-byte little-endian header
uses magic `OMGRFNE\0`, outer version 14, and the inherited flags, component
extents, result, and exit fields. The frame contains, in order, exact OMGCOMP,
exact OMGRSW4, exact CKIR12, and the exact conservative Linux x86-64 ELF.
Every extent and EOF is exact; inherited component and whole-frame ceilings
remain normative.

Two producer-backed entry carriers are conjunctive:

- the one-byte carrier materializes exact literal byte `0x46` (`"F"`), takes
  the nonempty true edge, computes head and one-byte tail, proves that tail
  empty, and returns 70; and
- the empty carrier materializes no payload byte, takes the authored false
  bypass without executing head/tail, and returns 70.

Both are produced from the frozen OMGLOWD version-13 frame and OMGRSW4. CKIR12
has exact kind-7 shared-slice identity over canonical full-range `u8`, one
opcode 22 `StaticByteView`, one opcode 23 `SliceNonEmpty` operation, one
opcode 24 `SliceHead`, one opcode 25 `SliceTailOne`, and exactly one bit-0
synthetic true-edge block. The synthetic block has one predecessor, receives
the exact slice tested by the predecessor, contains only head/tail over its
parameter 0, and jumps once to an authored non-synthetic block.

Responsibility ownership remains conjunctive and bounded:

- R1 owns exact outer identity, component extents and ceilings, entry/result
  framing, complete OMGCOMP custody, and EOF.
- R2 independently recognizes the exact one-byte or empty source carrier and
  validates OMGRSW4 identity, complete table extent, unique kind-7/full-`u8`
  identity, and at least one machine/state slice parameter. CKIR, ELF, and the
  claimed result are not proposition inputs.
- R3 independently binds OMGRSW4 to the complete canonical CKIR12 reference
  carrier, including literal DAG bytes, types, operations, values, blocks, and
  the synthetic edge.
- R4-lowering joins the exact authored literal profile to the exact CKIR12
  profile produced by OMGLOWD. R4-source-result reads neither CKIR nor ELF and
  independently owns the two source paths' result 70.
- R5-structure validates the complete CKIR12 bytes against the independent
  frozen reference model. R5-result owns the CKIR execution/result claim.
  R5-ELF reconstructs the conservative ELF header, code template, descriptor
  length, guarded head/tail sequence, static literal segment, and zero fill.

The R5 artifact template is finite and named rather than permutation-driven:
the two artifacts differ only in the static-view length immediate and the
single read-only literal byte. It is materialized without invoking the backend
and compared byte-for-byte by separately native- and self-compiled persisted-
Beta checkers.

Controls include outer/CKIR version cross-pairs, source-literal/CKIR profile
crossing, OMGRSW4 kind-7 and payload mutation, CKIR literal/opcode/synthetic-
flag/edge mutation, ELF descriptor-length and static-byte mutation, result
claim drift, trailing bytes, and whole-frame exhaustion. The CKIR12 reference
and backend gates plus OMGRFN13 remain required regressions.

These conjuncts establish only the frozen CKIR12 static shared-byte-view slice.
They do not expose a pointer ABI, make address identity observable, admit
mutable/dynamic slices, general indexing/subslicing, general string syntax, or
the facility to final `Ωself`.

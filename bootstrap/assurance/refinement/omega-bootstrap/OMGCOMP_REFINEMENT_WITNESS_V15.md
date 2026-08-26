# OMGCOMP refinement witness v15

Status: private bootstrap-assurance contract, frozen with the CKIR13 producer
milestone.

OMGRFN15 is the independent lower-rooted refinement carrier for direct
full-width `u32 in Trapping` subtraction. Its exact 40-byte little-endian
header uses magic `OMGRFNF\0`, outer version 15, and the inherited flags,
component extents, result, and exit fields. The frame contains, in order,
exact OMGCOMP, exact OMGRSW5, exact CKIR13, and the exact conservative Linux
x86-64 ELF. Every extent and EOF is exact; inherited component ceilings and
the 4,497,544-byte whole-frame ceiling remain normative.

The successful producer-backed carrier assigns `4294967295` and
`4294967290`, computes their exact difference 5, tests that result, and
returns 70. The companion underflow carrier computes `0 - 1`. It has the same
valid structural custody and borrow-to-trap artifact relation, but CKIR
execution traps. It is therefore a control, not an admitted result-70
carrier: both independent result owners reject it with semantic failure 251.

Responsibility ownership is conjunctive and bounded:

- R1 owns exact outer identity, component extents and ceilings, entry/result
  framing, complete OMGCOMP custody, and EOF.
- R2 independently recognizes the exact success or underflow source profile
  and validates OMGRSW5 identity, complete table extent, and canonical
  full-width `u32 in Trapping` identities. CKIR, ELF, and the claimed result
  are not proposition inputs.
- R3 independently binds OMGRSW5 to the complete canonical CKIR13 reference
  carrier, including all tables and exactly one opcode-26 subtraction.
- R4-lowering joins each exact authored source profile to its exact CKIR13
  lowering. R4-source-result reads neither CKIR nor ELF and independently
  owns the source execution/result claim; it accepts the successful carrier
  and rejects underflow.
- R5-structure validates complete CKIR13 bytes against the independent frozen
  reference model. R5-result owns CKIR execution and the result claim; it
  accepts the successful carrier and rejects underflow. R5-ELF reconstructs
  the complete conservative ELF bytes, including loads, subtraction,
  borrow-to-trap checks, range checks, stores, and zero fill.

The R5 artifact model is finite and byte-complete. It is materialized without
invoking the backend and compared byte-for-byte with fresh producer output.
Every R1--R5 checker is compiled both by the native Beta compiler and its
self-compiled successor; the generated assembly must agree exactly and each
checker tape remains under the inherited ceiling.

Controls include outer-version drift, trailing bytes, whole-frame exhaustion,
source literal drift, OMGRSW5 version drift, CKIR version/opcode mutation, ELF
`sub`-to-`add` mutation, result-claim drift and opacity, and source-to-witness,
witness-to-CKIR, source-to-CKIR, and CKIR-to-ELF cross-pairs. Fresh producer
equality covers both success and underflow witnesses, CKIR, and complete
8,192-byte ELF artifacts. OMGRFN14 remains the required predecessor
regression.

These conjuncts establish only the frozen direct full-width subtraction
slice. They do not admit nested subtraction inside multiplication or addition,
infer a general Delta `u32` surface, or settle any broader facility's final
`Omega-self` disposition.

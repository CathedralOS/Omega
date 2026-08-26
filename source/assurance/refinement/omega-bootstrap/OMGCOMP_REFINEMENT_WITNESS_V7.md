# OMGCOMP lower-rooted refinement witness, version 7

[`OMGRFN6`](OMGCOMP_REFINEMENT_WITNESS_V6.md) |
[`OMGRSW3`](../../../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V3.md) |
[`CKIR5`](../../../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V5.md)

`OMGRFN7` is the private lower-rooted carrier for the first payload-bearing
pure-sum tranche. It preserves the five independent responsibility owners and
adds only the relations required by `OMGRSW3`, `OMGLOW6`, and `CKIR5`.
OMGRFN5 and OMGRFN6 remain frozen.

## Version-7 frame

The outer 40-byte little-endian layout and component ceilings are unchanged:

```text
offset  width  field
0       8      magic: ASCII "OMGRFN7\0"
8       u32    version: 7
12      u32    flags: 0 library, 1 entry-bearing compilation
16      u32    exact OMGCOMP byte length
20      u32    exact OMGRSW3 byte length
24      u32    exact CKIR5 byte length
28      u32    exact ELF byte length
32      u32    claimed full result
36      u32    claimed exit projection
40      ...    OMGCOMP || OMGRSW3 || CKIR5 || ELF || exact EOF
```

Ceilings remain 267,280 OMGCOMP bytes, 524,288 OMGRSW3 bytes, 2,522,192 CKIR5
bytes, 1,183,744 ELF bytes, and 4,497,544 bytes for the complete frame. Exact
length, flags, entry/library, checked-addition, status, publication, and EOF
rules are inherited.

Carrier identities pair exactly: OMGRFN5 with OMGRSW1/CKIR4, OMGRFN6 with
OMGRSW2/CKIR4, and OMGRFN7 with OMGRSW3/CKIR5. Every cross-pair rejects at the
responsibility that owns that join. An untrusted packer creates bytes but grants
no proposition.

## Responsibility-local propositions

1. **Frame and source custody** accepts exact OMGRFN7 framing and independently
   reconstructs OMGCOMP. OMGRSW3, CKIR5, ELF, and claimed result remain opaque.
2. **Resolution** independently reconstructs canonical OMGRSW3 from exact
   source: pure-sum declarations, declaration-order cases, named payload fields,
   normalized nominal sum types, copyability prerequisites, contextual
   constructor/arm identities, the least-version rule, and every stated
   exclusion. CKIR and ELF remain unavailable.
3. **Declarations and intrinsic CKIR structure** joins OMGRSW3 to CKIR5 sums,
   cases, payload fields, nominal types, copyability/acyclicity, bridge-private
   layout, opcode-14 envelopes, CaseDispatch arms, and selected-edge argument
   identities while retaining every inherited declaration/root/table relation.
4. **Source lowering and source meaning** independently reparses exact bodies
   and reconstructs construction, Copy/Call transport, exact-case dispatch,
   selected payload binding, target arguments, and the source result without
   CKIR/ELF access. Artifact-free lowering separately reconstructs exact CKIR5.
5. **CKIR meaning and artifact reconstruction** remains source-body and witness-
   identity opaque. It independently validates complete CKIR5, derives active-
   payload meaning and exact result, and reconstructs the exact Linux x86-64 ELF.

The five labels remain responsibility boundaries, not a requirement that every
conjunct fit one monolithic executable. A version-7 owner may split unchanged
core and sum-specific checks into multiple bounded executables over the same
immutable frame. No split may share a producer verdict, skip a cross-pair, or
move evidence to an owner that cannot independently observe it.

## Required same-frame evidence

One immutable positive frame must make its result depend on a nonzero case tag
and bound payload and must exercise payload-free and payload-bearing cases,
one-to-four recursively copyable payload fields, nested aggregate payloads,
runtime construction, Copy, a structural Call argument, parameter and `self`
field dispatch, and exact result 70. Every R1/R2/R3/R4/R5 conjunct runs native
and Delta-self-built over those same bytes.

Phase-local mutations and distinct source/witness, witness/CKIR, CKIR/ELF, and
result cross-pairs must prove the ownership matrix. R1 and R5 remain witness-
identity opaque; R2 has no CKIR/ELF access; R3 owns witness/CKIR declaration and
intrinsic joins but not body meaning; R4 owns source body meaning without ELF;
R5 owns CKIR/result/ELF without source-body authority. Old V5/SW1 and V6/SW2
positive and separation controls remain live.

## Non-expansion

OMGRFN7 proves only the bounded pure-sum relation specified by OMGRSW3 and
CKIR5. It grants no package acceptance, public ABI, final `Ωself` admission,
explicit-discriminant meaning, evaluation-order ruling, mixed/generic sum
meaning, proof authority, Terminal-Psi dependency, or additional build rung.

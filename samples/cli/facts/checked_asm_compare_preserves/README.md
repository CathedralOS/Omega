# Checked comparison preserves its operands

Comparing a register with itself establishes the zero flag without changing its value; both label edges preserve the original register.

Owning requirement: **CHECKED-ASM-LABEL-CONTRACTS**, ordinary flag production and labeled control through checking, Terminal Psi and native execution. The same comparison uses `eax` with `jz` on x86-64 and `w0` with `b.eq` on AArch64. Every place binds all four hosted targets and declares expected exit 0. The bound is 37 throughout.

The impossible fallthrough explicitly requires `false`. This is an incoming-edge goal which the caller must prove from the comparison; it is never an admission. Label bodies still owe their outgoing obligations under their declared contracts. The final label explicitly ensures `output == 37`; extraction must preserve that checked goal through the ordinary helper call before the enclosing result contract can consume it. Original unproved source and its diagnostic are preserved in local checkpoint `d09e4a4d48a` and the task evidence; no runtime oracle was weakened.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [wrong preserved value](local/invalid/main.omg) |
| field | [program](field/main.omg) | [wrong preserved value](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [wrong preserved value](parameter/invalid/main.omg) |
| call result | [program](call_result/main.omg) | [wrong preserved value](call_result/invalid/main.omg) |
| match join | [program](match_join/main.omg) | [wrong preserved value](match_join/invalid/main.omg) |
| const name | [program](const_name/main.omg) | [wrong preserved value](const_name/invalid/main.omg) |
| other state | [program](other_state/main.omg) | [wrong preserved value](other_state/invalid/main.omg) |
| other machine | [program](other_machine/main.omg) | [wrong preserved value](other_machine/invalid/main.omg) |
| target body | [program](target_body/main.omg) | [wrong preserved value](target_body/invalid/main.omg) |

The same sources are pinned as `tests/omega/{pass,fail}/facts/checked_asm_compare_preserves_through_*`. A reject changes only the taken label's register preservation contract, from 37 to 0. The comparison and all instructions stay unchanged.

## Requirements and evidence

- Establishment: initialization establishes the register value; comparison establishes its zero flag without assigning the result to either operand. Incoming edge goals are proved, including the impossible edge.
- Identity and lifetime: comparison has a separate private result. Register versions and checked output places remain distinct; no scratch state crosses an assembly-body boundary.
- Composition: ordinary result and parameter contracts transport 37 through the nine places. Both match arms execute. The consuming label/return contract establishes the fact before the final runtime oracle observes it.
- Soundness: mixed ISA operands, unsupported register aliases, uninitialized registers, wrong target bodies, false label goals and overwritten/stale output goals have separate rejection controls. Helper postconditions are ordinary proof goals on every normal return.
- Psi 05–07: scalar computation, explicit zero-flag edges and captured output updates use ordinary Terminal operations, with native/Terminal agreement required on the matching host.
- Omega 00–02: recorded catalog target demands survive graph copying and helper extraction; four target-scoped implementations retain the same source contract.
- Omega 03–06: ordinary scalar and calling plans realize the private arithmetic result, edges and output writeback. Stack/frame/link/zero and wide aliases remain outside this register cohort.
- Omega 07–09 and emission: focused execution must validate the authored exit in both engines. Nonmatching hosts are not claimed as native execution evidence.

The [upper AArch64 register probe](arm_probe/main.omg) exercises the same fact at `w28`, beyond a 32-bit definite-initialization mask. The one-block conditional fixture tests a normal exit directly from a conditional fallthrough.

## Remaining acceptance

Validation is recorded in the task's qualification/evidence ledger. The original
scalar comparison repair is published; the modern nested-machine witness and
closed saved-subject production are being qualified. Other flag-producing catalog families, AArch64 countdown parity,
privileged effect settlement, supplied-environment interpretation, and the
integration canaries remain requirements of the full parent task. Completed,
reviewed repairs may be published through the normal PR and merge queue.


The `saved_subject/` production in each of these same nine places preserves the
same CMP/37 fact through a plain record. The final label authors the goal
`match output { AsmSavedOutput { value } -> value == 37 }`; the enclosing
machine still owes `result == 37`, and the original runtime oracle is retained.
Each new reject twin changes only that match goal to 38. The original scalar
production and reject twins remain controls. Both productions are pinned under
`tests/omega/{pass,fail}/facts/`.

This exercises closed logical subject custody: the copied goal retains its own
match definition, subject, nominal pattern head, and alias anchor. Declaration,
label exit, normal helper return, and caller consumption independently resolve
the same readable observation at their own proof frontier. No match goal or
label assertion supplies an assumption. This closed production alone does not establish transport with a separate
scalar observation or close the full board item.

The other-state production uses a nested ordinary machine with an explicit
computed argument, preserving the original 37 bound and success/failure oracle.
The shared nested-declaration prerequisite retains lexical identity before
flattening; it supplies no enclosing runtime capture.

## Captured seed in a label goal

The `ambient_capture/` production in each standard place keeps the comparison,
seed 37, output record, four target bodies and original exit-0 oracle. The
last label requires `output.value == seed` and ensures the same relation through
`match output { AsmSavedOutput { value } -> value == seed }`. The incoming
relation justifies that goal; the goal is never a premise. Both the saved
record subject and the ordinary captured scalar must keep their exact values
and storage versions when the label is extracted into an ordinary helper.

The optimized fixed baseline `ccc9bf848faf12acabb9762ec570c7572b90cc5d` rejects
this valid label goal in all four target bodies with `cannot prove asm ensures
fact at block exit`. The closed literal-37 production is retained as a control;
its historical Linux x86-64 run reports native exit 0 and interpreter exit 0.
Fresh qualified observations and exact producer identities remain in the
implementation evidence; historical observations are not results from a newer
compiler.

Every positive and its twin differ only in the last label's equality: the twin
requires the observed value to differ from the captured seed. The written
control changes the output after the label's entry requirement and must reject
the stale postcondition. No caller guard establishes either assembly goal.

| Place | Captured-seed production | False-goal twin |
| --- | --- | --- |
| local | [program](local/ambient_capture/main.omg) | [reject](local/ambient_capture/invalid/main.omg) |
| field | [program](field/ambient_capture/main.omg) | [reject](field/ambient_capture/invalid/main.omg) |
| parameter | [program](parameter/ambient_capture/main.omg) | [reject](parameter/ambient_capture/invalid/main.omg) |
| call result | [program](call_result/ambient_capture/main.omg) | [reject](call_result/ambient_capture/invalid/main.omg) |
| match join | [program](match_join/ambient_capture/main.omg) | [reject](match_join/ambient_capture/invalid/main.omg) |
| const name | [program](const_name/ambient_capture/main.omg) | [reject](const_name/ambient_capture/invalid/main.omg) |
| nested machine | [program](other_state/ambient_capture/main.omg) | [reject](other_state/ambient_capture/invalid/main.omg) |
| another machine | [program](other_machine/ambient_capture/main.omg) | [reject](other_machine/ambient_capture/invalid/main.omg) |
| target body | [program](target_body/ambient_capture/main.omg) | [reject](target_body/ambient_capture/invalid/main.omg) |

The sources are pinned as
`tests/omega/{pass,fail}/facts/checked_asm_compare_preserves_ambient_capture_through_*`.
The overlapping-write control is [written](local/ambient_capture/invalid_written/main.omg),
pinned as `fail/asm/checked_asm_ambient_capture_written_rejected`.

Establishment, identity and composition use the same independently proved label
requirement, ordinary explicit arguments and match-owned record subject. The
current-predicate binder observes the readable referent carrier of a whole
primitive loan while retaining its original place and live storage cut;
write-only references provide no readable carrier. Both state contracts and
current observations use that judgment. No capture producer is replayed, no
permission is widened, and writes still invalidate overlapping observations.

The existing Terminal lowering, provider and calling plans, instruction/frame
realization, encoding and native emission carry the unchanged program. Linux
x86-64 dual-engine runs establish execution on that host; the other hosted
targets are declared and checked, with matching-host execution still unverified.
This production addresses the captured-scalar goal frontier of
**CHECKED-ASM-LABEL-CONTRACTS**; the task's remaining catalog families, AArch64
countdown parity, effect retirement, privileged settlement, supplied-environment
interpretation and integration canaries remain open.

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

Validation is recorded in the task's qualification/evidence ledger. This source row is not yet declared complete. `other_state` still uses the compiler's existing legacy `state` spelling and does not establish the ratified nested-machine migration. Saved-subject snapshot goals retain their source assertions but are not exported without exact capture custody; that composition remains open. Privileged effects, catalog families beyond this comparison, AArch64 countdown parity, environment interpretation, and the full parent task remain open. No merge or publication is authorized.

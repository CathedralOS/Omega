# Checked assembly countdown

The checked seed-2 mov/sub/jnz CFG falls through with eax == 0 and stores output == 0.

The root is the original assigned reproduction. Each child carries the same
seed, instructions, loop contract and fallthrough contract. Its `invalid` twin
changes only `done requires eax == 0` to `done requires eax == 1`.
The initial output is 99, so skipping the assembly cannot satisfy the runtime
oracle. The match witness invokes both branches. The target witness has Linux
and Windows x86-64 bodies; only the matching Linux body executes here.

| Place | Positive | Reject twin |
|---|---|---|
| local | `local/main.omg` | `local/invalid/main.omg` |
| field | `field/main.omg` | `field/invalid/main.omg` |
| parameter | `parameter/main.omg` | `parameter/invalid/main.omg` |
| call result | `call_result/main.omg` | `call_result/invalid/main.omg` |
| match join | `match_join/main.omg` | `match_join/invalid/main.omg` |
| const name | `const_name/main.omg` | `const_name/invalid/main.omg` |
| another state | `other_state/main.omg` | `other_state/invalid/main.omg` |
| another machine | `other_machine/main.omg` | `other_machine/invalid/main.omg` |
| another target body | `target_body/main.omg` | `target_body/invalid/main.omg` |

Authority is [assembly](../../../../wiki/spec/language/assembly.md#blocks-and-assertions):
every incoming edge, including the backward edge and conditional fallthrough,
proves its destination contract against the exact modeled state. Label assertions
never supply admission. Ordinary checked state/call and arithmetic contracts
remain authoritative through normalization.

Initial failure at main `7875d3411f26`, before compiler changes: line 10 rejects
`sub` as an unknown instruction. Acceptance uses `omega run --both` for distinct native and admitted Terminal
observations. The nine source pairs are pinned under
`tests/omega/{pass,fail}/facts/checked_asm_countdown_<place>_*`.


All nine positives execute with native exit 0 and admitted Terminal exit 0 on
Linux x86-64. Every twin rejects `eax == 1` at the conditional fallthrough into
`done`; the failing operation is the destination contract in `loop`. The source
pair audit verifies that this is each twin's only source change. Both match
arms execute, and Windows x86-64 source bodies are checked here without claiming
Windows execution.

The compiler decodes fixed 32-bit GPR moves/subtraction and explicit ZF branches
into immutable value versions and ordinary checked state edges. Each helper
retains the exact borrowed roots, entry-state call identity, direct service row,
termination promise and catalog architecture requirement. Source contracts and
ordinary ownership checks still decide every edge. A false stronger backedge
(`eax > 1`) rejects. Guarded unsigned nonzero observations retain their ordinary
`>= 1` consequence; signed/interior exclusions keep their original meaning.

Focused regressions cover modular underflow, preserved/latest ZF, exact GPR and
RFLAGS clobbers, uninitialized register/flag/index reads, separate bodies,
unsupported aliases/hardware flag mixing, missing ranks, target sibling capture
identities and incompatible targets. Valid boundary operator declarations cannot
redefine catalog subtraction or ZF: this bounded cohort explicitly refuses
non-builtin selected meanings.

The parent board still owns other ISA/register views, indexed memory/address
composition, hardware flag observations, ranking-witness transport and exact
linear hardware-effect retirement. This fact establishes the assigned mov/sub/jnz
cohort across nine places; those broader assembly obligations remain open.

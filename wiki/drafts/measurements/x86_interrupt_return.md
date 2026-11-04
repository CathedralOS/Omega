# Returning from an Omega interrupt handler

An Omega handler now runs and returns to interrupted x86-64 code without losing
the checked register, flag or stack values. Two compiled programs each completed
eight software interrupt deliveries in QEMU: one writes a marker directly,
and one calls a separate helper. This completes the tested entry/return repair;
Cathedral's hardware timer and installed-root path remain unfinished.

The original compiler rejected a physically demanded interrupt entry with
`native entry bridge failed: IncompatiblePlan`. The frozen zero-argument marker
program reproduced that failure on base
`6299adddf42d3fda8e68d56854fb729d30f814ca` in 31.926 seconds. The separately frozen
Cathedral acknowledgement program reached the same failure in 34.517 seconds.
The baseline CLI was optimized at level 1, SHA-256
`ce6441d7cd25180481de9bfa3b7cc8ce8ae97ca4e2cc9d2a4b632d7ad8da2396`.

## What the repair establishes

The compiler keeps the complete entry plan through native emission. It derives
the handler's actual machine-state use from retained instruction/frame effects
and follows reached checked helpers. Instruction numbers are local to each
function; the effects join therefore retains both function and instruction.
The adapter saves all fifteen ordinary integer registers, clears DF for the
ordinary compiled body, restores the registers, and returns with `iretq`.
The CPU's unchanged five-word frame restores flags, IP and SP. Semantic stack
arguments start after that frame.

The entry refuses vector/control state it cannot restore. A masked handler
cannot enable interrupts or restore unconstrained flags; opaque callees must
promise masked execution. An unknown callee or missing effect record rejects.
These checks leave ordinary callable entry generation on its existing path.

## Execution and rejection evidence

The corpus producer is optimized at level 1, SHA-256
`0806307f87af10efbfc7c555f07788e4885fed6c71d20aa578f05f12fe657822`.
Its exact base plus changed compiler inputs are recorded in the claim's
`build/timer-entry-execution/corpus-v2-start.json`. Raw images, logs and receipts
remain in that ignored evidence directory. The source fixtures and
[CPU harness](../../../tools/native_entry_cpu/) retain the replay inputs.

| Witness | Observed result |
| --- | --- |
| `pass/build/interrupt_return_debug_marker` | Eight returns; exact `BT.T.T.T.T.T.T.T.P`, QEMU exit 33. |
| `pass/build/interrupt_return_helper_debug_marker` | Same result; two retained body functions and one actual internal call. |
| `fail/build/interrupt_return_vector_helper` | The reached floating-point argument/result transport exceeds the entry's saved machine state. |
| `fail/build/interrupt_return_missing_gpr_restore` | The plan permits interrupted state it does not save. |
| `fail/build/interrupt_return_masked_enable` | The masked handler can enable nested interrupts. |
| `pass/build/interrupt_return_cathedral_ack` | Emits the actual 40-byte acknowledgement parameter and checked PIC completion body; not invoked. |

The direct and helper entries are at `0x410620` and `0x410630` respectively.
Those addresses come from the emitted object's exact private-entry record and
image placement. The harness copies the original ELF load segments at their
original addresses; it supplies no replacement handler or return wrapper.
An independent review checked the ELF-to-payload copy, embedded payload,
entry coordinates, hashes and raw debug output.

Every return checks all fifteen non-SP general registers, exact SP, status/DF/IF
flags, sixteen XMM registers, MXCSR and guards around both stacks. Harness
controls detect a clobbered register, an ordinary return substituted for `iretq`,
and a missing body marker. Source tests, encoder/state unit checks and scoped
Clippy also pass. Preliminary vector controls that failed assembly formation or
unrelated floating-point selection are retained as setup failures, not evidence.

The original direct payload SHA-256 is
`e59629533603e3d4c64028ebf0a1757dabc48be584f14d9f9c60838459dea7af`;
the helper payload is
`5bb6834c154afee0894b3486321abf2323081e80184d51a0e4aa69586b46d832`.
Each QEMU execution took about 0.1 seconds. The six-fixture source qualification
uses the normal corpus runner and its opt-in artifact capture. A separate Cargo
driver began rebuilding dependencies and was abandoned; reusing the existing
runner avoided that second build graph.

## Remaining Cathedral work

Cathedral's fresh boot baseline still passes all fifteen ordered markers through
`Mapping checked`, including the UEFI handoff and retained memory. Its report is
`build/timer-boot-review-audit/REPORT.md` in the Cathedral worktree.

The timer still needs its installed table, owned stacks and retained wake state;
actual arrival arguments and exact acknowledgement/completion association;
PIC/PIT delivery; repeated hardware returns; and an observable wake in ordinary
kernel code. A materialized callback address alone establishes none of these.
The tested adapter assumes an admitted normalized no-error-code arrival frame;
error-code normalization, nesting, privilege transitions and broader machine-state
save protocols are not qualified here. This repair does not close
`EXCEPTION-ROOTS-AND-TIMER` or the user's kernel milestone.

# Nominal predicates at assembly label goals

Parent outcome: [CHECKED-ASM-LABEL-CONTRACTS](../../../../TASKS.md).

An admitted nominal length predicate fixes the selected value at seven and proves the assembly move stores seven.

The root production in each of the nine places uses `Counted where length == 7`. The move consumes the nominal predicate at the current statement prefix. `snapshot` creates the selected value before a later argument resets the cursor from two to zero. Replaying the producer at that later cursor would yield nine. Each root reject twin changes only the label Requires from seven to nine; it retains the move, the Ensures and the runtime oracle. The constant placement is a regression control which already checked on the predecessor; parameter and another-machine placements demonstrate the repaired proof failure.

The original `value_indexed/` production is complete across all nine places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), with matching reject twins. It uses `Counted<Count: u64>` and records the exact selected runtime index rather than requiring a fixed nominal carrier. Constant carrier eligibility, monomorphization inference across constrained generic carriers, and constructor static binding resolution have been repaired so all positive places pass Terminal Psi and native execution agreement (`omega run --both`, exit 0), and all nine reject twins reject for the specific fact violation. Both variants are pinned under `tests/omega/{pass,fail}/facts/`.

## Requirements and evidence

- Establishment: construction owes `length == 7` (or `length == Count`), and the helper checks that its captured subtraction equals seven. Label Requires and Ensures are questions, never premises.
- Identity and lifetime: argument order retains the selected scalar before the later cursor reset. Distinct subjects and overwritten outputs cannot borrow its fact.
- Composition: locals, fields, parameters, returned values, match joins, constants, an actual nested machine, another machine and target-selected bodies transport the same fact across both predicate-indexed and value-indexed generic carriers.
- Soundness boundaries: each negative changes only Requires7 to Requires9. Formation, uninitialized state, missing effects and stale-value controls retain their existing independent checks.
- Psi05–07: label queries erase after proof; the modeled move and ordinary calls reach Terminal execution. The native/Terminal oracle checks both output7 and the actual cursor0 effect.
- Omega00–02: all four hosted ProgramEntry bindings use ordinary scalar/value layout and call plans; this production supplies no device authority or port settlement evidence.
- Omega03–06: target-selected moves and helper calls use the normal scalar graph, register/frame homes and return paths.
- Omega07–09: all four hosted targets must check; host-native and Terminal execution must agree at exit0. Only recorded execution receipts establish agreement.

Remaining full-item acceptance includes other catalog/flag operations and ARM countdown, exact hardware-effect retirement, selected PortIo settlement, supplied-environment interpreted device sequences, and interrupt/secondary-processor canaries. This row does not close the parent outcome.

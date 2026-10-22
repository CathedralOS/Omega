# Indexed element mutable loan

A runtime `u64` index below 2 exclusively loans one `Room` to `bump`; the
selected count increases once and its neighbor is unchanged. Every place uses
`bump(&mut rooms[index])` on the same two-element array initialized with counts
0 and 10. `bump` adds 1 in `u32 in Wrapping`. Both indices run: selecting 0
must produce counts (1, 10), and selecting 1 must produce (0, 11).

The root `main.omg` and nine child applications have an observable oracle: exit 0
after count checks, or `Trap` on a wrong result. Their `build.omg` files bind
all four hosted targets. Native execution evidence must name its matching host.

## Places

| Place | Project | Positive Witness | Reject Twin | Transport to the indexed loan |
| --- | --- | --- | --- | --- |
| (root) | `.` | [source](main.omg) | — | root application performing the indexed loan |
| local | `local/` | [source](local/main.omg) | [source](local/invalid/main.omg) | a local copy of the runtime selector |
| field | `field/` | [source](field/main.omg) | [source](field/invalid/main.omg) | the selector is read directly from a record field |
| parameter | `parameter/` | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) | the array and selector are explicit callee parameters |
| call result | `call_result/` | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) | `select_index` returns the selector with an equality postcondition |
| match join | `match_join/` | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) | a join chooses between equal runtime selectors; both arms run |
| const name | `const_name/` | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) | `ROOM_COUNT = 2` supplies the bound on the runtime selector |
| another state | `other_state/` | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) | a successor state receives array and selector and performs the loan |
| another machine | `other_machine/` | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) | a forwarding machine transports array and selector to the loaning machine |
| another target body | `target_body/` | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) | each of four target bodies performs the same indexed loan |

Every `exercise` requires the same strict upper bound, `index < 2` (spelled
`index < ROOM_COUNT` in the const place). `u64` supplies nonnegativity. Neither
this bound nor the result checks manufacture mutable authority. The selected
loan must be exclusive, refer to the original array storage, and end before
the caller observes the counts.

Each project has an `invalid/` twin (as well as `controls/held_loan/`). It adds
a competing `&mut Room` loan of the same selected element, uses it after the
`bump` call, and must reject at that call for the still-active `held` borrow. It
retains the same bound, initialization and increment operation. A provider
failure, unproved bound, or another rejection does not satisfy `expected.txt`.

The canonical contracts are
[borrows and aliases](../../../../wiki/spec/language/ownership.md#borrows-and-aliases),
[collection bounds](../../../../wiki/spec/language/numeric_values.md#collection-bounds),
[captured places](../../../../wiki/spec/terminal-psi/loans.md#captured-places-and-available-premises),
and [referent identity](../../../../wiki/spec/terminal-psi/structural_access.md#referent-identity).
They require original storage and independent bounds and loan authority; they
do not permit copying a small referent and pretending that callee-local writes
establish caller-visible mutation.

Run the assigned row with:

```sh
python3 tools/sample_native_gate.py --cli <fixed-compiler> --facts --filter indexed_element_mut_loan
python3 tools/corpus_gate.py --filter indexed_element_mut_loan
python3 tools/corpus_gate.py --native --filter indexed_element_mut_loan
```

The corresponding regression fixtures are
`tests/omega/pass/facts/indexed_element_mut_loan_through_<place>_exit` and
`tests/omega/fail/facts/indexed_element_mut_loan_through_<place>_rejected`
(nine each; the reject fixture is a byte-identical copy of `<place>/invalid/main.omg`).

On the initial fixed compiler, three places ran and all nine twins rejected;
local, field, call result, match join and const name reached a missing
result-operand storage presentation, and another state reached a missing loan
transfer. The compiler repair retains the call's already evaluated selector
in the result-backed argument path and carries checked loans of affine local
backing across state edges. The repaired row runs all nine applications with
exit 0 on Linux x86-64 and Windows x86-64, and all nine twins retain their
intended rejection. The root application and nine place projects all verify
that loan with exit 0.

The additional regression
`tests/omega/pass/borrows/runtime_result_element_call_selector_capture_exit`
changes the selector in a later call argument and checks that the callee
mutates the element selected by the earlier argument. This tests formation
order rather than merely storing an index in an immutable local.

## Requirements and Evidence

- **Establishment:** Every positive witness establishes index validity via `requires from_index < 2` (or `from_index < ROOM_COUNT` with `ROOM_COUNT = 2`) on an unsigned `u64` runtime selector, proving in-bounds access before forming `&mut rooms[selected]`. No post-hoc caller equality guards manufacture authority.
- **Identity and lifetime:** The mutable loan `&mut rooms[selected]` refers directly to the selected element inside the backing `[Room; 2]` array storage for the duration of `bump`, mutating only the selected element while preserving the unselected neighbor's identity and retiring the loan before subsequent caller reads.
- **Composition:** Across locals, record fields, parameters, contracted call results (`ensures result == index`), `match` joins, named constants, state transitions (`other_state`), forwarding machines (`other_machine`), and all four target-scoped machine bodies (`target_body`), the runtime selector and exclusive element loan compose cleanly. Each `<place>/main.omg` vs `<place>/invalid/main.omg` differs only in the competing `let held: &mut Room = &mut rooms[...]` loan live across `bump`.
- **Soundness boundaries:** Each `<place>/invalid/main.omg` (and `<place>/controls/held_loan/main.omg`) introduces a competing exclusive borrow `held` of the same indexed element that remains live after `bump(&mut rooms[...])`, and is rejected at compile time with `while local borrow \`held\` is still active` (`exit 1`).
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Runtime-indexed mutable element loans lower without loss into canonical target-neutral Terminal Psi (`PSITERM\0`) borrow and write-back frames that capture the evaluated selector once and write the updated `Room` back to the exact indexed slot of the caller's `[Room; 2]` storage with explicit borrow restoration windows (`reborrow restored call uses`), without clobbering adjacent elements. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations. Target layout plans evaluate array layout geometry for `[Room; 2]` (where `Room` wraps `u32`, `size_of == 4`, array `size_of == 8`, `align_of == 4`), and target `CallPlan`/`StatePlan` pass the `&mut Room` mutable reference in standard ABI pointer register lanes (RCX on `windows_x86_64`, RDI on `linux_x86_64`, X0 on `macos_arm64` and `linux_arm64`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering selects effective-address computation instructions (`lea` with scaled index on `x86_64`, shifted add on `arm64`) to calculate `&rooms[index]`, performs indirect load/store instructions for `Room.count` incrementing, assigns disjoint stack-frame activation storage for the array, and inserts standard prologues and epilogues honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve to concrete frame offsets for array elements `rooms[0]` and `rooms[1]`, basic blocks and transition branches lay out with short/near branch relaxation and offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O executables are emitted, agreeing with Terminal Psi (`omega run --both` exit 0) across host targets including `windows_x86_64` and `macos_arm64`.

# Disjoint Field Borrow Windows

Fact: mutating or borrowing one field (pair.left) of a structural record (Pair { left: u64, right: u64 }) preserves the static bound on its provably disjoint sibling field (pair.right <= 75u64), discharging exact subtraction 75u64 - pair.right across all nine places.

Owning Task: `FIELD-BORROW-DISJOINTNESS`
Specification Reference: [Structural access](wiki/spec/terminal-psi/structural_access.md), [Loans](wiki/spec/terminal-psi/loans.md), and [Numeric values](wiki/spec/language/numeric_values.md)

This fact row demonstrates that mutating or mutably borrowing one field (`pair.left`) of a structural record `Pair { left: u64, right: u64 }` preserves the static contract bound on its provably disjoint sibling field (`pair.right <= 75u64`) across all nine standard places. Because field projections `.left` and `.right` are disjoint, writing through `&mut pair.left` does not invalidate facts known about `pair.right`, allowing exact subtraction `75u64 - pair.right` to be statically proved and executed without underflow across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness verifying field borrow disjointness in a local Pair | [`main.omg`](main.omg) | - |
| local | `local/` | the pair is held in a local variable, mutating left while preserving right | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the pair is nested inside a Holder record field, mutating left through nested projection | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | the pair is passed as an explicit mutable borrow parameter to exercise | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the pair under contract, then left is mutated | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the pair under contract before mutation | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | a named constant INIT_RIGHT initializes pair.right before left is mutated | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | `other_state/` | an terminal machine transfer transfers the pair to a successor machine before mutation | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine receives the pair and mutates left while preserving right | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies receives the pair and verifies disjoint preservation | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Pinned Corpus Fixtures

The nine positive places and nine reject twins are pinned in the compiler test corpus under:

- **Pass fixtures:** `tests/omega/pass/facts/disjoint_field_borrow_windows_through_<place>_exit/` (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), each carrying `build.omg`, `main.omg`, and `expected_native_exit.txt` (`0`).
- **Fail fixtures:** `tests/omega/fail/facts/disjoint_field_borrow_windows_through_<place>_rejected/` (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), each carrying `build.omg`, `main.omg`, and `expected.txt` (`may overflow \`u64\``).

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/disjoint_field_borrow_windows/main.omg
omega run --both samples/cli/facts/disjoint_field_borrow_windows/local/main.omg
omega run --both samples/cli/facts/disjoint_field_borrow_windows/field/main.omg
omega run --both samples/cli/facts/disjoint_field_borrow_windows/parameter/main.omg
omega run --both samples/cli/facts/disjoint_field_borrow_windows/call_result/main.omg
omega run --both samples/cli/facts/disjoint_field_borrow_windows/match_join/main.omg
omega run --both samples/cli/facts/disjoint_field_borrow_windows/const_name/main.omg
omega run --both samples/cli/facts/disjoint_field_borrow_windows/other_state/main.omg
omega run --both samples/cli/facts/disjoint_field_borrow_windows/other_machine/main.omg
omega run --both samples/cli/facts/disjoint_field_borrow_windows/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where `pair.right` is weakened to `76u64`, causing exact subtraction `75u64 - pair.right` to fail closed. Check each twin with:

```sh
omega --check samples/cli/facts/disjoint_field_borrow_windows/local/invalid/main.omg
omega --check samples/cli/facts/disjoint_field_borrow_windows/field/invalid/main.omg
omega --check samples/cli/facts/disjoint_field_borrow_windows/parameter/invalid/main.omg
omega --check samples/cli/facts/disjoint_field_borrow_windows/call_result/invalid/main.omg
omega --check samples/cli/facts/disjoint_field_borrow_windows/match_join/invalid/main.omg
omega --check samples/cli/facts/disjoint_field_borrow_windows/const_name/invalid/main.omg
omega --check samples/cli/facts/disjoint_field_borrow_windows/other_state/invalid/main.omg
omega --check samples/cli/facts/disjoint_field_borrow_windows/other_machine/invalid/main.omg
omega --check samples/cli/facts/disjoint_field_borrow_windows/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: exact arithmetic in machine ... may overflow \`u64\``

## Negative coverage and the recorded reject diagnostic

The nine reject twins for disjoint field borrow windows are pinned twice: as
`<place>/invalid/main.omg` under this sample, and as
`tests/omega/fail/facts/disjoint_field_borrow_windows_through_<place>_rejected`.
Each weakens only the construct's own bound/input on `pair.right` from `<= 75u64`
(`25u64`) to `<= 76u64` (`76u64`) while leaving the caller structure untouched,
causing exact subtraction `75u64 - pair.right` to fail closed with the recorded
diagnostic (`exact arithmetic in machine ... may overflow \`u64\``).

In addition, the complementary slice-window overlap twins under
`tests/omega/fail/facts/disjoint_windows_through_<place>_overlap` enforce the
structural overlap obligation directly: they widen the second exclusive window
to overlap the first (`let split: u64 = 1;` instead of `2`), and checking
refuses each with an active-loan overlap diagnostic (`creates local borrow \`b\`
while local borrow \`a\` is still active`).

## Discrimination: measured runtime and static verification

Both static checking and runtime execution confirm that the recorded outcome
measures disjoint field borrow preservation and exact subtraction rather than a
coincidental exit code:

- **1-token perturbation traps both engines:** Perturbing the demanded runtime
  result in `local/main.omg` by one token (`transition diff == 50u64` ->
  `transition diff == 51u64`, with `exercise` returning unconstrained `u64`)
  compiles and traps identically in both engines under `omega run --both`:
  `native exit -1073741795` and `interp crash: Trap (host presentation agrees)`.
  Restoring `50u64` immediately restores `native exit 0` and `interp exit: 0`.
- **Refused at checking (the overlap direction).** Holding two exclusive
  projections of the *same* field is rejected statically, never by a runtime
  trap. `let a: &mut u64 = &mut pair.left; let b: &mut u64 = &mut pair.left;`
  yields 3 diagnostics, including
  `statement 2 borrows \`pair.left\` while local borrow \`a\` is still active`.
  Holding a whole-struct exclusive loan while projecting a field of it is also
  refused statically: `statement 2 mutates \`pair.left\` while local borrow
  \`whole\` is still active`. Both are checking refusals with a borrow
  diagnostic, so the mechanism's negative side is enforced before execution.
- **Trapped at runtime (the write-through direction).** Two simultaneous `&mut`
  projections of *distinct* fields are admitted and both writes land:
  `let a: &mut u64 = &mut pair.left; let b: &mut u64 = &mut pair.right;`
  followed by two writes and a readback returns `native exit 0` with
  `interp exit: 0`. Seeding the written value from a boundary contract that
  admits a range (`ensures result >= 20u64 && result <= 40u64`) keeps the
  prover from constant-folding it; writing through `&mut pair.left` then leaves
  the sibling `pair.right` observation at `25u64`, and both engines agree at
  exit 0. Corrupting only that runtime value inside its still-admitted contract
  range (provider returns `33u64` instead of `25u64`) reaches execution and
  trips the authored guard: `native signal 5` and
  `interp crash: Trap (host presentation agrees)`.

## Requirements and Evidence

- **Parent outcome:** `FIELD-BORROW-DISJOINTNESS` (`TASKS.md`).
- **Obligation class:** Disjoint structural projection borrow window preservation + exact unsigned subtraction (`u64`) underflow discharge.
- **Requirement established:** End-to-end nine-place fact preservation for disjoint structural field mutations and borrows. Mutating or borrowing one field (`pair.left`) of a structural record (`Pair { left: u64, right: u64 }`) preserves the static bound on its provably disjoint sibling field (`pair.right <= 75u64`), discharging exact subtraction `75u64 - pair.right` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Structural access](wiki/spec/terminal-psi/structural_access.md), [Loans](wiki/spec/terminal-psi/loans.md#disjoint-path-loans), and [Numeric values](wiki/spec/language/numeric_values.md#exact-subtraction).

Full 8-layer specification and lowering evidence:

- **Establishment:** The structural record `Pair { left: u64, right: u64 }` introduces disjoint field storage locations. Binding or constraining `pair.right <= 75u64` (e.g. `25u64`) establishes an exact upper bound. Mutating or mutably borrowing `pair.left` via `touch_left(&mut pair.left)` creates a borrow window restricted strictly to path `.left`. Because the projection paths `.left` and `.right` are provably disjoint, the static knowledge and contract bound on `pair.right <= 75u64` survives the write without invalidation or alias pollution, establishing `75u64 - pair.right` without runtime checks or caller equality guards (`transition x == 100`).
- **Identity and lifetime:** Field projection maintains distinct identity for disjoint fields within the same composite record allocation. Mutating `.left` modifies only its own 8-byte slice of the activation record or memory buffer, leaving `.right`'s storage identity and value intact throughout the lifetime of the borrow and after borrow release.
- **Composition:** Disjoint field preservation composes through all 9 standard places: local variable record mutation (`local`), nested record field wrapping (`field`), machine parameter mutable borrow passing (`parameter`), function call return value binding (`call_result`), match branch convergence join (`match_join`), named constant initialization (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` and `tests/omega/fail/facts/disjoint_field_borrow_windows_through_<place>_rejected/main.omg` differs solely in supplying an out-of-range value or contract bound (`76u64`) for `pair.right`.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins (and their pinned corpus fail fixtures `disjoint_field_borrow_windows_through_<place>_rejected`) fail closed at compile time with `exact arithmetic in machine ... may overflow \`u64\`` because `75u64 - 76u64` is provably out of range for unsigned subtraction. The compiler soundly refuses any program where disjoint field facts cannot guarantee non-negative results for exact subtraction.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Disjoint projection paths lower cleanly into canonical Terminal Psi (`PSITERM\0`) with explicit path-based borrow windows (`BorrowPath` / `FieldProject`). Proof-term tags (`proof-term-tags`) retain field bounds across the borrow boundary. Both engines (`omega run --both`) execute identically and agree on exit code 0.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning assigns 16 bytes for `Pair` (two sequential 8-byte `u64` scalars at 8-byte alignment) with `left` at offset 0 and `right` at offset 8. In `CallPlan` and `StatePlan`, mutable references pass in target ABI pointer registers (`rcx`/`rdi`/`x0`), and field offsets are directly addressable.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 64-bit memory writes (`mov [reg], val` / `str val, [reg]`) for `.left` and 64-bit subtraction (`sub`) for `.right`. Disjoint activation storage ensures no spill slot collision between independent field registers. Worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`) are satisfied.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, conditional branches relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Dynamic slice window disjointness with non-constant symbolic index ranges and overlapping union field alias tracking remain governed under the wider `STRUCTURAL-LOAN-WINDOWS` and layout plan roadmap.

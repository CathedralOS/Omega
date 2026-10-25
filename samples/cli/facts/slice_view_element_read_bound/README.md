# Slice View Element Read Bound

A slice view (`&[u64]`) whose length is guarded (`view.len >= 3u64`) proves safe indexed element read at `view[2u64]` and preserves the length lower bound to discharge exact subtraction `view.len - 3u64` across all nine places.

Every place performs the same operation: borrow a 4-element array `[10u64, 20u64, 30u64, 40u64]` as a slice view `&[u64]`, transport the view through the place under the length contract `requires view.len >= 3`, safely read element `view[2u64]` (observing `30u64`), compute exact subtraction `view.len - 3u64` (observing `1u64`), and exit `0i32`. Only the transport mechanism of the slice view changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root guarded slice view element read and extent subtraction |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | slice view is transported through intermediate local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | struct field `view: &[u64]` carries the guarded slice view |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | slice view is passed to a machine parameter under length contract |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | return result of callee `borrow_items()` supplies the slice view |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms yield the guarded slice view into the joined binding |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constant `SUBTRAHEND: u64 = 3` defines the extent requirement |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor machine `check_view(v: &[u64])` receives and verifies the view |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the view through an intermediate call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies computes and verifies element read and extent |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/slice_view_element_read_bound/main.omg
omega run --both samples/cli/facts/slice_view_element_read_bound/local/main.omg
omega run --both samples/cli/facts/slice_view_element_read_bound/field/main.omg
omega run --both samples/cli/facts/slice_view_element_read_bound/parameter/main.omg
omega run --both samples/cli/facts/slice_view_element_read_bound/call_result/main.omg
omega run --both samples/cli/facts/slice_view_element_read_bound/match_join/main.omg
omega run --both samples/cli/facts/slice_view_element_read_bound/const_name/main.omg
omega run --both samples/cli/facts/slice_view_element_read_bound/other_state/main.omg
omega run --both samples/cli/facts/slice_view_element_read_bound/other_machine/main.omg
omega run --both samples/cli/facts/slice_view_element_read_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The slice view has 4 elements `[10u64, 20u64, 30u64, 40u64]`. Reading index 2 observes `30u64`, and subtracting `3u64` from `view.len` (4) yields `1u64`. Transition guard `elem == 30u64 && diff == 1u64` branches to `done()` (exit 0) and to `bad()` (`crash Trap;`, exit 255) on discrepancy.
- Each invalid twin attempts to read index `3u64` (`view[3]`) under the extent guard `requires view.len >= 3`. Since a slice of length 3 only admits indices 0, 1, and 2, the compiler statically verifies that index 3 is not provably in bounds (`3 < view.len` cannot be proved) and fails closed at compile time with `cannot prove requires contract for call index ...: 3 < view.len (needs fact \`index < items.len\` here)`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/slice_view_element_read_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/slice_view_element_read_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`index < items.len`).

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-SLICE-VIEW-VOCABULARY` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place slice view (`&[u64]`) extent guarding, safe indexed element reading, exact subtraction underflow prevention, and native realization agreement.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** The 4-element array `[u64; 4]` lends a slice view `&[u64]` via whole-array range borrow `&items[..]`. The contract guard `requires view.len >= 3` establishes both the upper bound for element index 2 (`2 < view.len`) and the non-underflow lower bound for exact subtraction `view.len - 3u64` directly from the statically verified length precondition, without requiring post-hoc caller equality guards (`transition x == 30`).
  - **Identity and lifetime:** The slice view `&[u64]` borrows `items` under a shared loan (`&`). The descriptor maintains backing storage version identity and element interval integrity across local bindings, struct fields (`Holder`), explicit parameters, call returns, match joins, named constants, state transitions, machine calls, and target bodies while `items` remains live and unmodified in place.
  - **Composition:** The guarded slice view composes through all 9 fact places into `read_guarded`. Function calls substitute exact actuals, match-arm joins unify the slice view across branches, and state transitions prove incoming arrival contracts (`requires v.len >= 3`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the indexed element offset (`2` vs `3`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time verification when attempting to read `view[3]`: the compiler verifies that index 3 is not provably within bounds under `view.len >= 3` and rejects with `cannot prove requires contract for call index ... 3 < view.len (needs fact \`index < items.len\` here)`. No unchecked or out-of-bounds index escapes into native realization.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Slice view range formation `&items[..]`, extent projection `view.len`, and element indexing `view[2]` lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof obligations while representing the slice view as a canonical two-word descriptor (pointer + element count). All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Fixed array layout `[u64; 4]` (`size_of = 32`, align 8) and slice view descriptor `&[u64]` (`size_of = 16`, align 8, 8-byte pointer + 8-byte length) validate. `CallPlan`/`StatePlan` assign slice descriptor register pairs (`rcx`/`rdx` on Windows x86_64, `rdi`/`rsi` on Linux x86_64, `x0`/`x1` on AArch64) per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects scaled-index memory loads (`mov rax, [rcx + 16]` for element 2 at 8-byte stride on x86_64, `ldr x0, [x0, #16]` on arm64), integer subtraction, and register moves. Low-level allocators assign register homes and stack-frame activation storage for array and descriptor storage with bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA memory and register forms encode into binary bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates within host limits, and direct PE/ELF/Mach-O binary images are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across hosted targets.

## What remains open

- Mutable slice element write bounds (`&mut [T]` with `view[idx] = val`).
- Dynamic slice subslice window composition with non-constant runtime offsets (`items[low..high]`).
- Multi-dimensional nested slice view descriptors.

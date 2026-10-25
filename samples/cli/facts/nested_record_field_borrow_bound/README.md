# Nested Record Field Borrow Bound

Fact: projecting a nested inner record field through a shared borrow preserves its exact static upper bound without moving either record, discharging exact subtraction 72u64 - quota across all nine places.

Owning Task: `CONSERVATION-CONTRACT` (`TASKS.md`)
Claim Ticket: `1ccddcaba99441caa2bc01e14b61479c`

Every place performs the same operation: define two-level nested records `Inner { quota: u64, tag: u64 }` with `where quota <= 72` and `Outer { inner: Inner, header: u64 }`, construct `outer: Outer` initialized with `quota: 30, tag: 7, header: 10`, take a shared borrow `&outer`, transport the shared reference or its projected field through the place, project `view.inner.quota` to observe `quota <= 72`, perform exact subtraction `72u64 - quota` (yielding `42u64`), and verify in `main` that `diff == 42` exiting 0. Only the transport mechanism of the shared reference or its projected field changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root nested record field borrow bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the shared borrow is held in a local binding, copied to a second local, and projected there |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the shared borrow is stored in a struct field and projected through member accesses |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the shared borrow is passed as a machine parameter and projected in the callee |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the shared borrow arrives as the return result of a helper machine and is projected by the caller |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms yield the shared borrow so the joined reference projects the nested field safely |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant `LIMIT: u64 = 72` provides the exact minuend for the nested field subtraction |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a transition forwards the shared borrow to a successor machine that projects the nested field |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the shared borrow to an intermediate machine for projection |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the nested shared borrow field projection and exact subtraction |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/nested_record_field_borrow_bound/main.omg
omega run --both samples/cli/facts/nested_record_field_borrow_bound/local/main.omg
omega run --both samples/cli/facts/nested_record_field_borrow_bound/field/main.omg
omega run --both samples/cli/facts/nested_record_field_borrow_bound/parameter/main.omg
omega run --both samples/cli/facts/nested_record_field_borrow_bound/call_result/main.omg
omega run --both samples/cli/facts/nested_record_field_borrow_bound/match_join/main.omg
omega run --both samples/cli/facts/nested_record_field_borrow_bound/const_name/main.omg
omega run --both samples/cli/facts/nested_record_field_borrow_bound/other_state/main.omg
omega run --both samples/cli/facts/nested_record_field_borrow_bound/other_machine/main.omg
omega run --both samples/cli/facts/nested_record_field_borrow_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, `Inner` is bounded with `where quota <= 72` and initialized with `quota: 30, tag: 7`. `Outer` contains `inner: Inner` and `header: 10`. Shared borrow field projection yields `view.inner.quota == 30u64`. Exact subtraction `72u64 - quota` produces `42u64`. In `main`, `transition diff == 42` transitions to `machine done() {}`, terminating with native exit 0. A 1-token perturbation of the demanded result (`transition diff == 42` to `transition diff == 43`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each invalid twin widens the inner field bound to `where quota <= 73`. Exact subtraction `72u64 - quota` could underflow if `quota == 73`, so compile-time proof obligation checking fails closed under the **Exact arithmetic overflow/underflow (decision 17)** obligation class with `may overflow \`u64\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/nested_record_field_borrow_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `may overflow \`u64\`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `may overflow \`u64\`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `may overflow \`u64\`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `may overflow \`u64\`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `may overflow \`u64\`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `may overflow \`u64\`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `may overflow \`u64\`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `may overflow \`u64\`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `may overflow \`u64\`` |

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\swarm\w26\worktrees\w26-6\target\debug\omega.exe --filter cli/facts/nested_record_field_borrow_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`may overflow \`u64\``).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/nested_record_field_borrow_bound_through_local_exit/`
  - `tests/omega/pass/facts/nested_record_field_borrow_bound_through_field_exit/`
  - `tests/omega/pass/facts/nested_record_field_borrow_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/nested_record_field_borrow_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/nested_record_field_borrow_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/nested_record_field_borrow_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/nested_record_field_borrow_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/nested_record_field_borrow_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/nested_record_field_borrow_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/nested_record_field_borrow_bound_through_local_rejected/`
  - `tests/omega/fail/facts/nested_record_field_borrow_bound_through_field_rejected/`
  - `tests/omega/fail/facts/nested_record_field_borrow_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/nested_record_field_borrow_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/nested_record_field_borrow_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/nested_record_field_borrow_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/nested_record_field_borrow_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/nested_record_field_borrow_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/nested_record_field_borrow_bound_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `1ccddcaba99441caa2bc01e14b61479c`.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for two-level nested record shared borrow `&Outer` field projection and bound delivery ([structural access](../../../../wiki/spec/terminal-psi/structural_access.md), [loans](../../../../wiki/spec/terminal-psi/loans.md)). Specifically, projecting a nested inner record field (`outer.inner.quota <= 72u64`) through a shared borrow (`&Outer`) preserves its exact static upper bound without moving or consuming either record, discharging exact subtraction `72u64 - quota` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded inner record definition (`data Inner where quota <= 72, { quota: u64; tag: u64; }`) establishes the domain invariant upon construction. When nested inside `data Outer { inner: Inner; header: u64; }`, creating a shared borrow `&outer` introduces a shared reference whose nested projection `view.inner.quota` directly carries the inner field invariant bound (`quota <= 72`) without requiring any post-hoc caller equality guard (`transition x == 42`) to justify forming the exact subtraction `72u64 - quota`.
  - **Identity and lifetime:** Both the outer record `outer` and nested inner record `outer.inner` retain their storage identity throughout the borrow window. The shared borrow `&Outer` provides non-exclusive, read-only loan authority over the entire composite record hierarchy without copying or moving either level of storage.
  - **Composition:** The nested projected field bound composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction (`72u64 - quota`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the inner record bound (`quota <= 72` vs `quota <= 73`), leaving exact subtraction unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof checking when `quota <= 73` is specified: exact subtraction `72u64 - quota` underflows below zero if `quota == 73`, strictly rejecting with `exact arithmetic in machine ... may overflow \`u64\``. No unproved underflow or invalid nested loan reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Two-level shared borrow projection (`BorrowShared` followed by chained `ProjectField` steps) and integer subtraction lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving reference addressing and chained field dereferencing. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The composite layout geometry plan (`Inner`: two `u64` fields at offsets 0 and 8, size 16, alignment 8; `Outer`: nested `Inner` at offset 0, `header` at offset 16, size 24, alignment 8; `Carrier`: reference pointer at offset 0, alignment 8) evaluates and validates, and target `CallPlan`/`StatePlan` assign reference pointers and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA address computation and memory dereference instructions (displacements into the nested inner struct offset: `mov rax, [rbp - disp]`, `mov rdx, [rax + offset]` on `x86_64`; `ldr` with base-plus-offset addressing on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA chained pointer dereference and subtraction instructions encode into machine bytes, basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Deep three-plus level nested borrow projections, mutable nested field projection reborrow windows, and dependent inter-field invariants across nested records.

# Guarded record and line loans

A selected `Complete` or `End` edge retains the mutable `Application` record and shared 64-byte line loans, so storing count in `application.ledger.count` and reading `line[0]` observes `37 + 5 = 42`.

Every positive and reject twin copies that fact sentence. The bound is 64 and the operation is the original record store followed by the byte-plus-record read. Each application exits 0 only if both the `Complete` and `End` deliveries observe 42. Before each invocation, the caller sets its record count to 99; after each return, it must observe count 5 in the original record. Full and Invalid return false.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness with caller-local record and state-local line owners |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | caller-local record and state-local line owners |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the line owner is a `Holder` record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | an extra ordinary invocation forwards the mutable `Application` parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the shared line reference is returned from a lifetime-related call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the shared line reference |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named `INDEX` is exactly zero for the same write and read |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | an extra state transports both references |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a separate machine performs the same record store and read |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | all four hosted target bodies perform that operation |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`.
`omega run --both` reports Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/guarded_record_line_loan/main.omg
omega run --both samples/cli/facts/guarded_record_line_loan/local/main.omg
omega run --both samples/cli/facts/guarded_record_line_loan/field/main.omg
omega run --both samples/cli/facts/guarded_record_line_loan/parameter/main.omg
omega run --both samples/cli/facts/guarded_record_line_loan/call_result/main.omg
omega run --both samples/cli/facts/guarded_record_line_loan/match_join/main.omg
omega run --both samples/cli/facts/guarded_record_line_loan/const_name/main.omg
omega run --both samples/cli/facts/guarded_record_line_loan/other_state/main.omg
omega run --both samples/cli/facts/guarded_record_line_loan/other_machine/main.omg
omega run --both samples/cli/facts/guarded_record_line_loan/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

Before each invocation, the caller sets its record count to 99 (`application.ledger.count = 99`).
It invokes `exercise(&mut application, false)` (`Complete`) and `exercise(&mut application, true)` (`End`).
Inside `exercise`, `observe` writes `application.ledger.count = count` and evaluates `(line[0] + application.ledger.count) == 42`.
The caller asserts `complete && completed_count == 5 && ended && application.ledger.count == 5`.
If either delivery fails to observe 42 or fails to mutate the ledger count to 5, the transition evaluates to `false`,
transitioning to `wrong() { crash Trap; }` and terminating with a non-zero exit/signal.

### Invalid controls and reject twins

Each place has an `invalid/` twin (and sibling `controls/shared_record/`) that consistently attenuates
only the `Application` loan from mutable (`&mut Application`) to shared (`&Application`) while retaining the same
storage, selected case edges, bound, store and read.

The expected diagnostic is:

```
assignment cannot write `application` because it is not mutable in this state
```

Restoring the mutable borrow restores clean compilation and execution.

```sh
omega --check samples/cli/facts/guarded_record_line_loan/local/invalid/main.omg
omega --check samples/cli/facts/guarded_record_line_loan/field/invalid/main.omg
omega --check samples/cli/facts/guarded_record_line_loan/parameter/invalid/main.omg
omega --check samples/cli/facts/guarded_record_line_loan/call_result/invalid/main.omg
omega --check samples/cli/facts/guarded_record_line_loan/match_join/invalid/main.omg
omega --check samples/cli/facts/guarded_record_line_loan/const_name/invalid/main.omg
omega --check samples/cli/facts/guarded_record_line_loan/other_state/invalid/main.omg
omega --check samples/cli/facts/guarded_record_line_loan/other_machine/invalid/main.omg
omega --check samples/cli/facts/guarded_record_line_loan/target_body/invalid/main.omg
```

## Requirements and Evidence

- **Parent outcome:** Multi-borrow lifetime preservation / [loans](../../../../wiki/spec/terminal-psi/loans.md) / [ownership](../../../../wiki/spec/terminal-psi/ownership.md).
- **Pinned fixtures:**
  - Positive passes: `tests/omega/pass/facts/guarded_record_line_loan_through_*`
  - Negative controls: `tests/omega/fail/facts/guarded_record_line_loan_through_*`

### Specification and Lowering Layers

1. **Establishment:** Selecting the `ReadOutcome::Complete` or `ReadOutcome::End` case edge establishes simultaneous loan availability: an exclusive mutable borrow `&mut Application` and a shared array borrow `&[u8; 64]` over disjoint backing allocations. This establishes authority for both the mutating ledger field assignment (`application.ledger.count = count`) and immutable byte index load (`line[0]`) directly without any post-hoc caller equality guard (`transition x == 100`).
2. **Identity and lifetime:** The mutable record borrow and shared byte-slice loan remain bound to their exact backing storage homes (`application` and `line`) across state transitions, case branches, field projections, parameter forwarding, and machine calls. State loan chains retain backing homes by checked successor membership, ensuring borrow validity without aliasing or invalidated storage across delivery boundaries.
3. **Composition:** The dual borrow transport composes through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into `observe(application, &line, count)`. Each positive `<place>/main.omg` vs. `<place>/invalid/main.omg` differs only in attenuating `&mut Application` to `&Application`, isolating the mutable loan requirement under test.
4. **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins (and `controls/shared_record/`) fail closed with compilation error `assignment cannot write \`application\` because it is not mutable in this state` when a shared borrow attempts mutation. Shared and mutable loan regions remain strictly disjoint; no simultaneous aliased mutable loans can be formed.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Multi-borrow loan chains lower into canonical target-neutral `PSITERM\0` operations, materializing explicit borrow restoration windows (`reborrow restored call uses`) and affine cleanup actions while erasing proof-only contracts. Tagged case payload dispatch (`ReadOutcome`) and simultaneous reference carriers execute identically in the Terminal Psi interpreter (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles uniformly across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Record layout plans for `Application`, `Ledger`, `Variant`, and `Sequence` resolve field offsets and alignments, array `[u8; 64]` occupies 64 contiguous bytes, and pointer carriers lower across standard ABI parameter registers and stack slots according to target calling conventions.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects native pointer-indirect stores and byte loads, assigns register homes and disjoint stack-frame activation storage for `Application` and `line[64]`, composes worst-case stack usage, and inserts entry prologues and epilogues honoring the `StatePlan` frame ceiling.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve for record and array activation storage, case dispatch branches relax with offset reflow, machine-state footprint evidence validates transitive usage, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets (`windows_x86_64` and `macos_arm64`).

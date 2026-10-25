# call_result_bound_discharge

A callee's `requires` discharges on a bound the caller learned from another call's closed result: the result is 4, so `echo` proves `scale <= 5` structurally at the instantiated judgment.

That one sentence is the fact. Only the transport of the closed result changes across the nine places; the bound (`scale <= 5`), the operation (`echo`), and the observed value (4) stay fixed. A place that changed any of them would be a different fact and would not count.

This fact is one row of nine places. Every positive place (and root `main.omg`) binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in its `build.omg`, carries `expected_native_exit.txt` = 0 and the header `// Expected exit: 0`, traps on a wrong result (`crash Trap;` in `state fail`), and agrees across native and Terminal execution (`omega run --both`). Each place has one reject twin fed a closed result of 9 instead of 4, which rejects at the same `scale <= 5` obligation.

## Places

| Place | Positive | Fact-sensitive reject twin | Transport of the closed result |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | bound to a local before the call |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | bound to a local before the call |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | stored in a record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | arrives as an explicit parameter |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | passed as the call result directly |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | both match arms yield it |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | named constant `SCALE` |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | a successor machine receives it |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | a forwarding machine returns it |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | each of four target bodies uses it |

Run one place or root directly:

```sh
omega --check samples/cli/facts/call_result_bound_discharge/main.omg
omega run --both samples/cli/facts/call_result_bound_discharge/main.omg
```

`omega run --both` reports the native exit and the Terminal Psi exit; agreement at zero is the place's runtime oracle.

## Reject twins

Each twin keeps the operation, the record and the construction shape identical and feeds a closed result of 9 instead of 4. Every twin rejects with exit 1 at `--check`:

| Place | Reject Twin | Authored diagnostic fragment |
| --- | --- | --- |
| local | [local/invalid/main.omg](local/invalid/main.omg) | `cannot prove requires contract for call echo from Main::main: s <= 5` |
| field | [field/invalid/main.omg](field/invalid/main.omg) | `cannot prove requires contract for call echo from Main::main: reading.scale <= 5` |
| parameter | [parameter/invalid/main.omg](parameter/invalid/main.omg) | `cannot prove requires contract for call deliver from Main::main: s <= 5` |
| call_result | [call_result/invalid/main.omg](call_result/invalid/main.omg) | `the instantiated fact is structurally false (`9 <= 5`)` |
| match_join | [match_join/invalid/main.omg](match_join/invalid/main.omg) | `cannot prove requires contract for call echo from choose: scale <= 5` |
| const_name | [const_name/invalid/main.omg](const_name/invalid/main.omg) | `the instantiated fact is structurally false (`9 <= 5`)` |
| other_state | [other_state/invalid/main.omg](other_state/invalid/main.omg) | `cannot prove requires contract for call hold from carry: scale <= 5` |
| other_machine | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | `the instantiated fact is structurally false (`9 <= 5`)` |
| target_body | [target_body/invalid/main.omg](target_body/invalid/main.omg) | `cannot prove requires contract for call take from Main::main: s <= 5` |

Check a twin with `omega --check samples/cli/facts/call_result_bound_discharge/<place>/invalid/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `ONE-INTEGER-BOUNDS-ENGINE` (`TASKS.md`)
- **Requirement families:** `language-state-contracts-001`, `terminal-psi-fact-validity-001`, `terminal-psi-integer-certificates-001`
- **Requirement established:** A callee's `requires` contract (`scale <= 5`) discharges on a static integer bound learned from another call's closed result (`opaque() -> 4u64`) across source checking, Terminal Psi representation, backend code generation, and native execution across all nine places.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** `opaque()` is a closed machine whose body is the literal `4u64`, so its return value is exact and known at compile time. The callee's contract `requires scale <= 5` is directly discharged by the instantiated judgment on the argument the call supplies without any post-hoc caller equality guard (`transition x == 100`). Removing the closed result removes the proof, as demonstrated by the twins.
- **Identity and lifetime:** Every place tracks one exact scalar value: the result of one call to `opaque()`. The static scalar integer bound `4u64 <= 5u64` persists across value copies, struct fields, parameters, join points, const bindings, and state hops without invalidation.
- **Composition:** The call-result bound fact composes through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming `echo(scale)` call. `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the construct's input value (4 vs 9), or contract (`other_machine`).
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed with structural or requires contract diagnostics (`the instantiated fact is structurally false (`9 <= 5`)` or `cannot prove requires contract for call ...`) when fed 9 instead of 4, refusing compilation rather than admitting invalid contracts downstream.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower without semantic loss into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof-formal tag lanes while retaining integer certificates, state transitions, and target-scoped body candidates, executing in the Terminal Psi interpreter with full agreement (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), layout geometry for scalar `u64` and records (`data Main { hits: i32 in Wrapping; }`, `data Reading { scale: u64; }`) evaluates and validates, and `CallPlan`/`StatePlan` place scalar arguments and returns across registers and stack frames according to target ABIs.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`mov`, `call`, `ret`), assigns register homes and disjoint stack-frame storage with live-across-call spill slots, and inserts entry prologues and multi-return epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve, basic blocks and branch offsets relax, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across all hosted targets including `windows_x86_64` and `macos_arm64`.

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/call_result_bound_discharge`: 10/10 samples meet their documented oracle on `windows_x86_64` (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract` or `structurally false`).
- The exit oracle discriminates: corrupting the observed result in the `local` place (`transition r == 4` to `transition r == 5`) makes the project trap under both engines (`crash Trap`), producing exit code 255 against the authored 0.
- Remaining open on `ONE-INTEGER-BOUNDS-ENGINE`: this row establishes the call-result discharge of a bounded integer obligation only. It does not establish bounds carried as borrowed projections, floating-point ranges, or generic instantiations.

## Pinned fixtures

- Positives: `tests/omega/pass/facts/call_result_bound_discharge_through_<place>_exit`, each binding `Main::main` for the four hosted targets and carrying `expected_native_exit.txt` = 0.
- Twins: `tests/omega/fail/facts/call_result_bound_discharge_through_<place>_rejected`, each carrying its authored fragment in `expected.txt`.

Both sets are byte-identical to the corresponding sample place and twin, so the fixture tree pins the same programs the row runs.

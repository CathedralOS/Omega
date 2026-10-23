# Case Payload Exclusive Loan Across the Nine Transport Places

Fact: a match on one evaluation of a copyable flag returns the exclusive loan of the selected case payload, writes 41 through that loan, and leaves the unselected payload unchanged.

Every place carries the same construct and operation: one `match` on a single evaluation of a copyable `bool` (`choose(&mut visits)`, contracted `requires counter == 0`, which increments `visits` to 1) returns the exclusive loan `&mut i64` of the selected `Slot::Present` payload. The loan is written through (`selected = 41`) and read back. The program exits 0 exactly when the subject was evaluated once (`visits == 1`) and the write through the loan landed (`observed == 41`); otherwise it branches to `wrong()` and executes `crash Trap;`.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness (the `local` program under the row's root `build.omg`) |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the loan is taken over two local `Slot` bindings inside `main` |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the payloads are record fields (`pair.chosen.value`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the loan is taken in a callee over `&mut Slot` parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the sources arrive as `present(value)` call results |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | the sources are produced by reconverging match joins |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the initial and unselected payload values are named constants |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state `pick` takes the loan and consumes it |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `forward` relays the loan to `apply` on a separate machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | four target-scoped `apply` bodies (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`) take the loan |

## Execution

Each positive place and root `main.omg` compiles and executes with agreement between Terminal Psi and native code, exiting 0 on all four hosted platforms (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`):

```sh
omega run --both samples/cli/facts/case_payload_exclusive_loan/main.omg
omega run --both samples/cli/facts/case_payload_exclusive_loan/local/main.omg
omega run --both samples/cli/facts/case_payload_exclusive_loan/field/main.omg
omega run --both samples/cli/facts/case_payload_exclusive_loan/parameter/main.omg
omega run --both samples/cli/facts/case_payload_exclusive_loan/call_result/main.omg
omega run --both samples/cli/facts/case_payload_exclusive_loan/match_join/main.omg
omega run --both samples/cli/facts/case_payload_exclusive_loan/const_name/main.omg
omega run --both samples/cli/facts/case_payload_exclusive_loan/other_state/main.omg
omega run --both samples/cli/facts/case_payload_exclusive_loan/other_machine/main.omg
omega run --both samples/cli/facts/case_payload_exclusive_loan/target_body/main.omg
```

### The oracle discriminates

- Clause "one evaluation of a copyable flag": `visits` is 0 at entry (`requires counter == 0`), `choose` increments it, and the transition requires `visits == 1`. A branch taken without evaluating the subject, or a subject evaluated twice, fails the transition and traps.
- Clause "writes 41 through that loan": the transition requires `observed == 41`, where `observed` reads the value back through the same loan binding. A loan that did not write, or wrote another value, fails the transition and traps.
- Clause "leaves the unselected payload unchanged": an exclusive loan targets the selected payload only, so writing the unselected payload would require taking a second loan — the static obligation this row's reject twins pin. The current runtime oracle observes the selected payload (`observed`) and once-only evaluation (`visits`); it does not read the unselected payload back, so clause 3 is enforced by the checking layer and the reject twins, not by a runtime comparison.

## Reject Twins

Each place has an `invalid/` twin that breaks only the exclusive-loan obligation by using the source while the result loan is live (`chosen = Slot::Present { value: 7 };` — or `pair.chosen` in the `field` place — inserted between the loan's creation and its first write). The checked compilation refuses with exit 1 and the exact per-place diagnostic below. All twins carry authored `invalid/sample.json` (`rejection_contains`) and `invalid/expected.txt` with the same fragment, and bind `ProgramEntry` for the four hosted targets like their positive place.

| Place | Twin | Pinned Diagnostic Fragment (verbatim) |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | ``statement 4 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 3; its last use is at statement 6)`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | ``statement 3 mutates `pair.chosen` while local borrow `selected` is still active (borrowed at statement 2; its last use is at statement 5)`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | ``statement 1 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 0; its last use is at statement 3 and it is released at state exit)`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | ``statement 4 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 3; its last use is at statement 6)`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | ``statement 4 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 3; its last use is at statement 6)`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | ``statement 4 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 3; its last use is at statement 6)`` |
| another state | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | ``statement 1 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 0; its last use is at statement 3)`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | ``statement 1 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 0; its last use is at statement 3 and it is released at state exit)`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | ``statement 1 mutates `chosen` while local borrow `selected` is still active (borrowed at statement 0; its last use is at statement 3 and it is released at state exit)`` (four diagnostics, one per target body) |

## Pinned Fixtures

- Positive passes: `tests/omega/pass/facts/case_payload_exclusive_loan_through_<place>_exit` (all 9 places pinned, each carrying `expected_native_exit.txt` with `0`).
- Negative controls: `tests/omega/fail/facts/case_payload_exclusive_loan_through_<place>_rejected` (all 9 places pinned; each `expected.txt` holds that place's verbatim diagnostic above).

### Run Record

Measured on base `9ff26a870215` with the shared wave producer
(`target/debug/omega`, `{"schema":"omega-compiler-build-profile-v1","opt_level":"1"}`):

- `omega run --both` on the root `main.omg` and the nine places: 10/10 `native exit 0` with `interp exit: 0`.
- `omega --check` on the nine `invalid/` twins: 9/9 exit 1, each with the verbatim diagnostic pinned above.
- `python3 tools/sample_gate.py --filter cli/facts/case_payload_exclusive_loan,cli/facts/loop_branch_relaxation_ranking --cli <producer> --write-results …`: 38/38 check on Darwin arm64, 18 intended-invalid controls matched.

Two native-leg rows are recorded after the repair described below:

- `python3 tools/corpus_gate.py --record --filter facts/case_payload_exclusive_loan` — 18 rows spliced (9 `checked` with `check-target:macos_arm64`, 9 `rejected ... expected`).
- `python3 tools/corpus_gate.py --native --record --filter facts/case_payload_exclusive_loan` — 9 `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`.

### Repair: a state's own reference handed to a call

Before the repair, the corpus native leg refused exactly one place of this row:
`pass/facts/case_payload_exclusive_loan_through_other_state_exit` observed
`rejected` with

```text
target legalization failed: SourceCustodyMismatch { machine: Some(MachineId(1)), operation: Some(OperationId(8)), site: withheld; set OMEGA_COMPILER_CHECK_SITES to reveal it }
```

The check leg accepted the same fixture (`checked`, `check-target:macos_arm64`)
and the CLI built and ran it (`omega run --both` native exit 0, `omega --target
macos_arm64` publishes), so the refusal was a legalization precision gap in
stage 03, not a language rule. A probe matrix isolated the shape: a successor
state passing one of its own reference parameters as a machine-call argument.
The same call from a machine function builds, and states holding references
without such a call build. The revealed site was
`omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization/scalar_graph_input/reference_custody.rs:1283`:
`call_arguments`' descriptor-free branch counted the state's own data-parameter
leaf — an owned arrival alias whose backing names the storage place the argument
resolves to — as an independent child borrow of that storage, so the alias
conflicted with itself. The branch now scans with
`storage_access_conflicts_beneath_argument`, which skips the argument's own
keyed permission leaf and such owned arrival aliases of the same path; a
reference-bearing leaf (a second loan, a child projection, a surviving carrier)
still refuses.

## Requirements and Evidence

- **Parent outcome:** `MATCH-SELECTIVE-LOWERING` (`TASKS.md`), borrowed joins with case-bearing referents: "Still missing: write-only joins (**WRITE-ONLY-BORROW**), case/reference/descriptor-bearing referents, runtime-sized element-view selections, ranges, computed roots, and join-place reads/projections. End loans at last use …"

### Specification and Lowering Layers

1. **Establishment:** `choose(&mut visits)` evaluates the subject once (`requires counter == 0`, `counter = 1`) and each match arm's `&mut <slot>.value` establishes the exclusive loan of the selected `Slot::Present` payload. The runtime transition requires the once-only evaluation (`visits == 1`) and the write landing through the loan (`observed == 41`); no post-hoc equality guard re-establishes the loan fact.
2. **Identity and lifetime:** the loan's storage identity is the selected payload field itself; the unselected payload is a disjoint field and is never written. The loan ends at its last use (`observed`), before the transition consumes the result. The reject twins pin the rule at the point of violation: using the source while the loan is live is refused (`mutates \`chosen\` while local borrow \`selected\` is still active`) — in every place, including through a call (`parameter`, `call_result`, `other_machine`) and inside a state (`other_state`).
3. **Composition:** the same fact composes without loss across all nine canonical places (local, field, parameter, call result, match join, const name, another state, another machine, another target body) into the consuming write + read. `git diff <place>/main.omg <place>/invalid/main.omg` shows only the loan-violating statement (plus the twin's header comment).
4. **Soundness boundaries:** all nine twins fail closed during checked compilation (exit 1) with the per-place diagnostic pinned above; the corpus gate records all nine as `rejected` with their expected fragment satisfied.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the scalar-subject match lowers to typed branches whose join carries the loan; the match arm selection executes only the selected arm; the Terminal Psi interpreter executes the same program (`interp exit: 0`) and agrees with native execution.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `Slot` is a tagged case payload (discriminant + `i64` payload) with `size_of`/`align_of` geometry; `ProgramEntry` settles for all four hosted targets; the `&mut` lanes cross calls in the `parameter`, `call_result`, `other_machine` and `target_body` places with register/stack placement under the calling plan.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** comparison/branch selection on the boolean subject, the store through the loan, and the reload; disjoint activation frames for `choose`/`apply`/`forward`; `visits` is mutated through a reference across call boundaries. Native execution agrees with Terminal Psi (`omega run --both` exit 0).
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** direct PE/ELF/Mach-O emission produces functioning executables for the realized host target; the native golden records nine `built ... exit:0` rows on `macos_arm64`, and the check leg records the portability verdict (`check-target:macos_arm64`) for the same nine fixtures.

### What Remains Open

- A runtime observation of the unselected payload (clause 3 of the fact sentence) would strengthen the oracle beyond the current selected-payload and once-only checks.
- Native execution rows for the non-host targets (`windows_x86_64`, `linux_x86_64`, `linux_arm64`) come from their own hosts' native goldens; this row records the `macos_arm64` host.
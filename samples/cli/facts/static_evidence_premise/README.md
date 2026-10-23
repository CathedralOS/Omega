# Static evidence premise

Owning task: OPERATOR-MACHINE-SUPPLY.

A call premise is a static evidence argument. The call records that evidence, and the caller reads the record. A supplied witness does not discharge a Boolean premise by itself.

The mechanism under test: `project<Proof satisfies Kept>(kept: bool)` requires `kept == true` and `Proof::holds(kept)`. The static evidence argument `YesKept` (the `Yes` realization of the `Kept` trait) records `Yes::holds(true) == true` at the selected realization of the call; the caller `main` then reads that record as `direct == true` and satisfies `hold(direct)`'s `requires direct == true`. `kept == true` stays a separate premise — the witness argument alone does not discharge the Boolean premise.

The same sentence is copied through local, field, parameter, call result, match join, const name, another state, another machine, and a target body. Each positive binds `ProgramEntry` for the four hosted targets and exits 0 on both engines; every positive place and its reject twin differ only in the static evidence argument (`YesKept` vs `NoKept`).

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | the subject is a local binding (`let kept: bool = true`) read at the call |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | a copyable record field carries the subject (`project<YesKept>(holder.kept)`) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | `observe` receives the subject as a parameter whose `requires kept == true` premise is discharged and calls `project<YesKept>(kept)` |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | the subject is a call result (`let kept: bool = produced()`) |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | both match arms keep the subject (`match flag { true -> true, false -> true }`) so the joined value still satisfies the premise |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | the named constant `KEPT` supplies the subject |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | a successor state `check(kept)` receives the subject and calls `project` there |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | a forwarding machine `relay` transports the subject to `observe` |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | each of the four target bodies (`observe` for `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`) performs the call |

Each positive place binds `ProgramEntry` for all four hosted targets and declares `Expected exit: 0`, carries `expected_native_exit.txt` = 0 and `expected_interpreter_exit.txt` = 0, and passes `omega run --both` — Terminal Psi and native execution agreeing:

```sh
omega run --both samples/cli/facts/static_evidence_premise/local/main.omg
omega run --both samples/cli/facts/static_evidence_premise/<place>/main.omg
```

## Reject twins

Each place has an `invalid/` twin that supplies `NoKept` instead of `YesKept` — `let direct: bool = project<NoKept>(kept);` — so the recorded evidence is `No::holds(true) == false` and the call's premise `Proof.holds(kept)` is structurally false at the selected realization. The twin changes only the construct's own argument; checking fails closed at compile time.

| Place | Reject twin | Observed diagnostic (verbatim, `gen-9d9ba4fb625` omega, 2026-10-22) |
| --- | --- | --- |
| local | [local/invalid/main.omg](local/invalid/main.omg) | error: machine `main` state `entry` value call to `project` violates required fact `Proof.holds(kept)`: the instantiated fact is structurally false |
| field | [field/invalid/main.omg](field/invalid/main.omg) | error: machine `main` state `entry` value call to `project` violates required fact `Proof.holds(kept)`: the instantiated fact is structurally false |
| parameter | [parameter/invalid/main.omg](parameter/invalid/main.omg) | error: machine `observe` state `entry` value call to `project` violates required fact `Proof.holds(kept)`: the instantiated fact is structurally false |
| call_result | [call_result/invalid/main.omg](call_result/invalid/main.omg) | error: machine `main` state `entry` value call to `project` violates required fact `Proof.holds(kept)`: the instantiated fact is structurally false |
| match_join | [match_join/invalid/main.omg](match_join/invalid/main.omg) | error: machine `main` state `entry` value call to `project` violates required fact `Proof.holds(kept)`: the instantiated fact is structurally false |
| const_name | [const_name/invalid/main.omg](const_name/invalid/main.omg) | error: machine `main` state `entry` value call to `project` violates required fact `Proof.holds(kept)`: the instantiated fact is structurally false |
| other_state | [other_state/invalid/main.omg](other_state/invalid/main.omg) | error: machine `main` state `check` value call to `project` violates required fact `Proof.holds(kept)`: the instantiated fact is structurally false |
| other_machine | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | error: machine `observe` state `entry` value call to `project` violates required fact `Proof.holds(kept)`: the instantiated fact is structurally false |
| target_body | [target_body/invalid/main.omg](target_body/invalid/main.omg) | error: machine `observe` state `entry` value call to `project` violates required fact `Proof.holds(kept)`: the instantiated fact is structurally false |

## Pinned Fixtures

The row is pinned by `tests/omega/{pass,fail}/facts/static_evidence_premise_through_*` fixtures — nine pass fixtures (each a byte-identical copy of its place program plus the `0` exit oracles) and nine fail fixtures (each a byte-identical copy of its place twin plus the pinned `expected.txt` fragments). The sample and the fixtures cannot drift apart.

## Requirements and Evidence

- **Parent outcome:** `OPERATOR-MACHINE-SUPPLY` (`TASKS.md`), whose remaining acceptance names this row: "A call premise that is a static evidence argument is recorded on the selected realization: `samples/cli/facts/static_evidence_premise` exits 0 on both engines through all nine places, and the caller reads `direct == true`. `kept == true` stays a separate premise. Each twin supplies `NoKept` instead of `YesKept` and rejects `Proof.holds(kept)`."
- **Owning requirement:** [call preconditions](wiki/spec/language/machines.md#call-preconditions) with [declaration-owned executable supply](wiki/spec/language/expressions.md#executable-supply) and [licensed normalization](wiki/spec/proofs/contracts.md#licensed-normalization); formation checks sit in `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/evidence.rs` and `.../proof_contracts/contract_entailment/specification_calls.rs`.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** The static evidence argument `YesKept` establishes `Yes::holds(true) == true` as the selected realization's recorded evidence; `requires kept == true` remains a separate premise discharged from the subject's own origin (a literal local, a record field, a parameter `requires`, a call result, a match join, the named constant `KEPT`, a successor state, a forwarding machine, or a target body). No caller-side post-hoc equality guard re-establishes the premise.
- **Identity and lifetime:** The subject `kept: bool`, the evidence argument (`YesKept`/`NoKept`), and the recorded result `direct: bool` stay bound to the exact call across value copies, record fields, parameters, call results, match joins, constants, state transitions, machine forwarding, and target-scoped bodies.
- **Composition:** The recorded evidence composes through all nine places into the consuming call `hold(direct)` (`requires direct == true`). `<place>/main.omg` vs `<place>/invalid/main.omg` differ only in the evidence argument's realization (`YesKept` vs `NoKept`).
- **Soundness boundaries:** All nine twins fail closed during compile-time verification with `structurally false` on `Proof.holds(kept)`; a supplied witness (`NoKept`) does not discharge the Boolean premise, and the refusal names the premise rather than accepting an unproved call.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The selected realization, the recorded evidence read and the consuming call lower without semantic loss into target-neutral Terminal Psi; the Terminal interpreter agrees with native execution on all nine places (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Each place settles `ProgramEntry` across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and lowers the `bool` carriers, trait-realization selection and machine plans according to target ABIs.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** The evidence read and the consuming call select target instructions, assign register homes and disjoint stack-frame storage for the `bool` carriers, and insert entry prologues and multi-return epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Instruction forms and symbolic displacements encode, branches relax with offset reflow, and direct PE/ELF/Mach-O emission executes with exit 0 agreeing with Terminal Psi (`omega run --both`) across the hosted targets.

### Verification Evidence

- Measured 2026-10-22 on macOS arm64 at `9d9ba4fb625` with the shared generation producer (`build/swarm/factsr1/producers/gen-9d9ba4fb625/omega`, sha256 `5a2fde4a3a91`): all nine positive places run `omega run --both` — nine of nine report `native exit 0` and `interp exit: 0`; all nine `invalid/` twins reject at compile time with `machine ... value call to project violates required fact Proof.holds(kept): the instantiated fact is structurally false`, nine of nine (see the table above for the per-place machine/state).
- The eighteen pinned fixtures reproduce that behavior under the corpus gate (check leg `checked` with `check-target:macos_arm64`, native leg `built` with `exit:0`, both leg `built` with `exit:0 interpreter-exit:0 interpreter:agrees`).

## Open items

- No requirement above is claimed closed beyond the nine places and nine twins of this row; the parent task's remaining acceptance (abstract callee attribution, domain/default predicates, receiver/projection terms, citation/induction guarantees) stays open on the board.
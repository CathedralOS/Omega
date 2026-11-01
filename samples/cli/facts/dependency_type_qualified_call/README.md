# Dependency type-qualified call

A call spelled through a dependency's package alias selects the data type's machine, proves its `left.value > 0` premise on the exact operand, and records its `ensures` sum for the caller.

Owning task: **OPERATOR-MACHINE-SUPPLY** (`TASKS.md`). The requirement this row establishes is the named route to a declared operation across packages: a consumer reaches a dependency's `machine + Wrapped::add` (or plain `Wrapped::describe`) as `lib::ops::Wrapped::add(&left, &right)`, with the callee's `requires` premise proved on the exact operands and its `ensures` result recorded for the caller. The private-helper witnesses of the item (`tests/omega/pass/expressions/declared_operator_private_helper_exit`) reach the sum through the operator only, because the named route was refused until this row. The row does not close the item ([What remains open](#what-remains-open)).

Every place copies that sentence and the same operation. `describe(left: &Wrapped, right: &Wrapped)` in the dependency carries `requires left.value > 0` and `ensures result == (left.value as u64) + (right.value as u64)`; the caller's `hold(total)` carries `requires total == 260` and is proved from the recorded result alone. A runtime conjunct `total == 260` follows `hold`, so a wrong callee, operand order or sum takes `bad()` (`crash Trap`). The reject twin changes only the operands, `left` from 250 to 0 and `right` from 10 to 260, so the sum stays 260, the premise is the only thing broken, and nothing reaches `bad()`. Only the transport of the two operands changes between places.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both operands are local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | both operands are fields of one record, borrowed as `&pair.left` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `observe` receives both operands and forwards its own `requires` and `ensures` |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | each operand is a call result whose `ensures` names its value |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | each operand is chosen by a `match` whose arms both yield the same record |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | each operand is copied from a named constant |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives both operands and performs the call |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `relay` forwards to `observe`, which performs the call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Build selects one implementation per hosted target; each performs the call |

Three more productions of the same layer, each with the same sentence, premise and oracle:

| Production | Positive | Reject twin | What changes |
| --- | --- | --- | --- |
| token-bound machine | [`operator_token/main.omg`](operator_token/main.omg) | [`operator_token/invalid/main.omg`](operator_token/invalid/main.omg) | the selected machine is `public machine + Wrapped::add`, called by name |
| by-value operands | [`by_value/main.omg`](by_value/main.omg) | [`by_value/invalid/main.omg`](by_value/invalid/main.omg) | the operands are `[copy]` data passed by value, `Pod::describe(left, right)` |
| private helper | [`private_helper/main.omg`](private_helper/main.omg) | [`private_helper/invalid/main.omg`](private_helper/invalid/main.omg) | `describe` delegates to a package-private `Wrapped::widen`; the twin removes `public` from `describe` and keeps every operand |

The row has no root `main.omg`; `local` is its canonical witness. Every place and production is a two-package project: the application and its `lib/` dependency, bound by `Build::depend_as(builder, "lib", …)`.

## Defect and repair

At base `2f167b2a795` every positive above was refused, and every twin was refused for the same reason instead of its authored one. The first diagnostic in all twenty-four:

```
machine `main` state `entry` statement 2: normal reference-slot completion for Handle { … } has no unique checked invocation
```

(`observe`, `check` and `X86Observe::observe` in the places that call from another machine.) Three neighbors of the same call ran at base and bound the defect: `use lib::ops::Wrapped;` followed by `Wrapped::describe(&left, &right)`; a same-package nested module spelled `outer::ops::Wrapped::describe(…)`; and the module-qualified `lib::ops::describe(…)`.

The call target resolved correctly. The receiver `lib::ops::Wrapped` did not keep its declaration chain. `lib` is a loader-certified package alias with no declaration symbol, so name resolution selects the longest exact prefix, the data type `Wrapped`, as the head of a three-member path. `assign_name_symbol` (`02_syntax-trees-to-symbol-resolved-trees`, `symbols/expressions/references.rs`) wrote the per-member chain only when every entry was valid, so the alias and module segments, which have no identity of their own, withheld the whole chain. `resolve_name_path_member_symbol` (`checking`, `receiver_topology.rs`) then re-derived the chain from the head: the first member from `head_symbol`, the last from `symbol`, the middle member `ops` as a child of `Wrapped`, which does not exist. The receiver symbol came back invalid, `call_receiver_parts` reported no receiver, the borrow-call collection skipped the call as not a machine call, and the later reference-slot completion found no invocation. A path of one or two members never reaches a middle member, and `outer::ops::Wrapped` resolves `outer` as a module, so its chain is complete.

The repair has two parts. `assign_name_symbol` now keeps a chain that names every authored member and ends at a valid declaration; the segments before the declaration stay without a symbol, and only a failed selection, which returns an empty chain, is withheld. `resolve_name_path_member_symbol` returns the authored declaration at the requested index before it re-derives the members that precede it. Two other edits were tried and dropped. A module-head fallback in path resolution did not change the diagnostic, because `lib` has no module symbol and the longest-prefix selection already returned `Wrapped`. A skip of the leading segments in the authored-selection ledger changed no result: all twenty-four stayed correct without it.

## Execution

Every positive binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/dependency_type_qualified_call/<place>/main.omg < /dev/null
omega --check samples/cli/facts/dependency_type_qualified_call/<place>/invalid/main.omg < /dev/null
python tools/sample_native_gate.py --cli <omega> --facts --filter cli/facts/dependency_type_qualified_call
python tools/corpus_gate.py --both --filter dependency_type_qualified_call
```

Measured on Windows x86-64 with an optimized producer built from `2f167b2a795` plus this change (`opt_level: 1`):

- All twelve positives report `native exit 0` and `interp exit: 0` through `omega run --both`.
- Eleven twins exit at compile time with ``cannot prove requires contract for call <callee> from <caller>: left.value > 0``: `describe` from `main` in `local`, `field`, `call_result`, `match_join`, `const_name` and `by_value`, `add` from `main` in `operator_token`, `describe` from `check` and from `X86Observe::observe` in the nested-machine and target-body places, and `observe` and `relay` from `main` in the two places whose forwarding machines carry the premise. The `private_helper` twin exits with ``selects private machine `ops::Wrapped::describe::describe` ``.
- The unmodified base producer fails all twelve positives and all twelve twins with the diagnostic above, so none of the twenty-four meets its oracle at base.

### The oracle discriminates

The program reaches `done()` only through `hold(total) && total == 260`. `hold`'s premise is discharged from the result `describe` records with the exact actuals (250 + 10), and the runtime conjunct compares the sum the native image computes. The dependency's body without its `ensures` records nothing: a probe of `local` with the `ensures` clause removed stops at ``cannot prove requires contract for call hold … total == 260``. No caller guard establishes the premise before the call.

## Reject twins

Each place and production has an `invalid/` twin whose `sample.json` carries `"role": "intended-invalid"` and `rejection_contains`: `["cannot prove requires contract for call", "left.value > 0"]`, or `["selects private machine", "ops::Wrapped::describe"]` for `private_helper`. In every twin except `private_helper` the only input that changes is the pair of operand values, 250 and 10 against 0 and 260. The `lib/` package is byte-identical to the positive's. In `private_helper` only `lib/ops.omg` differs, by the removed `public`.

The base producer rejects every twin, but with the reference-slot diagnostic. The twins pin that the resolved callee's contract and visibility are enforced on the path the repair opened.

## Pinned fixtures

The row is pinned by 24 fixtures: `tests/omega/pass/facts/dependency_type_qualified_call_through_<place>_exit` and `tests/omega/fail/facts/dependency_type_qualified_call_through_<place>_rejected` for the nine places, and `tests/omega/{pass,fail}/facts/dependency_type_qualified_call_{operator_token,by_value,private_helper}_{exit,rejected}` for the three productions. Each pass fixture is a byte-identical copy of its positive with the application renamed to the fixture, its `lib/` package and `expected_native_exit.txt` of `0`. Each fail fixture is a byte-identical copy of its twin with an `expected.txt` carrying the twin's fragments.

## Requirements and Evidence

1. **Establishment:** the dependency's `requires left.value > 0` is proved from the literal operand each place builds; the `ensures` sum is the only source of the caller's `total == 260`. Nothing re-establishes either after the call.
2. **Identity and lifetime:** `lib::ops::Wrapped` is the exact data declaration of the dependency package; its attached `describe` is selected by that identity, never by the spelling `Wrapped`. The call borrows `&left` and `&right` for its duration and neither is written.
3. **Composition:** the recorded result composes into `hold` at all nine places and three productions. The parameter place and another machine forward the dependency's premise and result through their own `requires` and `ensures`, and the nested machine receives the operands as arguments.
4. **Soundness boundaries:** the repaired path still refuses an unproved premise and a package-private callee. The chain stays empty when selection fails.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** each call lowers to an ordinary call of the dependency's machine with two reference arguments (value arguments for `by_value`); the package alias leaves no runtime trace. The Terminal Psi interpreter executes all twelve positives and agrees with native execution.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** every place and production binds `ProgramEntry` for the four hosted targets; `target_body` selects distinct implementations per target through Build rows. The operands pass as references and the sum returns as a `u64` register.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** the executed images hold two records and a scalar result around the call; a wrong sum takes the `bad()` trap.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** the emitted PE images execute and agree with Terminal Psi; the twins reject before emission.

## What remains open

- Native execution was observed on `windows_x86_64` locally. `linux_x86_64`, `macos_arm64` and `linux_arm64` are bound in every `build.omg` and realized by binding; hosted evidence is recorded in the landing pull request.
- The named route is pinned for a data type's attached machine. A path whose head is a dependency module's free machine (`lib::ops::describe`) already ran at base through the static-module call and is not part of this row.
- The remaining OPERATOR-MACHINE-SUPPLY bullets (evidence-argument const aliases, `OpenIndexOperationSelection` laws and the `operator` introducer migration) are untouched.

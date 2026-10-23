# Projection call premise

A call precondition is a projection term. The call records that projection
fact, and the caller reads the record.

`project(mark: Mark) -> bool` carries `requires mark.kept`: its precondition is
the parameter projection `mark.kept`, and its body returns that projection. The
call is admitted from the projection term, and the admission records the
resulting fact (`direct == true`) on the call's result. The consumer
`hold(direct: bool) requires direct == true` reads that record; the caller never
re-derives the fact from the carrier's bytes and never re-establishes it with a
post-hoc equality guard.

Every place copies that sentence: the projection premise is transported through
local, field, parameter, call result, match join, const name, another state,
another machine, and a target body, and in each place `hold(direct)` is proved
from the recorded projection. Only the transport of the fact changes.

The reject twin changes only the projected value from `true` to `false`. The
projection term is then false or unprovable, so the premise-forming call and/or
the consuming call is refused; both refusals are compile-time proof failures,
never a runtime trap.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the premise is projected from a local `Mark` and the caller reads the call's recorded result |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the projection is `holder.mark.kept` through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `observe(mark)` requires `mark.kept` and returns `project(mark)`; the caller reads `direct` |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `produced()` ensures `result.kept`; the premise projects from the call result |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match yield the same `Mark`; the projection survives the join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the projected value is the named constant `NAMED` |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state `check(mark)` performs the call and reads the record |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine `relay` supplies the projection to `observe`, which forms it |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of the four target bodies performs the call and reads the record |

## Execution

Every positive place binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares
`Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and
native execution agreement (`native exit 0` and `interp exit: 0`):

```sh
target/debug/omega run --both samples/cli/facts/projection_call_premise/local/main.omg
target/debug/omega run --both samples/cli/facts/projection_call_premise/field/main.omg
target/debug/omega run --both samples/cli/facts/projection_call_premise/parameter/main.omg
target/debug/omega run --both samples/cli/facts/projection_call_premise/call_result/main.omg
target/debug/omega run --both samples/cli/facts/projection_call_premise/match_join/main.omg
target/debug/omega run --both samples/cli/facts/projection_call_premise/const_name/main.omg
target/debug/omega run --both samples/cli/facts/projection_call_premise/other_state/main.omg
target/debug/omega run --both samples/cli/facts/projection_call_premise/other_machine/main.omg
target/debug/omega run --both samples/cli/facts/projection_call_premise/target_body/main.omg
python3 tools/corpus_gate.py --filter facts/projection_call_premise
```

### The oracle discriminates

Exit `0` is strictly discriminative in both directions. The positive's
`hold(direct)` call is admitted only from the recorded premise: without the
recorded `direct == true` the same statement refuses with `cannot prove requires
contract for call hold from main: direct == true`, exactly as the twins show.
The reject twins themselves are compile-time refusals, not executions: they
reach their intended obligation with only the projected value changed, so the
row distinguishes "the projection was recorded and read" from "a guard happened
to hold".

## Reject Twins

Each place has an `invalid/` reject twin that changes only the projected value
(`Mark { kept: true }` to `Mark { kept: false }`; verified with `git diff` —
the field, match-join and target-body twins change the same literal inside their
own transport). Proof checking fails closed during compile-time verification:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `cannot prove requires contract for call project` / `mark.kept` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `cannot prove requires contract for call project` / `mark.kept` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `structurally false` / `mark.kept` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `cannot prove ensures contract for exit from produced` / `result.kept` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `cannot prove requires contract for call project` / `mark.kept` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `structurally false` / `mark.kept` |
| another state | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `cannot prove requires contract for call project` / `mark.kept` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `cannot prove requires contract for call relay` / `mark.kept` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `cannot prove requires contract for call project` / `mark.kept` |

## Pinned Fixtures

The row is pinned by `tests/omega/{pass,fail}/facts/projection_call_premise_through_*`
fixtures: nine pass fixtures (checked, and built with `exit:0` plus Terminal
agreement on this host) and nine reject twins (rejected with their authored
fragments). The nine pass fixtures are byte-identical copies of their place
programs, and the nine fail fixtures are byte-identical copies of their twins,
so sample and fixture cannot drift apart.

## Requirements and Evidence

- **Parent outcome:** `OPERATOR-MACHINE-SUPPLY` (`TASKS.md`), call preconditions
  under [declaration-owned executable supply](../../../../wiki/spec/language/expressions.md#executable-supply)
  and [call preconditions](../../../../wiki/spec/language/machines.md#call-preconditions).
  The exact requirement this row establishes, quoted from the item's frontier:
  "A call precondition that is a parameter projection is recorded on the call:
  `samples/cli/facts/projection_call_premise` exits 0 on both engines through
  all nine places, and the caller reads `direct == true`."

### Specification and Lowering Layers

1. **Establishment:** The premise is established by the call itself. `project`
   takes `mark.kept` as its `requires` contract; admitting the call from that
   projection term records the fact the caller later reads (`direct == true`).
   No post-hoc caller equality guard (`transition x == 100`) re-establishes it,
   and the projection is not obtained from equal bytes.
2. **Identity and lifetime:** The recorded fact names the exact call's result
   `direct`, which is the projection `mark.kept` of that call's actual argument.
   The `<place>/main.omg` value is fresh per place (local, record field, call
   result of `produced`, match join of two arms, const `NAMED`, successor-state
   parameter, relayed machine call); the twins replace only that projected value.
3. **Composition:** The projection composes through all nine places into the
   consuming `hold(direct) requires direct == true`. `<place>/main.omg` vs
   `<place>/invalid/main.omg` differ only in the projected value.
4. **Soundness boundaries:** All nine twins fail closed during compile-time
   proof checking — the premise-forming call refuses with
   `cannot prove requires contract for call project` (or `structurally false`
   when the value makes `mark.kept` a literal falsehood), and the consuming
   `hold` refuses with `direct == true`. Missing projections are never replaced
   by a satisfier search or fallback arithmetic.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The call, the recorded
   premise, and the consuming contract lower without loss into canonical
   target-neutral Terminal Psi; the Terminal Psi interpreter executes all nine
   positive places in agreement with native execution (`omega run --both`).
   The pinned compiler mechanism is the checked-tree body-wide boolean join in
   `omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/structural_judgment/selected_contracts.rs`,
   which records `true` for a boolean call whose normal exits return the
   admitted parameter projection. The equation does not establish the premise.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
   Target realization joins Build companion rows (`ProgramEntry`,
   `ProviderPlan`, `CompilerBuiltinExecution`) across all four hosted targets.
   `Mark` (a copyable one-field record) and the `bool` result are laid out per
   target, and `CallPlan`/`StatePlan` place the calls' arguments and results.
7. **Instruction selection, register homes, and activation frame realization
   (`Omega 03–06`):** Backend lowering selects target ISA instructions for the
   projection reads, the call/return sequences, and the transition on the
   returned boolean, with register homes and disjoint frame slots under
   WCSU/bounded activation claims; the target-body place realizes all four
   target scoped bodies.
8. **Form encoding, branch relaxation, relocations, and native image emission
   (`Omega 07–09` + emission):** Final encoding resolves symbolic
   displacements, relaxes short/near branches, binds relocations, and emits
   direct PE/ELF/Mach-O images; on this host the macos_arm64 image executes
   with exit code 0 and agrees with Terminal Psi (`omega run --both`).

Remaining open scope: the parent item's broader call-premise acceptance —
calls in abstract signatures (`abstract_signature_call`), generic comparison
bodies (`generic_comparison_body`), static evidence arguments
(`static_evidence_premise`), and retiring the `operator` introducer — is carried
by the sibling rows; this row closes only the parameter-projection premise
recorded on the call and read by its caller.
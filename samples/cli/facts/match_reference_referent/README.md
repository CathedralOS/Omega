# Match-selected reference referent

A match selects one of two references, the join records that reference, and the consumer reads
through the recorded reference.

`choose()` is declared `ensures result == true`, so the join is taken on its true arm, and the
consumer `read_through` requires `place == 17`. Exit 0 therefore holds exactly when the join
recorded the reference whose referent is `chosen` (17) and not `spare` (5): the evidence is the
recorded referent, not a post-hoc caller comparison.

Every place copies that fact and performs the same operation on the same bound — a join over the
two candidate references followed by `read_through(joined)`, transitioning to `done()` only when
the observed value is 17. Only the transport of the two candidate references, and of the values
they name, changes.
## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness running the same join and consumer |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the two candidate references are local bindings (`chosen_ref`, `spare_ref`) |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the two candidate references are fields of a `Pair` record (`pair.chosen`, `pair.spare`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | both candidate references arrive as parameters of `apply` |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the two referent values arrive from `measure_chosen()`/`measure_spare()` call results whose `ensures` contracts carry 17 and 5 |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both referent values arrive through `match true` joins before they are borrowed |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the two referent values are the named constants `CHOSEN` and `SPARE` |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | the successor machine `pick` receives the two borrowed references |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine (`forward`) supplies the candidate references to `apply` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of the four target bodies of `apply` performs the join |

## Execution

Every positive place, and the row's root `main.omg`, binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares
`Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and native agreement
(`interp exit: 0` and `native exit 0`) for each:

```sh
omega run --both samples/cli/facts/match_reference_referent/main.omg
omega run --both samples/cli/facts/match_reference_referent/local/main.omg
omega run --both samples/cli/facts/match_reference_referent/field/main.omg
omega run --both samples/cli/facts/match_reference_referent/parameter/main.omg
omega run --both samples/cli/facts/match_reference_referent/call_result/main.omg
omega run --both samples/cli/facts/match_reference_referent/match_join/main.omg
omega run --both samples/cli/facts/match_reference_referent/const_name/main.omg
omega run --both samples/cli/facts/match_reference_referent/other_state/main.omg
omega run --both samples/cli/facts/match_reference_referent/other_machine/main.omg
omega run --both samples/cli/facts/match_reference_referent/target_body/main.omg
python tools/sample_native_gate.py --facts --filter cli/facts/match_reference_referent
```

## Reject Twins

Each place has an `invalid/` reject twin with an authored
[`sample.json`](local/invalid/sample.json) intended-invalid identity
(`"role": "intended-invalid"`, `"rejection_contains": [...]`) and a pinned
`tests/omega/fail/facts/match_reference_referent_through_<place>_rejected` fixture. The twin changes only the
construct's own input or selection, and checking fails closed during compile-time verification:

| Place | Twin | What it changes | Pinned Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | the join selects `spare_ref` on the true arm | `cannot prove requires contract for call read_through from main: match choose() { true => spare_ref, false => chosen_ref } == 17 (needs fact `place == 17` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | the join selects `pair.spare` on the true arm | `cannot prove requires contract for call read_through from main: match choose() { true => pair.spare, false => pair.chosen } == 17 (needs fact `place == 17` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `apply`'s join selects `spare` on the true arm | `cannot prove requires contract for call read_through from apply: match choose() { true => spare, false => chosen } == 17 (needs fact `place == 17` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the join selects `spare_ref` on the true arm | `cannot prove requires contract for call read_through from main: match choose() { true => spare_ref, false => chosen_ref } == 17 (needs fact `place == 17` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | the join selects `spare_ref` on the true arm | `cannot prove requires contract for call read_through from main: match choose() { true => spare_ref, false => chosen_ref } == 17 (needs fact `place == 17` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the join selects `spare_ref` on the true arm | `cannot prove requires contract for call read_through from main: match choose() { true => spare_ref, false => chosen_ref } == 17 (needs fact `place == 17` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `pick`'s join selects `spare` on the true arm | `cannot prove requires contract for call read_through from main: match choose() { true => spare, false => chosen } == 17 (needs fact `place == 17` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `pick`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `apply`'s join selects `spare` on the true arm | `cannot prove requires contract for call read_through from apply: match choose() { true => spare, false => chosen } == 17 (needs fact `place == 17` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each target body's join selects `spare` on the true arm | `cannot prove requires contract for call read_through from apply: match choose() { true => spare, false => chosen } == 17 (needs fact `place == 17` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |

## Pinned Fixtures

The row is pinned by `tests/omega/{pass,fail}/facts/match_reference_referent_through_*` fixtures — nine pass
fixtures (each a byte-identical copy of its place program plus `expected_native_exit.txt` `0`)
and nine fail fixtures (each a byte-identical copy of its place twin plus the pinned
`expected.txt` fragment). The sample and the fixtures cannot drift apart.

## Requirements and Evidence

- **Parent outcome:** `MATCH-SELECTIVE-LOWERING` (`TASKS.md`): "Lower value dispatch for owned,
  borrowed and linear results, structural/case/domain patterns and coverage through to native
  execution. Preserve once-only subject evaluation, authored first-match order, selected-arm-only
  execution, exact result/loan origins and death edges." Under *Borrowed joins* the task names
  case/reference/descriptor-bearing referents, ranges and join-place reads/projections as still
  missing, and its acceptance asks for corpus run fixtures for borrowed joins that run natively
  with interpreter agreement. This row supplies the reference-referent part of that acceptance.

1. **Establishment:** `choose()`'s `ensures result == true` establishes which arm of the join is
   taken; the referee's value is established where it originates (`let chosen: i64 = 17`, a
   `measure_chosen()` call result whose contract carries `17`, a `CHOSEN` constant, or a
   `match true` join) and is never established by a caller equality guard.
2. **Identity and lifetime:** the join records one exact referent; the recorded reference is read
   through once, before any write to the referee, and the twin shows that the *other* candidate
   cannot satisfy the consumer's requirement.
3. **Composition:** the join composes with local bindings, record fields, parameters, call
   results, match joins, constants, a successor machine, a forwarding machine and four target
   bodies; `<place>/main.omg` and `<place>/invalid/main.omg` differ only in the construct's own
   selection (which candidate the true arm names).
4. **Soundness boundaries:** every twin rejects during proof checking with
   ``cannot prove requires contract for call read_through …``; the recorded referent is not
   widened to "either candidate", and ownership/access of the borrowed referees is preserved.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the join, the borrow of each candidate and
   the consumer call lower without loss into Terminal Psi; the Terminal interpreter agrees with
   native execution on all nine places plus the root (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** each place
   binds `ProgramEntry` for the four hosted targets and passes the reference (a single machine
   word) through the calling plan; the root and all places declare `Expected exit: 0`.
7. **Instruction selection, register homes, activation frame realization (`Omega 03–06`):** the
   join's arm selection and the recorded reference lower to target instructions per target body
   family with register homes for the reference and the read-through result.
8. **Form encoding, branch relaxation, relocations, native image emission (`Omega 07–09` +
   emission):** the fixtures publish PE/ELF/Mach-O images that execute with exit 0 and agree with
   Terminal Psi; `tests/omega/pass/facts/match_reference_referent_through_*_exit`
   carry `expected_native_exit.txt` `0` and are recorded in the host's native golden.

## Open items

- A callee whose returned expression is itself a match join over its reference parameters does
  not record which arm was selected: `select(chosen, spare) -> &'a i64 { match choose() { true ->
  chosen, false -> spare } }` exports both parameter origins across the boundary
  (`trace callee-result: relative references=2 moves=0 cases=0`; the caller then cannot prove
  `joined == 17`). Measured at `9ff26a870215` with the shared producer: exit 200,
  ``cannot prove requires contract for call read_through from main: joined == 17``. The place
  witness therefore transports the referent values through call-result contracts instead (see the
  `call_result` place); closing the callee-returned-join route is `MATCH-SELECTIVE-LOWERING`
  follow-on work, not a fact change.
- A match join whose arm names a live reference carrier holding a join result refuses native
  target lowering: `let joined: &i64 = match choose() { true -> chosen_ref, false -> &spare }`
  and the two-arm form both report `native artifact target lowering failed: UnsupportedControlFlow`
  at `lowering/control_flow/references.rs` `establish` with `source_contract … root=None`, while
  the same join result copied into a plain local (`let joined: &i64 = chosen_ref;`) accepts.
  Recorded at `9ff26a870215` (trace: `OMEGA_TRACE_REFERENCE_ESTABLISH=1`, `trace establish: … source
  PlaceId(9) … root=None`). Minimal reproducer: the same `choose`/`read_through` prelude, then

  ```omg
  machine main() crashes Trap {
      let chosen: i64 = 17;
      let spare: i64 = 5;
      let chosen_ref: &i64 = match choose() {
          true -> &chosen
          false -> &spare
      };
      let joined: &i64 = match choose() {
          true -> chosen_ref
          false -> &spare
      };
      transition read_through(joined) == 17 { true -> done() false -> wrong() }

      machine done() {}
      machine wrong() { crash Trap; }
  }
  ```

  Copying the join result first (`let joined: &i64 = chosen_ref;`) accepts. The `match_join` place
  transports the referent values through joins and keeps the join over local borrows, which is the
  same transport the landed sibling row `facts/match_join_place_read` uses.
- No requirement above is claimed closed beyond the nine places and nine twins of this row; the
  parent task's remaining acceptance (write-only joins, runtime-sized element-view selections,
  computed roots, borrowed-result successor lifecycle) stays open on the board.


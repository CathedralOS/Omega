# Checked predicate disjoint loans

A checked distinct-index predicate authorizes two mutable element loans that write 17 and 53 to the original distinct caller elements.

Owner: **BORROW-PROOF-CONVERGENCE** in [TASKS.md](../../../../TASKS.md).
This row establishes relational consequences of an ordinary pure, total checked
predicate at loan formation. Its body states `first_index != second_index`;
its name and its Boolean result type establish no separation. The complete
integer carriers seed symbolic body interpretation. Only relations proved on
every true return reach borrow compatibility, substituted onto the exact actuals.

The consuming `replace` operation retains its two independent element loans,
then writes 17 and 53 to the original caller array. Every place transports the
same selected first index into that invocation and consumes its relation through
a local alias, record projection, parameter, completed result, match join,
constant, nested arrival, separate machine or target-scoped body. The runtime comparison observes
both updates; it does not establish the static separation required by the loans.
Every reject twin changes only the predicate's own body from `!=` to `<=`.
That weaker fact permits equal indices and must reject the same loans.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg), [exclusive call](local/exclusive_call/main.omg) | [insufficient predicate](local/invalid/main.omg), [call twin](local/exclusive_call/invalid/main.omg) |
| field | [program](field/main.omg), [exclusive call](field/exclusive_call/main.omg) | [insufficient predicate](field/invalid/main.omg), [call twin](field/exclusive_call/invalid/main.omg) |
| parameter | [program](parameter/main.omg), [exclusive call](parameter/exclusive_call/main.omg) | [insufficient predicate](parameter/invalid/main.omg), [call twin](parameter/exclusive_call/invalid/main.omg) |
| call_result | [program](call_result/main.omg), [exclusive call](call_result/exclusive_call/main.omg) | [insufficient predicate](call_result/invalid/main.omg), [call twin](call_result/exclusive_call/invalid/main.omg) |
| match_join | [program](match_join/main.omg), [exclusive call](match_join/exclusive_call/main.omg) | [insufficient predicate](match_join/invalid/main.omg), [call twin](match_join/exclusive_call/invalid/main.omg) |
| const_name | [program](const_name/main.omg), [exclusive call](const_name/exclusive_call/main.omg) | [insufficient predicate](const_name/invalid/main.omg), [call twin](const_name/exclusive_call/invalid/main.omg) |
| other_state | [program](other_state/main.omg), [exclusive call](other_state/exclusive_call/main.omg) | [insufficient predicate](other_state/invalid/main.omg), [call twin](other_state/exclusive_call/invalid/main.omg) |
| other_machine | [program](other_machine/main.omg), [exclusive call](other_machine/exclusive_call/main.omg) | [insufficient predicate](other_machine/invalid/main.omg), [call twin](other_machine/exclusive_call/invalid/main.omg) |
| target_body | [program](target_body/main.omg), [exclusive call](target_body/exclusive_call/main.omg) | [insufficient predicate](target_body/invalid/main.omg), [call twin](target_body/exclusive_call/invalid/main.omg) |

Each place also contains an `exclusive_call/` production with the same
predicate, indices, writes and oracle. Instead of retaining local element
loans, it forms both exclusive arguments for `write_pair` in one call. Its
reject twin weakens only the same predicate body; a callee argument roster
must consume the same established separation as local loan formation.
These pairs are pinned as `_through_<place>_exclusive_call_{exit,rejected}`.

The pairs are pinned under `tests/omega/{pass,fail}/facts/` as
`checked_predicate_disjoint_loans_through_<place>_{exit,rejected}`.
All positives bind all four hosted `ProgramEntry` targets and declare exit 0.

## Requirements and evidence

- **Establishment:** the ordinary checked `distinct` body, evaluated under full
  carrier bounds, proves disequality on every true return. The callee's requires
  predicate is established at each call; the body does not assume its own goal.
- **Identity and lifetime:** each consequence names the exact caller actuals at
  the available formation scope. Loan occurrence, backing, access, child closure
  and lifetime remain owned by existing resource checking. A relation cannot
  create a permission or retarget a captured selector.
- **Composition:** local, field, parameter, completed result, both match
  alternatives, constant, nested machine, another machine and target-scoped body
  preserve the selected index and the same consuming operation. The
  [composed predicate](controls/composed_predicate/main.omg) reverses actuals
  through another checked helper and uses a disjunction of strict comparisons.
- **Soundness:** [equal actuals](controls/equal_actuals/invalid/main.omg),
  [an unrelated always-true predicate](controls/unrelated_predicate/invalid/main.omg)
  and [an overlapping whole window](controls/wide_overlap/invalid/main.omg)
  reject. [Circular formation](controls/circular_formation/invalid/main.omg)
  cannot use a predicate result to establish that predicate call's own partial
  operands. [A changed selector](controls/stale_selector/invalid/main.omg)
  invalidates the entry relation before loan formation. Singleton disequality
  does not establish separation of wider windows. An
  [ordered result](controls/ordered_result/invalid/main.omg) can equal the other
  index even when its source differs: disequality transports only through an
  exact completed-result equation, never through another inequality. The
  [reversed equation](controls/reversed_equality/main.omg) remains positive;
  [a shifted middle](controls/nonzero_middle_shift/invalid/main.omg),
  [a partial true branch](controls/partial_true_branch/invalid/main.omg),
  [mixed signed carriers](controls/mixed_signed_carriers/invalid/main.omg),
  and [an overloaded comparison](controls/overloaded_comparison/invalid/main.omg)
  grant no missing separation. A [predicate requiring exclusive arguments](controls/borrow_formation/invalid/main.omg)
  cannot use its desired truth to form those borrow arguments or bypass
  observation-free specification-call admission.
  Unsupported/effectful bodies and exhausted proof work supply no evidence.
- **Psi 05–07:** admitted loans and stores use the existing Terminal Psi route;
  pure proof reasoning introduces no runtime authority or proof replay. Public
  `run --both` observes interpreted execution as well as native execution.
- **Omega 00–02:** ordinary array geometry and call plans retain original caller
  backing and select the two exact cells. This row qualifies no additional ABI lane.
- **Omega 03–06:** native addressing, stores and activation homes must preserve
  the two writes and their order through the existing realization path.
- **Omega 07–09 and emission:** native exit 0 must agree with Terminal exit 0 on
  the available macOS ARM64 host. Four bindings establish target eligibility;
  matching-host Windows and Linux execution remain separate qualification.

Canonical authority: [loan compatibility and captured premises](../../../../wiki/spec/terminal-psi/loans.md),
[invocation contracts](../../../../wiki/spec/language/state_contracts.md), and
[checked predicate consequences](../../../../wiki/spec/language/dependent_values.md#collection-invariants).

## Recorded baseline and acceptance

Fresh optimized producer `16b381462327a3f7d1eb8c7218bdb81114a8a10a` rejects all nine
local-loan places with overlapping-loan diagnostics and all nine exclusive-call
places with `as mutable more than once`. A source-identical scratch
control changing only the predicate requirement to direct `first_index != second_index`
checks and runs with native/Terminal exit 0 on macOS ARM64. Frozen source,
producer hashes and raw observations are retained in the lane qualification and
claim records; the compiler implementation changes after this observation.

Run the public acceptance with `omega run --both <place>/main.omg` after ordinary
package preparation, then `python3 tools/corpus_gate.py --both --filter checked_predicate_disjoint_loans`.
The parent retains remaining domain, runtime-projection and same-statement/theorem
premise acceptance beyond the relations established by this row.

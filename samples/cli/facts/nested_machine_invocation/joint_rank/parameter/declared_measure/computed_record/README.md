# A computed measure over one record

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../TASKS.md).

> A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

`Progress::Remaining(value: Progress)` computes `value.upper - value.lower`.
Both fields belong to the same typed record and invocation snapshot. The private
helper preserves the record on its incoming edge, then increments `lower` by
one while preserving `upper` and the canary. Seven iterations move the caller's
counter from 3 to 10; the equal-endpoint input performs none and preserves 10.
The completion checks both endpoints and canary 641. Each twin changes only the
helper's increment from 1 to 0.

| Place | Positive | Nondecreasing twin |
| --- | --- | --- |
| local | [source](../../../local/declared_measure/computed_record/main.omg) | [source](../../../local/declared_measure/computed_record/invalid/main.omg) |
| field | [source](../../../field/declared_measure/computed_record/main.omg) | [source](../../../field/declared_measure/computed_record/invalid/main.omg) |
| parameter | [source](../../../parameter/declared_measure/computed_record/main.omg) | [source](../../../parameter/declared_measure/computed_record/invalid/main.omg) |
| call_result | [source](../../../call_result/declared_measure/computed_record/main.omg) | [source](../../../call_result/declared_measure/computed_record/invalid/main.omg) |
| match_join | [source](../../../match_join/declared_measure/computed_record/main.omg) | [source](../../../match_join/declared_measure/computed_record/invalid/main.omg) |
| const_name | [source](../../../const_name/declared_measure/computed_record/main.omg) | [source](../../../const_name/declared_measure/computed_record/invalid/main.omg) |
| other_state | [source](../../../other_state/declared_measure/computed_record/main.omg) | [source](../../../other_state/declared_measure/computed_record/invalid/main.omg) |
| other_machine | [source](../../../other_machine/declared_measure/computed_record/main.omg) | [source](../../../other_machine/declared_measure/computed_record/invalid/main.omg) |
| target_body | [source](../../../target_body/declared_measure/computed_record/main.omg) | [source](../../../target_body/declared_measure/computed_record/invalid/main.omg) |

The [scalar-result helper](controls/scalar_result/main.omg) carries the computed
distance as an ordinary scalar rather than manufacturing a `Progress` input.
Every edge compares that produced value with the declared application.

The [repeated-read composition](controls/repeated_read/main.omg) adds
`value.canary - value.canary` to the measure. Its third coordinate remains an
authenticated read even though arithmetic cancels it. Controls independently
remove [entry formation](controls/missing_formation/main.omg), cancel a
[potentially overflowing intermediate](controls/cancelled_partial/main.omg),
and select a [different nominal input type](controls/wrong_nominal/main.omg).

## Requirements and evidence

The frozen optimized baseline is `f741b42d6ad0875c18b2c66ef12c98fc5aea1a63`,
CLI SHA256 `7c106ad93d250d99f93fa2dc27e90c9606760b8ccc2431b9493e4629f6f07747`.
It wrongly refuses all nine valid programs because no source-complete ranking
witness is available. Baseline and candidate source hashes, actual producer
identities, commands and engine observations are retained under
`build/record-measure/` and named in the publication record.

- Establishment: `lower <= upper` forms subtraction independently of descent;
  `upper <= 7` bounds the authored traversal. The missing-formation and
  cancelled-intermediate controls cannot borrow the desired rank as a premise.
- Identity and lifetime: one nominal root supplies the complete read roster.
  Repeated reads agree. Completed captures preserve their invocation values,
  while each arriving record is read at its own completed call frontier.
- Composition: the declared application and the helper's independently inferred
  observation are compared on every actual edge. Renaming or transporting the
  record does not replace the selected measure or fabricate a helper witness.
  A helper may instead carry an already-produced scalar result; that scalar is
  checked directly against the application on each peer edge.
- Soundness: type identity, arithmetic policy, selector/read authority,
  formation, call contracts and strict-cycle descent remain separate obligations.
  A record never receives an integer atom or pretend scalar carrier.
- Psi 06–08: every formal-relative read and its exact actual coordinate survives
  checking and runtime retention. The existing scalar computation graph produces
  one integer rank; no record-rank tuple or alternate evaluator is introduced.
- Omega 00–02: normal record layout and explicit call arguments preserve both
  endpoints, the canary and the original borrowed counter.
- Omega 03–06: caller-visible counter checks and record-canary checks expose lost
  writes, misdirected arguments and incorrect cycle lowering. Wider WCSU and
  activation-storage acceptance remain on the parent task.
- Omega 07–09: nine positives require native/Terminal agreement at exit 0; nine
  exact nondecreasing twins must reject. Corpus pins use
  `facts/nested_machine_joint_rank_record_measure_<place>_{exit,rejected}`.
  All programs bind four hosted targets; matching-host execution is macOS ARM64.
  Other hosted execution and broader parent acceptance remain unverified.

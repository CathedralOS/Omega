# Result-field equations in a ranked countdown

The unchanged walk decrements `held.remaining` through an ordinary helper
call on each recurrent arrival. `identity` publishes the exact equation
`result.remaining == value.remaining`; that equation transports the selected
rank field without borrowing evidence from the helper's private body.
The authored driver checks the full countdown from five to zero. Native and
Terminal execution must both exit zero.

```sh
omega run --both tests/omega/pass/termination/runtime_public_result_field_rank_arrivals_exit/main.omg
python3 tools/corpus_gate.py --both --filter termination/runtime_public_result_field_rank_arrivals_exit,termination/rank_arrival_fresh_nested_owner
python3 tools/corpus_gate.py --filter termination/rank_arrival_missing_public_result_field_equation,termination/rank_arrival_public_result_wrong_subject,termination/rank_arrival_formal_does_not_replace_subject,termination/rank_arrival_sibling_does_not_replace_subject,termination/rank_arrival_opaque_child_does_not_prove_subject_absent,termination/rank_arrival_opaque_operand_does_not_prove_subject_absent
```

The public command uses ordinary package/entry review; no acceptance policy
is included. Runtime qualification here is macOS ARM64, not evidence of
execution on the other declared targets.

## Historical declaration inventory

Each fixture below has one historical `Countdown.remaining: u64 [0..=5]`
declaration. It now keeps its plain field and containing data
`where remaining <= 5`; `u64` supplies the zero floor. The declarations in
the different packages do not share identity merely because their predicates
are equal. No named interval domain is introduced.

| Original revision | Fixture | Obligation retained |
|---|---|---|
| `fba85426683` | `runtime_public_result_field_rank_arrivals_exit` | field bound and exact result-field equation |
| `fba85426683` | `rank_arrival_missing_public_result_field_equation` | field bound, but deliberately no public equation |
| `fba85426683` | `rank_arrival_public_result_wrong_subject` | field bound and equation, but only the unrelated spare decreases |
| `bd68c42a6ef` | `rank_arrival_fresh_nested_owner` | field bound on fresh nested values, with their own guarded reception |
| `bd68c42a6ef` | `rank_arrival_formal_does_not_replace_subject` | field bound, but only the unrelated formal decreases |
| `bd68c42a6ef` | `rank_arrival_sibling_does_not_replace_subject` | field bound, but only the unrelated sibling decreases |
| `bd68c42a6ef` | `rank_arrival_opaque_operand_does_not_prove_subject_absent` | field bound; opaque completion establishes neither identity nor absence |
| `bd68c42a6ef` | `rank_arrival_opaque_child_does_not_prove_subject_absent` | the same obligation under a containing record |

The six invalid programs retain their original rank-range or strict-decrease
diagnostics. A bounded result alone is not the old selected subject. None of
the helpers or walks originally had a scalar result suffix, and this migration
does not invent one. The measure and rank window are unchanged.

Mechanical probes rebuild the helper result from `value.remaining + 0`, carry
the result and field through locals, and capture the nested owner before its
recurrent use. The same authored results must survive. A separate construction
probe supplies six and must refuse the data predicate. Probes are restored
before landing.

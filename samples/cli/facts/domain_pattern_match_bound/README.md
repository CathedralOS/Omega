# Predicate-domain expression match

A selected predicate-domain match arm establishes value >= 73, and the joined value discharges value - 73.

Owning outcome: **MATCH-SELECTIVE-LOWERING** in [TASKS.md](../../../../TASKS.md). The requirement is ordered predicate-domain selection over one saved subject, with the selected domain facts retained through the scalar result join and ordinary exact arithmetic.

The root [main.omg](main.omg) and its four-target [build.omg](build.omg) retain the frozen reproduction. Every positive declares `Expected exit: 0`. The same `Cell::Ready` predicate supplies `cell.value >= 73u64`; the wildcard supplies `73u64`. The consumer performs the same subtraction without a caller guard establishing the bound. Each twin changes only the domain predicate to `cell.value >= 72u64`.

| Place | Positive | Weakened-domain twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

The [ordered-effects control](controls/ordered_effects/main.omg) evaluates a computed record subject once, chooses the first of overlapping domains and executes only the selected effectful arm. [Wrong-carrier](controls/wrong_carrier/main.omg) and [missing-coverage](controls/missing_coverage/main.omg) controls preserve exact carrier and exhaustiveness obligations.

## Current failure and retained evidence

At fixed baseline `ccc9bf848faf12acabb9762ec570c7572b90cc5d`, the optimized shared producer rejects the unchanged root with `match pattern does not select a record or case of its subject's exact carrier`, followed by the missing lower-bound proof at `selected - 73u64`. The public command is `omega run --both samples/cli/facts/domain_pattern_match_bound/main.omg`; it must produce actual native and Terminal Psi exit 0. Neither engine executes at the recorded baseline refusal. The immutable failed qualification receipt and canonical implementation context are retained by claim `3cb73abd75a24eecaa6e25fddfe08b5a`.

The row and independent controls are under implementation. Runtime acceptance and fixture registration are not yet claimed.

## Requirements and evidence

- Establishment: a pure finite domain predicate establishes its own selected-arm lower bound; each result arrival independently supplies at least 73.
- Identity and lifetime: each predicate observes the same completed subject, with ordinary write invalidation and borrow authority. A saved observation cannot create facts about substituted storage.
- Composition: the joined bound travels through the nine places into exact subtraction; the weak-domain twins must fail their own missing bound.
- Soundness: field geometry alone cannot select another carrier's domain, a non-total domain does not supply coverage, and runtime predicate testing cannot introduce routed authority.
- Psi lowering: use ordinary saved-subject capture and ordered short-circuit predicate/arm control, retaining the checked result fact through canonical Terminal Psi.
- Provider/layout/ABI: every positive binds all four hosted ProgramEntry targets and uses ordinary record/reference/scalar calling plans.
- Instructions/frames: predicate branches and selected scalar transfer retain ordinary register and frame homes; the effect control must observe exactly one subject computation and one arm.
- Encoding/native: actual Terminal and native execution must agree with the authored exit and writeback oracles on each measured host.

The broader parent outcome remains open for its remaining affine/linear arms, borrowed-result joins, composite/indexed captured subjects, rewrite provenance and hosted execution acceptance. File presence does not establish any of those requirements.

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

The repaired optimized producer at source checkpoint `5a4d447f5268a1eaaea3a66fb5c4fb5fa0ddda74` runs the unchanged root, all nine places and ten composition controls with actual Linux x86-64 native/Terminal exit 0. The 19 positive corpus pins also interpret their unchanged Terminal artifact under all four hosted target descriptions, with exit 0. The nine threshold-only twins and the carrier, coverage, authority, invalidation, extra-predicate and nonfinite-predicate controls refuse their intended obligations. Claim `3cb73abd75a24eecaa6e25fddfe08b5a` retains raw streams, producer/library/toolchain identities, accepted locks and immutable qualification receipts. Foreign-host native execution remains unmeasured.

The [pure helper](controls/predicate_helper/main.omg) and its [weakened predicate](controls/predicate_helper/invalid/main.omg) exercise ordinary denotational calls. The [full-width field](controls/full_width_field/main.omg) uses `u64::MAX` and checks the exact large subtraction result. [Alias](controls/alias_routed_retained/main.omg), [inherited](controls/inherited_routed_retained/main.omg) and [nested membership](controls/nested_routed_retained/main.omg) tests retain the issuer-bound subject; their `*_unproven` controls remove that provenance and must reject. [Generic result](controls/generic_open_same_carrier/main.omg) and [exact type equality](controls/generic_equation_same_carrier/main.omg) controls preserve the tested carrier after application binding; corresponding wrong-carrier controls reject. The [transitive equality](controls/generic_transitive_equation/main.omg) control carries the declaration-owned chain `Value == Alias, Alias == Cell`; [unconstrained](controls/generic_unconstrained/main.omg) and [unrelated-binder](controls/generic_unrelated_equation/main.omg) templates still refuse despite only `Cell` callers. The [late generic carrier control](controls/generic_definition_wrong_carrier/main.omg) requires a carrier judgment after specialization. These controls test composition of this mechanism, without claiming additional fact rows. The broader result-bound transport also recovers exact disjointness from a completed pure identity call: the unchanged former unknown-index fixture is preserved as a [runtime positive](../../../../tests/omega/pass/domains/indexed_domain_known_disjoint_call_exit/main.omg). Its [unknown-input](../../../../tests/omega/fail/domains/indexed_domain_requires_invalidated_by_unknown_index_mutation/main.omg) and [index-zero](../../../../tests/omega/fail/domains/indexed_domain_requires_invalidated_by_known_index_mutation/main.omg) controls still invalidate the overlapping membership.

## Requirements and evidence

- Establishment: a pure finite domain predicate establishes its own selected-arm lower bound; each result arrival independently supplies at least 73.
- Identity and lifetime: each predicate observes the same completed subject, with ordinary write invalidation and borrow authority. A saved observation cannot create facts about substituted storage.
- Composition: the joined bound travels through the nine places into exact subtraction; the weak-domain twins must fail their own missing bound.
- Soundness: field geometry alone cannot select another carrier's domain, a non-total domain does not supply coverage, and runtime predicate testing cannot introduce routed authority.
- Psi lowering: use ordinary saved-subject capture and ordered short-circuit predicate/arm control, retaining the checked result fact through canonical Terminal Psi.
- Provider/layout/ABI: every positive binds all four hosted ProgramEntry targets and uses ordinary record/reference/scalar calling plans.
- Instructions/frames: predicate branches and selected scalar transfer retain ordinary register and frame homes; the effect control must observe exactly one subject computation and one arm.
- Encoding/native: actual Terminal and native execution must agree with the authored exit and writeback oracles on each measured host.

The broader parent outcome remains open. Fresh parent runtime observations contain 13 refusals in selected affine cleanup, case-payload/origin facts, reference-arrival custody and Build declaration selection; the retained baseline comparisons attribute them separately from this repaired row. Those programs and their original oracles remain unchanged, and this session retains their completion. Write-only joins, composite/indexed captured-subject/theorem requirements, rewrite provenance and foreign-host native execution also require their applicable evidence before parent closure. File presence and a no-loss comparison do not establish those requirements.

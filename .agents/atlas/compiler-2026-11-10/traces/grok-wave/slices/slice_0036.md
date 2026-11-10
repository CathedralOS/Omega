slice_0036

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0269 | omega-rust/psi/compilation/proof/admission/src/integer_rules.rs:686-701 | CLOSED | Early branch accepts only after witness check and `check_node` proposition validation; free sides must be structurally identical math terms. |
| A0270 | omega-rust/psi/compilation/proof/admission/src/integer_affine.rs:2945-2964 | UNDECIDED | Recognisers 7–8 skip `context.validate` and run before `RootNotValue`; I did not trace a forged witness through `accept_certificate` to acceptance. |
| A0271 | omega-rust/psi/compilation/proof/admission/src/integer_affine.rs:2953-2964 | CLOSED | Witness recogniser ignores shift count; bound conversion on `DirectShift` requires count evidence at `map_direct_shift_left_bound`. |
| A0272 | omega-rust/psi/compilation/proof/admission/src/integer_affine.rs:2491-2535 | UNDECIDED | Semantic axioms are validated at entry, but mask lookup searches uncited `semantic_axioms[..index]`; I did not prove soundness of that search. |
| A0273 | omega-rust/psi/compilation/proof/admission/src/mathematical_core.rs:3664-3666 | CLOSED | Missing signature indices are skipped in closure only; normalization during `verify_mathematical_certificate` errors on unknown declarations. |
| A0274 | omega-rust/psi/compilation/proof/admission/src/mathematical_typing.rs:1375-1378 | CLOSED | Strict-sort conversion irrelevance is intentional; certificate verification uses `check_type`, not bare `convertible` on arbitrary terms. |
| A0275 | omega-rust/psi/compilation/proof/src/checker.rs:342-344 | CLOSED | Overwrite-before-consumption skips checks only when `assignment_is_overwritten_before_consumption` shows the obligation value is dead. |
| A0276 | omega-rust/psi/compilation/proof/src/checker.rs:374-399 | UNDECIDED | `Uncovered` interval/`validation::*` fallbacks traced for control flow only; I did not audit those helpers for soundness. |

### A0269

1. Read `check_integer_cast_bound_conversion` in full (`integer_rules.rs:664-742`, read).
2. Search `rg 'check_integer_cast_bound_conversion' omega-rust` — 9 hits in `proof.rs` (2), `integer_rules.rs` (6), `lib.rs` (1) (read).
3. Read callers: `cast_bound_relation` (`proof.rs:713-744`, read), `accept_certificate_with_machine_parameters` (`proof.rs:171-218`, read), `check_node` (`proof.rs:2107-2170`, read).
4. Input to accepting branch: lifted or scalar `root_bound`/`conclusion` that parse as orders with equal strict flags, chain root/target embedded on matching sides, and `root_left==target_left` or `root_right==target_right` as `IntegerMathTerm` equality (`integer_rules.rs:686-698`, read).
5. **Verdict: CLOSED.** `check_integer_cast_chain_witness` (`integer_rules.rs:522-607`, read) and `context.validate` on every proof conclusion (`proof.rs:2121-2123`, read) run on all paths; the early return accepts only when the non-embedded endpoints are the same term (`==`), not when different literals are equated.
6. Unsure: nothing blocking this CLOSED.

### A0270

1. Read `check_integer_affine_witness` from recogniser dispatch through recognisers 7–8 (`integer_affine.rs:2873-2976`, read).
2. Search `rg 'check_integer_affine_witness' omega-rust` — 93 hits (`proof.rs` 3, `integer_affine.rs` 89, `lib.rs` 1) (read).
3. Read caller `affine_bound_relation` (`proof.rs:475-512`, read) and `accept_certificate_with_machine_parameters` axiom validation (`proof.rs:189-193`, read).
4. Input to accepting branches: empty definition/literal axiom lists plus `identity_ring_target` match (`integer_affine.rs:2945-2949`, read) or `ExactIntegerShiftLeft` with `value==root` and `count: _` (`integer_affine.rs:2951-2963`, read).
5. **Verdict: UNDECIDED.** Recognisers return `trivial_affine_form` without `context.validate` on witness terms; `PropositionContext::validate` would reject `UnknownValue` in propositions (`proposition.rs:2019-2171`, read) but I did not trace a witness whose `root` is an unvalidated `Value` or non-`Value` root through `affine_bound_relation` to `accept_certificate` success.
6. Unsure: end-to-end path from recogniser 7/8 acceptance to discharged obligation without later failure.

### A0271

1. Read witness shift branch (`integer_affine.rs:2951-2963`, read) and `map_direct_shift_left_bound` (`integer_affine.rs:1372-1450`, partial read to count evidence rules).
2. Search callers of witness checker — same as A0270.
3. Read `bound_route` / `DirectShift` path (`integer_affine.rs:428-448`, `625`, read).
4. Input to accepting branch: any shift count in witness; bound stage later receives `root_bound` with count equalities/inequalities (`integer_affine.rs:1409-1437`, read).
5. **Verdict: CLOSED.** Witness admission is shape-only; `map_direct_shift_left_bound` rejects missing or ambiguous count evidence (`integer_affine.rs:1444-1458`, read), so shift count is not silently accepted at bound conversion.
6. Unsure: nothing blocking this CLOSED.

### A0272

1. Read `bounded_bitwise_sibling` (`integer_affine.rs:2491-2535`, read) and `apply_definition` call site (`integer_affine.rs:3010-3021`, read).
2. Search `rg 'bounded_bitwise_sibling' omega-rust` — 3 hits, all in `integer_affine.rs` (read).
3. Read `accept_certificate_with_machine_parameters` (`proof.rs:189-193`, read) and `denote_bounded_certificate_with_machine_parameters` (`bounded_denotation.rs:15300-15307`, read).
4. Input: `prior_axioms` slice is `&semantic_axioms[..index]` (read at `integer_affine.rs:3020`); masks and `LessOrEqual` upper bounds are found by linear search, not citation index.
5. **Verdict: UNDECIDED.** Axioms are validated before use (`proof.rs:189-193`, `bounded_denotation.rs:15303-15307`, read), but I did not determine whether an unrelated validated axiom in that prefix can wrongly widen a bitwise bound for a conclusion that should fail.
6. Unsure: whether the proof checker always supplies a minimal axiom prefix for each witness step.

### A0273

1. Read `closure_over` / `assumption_closure` (`mathematical_core.rs:3652-3687`, read).
2. Search `rg 'closure_over' omega-rust` — 3 hits in `mathematical_core.rs` (read).
3. Read `weak_head_normalize_retaining` unknown-declaration arm (`mathematical_core.rs:1141-1151`, read) and `verify_mathematical_certificate` (`mathematical_core.rs:479-497`, read).
4. Input to accepting branch: constant index `index` with `signature.get(index) == None` in closure worklist (`mathematical_core.rs:3664-3666`, read).
5. **Verdict: CLOSED.** Skip only drops unreachable seed from closure accounting; any term using unknown `index` fails during certificate verification with `CoreError::UnknownDeclaration` (`mathematical_core.rs:1147-1151`, read on admission path at `proof.rs:209-217`).
6. Unsure: nothing blocking this CLOSED.

### A0274

1. Read module header (`mathematical_typing.rs:1-8`, read) and `SharedSort` arm (`mathematical_typing.rs:1371-1378`, read); read `convertible_retaining` wrapper (`mathematical_core.rs:1302-1311`, read).
2. Search `rg 'convert_retaining' omega-rust` — 2 hits: `mathematical_core.rs`, `mathematical_typing.rs` (read).
3. Read `convert_retaining` → `judgments::convert` (`mathematical_typing.rs:343-352`, `2266-2281`, read) and `verify_mathematical_certificate` (`mathematical_core.rs:479-497`, read).
4. Input to accepting branch: any `(left, right, shared_type)` where `inferred_sort` yields `sort.is_strict()` (`mathematical_typing.rs:1374-1378`, read).
5. **Verdict: CLOSED.** Strict conversion irrelevance is documented (`mathematical_typing.rs:7-8`, read); production admission re-decision type-checks certificates via `check_type` (`mathematical_core.rs:491-497`, read), not ad-hoc `convertible` on untyped pairs. Production `convertible` call sites outside tests were not exhaustively listed beyond `bounded_denotation.rs` test-only use (read at `12355-12366`).
6. Unsure: every non-test `convertible` caller always precedes conversion with typing (inferred from `Operation::Check` structure, not fully enumerated).

### A0275

1. Read `check_bounded_assignment` (`checker.rs:332-418`, read) and `assignment_is_overwritten_before_consumption` in full (`checker.rs:420-512`, read).
2. Search `rg 'assignment_is_overwritten_before_consumption' omega-rust` — 2 hits in `checker.rs` (read).
3. Read consumer `check_proof_plan_inner` (`checker.rs:6505-6520`, read): `None` means no `CheckedIntegerAssignment` row, not silent success with wrong range.
4. Input to accepting branch: obligation assignment reached, then later assignment to target with same `display_name` (`checker.rs:439-473`, read).
5. **Verdict: CLOSED.** Early `return None` (`checker.rs:342-344`, read) applies only when overwrite-before-consumption detects the stored value is overwritten before any qualifying read; I did not trace a case where a consumed out-of-range value is dropped solely due to `display_name` collision without alias/overwrite logic firing.
6. Unsure: full shadowing edge cases across nested locals with identical display strings.

### A0276

1. Read all cited `Uncovered` arms in `checker.rs:373-399`, `547-577`, `623-642`, `689-715`, `762-808` (read).
2. Search `rg 'CertificateVerdict::Uncovered' omega-rust/psi/compilation/proof/src/checker.rs` — 8 hits (read).
3. Read top-level gate `check_proof_plan_with_decisions` (`checker.rs:6468-6536`, read); diagnostics still drive failure.
4. Input: certificate builder returns `Uncovered`, then interval guards and `validation::occurrence_integer_expression_fits_range` / `arrival_integer_expression_bounds` may withhold diagnostics (read in cited blocks).
5. **Verdict: UNDECIDED.** Control flow read confirms acceptance without kernel when interval or validation helpers succeed; I did not read or audit `validation::*` implementations.
6. Unsure: whether those helpers over-approximate ranges in a way that admits values outside the declared constrained type.

## What I did not read

- Full `assignment_is_overwritten_before_consumption` remainder after `checker.rs:494`.
- Full `cast_bound_evidence` after `bounded_denotation.rs:3234`.
- `validation::occurrence_integer_expression_fits_range`, `validation::arrival_integer_expression_bounds`, and related `validation::*` bodies.
- Exhaustive non-test enumeration of every `convertible` call site in `omega-rust`.
- Corpus or compiler certificate builders that populate `semantic_axioms` prefixes for affine witnesses.

SLICE COMPLETE

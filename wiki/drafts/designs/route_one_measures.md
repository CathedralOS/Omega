# Route one measures

Measurement record, 2026-11-11, for the gate of route one in
[check on the lowered form](check_on_lowered_form.md#route-one-before-anything-else-is-built-on-it).
No compiler code was changed. It takes the eight measures that record asks
for and states, clause by clause, whether the route passes its stop rule.

**Route one does not pass its gate yet, and it has not triggered the stop
rule.** It proves relations between two values through its single
certificate route, so that clause passes. It does not yet agree with the
current checker on the widened subset. Every disagreement is classified: prover gaps, programs the current checker
should reject, one open owner question, and one lowering scope error that
wrongly accepts `fail/` programs. None needs a second route. "Clearly
smaller than what it replaces" cannot be decided today. It retires none of
Psi 05, validation or Psi 06: every program, the one-line ones included,
runs 45% of those stages' instrumented lines to check the bundled library, and the
library is outside the subset. The owner can withdraw on that ground, or
keep the route until the library enters the subset.

**Corrected 2026-11-11.** Two things in this record no longer stand, and
the numbers below are otherwise unchanged. The stop rule it judges has been
replaced by [a four-condition gate](check_on_lowered_form.md#the-gate),
because this record showed the rule could not be evaluated. And the last
two sentences above read the specification wrongly: checking the whole
bundled library on every compile is what the current pipeline does, not
what a check must do. [Validation subjects](../../spec/build/validation_subjects.md)
makes the library a dependency, checked only where a program demands it, so
the route need not wait for "the library enters the subset"
([the correction](check_on_lowered_form.md#the-library-is-a-dependency-not-a-subject)).
"One route per obligation kind" is also
[restated](check_on_lowered_form.md#one-meaning-per-obligation).

Amended 2026-11-11, after this record: the lowering scope error is closed.
`structure_elaboration::elaborate` now takes the check's entries, the
declarations its Build selected and its subject package, and lowers every
declaration that check claims, used or not, as
[validation subjects](../../spec/build/validation_subjects.md) requires; a
subject declaration it cannot lower makes the whole compilation fall back.
The rows below that state the error keep their measured numbers and say
where they are superseded. No other number here was re-measured.

The verdict, the evidence and its limits come first. How each number was
taken comes after, with exact commits, hosts and commands. Estimates are
labelled. Numbers quoted from another PR are attributed to it and were not
re-measured here, except where stated.

## Verdict against the stop rule

The stop rule: "If the route is not clearly smaller than what it replaces,
cannot reach agreement on the widened subset, or cannot prove two-value
relations through its single certificate route, the replacement stops and
this record is withdrawn in favor of migrating in place."

| Clause | Today | Evidence |
| --- | --- | --- |
| Clearly smaller than what it replaces | **Not decidable yet; leans against** | The route is 11,461 lines (7,141 production code) at `#2317`'s head. It replaces 792,766 lines in name (`main`). For the programs it handles it can replace almost none, because those programs need most of the old code to check the library ([measure 1](#1-lines-against-the-code-it-would-retire)). Construct rows so far added 8 to 858 production code lines each ([measure 4](#4-lines-added-per-construct-row)). |
| Reach agreement on the widened subset | **Fails today; causes are classified** | On the 121 programs the unscreened sweep lowers, run at `#2317`'s head: 101 agree and 20 disagree. 12 are prover gaps, 6 are current-checker errors and 2 wait on owner question Q6. `#2317`'s own two-way probe (quoted) has 135 of 223 agreeing and 13 `fail/` programs wrongly accepted through one scope error. No disagreement needs a second route. Agreement needs those prover rules and a ruling on Q6. It also needed every checked machine elaborated, not only the entry's closure; that is done since ([measure 7](#7-steps-that-drop-without-a-refusal)). |
| Prove two-value relations through one certificate route | **Passes** | `i < len`-style order steps, the correlated unsigned subtract and branch facts all reach the kernel as one certificate form. On the unscreened subset, 1 obligation at the `#2305` measurement head and 3 of the 121 programs' obligations at `#2317`'s head needed a relation, and each was kernel-checked. A relation through a multiply (`value * value >= value`) is still unproved ([measure 2](#2-verdict-agreement)). |

The four targets the record set beside the stop rule:

| Target | Today |
| --- | --- |
| All of the subset's obligations discharged with kernel-checked evidence | Not met. On the unscreened subset: 16 of 20 certificate obligations at the `#2305` measurement head, and 308 of 333 at `#2317`'s head. `#2317`'s own probe (quoted): 1,589 of 1,721. Every discharged obligation carries kernel-checked evidence. |
| No decisions keyed on text | Met. 0 in the three route crates at `#2317`'s head. |
| No step drops without a refusal | Not met. At the obligation level, every obligation gets one verdict. At the module level, `structure_elaboration::elaborate` lowers only the entry's call closure, so obligations in machines the entry never calls are never minted and never refused. `#2317` found 13 `fail/` programs accepted through it. Closed since this record: every declaration the check claims is a root, and `lowered_route_subjects` pins that none of those programs is accepted ([measure 7](#7-steps-that-drop-without-a-refusal)). |
| One route per obligation kind | Met. One prover for every certificate kind; the old checker tries 27 named routes for a call's `requires`. |

## 1. Lines against the code it would retire

Counted with `git show <commit>:<file>` over every `.rs` file in each
directory. A test is a `tests.rs` file, a file under `tests/`, or an inline
`#[cfg(test)] mod tests {`. Production code lines are non-blank lines that
do not start with `//`.

| | Total | Tests | Production | Production code |
| --- | ---: | ---: | ---: | ---: |
| `terminal-check-tables`, `main` `82d2ea5d5` | 990 | 106 | 884 | 454 |
| `structure-elaboration`, `main` | 5,160 | 1,378 | 3,782 | 3,324 |
| `lowered-discharge`, `main` | 3,313 | 1,017 | 2,296 | 2,068 |
| **Route on `main`** | **9,463** | 2,501 | 6,962 | **5,846** |
| `structure-elaboration`, `#2317` head `2a33e77d4` | 5,199 | 1,378 | 3,821 | 3,357 |
| `lowered-discharge`, `#2317` head (with `#2309`) | 5,272 | 1,564 | 3,708 | 3,330 |
| **Route at `#2317`** | **11,461** | 3,048 | 8,413 | **7,141** |
| Psi 05, `main` | 337,514 | 125 | 337,389 | 327,654 |
| `validation/src`, `main` | 331,424 | 0 | 331,424 | 319,864 |
| Psi 06, `main` | 123,828 | 0 | 123,828 | 120,354 |
| **Replaced in name, `main`** | **792,766** | | | **767,872** |

The proof kernel (`proof/admission`) is reused and not counted on either
side. `validation`'s `terminal-installation` (6,042) and
`representation-planning` (833) subcrates are not counted as replaced.

**Which old code the subset actually depends on.** Totals say nothing about
what the route would retire for the programs it handles, so the old stages
were run under coverage. One build of the `omega` CLI with
`-C instrument-coverage` ran `omega run --engine terminal` on three groups,
one program at a time:

- **Baseline:** `pass/build/conditional_root_binding`, a one-line entry
  (`machine launch() { let marker: u8 = 0; }`). It fails after checking,
  because it binds only a Windows entry, so its count covers checking but
  not all of Psi 06.
- **Subset sample:** 30 programs drawn with seed 2317 from the 121 that
  structure-only elaboration lowers ([measure 2](#2-verdict-agreement)).
- **Outside sample:** 30 programs drawn with seed 2317 from a seeded
  121-program sample (seed 2305) of programs it refuses.

In each sample, 27 programs ran to exit 0 and 1 ran to the trap it expects.
2 stopped after checking:
- in the subset sample, two with no entry bound for `linux_x86_64`;
- in the outside sample, one with no provider selected and one at native
  target lowering. Instrumented lines are lines with a
coverage counter.

| Lines | Psi 05 | validation | Psi 06 | All three |
| --- | ---: | ---: | ---: | ---: |
| Instrumented | 246,273 | 252,730 | 94,551 | 593,554 |
| Run by the one-line baseline | 113,793 | 119,170 | 33,572 | 266,535 |
| Run by the subset sample | 119,949 | 126,999 | 39,435 | 286,383 |
| Run by the outside sample | 131,201 | 138,731 | 45,141 | 315,073 |
| Subset sample, not baseline | 6,162 | 7,832 | 5,863 | 19,857 |
| Subset sample, neither baseline nor outside sample | 3,582 | 3,328 | 1,976 | **8,886** |

What this shows:

- **The library holds the old stages in place.** A one-line program runs
  45% of the three stages' instrumented lines, because every compile checks
  the bundled library (`compiled 28 source file(s)`). While the library is
  outside the subset, routing a subset program to the new checker removes
  none of that work and none of that code.
- **What the subset alone needs is small.** The 30 subset programs run 19,857
  lines beyond the baseline. 8,886 of those run in no outside program of the
  sample, an upper bound on what is specific to the subset (an estimate:
  30-program samples, and a line absent from 30 outside programs may serve
  others). Most of it sits where the record would predict:
  `contract_entailment.rs` 1,386, `checks.rs` 732, `checks_contracts.rs` 717,
  `checks_contracts_calls.rs` 693, `arithmetic_domains.rs` 595, and in Psi 06
  `attached_unit.rs` 532 and `retention.rs` 457.
- **So the ratio that decides this clause is not 11,461 against 792,766.**
  It is the route's size when the subset includes the library, against what
  the old stages then still need. Neither side exists yet. As an estimate
  only: the route is already larger than the 8,886 lines specific to its
  current subset.

## 2. Verdict agreement

Each program is checked by both routes through the comparison harness
(`omega-rust/omega/compilation/compiler/examples/route_one_comparison.rs`).
A disagreement is classified by reading the program, never averaged.

**The subset, found without a screen.** The unscreened sweep (below) ran
3,486 `pass/` and `run/` fixtures at the `#2305` measurement head. Structure-only
elaboration lowered 121 of them, 118 `pass/` and 3 `run/`. `#2305` had
measured 75 from `#2286`'s earlier sweep. The 121 were then run again at
`#2317`'s head.

| Head | Programs | Both accept | New route refuses an obligation | Whole module refused | Agreement |
| --- | ---: | ---: | ---: | ---: | ---: |
| `#2305` measurement head | 121 | 73 | 3 | 45 | 73 of 121 |
| `#2317` head `2a33e77d4` | 121 | 101 | 20 | 0 | **101 of 121** |

At the `#2305` head, 45 programs were refused as a whole module at the seam
between `#2305`'s discharge and `#2307`'s lowering:

- 35 with `invocation outside an operation`: a call with no normal
  completion;
- 10 with `clause names a value that is not a formal`: a callee `ensures`
  naming `result`.

`#2317` fixes both: a no-normal call adds no callee knowledge, and `result`
is renamed on each return edge. All 45 reach a verdict there.

**The 20 disagreements at `#2317`'s head**, each read:

| Cause | Class | Programs |
| --- | --- | ---: |
| Divide or remainder result bound: no definition-bound rule (`value / 2`, `value % divisor` under `ensures`) | prover gap | 5 (`runtime_bounded_dividend_remainder_exit`, `runtime_full_width_unsigned_quotient_exit`, `runtime_signed_minimum_remainder_postcondition_exit`, `run/contracts/scalar_crash_route_exit`, `scalar_crash_route_trap`) |
| A guard that contradicts `requires`; contradiction closes only inside a case split | prover gap | 4 (`contradictory_branch_call_admission_exit`, `_closes_obligation_exit`, `_result_guarantee_exit`, `contradictory_relational_branch_exit`) |
| Bitwise and, or, xor result bound | prover gap | 1 (`bitwise_singleton_signed_and_unsigned_exit`) |
| Relation through a multiply (`ensures result >= value` from `value * value`) | prover gap | 1 (`nonlinear_square_positive_operand_exit`) |
| Boolean `ensures result == value` on a return through a nested state | prover gap | 1 (`normal_return_expression_statement_sequence_exit`) |
| A nested machine with no `requires` divides or multiplies using its caller's path facts | current-checker error | 6 (`private_division_exact_pair_exit`, `_excluded_exit`, `private_division_signed_remainder_exit`, `runtime_boolean_divisor_state_exit`, `runtime_transitive_product_exit`, `runtime_boolean_guarded_product_exit`) |
| Callee result with no result-naming `ensures` | waits on Q6 | 2 (`boolean_result_all_normal_returns_exit`, `constant_boolean_return_join_exit`) |

- **Prover gaps.** In the programs I read (the bitwise, Boolean-return and
  remainder rows), each unproved obligation is true. The contradictory-guard
  and multiply rows are classified by `#2317`'s family readings, which name
  them.
- **Current-checker errors.** These follow `#2317`'s reading of
  `machines.md`: a nested declaration has its own contract and cannot use
  another invocation's live facts. For example, `machine divided(left, right)
  -> i64 { left / right }` has no `requires`, so it is not valid under its
  own contract.
- **Q6 in that measurement.** This was the then-open
  `callee-result-body-summary`, added by `#2317`. The 2026-11-11 settlement
  permits [checked local summaries](../../spec/language/state_contracts.md#local-call-summaries)
  and omitted private entry inference, while authored preconditions remain
  complete. The recorded classifications above are historical; the
  [acceptance plan](local_inference_and_contracts.md#acceptance) requires
  reconciliation of the affected rows, not a claim that they were rerun here.
- **Matching `#2317`'s families.** Every cause matches one of `#2317`'s
  families, and none is new. So `#2317`'s token screen missed none of these
  causes, though it can still have missed programs.

**Direction not covered here.** The 121 are all programs the current
checker accepts. `#2317`'s harness compares rejected programs too, through
`#2312`'s seam. Its own `fail/` run (quoted, not re-measured):

| `fail/` outcome | Programs |
| --- | ---: |
| Both reject | 119 |
| New route wrongly accepts: the defect is in a machine the entry never calls | 13 |
| New route accepts: a Build or duplicate-declaration check the route does not own rejects | 5 |
| Both accept, then later stages reject | 4 |

The 13 are the lowering scope error in the targets table. Until every
checked machine is elaborated, the route must not govern a program on its
own verdict.

Closed since this record, and wider than 13. `#2317` did not record which
programs it counted, so they were re-derived by a sweep on `main` at
`2df885a2a` with the fix applied, and the verdicts were run again by the test
on `e265ffe73`. Of 238 `fail/` fixtures whose entry is
`machine main() {}` (every such fixture of `arithmetic`, `calls`,
`contracts`, `proofs`, `ranges` and `termination`, and the screened ones
elsewhere), 49 bind an entry and reach the checker; the other 189 bind no
entry or stop before Psi 04. The entry-only scope accepted all 49. With
every subject declaration a root, the route rejects 5 and hands 44 back to
the current checker, and accepts none. The four programs `#2317` names are
among them. The other 567 `fail/` fixtures with an empty entry were not
swept.

## 3. Check time per program

Wall clock, by `std::time::Instant` around each route's own calls in the
comparison harness. The two numbers do not measure the same work. The
current figure covers fixture snapshot, discovery, review, Build execution,
the whole check and, from `#2317` on, one clone of the typed trees. The new
route's figure covers elaboration and discharge from those typed trees.

| Run | Programs | Current checker: total, median | New route: total, median, max |
| --- | ---: | --- | --- |
| Unscreened sweep, `#2305` measurement head (programs reaching a verdict) | 76 | 929.9 s, 11.36 s | 65.1 ms, 199 µs, 18.8 ms |
| The same 121, `#2317` head | 121 | 1,421.3 s, 11.27 s | 334.0 ms, 223 µs, 72.2 ms |
| `#2317`, its own probe (quoted) | 223 | 2,282.2 s, 10.02 s | 5.91 s, 3.49 ms, 724.9 ms |

Three jobs ran at once on a 4-CPU host at `opt-level` 1, so the absolute
times are inflated. The ratio holds in every run: the new route takes
milliseconds where the current pipeline takes about ten seconds. Measure 1
suggests most of those ten seconds checks the library. That is an inference
from lines run, not a timing, so this is not yet a like-for-like checker
comparison.

## 4. Lines added per construct row

Each commit's `.rs` diff (`git show -U0 <commit> -- '*.rs'`), split into the
three route crates (route) and the stages before them (upstream), and into
production and test lines. Test lines are a `tests.rs` file, a file under
`tests/`, or an inline test module.

| Construct row | Commit | Route production code | Route comments | Route test code | Upstream |
| --- | --- | ---: | ---: | ---: | --- |
| Contract `result` named by identity (`#2307`) | `e189ef5d7` | +110 −47 | +28 −5 | +109 −1 | +23 −6 code, +14 comment, +7 test |
| `insert_tree` keeps the `result` owner (`#2307`) | `766202c22` | 0 | 0 | 0 | +40 −1 code (an inline test among them), +11 comment |
| Crash statements, crash routes, no-normal calls (`#2307`) | `ba5e10eb4` | +676 −92 | +111 −6 | +354 −15 | 0 |
| Short-circuit with a crashing right operand (`#2307`) | `74aa123ca` | +8 −2 | +3 | +53 −1 | 0 |
| Crash sites as operations (`#2307`) | `df6d6b0b1` | +31 −17 | +26 −11 | +137 −37 | 0 |
| `crashes none` kept apart from an omitted clause (`#2307`) | `ead85d764` | +31 −4 | +17 −3 | +40 | +16 code, +16 comment, +14 test |
| Relations, branch facts, per-exit `ensures` (`#2305`) | `1e34e2732` | +538 −79 | +77 −9 | +347 −80 | harness +314 code |
| Multiply result bound (`#2311`) | `3b48c589b` | +133 −37 | +16 −4 | +112 | kernel tests +583 code |
| Join predicates and crash coverage (`#2317`) | `2a33e77d4` | +858 −88 | +74 −3 | +250 | harness +20 −36 code |

The `#2307` commits are those on its branch `work/psi-widen-structure-lowering`,
squash-merged as `e9b6fd49d`. `#2305`'s `1e34e2732` is on
`work/compiler-route-one-comparison`, squash-merged as `6bd43953f`.

A construct row costs from a few lines to a few hundred production code lines
in the route, plus about as many test lines. The two largest families,
crash lowering (`ba5e10eb4`) and join predicates with crash coverage
(`2a33e77d4`), come to 584 and 770 net. As an estimate only: if each of the
roughly ten lanes the record plans after route one costs what those two did,
the route grows by about 6,000 to 8,000 production code lines.

## 5. Obligations discharged with kernel-checked evidence

Every discharged certificate obligation carries a certificate that
`proof/admission` re-decided. No obligation is accepted without one, so
the open question is coverage, not trust.

| Run | Certificate obligations | Kernel-checked | Unproved | Outside the route |
| --- | ---: | ---: | ---: | ---: |
| Unscreened sweep, `#2305` measurement head, 76 programs | 20 | 16 (all `Judged`) | 3 | 1 `CrashCoverage` |
| The same 121, `#2317` head | 333 | 308 (2 with the core answering `Refused`) | 25 | 0 |
| `#2317`'s probe, 223 programs (quoted) | 1,721 | 1,589 (7 with the core answering `Refused`) | 132 | 0 |

At `#2317`'s head, on the 121 programs:

- arrival invariants: 114 of 114;
- crash coverage: 91 of 91;
- call requirements: 58 of 63;
- operation safety: 32 of 40, 2 of them with the core answering `Refused`;
- normal guarantees: 13 of 25;
- published clauses: 12 of 24 held.

By kind, at the `#2305` measurement head: `OperationSafety` 5 of 6,
`CallRequirement` 9 of 11, `NormalGuarantee` 2 of 2, and 2
`PublishedGuarantee` clauses held from their exits. The census in the record
counted 814 accepted obligations with no evidence in the current product.

## 6. Decisions keyed on text

None. Searched in the non-test source of the three route crates at `#2317`'s
head:

```
grep -nE 'as_str\(|== *"|"\s*==|from_name|\.name\b|display_|to_string\(\)|starts_with|strip_prefix|split\(|Identifier|is_reserved|HashMap|BTreeMap|&str'
```

Every hit is diagnostic text or a comment:

- `&'static str` causes for `Unproved` (`prove.rs`, `lib.rs`);
- `Refusal.detail` strings built with `format!`;
- `read.rs:8`, a comment naming the `PrimitiveType::from_name` spelling route
  the reader avoids;
- in `read.rs`, refusal of a string literal.

The one spelling decision the route depends on is upstream, and it is the
one the record asks for. Psi 02 (`symbols.rs`, `#2307`) tells the reserved
`result` apart from a parameter or local spelled `result` once, and records
it as the identity `reserved_result_of`. `crashes none` is read from the
boolean `declares_no_crash_route`, not from the absence of a row.

## 7. Steps that drop without a refusal

Searched with `_ =>`, `.ok()`, `unwrap_or`, `filter_map`, `let _ =` and
`unwrap_or_default` in the same files: 93 hits at `#2317`'s head (52 in
`prove.rs`). Classified:

| Class | Where | Effect |
| --- | --- | --- |
| Refuses with a reason | `ok_or` / `ok_or_else` into `Refusal` after `.ok()` index conversions (`lib.rs:242`, `crashes.rs:103`, `reuse.rs:127`); `_ =>` arms returning `Err` (`crashes.rs:85,127`, `lib.rs:406,532,676,692`, `read.rs`) | refusal |
| Ends `Unproved` | about 40 `.ok()?` and `?` inside the prover's bound, relation and split search (`prove.rs`); the goal fails with a cause | no wrong acceptance |
| Builds a proposition or filters | conjunction and disjunction arity (`crashes.rs:53,163`, `joins.rs:143,145`, `canonical.rs:96`); filters over equations and rows (`joins.rs:255,427`); `order()` and `may_sets.rs:66`, where an unknown entry absorbs a join as specified | not a drop |
| Index conversion | `unwrap_or(usize::MAX)` (`graph.rs:30`, `lib.rs:529,658`, `prove.rs`); the following `.get()` misses, then refuses or ends `Unproved` | no wrong acceptance |
| Weakens, then checks | `joins.rs:280` keeps a path fact untransported when transport fails. A join predicate that an arrival cannot prove becomes `Truth` and is listed in `weakened_joins`. The predicate is proved on every arrival before it is used. | weaker knowledge, never acceptance |
| Leaves `result` unrenamed | `lib.rs:788`, `_ => Vec::new()` when an exit has no scalar return value; the clause still names `result`, which nothing proves | ends `Unproved` |

Two places lose information without dropping an obligation. When the case
split also fails, `prove.rs:407` reports the direct search's cause and drops
the split's. And whole machines drop, as the targets table says: only the
entry's call closure is elaborated. That is not one of the sites above. It
is why the target is not met.

That drop is closed since this record. `elaborate` takes a `Compilation`
(the Build's entries, what its selections name and the subject package of
the request). A selected machine or data type is a root, and a selection of
anything else refuses the compilation. Every declaration of the subject is a
root too: a machine is lowered and judged whether or
not anything calls it, a `data` declaration is resolved as a record, and any
other kind of declaration refuses the compilation as `Unsupported`, as does
one with no source file. The 93 sites above were not searched again, so the
target as a whole is not re-measured. Two parts of the closure are still
outside the route and are stated, not dropped: the Build program in
`build.omg`, which the Build activation checks with the current checker, and
the bound target's entry-contract source, which the current checker keeps as
a subject. [The gate](check_on_lowered_form.md#the-gate), condition 1, holds
the open question.

## 8. Proof routes per obligation kind

| Obligation kind | Old checker | New route |
| --- | --- | --- |
| A call's `requires` | 27 named routes tried in order (below), plus a per-membership-case split; none emits evidence | `Prover::prove`, one certificate form |
| Operation safety (bounds, overflow) | Proof plans; per the census, none reaches a certificate | the same `Prover::prove` |
| A normal return's `ensures` | Psi 05's contract checks | the same `Prover::prove`, one obligation per return edge |
| A published `ensures` clause | | one function over its exits |
| Join predicates, crash coverage (`#2317`) | | goals for the same `Prover::prove` |

The old routes, from `check_call_requires` (`checks_contracts_calls.rs:3188`,
unchanged from `1e34e2732` to `main` `82d2ea5d5`):

- **Top level, 14 routes:**
  - `selected_evidence_proves_requirement`
  - `captured_boolean::judgment`
  - `boolean_requirements::proves`
  - `semantic_contexts_prove_contract_fact`
  - `transparent_requirement_proves`
  - `proven_predicates_grant_domain`
  - `transition_guard_proves_requires`
  - `string_literal_grants_domain`
  - `value_call_return_domain_grants`
  - `qualified_argument_grants_domain`
  - `subslice_grants_domain`
  - `parameter_domain_grants`
  - `reference_domains::proves`
  - `termination::profile_requirement_covered`
- **Inside `boolean_requirements::proves`, 11 routes plus a conjunction-split recursion:**
  - `closed_boolean_value`
  - `call_bounds::proves`
  - `call_entry_contexts_prove_boolean_contract_expression`
  - `proves_in_context`
  - `structural_call_requirement`
  - `explicit_subject_call_requirement_proven`
  - `incoming_guard_proves_requires`
  - `transition_guard_proves_boolean_requirement`
  - `contradictory_edge::proves`
  - `standing_domain_fact_proves_requirement`
  - `proves_call`
- **Inside `captured_boolean`, 2 routes:** `captured_contract_proven` and
  `captured_float_requirement`.

Inside the new prover, a goal is tried in five ways:

1. a known fact;
2. a closed relation;
3. bounds and order chains;
4. affine arithmetic;
5. one level of case split with contradiction.

All five produce the same certificate form, which the kernel checks. That is
one route with internal search, not five routes: none of the five can accept
without the kernel.

## What was not measured

- **The route's diagnostics on rejected programs.** Measured separately, on
  every `fail/` fixture, in
  [route one diagnostics](route_one_diagnostics.md): whether each rejection
  points a programmer at the fixture's defect.
- **Rejection agreement on an unscreened selection.** The harness at the
  `#2305` measurement head obtains typed trees only from a successful check,
  so it never runs the new route on a program the current checker rejects.
  `#2317`'s harness can, and its `fail/` numbers are quoted above. Its `fail/`
  selection is a token screen whose miss rate is unmeasured.
- **The whole corpus at the current head.** The unscreened sweep ran at the
  `#2305` measurement head, which held only `#2307`'s first three commits.
  `#2317`'s head was run only on the 121 programs that sweep lowered. A
  program that lowers only at `#2317` was not found here; `#2317`'s own probe
  covers part of that.
- **Native execution.** The harness runs no native code.
- **`Build::depend` fixtures, `facts/`, `pending/` and `fail/`** were left out
  of the unscreened sweep.
- **Coverage at the current head.** The coverage build is from the `#2305`
  measurement head. Psi 05, validation and Psi 06 differ from `main`
  `82d2ea5d5` by +733 −176 lines in 22 files, about 0.1% of them.

## How each number was taken

Host: a cloud Linux x86_64 container with 4 CPUs and 15 GB of memory, mbx
1.29.2, toolchain `nightly-2026-09-04`. Builds used
`CARGO_PROFILE_DEV_OPT_LEVEL=1`. `#2305`'s own numbers were taken at the
workspace dev profile on `omega-linux`, so times differ from those.

**Commits.**

- **`#2305` measurement head:** `1e34e27325d` (the pushed `#2305` head
  before its rebase, which merged `#2307`'s `240d99cd7`, `406a31044` and
  `734ada2dc`), plus one local, unpushed arm in the harness:
  `Some(ValueOrigin::MachineResult) => "result",`. Without it the harness at
  that head does not build (`E0004`, non-exhaustive match). The landed
  `#2305` carries the same arm.
- **`#2317` head:** `2a33e77d45a` (open draft, on `#2309`'s `c2c84ed4c`).
- **`main`:** `82d2ea5d5`.

**Unscreened sweep.** Every `tests/omega/{run,pass}/**/main.omg` outside
`facts/` whose `build.omg` does not call `Build::depend`: 3,486 fixtures,
155 `run/` and 3,331 `pass/`. One process per fixture, `timeout 300 nice -n 19`,
three at a time:

```
mbx build -p compiler --example route_one_comparison
target/debug/examples/route_one_comparison <fixture>/main.omg < /dev/null
```

Its first result is stamped 2026-11-11T00:25Z and it ended at 04:09Z. It
was resumed twice, after a container restart and after the 2-hour background
limit. Each fixture's result is written once, so no fixture was skipped or
run twice, and no process failed. Outcomes:

| Current checker / new route | Fixtures |
| --- | ---: |
| accepted / outside the subset | 2,028 |
| accepted / no program entry | 1,027 |
| rejected / not reached | 310 |
| accepted / accepted | 73 |
| accepted / whole module refused | 45 |
| accepted / refused (unproved obligation) | 3 |

The 121 programs that lowered (the last three rows) are the subset. The top
elaboration refusals among the 2,028 were:

| Refusal | Fixtures |
| --- | ---: |
| `type nominal type` | 630 |
| `statement uninitialized local` | 399 |
| `machine service reach` | 263 |
| `type fixed array` | 199 |
| `type constrained` | 97 |
| `call static application arguments` | 81 |
| `type reference` | 53 |
| `parameter mutable parameter` | 39 |

`#2317`'s screened probe ranks `expression cast` first. Its screen excluded
`data`, `[` and other tokens, which is why `type nominal type` and
`type fixed array` rank lower there.

**Re-run at `#2317`'s head.** The same script ran the 121 subset programs
from 05:58Z to 06:06Z, after
`mbx build -p compiler --example route_one_comparison` at `2a33e77d4`.

**Coverage.**

```
RUSTFLAGS="-C instrument-coverage" CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=<cov> mbx build -p omega
LLVM_PROFILE_FILE=<cov>/<group>-%4m.profraw <cov>/debug/omega run --engine terminal <fixture>/main.omg < /dev/null
llvm-profdata merge -sparse <group>-*.profraw -o <group>.profdata
llvm-cov export -format=lcov -instr-profile <group>.profdata <cov>/debug/omega
```

Run from 2026-11-11T04:25Z to 05:47Z, one program at a time, about 80 s
each. Lines were attributed by source path to
`pipeline/05_typed-trees-to-checked-trees/`, `validation/src/` and
`pipeline/06_checked-trees-to-lowered-psi/`. A line counts when its counter
is non-zero.

**Line counts, splits and greps** were taken with `git show` and `git
archive` at the commits named in each section, not on a working tree.

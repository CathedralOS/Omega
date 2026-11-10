# slice_0044

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0333 | omega-rust/psi/compilation/validation/src/arithmetic_domains.rs:795 | REACHABLE | Narrowing check runs only when both interval ends are known. |
| A0334 | omega-rust/psi/compilation/validation/src/arithmetic_domains.rs:6470-6481 | REACHABLE | Queues `IntegerRangeObligation` and returns false with no diagnostic on that branch. |
| A0335 | omega-rust/psi/compilation/validation/src/arithmetic_domains.rs:4975-4983 | CLOSED | Only `I8`–`I64` are signed fixed integers; all are listed in `signed_minimum`. |
| A0336 | omega-rust/psi/compilation/validation/src/arithmetic_domains.rs:5246 | REACHABLE | Proof `Int` divide/mod passes `signed_pair_excluded: true` without a MIN/-1 interval proof. |
| A0337 | omega-rust/psi/compilation/validation/src/arithmetic_domains.rs:10885-10938 | REACHABLE | Fixpoint loop caps at `states.len()+1` rounds and keeps the last `current` without a convergence check. |
| A0338 | omega-rust/psi/compilation/validation/src/arithmetic_domains.rs:13395-13416 | REACHABLE | Contradictory narrowed intervals reset the environment; no diagnostic from this block. |
| A0339 | omega-rust/psi/compilation/validation/src/arithmetic_domains.rs:3170-3202 | REACHABLE | Checked-body callee `ensures` are seeded as caller facts without proving them at the call site. |
| A0340 | omega-rust/psi/compilation/validation/src/arithmetic_domains.rs:13462 | UNDECIDED | Literals and builtin type members are always “in state”; impact on unsound acceptance beyond that not traced. |

## A0333

**Trace.** Digest row cites `ad:795`; opened `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs:756-804` (`check_narrowing_assignment`, read). At `795` the error fires only when `effective.low().is_some() && effective.high().is_some() && !range.contains(effective)` (read). If either endpoint is missing, the condition is false and no narrowing diagnostic is pushed (read). `Interval::contains` at `16466-16468` is `self.intersect(inner) == inner` (read), so a one-sided or unbounded `effective` can still intersect the primitive range without `contains` failing when an endpoint is unknown (inferred).

**Callers.** Search `check_narrowing_assignment` under `omega-rust/`: **9** matches (read): definition at `arithmetic_domains.rs:756`; calls at `arithmetic_domains.rs:888`, `22509`, and `program_validation.rs:1851`, `2229`, `2523`, `2938`; re-export/use lines at `22311`, `26765`.

**Acceptance path.** Upward from `program_validation.rs:1851` (read): assignment validation passes a stored-value interval into `check_narrowing_assignment`. Input that reaches `795`: a narrowing store to a fixed integer type where static analysis has a value interval with only one known bound (for example a positive lower bound and unknown upper) such that values outside the target primitive range remain possible; the branch is skipped and validation continues without decision-17 error (inferred).

**Verdict REACHABLE.** Smallest story (plain words): assign into a smaller integer type while the analyzer knows only a one-sided bound on the source, so an out-of-range value is still possible, yet no narrowing rejection is emitted.

## A0334

**Trace.** Opened `arithmetic_domains.rs:6426-6504` (`require_representable_result` on binary `Operation`, read). After `result_fits` fails and `definitely_outside` is false, if `recipe` is `Some` and `range.low` and `maximum` are available, `6469-6480` pushes `decisions.integer_range_obligations` and `6481` returns `false` without calling `report_unrepresentable` (read). `report_unrepresentable` runs only at `6502` on fall-through paths (read).

**Callers.** Search `require_representable_result` under `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs`: **2** production call sites (read): `5721`, `5975` (both in builtin binary analysis walk, read).

**Acceptance path.** Caller at `5721` (read) sets `representable` from the return value but does not require it to be true for compilation to proceed; obligations accumulate in `ArithmeticDecisions` and land in `ProgramValidationFacts.integer_range_obligations` at `program_validation.rs:4618-4621` (read). Consumer outside validation: `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/attached_unit.rs:7461-7464` skips non-pending obligations but errors on pending at lower time (read); validation itself does not emit a diagnostic on the `6481` path (read).

**Verdict REACHABLE.** Smallest story: an exact-domain integer operation whose result interval may exceed the carrier but is not provably disjoint from the allowed range gets a deferred range obligation and validation accepts the machine with no error from this helper.

## A0335

**Trace.** Opened `arithmetic_domains.rs:4975-4983` (`signed_minimum`, read) and enclosing `validate_operands` `5077-5151` (read). `signed_minimum` returns `None` only for `PrimitiveType` variants not matched in the `I8`–`I64` arms (read). `validate_operands` at `5091` returns early unless `primitive.is_fixed_integer()` (read).

**Types.** Opened `omega-rust/psi/compilation/representations/typed-trees/src/type_system.rs:4591-4605` and `4633-4644` (read): fixed integers are exactly `I8`, `I16`, `I32`, `I64`, `U8`, `U16`, `U32`, `U64`; signed fixed integers are the four `I*` variants only (read).

**MIN/-1 check path.** `ProvenDefinedness::from_intervals` and the `validate_operands` body use `signed_minimum(primitive)` at `5031`, `5124`, `5146` (read). For every signed fixed integer, `signed_minimum` is `Some` (read). Unsigned and non-integer primitives never reach this divide/mod definedness path via `is_fixed_integer` (read).

**Verdict CLOSED.** Closing gate: `validate_operands` `5091` plus `signed_minimum` arms `4977-4980` cover all signed fixed integer carriers that can hit partial divide/mod checking; digest concern about “signed type outside I8–I64” does not apply to `PrimitiveType` as defined (read).

## A0336

**Trace.** Opened `arithmetic_domains.rs:5225-5251` (`validate_proof_integer`, read). `5246` sets `signed_pair_excluded: true` unconditionally in `ProvenDefinedness` passed to `report_partial` (read). Contrast `validate_operands` `5077-5148`, which derives `signed_pair_excluded` from intervals and `signed_minimum` (read).

**Callers.** Search `validate_proof_integer` under `omega-rust/`: **2** matches (read): definition `5225`; call `omega-rust/psi/compilation/validation/src/contract_entailment.rs:23774` when proof `Int` divide/mod with mathematical operands (read).

**Acceptance path.** `report_partial` `5049-5054` treats `signed_pair_excluded` as satisfied when `true`, so signed `MIN / -1` definedness is not required for proof integers on this path (read). Input: proof obligation using exact `Int` division where neither operand is proven to exclude MIN or -1 (inferred from hard-coded `true`).

**Verdict REACHABLE.** Smallest story: a proof proof-context divide whose operands could still be MIN and -1 together is not rejected for missing the signed overflow pair exclusion.

## A0337

**Trace.** Opened `arithmetic_domains.rs:10781-10948` (`incoming_environments`, read). Fixpoint loop `for round in 0..=states.len()` at `10885-10838` (read). On stable `next == current`, breaks at `10928-10930` (read). If never stable within the round cap, loop ends and `current` remains the last `next` assignment with no convergence diagnostic (read).

**Callers.** Search `incoming_environments` under `omega-rust/psi/compilation/validation`: **4** matches (read): definition `10781`; calls `10171`, `14679`, `14705`.

**Acceptance path.** Used to seed per-state `ValueEnvironment` before arithmetic walks (read). Input: multi-state machine with in-machine arrivals whose join needs more than `states.len()+1` updates; analysis uses truncated environments (inferred).

**Verdict REACHABLE.** Smallest story: cyclic cross-state fact transport that has not equilibrated within the round budget still proceeds with the last iterated environments.

## A0338

**Trace.** Opened `RequirementSeeds::apply` `13375-13417` (read). After applying requires-driven narrows, `13395-13400` detects any stored interval with `low > high` (read). `13401-13415` builds `required` from the same requires/memberships only and assigns `*environment = required` (read). No `Diagnostic` push in this function (read).

**Callers.** `RequirementSeeds::apply` invoked from `incoming_environments` `10904-10905`, `10946` and related arrival seeding (read).

**Acceptance path.** Contradictory requires facts produce empty/ inconsistent intervals; environment is replaced rather than rejected (read). Downstream obligation interaction after replacement not fully traced here (read).

**Verdict REACHABLE.** Smallest story: mutually incompatible `requires` narrow intervals to contradiction and compilation continues after discarding prior environment content without an error from this site.

## A0339

**Trace.** Opened `seed_cited_element_bounds` `3127-3183` (read) and `checked_callee` `3185-3202` (read). `3170-3181` iterates callee `ensures_expression_facts` and `seed_conjunct` into caller `environment` (read). `checked_callee` filters `MachineSupplyMode::CheckedBody` and `body_is_present` `3192-3196` (read).

**Callers.** Search `seed_cited_element_bounds` under `omega-rust/`: **4** matches (read); production call `program_validation.rs:2369` (read).

**Acceptance path.** On a direct call to a checked-body machine, callee postconditions become caller interval facts at this site without re-proving ensures here (read). Soundness depends on callee body verification elsewhere (inferred), not established in this function (read).

**Verdict REACHABLE.** Smallest story: a call imports callee `ensures` as assumed facts in the caller environment even though this site does not discharge those clauses locally.

## A0340

**Trace.** Opened `condition_belongs_to_state` `13444-13549` (read). Early `true` for contract constant recipe `13451-13455`, literal `i64` `13457-13458`, builtin type member name `13461-13463`, and literal expression nodes `13527-13530` (read). Used from `RequirementSeeds::new` `13287-13305` to filter which `requires` enter seeds (read).

**Callers.** Search `condition_belongs_to_state` in `arithmetic_domains.rs`: **13** matches (read) (1 definition + recursive/internal uses in `RequirementSeeds`).

**Undecided.** Read confirms literals and builtin members always pass the filter (read). Whether that lets a program that should be rejected slip through for non-literal requires shapes (calls, cross-state names) depends on paths not opened in this slice (read). Digest note “open beyond that” [089] matches this gap (read).

**Verdict UNDECIDED.** Would need tracing of unsound requires shapes beyond literals/builtins through downstream narrowing and diagnostics.

## What I did not read

- Full bodies of `seed_conjunct`, `report_partial` callers beyond cited spans, and complete `contract_entailment` proof-integer formation aside from the call site at `23774` (not needed for row verdicts above).
- Remaining `IntegerRangeObligation` discharge logic in stage `05_typed-trees-to-checked-trees` beyond the lowering `prepare` entry at `attached_unit.rs:7456-7489`.
- Area 13 digest lines beyond the preamble and the eight TSV rows (read: preamble `digests/area_13.md:1-9`, rows from `grok/A/inputs/slice_0044.tsv`).

SLICE COMPLETE


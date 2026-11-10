slice_0044

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0333 | arithmetic_domains.rs:795 (read 728-733, 791-803) | REACHABLE | Narrowing error only runs when both interval ends are `Some`; one-sided or `stored_value_carrier` `(None,None)` skips the fit check. |
| A0334 | arithmetic_domains.rs:6463-6500 | CLOSED | Deferral queues obligations or pending formations; `discharge_storage_formations` and pending-obligation consumers enforce before completion. |
| A0335 | arithmetic_domains.rs:4975-4983 | CLOSED | `PrimitiveType` has no signed fixed integer outside I8–I64, so `signed_minimum` `None` never hits division on another signed carrier. |
| A0336 | arithmetic_domains.rs:5246 | CLOSED | Proof-integer division validation intentionally sets `signed_pair_excluded: true` for unbounded proof `Int`, not fixed carriers. |
| A0337 | arithmetic_domains.rs:10885-10938 | REACHABLE | Round cap can exit with `next != current` and use the last join with no diagnostic, widening facts. |
| A0338 | arithmetic_domains.rs:13395-13416 | UNDECIDED | Contradiction reset drops non-requires facts; I did not trace downstream obligation checks on the replaced environment. |
| A0339 | arithmetic_domains.rs:3185-3202 | REACHABLE | `checked_callee` picks the first qualifying machine by symbol; wrong callee can seed caller bounds from another body's `ensures`. |
| A0340 | arithmetic_domains.rs:13462, 13527-13530 | CLOSED | Cited arms only auto-accept contract literals and builtin type-member names, which are state-independent constants. |

### A0333

1. Read `stored_value_carrier` and `check_narrowing_assignment` in full at `arithmetic_domains.rs:720-804` (read).
2. Search `\bcheck_narrowing_assignment\b` in `omega-rust`: 9 hits (`program_validation.rs` 4, `arithmetic_domains.rs` 5).
3. Read call sites in `program_validation.rs:1851`, `check_validated_value_narrowing` at `arithmetic_domains.rs:888`, and `finish_diagnostics` at `program_validation.rs:4819-4826` (read); did not read every match-arm narrowing path in `arithmetic_domains.rs:806-865`.
4. Input reaching the accepting branch: assignment narrowing where `effective` has only one bounded end, or `source` is `None` from lines 728-733 so `effective` is not intersected with the source carrier range (`arithmetic_domains.rs:791-794`).
5. **Verdict: REACHABLE** — A program that stores into a fixed-width integer when static analysis knows a lower bound above the carrier maximum but leaves the upper bound unknown should be rejected yet passes because line 795 requires both `low` and `high` to be `Some` before emitting the narrowing error.

### A0334

1. Read `require_representable_result` in full at `arithmetic_domains.rs:6426-6504` and its caller in `expression_analysis` at `arithmetic_domains.rs:5721-5729` (read).
2. Search `\brequire_representable_result\b` in `omega-rust`: 3 hits, all in `arithmetic_domains.rs` (definition plus two call sites).
3. Read consumers: obligation push at `arithmetic_domains.rs:6470-6481`; pending formation at `6485-6500`; aggregation at `program_validation.rs:4537-4621`; `discharge_storage_formations` at `program_validation.rs:3897-3919` and `published_facts.rs:11163`; `storage_formations::discharge` at `arithmetic_domains.rs:24226-24267`; pending closure check at `build-time-evaluation/src/machine_execution.rs:952-1025`; lowering scan at `pipeline/06_checked-trees-to-lowered-psi/src/attached_unit.rs:7461-7494` (read).
4. Input: exact-domain binary whose result interval is neither proven in-range nor definitely outside, with a target-dependent recipe and (for the pending path) `allows_pending_formations` plus empty storage graph (`arithmetic_domains.rs:6485-6488`).
5. **Verdict: CLOSED** — The no-diagnostic `return false` is intentional deferral: pending formations must pass `discharge` (`arithmetic_domains.rs:24246-24267`) or error at `published_facts.rs:11163`; queued `IntegerRangeObligation` rows stay `is_pending` (`contract_and_obligation_facts.rs:738`) until target discharge or block in-demand closure (`machine_execution.rs:1010-1020`) and are prepared for lowering (`attached_unit.rs:7461-7494`).

### A0335

1. Read `signed_minimum` and `ProvenDefinedness::from_intervals` at `arithmetic_domains.rs:4975-5034` (read).
2. Search callers of `signed_minimum`: only within `division_definedness` in the same file (inferred from read of `validate_operands` at `5077-5150`).
3. Read `PrimitiveType` enum at `type_system.rs:4591-4605` and `is_fixed_integer` / `is_integer` at `4718-4720` (read).
4. Input: `signed_pair_excluded` uses `signed_minimum(primitive)` which is `None` for any primitive other than I8/I16/I32/I64 (`arithmetic_domains.rs:4975-4981`); `validate_operands` gates on `primitive.is_fixed_integer()` (`arithmetic_domains.rs:5091-5097`).
5. **Verdict: CLOSED** — No production signed fixed integer exists beyond I8–I64 (read `type_system.rs:4591-4605`), so the missing MIN branch cannot apply to a division on another signed carrier; entry’s hypothetical wider signed type is not present in the type system.

### A0336

1. Read `validate_proof_integer` in full at `arithmetic_domains.rs:5225-5251` (read).
2. Search `\bvalidate_proof_integer\b` in `omega-rust`: 2 hits (`arithmetic_domains.rs` definition, `contract_entailment.rs:23774` call) (read caller header at `23667-23776`).
3. Read `report_partial` at `arithmetic_domains.rs:5037-5073` (read).
4. Input: proof-body divide/modulo where nonzero may be proven via `proof_integer_nonzero` but `signed_pair_excluded` is forced `true` at line 5246.
5. **Verdict: CLOSED** — Proof-integer formation is unbounded mathematical `Int` (inferred from proof validation context and hard-coded exclusion); MIN/-1 overflow is not a definedness requirement for that domain, matching `ResultRepresentable` omission via `signed_pair_excluded: true`.

### A0337

1. Read `incoming_environments` in full at `arithmetic_domains.rs:10781-10948` (read).
2. Search `\bincoming_environments\b` in `omega-rust`: 3 hits in `arithmetic_domains.rs` (`10171`, `14679`, `14705`) (count).
3. Read `incoming_guard_environments` wrapper at `arithmetic_domains.rs:14664-14708` (read); did not read full `ArrivalWalk` or all uses of returned environments.
4. Input: machines with in-machine state arrivals where the join fixed point does not stabilize within `states.len()+1` rounds (`for round in 0..=states.len()` at `10885`).
5. **Verdict: REACHABLE** — If stabilization needs more rounds, the loop exits at `10937-10938` with no error and later analysis uses an overstated join, which can fail to refute out-of-range stores or divisions that a exact join would catch.

### A0338

1. Read `RequirementSeeds::apply` at `arithmetic_domains.rs:13375-13416` and `new` at `13274-13307` (read).
2. Search: no separate caller search; `apply` is invoked from `incoming_environments` at `10904-10905` and `10946` (read in A0337).
3. Did not trace all uses of environments after contradictory `requires` replacement.
4. Input: after seeding, any stored interval has `low > high` (`arithmetic_domains.rs:13395-13399`), triggering full environment replacement with requires-only facts (`13401-13415`).
5. **Verdict: UNDECIDED** — Replacement clearly drops prior non-requires facts (read `13395-13416`), but I did not follow whether later narrowing or formation checks still fail closed on the shrunk environment.

### A0339

1. Read `seed_cited_element_bounds` and `checked_callee` at `arithmetic_domains.rs:3127-3202` (read).
2. Search `\bseed_cited_element_bounds\b` in `omega-rust`: 4 hits (`arithmetic_domains.rs` 3, `program_validation.rs:2369`) (count).
3. Read call-site gate at `program_validation.rs:2357-2376` (non-boundary calls only) and ensures seeding loop at `3170-3181` (read).
4. Input: direct call with matching parameter count, `CheckedBody` machine whose symbol or first state symbol equals the call target, chosen by `Iterator::find` first match (`3185-3201`).
5. **Verdict: REACHABLE** — Two checked machines sharing a callable symbol/state naming pattern could cause the wrong body's `ensures` comparisons to be imported as caller interval facts, accepting calls that do not match the actually invoked callee's contract.

### A0340

1. Read `condition_belongs_to_state` in full at `arithmetic_domains.rs:13444-13554` (read).
2. Search `\bcondition_belongs_to_state\b` in `omega-rust`: hits only in `arithmetic_domains.rs` (recursive and `RequirementSeeds::new` at `13287`, `13304`) (inferred from read).
3. Read `RequirementSeeds::new` filtering at `13282-13306` (read); did not fully analyze the `Call` and `ordered_values::operand` branches beyond cited lines.
4. Input: `requires` conjuncts that are integer/boolean/float/NaN literals (`13527-13530`) or `Name` paths resolving to `builtin_type_member` (`13462-13463`).
5. **Verdict: CLOSED** — Cited returns are limited to literal constants and builtin type members, which do not incorrectly attribute state-local places; machine-level requires still require `machine_requires_survives` at `13288-13295` for non-entry states.

### What I did not read

- Full `ArrivalWalk` implementation and every consumer of `incoming_guard_environment` beyond `arithmetic_domains.rs:14690-14708`.
- Complete obligation discharge path for `IntegerRangeObligation::discharge` at target selection for ordinary (non-constant-invocation) programs.
- All `condition_belongs_to_state` arms after the cited literal/builtin lines (notably `ExpressionNode::Call` at `13531-13550`).
- Entire `seed_conjunct` comparison lowering after `arithmetic_domains.rs:3506`.

SLICE COMPLETE

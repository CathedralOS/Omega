# How far the two trace runs' verdicts can be trusted

Reviewed 2026-11-10 against the read-only checkout `.agents/worktree/worker-23` at `80ce6c9c461`. Nothing was built or run. "Read" means I opened those lines. "Inferred" means I reasoned from code I read without following it to the end.

**Bottom line.** In this sample, REACHABLE almost never meant "a wrong program gets through the checker". Of the 14 entries both runs called REACHABLE, 1 is a confirmed wrongly accepted program (A0441, already pinned by #2283). Two more may be real and need a compiler run (A0277, A0586). The other 11 are not real: in 10 the branch can be reached but the program is either correctly accepted or refused by a later check, and 1 is closed by another validator. Both runs' CLOSED verdicts were right every time (16 of 16 calls across the two runs). Wherever the two runs disagreed, the run that said CLOSED was right (8 of 8).

## 1. Sample

The entries were chosen with a seeded shuffle (`shuf --random-source` seeded by `20261110`) inside each stratum, so I did not pick the interesting ones. The joined verdict table and worklists are in `grok_review/join.tsv`, `wl_w.tsv` and `wl_m.tsv`. For all 30 ids, each run's worklist has the same entry text and citation once the group-label formatting is ignored (`[X] -` versus `GROUP: X: |`). The wave worklist sometimes lists extra secondary line citations, for example A0004.

| Stratum | Size in population | Ids |
|---|---|---|
| Both REACHABLE, area 08 / 09 / 10 / 11 / 13 / 14 | 7 / 10 / 10 / 3 / 20 / 112 | A0036 A0039 / A0153 A0173 / A0225 A0232 / A0277 A0279 / A0374 A0391 / A0586 A0469 A0494 A0441 |
| Wave CLOSED, mega REACHABLE | 203 | A0129 A0206 A0635 A0294 A0239 |
| Wave REACHABLE, mega CLOSED | 11 | A0316 A0753 A0793 |
| Both CLOSED | 121 | A0900 A0319 A0642 A0274 |
| Both UNDECIDED | 38 | A0298 A0346 A0406 A0345 |

How to read the run grades below: **V** = verdict right (Y/N/–; – means UNDECIDED and still open). **C** = citations match the code (all line numbers I opened matched this checkout). **P** = the run's "smallest program": `wrong` = would be wrongly accepted, `valid` = accepted, but correctly, `refused` = a check rejects it, `none` = no concrete program given.

## 2. Per-entry verdicts

### Both REACHABLE (14)

| Id | My verdict | Evidence | Wave V/C/P | Mega V/C/P |
|---|---|---|---|---|
| A0036 | BRANCH-ONLY (medium confidence) | The last-chance retry at `checks_borrows_resources.rs:4000-4013` (read) calls `collect_binding_observation` (`:4819-4885`, read). That helper requires an empty projection, an `Observation` completion, a bare local root, a loan *owned by that binding* that is live at the statement, and a compatible reborrow effect (`:4855-4865`). The comment at `:4814-4818` (read) states the rule: "Reading a reference binding uses the loan that binding owns." The permission it grants really exists. Neither run named a program in which the cited loan is the wrong one. | N/Y/none | N/Y/none |
| A0039 | BRANCH-ONLY | Before the cited early accept (`checks_content.rs:2307-2317`, read), `:2270-2277` (read) rejects unless the selected result contract *declares* this exact path and domain. Accepting a boundary trait's declared `ensures` at the call site is the ordinary modular rule. Both runs skipped the declared-contract guard. | N/Y/valid | N/Y/valid |
| A0153 | BRANCH-ONLY (not an acceptance site) | An `Unknown` receiver provenance (`published_facts.rs:412-455`, read) only drops capability-flow rows. Those rows are consumed by package review evidence and the inspect report (`review-evidence/src/capture/callables/policy.rs:108-110`, `package-manager/.../report/policy/callables.rs:92-95`, read by grep). No check accepts or rejects a program because of them. This is at most a gap in review evidence. Mega itself noted that no stage-05 check uses them. | N/Y/valid | N/Y/valid |
| A0173 | BRANCH-ONLY (correct by meaning) | `values_scalar.rs:6880-6882` and `:7552` (read): an empty conjunction is `true`. The membership projection fails closed on any fact kind it cannot express (`domains_and_predicates.rs:14526`, `ProofFact::Proposition(_) => return None`, read), so an empty list means the domain has no facts. `exact_predicate_intervals` returns rows only when `rows.len() == arguments.len()` and unfolding decides `true` (`domains_and_predicates.rs` `exact_predicate_intervals`, read), so empty rows mean a zero-argument predicate that is already proved true. | N/Y/valid | N/Y/valid |
| A0225 | BRANCH-ONLY (language rule) | `06/proofs.rs:2613-2645` (read): policy division needs only a nonzero divisor, while exact division also excludes `-1` (`:2573`). The spec says the same: `wiki/spec/language/numeric_values.md:472-475` (read): "Wrapping returns quotient `MIN`, Saturating returns `MAX`". | N/Y/valid | N/Y/valid |
| A0232 | BRANCH-ONLY (not an acceptance site) | `06/proofs.rs:7479-7487` (read) is stage-06 lowering of an already checked program. On a mismatch it falls back to the checker's own `fallback` carrier as a transitional input (`:7176-7186`, read). That carrier is less informative, not more permissive (inferred). The mismatch is between compiler tables (`filter_map` drops non-primitive parameters, `:7465-7472`), not something the source program decides. | N/Y/none | N/Y/none |
| A0277 | UNDECIDED: likely real, top candidate | `proof/src/checker.rs:5446-5473` (read) looks up `self.<field>` in the *first machine of the whole program* whose attached data has that field name. `operand_is_unsigned` discards the obligation's machine (`:5432`, `let _ = obligation;`, read). An unsigned match lifts the minimum to 0 (`:5220-5224`, `:5334-5337`, read). This range decides acceptance when no certificate can be built (`:373-397`, read). A certificate is built only for literals or for a declared range that already fits (`:1720-1808`, read). Later checks I looked for: `occurrence_integer_expression_fits_range` runs only when the interval does *not* fit (`:381-390`, read). `stage_trust.md:22-25` says the interval fallback is final. I did not read `collect_parameter_bound_comparisons`, so I have not confirmed that the witness reaches `self.v`. | Y?/Y/wrong (plausible) | Y?/Y/wrong (plausible) |
| A0279 | UNDECIDED: low | `proof/src/lib.rs:817-846` (read) does not look inside array literals, struct literals or atomics. `dehoisted_operand` only uses Indexed, Member or Name initializers (`:667-685`). `dehoisted_condition` uses any initializer (`:687-693`). Harm needs two locals whose identical initializers contain a call, with a fact proved about one being reused for the other (inferred, not traced). The wave's program (an array literal initializer) is not shown to be wrongly accepted. | –/Y/none | –/Y/none |
| A0374 | BRANCH-ONLY | `bind_formal_type` (`validation/calls.rs:38684-38788`, read) only infers type bindings for write-frame planning. A mismatched argument is rejected by argument validation: `calls.rs:40228-40256` (read), "argument `x` for state `S` expects ..., got ...". I inferred that transition calls take that path too. | N/Y/refused | N/Y/refused |
| A0391 | BRANCH-ONLY as stated | `contract_entailment.rs:21167-21290` (read). A lent place that is not a record field has no restricted-field handback to check, so the runs' programs (a "local or non-field place") are correctly accepted. Residual, noticed while reading: transition calls are skipped outright (`:21217`, `CallSite::TransitionNamed { .. } => continue`), and a reference local whose origin is unresolved also lands on the non-field path (`:21266-21285`). Both are worth a run (ranked list, items 2 and 6). | N/Y/valid | N/Y/valid |
| A0586 | UNDECIDED: possible | `value_custody_core.rs:9551` (read): when the 4096-step budget runs out, the search returns `None`, and `None` is read as "no runtime extent" (`:9762-9767`). The visited key includes the generic bindings (`:9552-9554`), so nested generic instances (`:9596-9620`) are visited again on each path. The visit count can roughly double with each nesting level (inferred). Each root parameter gets a fresh budget (`:9761`), so the exhausting type and the runtime-extent array must sit in one data type. Later checks I looked for: none in the checker. Back-end layout uses `closed_fixed_array_length` (`provider-planning/.../builder.rs:469`, grep only) and would probably fail later, but `--check` would pass. | –/Y/none | –/Y/none |
| A0469 | BRANCH-ONLY | `float_and_mathematical_contracts.rs:2633` (read): `_ => true` admits a `Name`. Every name a logical body can refer to is already checked elsewhere. Parameters must be immutable, non-const logical carriers (`:2425-2427`, read). Locals must be immutable and logical (`:2459-2472`, read). Assignments and assembly are refused (`:2433`). An unbound name, mega's program, is rejected at name resolution in stage 02 (inferred). | N/Y/refused | N/Y/refused |
| A0494 | CLOSED | Every input that leaves `matching` empty at `machine_calls_core.rs:7083-7105` (read) is rejected by `traits.rs`. A conformance with no requirement is "malformed conformance ... without an exact requirement" (`traits.rs:840-846`, read). A requirement name that no requirement matches is "has no requirement named" (`:1462-1472`, read). A missing trait is rejected at `traits.rs:758-773` (cited by the wave; not read by me). The wave cited `traits.rs` itself and still said REACHABLE. | N/Y/refused | N/Y/refused |
| A0441 | CONFIRMED-REACHABLE (by a compiler run, not by me) | `domains_and_predicates.rs:13606-13611`, `:13689-13698` (read): any crash statement turns off the end-of-state window check. Pinned by `09968b4717a` (#2283): `omega --check` exits 0 on the Mac mini at `c118c914794`, and the twin without the crash is rejected. The runs' variant (a guarded crash next to a fall-through return) has not been run. | Y/Y/wrong | Y/Y/wrong |

### Wave CLOSED, mega REACHABLE (5)

| Id | My verdict | Evidence | Wave | Mega |
|---|---|---|---|---|
| A0129 | BRANCH-ONLY | `flow_mutation.rs:1484-1505` (read): the predicate exists to spot *locals declared earlier in the state* that need their write origin. A failed lookup means the root is a parameter or field, which is correctly recorded at the authored place. | Y/Y/– | N/Y/valid |
| A0206 | BRANCH-ONLY | `06/expression_bindings.rs:1728-1749` (read): establishing the same storage `(source, path)` again replaces the earlier case knowledge, which is what storage holding one case means. This is lowering, not acceptance. | Y/Y/– | N/Y/valid |
| A0635 | CLOSED (no source input) | `03/selection_structural.rs:1009-1078` (read) checks back-end IR that the compiler built. No source program chooses the `OwnedResult` root. Geometry and projection are re-checked at `:1063-1078`. | Y/Y/– | N/Y/none |
| A0294 | CLOSED | Refusals stored as data at `terminal-production/src/lib.rs:173-177` are raised at `provider-planning/.../plan_computation.rs:115-119` (read), reached from `execution_settlement.rs:401` (grep). | Y/Y/– | N/Y/refused (mega read the later refusal itself) |
| A0239 | CLOSED | Omission is demand pruning (`06/retention.rs:2602-2624`, read). A provider that is needed but missing is refused at `provider-admission/.../retained.rs:385-391` (read), "has no retained external implementation". | Y/Y/– | N/Y/refused |

### Wave REACHABLE, mega CLOSED (3)

| Id | My verdict | Evidence | Wave | Mega |
|---|---|---|---|---|
| A0316 | CLOSED | Neither `unwrap_or(0)` helper (`checked-trees/src/flow_terminal_units.rs:968-974`, `:999-1006`, read) has a production caller (grep of `.statement_ordinal()` across `omega-rust`). The caller the wave cited, `execution_terminal_unit_state_graph.rs:4838`, takes a different `ReturnArm` type. | N/Y/none | Y/Y/– |
| A0753 | CLOSED | `AllocatorAvailabilityPlan::decode` is called only from tests (mega's grep; wave agrees). Production computes the plan. REACHABLE here meant "a tampered record would decode". | N/Y/none | Y/Y/– |
| A0793 | CLOSED | The commitment comes from the plan index in the same pass (`00/settlement.rs:154`, per both runs). There is no source-controlled path. Same tampered-record reading as A0753. | N/Y/none | Y/Y/– |

### Both CLOSED (4)

A0900 (`post_handoff_writer.rs:237-248`, read: `claim` refuses unpinned or published receipts), A0319 (`published_facts.rs:11027`, read: pending operator domains are resolved before checks), A0642 (`literal_fold_rewrites.rs:1221-1225`, `:1308-1311`, read: only unsigned literals, and `WrappingRemainderI64` is selected only for signed remainder), A0274 (strict proof irrelevance, documented at `mathematical_typing.rs:7-8`). All 4: CLOSED. Both runs Y/Y.

### Both UNDECIDED (4)

| Id | My verdict | Evidence |
|---|---|---|
| A0298 | BRANCH-ONLY | `checked-trees/facts/src/predicates.rs:100-148` (read) keeps the full `len == bytes.len()` term and stops comparing bytes at the capacity. When capacity < literal length the length term cannot hold, so the shortened predicate is equivalent to the full one. Shortening avoids reads past the end (inferred from len ≤ capacity). |
| A0346 | CLOSED | Calls with members missing are pruning steps. Final acceptance requires a witness for every member: `checks_termination.rs:279`, `Ok(natural) if bodies.len() == component.len()` (read). |
| A0406 | UNDECIDED | Atoms are keyed on `{term:?}` (`contract_entailment.rs:58147`, `:58149`). This is decided by reading `impl Debug for StructuralTerm` (around `:79981`) for fields that two distinct terms can share. A compiler run is not the first step. |
| A0345 | UNDECIDED | `call_cycles.rs:2764-2765` (read) skips assembly statements during the rank-preservation check, and write frames treat assembly as writing nothing (`calls.rs:2246`, `:5887`, grep). `assembly.md:105-116` (read) lets assembly bodies update places and memory, and its loop backedges owe a ranked decrease. Decided by a compiler run (ranked list, item 3). |

Both runs' UNDECIDED: not wrong, but 2 of 4 can be settled by reading about 50 more lines.

## 3. Accuracy by verdict class (this sample)

| Run | REACHABLE calls | Real (confirmed) | Real or plausible | CLOSED calls right | UNDECIDED |
|---|---|---|---|---|---|
| Wave (composer-2.5; 16 slices fast-only, per `TALLY.md`) | 17 (14 both + 3 R/C) | 1 (6%) | 3 (18%) | 9 of 9 | 4, of which 2 can be settled |
| Mega | 19 (14 both + 5 C/R) | 1 (5%) | 3 (16%) | 7 of 7 | 4, of which 2 can be settled |

Citations: every line number I opened (about 26 entries) matched this checkout, which suggests the code at the cited lines has not moved since `457adf4a11f`. The errors are in the reasoning, not in the line numbers.

## 4. What REACHABLE meant in practice

For both runs, REACHABLE meant "this accept, skip or default branch runs for some input, and I found no refusal in the same function". It did not mean that a wrong program gets through the checker. The recurring misses, each seen at least twice:

1. **A guard earlier in the same function was missed.** Example: A0039, where the declared-contract check at `checks_content.rs:2270-2277` runs before the cited early accept.
2. **The language rule makes the acceptance correct.** Examples: A0225 (the spec text on policy division), A0173 (an empty conjunction is true), A0298.
3. **A validator in another file refuses the program.** Examples: A0494 (`traits.rs`), A0374 (call-argument type check), A0469 (name resolution).
4. **The site is not where acceptance is decided.** These include published facts read only by package review (A0153), stage-06 lowering metadata (A0206, A0232), compiler-built back-end IR (A0635, A0753, A0793), and helpers with no production caller (A0316).

The wave run also used REACHABLE for "a hand-edited serialized record would pass" (A0753, A0793). That is an integrity concern, not a wrongly accepted program. The mega run treated REACHABLE as its default: 731 of 1115 entries, and twice even after its own trace had read the later refusal (A0294) or found that nothing consumes the result (A0153). Neither run produced a concrete source program except where the entry's own text already described one.

## 5. Estimate for the 252 both-REACHABLE entries

The sample was stratified, so I weight by stratum:

| Group | Count | Sample | Real-rate guess |
|---|---|---|---|
| Areas 01-06 (back end, inputs built by the compiler) | 90 | none in this stratum. The three back-end entries in the disagreement strata were all CLOSED for this reason | 0-5%, so 0-5 entries. These are trust-boundary gaps of the kind `stage_trust.md` targets, but they are not wrongly accepted source programs |
| Areas 08-11 | 30 | 8: none confirmed, A0277 likely, A0279 low | 3-15%, so 1-5 |
| Area 13 | 20 | 2: none, one residual lead | 0-20%, so 0-4 |
| Area 14 (validation tail) | 112 | 4: A0441 confirmed, A0586 possible | 10-50%, so 11-56 |

**Estimate: about 25 real entries of 252 (about 10%), with a plausible range of 10 to 60.** The width comes mostly from having only 4 samples in area 14, which holds 112 entries. The 203 entries where mega said REACHABLE and wave said CLOSED were 0 for 5 and should be trusted even less.

## 6. Candidates worth a compiler run, best first

Each item names the program to try and the twin it should be compared against.

1. **A0277: field signedness taken from the wrong machine.** Declare machine `first` with attached data `{ v: u32 }`, then machine `second` with attached data `{ v: i32, w: <range 1..=11> }`. In `second`, call a boundary trait machine whose `ensures` bounds its `&mut` argument (`out <= 10`) on `&mut self.v`, then assign `self.w = self.v + 1`. This must be rejected (`v` can be negative). The bug lifts `v`'s minimum to 0. Twin: declare `second` first; it should then be rejected.
2. **Handback skipped for transition calls (noticed in A0391, `contract_entailment.rs:21217`).** Use the confirmed fixture's `data Tally requires count <= len`. Transition to a state, or target, whose `&mut u32` parameter receives `&mut target.count` and writes 99 before returning. It must be rejected. Twin: the same write through an ordinary call.
3. **A0345: assembly inside a ranked recursive state.** A recursive machine ranked on `n` whose state runs an inline assembly block that rewrites `n` upward before the recursive call with `n - 1`. It must be rejected because it does not terminate. Twin: the same increment written in ordinary code.
4. **A0586: the inline-extent search runs out of budget.** One data type that holds a generic pair nested about 13 levels deep, followed by a fixed array whose length is a runtime value. It should report "runtime value ... for an inline fixed-array extent". Twin: remove the nesting.
5. **A0441, the runs' variant.** A guarded `crash` that *can* run, next to a fall-through return that leaves a data `requires` broken. This differs from the pinned dead-crash fixture and confirms that the cluster is wider than one shape.
6. **A0391, a reference local.** `let r = &mut target.count;` then pass `r` to a callee that writes 99, with the origin made hard to resolve (for example, `r` chosen in a `match`). It must be rejected.
7. **A0279: a call hidden inside an array literal.** `let a = [next(), 0][0]; let b = [next(), 0][0];`, then guard on `a > 5` and store `b` into a target whose range is 6 and up. It must be rejected.
8. **A0036: the binding-observation retry.** A reference local holding a live loan, passed where a more specific supplier rule fails, for example a reborrow of a projection. Low chance.
9. **A0406: debug-text atoms.** Read the `Debug` impl first. Run the compiler only if two distinct terms can print the same text.

Only nine entries merited a place. The tenth candidate I would have added is not supported by anything I read.

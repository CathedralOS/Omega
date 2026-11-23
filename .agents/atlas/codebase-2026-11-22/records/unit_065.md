# Record: unit_065 (area 04-source-stages-06-07)

All paths are under `source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/` (called `P06/src/` below) unless written in full. Every file in the unit was read in full. Line numbers are from the Read tool at the checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Counts come from Grep count/content runs, named where used.

## 1. places/loans.rs (3136 lines)

**Purpose.** Recomputes, from the lowered form, every `Loan` obligation row a machine owes (one per borrowed call argument, one per narrower region a returned reference keeps, one per persistent reference/element-view formation, one per successor edge borrow). It refuses a row set that differs from the recomputed one (`ForeignIdentity`), then judges each loan: exclusive overlap, access beyond the parent's access, and later use of lent storage while a retained loan is live. It also checks returned-reference origins and the disposal of loans at returns.

**Shape.**
- `ConflictingLoan` (`loans.rs:82`): loan row, optional invocation, argument position, `with`, `conflict`, `at`. It is the result type of `Storage::loans`.
- `Formed` (`92`): one recomputed loan: obligation, invocation, position, `access`, `lineage: LoanRegion`, `parent` access, `lender`, `selected_parents`, `from` descriptor, `permitted`, `at`, `formed_in`, `lifetime`, `returned_by`.
- `LoanRegion` (`161`): `Storage(Backing)` or `Provider`.
- `FormedLoans`, `BorrowedHeaderEnd`, `CleanupCohort`, `Touch`, `Ancestry`.
- `Storage::loans` (`2898`) is the entry. It calls `reference_results` (2670), `formed` (2385), `parents` (1991), `successor_borrows` (1857), `reference_completions` (2757), `retain_ancestors` (2203), `cleanup_cohorts` (1543), then `uses_retained_loan` (1315) per loan.
- Loan formation: `lent` (788), `reference_loan` (1046), loop in `formed` for successor edges (2500-2610).
- Free functions: `competing_borrows` (205), `holds` (3127), `reference_graph_with_nominal_calls` (245, `pub(crate)`), `reference_graph` (238, `#[cfg(test)]`).

**Inputs and outputs.** Reads `MachineTables` (obligations, invocations, edges, places, values), the lowered `TerminalMachine`, the `DescriptorAnalysis` (`references`) from `terminal_semantics`, `Backing`/`Lineage` from `places`. Mutates the `claimed` bitmap and the `Formed` vec. Returns `(Vec<(ObligationId, Checked<()>)>, Vec<ConflictingLoan>)`. The module comment says there is no certificate for loans: "a resource judgment, not a proposition" (`loans.rs:41-47`).

**How it decides.**
- Overlap: `competing_borrows` (205-232) keys on the two `Backing` regions via `places::overlap` (grepped: defined `places.rs:201`) and on access exclusivity. A `Provider` region with an exclusive access is `AccessExceedsParent` (215-219). Provider vs anything else non-storage returns `None` (220-224).
- Row matching: a recomputed loan is matched to a table row by `captured == place && access == borrowed && created_at == before && !claimed` (`879-882`, `971-974`, `1210-1214`, `2549-2552`). That is identity (place id, access, program point), not spelling.
- Lender match: `expected_lender.is_some_and(|e| e != lender)` refuses (893, 985, 1225, 2556). `source_lender` (622) derives the expected lender from the root's kind and incoming access.
- Retained-loan uses: `uses_retained_loan` walks every operation and terminator of the machine, takes `operation_read_effects`/`operation_changes`, and asks `retained_conflict` (1498). A shared loan is conflicted by any ordinary change (`1537`). Otherwise the change or read passes only if `authorized_reference` (1726) finds a loan (or descendant of it) whose lifetime authorizes the descriptor at that position.
- Ancestry: `ancestry` (2139) walks `Lender::Loan` links and `selected_parents`, with a cache of `Ancestry::Visiting/Checked`. `any` picks MAY vs MUST. Disjoint parents (by `overlap`) are skipped when `!any` (2179-2185).
- Final pair check (3010-3059): for each loan, in order: retained-use conflict, `ExclusiveOfShared` if exclusive and `parent == Some(Shared)` (3015), `AccessExceedsParent` if `!permitted` (3018), then pairwise against earlier loans that share an invocation or whose lifetimes hold at each other's position, skipping descent pairs and `returned_by` pairs.

**Cases by shape.**
- Provider loans: only "provider referent" when the descriptor's source origins are all `DescriptorSourceOrigin::Provider` (529-540).
- `lent` refuses "a write-only loan" (853-855) and "a loan to something not a call" (809-811).
- `reference_loan` handles only `EstablishReference`, `EstablishElementView`, `EstablishMutableElementView` (1067-1087); any other operation kind returns `Ok(())`. An exclusive representation view is refused: "exclusive representation view without establishment and handback evidence" (1115-1118).
- `reference_view_offset_is_zero` required for representation views (1157-1166).
- `released_reference` (1269): refuses release through borrowed storage or without an owned descriptor home (1277-1306).
- `capture_payload` refuses "multiple possible backing regions" (479-486).

**Same job elsewhere.**
- Cleanup-action write/read conflict logic is repeated three times in this file: `uses_retained_loan` (1431-1489), `successor_borrows` (1950-1985). Both call `cleanup_backing` and `cleanup_reference_accesses` and `competing_borrows` (read).
- Loan-parent discovery calls `terminal_semantics::borrowed_loan_sources` (3 uses here: 1807, 1903, 2010). Suspected: the same descriptor-lifetime query is used by psi-to-native reference custody (`abstract-operations` lists `descriptor_*` users in the grep of `Carried|...` which I did not read, so "suspected").
- `retained_in_caller_storage` (2709) and `reference_completions` (2757) overlap with `terminal_semantics::validate_reference_result_completion` (called at 2889); the checker here only builds the occurrence list.

**Unused or one-caller code.**
- `reference_graph` (238): `#[cfg(test)]`; grep `reference_graph\(` in `P06/src`: 1 definition + 8 calls, all in `places.rs` tests (8397-9371) and `tests/collection_members.rs:785`.
- `reference_graph_with_nominal_calls` (245): 4 uses (`lib.rs:1474`, `owned_cleanup.rs:558`, `places.rs:112` re-export, `loans.rs:242`).
- `Storage::loans`: 1 production call (`lib.rs:758`), 3 test calls (`tests/successor_loans.rs:411, 681, 1285`).
- `ConflictingLoan`: grep over `source-rust` count mode: 11 occurrences in 5 files (lib.rs 2, places.rs 1, loans.rs 4, verdict.rs 3, `representations/lowered-program/tables/mod.rs` 1).
- `LoanConflict` is re-exported from `terminal_check_tables` (`loans.rs:76`).

**Stated limits.** "a write-only loan" (855) and "a write-only persistent loan" (1096). "needs its exact restoration and child closure" (1285). "a reference retained in caller storage needs its checked output-loan correspondence" (2805). "retained reference needs exact arrival-loan correspondence across its backings" (2744). Doc comment: "Missing immediate-parent or returned-call correspondence remains unavailable" (loans.rs:7-9).

**Fail-open risks.**
- `reference_loan` `_ => return Ok(())` (1086): an operation that is not a reference formation forms no loan. Traced: `formed` calls it for every operation (2442); a missing `Loan` row for a real formation is caught at 2612-2620 ("a loan row that no call or reference formation owns" covers the other direction only, rows without a formation). A formation kind missing from the match with a table row for it would be refused by that final loop. A formation kind missing from the match with no table row would form no loan. Classification: open (not traced to the lowering side).
- `lent` early return `Ok(())` when every argument is `Owned` (800-808): with `retained` non-empty it refuses. Traced.
- `borrow_header_ends` returns an empty `selected` list when verification is unavailable (2338-2347, "A missing schedule can confer no implicit retirement"). Failing to find ends retires fewer loans, not more. Stated.
- `authorized_reference` `return Ok(false)` at 1771 (no descriptor identity) means "not authorized", so a conflict is reported. Fails closed.
- `retained_conflict` returns `Ok(None)` when `competing_borrows` is `None` (1529). `competing_borrows` returns `None` for a non-`Storage`/`Storage` pairing other than the provider cases (220-224); the other pairings are `Provider` with a non-exclusive access. Stated by the comment at 211-214 ("No address or disjointness fact follows from this case").
- `reference_completions` `continue` when `returned.is_empty()` (2843-2848): "ordinary loan and affine cleanup own the local remainder". Stated.
- `uses_retained_loan` `let Some(lifetime) ... else return Ok(None)` (1322-1324): a duration-only loan has no retained uses. Stated (doc at 23-25).

## 2. places/log.rs (758 lines)

**Purpose.** Derives, from operations alone, what each operation stores into which place, and checks that elaboration's write log, place table and snapshot rows say the same. A difference is a `ForeignIdentity` refusal.

**Shape.** `Stored` (59): `Vacated`, `Value`, `Moved(Lineage)`, `Case`, `Zeroed`, `Established`, `UnknownPayloadWrite`. `Logged` (100). `places_are_interned` (130). `Storage::stores` (217), `stored_by` (241), `index_reaching_writes` (656), `reaching_writes` (674), `log_is_stated` (704).

**Inputs and outputs.** Reads the lowered operations, `MachineTables` (`writes`, `places`, `roots`), `value_types`. Produces `Vec<Logged>` and fills `writes_by_root`. Refuses on mismatch.

**How it decides.** `stored_by` matches on `OperationKind` (280-649). `log_is_stated` compares, per operation id, rows in order: `row.0 != store.destination || row.1 != store.stored.content()` (741). Identity (lineage, `Content`) keyed, not names.

**Cases by shape.** The match lists operation kinds explicitly. Quotes: "An operation that is none of these, reads no storage and has no scalar semantics is refused as `Unsupported`" (28-30); final arm `_ =>` refuses unless `produced.is_none()` and `result_equation(...)` is `Some` (634-648). Atomic: `CompareExchangeOnce` is refused "single-attempt atomic outcome custody" (324); non-load/store atomic requires an integer resident (326-335). Mutable sequence construction refused (371). A `StructuralByteViewFieldStore` with `field: None` is not derived (494-497 comment: "The whole-home form ... stays refused below", it falls to the `_` arm).

**Same job elsewhere.** `Storage::cleanup_log_is_stated` (`places/cleanup.rs:190`, not in unit) does the same check for cleanup writes (grepped; not read: suspected).

**Unused or one-caller code.** `places_are_interned`: one production call, `places.rs:1083`. `log_is_stated`: one call, `places.rs:1181`. `index_reaching_writes`, `reaching_writes` are `pub(super)` (not grepped; used inside `places`).

**Stated limits.** "mutable sequence construction" (371), "atomic resident requires its checked establishment obligation" (297).

**Fail-open risks.** `OperationKind::DecomposeStructuralValue {} => {}` (543) and `ByteSequenceWrite` (536-540) emit no store row. Stated: "Shared affine replay authenticates the complete component roster" and "Payload writes are tracked by descriptor backing analysis" (537-543). Traced no further. `AtomicFence`, observers: stated (549-564). The call arm: `call_write_regions` returning `None` falls back to treating every mutable/write-only borrow as written (592-599). Fails closed.

## 3. prove.rs (5318 lines) and prove/ submodules

**Purpose of the family.** The one prover. It builds a `ProofNode` certificate for a goal from hypotheses; it never decides (`prove.rs:2-5`: "every node it emits is re-checked by `verify_obligation`"). The kernel is in `proof_admission`; the prover only asks it for images (`map_integer_affine_bound`, `check_value_equality_denotation`, `decide_primitive`).

### prove.rs (core)

**Shape.**
- `Prover<'a>` (795): `facts: Vec<Fact>`, `exact_facts`, `implications`, `equations`/`definitions`, `arrival_equations`, `bounds: IdTable<ValueId,[Slot;2]>`, `bound_facts`, `case_split`, `remaining_case_branches`, `active_cuts`, `active_goals`, `refuting`.
- `Prover::new` (1085), `prepare` (1105), `prove` (1444), `prove_goal` (1461), `prove_direct` (1918), `prove_structural` (1935).
- Bounds: `bound` (3485), `bound_literal` (3504), `search` (3527), `bound_from_fact` (3724), `literal_bound_from_fact` (3646), `compound_bound_from_fact` (3910), `definition` (4421), `product` (4502), `shifted` (4535), `transported` (4602), `image` (4648).
- Case split: `split` (1738), `branch` (1795), `prove_under` (1811). Contradiction: `unreachable` (1511), `contradiction` (1524), `refute` (1837), `refute_guard` (1672), `refute_closed_relation` (1602).
- Order: `relation` (2812), `steps` (2846), `compare` (2500), `successor` (2577), `sum_order` (2220), `strict_sum_bound` (2158), `strict_sum_order` (2292), `saturating_subtract_order` (2685).
- Classification: `covered` (134), `has_rules_for` (1293), `literal_bounds` (1366).
- Helper structs: `ExactFacts` (497), `ImplicationConclusions` (439), `BoundFacts` (614), `ProofSearchActivity` (695, `pub`).

**Inputs and outputs.** Inputs: `PropositionContext`, assumptions slice, axioms slice, `&dyn ProofValues`, `&mut ProofSearchActivity`. `Fact` and `Known` come from `crate::checking::known`. Output: `Result<ProofNode, &'static str>`. Mutates its bound cache and the activity counters.

**How it decides.** By proposition form, term structure and integer literal values, never by names. Key judgments:
- `covered` (134): form-only classification for "no derivation" vs "no rule for goal form". Cites `compare_integer_math_terms` from the kernel.
- A bound is tried in order: carrier bound, direct facts, then relations/definitions; the best is the one `Direction::prefers` (`integer.compare`) (317-322).
- Splits on a `Disjunction` fact limited by `CASE_BRANCH_BUDGET = 64` (84) and `active_cuts`; `has_scalar_counterexample` (counterexample.rs:8) aborts a split when an assignment satisfies all assumptions and falsifies the goal (1482-1490).
- `INTERVAL_CANDIDATES = 12` (259): at most 12 candidate literals tried in `literal_bounds` (1412).
- Facts are matched by exact `Proposition` equality after a hash pre-filter (`ExactFacts::find` 531-571).

**Cases by shape.** `covered` lists forms (134-218). `exact_fact_hash_eligible` (416-433): only `Truth`, `Falsehood`, `Equal/LessThan/LessOrEqual` over leaf terms (`Value`, `Integer`, `Boolean`, `IeeeFloatLiteral`) are hashed; "Other forms keep exact equality in one fallback bucket" (413-415). `prove_structural` refuses `Atom`, `IeeeFloatComparison`, `ByteSequenceEqual`, `StructuralIdentity`, `StructuralCaseMembership`, `ContentConservation` as "goal form outside this prover" (2055-2060). `strict_sum_order` requires a fixed carrier, non-address (2308). Many helpers `return None` for `integer.carrier() != IntegerCarrier::Fixed || integer.is_address()` (3056, 3122, 3175, 3221, 3243, 3740, 3918).

**Same job elsewhere.**
- `ranges.rs:117 landing` and `prove.rs:5099` etc. repeat "earlier axiom fixes term to a literal" logic; `find_axiom_before` (`ranges.rs:1065`) is a second spelling of the same lookup.
- `relation` (2812) plus `compound_order_step`/`below_compound`/`upper_subterm` (3163-3313) and `widths.rs value_bounds` (249) each build order chains over facts for different goal forms.
- `prove.rs:2372` `order_through_cast` and `ranges.rs:690 cast_bound` both cite `IntegerCastBound`/`check_integer_cast_chain_witness`.
- `prove.rs mathematical_value_order` (2454) and `mathematical_bounds.rs` (image functions) both chain upper image of left with lower image of right.
- `ieee_float_order.rs` `refute_ieee_relations` and `prove.rs refute*` are two contradiction searches (one per proposition family).
- `reuse.rs Transport` (289) renames terms in a certificate; `lib.rs substitute_goal` (1803) and `lib.rs ScalarActuals` (2017) rename values in goals; `representations/.../wire/src/contract_forms.rs:108 substitute` (grep hit, not read) is suspected the same job.

**Unused or one-caller code.**
- `has_rules_for` free function (`prove.rs:236`, `#[cfg(test)]`); `pub(crate) use prove::has_rules_for` at `lib.rs:120`. grep `\.has_rules_for\(|prove::has_rules_for`: 1 production call of the method (`lib.rs:2470`), plus a comment at `tests/arithmetic_policies.rs:268`.
- `Prover::beyond_rules`: 1 call (`lib.rs:2470`). `value_beyond_rules`: 2 calls (`entry_inputs.rs:726`, `result_summaries.rs:536`).
- `Prover::literal_bounds` (1366): 3 production calls (`entry_inputs.rs:736`, `joins.rs:621`, `result_summaries.rs:539`).
- `Prover::new`: grep `Prover::new\(` non-`tests.rs` count: 8 matches in 7 files (branch_warnings.rs 1, entry_inputs.rs 1, joins.rs 1, lib.rs 2 (`2464`, `2727`), machine_batch.rs 1, result_summaries.rs 1, tests/guard_contradiction.rs 1). The comment says "The one prover"; production users outside `prove/` itself: 7 call sites in 6 files.
- `ProofSearchActivity` outside `06_lowered-program-to-checked-program/`: grep count 0 matches. It is `pub` and `started()` is `#[cfg(any(test, feature="test-support"))]` (728-731).
- `prove/tests.rs` has 44 `#[test]` (grep count).

**Stated limits.** "Deeper clause expressions are outside this covered subset" (193). "Hash only terms whose existing derived hash cannot recurse" (412). "Running out supplies no premise or partial certificate" (83).

**Fail-open risks.**
- `ex_falso` (1068-1082): builds a proof of any goal from a node proving a false relation. The kernel recomputes the denotation (comment 1062-1067). Stated; certificate is re-checked.
- `prove_structural` returns `ex_falso` of an exact `Falsehood` fact before looking at the goal (1939-1941). A hypothesis list containing `Falsehood` proves everything. Traced: `Falsehood` as an assumption means the program point is unreachable; the test `falsehood_remains_available...` (`prove/tests.rs:118`) pins it.
- Counterexample short-circuit (1482-1490) only returns the earlier failure (`direct`), never accepts.
- The prover returns certificates only; acceptance is by the kernel (`verify`), so a prover bug can only cause a refusal or a kernel `RuleConclusionMismatch`. Classification for all prover returns: stated (file header 2-5).
- `Slot::InProgress` reuse (3511-3517): an in-progress bound may be reused if already holding a completed proof. Stated at 3508-3510.

### prove/counterexample.rs (73 lines)

**Purpose.** `Prover::has_scalar_counterexample` (8): tries an assignment of extremal values (min or max of each free integer, 0/1 for booleans), improved with literal equalities and result equations, and returns true only if the goal evaluates false and every assumption and axiom evaluates true. It "supplies no evidence" (1-2). Keyed on `known_predicate_with`/`known_scalar_value_with` from `terminal_semantics::inhabitation`. Free values come from `values.scalar_type`; only `Boolean` and integers with known `bits()` are guessed (17-27). Callers: 1 (`prove.rs:1487`). Fail-open: none seen (returning true only stops proof search, a refusal path).

### prove/float_integer.rs (84 lines)

**Purpose.** `Equalities::new` (15) indexes `IeeeFloatIntegerExact` preconditions and float-equality subjects once; `Prover::float_integer_conversion` (43) proves exactness or IEEE comparison goals by substituting a fixed source equality, then re-proving the computed goal. Keyed on `ScalarTerm` identity. Calls `float_integer_conversion_substitution` and `float_integer_equality_subject` from `proof_admission`. Uses: `prove.rs:2083, 2091`. Fail-open: none seen (certificate re-checked).

### prove/float_meaning.rs (107 lines)

**Purpose.** `Prover::float_meaning` (20): tries `check_float_meaning_normalization(goal, [])`, then collects connected `FloatMeaningEqual` facts by shared `ScalarTerm` subjects (BFS over `indexed`) and asks the kernel to normalize them. Budget errors (`WorkLimit`, `ValueLimit`) become "exceeds the normalization budget" refusals. Source literals (`IeeeFloatLiteral`) are not subjects (13-17). One caller (`prove.rs:2061`). Fail-open: none seen.

### prove/ieee_float_order.rs (410 lines)

**Purpose.** IEEE order reasoning: builds an order graph per float format from `<`/`<=` facts plus decided literal bridges, and finds a path from `left` to `right` (`ieee_order_from_graph` 370, BFS with `seen` keyed `(endpoint, strict)`). `refute_ieee_relations` (145) refutes using exact complements (`NotEqual`, `NotLessThan`, `NotLessOrEqual` against a provable positive) or a literal self-cycle the kernel calls false. Literal ordering is decided only by the kernel (`ieee_literal_bridge` 85, `ClosedIeeeFloatRelation`). NaN literals are excluded by filtering on `ieee_literal_bridge(term, term)` (323-326). Calls: `ieee_float_order` from `prove.rs:2090`; `refute_ieee_relations` from `prove.rs:1527`. Fail-open: none seen; the sort comparator (327-340) returns `Greater` for incomparable literals, which only shapes the edge list.

### prove/mathematical_bounds.rs (389 lines)

**Purpose.** Bounds on mathematical (erased, unbounded) sums: `mathematical_offset_order` (11), `mathematical_sum_image` (46), `mathematical_relation_image` (86), `mathematical_composed_image` (170), `common_sum_order` (265), `normalize_image_endpoint` (325), free `image_satisfies` (372). Decides on `compare_integer_math_terms` equality of terms (e.g. 24, 123, 276) and `ClosedIntegerEvaluator` for literal normalization (336). Callers: all from `prove.rs`. Fail-open: none seen (certificate nodes).

### prove/ranges.rs (1084 lines)

**Purpose.** One-step endpoint-chain bounds for divide, remainder, shifts, bitwise and/or, policy add/subtract (`chain_step` 132 using `chain_operands` 65), remainder relations (`remainder_below_divisor` 242, `remainder_within_dividend` 285, `remainder_by_fixed_divisor` 808, `multiple_remainder_bound` 919), `positive_difference` (351), `offset_rooms`/`interior_sum_edges`/`remaining_capacity_image` (446-548), `quotient_order` (553), `shift_left_image` (615), `cast_bound` (690), `value_image` (743), and `landing` (117).

**How it decides.** Keyed on `ScalarTerm` variants and on axiom indices: `landing` finds the earlier axiom `Equal(subject, literal)` with index `< before` (117-125). `remainder_*`/`quotient_order` ask the kernel's `check_integer_affine_witness` to accept the witness, then `map_integer_affine_bound` for the image.

**Cases by shape.** `chain_operands` (65-81) is a fixed list of operation shapes: divide, remainder, wrapping/saturating subtract, wrapping/saturating add, bitwise and/or, exact shift left/right; anything else returns an empty list. `multiple_remainder_bound` expands only `Value`, `Integer`, `IntegerWiden`, `IntegerExactCast`, `ExactIntegerAdd/Subtract/Multiply` (972-981). `quotient_order` handles `ExactIntegerDivide` only.

**Same job elsewhere.** `find_axiom_before` (1065) vs `landing` (117): two lookups of "earlier axiom fixing a term". `cast_bound` (690) vs `prove.rs order_through_cast` (2366).

**Stated limits.** Header lists "provisional rule" for `remainder_within_dividend`, `quotient_at_most_dividend`, `IntegerMultipleRemainder` (30-45): "The kernel trusts that statement as a provisional rule" (32-33). Grepping `provisional` was not done.

**Fail-open risks.** none seen in this file; every returned `Bound` has a certificate the kernel re-checks. `multiple_remainder_bound` uses `usize::MAX` as the `upper_axiom` limit when no remainder axiom is given (942), meaning any earlier axiom may be cited; the kernel re-checks axioms by index (traced via `check_integer_affine_witness` use at 1008).

### prove/reach.rs (174 lines)

**Purpose.** Classifies a goal as "beyond the rules" (`Prover::beyond_rules` 50, `value_beyond_rules` 58, `any_beyond_rules` 62) by following the equations that define the values it names to see if any is an operation the integer rules cannot bound: bitwise and/or/xor/not, any wrapping or saturating op (`unbounded_operation` 145-165), or divide/remainder/shift-right whose divisor/count is not a literal or landed (`reaches` 122-139). Also accepts an `unreached` value list. `within_rules` (172) is the term-level test. Used so that an unprovable goal of that form is classified `NoRuleForGoalForm` (Incomplete), not `NoDerivation` (reject) (`lib.rs:2470-2474`).

**Judgment.** By term variant and `landing` result, never by search outcome (header 29-32).

**Fail-open risk.** The classification moves a failed proof from Rejected to Incomplete; Incomplete still refuses the program (`publication.rs:126-138`: "the program is refused, with no claim that it is wrong"). So no wrong program is accepted. Stated (header 20-23, quoting the spec).

### prove/widths.rs (592 lines)

**Purpose.** Range goals at a parameter width `N` (rigid `u64` binder). `mentions_parameter_width` (47), `covered` (94), `Prover::width_goal` (175), `value_bounds` (249), `relational_bound` (365), `carrier_endpoint` (422), `total` (465), `width_order` (501), `power_order` (544). A goal `sum <= endpoint` is proved by bounding each operand and adding, comparing totals with the endpoint, using `IntegerPowerOfTwoOrder` from the width's lower bound. Keyed on `IntegerWidth::Parameter(parameter)` (165).

**Cases by shape.** `operands` (104-118): "one leaf, or two added"; `width_goal` handles one or two operands only (193-200: `_ => Vec::new()`). `power_order` handles exponent `N` or `N - 1` only (545-553). `value_bounds` depth limit 2 for casts (242, `depth > 0` at 298).

**Stated limits.** "Wrapping, saturating and trapping arithmetic at a parameter width are refused before they reach here (`prepared_typed_trees_to_lowered_program`)" (30-32).

**Fail-open.** none seen. `lift` (590) `expect("an integer operand")` and `width_binders` (160-163) `expect(...)` can panic on bad input, not accept.

### prove/tests.rs (2818 lines)

Unit tests of the prover: 44 `#[test]` (grep count). Groups: `exact_fact_index` (5), `bound_fact_index` (201), then top-level tests on refutation work counters, IEEE bounds and complements, wrapping headroom, counterexamples, case-split budgets, remaining capacity, compound orders, dense order work, strict relation bounds. Many assert both acceptance through `proof_admission::accept_certificate` and refusal of a negative twin (for example `1787-1817`, `2096-2109`). Several assert activity counters (`bound_certificate_copies`, `sign_queries`, `compound_order_fact_nodes`) to pin work done, not only results. Helper types: `CollidingHasher`, `NoTermHash` (8-33). No production decision lives here.

## 4. publication.rs (514) and publication/

### publication.rs

**Purpose.** `judge_publication` (19) is the crate's accept entry. It judges the lowered compilation, accepts only if every part is `Accepted`, optionally runs an optimization pass and re-judges, then builds `CheckedProgram` and `CheckedPublication`.

**Shape.** `judge_publication<Input: CompilationForPublication>` (19), `AcceptedCompilation` (78), `accept_compilation` (83), `route_defect` (167). Tests (173-514): 7 test functions plus fixture helpers.

**How it decides.** `accept_compilation` (83-165) matches each `Result<RouteVerdict, CompilationRefusal>`:
- `AcceptedPrefix` is accepted only at position 0 (91-93); at another position it is the diagnostic "a generic form or conformance cannot depend on an unfinished Build" (94-96).
- `Accepted` at position 0 is the root; others are `additional` (97-103).
- `Rejected` collects diagnostics from five lists; `unproved` entries with `cause.is_incomplete()` are skipped in this list (115) because the same compilation's `Incomplete` verdict path reports them; if no diagnostic was added it adds "a rejection names no failure" (122-124).
- `Incomplete(unproved)` and `FallBack` become "not supported yet" errors (126-146).
- `Err(CompilationRefusal)` becomes a "defect of the checker" error (147-153).
- Any diagnostic makes the result `Err` (155-157). No accepted root is also an error (158).
- Judged by verdict variant and position.

**Cases by shape.** "root at position 0, additional at others" (91-103).

**Same job elsewhere.** `verdict.rs` `judge_compilation` (grepped `judge_publication\(` count mode: 5 occurrences in 5 files) is the producer of the verdict list; it was not read.

**Unused or one-caller code.** `judge_publication`: callers `driver/judge_lowered_source.rs` (1), `07_checked-program-to-psi/lib.rs` (1), `07.../src/accepted.rs` (1), `publication/advisories.rs:171` test (1), `tests/optimization.rs` (1).

**Fail-open risks.** `Ok(RouteVerdict::Rejected {..})` with `unproved` all `is_incomplete()` and every other list empty yields "a rejection names no failure" (122-124), refusing. `Rejected` with only incomplete unproved is refused by that fallback. Traced: refuses. None seen otherwise.
`let candidate = (!selections.is_empty()).then(|| program.clone())` (26): with no selections, no re-judge. Stated at 24-25.

### publication/accepted_closure.rs (31 lines)

**Purpose.** `AcceptedClosure` (8): elaboration, discharge, seals, optimization record; four accessors. Used in `publication.rs:55-60` and `advisories.rs`. grep `AcceptedClosure\b` outside `P06/src`: 1 occurrence (`05_prepared-typed-trees-to-lowered-program/src/declare.rs`; not read).

### publication/advisories.rs (211 lines)

**Purpose.** `AcceptedAdvisories` (17): warnings about accepted parts, computed on first `get()` (61) under a `Mutex<AdvisoryParts>` and `OnceLock`. `aggregate` (90) de-duplicates by equal diagnostic runs (a warning plus following notes is one advisory). When an optimization ran, advisories come from the original (pre-transformation) closure (69-71 comment "source warnings must still describe the program as written"). Tests (112-211): 2, reading fixtures under `tests/fixtures/contracts/pass/`. Fail-open: none seen (advisory only).

### publication/optimization.rs (261 lines)

**Purpose.** `apply` (12): runs selected Psi optimization on the unsealed first-judgment input. The only implemented pass is `SparseConditionalConstantPropagation` (28); any other pass is refused "has no implemented pass" (38-43). `propagate_constants` (224) does a worklist dataflow over blocks with a `Fact` lattice (`Unseen`, `Boolean`, `Integer`, `Varying`; `join` 59-66), then rewrites any operation whose result has a known `Fact::Boolean` into `BooleanConstant`. Integer constants are tracked but never written back (253-258).

**How it decides.** By `OperationKind`: only `BooleanConstant`, `IntegerConstant`, `BooleanNot`, `BooleanEqual`, `IntegerEqual`, `IntegerLessThan`, `IntegerLessOrEqual` are evaluated; every other operation is `Varying` (186). Conditional edges are visited only when the condition could be that value (209). Case terminators visit all successors with `None` arguments (214-219).

**Cases by shape.** Same evaluator list above. Mixed signed/unsigned integer facts return `Varying` (178).

**Same job elsewhere.** grep `PsiOptimization::SparseConditionalConstantPropagation` over `source-rust`: 4 hits: `foundation/optimization-selections/src/lib.rs:347`, `psi-to-native/.../abstract_optimization/rules.rs:225`, `publication/optimization.rs:28`, `tests/optimization.rs:40`. The native side hit is a separate stage-03 rule (suspected same optimization name; not read).

**Stated limits.** "Only a pure comparison or negation can infer a new Boolean" (254). `closure.module.machines` only: "Only the executable closure is transformed" (module header 2).

**Fail-open risks.**
- `Op::IntegerEqual` etc. read `Fact::Integer` of the same sign only; ordering for `IntegerLessThan` on a pair of booleans (`Fact::Boolean`) compares `bool`, only reachable if types differ from the carrier (traced: types are fixed by lowering; not traced further): open.
- `expect("checked scalar use")` (112) and `expect("checked block ordinal")` (108) panic if the first judgment's checks are wrong. Stated at 107 ("The first judgment checked the dense ordinal order").
- The rewritten program goes through `judge_compilation` again (`publication.rs:35`), so a wrong rewrite is caught, not accepted unchecked.

## 5. rankings.rs (990 lines)

**Purpose.** Admits loop-termination rankings (`ranked_scc`): checks the declared natural cycles are the actual cyclic components of the control graph, rank tuples are well-typed and in scope, successor ranks correspond to the target's rank under the edge's actual arguments, and the obligation rows are exactly those the lowered form owes. Builds the proposition for each rank comparison. Settles weaker comparisons.

**Shape.** `FiniteIntegerRanking` (23), `Bindings` (93), `Binding` (85), `admit` (127), `owed` (47), `claims_strict` (76), `settle_comparisons` (484), `publish_comparisons` (597), `well_founded` (621), `goal` (653), `Comparison` (647), `Normalize` (733) with `typed`, `available`, `corresponds`, `value`. `RankValue` (719).

**Inputs and outputs.** Reads `TerminalMachine`, `MachineTables`, `Graph` (dominance), `Storage`, `DescriptorAnalysis`. Output: `Bindings` of `(ObligationId, Binding)` rows sorted for binary search, or a refusal.

**How it decides.**
- `admit` (127): termination `Terminates` with no ranks over a real cycle is refused "contract termination promise over an unranked cycle" (147-160). Ranked component list must cover `control_cycle_members` (161-166). Rank edges must equal every internal edge in canonical order (216-227). Carriers must be fixed finite integers: `is_address() || bits().is_none_or(== 0)` refused (239-249). Strict-edge removal must leave an acyclic graph (`acyclic` 443; 335-342).
- `Normalize::corresponds` (776): a successor rank equals the target rank by block-parameter position, machine parameter identity, or an `Operation`-origin value compared as `RankValue` after substitution (815-828).
- `Normalize::value` (839): normalizes to `Scalar`, `Integer`, `Difference`, or `Length(place, path)`. Budget `table.values.len() + 1` (819).
- `goal` (653): lexicographic proposition: `Disjunction(after<before, Conjunction(after==before, rest))` (708-714); last coordinate strict or preserving.
- `settle_comparisons` (484): for strict claims that failed with `NoDerivation|NoRuleForGoalForm`, probes a weaker `Preserving` proof, and relabels edges `Preserving` if the graph stays acyclic over preserving edges (549-569). If not acyclic, it checks `Comparison::Equal` and replaces the refusal with "the caller and successor measures are provably equal here" (574-591).

**Cases by shape.** `Normalize::value` handles only `IntegerConstant`, `ExactIntegerSubtract`/`SaturatingIntegerSubtract`, `ByteSequenceLength`/`ElementViewLength`; with substitution any other producer is refused: "ranking observation outside constants, exact subtraction and descriptor length" (975-981). Without substitution (`substitution.is_none()`), any other producer is `RankValue::Scalar(value)` (975). `edge_arguments` (390) covers `Jump`, `Conditional`, `StructuralCase`; case edges have no scalar arguments (419).

**Same job elsewhere.** `components_rankings.rs` in stage 05 (`05_prepared-typed-trees-to-lowered-program/src/components_rankings.rs:155 fn substitute`, grep hit, not read) produces the rankings this file admits (suspected related). `terminal_semantics::control_graph::control_cycle_members` is the shared graph query.

**Unused or one-caller code.** grep `settle_comparisons|publish_comparisons|rankings::well_founded|claims_strict|rankings::owed|rankings::admit|rankings::goal` on `source-rust`: `claims_strict` 1 (`describe.rs:185`); `publish_comparisons` 1 (`formation.rs:248`); `well_founded` 1 (`lib.rs:837`); `settle_comparisons` 1 (`lib.rs:917`); `admit` 1 (`lib.rs:1491`); `goal` 1 (`lib.rs:2279`); `owed` 1 (`minted.rs:1066`).

**Stated limits.** "ranking requires closed finite integer carriers, not addresses or parameter widths" (247). "ranking requires a complete valid reachable control graph" (140).

**Fail-open risks.**
- `settle_comparisons` early `return Ok(())` if no strict comparison failed with a missing-derivation cause (500-505): leaves earlier verdicts as they were. Fails closed.
- `settle_comparisons` mutates `checked.bindings.edges[*index].comparison = Preserving` and marks the strict obligation discharged with a `Preserving` certificate (562-568) only when the preserving-edges graph is acyclic. Traced (`acyclic(machine, cycle, preserves)` at 553). So a strict claim is replaced by a weaker one only when the weaker still leaves no cycle with only preserving edges... see unsure section.
- `Normalize::corresponds` `_ => {}` falls to an error (834-837). Fails closed.
- `if machine.ranked_scc.is_some() && ranked.len() != actual.len()` is checked only when `ranked_scc` is set (161); when `ranked_scc` is `None` and termination is not `Terminates`, `actual` is empty and nothing is checked: a cycle without a termination promise is not asked for a ranking. Stated by the contract (`Terminates` gates the requirement, 147).

## 6. refinement.rs (237 lines)

**Purpose.** (a) `prove_callable_refinement` (25, test-support only): proves a witness that calls an abstract supplied contract, returns an error for the first failing verdict, use or window, and a refusal for a supplied callable outside declared ceilings. (b) `finalize_callable_refinements` (114): derives and attaches refinement certificates to every callable descriptor in the module, via `derive_contract_obligation`, and validates them.

**How it decides.** `prove_callable_refinement` skips machines in `witness.requirements` (31-33) and a failing `PublishedGuarantee` whose matching `NormalGuarantee` fails with `Unproved(_)` (57-64) (the normal guarantee's own error is returned later in the same loop). `finalize_` skips a descriptor that already has `refinement_evidence` (157-161: "Receiving admission below authenticates the exact retained questions and certificates").

**Cases by shape.** The ceiling match lists five axes: `Reach`, `Entries`, `Suspension`, `Blocking`, `Termination` (89-105).

**Unused or one-caller code.** `prove_callable_refinement`: `#[cfg(any(test, feature="test-support"))]`; grep: 3 uses in tests (`tests/ceilings.rs:532, 566`, `tests/source_operators.rs:463`). `finalize_callable_refinements`: 1 production call (`formation.rs:257`, through `lib.rs:528 scope.finalize_callable_refinements`) plus 7 test calls (`tests/callable_contracts.rs`).

**Fail-open risks.** `if !descriptor.refinement_evidence.is_empty() { continue; }` (157): a descriptor with imported evidence skips derivation. Stated at 158-159 (receiving admission validates it). Whether `validate_descriptor_refinement_evidence` (227-235) covers each skipped descriptor's evidence is by its callee in `terminal_semantics`; not read: open for the imported-evidence case.

## 7. result_summaries.rs (702 lines)

**Purpose.** Infers an extra `ensures` clause for private helpers of the subject package from what their bodies return (comparisons about `result`, and the widest literal bounds), and adds it only after every normal return proves it. Callers then rely on it.

**Shape.** `infer` (84), `summarize` (394), `publish` (620), `checked_results` (111), `callees_first` (202), `callees` (183), `Returned` (244, implements `Bindings`), `about_result` (294), `result_choice` (318), `widest` (345), `with_formation_conditions` (372).

**How it decides.** Machines come from `Elaboration::local_results`. `summarize` skips: machines with a `PendingNormalCompletion` (404-411), non-scalar results (412-414), `Callable` results (417-419), no `Return` exits (429-431). Per return it derives parts with `entry_inputs::describe`, bounds via `prover.literal_bounds`, then keeps the intersection of parts and the widest bounds (550-557, 559-570). A summary equal to an existing `ensures`, `Truth`, or `Falsehood` is dropped (587-595). Each return must prove the summary via `decide` (599-616) before `scope.summarize`.

**Cases by shape.** `about_result` keeps only `Equal`/`LessThan`/`LessOrEqual` where each side is a whole `Value` or does not mention `result` (298-312). `result_choice` accepts a right/left side that is `Integer`, `Value`, or `Application` (330-336). "Only a scalar result is summarized" (51).

**Unused or one-caller code.** `infer`: 1 call (`formation.rs:168`). `publish`: 1 call (`formation.rs:197`). `checked_results`: 1 call (`verdict.rs:1475`). `widest` is `pub(super)`; grep for `result_summaries::widest` found no call outside this file (grepped; uses inside file at 540-541).

**Stated limits.** "Only a scalar result is summarized"; "Storage a helper writes through a borrowed formal gets no summary" (59-60). "A helper whose returns differ is summarized by what they share, never by one alternative per return" (53-55).

**Fail-open risks.** Several `return Ok(())` that drop the summary: `hypotheses(...)` failing (523-526), `entry_inputs::canonical` failing (576-578), any `decide` failing (612-614). These leave the written contract only, which is weaker. Stated at 596-598. `publish` forms a proposed contract through `formation::contract` before adding it (632-637). `checked_results` refuses when a summary's receipts do not match every normal return (118-171): traced.

## 8. reuse.rs (2831 lines)

**Purpose.** Reuses saved proofs: a consumer re-decides a certificate found elsewhere against its own goal and hypotheses via the kernel (`verify`), never searching. The production path is `Carry`/`Carried`: a machine generated from a generic machine takes its certificates from the generic check form (`lib.rs:804`, `verdict.rs:1753-1866`). `reverify`, `reverify_at_call`, `reverify_specialization` are test-support entry points.

**Shape.** `Transport` (289) moves a `ProofNode` between value namespaces (`certificate` 301, `node` 498, `term` 324, `math` 374, `proposition` 408). `Moved`, `Cited` (269, 278). `Instantiation` (903) with `of` (935), `structural_type` (1135), `unify` (1387), `pair` (1359), `moved` (1435). `Carried` (740), `CarriedForm` (747), `Selection` (726), `Carry` (1511) with `new` (1542), `certify` (1594), `corresponds` (1676), `proof` (1867), `carried` (1906), `call_requirement` (2074), `moved_from_witness` (2206).

**How it decides.**
- The consumer's goal and hypotheses are its own; only the certificate is carried (header 9-17). The kernel re-checks conclusion and each cited assumption.
- `Carry::corresponds` (1676-1740): values table must equal the generic's after removing bound value parameters (same origin, same instantiated scalar type), and invocation tables must have equal sites and moved actuals. Otherwise `Unsupported` "the specialization's values and calls are not the generic body's".
- `Instantiation::of` (935): binds rigid types, width binders, structural extents and value parameters by pairing contracts; refuses more than 8 `requires` clauses ("ponytail: tries every pairing... factorial", 1119-1123).
- `Carried::carrier` (757): first form whose `Carry::new` succeeds; `.ok()` discards the reason (776).
- `Transport::node` (498-712): fixed list of `ProofRule` forms; `IntegerCastBound` and `IntegerCorrelatedForbiddenRoots` return `Err(None)` (703-706), which refuses the move.
- `matching_proof` (2143): exactly one carried certificate must match the call judgment exactly; two matches is refused (2159-2165).

**Cases by shape.** `Transport::term` (324-372): only `Value`, `Integer`, `Boolean`, `IeeeFloatLiteral`, `ExactIntegerAdd/Subtract/Multiply`; all else `None` (370). `proposition` (415-495) lists comparison/float/connective forms; `_ => None`. `leaf_terms` (1478): "a form a contract clause of this subset does not have" returns false. `Carry::proof` (1867-1899) handles `CallRequirement`, `NormalGuarantee`, `OperationSafety`, `DataDomain`; other kinds refuse "obligation kind outside this route" (1890-1894). `structural_type` (1135-1287) handles `TypeParameter`, `PrimitiveScalar`, `Reference`, `FixedArray`, `Opaque`/`ByteSequence`/`AtomicCell`; "A container needs correspondence of every child and nominal application" returns false (1283-1285).

**Same job elsewhere.** `Transport` renames certificate values; `lib.rs:1803 substitute_goal` and `lib.rs:2017 ScalarActuals` substitute actuals into goals (not read in detail). `psi-to-native/.../selection_structural.rs:5372` also has a type named `Transport` (different job, grepped name only).

**Unused or one-caller code.**
- `reverify`, `reverify_at_call`, `reverify_specialization`: `#[cfg(any(test, feature="test-support"))]`. grep `reverify\(|reverify_at_call\(|reverify_specialization\(`: no production call; test calls in `tests.rs`, `tests/array_lengths.rs`, `tests/ranges.rs`, `tests/rankings.rs`, `tests/termination.rs`, `tests/type_parameters.rs`, `tests/value_parameters.rs` (22 hits in total listed).
- `Carried`/`CarriedForm`/`Selection`: production use in `lib.rs:429,437,690,738,804`, `machine_inputs.rs`, `verdict.rs:952,1753,1859,1866`.
- Tests: 9 `#[test]` (grep count) in 5 `#[cfg(test)] mod`s.

**Stated limits.** "the subset has no generics" (129-130; a stale sentence given that this file handles type and width parameters). "Clause ordinals in different contracts are never treated as correspondence" (796-797). "Returns only conditional mathematical evidence per obligation, not an accepted compilation" (811-812).

**Fail-open risks.**
- `Carried::carrier` `.ok()` (776): a failed `Carry::new` means "nothing is carried and the machine is decided as any closed machine is" (754-756). Fails closed (falls back to a normal proof).
- `Transport::node` `ProofRule::Primitive(_) => node.rule.clone()` (507): primitive judgments are copied unchanged; the kernel re-decides them on the moved conclusion. Traced via the final `verify` in `certify` (1618) and `verify_obligation` (228).
- `Carry::corresponds` (1683-1691) accepts extra consumer values only if they are `ValueOrigin::StorageContent` with no `rigid`. Stated at 1666-1670.
- `Instantiation::unify` loop `continue`s on a leaf pair that is not `(Value, Integer)` (1396-1405) and decides by moving the whole clause and comparing (1424-1431). Fails closed.
- `reverify_at_call` check for requirements `matches!(verdicts.get(*requirement), Some(Ok(_)))` (166-173): a missing verdict is an error. Fails closed.

## 9. static_contracts.rs (1073 lines)

**Purpose.** Admits static (trait-like) contract declarations and checks each selected implementation refines the required header. Also publishes provider-candidate conformances onto the module.

**Shape.** `check_envelope` (45), `requirement_declarations` (136), `schema_context` (258), `check_service_limit` (398), `check_deferred_callback_crashes` (429), `machine_operations` (506), `admit` (559), `publish` (790), `order_provider_candidates` (937). Tests (980-1073): 3.

**Inputs and outputs.** Reads `Elaboration` (`static_schemas`, `static_conformances`, `requirement_ceilings`, `generic_*`, `static_reach_*`, `external_provider_candidates`). `admit` returns `Ok(())` or a refusal; `publish` calls `admit` then fills `module.interfaces.provider_candidates`.

**How it decides.**
- `requirement_declarations` (136-256) compares each abstract requirement's header against the prepared bodyless machine field by field (15 comparisons at 177-249) and refuses the first difference. Keyed on equality of tables and ids, plus `identities.text(instance.member)` non-empty (163-167).
- `check_service_limit` (398): a `Closed` ceiling refuses an open callback dependency and any concrete service not in the sorted list (`binary_search`, 414).
- `admit` runs `callable_operational_refines`, then `contract_refinement_goals`, and calls `derive_contract_obligation` for each goal (742-782); a failure is `Unproved(NoDerivation)`.
- `publish` (790): compares the boundary machine against `question.required` field by field (820-862) and refuses "static provider publication changes its proved required header".

**Cases by shape.** `check_envelope` rejects non-empty termination premises: "static progress premises need retained declaration subjects" (121-130). `machine_operations`: an executable body, or a generic checking body with a selected application (514-557). `StaticImplementation::Machine` or `Native` (694-741). `schema_context` handles `Rigid`, record, scalar and place formals (283-335).

**Same job elsewhere.** `static_reach.rs` (`P06/src`, not in unit) also calls `check_envelope` (`static_reach.rs:239`) and `requirement_declarations` (`static_reach.rs:63`); `ceilings.rs:834` calls `requirement_declarations`.

**Unused or one-caller code.** grep `static_contracts::|check_envelope\(|requirement_declarations\(`: `requirement_declarations` 2 production calls (`ceilings.rs:834`, `static_reach.rs:63`) + 3 test calls; `check_envelope` 4 inside file + `static_reach.rs:239`; `admit` 1 production call inside `publish` (791) + 3 test calls; `publish` 1 (`verdict.rs:1403`).

**Stated limits.** "static progress premises need retained declaration subjects" (128). "the nested callback's crash routes need coverage at its invocation" (491). "an open crash dependency has no supplied callback application" (474).

**Fail-open risks.**
- `check_deferred_callback_crashes` `return Ok(())` when `required.crash_policy == Open` (435-437). Stated in the doc comment (426-428): an open requirement leaves crash routes to be added at selection.
- `check_envelope` with `signature == None` skips the `Parameter(position)` check (89 `if let Some(signature)`) and skips formal-namespace validation. Callers pass `None` at 628, 693 and `static_reach.rs:239`. Traced to those three sites; what a bad `Parameter` ordinal does then is not traced: open.
- `schema_context` `continue` for an opaque `Rigid` formal with no `borrowed` entry (284-295): stated ("Any predicate trying to observe it must still fail scope validation").
- `requirement_declarations` `continue` when `declaration.instance` is `None` (143-145): a declaration with no instance is not compared. Open (not traced to the producer).
- `admit` early `return Ok(())` when there are no schemas and no conformances (560-562): nothing to check.
- In `machine_operations` a body with `declaration.is_some()` uses the published runtime envelope; a body without one needs a generic declaration entry or is refused (529-534). Fails closed.

---

## Coverage

All 21 ranges in `units/unit_065.tsv` were read in full, in order:

| File | Lines read in full |
|---|---|
| `src/places/loans.rs` | 1-3136 (3136) |
| `src/places/log.rs` | 1-758 (758) |
| `src/prove.rs` | 1-5318 (5318) |
| `src/prove/counterexample.rs` | 1-73 (73) |
| `src/prove/float_integer.rs` | 1-84 (84) |
| `src/prove/float_meaning.rs` | 1-107 (107) |
| `src/prove/ieee_float_order.rs` | 1-410 (410) |
| `src/prove/mathematical_bounds.rs` | 1-389 (389) |
| `src/prove/ranges.rs` | 1-1084 (1084) |
| `src/prove/reach.rs` | 1-174 (174) |
| `src/prove/tests.rs` | 1-2818 (2818) |
| `src/prove/widths.rs` | 1-592 (592) |
| `src/publication.rs` | 1-514 (514) |
| `src/publication/accepted_closure.rs` | 1-31 (31) |
| `src/publication/advisories.rs` | 1-211 (211) |
| `src/publication/optimization.rs` | 1-261 (261) |
| `src/rankings.rs` | 1-990 (990) |
| `src/refinement.rs` | 1-237 (237) |
| `src/result_summaries.rs` | 1-702 (702) |
| `src/reuse.rs` | 1-2831 (2831) |
| `src/static_contracts.rs` | 1-1073 (1073) |

Total read in full: 21,832 lines (sum of the table). Skimmed: 0. Not read: none. Outside-unit files named above (for example `lib.rs` around 2455-2484, `places/cleanup.rs`, `static_reach.rs`, `verdict.rs`, `components_rankings.rs`, `wire/src/contract_forms.rs`) were only grepped or opened for a few lines, as marked.

## Five things a maintainer most needs to know about this unit

1. `prove.rs` and its submodules never decide anything. Every `ProofNode` is re-checked by the kernel in `proof_admission`; the prover's own judgments (such as `covered`, the counterexample check, `beyond_rules`) only select search paths or classify a failure as `NoDerivation` (reject) vs `NoRuleForGoalForm` (Incomplete, still refused). The only exceptions to "kernel re-check" are non-certificate judgments: loans (`places/loans.rs`, header 41-47), rankings structure (`rankings::admit`), and the write log (`log.rs`), which refuse by direct comparison.
2. `places/loans.rs` recomputes every `Loan` row from the lowered form and matches rows by `(captured place, access, created_at, !claimed)`; a row with no formation, or a formation with no row, is a `ForeignIdentity` defect (`loans.rs:2612-2620`, `890-898`). The per-loan verdict is a fixed pipeline: retained-use conflict, `ExclusiveOfShared`, `!permitted`, then pairwise (`loans.rs:3010-3059`). It depends heavily on `terminal_semantics` descriptor-lifetime queries (`borrowed_loan_sources`, `reference_call_lifetimes`, flow steps), which are outside this unit.
3. `reuse.rs` has two halves: production (`Carried`/`Carry`, used from `lib.rs:804` and `verdict.rs:1753-1866`) and test-support-only entry points (`reverify`, `reverify_at_call`, `reverify_specialization`, no production caller by grep). `Carry::corresponds` (1676-1740) and `Instantiation::of` (935) decide whether a specialization "is" its generic body by table equality; failure silently falls back to a normal proof (`Carried::carrier` `.ok()`, 776).
4. Wrapping, saturating, bitwise and non-literal divide/remainder/shift-right goals are classified "beyond the rules" (`prove/reach.rs`) and come out as `Incomplete` ("not supported yet", `publication.rs:126-138`), not as a rejection. `prove/ranges.rs` header lists three "provisional" kernel rules (remainder within dividend, quotient at most dividend, multiple remainder) that the kernel "trusts" without a derivation.
5. `publication/optimization.rs` is the only place in this unit that edits the program. It implements one pass (SCCP) that rewrites only boolean results to `BooleanConstant`; the result is judged again (`publication.rs:35`), with any failure reported as "compiler defect: optimized Psi failed the lowered-form check". The crate map's note that P07's README places this pass under stage 07 is confirmed wrong for this code: it lives here.

## What you are unsure about

- `rankings::settle_comparisons` (562-568): it relabels strict edges `Preserving` and marks them discharged by a `Preserving` certificate when `acyclic(preserves)` holds. I did not trace whether `FiniteIntegerRanking.bindings` (now with weakened edge labels) is re-verified by the proof-kernel replay later (`publish_comparisons`, 597-619, writes the relabeled cycle into the machine). The acyclicity test is the soundness condition, but I did not read the downstream check that consumes the relabeled `ranked_scc`.
- `reference_loan` `_ => return Ok(())` (loans.rs:1086): I did not check that every operation that forms a persistent loan in stage 05 is in that list.
- `refinement.rs:157-161` skip when `refinement_evidence` is non-empty: whether `validate_descriptor_refinement_evidence` (a `terminal_semantics` function) authenticates evidence for every skipped descriptor is not read.
- `requirement_declarations` `continue` on `declaration.instance == None` (static_contracts.rs:143-145): whether a producer can emit a bodyless requirement without an instance was not traced.
- `check_envelope(..., None)` (static_contracts.rs:89): effect of an out-of-range `Parameter` ordinal when no signature is passed.
- The reuse.rs comment "the subset has no generics" (129-130) conflicts with the generic/width handling in the same file; I recorded it as stale but did not confirm with git history.
- Duplicate-job claims against `wire/src/contract_forms.rs:108 substitute`, `psi/semantics/src/substitution.rs`, `psi-to-native/.../abstract_optimization/rules.rs:225` and `places/cleanup.rs` are "suspected"; none was read.

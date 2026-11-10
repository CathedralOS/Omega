# Unit 73 notes

Files covered (both read in full):
- `omega-rust/psi/compilation/proof/admission/src/bounded_denotation.rs` (lines 1-18126)
- `omega-rust/psi/compilation/proof/admission/src/classicality.rs` (lines 1-141)

Paths below are relative to `omega-rust/psi/compilation/proof/admission/src/`. "bd" = bounded_denotation.rs. Everything marked "read" was read line by line; counts come from the Grep tool's count mode and are named where given.

Rough layout of bd (read): header 1-38; `mod addition` 40-1280 (tests 605-1279); `mod binary_numerals` 1281-1898; `mod booleans` 1899-3069; `mod casts` 3070-3611; `mod denotation` 3612-5518; `mod equality_transport` 5519-7109; `mod elaboration` 7111-8707; `mod forbidden_roots` 8709-10635; `mod integer_operations` 10636-11114; `mod multiplication` 11115-11902; `mod order_translation` 11903-12234; `mod source_content` 12235-12378; `mod source_integer` 12379-13398; `mod subtraction` 13400-15146; public API and shared helpers 15149-15463; test modules 15465-17936; `test_utilities` 17938-18126. Roughly 55% of the file is `#[cfg(test)]` code.

---

## 1. bounded_denotation.rs

### Purpose
Takes a bounded proof tree (`ProofNode` with `ProofRule`) plus its context, assumptions and "semantic axioms", and rebuilds it as a term in the mathematical core (`TermArena`, `Term`, `Declaration`). The goal proposition becomes a type, each rule application becomes a term, and the result is a `MathematicalCertificate` that `verify_mathematical_certificate` re-checks (bd:1-9, 15343-15363). Where the elaborator cannot build a real derivation for a rule, it declares the rule instance as an opaque assumption (see Fail-open). It also hosts two public "source theories" used by other crates (`SourceIntegerTheory`, `SourceContentTheory`).

### Shape (main types and entry points)
- `BoundedDenotation { arena, certificate, rules: Vec<AcceptedProofRule>, assumptions, semantic_axioms }` bd:15149-15155; `receipt()` bd:15158.
- `BoundedDenotationError { Certificate(ProofError), Unsupported(&'static str), Kernel(CoreError), DepthLimitExceeded }` bd:15172-15177.
- Public entry points: `denote_bounded_certificate` bd:15273 (no kernel re-check), `verify_bounded_certificate` bd:15324 (denote then `verify_mathematical_certificate`), `pub(crate) verify_bounded_certificate_with_machine_parameters` bd:15343. Private `denote_bounded_certificate_with_machine_parameters` bd:15290 validates goal and every assumption against `PropositionContext`, builds `Elaboration::new`, runs `node`, then compares `proof.conclusion != *goal` (bd:15316).
- `Denotation` struct bd:3846-3887: arena plus ~25 interned-position tables (`atoms`, `carriers`, `terms`, `content_terms`, `integer`, `integer_less_than`, `integer_less_or_equal`, `integer_laws`, `math_terms`, `closed_math_values`, `integer_operations`, `cast_identities`, `decisions`, `carrier_bounds`, `rule_axioms`, `constants`, `denotations` (logical cache)) plus per-operation sub-structs (`binary_numerals`, `subtraction`, `addition`, `multiplication`, `forbidden_roots`). `Denotation::new` bd:4400, `new_in` bd:4404.
- `Elaboration` bd:7117-7133 (fields: `denotation`, `context`, `ambient_assumptions`, `axioms`, `machine_parameter_values`, `roster`, `positions`, `depth`, `steps`, `rules`, `assumptions: PremiseRoster`, `semantic_axioms: PremiseRoster`). `Elaboration::new` bd:7759, `node` bd:7802 (-> `walk::elaborate` bd:7486 -> `drive` bd:7513), `finish_regular` bd:7809-8559 (one match arm per `ProofRule`), `finish` bd:8682.
- Public theories: `SourceIntegerTheory` bd:13301 (`new`, `source_safety_roster`, `vocabulary`, `literal`, `closed_predicate`, `predicate_from_assumptions`, bd:13307-13382), `SourceIntegerVocabulary` bd:13291, `SourceContentTheory` bd:12248 (`new`, `carrier`). Both temporarily swap the caller's arena/declarations into an internal `Denotation` (bd:12254-12257, 13390-13395).
- Shared law vocabulary: `IntegerLaw` (10 variants) bd:15366-15377, `integer_law_statement` bd:15405-15463; `IntegerRelation` bd:15399; `MathTermKey { Closed(BigInt), Open(IntegerMathTerm) }` bd:15380.

### Inputs and outputs
Reads: `semantic_vocabulary` (Proposition, ScalarTerm, IntegerMathTerm, ContentTerm…), `crate::kernel::decide_primitive` (bd:20, used bd:7816), `crate::proof::*` rule-check functions (`subtract_order::check`, `add_order::check`, `subtract_antitone::check`, `order_discreteness::check`, `integer_order_rules::*`, `equality_rules::*`, `integer_bound_rules::*`, bd:7976-8521), `lift_fixed_integer_relation` and `propositions_match_under_integer_math_normalization` (bd:29-31), `ClosedIntegerEvaluator`, `mathematical_core::{scheme_dsl, substitution, term, typing, certificate, conversion}`. Produces: arena terms, `Declaration`s (assumptions via `Declaration::assumption`, definitions via `Declaration::definition` bd:1366), and the certificate. Mutates only its own `Denotation`/`Elaboration`, except the two source theories which swap into the caller's arena.

### How it decides
Mostly structural comparison of denoted terms (`arena.structurally_equal`) and equality of `Proposition`/`ScalarTerm` values (`==`), not spelling. Specific judgments:
- Rule acceptance: each `finish_regular` arm first calls an external `*_relation`/`check` function and returns `Err` on mismatch (e.g. bd:8066, 8083, 8108, 8139, 8151, 8175, 8212, 8242, 8264, 8281, 8315, 8421, 8476, 8513), then tries a derivation, then falls back to `rule_instance` (bd:8640).
- Assumption citation `cited` bd:8561: keyed on index into `roster` and `propositions_match` (integer-math-normalised equality) bd:8569, then `same_denotation` bd:8587 demands structurally equal denotations or returns `Unsupported("integer-math-normalized citation across denotation shapes")`.
- Atoms are interned by the whole `Proposition` value after `lift_fixed_integer_relation` (bd:4467); scalar leaves by `ScalarTerm` (bd:4506-4510); math terms by `MathTermKey` (bd:4756-4787); closed math values by BigInt value; logical syntax by canonical key (`LogicalSyntax`, bd:3623-3631).
- Equality orientation for substitution is decided by structural equality of endpoints (`directed_equality` bd:5253, `order_substitution_evidence` bd:5229-5239, `symmetry_evidence` bd:5009, `transitivity_evidence` bd:5026 tries four endpoint matchings bd:5042-5072).
- Bound rules (`IntegerAffineBound` bd:8310-8398) try derivers in a fixed order: `unsigned_subtract_bound_evidence`, `correlated_subtract_bound_evidence`, `correlated_add_bound_evidence`, `correlated_multiply_bound_evidence`, `direct_add_bound_evidence`, `direct_subtract_bound_evidence`; first `Some` wins, else `rule_instance`.
- Search budgets: `MAX_ELABORATION_NODES = 1<<16` bd:38, used as `remaining` in boolean case analysis (bd:2063), `source_rewrite_proof` (bd:6332), `compositional_transport` (bd:6396), `oriented_evidence` (bd:6657) and as the proof-node step cap (bd:7503-7506, `DepthLimitExceeded`).
- Source theories: depth cap `> 128` (bd:12467, 12567, 12813, 12841, 12926, 12990).

### Cases by shape (fixed lists that refuse the rest)
- Direct add/subtract bounds: premise must be a two-element `Proposition::Conjunction` (`let [left_bound, right_bound] = bounds.as_slice() else { return Ok(None) }` bd:4141-4146) and conclusion an `IntegerMathLessOrEqual` with an `IntegerLiteral` bound (bd:4162).
- `affine_witness_root_expression` bd:15203-15215: only zero or exactly one definition axiom; otherwise `None`.
- Correlated forbidden roots: signed fixed carrier only (bd:9929-9932); `[lower_bound, upper_bound] = bound_evidence` exactly two (bd:9982); both chains nonempty and `cursor == axiom_evidence.len()` (bd:9957-9961); steps are only add/sub/multiply with `integer_type` equal (bd:8853-8876); literal values `i128` (bd:8741); divisor case split hard-wires `d <= -2`, `d == -1`, `d == 0`, `1 <= d` (bd:9125-9161).
- Cast bounds: `Truth` root only non-strict (bd:3232-3234); each edge must be an `IntegerWiden`/`IntegerExactCast` definition (bd:3139-3163); carriers need `fixed_native_interval` (bd:3246).
- Multiply bound: sign evidence only `1 <= r` or `r <= -2` (bd:3419-3425 region is casts; multiply is bd:11419-11428); root expression must be an `ExactIntegerDivide` of the endpoint by the same divisor (bd:11442-11452).
- `IntegerOperation` table of 24 operations with arity 1 or 2 (bd:10872-10925, 10945-10969).
- `ChildEvidence`/`Children` hold at most three fixed children plus a slice (bd:7186-7219).
- Source integer: `closed_source_predicate` handles only `Id`, `<`/`<=` order, `Sigma`, and `Box (Empty->Empty)` (bd:12571-12651); `source_normal_integer` expands only add/sub/mul and distributes only a multiply over an add on the right (bd:12856-12912); `source_order_goal` only splits `<=` of two additions (bd:13001-13022); dependent `Sigma` premises return `Unsupported("dependent source predicate premises require value abstraction")` (bd:12931-12934).
- Equality coercion `coercion::construct` bd:5614 matches only (Id,Id), (Id over Two, anything), (Pi,Pi non-dependent), (Sigma,Sigma), then falls back to shuffling and tagged-sum splitting (bd:5659-5780).
- Closed inhabitants `inhabitants::step` bd:6173: identity with equal endpoints, closed integer order, non-dependent Sigma/Pi, tagged sum with a non-dependent branch; everything else `Ready(None)` (bd:6242-6269).

### Same job elsewhere (within the unit, and suspected outside)
- Three per-operation "correlated bound" functions share one skeleton (`affine_witness_root_expression`, `affine_root_premise`, `affine_carrier_endpoint_equality`, `affine_root_equality_step`, `affine_endpoint_equality_order`): add bd:195-342, subtract bd:13819-13964, multiply bd:11375-11632. Add and subtract also share `BinaryBoundOperation` bd:4084 (built by `Addition::bound_operation` bd:583 and `Subtraction::bound_operation` bd:14029) feeding `direct_binary_bound_evidence` bd:4133 and `exact_binary_definition_bound_evidence` bd:4236; multiply does not use it.
- Numeral equations: `numeral_sum` bd:173, `numeral_difference` bd:13966, `numeral_product` bd:11313 all go through `numeral_binary_operation` bd:11080; `numeral_quotient` bd:11335 is a separate copy of the same intern-an-Id-assumption pattern (Rust BigInt `div_rem`).
- Per-operation `Law` enums, each with a `roster_law_position` statement builder: `addition::Law` bd:62, `binary_numerals::Law` bd:1298, `subtraction::Law` bd:13416, `multiplication::Law` bd:11131, `forbidden_roots::Law` bd:8965, plus global `IntegerLaw` bd:15366. All route through `roster_law_position` bd:4720.
- Six hand-written explicit-stack drivers for recursive structure: `logical::denote` bd:3774, `scalar_denotation::denote` bd:1915, `fixed_scalar::denote` bd:10674, `coercion::construct` bd:5614, `inhabitants::construct` bd:6096, `walk::drive` bd:7513.
- Four BigInt <-> `IntegerValue` converters: `binary_numerals::fixed_magnitude` bd:1314, `integer_value_of` bd:15385, `math_literal_value` bd:15252, the local `numeral` closure in `closed_order_bound_evidence` bd:514-525; also `unbounded_integer` bd:15266 (reverse direction).
- "Apply the <= or < relation constant to two terms": `order_term_application` bd:4865, `integer_le_term` bd:9169 (forbidden_roots), `cast_order_term` bd:3130 (wraps the first). Numeral to term: `binary_term` bd:1393 vs `integer_numeral` bd:9179.
- Equality orientation logic appears three times: `directed_equality` bd:5253, inline in `order_substitution_evidence` bd:5229-5239, and in `symmetry_evidence` bd:5009-5024.
- Tagged-sum eliminator / injection builders: `case_term` bd:7715, `tagged_sum_elim` bd:9694, `Cases` plumbing bd:5991-6067, `disjunction_intro` bd:8654 and `disjunct` bd:9736 (the latter hard-codes indices 0, 1, "2+"). Suspected same job; not compared line by line.
- `Term::EmptyElim {` constructed inline 10 times in bd (Grep count mode, pattern `Term::EmptyElim \{`), while a helper `empty_elimination` exists at bd:9648 (used only inside forbidden_roots).
- Closed-goal inhabitation done by `closed_inhabitant` bd:6661 (`inhabitants`), `boolean_leaf_inhabitant` bd:2184, and `closed_source_predicate` bd:12562.
- Outside the unit: rule-check functions (`subtract_order::check`, `integer_bound_rules::*`, etc.) live in `proof.rs`/other files; I did not read them (suspected the same "does this witness match" logic as the `correlated_*` functions here, but unverified).

### Unused or one-caller code (greps)
- `denote_bounded_certificate` / `verify_bounded_certificate` outside bd: 11 occurrences in 5 files (`tests/theorem_certificate_admission.rs` 2, `proof.rs` 4, `lib.rs` 2, `mathematical_core.rs` 1, `terminal-psi/semantics/src/descriptors.rs` 2). Command: Grep count, pattern `denote_bounded_certificate\b|verify_bounded_certificate\b`, glob excluding bd.
- `SourceIntegerTheory|SourceContentTheory` outside bd: 9 occurrences in 3 files (`proof/src/mathematical_declarations.rs` 7, `mathematical_core.rs` 1, `lib.rs` 1).
- `SourceIntegerTheory::source_safety_roster` and `SourceIntegerTheory::vocabulary` callers not separately counted (inferred used from `mathematical_declarations.rs`; not verified).
- `Denotation::{add_order_translation, add_order_both}` have in-file callers (bd:12 `mathematical_order_translation` bd:11925, `source_order_goal` bd:13022). `Elaboration::finish` etc. fine.
- `Denotation::disjunction` bd:5460 is `pub(super)`; in-file callers: `Pending::CaseZero` bd:7667 and tests. `Denotation::position` etc. are in-file.
- The docs here do not claim "single/shared/canonical" except: comment bd:3857-3860 (content carriers "can never compose" across algebras), bd:3863-3867 (key is the term itself, "syntactic identity is the algebra's own canonical form"), bd:12236-12240 ("Source content values use the same opaque canonical algebra carriers as checked content derivations") and bd:12247 ("Stored handles belong to that arena/signature pair, just as they do in SourceIntegerTheory"). The first claim is checked by test bd:17798-17846; the third is tied to the `content_carrier_at` function (bd:4545) that both `Denotation::content_carrier` (bd:4524) and `SourceContentTheory::carrier` (bd:12260) call, so the sharing is real inside the file.

### Stated limits
- bd:12932 "dependent source predicate premises require value abstraction" (Unsupported).
- bd:11969 "translated numeral exceeds existing numeral evidence" (Unsupported) and bd:11974 "translated numeral lacks exact addition evidence".
- bd:1498 "discreteness needs a fixed literal"; bd:1654 "checked discreteness has integer order endpoints"; bd:1668 "checked discreteness denotes in Int"; bd:9013, 9026 antisymmetry endpoint messages.
- bd:6983 test name documents a default-stack bound: "deep_oriented_refutation_uses_default_stack_and_keeps_true_premises_open".
- Test comments: bd:1199 "an unrepresentable endpoint sum keeps the explicit instance assumption"; bd:14469 same for subtract.
- `MAX_ELABORATION_NODES` caps (bd:38) and depth caps (bd:12467 etc.) are the only numeric limits.
- No "TODO"/"migration debt" comments seen in the unit.

### Fail-open risks
1. `rule_instance` bd:8640-8652 / `Denotation::rule_axiom` bd:5272-5293: when no real derivation is built, the elaborator interns an assumption of type `premises -> conclusion` and applies it to the evidence. It is used as the fallback at bd:7962, 7991, 8019, 8053, 8092, 8125, 8161, 8195, 8228, 8302, 8392, 8460, 8504, 8551. Wrong program that could get through: a rule application whose external `*_relation`/`check` accepted something false. The elaborator itself adds no check at that point; acceptance rests on the earlier check in the same arm (read: each arm calls its check before the fallback and uses `?`). Classification: traced for call order; the checkers themselves are outside this unit and were not read. The resulting assumption is visible: it enters `signature` and the assumption closure (`certificate_assumption_closure`), and tests assert `rule_axioms.is_empty()` for derivable cases, e.g. bd:721, 1006, 1089, 3426.
2. `Primitive` arm bd:7815-7824: after `decide_primitive` passes, if `closed_inhabitant` returns `None`, `Denotation::decision(denoted)` (bd:5295) interns the denoted type itself as an opaque assumption. Wrong program: any primitive judgment `decide_primitive` wrongly accepts. Classification: traced for flow; `decide_primitive` unread (`kernel`). Tests at bd:17907-17921 assert no decision for closed integer order.
3. `affine_root_equality_step` bd:5313-5315 and `affine_endpoint_equality_order` bd:5348-5350: `None` equality returns `Ok(Some(evidence))` unchanged. Traced: `None` comes from `affine_witness_root_expression` bd:15207-15208 (witness with no definition/literal axioms, so root is used directly) and `affine_carrier_endpoint_equality` bd:15242-15243 (endpoint already the carrier literal). In both cases the premise equality was checked separately (e.g. `premise != &expected_premise` bd:263, 1887 region). "stated" is not present as a comment; the logic is traced.
4. `rewrite_atom` bd:2518-2524 returns `Some(term)` unchanged for `Variable/Constant/Sort/Two/...`: not an accept, just no rewrite; other term kinds return `None` (refuse). None seen as fail-open.
5. `source_rewrite_proof` bd:6299-6318 silently `continue`s on identities whose carrier is not `Int` or whose endpoints are not declaration-without-body parameters (equations dropped). Wrong program: not accepted; a dropped equation only weakens the rewrite, and the final `solve` must still produce evidence. Classification: traced; not a fail-open.
6. `compositional_transport` bd:6381-6385 skips equations that are no-ops or duplicates (`from` equal to `to` or already present); bd:6386 refuses `from` that is not a `Constant`. Not fail-open.
7. `integer_value_of`/`fixed_magnitude` return `None` on overflow, and callers return `Ok(None)` (fall back to `rule_instance`, i.e. item 1) rather than error. So an out-of-range numeral lands on the opaque-instance path, not rejection: bd:1199 and bd:14469 tests document this ("keeps the explicit instance assumption"). Classification: stated (test message).
8. `cited` bd:8576-8578: `index >= self.ambient` (discharged hypotheses) are not recorded in `assumptions` roster; recorded only for ambient premises. This is by design (test bd:15746-15807: "a discharged hypothesis is not an ambient premise").
9. `PartialNode::new` bd:7339-7345 uses `unreachable!` for scoped rules; guarded by `drive` bd:7524-7556 sending those rules elsewhere. Panic risk only on a code change.
10. `Denotation::new`/`logical::Cache::publish` uses `.expect("an invalid logical arity cannot finish denotation")` bd:3676; arity errors are returned earlier (bd:3790-3799). Panic not accept.
Overall: no case seen where an unmatched shape is accepted without either a real term or an explicit opaque assumption; the open question is the correctness of external checkers.

---

## 2. classicality.rs (141 lines, read in full)

### Purpose
Classifies each `AcceptedProofRule` as constructive, constructive-decidable, trusted-admission or classical, and says which rules cite semantic axioms (classicality.rs:1-8, 19-64).

### Shape
- `enum ProofRuleFoundation { Constructive, ConstructiveDecidable, TrustedAdmission, Classical }` classicality.rs:11-17.
- `impl AcceptedProofRule { const fn foundation(self) -> ProofRuleFoundation }` classicality.rs:20-51; `const fn cites_semantic_axioms(self) -> bool` classicality.rs:53-63.
- Tests classicality.rs:66-141 with a hand-listed `ALL: [AcceptedProofRule; 28]` classicality.rs:70-99.

### Inputs/outputs and decisions
Pure match on the enum variant (identity of the rule). `Primitive` and ten others -> `ConstructiveDecidable`; `SemanticAxiom` -> `TrustedAdmission`; sixteen others -> `Constructive` (classicality.rs:22-49). No variant maps to `Classical` (test classicality.rs:101-110).

### Cases by shape
The match is exhaustive over 28 variants (no wildcard), so a new `AcceptedProofRule` variant fails to compile there; the test array `ALL` is a separate hand list (classicality.rs:70) with length 28 typed in the array, so adding a variant would also need that list updated (compile error from array length only if the count differs; otherwise the test silently omits it). Read; inferred for the second point.

### Same job elsewhere
`cites_semantic_axioms` (classicality.rs:53) is a second list of the rules that cite semantic axioms; bd's `finish_regular` has its own cites: `SemanticAxiom` bd:7826 and the bound rules (`cited_axiom` bd:8626 called at bd:8334, 8431, 8489, 8528) record into `semantic_axioms` roster. Not compared programmatically; suspected same set.

### Unused/one-caller (greps)
- `.foundation()|ProofRuleFoundation|cites_semantic_axioms` outside classicality.rs: 10 occurrences in 2 files (`proof/src/lemmas.rs` 9, `admission/src/lib.rs` 1). Grep count mode.
- `ProofRuleFoundation` is described as letting consumers "audit whether a certificate remains within the constructive core" (classicality.rs:5-6): outside users exist (9 in `lemmas.rs`), not traced further.

### Stated limits / fail-open
None seen. `Classical` has no producer; `TrustedAdmission` only for `SemanticAxiom` (test classicality.rs:112-121). Note `PredicateDenotation`, `IntegerAffineBound` and other rules whose bd fallback is an opaque `rule_axiom` are labelled `ConstructiveDecidable`, not `TrustedAdmission` (classicality.rs:40-49 vs bd:8640): the foundation label does not reflect whether the fallback assumption was used for a particular certificate. Read, noted as an observation.

---

## Coverage
- bounded_denotation.rs 1-18126: read in full, 18126 lines. Skimmed: 0. Not read: 0.
- classicality.rs 1-141: read in full, 141 lines.
- Total read in full 18267 of 18267. Not read: none.

## Five things a maintainer most needs to know
1. The elaborator does not prove everything itself. When it cannot construct a derivation it interns an opaque assumption (`rule_instance` bd:8640 via `rule_axiom` bd:5272; `decision` bd:5295 for primitives bd:7823). Safety of those paths rests on the external check run just before in each arm of `finish_regular` (bd:7809-8559). Tests count `rule_axioms`/`decisions` to show which shapes derive (e.g. bd:721, 1006, 2778, 14110).
2. Bound rules have a fixed ladder of derivers (bd:8323-8383): unsigned-subtract, correlated subtract/add/multiply, direct add/subtract, then opaque. Each deriver recognises only narrow witness shapes (see "Cases by shape"), so adding a new arithmetic form means a new sibling of the three near-copy `correlated_*` functions.
3. Everything is interned by position in `declarations`; every law, operation, numeral and carrier is an assumption declaration plus a cached position (`interned_keyed_assumption` bd:4704, `roster_law_position` bd:4720). `source_safety_roster` functions (bd:596, 1690, 11636, 14042, 13330) enumerate the positions a source consumer must trust; `SourceIntegerTheory::source_safety_roster` omits `forbidden_roots` and `cast_identities` laws (read bd:13330-13346).
4. Recursion is deliberately converted to explicit stacks, and tests spawn threads to prove default-stack safety (bd:6983, 16865, 16970, 17233, 17455). Several terms are searched with a shared `remaining` budget of `1<<16` (bd:38).
5. `SourceIntegerTheory` and `SourceContentTheory` (bd:12248, 13301) are public and reuse the same `Denotation` by swapping arenas; outside users are in `proof/src/mathematical_declarations.rs` (7 hits) and two others.

## What I am unsure about
- I did not read the external checkers (`integer_bound_rules::*`, `subtract_order::check`, `equality_rules::*`, `decide_primitive`), so whether the opaque-assumption paths can admit a false judgment is undetermined here.
- The claim that `SourceIntegerTheory::source_safety_roster` omits some laws is from reading bd:13330-13346 only; I did not trace whether `forbidden_roots`/cast laws can appear in source terms.
- The `disjunct` (bd:9736) vs `disjunction_intro` (bd:8654) similarity, and `cites_semantic_axioms` vs the elaborator's roster recording, are marked suspected.
- Counts for `SourceIntegerTheory::vocabulary` and `source_safety_roster` callers were not separately run.

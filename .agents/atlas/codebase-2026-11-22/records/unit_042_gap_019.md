# Record: unit 042, gap 019 (second pass)

Range: `source-rust/subsystems/compiler/psi/semantics/proofs/bounded/bounded_denotation.rs` lines 3101-9100 (6,000 lines), all read.
The file is one 17,334-line file of nested `mod`s; every module below adds methods to one `Denotation` struct (`:3821`) or drives one `Elaboration` (`:7010`). The first reader covered lines 1-3100 (see `unit_042.md`). Line 3101 starts inside the `cast_bound` module's `impl`; I did not see its start.

## 3101-3586: tail of the cast-bound module (`cast_bound_evidence` and tests)

1. **Purpose.** Builds kernel evidence that a bound on an integer carries through a chain of exact casts or widenings: from `root <= k` to `target <= k`, one cast step at a time.
2. **Shape.** `cast_edge_definition` (`:3114`) accepts a semantic axiom `next = IntegerWiden(..)` or `next = IntegerExactCast(..)` and returns the operation. `cast_bound_evidence` (`:3140`) is the entry. It builds one `CastEdge` per chain axiom (`:3170`), then rewrites the bound edge by edge with `order_substitution_evidence` (`:3282`). Tests at `:3304-3585`.
3. **Inputs/outputs.** Inputs: the root bound proposition, its evidence, a `CheckedIntegerCastChain` (`root`, `target`, `definition_axioms`, `carriers`), axiom evidence, the semantic axioms, the conclusion. Output: `Ok(Some(term))`, or `Ok(None)` for "cannot derive here".
4. **How it decides.** By the denoted kernel terms of the goal: `integer_relation(goal)` must be `<` or `<=` (`:3150`). The root bound must have the root on one side, compared by structural term equality (`:3265-3270`). For a `Truth` root bound it picks the tightest carrier by numeric interval (`fixed_native_interval`, `:3221-3239`, uses `i128` bounds). It keys on identity (`TermHandle` structure and axiom index), not spelling.
5. **Cases by shape.** Truth root with a strict goal is refused (`:3207`). A Truth root needs exactly one of the goal's sides equal to the target (`:3212`). It needs one carrier per value (`:3215`). Edges other than widen or exact cast return `None` (`:3135`).
6. **Same job elsewhere.** The `order_substitution_evidence` rewrite loop is also how `IntegerOrderSubstitution` elaborates (`:8313`).
7. **Unused/one-caller.** `cast_bound_evidence`: grep shows its definition (`:3140`) and one call (`:8508`).
8. **Stated limits.** None in the range.
9. **Fail-open.** None seen. Every unsupported shape returns `Ok(None)`, and the caller (`:8508-8524`) then falls back to `rule_instance` (see below), but only after `cast_bound_relation` accepted the witness (`:8491`). Classification: traced.

Tests (`:3304-3585`): four `#[test]` functions cover one edge, a multi-edge word in both directions, the Truth root with carrier membership, and live endpoints that keep strictness. Each asserts `rule_axioms == 0` and counts cast identities. The last one also tampers with the conclusion and drops the axioms and asserts refusal.

## 3587-5486: `mod denotation` (proposition to kernel term, and its helper laws)

1. **Purpose.** Defines `Denotation`, the table that turns semantic propositions into mathematical-kernel types and builds proof terms for the integer order and equality laws.
2. **Shape.**
   - `logical` submodule (`:3592-3819`): `Cache` (`:3609`: atom map `BTreeMap<Proposition, SyntaxKey>`, logical map keyed by child keys, `terms: Vec<Option<TermHandle>>`). `Prepared::new` (`:3675`) walks a proposition with an explicit stack and no recursion. `denote` (`:3749`) is an explicit-stack driver (`pending` frames). Conjunction becomes nested `Sigma`, disjunction a tagged sum, implication `Pi`.
   - `Denotation` (`:3821-3862`): `arena: TermArena`, `declarations: Vec<Declaration>`, about 15 interning tables (`BTreeMap`/`HashMap` keyed by `Proposition`, `ScalarType`, `ScalarTerm`, `MathTermKey`, `IntegerLaw`, ...), plus per-family sub-tables (`binary_numerals`, `subtraction`, `addition`, `multiplication`, `forbidden_roots`).
   - Generic binary-bound machinery: `BinaryBoundOperation` (`:4059`), `BinaryBoundChain` (`:3911`), `binary_bound_chain` (`:3924`), `binary_bound_order` (`:3978`), `exact_definition_bound_target` (`:4014`), `direct_binary_bound_evidence` (`:4108`), `exact_binary_definition_bound_evidence` (`:4211`).
   - `PositionRoster<K>` trait (`:4082`), implemented for `BTreeMap<K,u32>` and `HashMap<K,u32>`.
   - Interning: `interned_slot_assumption` (`:4651`), `interned_slot` (`:4666`), `interned_keyed_assumption` (`:4679`), `roster_law_position` (`:4695`), `roster_law_application` (`:4710`).
   - Term builders: `atom` (`:4438`), `carrier` (`:4452`), `scalar_leaf` (`:4474`), `content_carrier` (`:4499`), `content_term` (`:4556`), `math_term` (`:4724`), `fixed_scalar_relation` (`:4804`), `integer_relation` (decoder, `:4859`), `symmetry` (`:4904`), `transitivity` (`:4940`).
   - Evidence builders: `symmetry_evidence` (`:4977`), `transitivity_evidence` (`:4994`), `weakening_evidence` (`:5052`), `order_transitivity_evidence` (`:5099`), `order_substitution_evidence` (`:5145`), `directed_equality` (`:5221`), `rule_axiom` (`:5240`), `decision` (`:5263`), `affine_root_equality_step` (`:5272`), `affine_endpoint_equality_order` (`:5307`).
   - `denote_leaf` (`:5350`), `conjunction_terms` (`:5415`), `disjunction` (`:5428`), `tagged_sum` (`:5464`).
3. **Inputs/outputs.** Reads propositions, scalar terms, `IntegerMathTerm`s. Writes into `arena` and appends `Declaration::assumption(..)` entries to `declarations`. Each new declaration is a stated-but-unproved constant in the kernel signature (propositions-as-atoms, integer carrier, order relations, laws, numerals).
4. **How it decides.**
   - Atom identity: a proposition is an atom keyed by value equality of the `Proposition` after `lift_fixed_integer_relation` (`:4443`), so two spellings of the same fixed-integer relation share one atom.
   - `integer_relation` recognizes a relation by comparing the kernel `Constant.declaration` against the stored slots `self.integer`, `integer_less_than`, `integer_less_or_equal` (`:4862,4876,4881`). It keys on declaration identity.
   - `math_term` (`:4724-4763`): closed terms are evaluated by `ClosedIntegerEvaluator` and keyed by value (`MathTermKey::Closed`); open terms keyed by the term itself. An evaluator `Err` is returned as a certificate error (`:4756`), not swallowed.
   - Law choice: `order_transitivity_evidence` selects among four laws by the strictness triple (`:5128-5134`); other triples return `None`.
   - `fixed_scalar_relation` (`:4812`): only integer carriers that are `Fixed` and not address types. All else returns `None`, then `denote_leaf` falls back to an opaque `atom` (`:5383`).
   - Content algebras: keyed by `(ContentAlgebra, ContentTerm)` (`:3842`). The comment at `:3838-3841` says syntactic identity is the algebra's canonical form.
5. **Cases by shape.** `Prepared::new` treats a conjunction or disjunction with fewer than two members as keyless and `denote` then errors with `RuleConclusionMismatch("conjunction arity")` / `("disjunction arity")` (`:3765-3774`). `direct_binary_bound_evidence` requires a premise that is a 2-element conjunction (`:4119`: "let [left_bound, right_bound] = ..."), `Fixed` carrier, empty definition and literal axiom lists (`:4125-4129`). `exact_definition_bound_target` requires `LessOrEqual` with an integer literal on exactly one side and an `Equal` definition whose one side is a `ScalarTerm::Value` (`:4019-4040`). `content_carrier_at` has two algebra kinds, `IntervalSet` and `CountedQuantity` (`:4526-4529`).
6. **Same job elsewhere.**
   - `symmetry`/`transitivity` (`:4904,4940`) are built by hand with `Term::IdElim` here. `forbidden_roots` has its own `integer_id_elim` helpers (`:8813`), a second route to the same J-eliminator pattern (read; `integer_id_elim` itself is outside my range).
   - `directed_equality` (`:5221`) and the `if structurally_equal(s,old) .. else if (s,new) .. symmetry` pattern is repeated inline in `order_substitution_evidence` (`:5197-5207`) and `compositional_transport` (`:6282-6288`).
   - Three interning styles coexist: `interned_slot*`, `interned_keyed_assumption`, and `roster_law_*`. `PositionRoster` exists because some rosters are `HashMap` and some `BTreeMap` (`:3829-3858`; `decisions` and `constants` are `HashMap`, the rest `BTreeMap`).
   - Dense-ID rule in AGENTS.md prefers flat tables; these maps are keyed by values, not dense ids (observation only).
7. **Unused/one-caller (greps in the file, `grep -c -F`).**
   - `exact_definition_bound_target(`: 1 match, the definition only (`:4014`), plus its call at `:4230` is the single match for the helper itself in a form that counts as "1" because the `fn` line spells `exact_definition_bound_target<'a>(`. I did not find an external caller; unverified outside this file.
   - `direct_binary_bound_evidence(`: 3 matches (definition `:4108`, calls `:340`, `:13031`; other crate files: none).
   - `exact_binary_definition_bound_evidence(`: 3. `binary_bound_chain(`: 3. `binary_bound_order(`: 3.
   - `weakening_evidence(`: 2 (definition + `:8178`). `compositional_transport(`: 2 (definition + `:8049`). `symmetry_evidence(`: 4. `math_less_or_equal_sides(`: 5.
   - `rule_axiom` is called only through `rule_instance`, per `grep -c 'rule_instance('` = 23 (definition `:8660` plus call sites).
8. **Stated limits.** `Unsupported("integer-math-normalized citation across denotation shapes")` (`:8625`, returned by `same_denotation`). `DepthLimitExceeded` doubles as the error for `u32` overflow of declaration positions (`:4423`).
9. **Fail-open.**
   - `denote_leaf` fallthrough `atomic => self.atom(atomic)` (`:5411`) and the `Equal/LessThan/LessOrEqual` fallback to `self.atom(proposition)` (`:5383`): an unrecognized proposition becomes an opaque assumption of type `Type 0`. A proposition the denotation does not understand is therefore not dropped, but it denotes a fresh atom that nothing proves, so a goal on it cannot be inhabited. Classification: traced (the opaque atom is only equal to itself; `atom()` keys by value at `:4443`).
   - `rule_axiom` (`:5240-5261`): adds `premises -> conclusion` as an assumed declaration (`Declaration::assumption`), see `rule_instance` below. This is the main acceptance route in the range, classified under the Elaboration entry.
   - `closed_math_value` returns `None` for unknown handles (`:4790`); callers treat `None` as "no closed value" and decline. Not fail-open.

Tests: no tests in 3587-5486 itself.

## 5487-7002: `mod equality_transport` (coercion search, inhabitants, replacement transport)

1. **Purpose.** Given a premise type, a goal type and evidence of the premise, tries to build evidence of the goal by structure: reflexive/closed inhabitants, ex-falso from false closed relations, function/product/sum shuffles, and rewriting along equalities (`ValueEqualityTransport`).
2. **Shape.**
   - `coercion::construct` (`:5582`): an explicit-stack search over `Work` and `Pending` enums (`:5525-5580`); no recursion. A shared counter `remaining: &mut u64` is decremented once per `Coerce` (`:5594`).
   - `inhabitants::construct` (`:6064`) with `step` (`:6141`): builds a closed inhabitant of a goal type: `Refl` for equal sides, `literal_order` for true closed integer inequalities, pairs for non-dependent `Sigma`, tagged sums, lambdas for non-dependent `Pi`.
   - `Denotation::compositional_transport` (`:6255`), `expand_equations` (`:6339`), `replace_evidence` (`:6367`), `replacement_context` (`:6407`), `oriented_evidence` (`:6556`), `closed_inhabitant` (`:6569`), `closed_relation_refutation` (`:6576`), `tagged_sum_parts` (`:6714`), `occurs_free` (`:6745`).
3. **Inputs/outputs.** Inputs: `TermHandle`s for premise/goal/evidence and a budget. Output: `Option<TermHandle>`; `None` means "not derivable by this route". Mutates the arena only.
4. **How it decides.**
   - Structural term equality (`structurally_equal`) throughout: `:5597`, `:5637-5639`.
   - `closed_relation_refutation` decides by closed numeric values: `LessThan` with `left_value >= right_value`, or `LessOrEqual` with `left > right`, is refuted (`:6650`, `:6680`); an identity of two different closed values likewise (`:6613-6615`). The boolean `Id Two` case checks that the sides are exactly the literals `two_one`/`two_zero` (`:6584-6599`).
   - `compositional_transport` builds one `Equation` per cited equality only if the left side `from` is a kernel `Term::Constant` (`:6294`), else returns `None`. A reflexive or duplicate-source equation is skipped silently (`:6289-6293`).
   - `replacement_context` replaces by handle identity (`original == equation.from`, `:6418`).
5. **Cases by shape.** `replacement_context` handles `Variable`, `Constant/Sort/Two/TwoZero/TwoOne`, `Apply`, `Id`, `Pi`, `Sigma`, `Lambda`, `CaseTwo`; any other term kind returns `Ok(None)` (`:6541`). The coercion engine handles only: `Id` vs `Id` reversal, `Id` over `Two`, `Pi`/`Pi` when neither codomain mentions the bound variable (`:6663`), `Sigma`/`Sigma`, tagged sums with identical motive and domain and `Variable(0)` scrutinee (`:5815-5827`). Everything else `None` (`:5698`). `inhabitants::step` handles `Id`, `<`, `<=`, non-dependent `Sigma`, two-case tagged sums, non-dependent `Pi`; else `Ready(None)` (`:6236`).
6. **Same job elsewhere.** `closed_inhabitant` is called 8 times in the file (`grep -c -F 'closed_inhabitant('`), including `:5599` (coercion) and `:7825` (`Primitive` rule). `inhabitants::step`'s literal-order evidence duplicates `closed_relation_refutation`'s use of `literal_order` and `irreflexive` (read, same file).
7. **Unused/one-caller.** `oriented_evidence(`: 4 matches (definition `:6556`; calls at `:8020` and in the tests). `oriented_coercion` has one call (`:6566`). `closed_inhabitant`: 8 matches (above).
8. **Stated limits.** Budget: `MAX_ELABORATION_NODES` (`:38`, `1 << 16`) is the allowance for `compositional_transport` (`:6304`) and `oriented_evidence` (`:6565`). Test names state that allowance exhaustion cannot refund (`:6950-7000`) and that deep refutation runs on the default stack (`:6876`).
9. **Fail-open.** None seen in the engine: every failure path yields `None` or an error, and the caller falls back to `rule_instance` (see below). Budget exhaustion in `coercion::construct` returns `None` (`:5606`) and `expand_equations` returns `None` when the loop does not settle (`:6364`); neither accepts. Classification: traced.

Tests (`:6750-7001`): a `check` helper that runs `verify_mathematical_certificate` and asserts that `rule_axioms.is_empty()` (derived, not assumed) and that a missing cited equality is rejected. Cases: reversed identity in a disjunction and in a conjunction, an unlicensed reversal inside an implication (rejected), deep (8,192 node) refutation with a true-premise control, and shared allowance across failed alternatives.

## 7003-8727: `mod elaboration` (proof tree to kernel term)

1. **Purpose.** `Elaboration` walks a `ProofNode` tree and produces one kernel term per node, checking each rule's own side conditions with the helper checkers first. It is the bridge from bounded-certificate rules to the mathematical kernel.
2. **Shape.**
   - `Elaboration` (`:7010`): `denotation`, `context`, `ambient_assumptions`, `axioms`, `machine_parameter_values`, `roster: Vec<Proposition>`, `positions: Vec<u32>`, `depth`, `steps`, `rules: BTreeSet<AcceptedProofRule>`, `assumptions`/`semantic_axioms: PremiseRoster` (records which premises were used).
   - `walk` module (`:7035-7662`): explicit-stack driver `drive` (`:7419`) with `Work`/`Pending` enums; `PartialNode::new` (`:7124`) lists children per rule; `Scope` (`:7285`) restores `depth/roster/positions`; `case_term` (`:7621`) builds the disjunction eliminator.
   - `Elaboration::new` (`:7665`), `node` (`:7727`), `finish_regular` (`:7734-8587`, one arm per `ProofRule`), `cited` (`:8589`), `same_denotation` (`:8611`), `ambient_premise` (`:8630`), `cited_axiom` (`:8646`), `rule_instance` (`:8660`), `disjunction_intro` (`:8674`), `finish` (`:8702`, builds `BoundedDenotation` with `MathematicalCertificate`).
3. **Inputs/outputs.** Inputs: `PropositionContext`, goal, assumptions, semantic axioms, machine-parameter value set. Outputs: `BoundedDenotation { arena, certificate, rules, assumptions, semantic_axioms }`. `new` denotes every assumption and axiom as a context binding in order (assumptions first, then axioms, `:7683`).
4. **How it decides.**
   - Each `ProofRule` arm first calls the rule's own checker (`subtract_order::check`, `add_order::check`, `order_discreteness::check`, `integer_order_rules::*`, `affine_bound_relation`, `cast_bound_relation`, ...) and returns the error on failure. Only after this does it try to derive kernel evidence.
   - A citation (`Assumption`, `SemanticAxiom`) is matched by position index in `roster`/`axioms` and by `propositions_match` (`:8597`, `:7835`). If the propositions are not equal it compares their denotations structurally (`same_denotation`, `:8616-8626`).
   - `enter` (`:7401`): step budget `MAX_ELABORATION_NODES`, then `context.validate(&proof.conclusion)` unless `cites_validated_premise` says the node merely cites a roster premise (`:7409`).
   - Variable positions are computed from `depth - 1 - position` (`:7723`).
   - Implication and disjunction branch conclusions are compared with `!=` on `Proposition` values (`:7518`, `:7554`).
5. **Cases by shape.** `PartialNode::new` (`:7124`) fixes children per rule: at most three fixed children, or a slice (`fixed: [Option<&ProofNode>; 3]`, `:7080`). `ConjunctionIntroduction` needs a `Conjunction` conclusion with matching arity (`:7133-7140`). `IntegerOrderDiscreteness` derives only `a <= b ⊢ a < b + 1` and takes the instance route for the other forms (comment `:8146-8149`). `IntegerAffineBound` tries seven derivations in a fixed order (`unsigned_subtract`, `correlated_subtract`, `correlated_add`, `correlated_multiply`, `direct_add`, `direct_subtract`, then instance; `:8343-8412`).
6. **Same job elsewhere.** The `rule_instance` fallback appears in 23 call sites (grep count above), one per non-derived rule. The two `rules.insert(..)` + derive-or-instance patterns are repeated per arm (read).
7. **Unused/one-caller.** `Elaboration::node`: called at `:6795` (a test) and by `denote_bounded_certificate` outside my range (`bounded_denotation.rs:14413` per the crate map; unread). `ambient_premise` has one use (`:8550`).
8. **Stated limits.**
   - `:7740-7743`: "Provisional float rules: the core has no IEEE model to derive them from".
   - `:8573-8574`: "Provisional: the core has no power of two to derive it from, so the checked row is an instance of its statement."
   - `:7712`: repeated evidence keeps one raw syntax node (a cache comment, not a limit).
9. **Fail-open.**
   - **`rule_instance` (`:8660-8672`)** calls `rule_axiom`, which declares `premises -> conclusion` as an assumption in the kernel signature (`:5245-5260`) and applies it to the premise evidence. The kernel then accepts the term because the axiom is in the signature. So the kernel does not itself prove these steps. Which rules take this route:
     - Always: `IntegerFloatExact` (`:7748`), `FloatIntegerConversion` (`:7771`), `FloatMeaningNormalization` (`:7790`), `IeeeFloatOrderChain` (`:7810`), `IntegerPowerOfTwoOrder` (`:8579`).
     - As a fallback when derivation returns `None`: `EqualitySymmetry`, `EqualityTransitivity`, `PredicateDenotation`, `ValueEqualityTransport`, `IntegerAddOrder`, `IntegerSubtractAntitone`, `IntegerOrderDiscreteness` (non-`<=`/`<` forms), `IntegerOrderWeakening`, `IntegerLessOrEqualTransitivity`, `IntegerStrictOrderTransitivity`, `IntegerOrderSubstitution`, `IntegerAffineBound`, the two `ExactDefinitionBound` rules, `IntegerCastBound`, `IntegerCorrelatedForbiddenRoots`.
     - What a wrong proof could get through: a proof whose rule-specific checker (called first) passes but whose kernel derivation is not found. The side condition is checked by the helper checker, not by the kernel. Stated for the float and power-of-two rules (comments above); for the fallbacks no comment at the site says why. Classification: stated for float/power-of-two, traced for the rest (the helper checker runs before the fallback in every arm I read).
     - The accepted rule is added to `rules` (`:8670`), so a consumer can see which rules were used (see `BoundedDenotation.rules`, `:8721`). Whether the consumer restricts it: not read (outside range).
   - `Primitive` (`:7820-7829`): after `decide_primitive` accepts, the denoted conclusion gets `closed_inhabitant` evidence or else a `decision` assumption (`:7828`, a keyed assumption `ty -> ty`). The kernel check does not re-decide the primitive. Classification: traced (the decision is made by `decide_primitive` first, `:7821`).
   - `same_denotation` returns `Ok` when `retained == requested` without denoting (`:8616`). Not a risk: equal values.
   - `ConjunctionElimination` uses `get(*conjunct)` and compares to the conclusion (`:7882-7886`); `conjuncts.len() - 1` at `:7889` is safe because the arm already rejected an out-of-range index and a conjunction needs two members. Traced.
   - `DisjunctionIntroduction` indexes `disjuncts[*index]` at `:7904` after `PartialNode::new` checked `*index < disjuncts.len()` (`:7159`). Traced.

## 8729-9100: start of `mod forbidden_roots` (`chains` submodule, `Law` enum, antisymmetry)

1. **Purpose.** Evidence builders for the correlated-forbidden-roots rule: chains of add/subtract/multiply definitions with literal siblings, plus antisymmetry and add-right-inverse laws. The module is cut at line 9100 (`Law::SubtractRightInverse` statement); the rest is outside this range.
2. **Shape.** `ChainStep` (`:8754`: `definition`, `literal`, `operation`, `next`, `sibling`, `literal_term`, `literal_value: i128`, `expression`, `current`), `ChainOperation` {Add, Subtract, Multiply} (`:8767`), `chain_operation_terms` (`:8774`), `chain_operation_eval` (`:8787`, uses `checked_*` on `i128`), `chain_sibling_congruence` (`:8799`), `chain_operand_congruence` (`:8823`), `chain_steps` (`:8847`), `chain_step_lift` (`:8965`). `Law` (`:8985`, 8 variants, four of them carry an `IntegerType`). `ForbiddenRoots { laws: BTreeMap<Law,u32> }` (`:9006`). `order_antisymmetry_evidence` (`:9018`), `add_right_inverse` (`:9052`), `roots_law_application` (`:9062`, law statement built with `scheme_dsl`).
3. **Inputs/outputs.** `chain_steps` consumes a `CorrelatedAffineBranchWitness` and a cursor into `(Proposition, TermHandle)` evidence; it produces `Vec<ChainStep>` or `None`.
4. **How it decides.** `chain_steps` matches the definition's right-hand `ScalarTerm` variant (`ExactIntegerAdd/Subtract/Multiply`) and requires `scalar_type == integer_type` (`:8873-8895`). Literals: a step with no literal axiom must have a signed integer literal sibling directly (`signed_literal`, `:8918`); with an axiom, `Equal(left, right)` must have `left == sibling` by `ScalarTerm` equality (`:8927`) and a literal `right` (`:8930`). `signed_literal` accepts only `IntegerValue::Signed` of exactly the expected type (`:9010-9015`), so unsigned literals return `None`.
5. **Cases by shape.** Only Add, Subtract, Multiply definitions; unsigned literals unsupported (`:9012`); `i128` literal values.
6. **Same job elsewhere.** `chain_sibling_congruence` and `chain_operand_congruence` are congruence-by-J constructions like `replace_evidence` (`:6367`) and `symmetry`/`transitivity` (`:4904`). `order_antisymmetry_evidence` ends by choosing identity orientation with the same structural compare as `directed_equality` (`:9037-9048`).
7. **Unused/one-caller.** Not counted for this part (the callers are past line 9100). Not grepped.
8. **Stated limits.** None in range.
9. **Fail-open.** None seen. Every mismatch returns `Ok(None)` or `Unsupported(..)` (`:9033`, `:9046`).

## Coverage
- Read in full: `bounded_denotation.rs` lines 3101-9100 (6,000 lines), in four reads (3101-4600, 4601-6100, 6101-7600, 7601-9100).
- Skimmed: none. Not read: none within this gap's range. (Line 3101 begins mid-`impl`; its module header lies in the first reader's range.)
- Greps run: per-name `grep -c -F` counts in `bounded_denotation.rs` listed above, and one grep for `cast_bound_evidence(`/`direct_binary_bound_evidence(` across the `bounded/` directory (callers all in this file).

## Five things a maintainer most needs to know
1. `Elaboration::rule_instance` (`:8660`) is the escape hatch: it declares `premises -> conclusion` as a kernel assumption (`rule_axiom`, `:5240`). Float rules and the power-of-two rule always use it (`:7748-7810`, `:8579`). Almost every other rule uses it when the kernel derivation returns `None`. The rule's own helper checker is the only judge in those cases.
2. Derivation helpers all return `Ok(None)` for "unsupported shape" rather than an error, so a missing derivation never refuses a program; it switches to `rule_instance`. Tests assert `rule_axioms.is_empty()` to pin which proofs are truly derived (`:3401`, `:6796`).
3. Denoting is table-driven and interned: `Denotation` carries about 15 value-keyed maps whose position numbers become kernel declaration indices (`:3821-3862`). Order of first use fixes declaration order. `PositionRoster` papers over `BTreeMap` vs `HashMap` rosters (`:4082`).
4. The three search/transform engines (`logical::denote`, `coercion::construct`, `inhabitants::construct`, `walk::drive`) use explicit stacks rather than recursion; two of them share a counted allowance (`MAX_ELABORATION_NODES = 1<<16`, `:38`) and one test checks that failed alternatives do not refund it (`:6950`).
5. `replacement_context` and `expand_equations` rewrite by handle identity and refuse any term kind outside nine listed variants (`:6541`); equalities whose left side is not a `Constant` make the whole transport decline (`:6294`).

## What I am unsure about
- The first lines of the `cast_bound` module (before 3101) are not in this range, so I could not confirm how `fixed_native_interval` and `carrier_bound` are defined.
- I did not read `denote_bounded_certificate` (`:14413`), so I cannot say whether the `rules` set that `rule_instance` fills is restricted by the consumer.
- The count for `exact_definition_bound_target(` returned 1, which includes only a spelling that matches `fn exact_definition_bound_target<'a>(` only if `<'a>` is absent. It did not match the definition's own line, so the call at `:4230` is the single match; callers in other files were not grepped.
- `integer_id_elim`, `integer_numeral`, `literal_order`, `irreflexive`, `add_terms`, `subtract_terms`, `multiply_terms`, `binary_integer` are called here but defined outside the range; I did not read them.

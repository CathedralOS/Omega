# Unit 042 record: `proofs/bounded` (area 07-psi-semantics)

Root: `source-rust/subsystems/compiler/psi/semantics/proofs/bounded/`. Paths below are relative to it.
INCOMPLETE UNIT: `bounded_denotation.rs` lines 3101-17334 were NOT read (see Coverage). Nothing is claimed about them except
the few facts shown by grep, marked "grepped".

## bounded_denotation.rs lines 1-3100 (read) of 17,334

1. Purpose. Turns a bounded `ProofNode` certificate into a term of the mathematical kernel (`Term`s in a `TermArena`, plus
   `Declaration`s) and re-decides it with `verify_mathematical_certificate` (header comment, :2-9). The file is a set of inline
   `mod`s, one per rule family, all adding methods to one `Denotation` struct. In 1-3100 these are `addition`, `binary_numerals`,
   `booleans`, and the start of `casts`.
2. Shape.
   - Imports and `MAX_ELABORATION_NODES = 1 << 16` (:38). The `Denotation`, `Elaboration`, `BoundedDenotationError` definitions are
     past line 3100 (not read); here only their uses appear.
   - `mod addition` (:40-1265). `enum Law {Monotone, CancelSubtract, MonotoneBoth}` (:62). `struct Addition {operation: Option<u32>,
     laws: BTreeMap<Law,u32>, numeral_sums: BTreeMap<(IntegerValue,IntegerValue,IntegerValue),u32>}` (:69-73). Methods on
     `Denotation`: `add_order_translation` (:76), `add_operation` (:86), `add_terms` (:90), `add_law_application` (:98, builds the law
     statement as a `scheme_dsl` Pi type and interns it through `roster_law_application`), `numeral_sum` (:162),
     `correlated_add_bound_evidence` (:184), `direct_add_bound_evidence` (:333), `exact_add_definition_bound_evidence` (:349),
     `carrier_bound` (:371), `add_bound_endpoint` (:402), `closed_order_bound_evidence` (:496). `Addition::bound_operation` (:575)
     packages add into a `BinaryBoundOperation` shared with subtraction (grepped use: `super::denotation::BinaryBoundOperation`,
     imported :51; its definition not read). Inline tests :589-1264.
   - `mod binary_numerals` (:1266-1873). Encodes integers as binary numerals built from three constructors `Double`, `Odd`,
     `Negate` over `zero` (:1276, :1292-1297), with five law statements `DoubleZero, NegateZero, EvenBeforeOdd, OddBeforeNextEven,
     NegateOrder` (:1283-1289, statements :1402-1440). Entry points: `binary_integer` (:1358), `literal_order` (:1575, proves
     `lower < upper` for two literals by recursion on bits), `discreteness_evidence` (:1628), plus `fixed_magnitude(&BigInt)`
     (:1299). Inline tests :1674-1872.
   - `mod booleans` (:1874-3044). `scalar_denotation::denote` (:1890) turns `ScalarTerm` boolean expressions into kernel terms with
     an explicit stack (`Pending::{Not,EqualLeft,EqualRight}`, :1884-1888). `equality_function` (:1958) builds the two-valued
     equality lambda. `boolean_identity_transport` (:2011) and `boolean_goal_transport` (:2026) build a case-analysis function
     (`Term::CaseTwo`) over neutral Boolean atoms; helpers `boolean_goal_case` (:2050), `boolean_leaf_inhabitant` (:2159),
     `instantiated_goal_case` (:2185), `first_boolean_atom` (:2209), `neutral_atom_is_two` (:2267), `two_identity_discriminant`
     (:2301), `boolean_eval` (:2355), `boolean_function_head` (:2403), `abstract_atom`/`instantiate_atom`/`rewrite_atom`
     (:2457-2550). Enums `BooleanEval {Literal,Blocked,Unsupported}`, `BooleanHead`, `AtomRewrite` (:2553-2571). Inline tests
     :2573-3043.
   - `mod casts` begins at :3045; only :3045-3100 read (`cast_identity_law`, :3063, keyed on `IntegerOperation`).
3. Inputs and outputs. Reads `Proposition`/`ScalarTerm`/`IntegerMathTerm` from `semantic_vocabulary`, the evaluator
   `ClosedIntegerEvaluator` (:209), and the kernel types. Mutates `Denotation` tables (`self.arena`, `self.declarations`,
   interned roster maps such as `addition.laws`, `binary_numerals.constructors`) and returns `TermHandle` evidence or
   `Ok(None)` when the shape is not covered.
4. How it decides.
   - `correlated_add_bound_evidence` is keyed on the witness target being `ScalarTerm::ExactIntegerAdd` with a fixed carrier
     (:192-202), then on structural equality of the stated premise with a recomputed `affine_root_premise` (:251-252), and on
     structural equality of types and operands (`subtract_type != scalar_type || decrement != right`, :252). All mismatches
     return `Ok(None)`, not an error.
   - `add_bound_endpoint` keys on the proposition variant (`Truth`, `Equal`, `LessOrEqual`) and on which side holds the operand
     (`first == operand`, `second == operand`, :433-482). Compared by `ScalarTerm` equality, not by name.
   - `closed_order_bound_evidence` decides by comparing the evaluated closed values (:518). `Ordering::Greater` is accepted only when
     the premise is itself a literal `LessOrEqual` that is false (`left_value <= right_value` returns `None`, :556) and then
     builds an `EmptyElim` from the contradiction (:568).
   - Boolean transport decides by exhaustive case split over atoms: it picks a blocked atom, builds both branches, recurses
     (:2065-2129). With no atom left it needs both sides to evaluate to literals (:2130-2136). `boolean_eval` spends one unit of
     `remaining` per step and returns `Unsupported` at zero (:2357-2359). Neutral detection is by declaration kind
     (`declaration.is_assumption()` plus structural equality of its type with `two`, :2364-2371), i.e. by declaration identity
     and type, not by name.
5. Cases by shape.
   - `correlated_add_bound_evidence` handles only a fixed-width `ExactIntegerAdd` witness whose goal is
     `math_less_or_equal_sides(conclusion, is_math_add)` and whose root is `ExactIntegerSubtract` (:192-249); everything else
     returns `Ok(None)`.
   - `carrier_bound` refuses a width that is a parameter: `Unsupported("a carrier bound of an integer whose width is a
     parameter")` (:382-384).
   - `literal_adjacent` needs a fixed literal: `Unsupported("discreteness needs a fixed literal")` (:1483).
   - `discreteness_evidence` requires premise `LessOrEqual` and goal `LessThan`: `Unsupported("checked discreteness has integer
     order endpoints")` (:1634-1641).
   - `fixed_magnitude` returns `None` above 128 bits (:1300). `closed_order_bound_evidence` maps values outside i128/u128 to `None`
     (:506-517).
   - `boolean_goal_transport` handles only an identity over the two-element type (`structurally_equal(carrier, self.two)`,
     :2035); `rewrite_atom` returns `None` on any term form other than the nine it lists (:2548).
6. Same job elsewhere. The three bound families `direct_*_bound_evidence`, `exact_*_definition_bound_evidence` are written once in
   `addition` and parameterised by `BinaryBoundOperation` (:575-586); the subtraction side is in an unread region (suspected, by
   the `subtract_operation()` call at :145 and `affine_*` helpers imported at :53-56). `closed_order_bound_evidence` and
   `literal_order` both prove closed `<` / `<=` between literals; `float_integer.rs:205-210` does a different, unrelated
   exactness test.
7. Unused or one-caller code. `pub(super)` methods in these modules were not individually grepped because their callers are
   in the unread part of the file. Grepped: `record_denotation` is called once, at `bounded_denotation.rs:14439`; `record_admission`
   once, at `proof.rs:199`; `bounded_verification_entries` only inside tests of `proof.rs` (grep output above, lines 3629-5108).
8. Stated limits. "a carrier bound of an integer whose width is a parameter" (:382). "discreteness needs a fixed literal" (:1483).
   Test names/messages state: "an unrepresentable endpoint sum keeps the explicit instance assumption" (:1184), meaning an
   out-of-range numeral sum falls back to an assumed instance (`rule_axioms.len() == 1`).
9. Fail-open risks.
   - `Ok(None)` returns in the add-bound functions mean "not covered", and callers (past :3100, not read) decide what follows.
     Status: open (callers not read).
   - The fallback at :1181-1185 (test `direct_add_bound_out_of_numeral_range_keeps_instance_fallback`) shows an unrepresentable
     endpoint sum is covered by an explicit assumption in `rule_axioms`. Status: stated (the test message names it); whether the
     verifier then accepts the certificate depends on code not read.
   - `boolean_eval` returns `Unsupported` on budget exhaustion rather than a value (:2357-2359), so exhaustion declines, not
     accepts. Status: traced within the module.

## bounded_observations.rs (15 lines, read)

1. Purpose. A thread-local pair of counters (admission entries, denotation entries), used by tests to confirm that admission
   and denotation each ran once.
2. Shape. `static ENTRIES: Cell<(usize,usize)>` (:1-3); `bounded_verification_entries()` (:5), `record_admission()` (:9),
   `record_denotation()` (:13).
3. IO. Increments on call; returns the tuple.
4. Decisions. None.
7. Callers (Grep over `source-rust`, regex `bounded_verification_entries|record_admission|record_denotation`): `record_admission`
   1 production call (`proof.rs:199`), `record_denotation` 1 production call (`bounded_denotation.rs:14439`),
   `bounded_verification_entries` 0 production calls, 14 uses in `proof.rs` tests (:3629-5108). Counters are always on, not behind
   `cfg(test)` (read: no cfg attribute in the file).
9. Fail-open: none seen.

## classicality.rs (205 lines, read)

1. Purpose. Labels each `AcceptedProofRule` with a foundation (constructive, constructive-decidable, trusted admission, classical),
   says which rules are provisional, and which cite semantic axioms (header :2-7).
2. Shape. `enum ProofRuleFoundation {Constructive, ConstructiveDecidable, TrustedAdmission, Classical}` (:12). On
   `AcceptedProofRule`: `foundation()` (:20), `is_provisional()` (:68), `cites_semantic_axioms()` (:82). Tests with a hand-copied
   `ALL: [AcceptedProofRule; 36]` (:102).
3. IO. Pure `match` on the enum value.
4. Decisions. Keyed on the enum variant (identity of the rule), exhaustive `match` with no wildcard in `foundation()` (:21-58),
   so a new variant breaks compilation. `is_provisional` and `cites_semantic_axioms` use `matches!` with a variant list
   (:69-79, :83-94); a new variant defaults to `false`.
5. Cases by shape. Fixed lists: provisional = 8 rules (:71-78): three remainder/quotient/multiple rules, `IntegerPowerOfTwoOrder`,
   and four float rules. Quote (:61-67): "A provisional row was tested against computed arithmetic and is to be replaced by a
   checked proof; none may remain when the kernel is claimed verified."
6. Same job elsewhere. The test lists at :163-183 and :185-204 repeat the same variant lists as the functions they test, by
   hand; `ALL` has 36 entries and is not generated from the enum (read).
7. Callers (Grep `is_provisional\(|cites_semantic_axioms\(|\.foundation\(\)` over `source-rust`): 14 matches in 8 files, of which
   6 in `classicality.rs`, 4 in float test files, 1 in `proof.rs`, 3 in `tests/*` for remainder/quotient/multiple. `foundation()`
   itself: no caller found outside `classicality.rs` among those matches (the `proof.rs` match was not opened, so which of the
   three functions it calls is not known).
8. Stated limits. The "provisional" quote above (:63-65); only `SemanticAxiom` is `TrustedAdmission` (:23, test :153-160).
9. Fail-open: `is_provisional` returns `false` for any rule not listed, including future ones; the doc comment says "`false` on an
   older row does not say the core derives it" (:66-67). Status: stated.

## float_integer.rs (254 lines, read)

1. Purpose. Checks float-to-integer and integer-to-float exactness claims before the compiler uses a rounded converter (header
   :1-7).
2. Shape. `enum FloatIntegerError {Malformed, Semantic, Transport, Unsupported, NotEstablished}` (:20). Functions:
   `closed_float_integer_relation` (:38, budget 4096 bits), private `closed_with_endpoint_budget` (:44),
   `finite_literal_equality` (:102), `float_integer_equality_subject` (:128), `float_integer_conversion_substitution` (:135),
   `check_float_integer_conversion` (:170), `integer_magnitude_exact` (:205), `check_integer_float_exact` (:216).
3. IO. Reads a `Proposition` and `PropositionContext`; uses `numerics::float_semantics` (`FloatMeaning`,
   `FloatSemanticOperation`) and `semantic_vocabulary`. Returns `Result<Option<bool>>` or `Result<()>`.
4. Decisions.
   - `closed_with_endpoint_budget` decides on the proposition variant: `IeeeFloatIntegerExact` with a literal source;
     `FloatIntegerConversion` with literal source and literal integer result; else `Ok(None)` (:51-99). For a conversion clause it
     looks the operation up by contract identity (`for_contract_identity`, :77) and errors if absent (:78-80), i.e. keyed on
     a catalog identity, not a name.
   - The exact test: finite nonzero must convert to an integer the target admits; zero is exact only if not negative; NaN and
     infinity are not exact (:58-64).
   - `finite_literal_equality` accepts only an IEEE `Equal` whose one side is a finite nonzero literal of the stated format
     and whose other side has that float type (:102-123).
   - `check_float_integer_conversion` has two modes: `computed == None` requires the premise to be `IeeeFloatIntegerExact` and the
     goal to be in `float_integer_preconditions(...)` (:176-196); otherwise the goal substituted by the equality must equal the
     supplied `computed` proposition (:197-202).
   - `check_integer_float_exact`: all values of the carrier fit if `width - signed <= precision && width - 1 <= max_exponent`
     (:236-237); else only a literal source passes, using `integer_magnitude_exact` (:205-210, trailing-zero count test).
5. Cases by shape. Precision/exponent pairs are hard-coded to the two formats: `(24, 127)` for Binary32 and `(53, 1023)` for
   Binary64 (:232-235). Only literal sources get `Some(..)` from `closed_*` (:53-55, :67-72); symbolic sources return `None`.
   Quote (:5-7): "an equality with zero cannot discard a negative zero sign."
6. Same job elsewhere. `meaning()` is defined identically in `float_integer.rs:28-33` and `float_meaning.rs:443-448` (read both;
   two copies). `fixed_integer_type` and `float_integer_preconditions` are in `semantic_vocabulary` (not read).
7. Callers (Grep count over `source-rust`, excluding `tests.rs` files, of the five public names): 21 matches in 8 files:
   `bounded.rs` 3, `bounded_denotation.rs` 2, `float_integer.rs` 6 (self), `kernel.rs` 2, `proof.rs` 2,
   `source-to-psi/.../prove.rs` 2, `prove/float_integer.rs` 3, `reuse.rs` 1. Not broken down by name.
8. Stated limits. "Total conversion graphs retain named converter behavior separately from these obligations" (:3-4);
   "Unknown inputs remain unknown; only semantic conversion failure makes a graph false" (:36-37).
9. Fail-open risks. `closed_*` returns `Ok(None)` ("unknown") for non-literal sources (:53-55, :67-72, :98); stated at :36 ("Unknown inputs
   remain unknown"). Whether a caller treats `None` as acceptance is in the callers (not read): open for callers, not traced.
   `check_integer_float_exact` with a non-literal source on a carrier wider than the format refuses (`false` -> `NotEstablished`,
   :246-249): fail-closed.

## float_integer/tests.rs (604 lines, read)

Tests for `float_integer.rs`. Test list: `conversion_rule_checks_both_children_and_keeps_only_the_cited_equation` (:58),
`exactness_is_stronger_than_the_named_graph_and_resources_never_mean_false` (:156), `conversion_certificates_match_all_independent_miniature_encodings`
(:272, all 32 miniature encodings for both formats with 2-bit targets, expects 4 successes each, :343-346),
`converter_preconditions_need_the_checked_exact_source_and_destination` (:352),
`converter_lower_bound_switches_only_when_minimum_minus_one_rounds_to_minimum` (:406), `integer_float_exact_matches_an_independent_small_integer_model` (:454),
`integer_float_exact_checks_complete_carriers_and_literal_certificates` (:471), `integer_float_exact_uses_checked_integer_equality_and_refuses_wrong_sources` (:562).
Contains its own miniature float oracle (`Mini`, `mini`, `embedded`, :213-269), a second copy of the miniature-format model
that also exists in `float_meaning/model.rs` and `ieee_float_rules/model.rs` (three separate miniature models: 2-bit-exponent
5-bit here :213; 4-bit/3-bit in `float_meaning/model.rs`; 5-bit with NaN payload classes in `ieee_float_rules/model.rs`).
Stated: `accept_certificate` returns an error for a forged child (:94, :124). Fail-open: none seen.

## float_meaning.rs (459 lines, read)

1. Purpose. Decides whether an equation between float meanings follows from a list of cited equations, by congruence closure
   with closed evaluation of known applications and cancellation of exact widening (header :1-5).
2. Shape. `check_float_meaning_normalization(conclusion, premises)` (:38). `struct Normalization` (:102) with dense scratch
   vectors `values, parents, sizes, users, literals, represented, keys, active_keys, pending, queued, work, convert`;
   `enum Value {Projection, Literal, Application}` (:76); `enum Operand {Format, Meaning, Integer}` (:69); methods
   `join` (:176, union by size), `intern` (:238), `import` (:275), `finish` (:347). Limits `VALUE_LIMIT = 4096`,
   `WORK_LIMIT = 65_536` (:23-24). Error enum `FloatMeaningNormalizationError {Malformed, WorkLimit, ValueLimit, Semantic, NotEstablished}` (:27).
3. IO. Reads `FloatMeaningClause` rows from `Proposition::FloatMeaningEqual` only (`equation`, :57-66); uses
   `FloatSemanticOperation::kernel_discharge` for closed values (:419-422). Produces `Ok(())` or an error; no proof object.
4. Decisions.
   - The goal is accepted iff the union-find roots of its two sides are equal after all premises are joined (:50-53).
   - A join across two different formats is refused as `Malformed` (:183-187), keyed on `IeeeFloatFormat`.
   - A conversion application `convert(target, source)` creates the inverse application and joins it with `source` only when
     `source_format.can_exactly_widen_to(target)` and the source is `represented` (a projection or literal) (:369-387). Identified by
     `contract == self.convert`, a contract identity found from the catalog row ("FloatSemantics", "convert") (:120-129).
   - An application whose meaning operands all have known literals and integer operands are closed literals is evaluated and
     joined with the resulting literal (:389-430).
   - Literals are canonicalised through `encode(meaning(v))` so different NaN payloads become one value (:457-459); signed zeros
     stay distinct (tests :245-258 in `float_meaning/tests.rs`).
5. Cases by shape. Only `FloatMeaningEqual` propositions are accepted as premises or goals (:57-66). Operand kinds are fixed to
   format, meaning slot, integer term (:69-73). Integer operands are closed only when `source.integer_value()` is `Some` (:403).
6. Same job elsewhere. `meaning()` duplicated in `float_integer.rs:28`. Work-limit accounting style (`spend`) resembles the `Budget`
   in the kernel (not read).
7. Callers (Grep `check_chain\(|check_float_meaning_normalization\(` over `source-rust`): `check_float_meaning_normalization`
   in `bounded_denotation.rs` (2 matches in total for both names, so probably 1 each; the two names were counted together),
   `proof.rs` 2, `source-to-psi/.../prove/float_meaning.rs` 2, tests 10.
8. Stated limits. "The provisional arithmetic law cancels conversion out of an exact widening; it never cancels arbitrary
   narrowing" (:3-4). Comment at :35-37: "grants no source ownership."
9. Fail-open risks. An unclosed application stays an uninterpreted term (:392-398 `closed = false; break`), which never creates a
   false equality. `finish` evaluates an application whose catalog row is missing as an error, not a skip (:414-418). Exceeding
   `WORK_LIMIT` returns `WorkLimit` (refuses, test `normalization_budget_never_publishes_a_partial_success`, tests.rs:292). None seen.

## float_meaning/model.rs (158 lines, read)

Test-only oracle (`#[path]` included from `float_meaning/tests.rs:9-10`): miniature formats `NARROW` (precision 3, 2 exponent bits,
bias 1, :21) and `WIDE` (precision 4, 3 bits, bias 3, :26). `Format::decode` (:57), `round` (:90, enumerates all destination
magnitudes, minimum distance, tie to even significand, :107-116), `embeds_into` (:120), `roundtrip` (:126), `binary_bits` (:133).
Stated: "uses neither production float semantics nor host float arithmetic" (:4). Shape limit: finite values are multiples of 2^-12
(`assert!(shift >= 0, ...)`, :82-85). No decisions about program input. Fail-open: not applicable.

## float_meaning/tests.rs (490 lines, read)

Tests for `float_meaning.rs`: roundtrip certificate with a forged child (:87), storage versions and IEEE relation separation (:162;
a `Current` field equation does not prove an `Entry` field goal, :181-202), work limit with `WORK_LIMIT` repeated premises (:292), integer
operand carrier identity (:303), literal integer operands (:361), path-length exhaustion (:396, path of `WORK_LIMIT` segments
gives `WorkLimit`), and an exhaustive 32-encoding roundtrip against `model.rs` (:429). Includes `mod model` via `#[path]` (:9).
Fail-open: none seen.

## ieee_float_rules.rs (82 lines, read)

1. Purpose. Checks a chain of positive IEEE order facts `a < b`, `b <= c`, ... against a goal `a <? z` (header :1-4).
2. Shape. `struct Order {format, left, right, strict}` (:16); `order(&Proposition) -> Option<Order>` (:23); `check_chain(conclusion, premises)`
   (:51, `pub(crate)`).
3. IO. Reads `Proposition::ScalarIeeeFloatComparison`; returns `Result<(), ProofError>` using `ProofError::IeeeFloatOrderMismatch`.
4. Decisions. Keyed on structural equality of `ScalarTerm`s at each joint (`first.left != goal.left`, `next.left != last`,
   `last != goal.right`, :62, :69, :75) and on `IeeeFloatFormat` equality; strict goal requires some strict premise (:78). Only
   `LessThan` and `LessOrEqual` kinds count as order (:33-37); `NotLessThan` and the like are not edges.
5. Cases by shape. Any chain length of one or more; an empty chain is refused (:58-61); comment: "an empty chain establishes nothing,
   in particular no reflexive fact about a possibly-NaN value" (:48-50).
6. Same job elsewhere. A chain check for integers exists elsewhere (suspected: `integer_order_rules`, `strict_order_transitivity` imported
   at `bounded_denotation.rs:32-36`; not read).
7. Callers: `check_chain` is counted with `check_float_meaning_normalization` above (2 matches in tests, 2 in `proof.rs`).
8. Stated limits: none beyond the header.
9. Fail-open: none seen; every mismatch is an `Err`.

## ieee_float_rules/model.rs (268 lines, read)

Test-only oracle: all 32 encodings of sign:1/exponent:2/fraction:2 (:1-3), with F32/F64 embeddings (`embed`, :86), a relation table
(`Relations`, :118; `compare`, :127), `Model::triples` yielding exactly 32768 triples per format (:173-185), and `Triple::chains` giving the four
`(<=,<)` compositions (:231-253). A trailing comment block at :256-267 describes intended use ("this scaffold does not assert kernel
results"). Fail-open: not applicable.

## ieee_float_rules/tests.rs (415 lines, read)

Tests: every pair of the miniature model against `decide_primitive(ClosedIeeeFloatRelation)` for six kinds (:53); every triple chain
through `check_chain` (:139); certificate controls with forged assumption indices, disconnected chains, empty and short chains,
a reflexive `a <= a` (refused, :307-317) and a strict goal over non-strict premises (:326-341); NaN complement gives no order edge (:345-362);
a `Current` vs `Entry` storage version cannot join (:377-414). Fail-open: none seen.

## Coverage

- Read in full: `bounded_observations.rs` 1-15 (15); `classicality.rs` 1-205 (205); `float_integer.rs` 1-254 (254);
  `float_integer/tests.rs` 1-604 (604); `float_meaning.rs` 1-459 (459); `float_meaning/model.rs` 1-158 (158);
  `float_meaning/tests.rs` 1-490 (490); `ieee_float_rules.rs` 1-82 (82); `ieee_float_rules/model.rs` 1-268 (268);
  `ieee_float_rules/tests.rs` 1-415 (415); `bounded_denotation.rs` 1-3100 (3,100). Total read in full: 6,050 lines.
- Skimmed: none.
- NOT read: `bounded_denotation.rs` lines 3101-17334 (14,234 lines). The reading tool refused further large reads in this session.
  Everything this record says about that file beyond line 3100 comes only from the greps shown (e.g. `record_denotation` call at :14439).

## Five things a maintainer most needs to know about this unit

1. `bounded_denotation.rs` is one file holding many `mod`s that each add methods to one `Denotation` struct; the rule families I read
   (`addition`, `binary_numerals`, `booleans`) share intern tables (`laws`, `constructors`) keyed by small enums (`bounded_denotation.rs:62`, :1276, :1283).
2. Add, Boolean and numeral evidence functions return `Ok(None)` for shapes they do not cover rather than an error
   (e.g. :198, :239, :2063); what happens next is decided by callers beyond line 3100, which I did not read.
3. Eight proof rules are marked provisional (`classicality.rs:68-80`); the comment says none may remain when the kernel is claimed
   verified. `foundation()` has an exhaustive match; `is_provisional` and `cites_semantic_axioms` default new variants to `false`.
4. The float rules (`float_integer.rs`, `float_meaning.rs`, `ieee_float_rules.rs`) are separate checkers, each returning `Result`, and are
   tested against three separate miniature oracle models (`float_integer/tests.rs:213`, `float_meaning/model.rs`, `ieee_float_rules/model.rs`),
   exhaustively over all 32 encodings (or 32^3 triples).
5. `bounded_observations.rs` counters are unconditional thread-local state (not `cfg(test)`); only `proof.rs` tests read them.

## What you are unsure about

- Everything in `bounded_denotation.rs:3101-17334`, including `Denotation`, `Elaboration`, `BoundedDenotationError`,
  `denote_bounded_certificate` (:14413 per the crate map), `verify_bounded_certificate` (:14456 per the crate map), and the subtraction, multiplication,
  cast, shift and other rule modules.
- Which of `foundation`, `is_provisional`, `cites_semantic_axioms` the one `proof.rs` match calls (match not opened).
- The per-name caller counts for the float-integer functions (counted together, 21 matches in 8 files).
- Whether callers treat `Ok(None)` from the add-bound functions as acceptance or as a fall-through to an assumed instance.

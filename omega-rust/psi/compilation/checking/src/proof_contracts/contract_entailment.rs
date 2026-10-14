//! Requires -> ensures ENTAILMENT for proof machines.
//!
//! An empty-body machine with `requires`/`ensures` contracts is a proof
//! artifact (chapter 10): the claim is that the requirements entail the
//! guarantees. This module judges each ensures fact as PROVEN, REFUTED, or
//! UNKNOWN, and -- when the whole contract lies inside the engine's language --
//! rejects anything it cannot prove, so a false theorem can no longer pass
//! `--check` silently (see this crate's README.md#source-proof-automation).
//!
//! Ordinary checked bodies first use `result_bounds` to collect fixed-integer
//! return bounds across every state. Exact current guards can narrow a returned
//! read, but internal transfers contribute no assumptions. Only result-only
//! goals proved by those complete bounds are discharged; remaining goals retain
//! the inductive judgment and its existing fail-closed stand-down behavior.
//!
//! Ladder rung L7 extends the same judgment to INDUCTIVE theorems: a machine
//! whose body is a chain of guarded value/tail-recursion transitions (the
//! shape `transition n > 0 { true -> self.f(n - 1, ...) false -> base }`).
//! Each transition arm is one proof obligation: the ensures with `result`
//! bound to the arm's value, under the requires plus that arm's guard
//! polarity. On a tail SELF-call arm the engine may assume the machine's own
//! ensures for the call's arguments -- the INDUCTION HYPOTHESIS -- but only
//! after discharging a strict decrease of the declared `decreases` measure at
//! that exact call site (measure strictly smaller AND still non-negative
//! under the arm's facts), which is the well-foundedness that makes the
//! induction sound. No decreases clause, or an undischarged one, means no
//! hypothesis. See `inductive_transition_entailment` below.
//!
//! ## The engine's language
//!
//! Terms: integer literals, the machine's own parameters, `+ - *` over those,
//! `t % k` with a positive constant `k` (with the euclidean range lemma), and
//! exact compiler-owned integer embeddings. Ordinary source-defined proof
//! views use selected declarations and structural proof, not name-based
//! arithmetic atoms. Facts: comparisons (`== != < <= > >=`) and range membership
//! (`t in lo..=hi`) over such terms. Anything else (domain membership,
//! unknown calls, non-parameter places) is OUTSIDE the language: the engine
//! still tries to prove with what it can see (extra unknown hypotheses can
//! only help, never hurt soundness of a proof), but it never REJECTS a
//! contract it cannot fully read.
//!
//! ## Mechanics
//!
//! 1. Terms normalize to canonical POLYNOMIALS over atoms (sum of monomials
//!    with i64 coefficients, atoms = parameter names / integer embeddings /
//!    mod terms). Polynomial identity proves L0 constants, L3/L4 congruence
//!    and commutativity, and distributivity with no hypotheses at all.
//! 2. `requires` equations whose one side is a lone atom become directed
//!    SUBSTITUTIONS (occurs-checked, applied to fixpoint), so `a == b` lets
//!    `a + 1 == b + 1` normalize to `0 == 0`.
//! 3. Order and range facts become LOWER BOUNDS on difference polynomials
//!    (integer semantics: strict `<` is slack 1). Single-atom bounds and
//!    atom-minus-atom bounds feed a difference-bound matrix over the atoms
//!    plus a virtual ZERO atom, closed transitively (Floyd-Warshall), which
//!    proves L1 transitivity and L6 antisymmetry. A positive self-cycle means
//!    the requires set is UNSATISFIABLE: every ensures is vacuously true.
//! 4. Remaining goals go to an INTERVAL evaluator: atom intervals come from
//!    the closed matrix (plus `unsigned >= 0` and the mod lemma), monomials
//!    multiply intervals with CORRELATED powers (`a * a` squares one
//!    interval; it never treats the factors as independent), which proves L2
//!    range sums and L5 square ranges.
//! 5. REFUTATION is proving the goal's negation with the same machinery.
//!    Refutations gate on satisfiability of the visible requires set, exactly
//!    like the vacuity rule above.
//!
//! This file validates one machine contract entailment.
//! `strict_arithmetic.rs` judges strict arithmetic implications,
//! `transparent_applications.rs` entails transparent proposition
//! applications, `fact_mentions.rs` detects facts naming proof-only data,
//! `citations.rs` collects, suggests and instantiates citations,
//! `proof_edges.rs` judges strict decrease along proof edges,
//! `refuted_requires.rs` rejects refuted value-call requirements,
//! `specification_calls.rs` requires positive precondition evidence before
//! concrete machine/state contract terms form,
//! `structural_case_arms.rs` recognizes guarded structural case arms and
//! `self_induction.rs` intakes self-induction hypotheses.

mod arithmetic_judgment;
mod call_requirements;
mod citations;
mod completed_values;
mod const_ranges;
mod content_conservation;
mod contract_refinement;
mod exit_coverage;
mod fact_mentions;
mod inductive_judgment;
mod law_conformance;
mod observed_projections;
mod proof_edges;
mod proof_integer;
mod quotient_congruence;
mod ranking_range;
mod refuted_requires;
mod result_bounds;
mod scoped_arithmetic;
mod self_induction;
pub(crate) mod specification_calls;
mod static_applications;
mod strict_arithmetic;
mod structural_case_arms;
pub(crate) mod structural_judgment;
mod structural_prefix;
mod structural_terms;
mod transparent_applications;

pub use arithmetic_judgment::integer_embedding_sources_equal;
pub use call_requirements::{
    selected_value_call_requirement_entailed, structural_call_requirement_entailed,
};
pub(crate) use const_ranges::{
    const_range_bound_is_admitted, selected_const_call_result_bounds, symbolic_range_contains,
    validate_const_range_call, validate_const_range_call_in_environment,
};
pub(crate) use contract_refinement::check_contract_refinement;
pub(crate) use exit_coverage::entailment_covers_all_exits;
pub use law_conformance::{
    InheritedRequirementApplication, inherited_requirement_proposition_application,
    inherited_requirement_proposition_label, inherited_satisfier_parameters,
};
pub use law_conformance::{MatchedLawGuarantee, matched_machine_law_guarantees};
pub(crate) use law_conformance::{
    check_law_conformance, check_operator_contract_conformance, checked_operator_contract_snapshot,
};
pub(crate) use proof_edges::proof_edge_strict_decrease_judged;
pub use proof_integer::{anonymous_integer_math_literal, proof_integer_expression};
pub(crate) use proof_integer::{
    proof_integer_nonnegative, proof_nat_cast, validate_proof_fact_integer_casts,
};
pub use ranking_range::{
    ArithmeticCallObservation, RankArrivalCompatibility, RankArrivalCorrespondence,
    RankArrivalDemand, RankCoordinateBinding, RankCoordinateTransport, RankLocalCapture,
    RankPremiseCoordinate, RankPremiseInput, RankPremiseObservation, RankProtectedPath,
    RankReceivingCoordinate, RankStateBindings, RankSubjectCoordinate, RankValueDerivation,
    RankingRangeEdgeProof, RankingRangeMeasure, RankingRangePremises, RankingRangeState,
    StateEntryCorrespondence, arithmetic_entry_requirement_is_covered,
    discover_rank_arrival_correspondence, frame_preserves_rank_premises, protect_rank_premise,
    prove_arithmetic_call_requirement, prove_captured_arithmetic_call_requirement,
    prove_ranking_range_edge, prove_ranking_range_entry, prove_ranking_range_transition,
    rank_protected_path, ranking_coordinate_type, ranking_range_arrival_preserves_roles,
    ranking_range_premise_symbols, ranking_range_required_coordinates,
    ranking_range_required_symbols, ranking_range_state_premise_coordinates,
    ranking_slice_length_observation, select_rank_arrival_correspondence,
};
pub use ranking_range::{
    ComputationBodyShape, DeclaredIdentityView, DeclaredScalarView, MeasureBodyShape,
    ProjectionStep, ScalarViewComputation, computation_body_shape, declared_scalar_view,
    find_declared_measure, identity_subject_matches, measure_body_shape,
    measure_constraints_cover_subject, unique_literal_field,
};
pub(crate) use ranking_range::{
    RankingRangeCallEdge, RankingRangeCallMember, RankingRangeCallProgress, RankingRangeCallSite,
    call_member_premise_symbols, mixed_call_endpoints_are_pinned, prove_ranking_range_call,
};
pub use ranking_range::{
    captured_collection_length, captured_integer_value, closed_collection_length,
    closed_collection_length_at_place,
};
pub use refuted_requires::instantiated_value_call_requires_proven;
pub(crate) use refuted_requires::reject_refuted_value_call_requires;
pub(crate) use scoped_arithmetic::scoped_arithmetic_implication_for_declaration;
pub use scoped_arithmetic::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticComparison,
    ScopedArithmeticExpression, ScopedArithmeticHypothesis, ScopedArithmeticValue,
    scoped_arithmetic_comparisons_unsatisfiable, scoped_arithmetic_implication,
    scoped_arithmetic_values_relation,
};
pub use specification_calls::call_contract_parameter_position;
pub(crate) use specification_calls::{
    SelectedCallContract, selected_call_contract, validate_specification_call_requirements,
};
pub use static_applications::static_application_where_judgment;
pub use strict_arithmetic::{
    StrictArithmeticBindingValue, StrictArithmeticExpressionBinding,
    StrictArithmeticImplicationJudgment, StrictArithmeticSymbolBinding,
    strict_arithmetic_expression_implication,
};
pub(crate) use structural_judgment::declared_index_algebra_laws;
pub use transparent_applications::{
    InheritedRequirementEdge, transparent_call_requirement_entailed,
    transparent_proposition_application_entailed,
};

use std::collections::BTreeMap;

use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContractKind, StateSignature};
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::trait_definition::TraitDefinition;

use crate::proof_contracts::contract_entailment::citations::{
    collect_citation_equations, instantiated_fact_established, is_citation_statement,
    suggest_missing_citation,
};
use crate::proof_contracts::contract_entailment::fact_mentions::{
    fact_mentions_proof_only_data, fact_mentions_zero_value,
};
use crate::proof_contracts::contract_entailment::structural_case_arms::{
    StructuralCaseArm, recognize_guarded_structural_value_arms, recognize_structural_case_arms,
};
use arithmetic_judgment::{Engine, Judgment, Polynomial};
use inductive_judgment::inductive_transition_entailment;
use quotient_congruence::{quotient_equality_from_requires, quotient_equality_names};
use structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use structural_terms::{
    structural_call_machine_name, structural_term, structural_term_pair, term_contains,
    unfold_constant_applications,
};

/// Whether `OMEGA_STRUCT_TRACE` asks for structural-judgment tracing. Read
/// once: each environment lookup takes the process environment lock, and
/// body unfolding asks once per application.
fn struct_trace() -> bool {
    static TRACE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *TRACE.get_or_init(|| std::env::var_os("OMEGA_STRUCT_TRACE").is_some())
}

/// The reserved binder naming a machine's return value inside `ensures`
/// facts. Matches the call-site substitution rule in the checked-trees
/// contract prover: a single-segment `result` that does not shadow a real
/// parameter denotes the produced value.
const RESULT_BINDER: &str = "result";

/// Arm-pattern exhaustiveness markers (`__arm_destructure#...` locals) are
/// VALIDATION carriers minted by the transition parser, not body shape:
/// every proof-side statement-shape walk steps over them, the same way
/// citation statements are stepped over.
pub fn is_arm_pattern_marker(statement: &StatementNode) -> bool {
    matches!(
        statement,
        StatementNode::LocalData(local)
            if local.role == language_core::LocalBindingRole::ArmPattern
    )
}

pub(crate) fn validate_machine_contract_entailment<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    diagnostics: &mut Vec<Diagnostic>,
    proven: &mut Vec<ExpressionHandle>,
) {
    // A top-level requirement publishes the contract that its selected
    // satisfier must refine. The declaration has no implementation body whose
    // exits could prove those guarantees.
    if machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement {
        return;
    }
    let mut requires = Vec::new();
    let mut requires_propositions = Vec::new();
    let mut ensures = Vec::new();
    // Membership facts (`value in Domain`) are outside the engine's language.
    // The empty-body path drops them silently (a dropped hypothesis only
    // weakens proving power); the inductive path additionally refuses to
    // REJECT when any are present, since the unread fact could entail the
    // goal.
    let mut all_facts_are_expressions = true;
    for contract in program.machine_contracts(machine) {
        let bucket = match &contract.kind {
            SignatureContractKind::Requires => &mut requires,
            SignatureContractKind::Ensures => &mut ensures,
            SignatureContractKind::EnsuresForResultCase { .. }
            | SignatureContractKind::Crashes { .. } => continue,
        };
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match fact {
                ProofFact::Expression(expression) => {
                    bucket.push(*expression);
                }
                ProofFact::Proposition(application) => {
                    if matches!(contract.kind, SignatureContractKind::Requires) {
                        requires_propositions.push(application);
                    }
                    all_facts_are_expressions = false;
                }
                ProofFact::Membership(_) => {
                    all_facts_are_expressions = false;
                }
            }
        }
    }
    if ensures.is_empty() {
        return;
    }
    // Whether ANY requires fact exists (expression or membership) -- the
    // inductive-hypothesis guard: a requires-bearing machine's ensures is
    // CONDITIONAL, so it must not self-cite as an unconditional IH.
    let machine_has_requires = program.machine_contracts(machine).iter().any(|contract| {
        matches!(contract.kind, SignatureContractKind::Requires)
            && !program.proof_facts.span_or_empty(contract.facts).is_empty()
    });

    // STRUCTURAL claims -- ensures conjuncts whose operands mention
    // PROOF-ONLY data (`result == Nat::Zero`, `add(a, b) == add(b, a)`) --
    // have no judging tier yet: the polynomial engine's language is
    // integers, so such a conjunct would stand down as out-of-language and
    // silently CERTIFY an unproven (possibly false) mathematical claim.
    // Refuse loudly until the extraction/rearrange tier (math roster N3)
    // lands; probed 2026-07-11 with a false `result == Nat::Zero` that
    // compiled clean before this fence.
    let proof_only = crate::proof_only_classification(program);
    let mut fenced_structural = false;
    let mut any_structural = false;
    // N3 rung 1: a tiny STRUCTURAL judge for the conjuncts the fence would
    // otherwise refuse -- variable substitution from requires equalities
    // (symmetry/transitivity fall out) plus nullary-constructor comparison
    // (reflexivity proves, distinct cases refute). Contradictory structural
    // hypotheses accept everything (absurd), mirroring the polynomial
    // engine's vacuity rule. Payload-carrying constructor TERMS in facts are
    // grammar-gated today (struct literals do not parse in contract
    // position), so injectivity decomposition is the recorded next rung.
    let mut structural = StructuralJudge::from_requires(program, machine, &requires, true);
    if let Some(entry) = program.machine_states(machine).first() {
        transparent_applications::intake_transparent_requires(
            program,
            machine,
            entry,
            program.machine_contracts(machine),
            &mut structural,
        );
    }
    // CITATIONS (ch10 "Citing Proofs"; the settled proof-citation rule):
    // each statement call to a free proof machine carries the callee's
    // proven ensures to this proof, instantiated at the call's argument
    // terms -- fact injection, the explicit default. Nothing is global:
    // the call IS the use declaration.
    // Citations discharge their callee's requires against facts available at
    // that exact statement. Earlier citations feed later ones, preserving the
    // authored proof order without making any lemma global or implicit.
    let citation_equations = {
        let judge_for_discharge = &structural;
        collect_citation_equations(
            program,
            &proof_only,
            machine,
            diagnostics,
            Some(judge_for_discharge),
        )
    };
    for (left, right) in citation_equations {
        structural.intake_equation(left, right, 0);
    }
    let structural = structural;
    // A bodied lemma whose single state carries EXACTLY ONE unguarded value
    // arm (`transition { _ -> (b) }`) binds `result` to that arm's term --
    // the arm always fires, so the binding is total and a ground refutation
    // under it is a real disproof. Anything wider (guarded arms, multiple
    // arms with first-match reachability, tail self-calls) judges without
    // the binding, which can only weaken toward Unknown -- never unsound.
    // The identity lemma `-> (b)` with `ensures result == b` proves here.
    // Recognized shape: leading `let` locals (the terminal auto-hoist
    // rewrites `-> (call(..))` into `let __hoist = call(..); -> (__hoist)`)
    // folding into an environment over the lemma's own params, then exactly
    // one Always value arm. Params map to themselves (they are the fact's
    // vocabulary); locals map to their initializer terms.
    let sole_arm_value = |judge: StructuralJudge<'program>| {
        let [root] = program.machine_states(machine) else {
            return None;
        };
        let environment: Vec<(String, StructuralTerm)> = program
            .state_parameters(root)
            .iter()
            .map(|parameter| {
                let name = parameter.name.as_str().to_owned();
                (name.clone(), StructuralTerm::Variable(name))
            })
            .collect();
        let mut frames = vec![(environment, judge)];
        let mut results = None;
        for statement in program.statement_table.statements(root.statement_nodes) {
            if results.is_some() {
                return None;
            }
            if is_arm_pattern_marker(statement) {
                continue;
            }
            match statement {
                StatementNode::Call(call)
                    if is_citation_statement(program, &proof_only, machine, call)
                        || content_conservation::is_empty_body_statement_call(program, call) => {}
                StatementNode::LocalData(local) => {
                    let mut next = Vec::new();
                    for (environment, judge) in frames {
                        for outcome in judge.normal_return_paths(
                            local.initial_value,
                            &environment,
                            (machine, root, local.type_reference),
                        )? {
                            let mut environment = environment.clone();
                            environment.push((local.name.as_str().to_owned(), outcome.value));
                            next.push((environment, outcome.judge));
                        }
                    }
                    frames = next;
                }
                StatementNode::Expression(value) => {
                    let mut values = Vec::new();
                    for (environment, judge) in frames.drain(..) {
                        values.extend(judge.normal_return_paths(
                            *value,
                            &environment,
                            (machine, root, root.return_type),
                        )?);
                    }
                    results = Some(values);
                }
                StatementNode::Transition(transition) => {
                    if !matches!(transition.guard, TransitionGuardNode::Always)
                        || transition.continuation.is_valid()
                    {
                        return None;
                    }
                    let TransitionTargetNode::Value(value) =
                        program.statement_table.transition_target(transition.target)
                    else {
                        return None;
                    };
                    let mut values = Vec::new();
                    for (environment, judge) in frames.drain(..) {
                        values.extend(judge.normal_return_paths(
                            *value,
                            &environment,
                            (machine, root, root.return_type),
                        )?);
                    }
                    results = Some(values);
                }
                _ => return None,
            }
        }
        results
    };
    let sole_arm_result = sole_arm_value(structural.clone());
    // Result-to-parameter tag forwarding needs the actual returned subject,
    // before legacy unfolding can erase its origin. Reuse the same extraction
    // with the restricted subject resolver; a tag remains no field equation.
    let case_structural = StructuralJudge::from_case_requires(program, machine, &requires);
    let sole_arm_case_result = sole_arm_value(case_structural.clone());
    if struct_trace() {
        eprintln!(
            "STRUCT machine={} sole_arm={:?}",
            machine.name,
            sole_arm_result
                .as_ref()
                .map(|paths| paths.iter().map(|path| &path.value).collect::<Vec<_>>())
        );
    }
    // STRUCTURAL INDUCTION (the L7 protocol's structural twin): a bodied
    // machine whose single state is a chain of case arms over one matched
    // parameter judges the ensures PER ARM -- the arm's case hypothesis
    // substitutes the subject with a constructor over FRESH payload
    // variables, `result` binds to the arm's value term, and every
    // self-application in that term assumes the machine's own ensures for
    // its arguments (the inductive hypothesis; sound because
    // validate_proof_machine_recursion refuses non-descending self-calls in
    // this same diagnostics batch, so an unsound assumption never certifies
    // a program that compiles). Case arms of inhabited proof data are
    // reachable, so a ground refutation on any arm refutes the claim.
    // Pre-term-ified ensures equalities, the raw material of the inductive
    // hypothesis instantiation (only top-level `==` conjuncts serve as IH).
    let ensures_terms: Vec<Option<(StructuralTerm, StructuralTerm)>> = ensures
        .iter()
        .map(|fact| {
            let Some(binary) = program.expression_table.binary_node(*fact) else {
                return None;
            };
            let entry = program.machine_states(machine).first()?;
            let parameters = program.state_parameters(entry);
            if binary.operator != BinaryOperator::Equal
                || !structural_terms::has_identity_equality_meaning(
                    program,
                    machine.symbol,
                    parameters,
                    *fact,
                )
            {
                return None;
            }
            structural_terms::structural_term_pair_for_parameters(
                program,
                machine.symbol,
                parameters,
                binary,
            )
        })
        .collect();
    let case_arms: Option<Vec<StructuralCaseArm>> = if sole_arm_result.is_some() {
        None
    } else {
        recognize_structural_case_arms(
            program,
            machine,
            &structural,
            &proof_only,
            diagnostics,
            None,
        )
        .or_else(|| recognize_guarded_structural_value_arms(program, machine, &structural))
    };
    // This legacy term vocabulary is name-based. An authored entry parameter
    // takes precedence over the reserved result spelling in every conjunct,
    // so no returned-value alias may replace that parameter's hypotheses.
    let result_is_parameter = program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| {
            program
                .state_parameters(entry)
                .iter()
                .any(|parameter| parameter.name.as_str() == RESULT_BINDER)
        });
    // An authored content-conservation theorem is judged over the sole-arm
    // normal-return value; its projection plans are read once per machine.
    let content_plans =
        crate::value_custody::content_conservation::content_conservation_plans(program);
    let projection_plans = if ensures.iter().any(|fact| {
        content_conservation::content_conservation_plan(machine, *fact, &content_plans).is_some()
    }) {
        crate::value_custody::content_projections::build_content_projection_plans(program)
    } else {
        Vec::new()
    };
    let judge_structural = |fact: ExpressionHandle| -> StructuralJudgment {
        if let Some(plan) =
            content_conservation::content_conservation_plan(machine, fact, &content_plans)
        {
            let Some(paths) = &sole_arm_result else {
                return StructuralJudgment::Unknown;
            };
            let mut verdict = StructuralJudgment::Proven;
            for path in paths {
                match content_conservation::judge_content_conservation(
                    &path.judge,
                    &path.value,
                    plan,
                    &projection_plans,
                ) {
                    StructuralJudgment::Proven => {}
                    StructuralJudgment::Refuted => return StructuralJudgment::Refuted,
                    StructuralJudgment::Unknown => verdict = StructuralJudgment::Unknown,
                }
            }
            return verdict;
        }
        if structural_terms::is_case_observation(program, fact)
            && let Some(paths) = &sole_arm_case_result
            && let ExpressionNode::Binary(comparison) = program.expression_table.expression(fact)
        {
            // A parameter named `result` is not the reserved returned value.
            // Rejoin the authored occurrence before installing a textual alias,
            // including before the legacy value-normalizer fallback below.
            if !crate::reserved_result_place(program, comparison.left)
                .is_some_and(|place| place.machine_symbol == machine.symbol)
            {
                return structural.judge(program, fact);
            }
            if paths.iter().all(|path| {
                let mut bound = path.judge.clone();
                bound.intake_case_equation(
                    StructuralTerm::Variable(RESULT_BINDER.to_owned()),
                    path.value.clone(),
                    0,
                );
                matches!(bound.judge(program, fact), StructuralJudgment::Proven)
            }) {
                return StructuralJudgment::Proven;
            }
        }
        if let Some(proven) = quotient_equality_from_requires(
            program,
            machine,
            &requires,
            &requires_propositions,
            fact,
        ) {
            return if proven {
                StructuralJudgment::Proven
            } else {
                StructuralJudgment::Unknown
            };
        }
        if let Some(paths) = &sole_arm_result {
            let mut verdict = StructuralJudgment::Proven;
            for path in paths {
                let mut bound = path.judge.clone();
                if !result_is_parameter {
                    std::sync::Arc::make_mut(&mut bound.substitutions)
                        .insert(0, (RESULT_BINDER.to_owned(), path.value.clone()));
                }
                match bound.judge(program, fact) {
                    StructuralJudgment::Refuted => return StructuralJudgment::Refuted,
                    StructuralJudgment::Unknown => verdict = StructuralJudgment::Unknown,
                    StructuralJudgment::Proven => {}
                }
            }
            return verdict;
        }
        let Some(arms) = &case_arms else {
            return structural.judge(program, fact);
        };
        let mut verdict = StructuralJudgment::Proven;
        for arm in arms {
            let mut bound = structural.clone();
            for (subject_term, constructor) in &arm.case_equations {
                // Computed-subject refinements are equations.
                bound.intake_equation(subject_term.clone(), constructor.clone(), 0);
            }
            for (subject, constructor) in &arm.case_hypotheses {
                std::sync::Arc::make_mut(&mut bound.substitutions)
                    .insert(0, (subject.clone(), constructor.clone()));
            }
            if machine_has_requires
                && (!arm.case_equations.is_empty() || !arm.case_hypotheses.is_empty())
            {
                // REQUIRES-BEARING INDUCTION: re-intake the requires after
                // every refinement on the path. Nested case splits may expose
                // the payload-level premise only at the final leaf.
                //
                // VACUOUS LEAF: judge refutation before intake, because
                // intaking the premise itself could mask a constructor clash.
                if requires
                    .iter()
                    .any(|fact| matches!(bound.judge(program, *fact), StructuralJudgment::Refuted))
                {
                    continue;
                }
                for fact in &requires {
                    bound.intake(program, *fact);
                }
                if bound.hypotheses_contradictory {
                    continue;
                }
            }
            // Per-arm citations (N3 rung 2): the arm's sub-state facts,
            // already instantiated under this arm's environment.
            for (left, right) in arm.citations.iter().chain(&arm.normal_guarantees) {
                bound.intake_equation(left.clone(), right.clone(), 0);
            }
            // Inductive hypotheses: instantiate every ensures conjunct for
            // each self-application in the arm's value term. For a
            // REQUIRES-bearing machine the IH is CONDITIONAL: its requires,
            // instantiated at the self-call's operands, must judge PROVEN
            // against the arm's hypotheses before the ensures intakes --
            // otherwise that application contributes no IH (over-refusal
            // safe). Membership requires are outside the judge's language:
            // no IH at all (`all_facts_are_expressions` guards).
            let mut applications = Vec::new();
            if !machine_has_requires || all_facts_are_expressions {
                StructuralJudge::self_applications(
                    &arm.value,
                    program
                        .machine_states(machine)
                        .first()
                        .map_or(SymbolHandle::invalid(), |state| state.symbol),
                    &mut applications,
                );
            }
            for application in applications {
                let StructuralTerm::Application { arguments, .. } = application else {
                    continue;
                };
                let mut map: Vec<(String, StructuralTerm)> = arm
                    .parameter_names
                    .iter()
                    .cloned()
                    .zip(arguments.iter().cloned())
                    .collect();
                if machine_has_requires
                    && !requires.iter().all(|fact| {
                        program
                            .machine_states(machine)
                            .first()
                            .is_some_and(|entry| {
                                instantiated_fact_established(
                                    program,
                                    &bound,
                                    machine.symbol,
                                    program.state_parameters(entry),
                                    *fact,
                                    &map,
                                )
                            })
                    })
                {
                    continue;
                }
                map.push((RESULT_BINDER.to_owned(), application.clone()));
                for conjunct in &ensures_terms {
                    let Some((left, right)) = conjunct else {
                        continue;
                    };
                    bound.intake_equation(
                        StructuralJudge::substitute_term(left, &map),
                        StructuralJudge::substitute_term(right, &map),
                        0,
                    );
                }
            }
            if !result_is_parameter {
                std::sync::Arc::make_mut(&mut bound.substitutions)
                    .insert(0, (RESULT_BINDER.to_owned(), arm.value.clone()));
            }
            match bound.judge(program, fact) {
                StructuralJudgment::Proven => {}
                StructuralJudgment::Refuted => return StructuralJudgment::Refuted,
                StructuralJudgment::Unknown => verdict = StructuralJudgment::Unknown,
            }
        }
        verdict
    };
    let mut ensures: Vec<ExpressionHandle> = ensures
        .into_iter()
        .filter(|fact| {
            let zero_value_mention = fact_mentions_zero_value(program, *fact);
            let proof_only_mention =
                fact_mentions_proof_only_data(program, &proof_only, machine, *fact);
            let mention = zero_value_mention.clone().or_else(|| {
                proof_only_mention
                    .as_ref()
                    .map(|name| name.as_str().to_owned())
            });
            if struct_trace() {
                eprintln!(
                    "ROUTE machine={} fact=`{}` mention={:?} zero_value={}",
                    machine.name,
                    program.expression_table.display_name(*fact),
                    mention.as_deref(),
                    zero_value_mention.is_some(),
                );
            }
            // Hypothesis discharge: an ensures fact spelled verbatim in the
            // machine's own requires contract follows by assumption alone.
            // Law satisfiers on carriers outside the engine's language rely
            // on it — their declared law equation is the conformance's
            // evidence, carried forward as a caller obligation. The discharge
            // only publishes hypotheses spelled over declared call targets:
            // an undeclared view name has no selected declaration to supply
            // a proof term.
            if requires.iter().any(|required| {
                program
                    .expression_table
                    .expressions_structurally_equal(*required, *fact)
            }) && fact_call_targets(program, *fact).all_declared
            {
                proven.push(*fact);
                return false;
            }
            let Some(held) = mention else {
                // Selected normal-return guarantees and ordinary value
                // equations use the same prepared path judgment as proof-only
                // carriers. A projection shape does not decide which facts
                // may consume that evidence. Only a proved fact is discharged;
                // other judgments retain the general engine's existing route.
                if matches!(judge_structural(*fact), StructuralJudgment::Proven) {
                    proven.push(*fact);
                    return false;
                }
                return true;
            };
            any_structural = true;
            if structural.hypotheses_contradictory {
                proven.push(*fact);
                return false;
            }
            // CH10 ACCEPTED tier (GR6d): a bodyless boundary machine's
            // ensures is an AXIOM -- believed under the grant-locality rule
            // (own-package dev-active; the trust report carries the row),
            // never proven. The ENGINE VETO still applies: a statement the
            // judge can REFUTE is a compile error, grants notwithstanding.
            if machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim
                && zero_value_mention.is_none()
            {
                if matches!(judge_structural(*fact), StructuralJudgment::Refuted) {
                    diagnostics.push(Diagnostic::error(format!(
                        "accepted boundary machine `{}` claims `{}`, which the \
                         engine REFUTES structurally -- a refutable statement is a \
                         compile error, grants notwithstanding (chapter 10 engine \
                         veto)",
                        machine.name,
                        program.expression_table.display_name(*fact),
                    )));
                    fenced_structural = true;
                }
                return false;
            }
            match judge_structural(*fact) {
                StructuralJudgment::Proven => proven.push(*fact),
                StructuralJudgment::Refuted => {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` ensures contract proof fact `{}` is disproved \
                         structurally: under the requires hypotheses the sides resolve \
                         to constructor forms that contradict the claim",
                        machine.name,
                        program.expression_table.display_name(*fact),
                    )));
                    fenced_structural = true;
                }
                StructuralJudgment::Unknown => {
                    // The settled ergonomics mitigation: when a known
                    // lemma's ensures shape-matches the fenced goal, the
                    // diagnostic names the missing citation. Suggestion at
                    // failure, never silent application.
                    let suggestion = suggest_missing_citation(program, &proof_only, machine, *fact)
                        .map(|note| format!("; {note}"))
                        .unwrap_or_default();
                    if let Some((quotient, relation)) = quotient_equality_names(program, *fact) {
                        diagnostics.push(Diagnostic::error(format!(
                            "machine `{}` cannot prove quotient equality `{}`: add the \
                             corresponding `{relation}(left_carrier, right_carrier)` requires \
                             fact for quotient `{quotient}`",
                            machine.name,
                            program.expression_table.display_name(*fact),
                        )));
                    } else if zero_value_mention.is_some() {
                        diagnostics.push(Diagnostic::error(format!(
                            "machine `{}` cannot establish zero-value representation obligation \
                             `{}` for `{held}`: all-zero storage is gated, has no payload-free \
                             authored home case, or is not a cased data representation",
                            machine.name,
                            program.expression_table.display_name(*fact),
                        )));
                    } else {
                        diagnostics.push(Diagnostic::error(format!(
                            "machine `{}` ensures contract proof fact `{}` speaks about proof-only \
                             `{held}`, which no entailment tier judges yet -- accepting it would \
                             certify an unproven structural claim. Spell the fact over integer \
                             measures, or wait for the structural extraction tier (math roster \
                             N3){suggestion}",
                            machine.name,
                            program.expression_table.display_name(*fact),
                        )));
                    }
                    fenced_structural = true;
                }
            }
            // Structural conjuncts never reach the polynomial engine: judged
            // here (proven/refuted/fenced), they have no integer reading.
            false
        })
        .collect();
    // Structural REQUIRES are hypotheses the polynomial engine cannot read:
    // mark the contract not-fully-visible so it stands down instead of
    // rejecting integer goals it cannot prove without them.
    if any_structural
        || requires.iter().any(|fact| {
            fact_mentions_zero_value(program, *fact).is_some()
                || fact_mentions_proof_only_data(program, &proof_only, machine, *fact).is_some()
        })
    {
        all_facts_are_expressions = false;
    }
    if fenced_structural || ensures.is_empty() {
        return;
    }

    let body_is_empty = program.machine_states(machine).iter().all(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .is_empty()
    });
    if !body_is_empty {
        if let Some(bounds) = result_bounds::infer(program, machine) {
            ensures.retain(|fact| {
                if result_bounds::proves(program, machine, *fact, &bounds) {
                    proven.push(*fact);
                    false
                } else {
                    true
                }
            });
        }
        if ensures.is_empty() {
            return;
        }
        inductive_transition_entailment(
            program,
            machine,
            &requires,
            &ensures,
            all_facts_are_expressions,
            diagnostics,
            proven,
        );
        return;
    }

    let mut engine = Engine::new(program, machine);
    let requires_fully_visible = engine.add_requires(&requires);
    if engine.requires_unsatisfiable {
        // Contradictory hypotheses entail everything: the proof-theoretic
        // `absurd` case. Accept.
        proven.extend_from_slice(&ensures);
        return;
    }

    for fact in &ensures {
        if std::env::var("OMEGA_ENTAILMENT_TRACE").is_ok() {
            eprintln!(
                "ENTAILDBG machine={} fact={} visible={} params={:?}",
                machine.name,
                program.expression_table.display_name(*fact),
                requires_fully_visible,
                engine.parameter_atoms
            );
        }
        // A fact already folded to a Boolean constant carries the constant
        // arithmetic's verdict directly: `true` is proven and `false` is
        // disproved. Standing down on `false` would let a refuted
        // postcondition pass `--check` silently.
        if let Some(constant) = program.expression_table.boolean_node(*fact) {
            if *constant {
                proven.push(*fact);
            } else {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` ensures contract proof fact `{}` is disproved by constant arithmetic",
                    machine.name,
                    program.expression_table.display_name(*fact)
                )));
            }
            continue;
        }
        match engine.judge(*fact) {
            Judgment::Proven => proven.push(*fact),
            Judgment::Refuted => {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` ensures contract proof fact `{}` is disproved: the requires contract entails its negation",
                    machine.name,
                    program.expression_table.display_name(*fact)
                )));
            }
            Judgment::ConstantFalse => {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` ensures contract proof fact `{}` is disproved by constant arithmetic",
                    machine.name,
                    program.expression_table.display_name(*fact)
                )));
            }
            Judgment::Unknown { goal_in_language } => {
                // A collection helper theorem: the polynomial engine cannot
                // read an element or a predicate application, but exploring
                // the predicate machine's own checked body under the
                // `requires` conjuncts proves the goal on every fork.
                if crate::proof_contracts::collection_predicates::premises_prove_expression(
                    program, machine, *fact,
                ) {
                    proven.push(*fact);
                    continue;
                }
                if goal_in_language && requires_fully_visible {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` cannot prove ensures contract proof fact `{}` from the requires contract",
                        machine.name,
                        program.expression_table.display_name(*fact)
                    )));
                }
                // Otherwise the contract leans on facts outside the engine's
                // language (domain membership, unknown calls, non-parameter
                // places): stand down rather than reject what we cannot read.
            }
        }
    }
}

/// Call-target summary of a contract fact: whether it spells any call, and
/// whether every call head resolves to a declared operator or machine
/// (vacuously true with no calls). The verbatim-requires discharge publishes
/// a hypothesis only when its spelling is over real declarations — an
/// undeclared view name has no selected declaration to supply a proof term.
struct FactCallTargets {
    any: bool,
    all_declared: bool,
}

fn fact_call_targets(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> FactCallTargets {
    if !expression.is_valid() {
        return FactCallTargets {
            any: false,
            all_declared: true,
        };
    }
    let merge = |handles: &mut dyn Iterator<Item = ExpressionHandle>| -> FactCallTargets {
        let mut summary = FactCallTargets {
            any: false,
            all_declared: true,
        };
        for handle in handles {
            let nested = fact_call_targets(program, handle);
            summary.any |= nested.any;
            summary.all_declared &= nested.all_declared;
        }
        summary
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => merge(&mut crate::expression_traversal::match_children(
            program, *dispatch,
        )),
        ExpressionNode::Atomic(atomic) => merge(&mut [atomic.value].into_iter()),
        ExpressionNode::Call(call) => {
            let declared = call.target_symbol.is_valid()
                || typed_trees::operator::resolve_named_expression_call(program, call).is_some()
                || program
                    .machines()
                    .iter()
                    .any(|machine| machine.name.as_str() == call.target.as_str());
            let mut nested = merge(
                &mut [call.receiver].into_iter().chain(
                    program
                        .expression_table
                        .expression_handles(call.arguments)
                        .iter()
                        .copied(),
                ),
            );
            nested.any = true;
            nested.all_declared &= declared;
            nested
        }
        ExpressionNode::Binary(binary) => merge(&mut [binary.left, binary.right].into_iter()),
        ExpressionNode::Unary(unary) => merge(&mut [unary.operand].into_iter()),
        ExpressionNode::Cast(cast) => merge(&mut [cast.value].into_iter()),
        ExpressionNode::Member(member) => merge(&mut [member.receiver].into_iter()),
        ExpressionNode::Borrow(inner) => merge(&mut [inner.target].into_iter()),
        ExpressionNode::Indexed(indexed) => {
            merge(&mut [indexed.collection, indexed.index].into_iter())
        }
        ExpressionNode::Range(range) => merge(&mut [range.start, range.end].into_iter()),
        ExpressionNode::StructLiteral(struct_literal) => merge(
            &mut program
                .expression_table
                .struct_fields(struct_literal.fields)
                .iter()
                .map(|field| field.value),
        ),
        ExpressionNode::ArrayLiteral(items) => merge(
            &mut program
                .expression_table
                .expression_handles(*items)
                .iter()
                .copied(),
        ),
        typed_trees::literal_expression_pattern!() => FactCallTargets {
            any: false,
            all_declared: true,
        },
    }
}

pub(crate) use structural_terms::source_operations_have_same_meaning;

//! Requires -> ensures ENTAILMENT for proof machines.
//!
//! An empty-body machine with `requires`/`ensures` contracts is a proof
//! artifact (chapter 10): the claim is that the requirements entail the
//! guarantees. This module judges each ensures fact as PROVEN, REFUTED, or
//! UNKNOWN, and -- when the whole contract lies inside the engine's language --
//! rejects anything it cannot prove, so a false theorem can no longer pass
//! `--check` silently (see this crate's README.md#source-proof-automation).
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
//! applications, `stand_downs.rs` records expression stand-downs,
//! `citations.rs` collects, suggests and instantiates citations,
//! `proof_edges.rs` judges strict decrease along proof edges,
//! `refuted_requires.rs` rejects refuted value-call requirements,
//! `specification_calls.rs` requires positive precondition evidence before
//! concrete machine/state contract terms form,
//! `structural_case_arms.rs` recognizes guarded structural case arms and
//! `self_induction.rs` intakes self-induction hypotheses.

#[cfg(test)]
mod argument_tests;
mod arithmetic_judgment;
mod call_requirements;
mod citations;
mod const_ranges;
mod exit_coverage;
mod inductive_judgment;
mod law_conformance;
#[cfg(test)]
mod outcome_tests;
mod proof_edges;
mod proof_integer;
#[cfg(test)]
mod proof_view_tests;
mod quotient_congruence;
mod ranking_range;
mod refuted_requires;
mod scoped_arithmetic;
mod self_induction;
mod specification_calls;
mod stand_downs;
mod strict_arithmetic;
mod structural_case_arms;
mod structural_judgment;
mod structural_terms;
mod transparent_applications;

pub use arithmetic_judgment::integer_embedding_sources_equal;
pub use call_requirements::structural_call_requirement_entailed;
pub(crate) use const_ranges::{
    const_range_bound_is_supported, selected_const_call_result_bounds, symbolic_range_contains,
    validate_const_range_call,
};
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
pub(crate) use proof_integer::{
    proof_integer_expression, proof_integer_nonnegative, proof_nat_cast,
    validate_proof_fact_integer_casts, validate_proof_integer_casts,
};
pub use ranking_range::{
    ComputationBodyShape, DeclaredIdentityView, DeclaredScalarView, MeasureBodyShape,
    ProjectionStep, ScalarViewComputation, computation_body_shape, declared_identity_view,
    declared_scalar_view, find_declared_measure, identity_subject_matches, measure_body_shape,
    measure_constraints_cover_subject, unwrap_constraint_shells,
};
pub(crate) use ranking_range::{
    RankingRangeCallEdge, RankingRangeCallMember, RankingRangeCallProgress, RankingRangeCallSite,
    call_member_premise_symbols, mixed_call_endpoints_are_pinned, positive_step_amount,
    prove_ranking_range_call, prove_ranking_range_call_entry,
};
pub use ranking_range::{
    RankingRangeEdgeProof, RankingRangeMeasure, RankingRangePremises, RankingRangeState,
    arithmetic_entry_requirement_is_covered, discover_state_entry_mappings,
    discover_state_entry_mappings_preferring, prove_arithmetic_call_requirement,
    prove_ranking_range_edge, prove_ranking_range_entry, prove_ranking_range_transition,
    ranking_range_premise_symbols, ranking_range_required_symbols,
};
pub(crate) use refuted_requires::reject_refuted_value_call_requires;
pub use scoped_arithmetic::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, scoped_arithmetic_implication,
};
pub(crate) use specification_calls::validate_specification_call_requirements;
pub use strict_arithmetic::{
    StrictArithmeticBindingValue, StrictArithmeticExpressionBinding,
    StrictArithmeticImplicationJudgment, StrictArithmeticSymbolBinding,
    strict_arithmetic_expression_implication,
    strict_arithmetic_expression_implication_with_arguments,
};
pub(crate) use structural_judgment::proved_index_algebras_for_provider;
pub use transparent_applications::transparent_proposition_application_entailed;

use std::collections::BTreeMap;

use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
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
use crate::proof_contracts::contract_entailment::stand_downs::{
    fact_mentions_proof_only_data, fact_mentions_zero_value, record_expression_stand_down,
};
use crate::proof_contracts::contract_entailment::structural_case_arms::{
    StructuralCaseArm, recognize_guarded_structural_value_arms, recognize_structural_case_arms,
};
use arithmetic_judgment::{Engine, Judgment, Polynomial};
use inductive_judgment::inductive_transition_entailment;
use quotient_congruence::{quotient_equality_from_requires, quotient_equality_names};
use structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use structural_terms::{
    split_structural_machine_name, structural_call_machine_name, structural_term, term_contains,
    unfold_constant_applications,
};

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
            if local.name.as_str().starts_with("__arm_destructure#")
    )
}

pub(crate) fn validate_machine_contract_entailment(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_machine_contract_entailment_with_stand_downs(
        program,
        machine,
        diagnostics,
        &mut Vec::new(),
    );
}

pub(crate) fn validate_machine_contract_entailment_with_stand_downs(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
    stand_downs: &mut Vec<crate::ContractEntailmentStandDown>,
) {
    validate_machine_contract_entailment_with_outcomes(
        program,
        machine,
        diagnostics,
        stand_downs,
        &mut Vec::new(),
    );
}

pub(crate) fn validate_machine_contract_entailment_with_outcomes(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
    stand_downs: &mut Vec<crate::ContractEntailmentStandDown>,
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
    let mut ensures_coordinates = Vec::new();
    let account_stand_downs = matches!(
        machine.supply_mode,
        language_semantics::MachineSupplyMode::CheckedBody
            | language_semantics::MachineSupplyMode::Boundary
    );
    // Membership facts (`value in Domain`) are outside the engine's language.
    // The empty-body path drops them silently (a dropped hypothesis only
    // weakens proving power); the inductive path additionally refuses to
    // REJECT when any are present, since the unread fact could entail the
    // goal.
    let mut all_facts_are_expressions = true;
    for (contract_index, contract) in program.machine_contracts(machine).iter().enumerate() {
        let bucket = match &contract.kind {
            SignatureContractKind::Requires => &mut requires,
            SignatureContractKind::Ensures => &mut ensures,
            SignatureContractKind::EnsuresForResultCase { .. }
            | SignatureContractKind::Crashes { .. } => continue,
        };
        for (fact_index, fact) in program
            .proof_facts
            .span_or_empty(contract.facts)
            .iter()
            .enumerate()
        {
            match fact {
                ProofFact::Expression(expression) => {
                    bucket.push(*expression);
                    if matches!(contract.kind, SignatureContractKind::Ensures) {
                        ensures_coordinates.push((*expression, contract_index, fact_index));
                    }
                }
                ProofFact::Proposition(application) => {
                    if matches!(contract.kind, SignatureContractKind::Requires) {
                        requires_propositions.push(application);
                    } else if account_stand_downs {
                        stand_downs.push(crate::ContractEntailmentStandDown {
                            machine_symbol: machine.symbol,
                            contract_index,
                            fact_index,
                            reason:
                                crate::ContractEntailmentStandDownReason::UnsupportedEnsuresFact,
                        });
                    }
                    all_facts_are_expressions = false;
                }
                ProofFact::Membership(_) => {
                    if matches!(contract.kind, SignatureContractKind::Ensures)
                        && account_stand_downs
                    {
                        stand_downs.push(crate::ContractEntailmentStandDown {
                            machine_symbol: machine.symbol,
                            contract_index,
                            fact_index,
                            reason:
                                crate::ContractEntailmentStandDownReason::UnsupportedEnsuresFact,
                        });
                    }
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
    let proof_only = typed_trees::proof_only::classify(program);
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
    let mut structural = StructuralJudge::from_requires(program, machine, &requires);
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
    let sole_arm_value = |judge: &StructuralJudge| -> Option<StructuralTerm> {
        let [root] = program.machine_states(machine) else {
            return None;
        };
        let mut environment: Vec<(String, StructuralTerm)> = program
            .state_parameters(root)
            .iter()
            .map(|parameter| {
                let name = parameter.name.as_str().to_owned();
                (name.clone(), StructuralTerm::Variable(name))
            })
            .collect();
        let mut result = None;
        for statement in program.statement_table.statements(root.statement_nodes) {
            if result.is_some() {
                return None; // statements after the value arm: out of shape
            }
            if is_arm_pattern_marker(statement) {
                continue; // exhaustiveness carrier, not shape
            }
            match statement {
                // Citation statements carry facts, not shape: their
                // equations are already in the judge's hypotheses.
                StatementNode::Call(call)
                    if is_citation_statement(program, &proof_only, machine, call) => {}
                StatementNode::LocalData(local_data) => {
                    let term = judge.callee_term(local_data.initial_value, &environment, 0)?;
                    environment.push((local_data.name.as_str().to_owned(), term));
                }
                StatementNode::Expression(value) => {
                    result = Some(judge.callee_term(*value, &environment, 0)?);
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
                    result = Some(judge.callee_term(*value, &environment, 0)?);
                }
                _ => return None,
            }
        }
        result
    };
    let sole_arm_result = sole_arm_value(&structural);
    // Result-to-parameter tag forwarding needs the actual returned subject,
    // before legacy unfolding can erase its origin. Reuse the same extraction
    // with the restricted subject resolver; a tag remains no field equation.
    let case_structural = StructuralJudge::from_case_requires(program, machine, &requires);
    let sole_arm_case_result = sole_arm_value(&case_structural);
    if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
        eprintln!(
            "STRUCT machine={} sole_arm={:?}",
            machine.name, sole_arm_result
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
            let ExpressionNode::Binary(binary) = program.expression_table.expression(*fact) else {
                return None;
            };
            if binary.operator != BinaryOperator::Equal {
                return None;
            }
            Some((
                structural_term(program, binary.left)?,
                structural_term(program, binary.right)?,
            ))
        })
        .collect();
    let case_arms: Option<Vec<StructuralCaseArm>> = if sole_arm_result.is_some() {
        None
    } else {
        recognize_structural_case_arms(program, machine, &structural, &proof_only, diagnostics)
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
    let judge_structural = |fact: ExpressionHandle| -> StructuralJudgment {
        if structural_terms::is_case_observation(program, fact)
            && let Some(term) = &sole_arm_case_result
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
            let mut bound = case_structural.clone();
            bound.intake_case_equation(
                StructuralTerm::Variable(RESULT_BINDER.to_owned()),
                term.clone(),
                0,
            );
            if matches!(bound.judge(program, fact), StructuralJudgment::Proven) {
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
        if let Some(term) = &sole_arm_result {
            let mut bound = structural.clone();
            if !result_is_parameter {
                bound
                    .substitutions
                    .insert(0, (RESULT_BINDER.to_owned(), term.clone()));
            }
            return bound.judge(program, fact);
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
                bound
                    .substitutions
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
            for (left, right) in &arm.citations {
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
                bound
                    .substitutions
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
    let ensures: Vec<ExpressionHandle> = ensures
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
            if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                eprintln!(
                    "ROUTE machine={} fact=`{}` mention={:?} zero_value={}",
                    machine.name,
                    program.expression_table.display_name(*fact),
                    mention.as_deref(),
                    zero_value_mention.is_some(),
                );
            }
            let Some(held) = mention else {
                // Pure, total fact-call projections are validated separately.
                // Their denotational field terms belong to the structural
                // judge, not to the polynomial engine. Reuse only its actual
                // positive judgment; an unread projection still stands down.
                if crate::machine_calls::fact_call_projections::expression_contains_call_projection(
                    program, *fact,
                ) && matches!(judge_structural(*fact), StructuralJudgment::Proven)
                {
                    proven.push(*fact);
                }
                // Not structural: stays with the polynomial engine below.
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
        inductive_transition_entailment(
            program,
            machine,
            &requires,
            &ensures,
            all_facts_are_expressions,
            diagnostics,
            account_stand_downs,
            &ensures_coordinates,
            stand_downs,
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
                if goal_in_language && requires_fully_visible {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` cannot prove ensures contract proof fact `{}` from the requires contract",
                        machine.name,
                        program.expression_table.display_name(*fact)
                    )));
                } else if account_stand_downs {
                    record_expression_stand_down(
                        machine,
                        *fact,
                        crate::ContractEntailmentStandDownReason::OutsideEntailmentLanguage,
                        &ensures_coordinates,
                        stand_downs,
                    );
                }
                // Otherwise the contract leans on facts outside the engine's
                // language (domain membership, unknown calls, non-parameter
                // places): stand down rather than reject what we cannot read.
            }
        }
    }
}

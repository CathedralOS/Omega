//! Requires -> ensures ENTAILMENT for proof machines.
//!
//! An empty-body machine with `requires`/`ensures` contracts is a proof
//! artifact (chapter 10): the claim is that the requirements entail the
//! guarantees. This module judges each ensures fact as PROVEN, REFUTED, or
//! UNKNOWN, and -- when the whole contract lies inside the engine's language --
//! rejects anything it cannot prove, so a false theorem can no longer pass
//! `--check` silently (see `wiki/proof_engine_roadmap.md`).
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
//! opaque proof-view applications (`Bag(items)`, `Seq(items)`) compared only
//! by equality. Facts: comparisons (`== != < <= > >=`) and range membership
//! (`t in lo..=hi`) over such terms. Anything else (domain membership,
//! unknown calls, non-parameter places) is OUTSIDE the language: the engine
//! still tries to prove with what it can see (extra unknown hypotheses can
//! only help, never hurt soundness of a proof), but it never REJECTS a
//! contract it cannot fully read.
//!
//! ## Mechanics
//!
//! 1. Terms normalize to canonical POLYNOMIALS over atoms (sum of monomials
//!    with i64 coefficients, atoms = parameter names / view applications /
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

use std::collections::BTreeMap;

use psi_diagnostics::Diagnostic;
use psi_numerics::bignum::BigInt;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::{SignatureContractKind, StateSignature};
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use psi_typed_trees::trait_definition::TraitDefinition;

mod arithmetic_judgment;
mod inductive_judgment;
mod law_conformance;
mod quotient_congruence;
mod structural_judgment;
mod structural_terms;

use arithmetic_judgment::{Engine, Judgment, Polynomial};
use inductive_judgment::inductive_transition_entailment;
pub(crate) use law_conformance::{check_law_conformance, check_operator_contract_conformance};
use law_conformance::{
    collect_equality_conjuncts, diagnostic_shape_match, display_structural_term,
    term_mentions_variable,
};
use quotient_congruence::{quotient_equality_from_requires, quotient_equality_names};
pub(crate) use structural_judgment::proved_index_algebras_for_provider;
use structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use structural_terms::{
    split_structural_machine_name, structural_call_machine_name, structural_term, term_contains,
    unfold_constant_applications,
};

/// Prove one transparent Boolean proposition application through the same
/// structural entailment engine used for ordinary Boolean contracts. Named
/// propositions remain the public fact identity; transparency is only the
/// proof-side expansion. The first rung accepts equality formulas directly
/// and never treats a primitive or witness-bearing proposition as transparent.
pub(crate) fn transparent_proposition_application_entailed(
    program: &TypedTrees,
    machine: &Machine,
    state: &psi_typed_trees::state::State,
    goal: &psi_typed_trees::proposition::PropositionApplication,
) -> bool {
    fn equation(
        program: &TypedTrees,
        judge: &StructuralJudge<'_>,
        application: &psi_typed_trees::proposition::PropositionApplication,
    ) -> Option<(StructuralTerm, StructuralTerm)> {
        fn argument_term(
            program: &TypedTrees,
            expression: ExpressionHandle,
        ) -> Option<StructuralTerm> {
            let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
                return structural_term(program, expression);
            };
            if call.receiver.is_valid() {
                return structural_term(program, expression);
            }
            let matches = program
                .machines()
                .iter()
                .flat_map(|machine| program.machine_states(machine))
                .filter(|state| state.symbol == call.target_symbol)
                .collect::<Vec<_>>();
            let [state] = matches.as_slice() else {
                return None;
            };
            let data_symbol = match program
                .type_reference_table
                .type_reference(state.return_type)
            {
                psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } => *symbol,
                psi_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } => {
                    *base_symbol
                }
                _ => return structural_term(program, expression),
            };
            let data = program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == data_symbol)?;
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| structural_term(program, *argument))
                .collect::<Option<Vec<_>>>()?;
            let machine =
                structural_call_machine_name(call.target.as_str(), &call.machine_arguments, &[]);
            let mut fields = program
                .data_members(data)
                .iter()
                .filter_map(|member| {
                    let psi_typed_trees::data::DataMember::Field(field) = member else {
                        return None;
                    };
                    Some((
                        field.name.as_str().to_owned(),
                        StructuralTerm::CallProjection {
                            target: call.target_symbol,
                            machine: machine.clone(),
                            result_type: state.return_type,
                            field: field.symbol,
                            field_name: field.name.as_str().to_owned(),
                            arguments: arguments.clone(),
                        },
                    ))
                })
                .collect::<Vec<_>>();
            fields.sort_by(|left, right| left.0.cmp(&right.0));
            Some(StructuralTerm::Constructor {
                data: data.name.as_str().to_owned(),
                case: String::new(),
                fields,
            })
        }

        let declaration = program
            .propositions()
            .iter()
            .find(|candidate| candidate.symbol == application.proposition)?;
        let psi_typed_trees::proposition::PropositionBody::Transparent {
            proposition:
                psi_typed_trees::proposition::PropositionFormula::BooleanExpression(formula),
        } = declaration.body
        else {
            return None;
        };
        let parameters = program.proposition_parameters(declaration);
        let arguments = program
            .expression_table
            .expression_handles(application.arguments);
        if parameters.len() != arguments.len() {
            return None;
        }
        let environment = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| {
                Some((
                    parameter.name.as_str().to_owned(),
                    argument_term(program, *argument)?,
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        let ExpressionNode::Binary(binary) = program.expression_table.expression(formula) else {
            return None;
        };
        if binary.operator != BinaryOperator::Equal {
            return None;
        }
        Some((
            judge.callee_term(binary.left, &environment, 0)?,
            judge.callee_term(binary.right, &environment, 0)?,
        ))
    }

    let expression_requires = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut judge = StructuralJudge::from_requires(program, machine, &expression_requires);
    for application in program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| match fact {
            ProofFact::Proposition(application) => Some(application),
            _ => None,
        })
    {
        if let Some((left, right)) = equation(program, &judge, application) {
            judge.intake_equation(left, right, 0);
        }
    }
    let Some((left, right)) = equation(program, &judge, goal) else {
        return false;
    };
    matches!(
        judge.judge_equation(judge.resolve(left), judge.resolve(right), 0),
        StructuralJudgment::Proven
    )
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
pub(super) fn is_arm_pattern_marker(statement: &StatementNode) -> bool {
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
    let mut requires = Vec::new();
    let mut requires_propositions = Vec::new();
    let mut ensures = Vec::new();
    let mut ensures_coordinates = Vec::new();
    let account_stand_downs = matches!(
        machine.supply_mode,
        psi_language_semantics::MachineSupplyMode::CheckedBody
            | psi_language_semantics::MachineSupplyMode::Boundary
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
            SignatureContractKind::Boundary | SignatureContractKind::Crashes { .. } => continue,
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
    let proof_only = psi_typed_trees::proof_only::classify(program);
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
    let sole_arm_result: Option<StructuralTerm> = (|| {
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
                    let term = structural.callee_term(local_data.initial_value, &environment, 0)?;
                    environment.push((local_data.name.as_str().to_owned(), term));
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
                    result = Some(structural.callee_term(*value, &environment, 0)?);
                }
                _ => return None,
            }
        }
        result
    })();
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
    let judge_structural = |fact: ExpressionHandle| -> StructuralJudgment {
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
            bound
                .substitutions
                .insert(0, (RESULT_BINDER.to_owned(), term.clone()));
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
                    &arm.machine_name,
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
                    && !requires
                        .iter()
                        .all(|fact| instantiated_fact_established(program, &bound, *fact, &map))
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
            bound
                .substitutions
                .insert(0, (RESULT_BINDER.to_owned(), arm.value.clone()));
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
                // Not structural: stays with the polynomial engine below.
                return true;
            };
            any_structural = true;
            if structural.hypotheses_contradictory {
                return false;
            }
            // CH10 ACCEPTED tier (GR6d): a bodyless boundary machine's
            // ensures is an AXIOM -- believed under the grant-locality rule
            // (own-package dev-active; the trust report carries the row),
            // never proven. The ENGINE VETO still applies: a statement the
            // judge can REFUTE is a compile error, grants notwithstanding.
            if machine.supply_mode == psi_language_semantics::MachineSupplyMode::Accepted
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
                StructuralJudgment::Proven => {}
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
        );
        return;
    }

    let mut engine = Engine::new(program, machine);
    let requires_fully_visible = engine.add_requires(&requires);
    if engine.requires_unsatisfiable {
        // Contradictory hypotheses entail everything: the proof-theoretic
        // `absurd` case. Accept.
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
            Judgment::Proven => {}
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

fn record_expression_stand_down(
    machine: &Machine,
    expression: ExpressionHandle,
    reason: crate::ContractEntailmentStandDownReason,
    coordinates: &[(ExpressionHandle, usize, usize)],
    stand_downs: &mut Vec<crate::ContractEntailmentStandDown>,
) {
    for (_, contract_index, fact_index) in coordinates
        .iter()
        .filter(|(candidate, _, _)| *candidate == expression)
    {
        stand_downs.push(crate::ContractEntailmentStandDown {
            machine_symbol: machine.symbol,
            contract_index: *contract_index,
            fact_index: *fact_index,
            reason,
        });
    }
}

fn record_all_expression_stand_downs(
    machine: &Machine,
    expressions: &[ExpressionHandle],
    reason: crate::ContractEntailmentStandDownReason,
    coordinates: &[(ExpressionHandle, usize, usize)],
    stand_downs: &mut Vec<crate::ContractEntailmentStandDown>,
) {
    for expression in expressions {
        record_expression_stand_down(machine, *expression, reason, coordinates, stand_downs);
    }
}

/// Does this contract conjunct SPEAK ABOUT proof-only data? Structural
/// detection over the expression tree: a machine parameter (or the `result`
/// atom) whose declared type mentions proof-only data, a classifier or case
/// literal naming a proof-only definition (`Nat::Zero`,
/// `Nat::Succ { .. }`), or a call whose target machine returns one. Returns
/// the named proof-only type for the diagnostic. Used by the ensures fence
/// above: such conjuncts are outside every judging tier today, and standing
/// down would silently certify them (math roster N3 owns the real tier).
fn fact_mentions_proof_only_data(
    program: &TypedTrees,
    classification: &psi_typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    expression: ExpressionHandle,
) -> Option<psi_typed_trees::name::Identifier> {
    if !expression.is_valid() {
        return None;
    }
    let recurse = |handle: ExpressionHandle| {
        fact_mentions_proof_only_data(program, classification, machine, handle)
    };
    let proof_only_definition = |name: &str| {
        program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .filter(|definition| classification.is_proof_only(definition.symbol))
            .map(|definition| definition.name.clone())
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => recurse(atomic.value),
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            match members {
                [single] => {
                    let entry = program.machine_states(machine).first()?;
                    if single.as_str() == "result" {
                        if entry.return_type.is_valid() {
                            return classification.proof_only_mention(program, entry.return_type);
                        }
                        return None;
                    }
                    program
                        .state_parameters(entry)
                        .iter()
                        .find(|parameter| parameter.name.as_str() == single.as_str())
                        .and_then(|parameter| {
                            classification.proof_only_mention(program, parameter.type_reference)
                        })
                }
                [first, ..] => proof_only_definition(first.as_str()),
                [] => None,
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            proof_only_definition(struct_literal.type_name.as_str()).or_else(|| {
                program
                    .expression_table
                    .struct_fields(struct_literal.fields)
                    .iter()
                    .find_map(|field| recurse(field.value))
            })
        }
        ExpressionNode::Call(call) => program
            .machines()
            .iter()
            .find(|target| {
                target.attached_data.is_none() && target.name.as_str() == call.target.as_str()
            })
            .and_then(|target| {
                let entry = program.machine_states(target).first()?;
                if !entry.return_type.is_valid() {
                    return None;
                }
                classification.proof_only_mention(program, entry.return_type)
            })
            .or_else(|| recurse(call.receiver))
            .or_else(|| {
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .find_map(|argument| recurse(*argument))
            }),
        ExpressionNode::Binary(binary) => recurse(binary.left).or_else(|| recurse(binary.right)),
        ExpressionNode::Unary(unary) => recurse(unary.operand),
        ExpressionNode::Cast(cast) => recurse(cast.value),
        ExpressionNode::Member(member) => recurse(member.receiver),
        ExpressionNode::Borrow(inner) => recurse(inner.target),
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection).or_else(|| recurse(indexed.index))
        }
        ExpressionNode::Range(range) => recurse(range.start).or_else(|| recurse(range.end)),
        ExpressionNode::ArrayLiteral(items) => program
            .expression_table
            .expression_handles(*items)
            .iter()
            .find_map(|item| recurse(*item)),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => None,
        ExpressionNode::ZeroValue(type_reference) => {
            classification.proof_only_mention(program, *type_reference)
        }
    }
}

/// Return the observed data name when a contract fact contains
/// `zero_value<T>()`. Unlike the proof-only-data fence above, this route also
/// covers ordinary runtime data: the observation is structural because its
/// meaning comes from authored home representation, not from integer
/// arithmetic.
fn fact_mentions_zero_value(program: &TypedTrees, expression: ExpressionHandle) -> Option<String> {
    use psi_typed_trees::types::TypeReferenceNode;

    if !expression.is_valid() {
        return None;
    }
    let recurse = |handle: ExpressionHandle| fact_mentions_zero_value(program, handle);
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => recurse(atomic.value),
        ExpressionNode::Binary(binary) => recurse(binary.left).or_else(|| recurse(binary.right)),
        ExpressionNode::Unary(unary) => recurse(unary.operand),
        ExpressionNode::Cast(cast) => recurse(cast.value),
        ExpressionNode::Member(member) => recurse(member.receiver),
        ExpressionNode::Borrow(inner) => recurse(inner.target),
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection).or_else(|| recurse(indexed.index))
        }
        ExpressionNode::Range(range) => recurse(range.start).or_else(|| recurse(range.end)),
        ExpressionNode::ArrayLiteral(items) => program
            .expression_table
            .expression_handles(*items)
            .iter()
            .find_map(|item| recurse(*item)),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find_map(|field| recurse(field.value)),
        ExpressionNode::Call(call) => recurse(call.receiver).or_else(|| {
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .find_map(|argument| recurse(*argument))
        }),
        ExpressionNode::ZeroValue(type_reference) => {
            let observed = *type_reference;
            let mut current = *type_reference;
            loop {
                match program.type_reference_table.type_reference(current) {
                    TypeReferenceNode::Constrained { base_type, .. } => current = *base_type,
                    TypeReferenceNode::Generic { base_name, .. } => {
                        return Some(base_name.as_str().to_owned());
                    }
                    TypeReferenceNode::Named { name, .. } => {
                        return Some(name.as_str().to_owned());
                    }
                    TypeReferenceNode::Reference { .. }
                    | TypeReferenceNode::FixedArray { .. }
                    | TypeReferenceNode::Slice { .. }
                    | TypeReferenceNode::DynamicTrait { .. }
                    | TypeReferenceNode::ConstExpression(_)
                    | TypeReferenceNode::Unit => {
                        return Some(program.display_type_reference(observed));
                    }
                }
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => None,
    }
}

/// Whether a statement is a CITATION: a free (receiver-less) call whose
/// callee resolves to a free PROOF MACHINE other than the enclosing one
/// (self-calls are recursion, owned by the descent checks and the IH).
fn is_citation_statement(
    program: &TypedTrees,
    classification: &psi_typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    call: &psi_typed_trees::statement::TableCall,
) -> bool {
    if !call.receiver.is_empty() {
        return false;
    }
    program
        .machines()
        .iter()
        .find(|candidate| {
            candidate.attached_data.is_none() && candidate.name.as_str() == call.target.as_str()
        })
        .is_some_and(|callee| {
            !std::ptr::eq(callee, machine) && classification.is_proof_machine(program, callee)
        })
}

/// Statement-call CITATIONS (ch10 "Citing Proofs"; the settled proof-citation
/// rule, 2026-07-18): `add_zero_right(b);` inside a proof body delivers
/// the callee's proven ensures to this site, instantiated at the call's
/// argument terms -- the equations these return feed the structural judge's
/// hypotheses exactly like requires facts. Fact injection is the explicit
/// default; no global rules, no pattern matching, nothing applies silently.
///
/// Soundness: the callee's ensures is machine-checked in this same
/// validation batch (a false lemma raises its own error, so no compiling
/// program cites an unproven fact), and lemma-citing-lemma cycles are
/// machine CALL cycles -- banned by the call-graph rule, so mutual
/// false-certification is structurally impossible.
///
/// v1 boundary: a REQUIRES-bearing lemma cannot be cited yet (a theorem
/// applies only at operands satisfying its requires; site discharge is the
/// recorded next rung) -- citing one errors loudly rather than silently
/// injecting a conditional fact.
fn collect_citation_equations(
    program: &TypedTrees,
    classification: &psi_typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
    judge: Option<&StructuralJudge>,
) -> Vec<(StructuralTerm, StructuralTerm)> {
    let mut equations = Vec::new();
    let mut site_judge = judge.cloned();
    // Machine level reads the ENTRY state only: sub-state citations
    // reference sub-state parameters, which have no machine-level frame --
    // they intake PER ARM in `recognize_structural_case_arms`, converted
    // under that arm's environment.
    let Some(entry) = program.machine_states(machine).first() else {
        return equations;
    };
    // Entry-state citations may follow authored or lowering-generated `let`
    // bindings. Termify their operands under the same incremental local
    // environment used by sub-state proof arms. Treating a local as a fresh
    // variable loses the cited equation exactly when a proof names an
    // intermediate term (`let cross = mul(..); sub_self(cross);`).
    let mut environment: Vec<(String, StructuralTerm)> = program
        .state_parameters(entry)
        .iter()
        .map(|parameter| {
            let name = parameter.name.as_str().to_owned();
            (name.clone(), StructuralTerm::Variable(name))
        })
        .collect();
    for statement in program.statement_table.statements(entry.statement_nodes) {
        if let StatementNode::LocalData(local) = statement {
            if let Some(judge) = site_judge.as_ref()
                && let Some(term) = judge.callee_term(local.initial_value, &environment, 0)
            {
                environment.push((local.name.as_str().to_owned(), term));
            }
            continue;
        }
        let Some((target, argument_handles)) = citation_call_in_statement(program, statement)
        else {
            continue;
        };
        let mut argument_terms: Vec<StructuralTerm> = Vec::with_capacity(argument_handles.len());
        let mut arguments_termify = true;
        for argument in &argument_handles {
            let term = site_judge
                .as_ref()
                .and_then(|judge| judge.callee_term(*argument, &environment, 0))
                .or_else(|| structural_term(program, *argument));
            let Some(term) = term else {
                arguments_termify = false;
                break;
            };
            argument_terms.push(term);
        }
        if !arguments_termify {
            continue;
        }
        let before = equations.len();
        instantiate_citation(
            program,
            classification,
            machine,
            target,
            &argument_terms,
            diagnostics,
            &mut equations,
            site_judge.as_ref(),
        );
        if let Some(judge) = &mut site_judge {
            for (left, right) in &equations[before..] {
                judge.intake_equation(left.clone(), right.clone(), 0);
            }
        }
    }
    if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
        eprintln!("CITE machine={} equations={equations:?}", machine.name);
    }
    equations
}

/// The failure-side HALF of the citation ergonomics (the OWNER_QUESTIONS
/// #14 answer, verbatim design: "when an obligation fails and a known
/// lemma's ensures shape-matches it, the diagnostic NAMES the missing
/// citation... Suggestion at failure, never silent application"). Scans
/// requires-free free proof machines for an ensures `==`-conjunct that
/// first-order matches the fenced goal (lemma parameters as pattern
/// variables, either orientation) and renders the exact citation statement
/// to write. DIAGNOSTIC ONLY -- nothing here feeds the judge.
fn suggest_missing_citation(
    program: &TypedTrees,
    classification: &psi_typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    fact: ExpressionHandle,
) -> Option<String> {
    // Walk the fact's `&&`-conjuncts; the first suggestible equation wins.
    let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => {
            return suggest_missing_citation(program, classification, machine, binary.left)
                .or_else(|| {
                    suggest_missing_citation(program, classification, machine, binary.right)
                });
        }
        BinaryOperator::Equal => {}
        _ => return None,
    }
    let goal_left = structural_term(program, binary.left)?;
    let goal_right = structural_term(program, binary.right)?;

    for lemma in program.machines() {
        if lemma.attached_data.is_some()
            || std::ptr::eq(lemma, machine)
            || !classification.is_proof_machine(program, lemma)
        {
            continue;
        }
        let mut has_requires = false;
        let mut ensures_facts: Vec<ExpressionHandle> = Vec::new();
        for contract in program.machine_contracts(lemma) {
            match &contract.kind {
                SignatureContractKind::Requires => {
                    has_requires |= !program.proof_facts.span_or_empty(contract.facts).is_empty();
                }
                SignatureContractKind::Ensures => {
                    for lemma_fact in program.proof_facts.span_or_empty(contract.facts) {
                        if let ProofFact::Expression(expression) = lemma_fact {
                            ensures_facts.push(*expression);
                        }
                    }
                }
                SignatureContractKind::Boundary | SignatureContractKind::Crashes { .. } => {}
            }
        }
        // A requires-bearing lemma cannot be cited yet; suggesting it would
        // walk the author into the v1 refusal.
        if has_requires {
            continue;
        }
        let Some(entry) = program.machine_states(lemma).first() else {
            continue;
        };
        let parameters: Vec<String> = program
            .state_parameters(entry)
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect();
        for lemma_fact in &ensures_facts {
            if let Some(suggestion) = suggest_conjunct_match(
                program,
                lemma,
                &parameters,
                *lemma_fact,
                &goal_left,
                &goal_right,
            ) {
                return Some(suggestion);
            }
        }
    }
    None
}

fn suggest_conjunct_match(
    program: &TypedTrees,
    lemma: &Machine,
    parameters: &[String],
    lemma_fact: ExpressionHandle,
    goal_left: &StructuralTerm,
    goal_right: &StructuralTerm,
) -> Option<String> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(lemma_fact) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => {
            return suggest_conjunct_match(
                program,
                lemma,
                parameters,
                binary.left,
                goal_left,
                goal_right,
            )
            .or_else(|| {
                suggest_conjunct_match(
                    program,
                    lemma,
                    parameters,
                    binary.right,
                    goal_left,
                    goal_right,
                )
            });
        }
        BinaryOperator::Equal => {}
        _ => return None,
    }
    let lemma_left = structural_term(program, binary.left)?;
    let lemma_right = structural_term(program, binary.right)?;
    let result_binder = RESULT_BINDER.to_owned();
    // `result`-shaped conjuncts describe the lemma's application, not a
    // free-standing law; the law conjuncts are the suggestible material.
    if term_mentions_variable(&lemma_left, &result_binder)
        || term_mentions_variable(&lemma_right, &result_binder)
    {
        return None;
    }
    for (first, second) in [(goal_left, goal_right), (goal_right, goal_left)] {
        let mut bindings: Vec<(String, StructuralTerm)> = Vec::new();
        if diagnostic_shape_match(&lemma_left, first, parameters, &mut bindings)
            && diagnostic_shape_match(&lemma_right, second, parameters, &mut bindings)
        {
            let arguments: Vec<String> = parameters
                .iter()
                .map(|parameter| {
                    bindings
                        .iter()
                        .find(|(name, _)| name == parameter)
                        .map(|(_, term)| display_structural_term(term))
                        .unwrap_or_else(|| "..".to_owned())
                })
                .collect();
            return Some(format!(
                "note: `{lemma}` proves this shape -- cite it: `{lemma}({arguments});`",
                lemma = lemma.name.as_str(),
                arguments = arguments.join(", "),
            ));
        }
    }
    None
}

/// Extract a potential citation call from a statement: the target name and
/// argument expression handles, for either spelling -- the bare statement
/// call (ch10's canonical form) or the let-bound call (a legal spelling in
/// its own right, and what the trailing-return auto-hoist lowers the bare
/// form into). The proof-machine gates apply in `instantiate_citation`.
///
/// NOTE the two spellings' argument spans live in DIFFERENT arenas:
/// statement calls own theirs in the statement table, expression calls in
/// the expression table.
fn citation_call_in_statement<'program>(
    program: &'program TypedTrees,
    statement: &'program StatementNode,
) -> Option<(
    &'program psi_typed_trees::name::Identifier,
    Vec<ExpressionHandle>,
)> {
    match statement {
        StatementNode::Call(call) if call.receiver.is_empty() => Some((
            &call.target,
            program
                .statement_table
                .expression_handles(call.arguments)
                .to_vec(),
        )),
        StatementNode::LocalData(local_data) => {
            let ExpressionNode::Call(call) = program
                .expression_table
                .expression(local_data.initial_value)
            else {
                return None;
            };
            if call.receiver.is_valid() {
                return None;
            }
            Some((
                &call.target,
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec(),
            ))
        }
        _ => None,
    }
}

/// Resolve and gate ONE citation, pushing the callee's ensures conjuncts
/// instantiated at `argument_terms` (the call's arguments ALREADY converted
/// to terms in the consumer's frame: machine-level intake reads them raw,
/// per-arm intake converts under the arm environment first). `result` maps
/// to the application at these operands.
/// N4 slice a3 (gcd): judge a recursive proof machine edge's STRICT-DECREASE
/// obligation through the structural judge -- the general route when the
/// syntactic citation match cannot see through arm destructuring. The judge
/// starts from the machine's requires, gains the source state's INCOMING-ARM
/// hypotheses (guard equations, plus the MATERIALIZED payload alias
/// `subject == Case { field: param }` recovered from a tag-only guard, the
/// data declaration, and the incoming transition's payload-read target
/// arguments), then intakes the source state's citations IN ORDER --
/// statement calls and `let`-bound call initializers alike -- each with its
/// requires judged Proven first (skipped otherwise; over-refusal safe) and
/// its ensures instantiated with `result` mapped to the call term. The
/// obligation `saturating_sub(Succ(ARG), MEASURE) == Zero` then judges under the
/// accumulated hypotheses.
pub(crate) fn proof_edge_strict_decrease_judged(
    program: &TypedTrees,
    machine: &Machine,
    state: &psi_typed_trees::state::State,
    edge_argument: ExpressionHandle,
    measure_name: &str,
) -> bool {
    let requires = machine_requires_facts(program, machine);
    let mut judge = StructuralJudge::from_requires(program, machine, &requires);
    let trace = std::env::var_os("OMEGA_EDGE_TRACE").is_some();

    // Incoming-arm hypotheses -- SOUND only when this state has exactly ONE
    // incoming edge (otherwise a second path could reach it without the
    // arm's case holding; conservative: intake nothing).
    let mut incoming = 0usize;
    for other in program.machine_states(machine) {
        for statement in program.statement_table.statements(other.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            if !transition.target.is_valid() {
                continue;
            }
            if let TransitionTargetNode::Named { path, .. } =
                program.statement_table.transition_target(transition.target)
                && program
                    .statement_table
                    .name_path_members(path.members)
                    .last()
                    .is_some_and(|name| name.as_str() == state.name.as_str())
            {
                incoming += 1;
            }
        }
    }
    if incoming != 1 {
        if trace {
            eprintln!(
                "EDGE {}: {} incoming edges -- arm facts skipped",
                state.name.as_str(),
                incoming
            );
        }
    }
    for other in program.machine_states(machine) {
        if incoming != 1 {
            break;
        }
        for statement in program.statement_table.statements(other.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            let TransitionGuardNode::When(guard) = transition.guard else {
                if trace {
                    eprintln!(
                        "EDGE incoming arm into {}: guard NOT When",
                        state.name.as_str()
                    );
                }
                continue;
            };
            if !transition.target.is_valid() {
                continue;
            }
            let TransitionTargetNode::Named {
                path, arguments, ..
            } = program.statement_table.transition_target(transition.target)
            else {
                continue;
            };
            let targets_state = program
                .statement_table
                .name_path_members(path.members)
                .last()
                .is_some_and(|name| name.as_str() == state.name.as_str());
            if !targets_state {
                continue;
            }
            // Materialize the payload alias for a tag-only case guard
            // BEFORE any raw intake: a fieldless `b == Nat::Succ`
            // substitution would win first and mask the payload (first
            // binding wins), leaving `b`'s prev unreachable.
            let ExpressionNode::Binary(comparison) = program.expression_table.expression(guard)
            else {
                judge.intake(program, guard);
                continue;
            };
            if comparison.operator != BinaryOperator::Equal {
                judge.intake(program, guard);
                continue;
            }
            let Some(subject_term) = structural_term(program, comparison.left) else {
                continue;
            };
            let Some(StructuralTerm::Constructor { data, case, fields }) =
                structural_term(program, comparison.right)
            else {
                judge.intake(program, guard);
                continue;
            };
            if !fields.is_empty() {
                judge.intake(program, guard);
                continue;
            }
            let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == data.as_str())
            else {
                judge.intake(program, guard);
                continue;
            };
            let Some(declared_fields) =
                program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        psi_typed_trees::data::DataMember::Variant(variant)
                            if variant.name.as_str() == case.as_str() =>
                        {
                            Some(
                                program
                                    .data_payload_fields(variant)
                                    .iter()
                                    .map(|field| field.name.as_str().to_owned())
                                    .collect::<Vec<_>>(),
                            )
                        }
                        _ => None,
                    })
            else {
                continue;
            };
            if declared_fields.is_empty() {
                judge.intake(program, guard);
                continue;
            }
            // Payload reads in the incoming target arguments: an argument
            // `<subject>.field` delivered to sub-state param `p` aliases
            // the payload as `p`.
            let parameters = program.state_parameters(state);
            let argument_handles = program.statement_table.expression_handles(*arguments);
            if parameters.len() != argument_handles.len() {
                judge.intake(program, guard);
                continue;
            }
            let mut aliased: Vec<(String, StructuralTerm)> = Vec::new();
            for (parameter, argument) in parameters.iter().zip(argument_handles) {
                let ExpressionNode::Member(member) = program.expression_table.expression(*argument)
                else {
                    continue;
                };
                let Some(receiver_term) = structural_term(program, member.receiver) else {
                    continue;
                };
                if receiver_term != subject_term {
                    continue;
                }
                let member_name = member.member.as_str().to_owned();
                if declared_fields.iter().any(|field| *field == member_name) {
                    aliased.push((
                        member_name,
                        StructuralTerm::Variable(parameter.name.as_str().to_owned()),
                    ));
                }
            }
            if trace {
                eprintln!(
                    "EDGE alias for {}: aliased {}/{} declared",
                    state.name.as_str(),
                    aliased.len(),
                    declared_fields.len()
                );
            }
            if aliased.len() == declared_fields.len() {
                aliased.sort_by(|(left, _), (right, _)| left.cmp(right));
                judge.intake_equation(
                    subject_term,
                    StructuralTerm::Constructor {
                        data,
                        case,
                        fields: aliased,
                    },
                    0,
                );
            } else {
                judge.intake(program, guard);
            }
        }
    }

    // Citations + local bindings, in statement order.
    for statement in program.statement_table.statements(state.statement_nodes) {
        match statement {
            StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                if let ExpressionNode::Call(call) =
                    program.expression_table.expression(local.initial_value)
                {
                    intake_citation_for_edge(
                        program,
                        &mut judge,
                        call,
                        Some(local.name.as_str()),
                        local.initial_value,
                    );
                } else if let Some(term) = structural_term(program, local.initial_value) {
                    judge.intake_equation(
                        StructuralTerm::Variable(local.name.as_str().to_owned()),
                        term,
                        0,
                    );
                }
            }
            StatementNode::Call(call) => {
                let receiver_members = program.statement_table.name_path_members(call.receiver);
                if !receiver_members.is_empty() {
                    continue;
                }
                intake_statement_citation_for_edge(program, &mut judge, call);
            }
            _ => {}
        }
    }

    // The obligation.
    let Some(argument_term) = structural_term(program, edge_argument) else {
        return false;
    };
    let left = StructuralTerm::Application {
        machine: "saturating_sub".to_owned(),
        arguments: vec![
            StructuralTerm::Constructor {
                data: "Nat".to_owned(),
                case: "Succ".to_owned(),
                fields: vec![("prev".to_owned(), argument_term)],
            },
            StructuralTerm::Variable(measure_name.to_owned()),
        ],
    };
    let right = StructuralTerm::Constructor {
        data: "Nat".to_owned(),
        case: "Zero".to_owned(),
        fields: Vec::new(),
    };
    let verdict = judge.judge_equation(judge.resolve(left.clone()), judge.resolve(right), 0);
    if trace {
        eprintln!(
            "EDGE obligation in {}: resolved LHS {:?} verdict {}",
            state.name.as_str(),
            judge.resolve(left),
            match verdict {
                StructuralJudgment::Proven => "Proven",
                StructuralJudgment::Refuted => "Refuted",
                StructuralJudgment::Unknown => "Unknown",
            }
        );
    }
    matches!(verdict, StructuralJudgment::Proven)
}

/// One citation for the edge judge: the callee's requires must judge Proven
/// under the CURRENT hypotheses (else the citation contributes nothing);
/// its ensures intake with `result` mapped to the call term, and a `let`
/// binder aliases the call term.
fn intake_citation_for_edge(
    program: &TypedTrees,
    judge: &mut StructuralJudge<'_>,
    call: &psi_typed_trees::expression::TableCallExpression,
    binder: Option<&str>,
    _call_expression: ExpressionHandle,
) {
    let Some(callee) = program.machines().iter().find(|candidate| {
        candidate.attached_data.is_none()
            && candidate
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(candidate.name.as_str())
                == call.target.as_str()
    }) else {
        return;
    };
    let Some(entry) = program.machine_states(callee).first() else {
        return;
    };
    let parameters = program.state_parameters(entry);
    let argument_handles = program.expression_table.expression_handles(call.arguments);
    if parameters.len() != argument_handles.len() {
        return;
    }
    let mut argument_terms = Vec::with_capacity(argument_handles.len());
    for argument in argument_handles {
        let Some(term) = structural_term(program, *argument) else {
            return;
        };
        argument_terms.push(term);
    }
    let call_term = StructuralTerm::Application {
        machine: call.target.as_str().to_owned(),
        arguments: argument_terms.clone(),
    };
    let mut map: Vec<(String, StructuralTerm)> = parameters
        .iter()
        .zip(argument_terms)
        .map(|(parameter, term)| (parameter.name.as_str().to_owned(), term))
        .collect();
    map.push((RESULT_BINDER.to_owned(), call_term.clone()));

    let facts = |kind: psi_typed_trees::signature::SignatureContractKind| {
        program
            .machine_contracts(callee)
            .iter()
            .filter(|contract| contract.kind == kind)
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
            .filter_map(|fact| match fact {
                ProofFact::Expression(expression) => Some(*expression),
                ProofFact::Membership(_) | ProofFact::Proposition(_) => None,
            })
            .collect::<Vec<_>>()
    };
    for fact in facts(psi_typed_trees::signature::SignatureContractKind::Requires) {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
            return;
        };
        if binary.operator != BinaryOperator::Equal {
            return;
        }
        let (Some(left), Some(right)) = (
            structural_term(program, binary.left),
            structural_term(program, binary.right),
        ) else {
            return;
        };
        let left = StructuralJudge::substitute_term(&left, &map);
        let right = StructuralJudge::substitute_term(&right, &map);
        let verdict = judge.judge_equation(judge.resolve(left.clone()), judge.resolve(right), 0);
        if std::env::var_os("OMEGA_EDGE_TRACE").is_some() {
            eprintln!(
                "EDGE citation {} requires resolved {:?} verdict {}",
                call.target.as_str(),
                judge.resolve(left),
                match verdict {
                    StructuralJudgment::Proven => "Proven",
                    StructuralJudgment::Refuted => "Refuted",
                    StructuralJudgment::Unknown => "Unknown",
                }
            );
        }
        if !matches!(verdict, StructuralJudgment::Proven) {
            return;
        }
    }
    for fact in facts(psi_typed_trees::signature::SignatureContractKind::Ensures) {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
            continue;
        };
        if binary.operator != BinaryOperator::Equal {
            continue;
        }
        let (Some(left), Some(right)) = (
            structural_term(program, binary.left),
            structural_term(program, binary.right),
        ) else {
            continue;
        };
        judge.intake_equation(
            StructuralJudge::substitute_term(&left, &map),
            StructuralJudge::substitute_term(&right, &map),
            0,
        );
    }
    if let Some(binder) = binder {
        // A `let` binder EXPANDS to its call term (a substitution, not an
        // intake -- intake_equation orients application sides REDUCING,
        // which is exactly backwards for a binder the obligation must see
        // through).
        judge
            .substitutions
            .insert(0, (binder.to_owned(), call_term));
    }
}

/// A bare statement-call citation (no binder).
fn intake_statement_citation_for_edge(
    program: &TypedTrees,
    judge: &mut StructuralJudge<'_>,
    call: &psi_typed_trees::statement::TableCall,
) {
    let Some(callee) = program.machines().iter().find(|candidate| {
        candidate.attached_data.is_none()
            && candidate
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(candidate.name.as_str())
                == call.target.as_str()
    }) else {
        return;
    };
    let Some(entry) = program.machine_states(callee).first() else {
        return;
    };
    let parameters = program.state_parameters(entry);
    let argument_handles = program.statement_table.expression_handles(call.arguments);
    if parameters.len() != argument_handles.len() {
        return;
    }
    let mut argument_terms = Vec::with_capacity(argument_handles.len());
    for argument in argument_handles {
        let Some(term) = structural_term(program, *argument) else {
            return;
        };
        argument_terms.push(term);
    }
    let call_term = StructuralTerm::Application {
        machine: call.target.as_str().to_owned(),
        arguments: argument_terms.clone(),
    };
    let mut map: Vec<(String, StructuralTerm)> = parameters
        .iter()
        .zip(argument_terms)
        .map(|(parameter, term)| (parameter.name.as_str().to_owned(), term))
        .collect();
    map.push((RESULT_BINDER.to_owned(), call_term));
    let collect = |kind: psi_typed_trees::signature::SignatureContractKind| {
        program
            .machine_contracts(callee)
            .iter()
            .filter(|contract| contract.kind == kind)
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
            .filter_map(|fact| match fact {
                ProofFact::Expression(expression) => Some(*expression),
                ProofFact::Membership(_) | ProofFact::Proposition(_) => None,
            })
            .collect::<Vec<_>>()
    };
    for fact in collect(psi_typed_trees::signature::SignatureContractKind::Requires) {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
            return;
        };
        if binary.operator != BinaryOperator::Equal {
            return;
        }
        let (Some(left), Some(right)) = (
            structural_term(program, binary.left),
            structural_term(program, binary.right),
        ) else {
            return;
        };
        let left = StructuralJudge::substitute_term(&left, &map);
        let right = StructuralJudge::substitute_term(&right, &map);
        if !matches!(
            judge.judge_equation(judge.resolve(left), judge.resolve(right), 0),
            StructuralJudgment::Proven
        ) {
            return;
        }
    }
    for fact in collect(psi_typed_trees::signature::SignatureContractKind::Ensures) {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
            continue;
        };
        if binary.operator != BinaryOperator::Equal {
            continue;
        }
        let (Some(left), Some(right)) = (
            structural_term(program, binary.left),
            structural_term(program, binary.right),
        ) else {
            continue;
        };
        judge.intake_equation(
            StructuralJudge::substitute_term(&left, &map),
            StructuralJudge::substitute_term(&right, &map),
            0,
        );
    }
}

/// The machine's requires facts as expressions (empty when none).
fn machine_requires_facts(program: &TypedTrees, machine: &Machine) -> Vec<ExpressionHandle> {
    program
        .machine_contracts(machine)
        .iter()
        .filter(|contract| {
            matches!(
                contract.kind,
                psi_typed_trees::signature::SignatureContractKind::Requires
            )
        })
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
        .filter_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            ProofFact::Membership(_) | ProofFact::Proposition(_) => None,
        })
        .collect()
}

fn instantiate_citation(
    program: &TypedTrees,
    classification: &psi_typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    target: &psi_typed_trees::name::Identifier,
    argument_terms: &[StructuralTerm],
    diagnostics: &mut Vec<Diagnostic>,
    equations: &mut Vec<(StructuralTerm, StructuralTerm)>,
    judge: Option<&StructuralJudge>,
) {
    let Some(callee) = program.machines().iter().find(|candidate| {
        candidate.attached_data.is_none() && candidate.name.as_str() == target.as_str()
    }) else {
        return;
    };
    if std::ptr::eq(callee, machine) || !classification.is_proof_machine(program, callee) {
        return;
    }
    let mut requires_facts: Vec<ExpressionHandle> = Vec::new();
    let mut requires_out_of_language = false;
    let mut ensures_facts: Vec<ExpressionHandle> = Vec::new();
    for contract in program.machine_contracts(callee) {
        match &contract.kind {
            SignatureContractKind::Requires => {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    match fact {
                        ProofFact::Expression(expression) => requires_facts.push(*expression),
                        // Membership requires are outside the structural
                        // judge's language: the site cannot discharge them.
                        ProofFact::Membership(_) | ProofFact::Proposition(_) => {
                            requires_out_of_language = true;
                        }
                    }
                }
            }
            SignatureContractKind::Ensures => {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    if let ProofFact::Expression(expression) = fact {
                        ensures_facts.push(*expression);
                    }
                }
            }
            SignatureContractKind::Boundary | SignatureContractKind::Crashes { .. } => {}
        }
    }
    // The ENTRY state carries the signature; further states are the
    // lemma's own sub-proofs (add_comm's per-arm states) and do not affect
    // what a citation delivers.
    let Some(entry) = program.machine_states(callee).first() else {
        return;
    };
    let parameters = program.state_parameters(entry);
    if parameters.len() != argument_terms.len() {
        return;
    }
    let mut map: Vec<(String, StructuralTerm)> = Vec::with_capacity(parameters.len() + 1);
    for (parameter, term) in parameters.iter().zip(argument_terms) {
        map.push((parameter.name.as_str().to_owned(), term.clone()));
    }
    // SITE DISCHARGE (math roster N3): a theorem applies only at operands
    // satisfying its REQUIRES, so each requires conjunct instantiates under
    // the citation's argument map and must judge PROVEN against the facts
    // established before this statement (machine requires, case refinements,
    // an available IH, and earlier citations). Sites without a judge keep the
    // blanket refusal.
    if requires_out_of_language || (!requires_facts.is_empty() && judge.is_none()) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` cites `{}`, whose requires contract is not \
             discharged at citation sites yet -- cite a requires-free \
             lemma, or wait for the site-discharge rung (math roster N3)",
            machine.name, callee.name,
        )));
        return;
    }
    if let Some(judge) = judge {
        for fact in &requires_facts {
            if !instantiated_fact_established(program, judge, *fact, &map) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` cites `{}`, but the callee's requires fact \
                     `{}` is not established at this citation site under the \
                     citing machine's hypotheses -- add the matching requires \
                     (or cite at operands that satisfy it)",
                    machine.name,
                    callee.name,
                    program.expression_table.display_name(*fact),
                )));
                return;
            }
        }
    }
    // `result` in the callee's ensures denotes the application itself at
    // these operands.
    map.push((
        RESULT_BINDER.to_owned(),
        StructuralTerm::Application {
            machine: callee.name.as_str().to_owned(),
            arguments: argument_terms.to_vec(),
        },
    ));
    for fact in ensures_facts {
        collect_instantiated_conjuncts(program, fact, &map, equations);
    }
}

/// Does the callee's requires fact, instantiated at the citation's argument
/// map, judge PROVEN under the citing machine's hypotheses? `&&` recurses;
/// only `==` conjuncts are in the judge's language (anything else is
/// conservatively NOT established).
fn instantiated_fact_established(
    program: &TypedTrees,
    judge: &StructuralJudge,
    fact: ExpressionHandle,
    map: &[(String, StructuralTerm)],
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
        return false;
    };
    match binary.operator {
        BinaryOperator::And => {
            instantiated_fact_established(program, judge, binary.left, map)
                && instantiated_fact_established(program, judge, binary.right, map)
        }
        BinaryOperator::Equal => {
            let (Some(left), Some(right)) = (
                structural_term(program, binary.left),
                structural_term(program, binary.right),
            ) else {
                return false;
            };
            let left = StructuralJudge::substitute_term(&left, map);
            let right = StructuralJudge::substitute_term(&right, map);
            matches!(
                judge.judge_equation(judge.resolve(left), judge.resolve(right), 0),
                StructuralJudgment::Proven
            )
        }
        _ => false,
    }
}

/// Reject a value-position call when one of the callee's equality-style
/// `requires` facts is structurally FALSE at the concrete operands. This is
/// the fail-safe first rung of general value-call precondition discharge: a
/// proven contradiction is an error, while an unproved/unknown fact remains for
/// the later fail-closed obligation rung instead of becoming a false positive.
pub(crate) fn reject_refuted_value_call_requires(
    program: &TypedTrees,
    caller_machine: &Machine,
    caller_state: &psi_typed_trees::state::State,
    callee_machine: &Machine,
    callee_state: &psi_typed_trees::state::State,
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parameters = program
        .state_parameters(callee_state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return;
    }

    let mut map = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let Some(term) = structural_term(program, *argument) else {
            return;
        };
        map.push((parameter.name.as_str().to_owned(), term));
    }

    let mut caller_requires = Vec::new();
    for contract in program.machine_contracts(caller_machine) {
        if contract.kind == SignatureContractKind::Requires {
            caller_requires.extend(
                program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .filter_map(|fact| match fact {
                        ProofFact::Expression(expression) => Some(*expression),
                        ProofFact::Membership(_) | ProofFact::Proposition(_) => None,
                    }),
            );
        }
    }
    for contract in program.state_contracts(caller_state) {
        if contract.kind == SignatureContractKind::Requires {
            caller_requires.extend(
                program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .filter_map(|fact| match fact {
                        ProofFact::Expression(expression) => Some(*expression),
                        ProofFact::Membership(_) | ProofFact::Proposition(_) => None,
                    }),
            );
        }
    }
    let judge = StructuralJudge::from_requires(program, caller_machine, &caller_requires);
    if judge.hypotheses_contradictory {
        return;
    }

    for contract in program
        .machine_contracts(callee_machine)
        .iter()
        .chain(program.state_contracts(callee_state))
    {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            if matches!(
                instantiated_fact_judgment(program, &judge, *expression, &map),
                StructuralJudgment::Refuted
            ) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` value call to `{}` violates required fact `{}`: the instantiated fact is structurally false",
                    caller_machine.name,
                    caller_state.name,
                    callee_machine.name,
                    program.expression_table.display_name(*expression),
                )));
            }
        }
    }
}

fn instantiated_fact_judgment(
    program: &TypedTrees,
    judge: &StructuralJudge,
    fact: ExpressionHandle,
    map: &[(String, StructuralTerm)],
) -> StructuralJudgment {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
        return StructuralJudgment::Unknown;
    };
    match binary.operator {
        BinaryOperator::And => {
            match (
                instantiated_fact_judgment(program, judge, binary.left, map),
                instantiated_fact_judgment(program, judge, binary.right, map),
            ) {
                (StructuralJudgment::Refuted, _) | (_, StructuralJudgment::Refuted) => {
                    StructuralJudgment::Refuted
                }
                (StructuralJudgment::Proven, StructuralJudgment::Proven) => {
                    StructuralJudgment::Proven
                }
                _ => StructuralJudgment::Unknown,
            }
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            let (Some(left), Some(right)) = (
                structural_term(program, binary.left),
                structural_term(program, binary.right),
            ) else {
                return StructuralJudgment::Unknown;
            };
            let equality = judge.judge_equation(
                judge.resolve(StructuralJudge::substitute_term(&left, map)),
                judge.resolve(StructuralJudge::substitute_term(&right, map)),
                0,
            );
            if binary.operator == BinaryOperator::Equal {
                equality
            } else {
                match equality {
                    StructuralJudgment::Proven => StructuralJudgment::Refuted,
                    StructuralJudgment::Refuted => StructuralJudgment::Proven,
                    StructuralJudgment::Unknown => StructuralJudgment::Unknown,
                }
            }
        }
        _ => StructuralJudgment::Unknown,
    }
}

/// Walk an ensures fact's `&&`-conjuncts; each `==` conjunct whose sides
/// term-ify yields one instantiated equation under the citation's
/// parameter map.
fn collect_instantiated_conjuncts(
    program: &TypedTrees,
    fact: ExpressionHandle,
    map: &[(String, StructuralTerm)],
    equations: &mut Vec<(StructuralTerm, StructuralTerm)>,
) {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
        return;
    };
    match binary.operator {
        BinaryOperator::And => {
            collect_instantiated_conjuncts(program, binary.left, map, equations);
            collect_instantiated_conjuncts(program, binary.right, map, equations);
        }
        BinaryOperator::Equal => {
            let (Some(left), Some(right)) = (
                structural_term(program, binary.left),
                structural_term(program, binary.right),
            ) else {
                return;
            };
            equations.push((
                StructuralJudge::substitute_term(&left, map),
                StructuralJudge::substitute_term(&right, map),
            ));
        }
        _ => {}
    }
}

/// N3 rung 1: the structural mini-judge for contract conjuncts over
/// proof-only data. Term language (bounded by today's contract grammar --
/// struct literals do not parse in fact position, so payload-carrying
/// constructor terms and their injectivity decomposition are the recorded
/// next rung): variables (single-segment names), nullary case classifiers
/// (`Nat::Zero`), and opaque applications compared only by display name.
/// `requires` equalities with a variable side become directed substitutions
/// (first binding wins; symmetry and transitivity fall out of resolution);
/// two distinct nullary cases equated make the hypotheses CONTRADICTORY and
/// every goal holds vacuously, mirroring the polynomial engine's rule.
/// One recognized case arm of a structurally-inductive proof machine.
struct StructuralCaseArm {
    /// The machine's short (call-target) name, for self-application search.
    machine_name: String,
    /// Entry parameter names, positionally matching self-call arguments.
    parameter_names: Vec<String>,
    /// Case refinements accumulated along the path to this leaf. A variable
    /// subject becomes a substitution to its constructor over FRESH payload
    /// variables. Nested case states contribute one refinement apiece.
    case_hypotheses: Vec<(String, StructuralTerm)>,
    /// COMPUTED-SUBJECT refinements accumulated along the path. A subject
    /// such as `saturating_sub(b, a)` cannot be a substitution, so it is retained as an
    /// equation to intake instead.
    case_equations: Vec<(StructuralTerm, StructuralTerm)>,
    /// PER-ARM CITATIONS (N3 rung 2): equations injected by citation
    /// statements in the arm's SUB-STATE, instantiated under the arm
    /// environment -- the only route to cite a lemma AT A CASE PAYLOAD
    /// (comm's step case cites add_succ_law at `prev`, which machine-level
    /// statements cannot see). Empty for direct value arms.
    citations: Vec<(StructuralTerm, StructuralTerm)>,
    /// The arm's value term, converted under the case environment (payload
    /// member reads resolve against the fresh-variable constructor).
    value: StructuralTerm,
}

/// Recognize the integer-measured structural-induction bridge: a total
/// two-way guarded value transition may build proof data in each branch even
/// though the guard itself is an ordinary integer proposition (`n > 0`).
/// Structural judging needs no interpretation of that proposition: it proves
/// the contract for BOTH exhaustive values.  Self-applications in either
/// value still become induction hypotheses only when the separate recursion
/// validator proves their declared measure decreases, so recognizing this
/// body shape cannot license an ungrounded induction.
fn recognize_guarded_structural_value_arms(
    program: &TypedTrees,
    machine: &Machine,
    judge: &StructuralJudge<'_>,
) -> Option<Vec<StructuralCaseArm>> {
    let [root] = program.machine_states(machine) else {
        return None;
    };
    let statements: Vec<&StatementNode> = program
        .statement_table
        .statements(root.statement_nodes)
        .iter()
        .filter(|statement| !is_arm_pattern_marker(statement))
        .collect();
    let [first, second] = statements.as_slice() else {
        return None;
    };
    let (StatementNode::Transition(first), StatementNode::Transition(second)) = (first, second)
    else {
        return None;
    };
    let branch = |transition: &psi_typed_trees::statement::TableTransition| {
        if transition.continuation.is_valid() || !transition.target.is_valid() {
            return None;
        }
        let TransitionGuardNode::When(guard) = transition.guard else {
            return None;
        };
        let ExpressionNode::Binary(equality) = program.expression_table.expression(guard) else {
            return None;
        };
        if equality.operator != BinaryOperator::Equal {
            return None;
        }
        let ExpressionNode::Boolean(polarity) = program.expression_table.expression(equality.right)
        else {
            return None;
        };
        Some((equality.left, *polarity, transition.target))
    };
    let (first_condition, first_polarity, first_target) = branch(first)?;
    let (second_condition, second_polarity, second_target) = branch(second)?;
    if first_polarity == second_polarity
        || !guard_expressions_equal(program, first_condition, second_condition)
    {
        return None;
    }

    let parameter_names: Vec<String> = program
        .state_parameters(root)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect();
    let environment: Vec<(String, StructuralTerm)> = parameter_names
        .iter()
        .map(|name| (name.clone(), StructuralTerm::Variable(name.clone())))
        .collect();
    let machine_name = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(machine.name.as_str())
        .to_owned();

    [first_target, second_target]
        .into_iter()
        .map(|target| {
            let TransitionTargetNode::Value(value) =
                program.statement_table.transition_target(target)
            else {
                return None;
            };
            Some(StructuralCaseArm {
                machine_name: machine_name.clone(),
                parameter_names: parameter_names.clone(),
                case_hypotheses: Vec::new(),
                case_equations: Vec::new(),
                citations: Vec::new(),
                value: judge.callee_term(*value, &environment, 0)?,
            })
        })
        .collect()
}

/// Equality for the duplicated condition trees produced by boolean-arm
/// lowering.  This deliberately recognizes only the pure scalar grammar
/// needed to prove that `condition == true` and `condition == false` are the
/// two faces of ONE condition; unsupported trees simply refuse the
/// structural-induction shortcut.
fn guard_expressions_equal(
    program: &TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => {
            left.value_i64() == right.value_i64()
        }
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            left.symbol == right.symbol
                && program.expression_table.name_path_members(left.members)
                    == program.expression_table.name_path_members(right.members)
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && guard_expressions_equal(program, left.left, right.left)
                && guard_expressions_equal(program, left.right, right.right)
        }
        _ => false,
    }
}

type PendingStructuralCitation = (psi_typed_trees::name::Identifier, Vec<StructuralTerm>);

/// Recognize a proof machine as a tree of structural case states. Each named
/// state can either terminate in a value or refine another subject and hand
/// the branch to a further named state. Unsupported statement order, an
/// unresolved target, or a state cycle fails the whole recognition closed.
fn recognize_structural_case_arms(
    program: &TypedTrees,
    machine: &Machine,
    judge: &StructuralJudge<'_>,
    classification: &psi_typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<StructuralCaseArm>> {
    let states = program.machine_states(machine);
    let root = states.first()?;
    let machine_name = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(machine.name.as_str())
        .to_owned();
    let parameter_names: Vec<String> = program
        .state_parameters(root)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect();
    let environment: Vec<(String, StructuralTerm)> = parameter_names
        .iter()
        .map(|name| (name.clone(), StructuralTerm::Variable(name.clone())))
        .collect();
    let mut path = Vec::new();
    let mut fresh = 0usize;
    let arms = recognize_structural_state_leaves(
        program,
        machine,
        judge,
        classification,
        diagnostics,
        states,
        root,
        &machine_name,
        &parameter_names,
        environment,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
        &mut path,
        &mut fresh,
    )?;
    (!arms.is_empty()).then_some(arms)
}

#[allow(clippy::too_many_arguments)]
fn recognize_structural_state_leaves(
    program: &TypedTrees,
    machine: &Machine,
    judge: &StructuralJudge<'_>,
    classification: &psi_typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    states: &[psi_typed_trees::state::State],
    state: &psi_typed_trees::state::State,
    machine_name: &str,
    parameter_names: &[String],
    mut environment: Vec<(String, StructuralTerm)>,
    case_hypotheses: Vec<(String, StructuralTerm)>,
    case_equations: Vec<(StructuralTerm, StructuralTerm)>,
    mut pending_citations: Vec<PendingStructuralCitation>,
    collect_citations: bool,
    path: &mut Vec<SymbolHandle>,
    fresh: &mut usize,
) -> Option<Vec<StructuralCaseArm>> {
    if !state.symbol.is_valid() || path.iter().any(|symbol| *symbol == state.symbol) {
        return None;
    }
    path.push(state.symbol);

    let result = (|| {
        let mut transitions = Vec::new();
        let mut saw_transition = false;
        for statement in program.statement_table.statements(state.statement_nodes) {
            if is_arm_pattern_marker(statement) {
                continue;
            }
            match statement {
                StatementNode::LocalData(local) if !saw_transition => {
                    let term = judge.callee_term(local.initial_value, &environment, 0)?;
                    if collect_citations
                        && let Some((target, argument_handles)) =
                            citation_call_in_statement(program, statement)
                    {
                        let argument_terms = argument_handles
                            .iter()
                            .map(|argument| judge.callee_term(*argument, &environment, 0))
                            .collect::<Option<Vec<_>>>()?;
                        pending_citations.push((target.clone(), argument_terms));
                    }
                    environment.push((local.name.as_str().to_owned(), term));
                }
                StatementNode::Call(_) if !saw_transition && collect_citations => {
                    let Some((target, argument_handles)) =
                        citation_call_in_statement(program, statement)
                    else {
                        return None;
                    };
                    let argument_terms = argument_handles
                        .iter()
                        .map(|argument| judge.callee_term(*argument, &environment, 0))
                        .collect::<Option<Vec<_>>>()?;
                    pending_citations.push((target.clone(), argument_terms));
                }
                StatementNode::Call(call)
                    if !saw_transition
                        && is_citation_statement(program, classification, machine, call) =>
                {
                    // Entry citations were already intaken machine-wide.
                }
                StatementNode::Transition(transition) => {
                    saw_transition = true;
                    if transition.continuation.is_valid() || !transition.target.is_valid() {
                        return None;
                    }
                    transitions.push(transition);
                }
                _ => return None,
            }
        }
        if transitions.is_empty()
            || (transitions.len() > 1
                && transitions
                    .iter()
                    .any(|transition| matches!(transition.guard, TransitionGuardNode::Always)))
        {
            return None;
        }

        let mut leaves = Vec::new();
        for transition in transitions {
            let mut branch_environment = environment.clone();
            let mut branch_hypotheses = case_hypotheses.clone();
            let mut branch_equations = case_equations.clone();
            if let TransitionGuardNode::When(guard) = transition.guard {
                let ExpressionNode::Binary(comparison) = program.expression_table.expression(guard)
                else {
                    return None;
                };
                if comparison.operator != BinaryOperator::Equal {
                    return None;
                }
                let raw_subject = structural_term(program, comparison.left)?;
                let subject = judge.callee_term(comparison.left, &branch_environment, 0)?;
                let Some(StructuralTerm::Constructor { data, case, fields }) =
                    structural_term(program, comparison.right)
                else {
                    return None;
                };
                if !fields.is_empty() {
                    return None;
                }
                let definition = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name.as_str() == data.as_str())?;
                let variant_fields: Vec<String> = program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        psi_typed_trees::data::DataMember::Variant(variant)
                            if variant.name.as_str() == case.as_str() =>
                        {
                            Some(
                                program
                                    .data_payload_fields(variant)
                                    .iter()
                                    .map(|field| field.name.as_str().to_owned())
                                    .collect(),
                            )
                        }
                        _ => None,
                    })?;
                let branch_id = *fresh;
                *fresh += 1;
                let mut fields: Vec<(String, StructuralTerm)> = variant_fields
                    .into_iter()
                    .map(|field| {
                        let variable =
                            format!("__ih_{}_{}_{}", state.name.as_str(), branch_id, field);
                        (field, StructuralTerm::Variable(variable))
                    })
                    .collect();
                fields.sort_by(|(left, _), (right, _)| left.cmp(right));
                let constructor = StructuralTerm::Constructor { data, case, fields };

                match subject {
                    StructuralTerm::Variable(subject) => {
                        branch_hypotheses.push((subject, constructor.clone()));
                    }
                    StructuralTerm::Application { .. } => {
                        branch_equations.push((subject, constructor.clone()));
                    }
                    _ => return None,
                }
                if let StructuralTerm::Variable(raw_name) = raw_subject {
                    let (_, binding) = branch_environment
                        .iter_mut()
                        .find(|(name, _)| name == &raw_name)?;
                    *binding = constructor;
                } else if matches!(raw_subject, StructuralTerm::Application { .. }) {
                    // A computed subject has no environment binding to refine.
                } else {
                    return None;
                }
            }

            match program.statement_table.transition_target(transition.target) {
                TransitionTargetNode::Value(value) => {
                    let value = judge.callee_term(*value, &branch_environment, 0)?;
                    leaves.push(finalize_structural_case_arm(
                        program,
                        machine,
                        judge,
                        classification,
                        diagnostics,
                        machine_name,
                        parameter_names,
                        branch_hypotheses,
                        branch_equations,
                        pending_citations.clone(),
                        value,
                    ));
                }
                TransitionTargetNode::Named {
                    path: target,
                    arguments,
                    ..
                } => {
                    let [state_name] = program.statement_table.name_path_members(target.members)
                    else {
                        return None;
                    };
                    let target_state = states[1..]
                        .iter()
                        .find(|candidate| candidate.name.as_str() == state_name.as_str())?;
                    let target_parameters = program.state_parameters(target_state);
                    let argument_handles = program.statement_table.expression_handles(*arguments);
                    if target_parameters.len() != argument_handles.len() {
                        return None;
                    }
                    let target_environment = target_parameters
                        .iter()
                        .zip(argument_handles)
                        .map(|(parameter, argument)| {
                            Some((
                                parameter.name.as_str().to_owned(),
                                judge.callee_term(*argument, &branch_environment, 0)?,
                            ))
                        })
                        .collect::<Option<Vec<_>>>()?;
                    leaves.extend(recognize_structural_state_leaves(
                        program,
                        machine,
                        judge,
                        classification,
                        diagnostics,
                        states,
                        target_state,
                        machine_name,
                        parameter_names,
                        target_environment,
                        branch_hypotheses,
                        branch_equations,
                        pending_citations.clone(),
                        true,
                        path,
                        fresh,
                    )?);
                }
                _ => return None,
            }
        }
        Some(leaves)
    })();

    path.pop();
    result
}

#[allow(clippy::too_many_arguments)]
fn finalize_structural_case_arm(
    program: &TypedTrees,
    machine: &Machine,
    judge: &StructuralJudge<'_>,
    classification: &psi_typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    machine_name: &str,
    parameter_names: &[String],
    case_hypotheses: Vec<(String, StructuralTerm)>,
    case_equations: Vec<(StructuralTerm, StructuralTerm)>,
    pending_citations: Vec<PendingStructuralCitation>,
    value: StructuralTerm,
) -> StructuralCaseArm {
    let mut arm_judge = judge.clone();
    for (subject, constructor) in &case_equations {
        arm_judge.intake_equation(subject.clone(), constructor.clone(), 0);
    }
    for (subject, constructor) in &case_hypotheses {
        arm_judge
            .substitutions
            .insert(0, (subject.clone(), constructor.clone()));
    }
    let requires = machine_requires_facts(program, machine);
    let mut vacuous = requires
        .iter()
        .any(|fact| matches!(arm_judge.judge(program, *fact), StructuralJudgment::Refuted));
    for fact in &requires {
        arm_judge.intake(program, *fact);
    }
    vacuous = vacuous || arm_judge.hypotheses_contradictory;

    // The induction hypothesis is available before an authored citation only
    // when the recursive application's own preconditions are already proven
    // at that point.  This is the conditional theorem rule: a requires-bearing
    // self-call never contributes its ensures merely because it descends.
    // Membership premises remain outside the structural language and suppress
    // the IH entirely.
    intake_available_self_induction_hypotheses(
        program,
        machine,
        machine_name,
        parameter_names,
        &value,
        &mut arm_judge,
    );

    let mut citations = Vec::new();
    if !vacuous {
        for (target, arguments) in pending_citations {
            let before = citations.len();
            instantiate_citation(
                program,
                classification,
                machine,
                &target,
                &arguments,
                diagnostics,
                &mut citations,
                Some(&arm_judge),
            );
            for (left, right) in &citations[before..] {
                arm_judge.intake_equation(left.clone(), right.clone(), 0);
            }
            // An earlier citation may establish a recursive application's
            // requires, making its conditional IH available to later
            // citations in the same authored statement order.
            intake_available_self_induction_hypotheses(
                program,
                machine,
                machine_name,
                parameter_names,
                &value,
                &mut arm_judge,
            );
        }
    }

    StructuralCaseArm {
        machine_name: machine_name.to_owned(),
        parameter_names: parameter_names.to_vec(),
        case_hypotheses,
        case_equations,
        citations,
        value,
    }
}

/// Intake every structurally descending self-application's ensures whose
/// requires are proven in `judge` at the current statement boundary.  The
/// recursion validator separately licenses descent; this helper only governs
/// the conditional contract attached to that already-licensed application.
fn intake_available_self_induction_hypotheses(
    program: &TypedTrees,
    machine: &Machine,
    machine_name: &str,
    parameter_names: &[String],
    value: &StructuralTerm,
    judge: &mut StructuralJudge<'_>,
) {
    let mut requires = Vec::new();
    let mut requires_out_of_language = false;
    let mut ensures = Vec::new();
    for contract in program.machine_contracts(machine) {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match (&contract.kind, fact) {
                (SignatureContractKind::Requires, ProofFact::Expression(expression)) => {
                    requires.push(*expression);
                }
                (SignatureContractKind::Requires, ProofFact::Membership(_)) => {
                    requires_out_of_language = true;
                }
                (SignatureContractKind::Requires, ProofFact::Proposition(_)) => {
                    requires_out_of_language = true;
                }
                (SignatureContractKind::Ensures, ProofFact::Expression(expression)) => {
                    ensures.push(*expression);
                }
                _ => {}
            }
        }
    }
    if requires_out_of_language {
        return;
    }

    let mut applications = Vec::new();
    StructuralJudge::self_applications(value, machine_name, &mut applications);
    for application in applications {
        let StructuralTerm::Application { arguments, .. } = &application else {
            continue;
        };
        let mut map: Vec<(String, StructuralTerm)> = parameter_names
            .iter()
            .cloned()
            .zip(arguments.iter().cloned())
            .collect();
        let requirements_established = requires
            .iter()
            .all(|fact| instantiated_fact_established(program, judge, *fact, &map));
        if !requirements_established {
            continue;
        }
        map.push((RESULT_BINDER.to_owned(), application.clone()));
        for fact in &ensures {
            let mut equations = Vec::new();
            collect_instantiated_conjuncts(program, *fact, &map, &mut equations);
            for (left, right) in equations {
                judge.intake_equation(left, right, 0);
            }
        }
    }
}

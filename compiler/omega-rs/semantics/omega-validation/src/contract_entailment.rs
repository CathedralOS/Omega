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

use omega_core::bignum::BigInt;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::{SignatureContractKind, StateSignature};
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use omega_typed_trees::trait_definition::TraitDefinition;

/// The reserved binder naming a machine's return value inside `ensures`
/// facts. Matches the call-site substitution rule in the checked-trees
/// contract prover: a single-segment `result` that does not shadow a real
/// parameter denotes the produced value.
const RESULT_BINDER: &str = "result";

/// Arm-pattern exhaustiveness markers (`__arm_destructure#...` locals) are
/// VALIDATION carriers minted by the transition parser, not body shape:
/// every proof-side statement-shape walk steps over them, the same way
/// citation statements are stepped over.
fn is_arm_pattern_marker(statement: &StatementNode) -> bool {
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
    let mut requires = Vec::new();
    let mut ensures = Vec::new();
    // Membership facts (`value in Domain`) are outside the engine's language.
    // The empty-body path drops them silently (a dropped hypothesis only
    // weakens proving power); the inductive path additionally refuses to
    // REJECT when any are present, since the unread fact could entail the
    // goal.
    let mut all_facts_are_expressions = true;
    for contract in program.machine_contracts(machine) {
        let bucket = match contract.kind {
            SignatureContractKind::Requires => &mut requires,
            SignatureContractKind::Ensures => &mut ensures,
            SignatureContractKind::Boundary => continue,
        };
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match fact {
                ProofFact::Expression(expression) => bucket.push(*expression),
                ProofFact::Membership(_) => all_facts_are_expressions = false,
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
    let proof_only = omega_typed_trees::proof_only::classify(program);
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
    // CITATIONS (ch10 "Citing Proofs"; the OWNER_QUESTIONS #14 answer):
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
        if let Some(proven) = quotient_equality_from_requires(program, &requires, fact) {
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
            let mention = fact_mentions_proof_only_data(program, &proof_only, machine, *fact);
            if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                eprintln!(
                    "ROUTE machine={} fact=`{}` mention={:?}",
                    machine.name,
                    program.expression_table.display_name(*fact),
                    mention.as_ref().map(|name| name.as_str()),
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
            if machine.supply_mode == omega_core::semantics::MachineSupplyMode::Accepted {
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
            fact_mentions_proof_only_data(program, &proof_only, machine, *fact).is_some()
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
                }
                // Otherwise the contract leans on facts outside the engine's
                // language (domain membership, unknown calls, non-parameter
                // places): stand down rather than reject what we cannot read.
            }
        }
    }
}

/// N6 quotient congruence: equality of two quotient mints is exactly the
/// quotient relation over their carrier expressions. `Some(false)` means the
/// goal is a well-formed quotient equality but its relation premise is absent;
/// callers reject it instead of letting the generic structural tier stand down.
fn quotient_equality_from_requires(
    program: &TypedTrees,
    requires: &[ExpressionHandle],
    fact: ExpressionHandle,
) -> Option<bool> {
    let (quotient, left, right) = quotient_equality_goal(program, fact)?;
    let relation = quotient.quotient.as_ref()?;
    Some(requires.iter().any(|required| {
        relation_fact_call(program, *required).is_some_and(|call| {
            relation_call_matches_quotient(program, call, relation.relation_symbol)
                && matches!(
                    program.expression_table.expression_handles(call.arguments),
                    [required_left, required_right]
                        if (program.expression_table.expressions_structurally_equal(*required_left, left)
                            && program.expression_table.expressions_structurally_equal(*required_right, right))
                            || (program.expression_table.expressions_structurally_equal(*required_left, right)
                                && program.expression_table.expressions_structurally_equal(*required_right, left))
                )
        })
    }))
}

fn quotient_equality_names(
    program: &TypedTrees,
    fact: ExpressionHandle,
) -> Option<(String, String)> {
    let (definition, _, _) = quotient_equality_goal(program, fact)?;
    let quotient = definition.quotient.as_ref()?;
    Some((
        definition.name.as_str().to_owned(),
        quotient
            .relation
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::"),
    ))
}

fn quotient_equality_goal(
    program: &TypedTrees,
    fact: ExpressionHandle,
) -> Option<(
    &omega_typed_trees::data::DataDefinition,
    ExpressionHandle,
    ExpressionHandle,
)> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
        return None;
    };
    if binary.operator != BinaryOperator::Equal {
        return None;
    }
    let (ExpressionNode::Cast(left), ExpressionNode::Cast(right)) = (
        program.expression_table.expression(binary.left),
        program.expression_table.expression(binary.right),
    ) else {
        return None;
    };
    if left.form.is_recast() || right.form.is_recast() {
        return None;
    }
    let left_name = program.named_type_reference(left.target_type)?;
    let right_name = program.named_type_reference(right.target_type)?;
    if left_name.as_str() != right_name.as_str() {
        return None;
    }
    let quotient = program.data_definitions().iter().find(|definition| {
        definition.name.as_str() == left_name.as_str() && definition.quotient.is_some()
    })?;
    Some((quotient, left.value, right.value))
}

fn relation_fact_call(
    program: &TypedTrees,
    fact: ExpressionHandle,
) -> Option<&omega_typed_trees::expression::TableCallExpression> {
    match program.expression_table.expression(fact) {
        ExpressionNode::Call(call) => Some(call),
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) {
                relation_fact_call(program, binary.left)
            } else if matches!(
                program.expression_table.expression(binary.left),
                ExpressionNode::Boolean(true)
            ) {
                relation_fact_call(program, binary.right)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn relation_call_matches_quotient(
    program: &TypedTrees,
    call: &omega_typed_trees::expression::TableCallExpression,
    relation_symbol: omega_core::symbols::SymbolHandle,
) -> bool {
    if call.target_symbol == relation_symbol {
        return true;
    }
    let relation_name = program
        .data_definitions()
        .iter()
        .filter_map(|definition| definition.quotient.as_ref())
        .find(|quotient| quotient.relation_symbol == relation_symbol)
        .and_then(|quotient| quotient.relation.last())
        .map(|name| name.as_str());
    if relation_name.is_some_and(|name| call.target.as_str() == name) {
        return true;
    }
    program.machines().iter().any(|machine| {
        (machine.symbol == relation_symbol
            || program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == relation_symbol))
            && program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == call.target_symbol)
    })
}

/// One recognized transition arm of an inductive machine body.
struct TransitionArm {
    /// The arm's guard expression (`None` for an always-firing arm, which
    /// contributes no path fact).
    guard: Option<ExpressionHandle>,
    kind: ArmKind,
}

enum ArmKind {
    /// `guard -> value`: the machine exits producing `value`.
    Value(ExpressionHandle),
    /// `guard -> self.this_machine(args)`: tail self-recursion. The arm's
    /// result IS the recursive call's result, so the induction hypothesis and
    /// the goal share the `result` atom.
    TailSelfCall(Vec<ExpressionHandle>),
}

/// L7: judge the ensures of a machine whose body is a chain of guarded
/// value / tail-self-call transitions. Anything outside that recognized shape
/// stands down (bodied machines were previously never judged here, so this
/// path only ever ADDS judgments for the shape it fully reads).
fn inductive_transition_entailment(
    program: &TypedTrees,
    machine: &Machine,
    requires: &[ExpressionHandle],
    ensures: &[ExpressionHandle],
    all_facts_are_expressions: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Single-state machines only: the state graph IS the recursion structure,
    // and a tail self-call is a transition back to the root state.
    let states = program.machine_states(machine);
    let [root] = states else {
        return;
    };
    let statements = program.statement_table.statements(root.statement_nodes);
    if statements.is_empty() {
        return;
    }
    let mut arms = Vec::new();
    for statement in statements {
        if is_arm_pattern_marker(statement) {
            continue; // exhaustiveness carrier, not shape
        }
        let StatementNode::Transition(transition) = statement else {
            return; // assignments / locals / calls: out of shape, stand down
        };
        if transition.continuation.is_valid() {
            return;
        }
        let guard = match transition.guard {
            TransitionGuardNode::When(guard) => Some(guard),
            TransitionGuardNode::Always => None,
        };
        let target = program.statement_table.transition_target(transition.target);
        let kind = match target {
            TransitionTargetNode::Value(value) => ArmKind::Value(*value),
            TransitionTargetNode::Named { path, arguments } if path.symbol == root.symbol => {
                ArmKind::TailSelfCall(
                    program
                        .statement_table
                        .expression_handles(*arguments)
                        .to_vec(),
                )
            }
            // Transitions to other states (or `self` / terminal targets) are
            // outside the recognized inductive shape.
            _ => return,
        };
        arms.push(TransitionArm { guard, kind });
    }

    let trace = std::env::var("OMEGA_ENTAILMENT_TRACE").is_ok();
    let mut judged_arms = Vec::new();
    let mut every_arm_visible = true;
    for arm in &arms {
        let Some(judged) = prepare_arm(program, machine, root, requires, ensures, arm) else {
            // The arm's value or argument list is unreadable: the whole body
            // cannot be anchored, so nothing can be judged or rejected.
            return;
        };
        every_arm_visible &= judged.fully_visible;
        judged_arms.push(judged);
    }

    let machine_fully_visible = all_facts_are_expressions && every_arm_visible;
    for fact in ensures {
        let mut constant_false_arm = None;
        let mut refuted_arm = None;
        let mut unknown_arm = None;
        let mut goal_always_in_language = true;
        for arm in &mut judged_arms {
            if arm.vacuous {
                // The arm's visible facts are contradictory: that arm is
                // unreachable, so its obligation holds vacuously.
                continue;
            }
            let judgment = arm.engine.judge(*fact);
            if trace {
                eprintln!(
                    "ENTAILDBG inductive machine={} arm_guard={} fact={} visible={}",
                    machine.name,
                    arm.guard_display,
                    program.expression_table.display_name(*fact),
                    arm.fully_visible,
                );
            }
            match judgment {
                Judgment::Proven => {}
                Judgment::ConstantFalse => {
                    constant_false_arm.get_or_insert(arm.guard_display.clone());
                }
                Judgment::Refuted => {
                    refuted_arm.get_or_insert(arm.guard_display.clone());
                }
                Judgment::Unknown { goal_in_language } => {
                    unknown_arm.get_or_insert(arm.guard_display.clone());
                    goal_always_in_language &= goal_in_language;
                }
            }
        }
        if let Some(guard) = constant_false_arm {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` ensures contract proof fact `{}` is disproved by constant arithmetic on the transition arm guarded by `{}`",
                machine.name,
                program.expression_table.display_name(*fact),
                guard
            )));
        } else if let Some(guard) = refuted_arm {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` ensures contract proof fact `{}` is disproved on the transition arm guarded by `{}`: the arm's facts entail its negation",
                machine.name,
                program.expression_table.display_name(*fact),
                guard
            )));
        } else if let Some(guard) = unknown_arm
            && goal_always_in_language
            && machine_fully_visible
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` cannot prove ensures contract proof fact `{}` on the transition arm guarded by `{}`",
                machine.name,
                program.expression_table.display_name(*fact),
                guard
            )));
        }
        // An unknown that is not fully visible means some fact lies outside
        // the engine's language: stand down rather than reject what we
        // cannot fully read.
    }
}

/// An arm with its proof context installed, ready to judge ensures goals.
struct JudgedArm<'program> {
    engine: Engine<'program>,
    /// The arm's visible facts are unsatisfiable (unreachable arm).
    vacuous: bool,
    /// Every hypothesis the arm relies on was readable -- including, for a
    /// self-call arm, a DISCHARGED strict decrease and a fully instantiated
    /// induction hypothesis. Only fully visible arms may drive rejection.
    fully_visible: bool,
    guard_display: String,
}

fn prepare_arm<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    root: &omega_typed_trees::state::State,
    requires: &[ExpressionHandle],
    ensures: &[ExpressionHandle],
    arm: &TransitionArm,
) -> Option<JudgedArm<'program>> {
    let guard_display = match arm.guard {
        Some(guard) => program.expression_table.display_name(guard),
        None => "<always>".to_owned(),
    };

    let mut engine = Engine::with_result_atom(program, machine, root);
    let mut comparisons = Vec::new();
    let mut fully_visible = engine.collect_comparisons(requires, &mut comparisons);
    engine.collect_entry_range_hypotheses(&mut comparisons);

    // The arm's path condition: the guard with its boolean polarity applied
    // (`expr == false` lowers each dispatch arm; the negated comparison is the
    // arm's fact). An unreadable guard only weakens proving power.
    if let Some(guard) = arm.guard {
        match guard_arm_comparison(&mut engine, guard) {
            Some(comparison) => comparisons.push(comparison),
            None => fully_visible = false,
        }
    }

    let value = match &arm.kind {
        ArmKind::Value(value) => {
            // Bind `result` to the arm's value, so ensures goals over
            // `result` ground out in parameter terms.
            let polynomial = engine.normalize(*value)?;
            Some(polynomial)
        }
        ArmKind::TailSelfCall(arguments) => {
            // INDUCTION: the recursive call's instantiated ensures may enter
            // the arm's facts, exactly as a nested callee's ensures would,
            // PROVIDED a strict decrease is discharged at this exact call
            // site. Build the gate context first (requires + guard, no
            // hypothesis), then instantiate.
            let mut gate_engine = Engine::with_result_atom(program, machine, root);
            let mut gate_comparisons = Vec::new();
            gate_engine.collect_comparisons(requires, &mut gate_comparisons);
            gate_engine.collect_entry_range_hypotheses(&mut gate_comparisons);
            if let Some(guard) = arm.guard
                && let Some(comparison) = guard_arm_comparison(&mut gate_engine, guard)
            {
                gate_comparisons.push(comparison);
            }
            gate_engine.install_hypotheses(gate_comparisons);

            let argument_map = self_call_argument_map(&mut engine, root, arguments)?;
            if gate_engine.requires_unsatisfiable {
                // Unreachable arm: vacuous regardless of the hypothesis.
            } else if discharges_strict_decrease(program, machine, &mut gate_engine, &argument_map)
            {
                // Instantiate the machine's own ensures over the call's
                // arguments; `result` stays shared (the arm's result IS the
                // call's result). Conjuncts the engine cannot instantiate are
                // dropped -- a weaker hypothesis is sound but blocks
                // rejection.
                for fact in ensures {
                    for conjunct in engine.conjuncts(*fact) {
                        match instantiated_hypothesis(&mut engine, conjunct, &argument_map) {
                            Some(comparison) => comparisons.push(comparison),
                            None => fully_visible = false,
                        }
                    }
                }
            } else {
                // No discharged decrease at this call site: NO induction
                // hypothesis. The goal must prove some other way; rejection
                // is suppressed because the missing hypothesis (not the
                // theorem) may be at fault.
                fully_visible = false;
            }
            None
        }
    };

    if let Some(polynomial) = value {
        engine
            .substitutions
            .insert(RESULT_BINDER.to_owned(), polynomial);
    }
    fully_visible &= engine.install_hypotheses(comparisons);
    let vacuous = engine.requires_unsatisfiable;
    Some(JudgedArm {
        engine,
        vacuous,
        fully_visible,
        guard_display,
    })
}

/// Read a transition arm's guard as a comparison fact. The dispatch lowering
/// wraps each arm's guard as `scrutinee == true` / `scrutinee == false`;
/// unwrap the wrapper and fold the polarity into the comparison operator.
fn guard_arm_comparison(
    engine: &mut Engine<'_>,
    guard: ExpressionHandle,
) -> Option<(BinaryOperator, Polynomial, Polynomial)> {
    let node = engine.program.expression_table.expression(guard).clone();
    if let ExpressionNode::Binary(binary) = &node
        && binary.operator == BinaryOperator::Equal
        && let ExpressionNode::Boolean(polarity) =
            engine.program.expression_table.expression(binary.right)
    {
        let polarity = *polarity;
        let (operator, left, right) = engine.comparison_polynomials(binary.left)?;
        let operator = if polarity {
            operator
        } else {
            negated_comparison(operator)?
        };
        return Some((operator, left, right));
    }
    engine.comparison_polynomials(guard)
}

/// The classical negation of a comparison operator (integer semantics).
fn negated_comparison(operator: BinaryOperator) -> Option<BinaryOperator> {
    match operator {
        BinaryOperator::Equal => Some(BinaryOperator::NotEqual),
        BinaryOperator::NotEqual => Some(BinaryOperator::Equal),
        BinaryOperator::Less => Some(BinaryOperator::GreaterOrEqual),
        BinaryOperator::LessOrEqual => Some(BinaryOperator::Greater),
        BinaryOperator::Greater => Some(BinaryOperator::LessOrEqual),
        BinaryOperator::GreaterOrEqual => Some(BinaryOperator::Less),
        _ => None,
    }
}

/// Positional map from the machine's non-self parameters to the recursive
/// call's argument polynomials. `None` when an argument is outside the
/// engine's language.
fn self_call_argument_map(
    engine: &mut Engine<'_>,
    root: &omega_typed_trees::state::State,
    arguments: &[ExpressionHandle],
) -> Option<BTreeMap<String, Polynomial>> {
    let parameters: Vec<String> = engine
        .program
        .state_parameters(root)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect();
    if parameters.len() != arguments.len() {
        return None;
    }
    let mut map = BTreeMap::new();
    for (name, argument) in parameters.into_iter().zip(arguments) {
        let polynomial = engine.normalize(*argument)?;
        map.insert(name, polynomial);
    }
    Some(map)
}

/// SOUNDNESS GATE for the induction hypothesis: prove, from the arm's own
/// facts (requires + guard, no hypothesis), that the declared `decreases`
/// measure is strictly smaller at the recursive call AND still non-negative
/// there. A strictly decreasing integer measure bounded below by zero admits
/// no infinite descent, which is exactly the well-foundedness that justifies
/// assuming the contract for the smaller instance. Only the polynomial
/// readings are verified here: the plain descending-naturals order
/// (`decreases value`, or explicitly `-> Nat::Descending`) and the named
/// bounded distance (the argumented tuple `decreases (lower, upper)`, or
/// explicitly `-> Nat::BoundedDistance`), whose distance polynomial
/// `upper - lower` goes through the identical strict-decrease +
/// non-negativity check. Other view and declared-measure orders have meanings
/// the polynomial engine cannot read, so they never gate a hypothesis in. The
/// machine-level termination pass independently re-checks the declared clause
/// and fails compilation when it cannot.
fn discharges_strict_decrease(
    program: &TypedTrees,
    machine: &Machine,
    engine: &mut Engine<'_>,
    argument_map: &BTreeMap<String, Polynomial>,
) -> bool {
    // TPR3 slice 1: the hypothesis gate keys on the WITNESS (a measured
    // body), read from the normalized plan (decision 23); the compatibility
    // bools agree by construction until TPR6 retires them.
    if machine.termination_plan.implementation_witness.is_none() {
        return false;
    }
    let decreases = program
        .expression_table
        .expression_handles(machine.decreases);
    let order = program.machine_decrease_order(machine.decrease_order);
    // TPR3: the argumented `Nat::IncreasingTo(limit)` is polynomial too --
    // its measure is the distance `limit - subject` with the bound taken
    // from the view's argument.
    let increasing_to =
        order.len() == 2 && order[0].as_str() == "Nat" && order[1].as_str() == "IncreasingTo";
    let polynomial_order = increasing_to
        || order.is_empty()
        || (order.len() == 2
            && order[0].as_str() == "Nat"
            && matches!(order[1].as_str(), "Descending" | "BoundedDistance"));
    if !polynomial_order {
        return false;
    }
    let measure = if increasing_to {
        let arguments = program
            .expression_table
            .expression_handles(machine.decrease_view_arguments);
        match (decreases, arguments) {
            ([subject], [limit]) => engine
                .normalize(*limit)
                .zip(engine.normalize(*subject))
                .map(|(limit, subject)| limit.sub(&subject)),
            _ => None,
        }
    } else {
        match decreases {
            [single] => engine.normalize(*single),
            // The two-subject bounded distance: the subjects bind in order to the
            // view's (lower, upper) parameters and the measure polynomial is the
            // distance `upper - lower`.
            [lower, upper] => engine
                .normalize(*upper)
                .zip(engine.normalize(*lower))
                .map(|(upper, lower)| upper.sub(&lower)),
            _ => None,
        }
    };
    let Some(measure) = measure else {
        return false;
    };
    let Some(measure_after) = apply_argument_map(&measure, argument_map) else {
        return false;
    };
    let measure_now = engine.substituted(&measure);
    let measure_after = engine.substituted(&measure_after);
    let difference = measure_now.sub(&measure_after);
    engine.prove_at_least(&difference, &BigInt::from_i64(1))
        && engine.prove_at_least(&measure_after, &BigInt::zero())
}

/// One ensures conjunct instantiated over the recursive call's arguments:
/// parameter atoms are replaced (simultaneously) by argument polynomials and
/// `result` is kept shared. This is the induction hypothesis the arm may
/// assume once the decrease gate has discharged.
fn instantiated_hypothesis(
    engine: &mut Engine<'_>,
    conjunct: ExpressionHandle,
    argument_map: &BTreeMap<String, Polynomial>,
) -> Option<(BinaryOperator, Polynomial, Polynomial)> {
    let (operator, left, right) = engine.comparison_polynomials(conjunct)?;
    let left = apply_argument_map(&left, argument_map)?;
    let right = apply_argument_map(&right, argument_map)?;
    Some((operator, left, right))
}

/// Simultaneous single-pass substitution of parameter atoms by argument
/// polynomials. Single-pass is essential: arguments mention the same
/// parameters (`n -> n - 1`), so a fixpoint application would telescope.
/// Atoms that are neither mapped parameters nor `result` (mod-term and
/// proof-view atoms embed parameter names in their rendered form) cannot be
/// instantiated and fail the substitution.
fn apply_argument_map(
    polynomial: &Polynomial,
    argument_map: &BTreeMap<String, Polynomial>,
) -> Option<Polynomial> {
    let mut result = Polynomial::default();
    for (monomial, coefficient) in &polynomial.terms {
        let mut piece = Polynomial::constant(coefficient.clone());
        for (atom, power) in monomial {
            let base = if let Some(replacement) = argument_map.get(atom) {
                replacement.clone()
            } else if atom == RESULT_BINDER {
                Polynomial::atom(atom.clone())
            } else {
                return None;
            };
            for _ in 0..*power {
                piece = piece.checked_mul(&base)?;
            }
        }
        result = result.add(&piece);
    }
    Some(result)
}

enum Judgment {
    Proven,
    /// Disproved purely by folding both sides to constants.
    ConstantFalse,
    /// The visible requires facts prove the goal's negation.
    Refuted,
    Unknown {
        goal_in_language: bool,
    },
}

/// A monomial: atoms (by canonical display name) to powers. Empty = the
/// constant monomial.
type Monomial = BTreeMap<String, u32>;

/// A polynomial: monomials to EXACT BigInt coefficients (math roster N2:
/// coefficient arithmetic never overflows, so a provable goal never
/// downgrades to "unknown" by width). Zero coefficients are never stored,
/// so structural equality is polynomial identity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Polynomial {
    terms: BTreeMap<Monomial, BigInt>,
}

impl Polynomial {
    fn constant(value: BigInt) -> Self {
        let mut polynomial = Self::default();
        if !value.is_zero() {
            polynomial.terms.insert(Monomial::new(), value);
        }
        polynomial
    }

    fn atom(name: String) -> Self {
        let mut monomial = Monomial::new();
        monomial.insert(name, 1);
        let mut polynomial = Self::default();
        polynomial.terms.insert(monomial, BigInt::from_i64(1));
        polynomial
    }

    fn constant_value(&self) -> Option<BigInt> {
        match self.terms.len() {
            0 => Some(BigInt::zero()),
            1 => self.terms.get(&Monomial::new()).cloned(),
            _ => None,
        }
    }

    fn add(&self, other: &Self) -> Self {
        let mut terms = self.terms.clone();
        for (monomial, coefficient) in &other.terms {
            let entry = terms.entry(monomial.clone()).or_insert_with(BigInt::zero);
            *entry = entry.add(coefficient);
            if entry.is_zero() {
                terms.remove(monomial);
            }
        }
        Self { terms }
    }

    fn neg(&self) -> Self {
        let mut terms = BTreeMap::new();
        for (monomial, coefficient) in &self.terms {
            terms.insert(monomial.clone(), coefficient.negate());
        }
        Self { terms }
    }

    fn sub(&self, other: &Self) -> Self {
        self.add(&other.neg())
    }

    /// Coefficients are exact; the only remaining failure is monomial POWER
    /// overflow (u32), which no writable program reaches.
    fn checked_mul(&self, other: &Self) -> Option<Self> {
        let mut result = Self::default();
        for (left_monomial, left_coefficient) in &self.terms {
            for (right_monomial, right_coefficient) in &other.terms {
                let coefficient = left_coefficient.mul(right_coefficient);
                let mut monomial = left_monomial.clone();
                for (atom, power) in right_monomial {
                    let entry = monomial.entry(atom.clone()).or_insert(0);
                    *entry = entry.checked_add(*power)?;
                }
                let entry = result
                    .terms
                    .entry(monomial.clone())
                    .or_insert_with(BigInt::zero);
                *entry = entry.add(&coefficient);
                if entry.is_zero() {
                    result.terms.remove(&monomial);
                }
            }
        }
        Some(result)
    }

    /// `(difference-of-two-unit-atoms, constant)`: `a - b + c` as
    /// `Some((a, b, c))`. The shape the difference-bound matrix consumes.
    fn as_atom_difference(&self) -> Option<(String, String, BigInt)> {
        let mut positive = None;
        let mut negative = None;
        let mut constant = BigInt::zero();
        for (monomial, coefficient) in &self.terms {
            if monomial.is_empty() {
                constant = coefficient.clone();
                continue;
            }
            if monomial.len() != 1 || *monomial.values().next().unwrap() != 1 {
                return None;
            }
            let atom = monomial.keys().next().unwrap().clone();
            if *coefficient == BigInt::from_i64(1) && positive.is_none() {
                positive = Some(atom);
            } else if *coefficient == BigInt::from_i64(-1) && negative.is_none() {
                negative = Some(atom);
            } else {
                return None;
            }
        }
        Some((positive?, negative?, constant))
    }

    /// `(single-unit-atom, coefficient-sign, constant)` for bounds like
    /// `a + c >= 0` / `-a + c >= 0`.
    fn as_single_atom(&self) -> Option<(String, i64, BigInt)> {
        let mut atom = None;
        let mut coefficient_value = BigInt::zero();
        let mut constant = BigInt::zero();
        for (monomial, coefficient) in &self.terms {
            if monomial.is_empty() {
                constant = coefficient.clone();
                continue;
            }
            if monomial.len() != 1 || *monomial.values().next().unwrap() != 1 || atom.is_some() {
                return None;
            }
            atom = Some(monomial.keys().next().unwrap().clone());
            coefficient_value = coefficient.clone();
        }
        let atom = atom?;
        let sign = if coefficient_value == BigInt::from_i64(1) {
            1
        } else if coefficient_value == BigInt::from_i64(-1) {
            -1
        } else {
            return None;
        };
        Some((atom, sign, constant))
    }
}

/// An interval with optional (= unbounded) ends; end arithmetic is exact.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Interval {
    low: Option<BigInt>,
    high: Option<BigInt>,
}

impl Interval {
    fn unbounded() -> Self {
        Self {
            low: None,
            high: None,
        }
    }

    fn constant(value: BigInt) -> Self {
        Self {
            low: Some(value.clone()),
            high: Some(value),
        }
    }

    fn add(&self, other: &Self) -> Self {
        Self {
            low: match (&self.low, &other.low) {
                (Some(a), Some(b)) => Some(a.add(b)),
                _ => None,
            },
            high: match (&self.high, &other.high) {
                (Some(a), Some(b)) => Some(a.add(b)),
                _ => None,
            },
        }
    }

    fn scale(&self, factor: &BigInt) -> Self {
        let scaled_low = self.low.as_ref().map(|value| value.mul(factor));
        let scaled_high = self.high.as_ref().map(|value| value.mul(factor));
        if factor.is_negative() {
            Self {
                low: scaled_high,
                high: scaled_low,
            }
        } else {
            Self {
                low: scaled_low,
                high: scaled_high,
            }
        }
    }

    fn multiply(&self, other: &Self) -> Self {
        // An unbounded end makes the product unbounded on the side it could
        // extend; with all four ends finite the corner products are exact.
        let (Some(self_low), Some(self_high), Some(other_low), Some(other_high)) =
            (&self.low, &self.high, &other.low, &other.high)
        else {
            return Interval::unbounded();
        };
        let candidates = [
            self_low.mul(other_low),
            self_low.mul(other_high),
            self_high.mul(other_low),
            self_high.mul(other_high),
        ];
        Self {
            low: candidates.iter().min().cloned(),
            high: candidates.iter().max().cloned(),
        }
    }

    /// `self` raised to `power`, treating repeated factors as CORRELATED:
    /// an even power of any interval is non-negative, and the square of
    /// `[lo, hi]` is exact rather than the independent product.
    fn correlated_power(&self, power: u32) -> Self {
        if power == 0 {
            return Self::constant(BigInt::from_i64(1));
        }
        if power == 1 {
            return self.clone();
        }
        let (Some(low), Some(high)) = (&self.low, &self.high) else {
            // Unbounded base: an even power is still known non-negative.
            return if power % 2 == 0 {
                Self {
                    low: Some(BigInt::zero()),
                    high: None,
                }
            } else {
                Interval::unbounded()
            };
        };
        let corner_low = pow(low, power);
        let corner_high = pow(high, power);
        if power % 2 == 1 {
            return Self {
                low: Some(corner_low),
                high: Some(corner_high),
            };
        }
        let max_corner = corner_low.clone().max(corner_high.clone());
        let min_corner = if !low.is_negative() || high.is_negative() {
            corner_low.min(corner_high)
        } else {
            // The base interval straddles zero: the even power bottoms at 0.
            BigInt::zero()
        };
        Self {
            low: Some(min_corner),
            high: Some(max_corner),
        }
    }
}

fn pow(base: &BigInt, power: u32) -> BigInt {
    let mut result = BigInt::from_i64(1);
    for _ in 0..power {
        result = result.mul(base);
    }
    result
}

struct Engine<'program> {
    program: &'program TypedTrees,
    /// The machine this engine judges (entry-range hypotheses resolve
    /// through it).
    machine_symbol: SymbolHandle,
    /// Canonical atom names for the machine's parameters.
    parameter_atoms: Vec<String>,
    /// Parameters whose primitive type is unsigned carry an implicit `>= 0`.
    unsigned_atoms: Vec<String>,
    /// Directed substitutions from requires equations (`atom := polynomial`),
    /// applied to fixpoint during normalization.
    substitutions: BTreeMap<String, Polynomial>,
    /// Lower bounds: each entry means `polynomial >= bound`.
    bounds: Vec<(Polynomial, BigInt)>,
    /// Mod-term atoms with their euclidean intervals (`t % k` in `0 ..= k-1`).
    mod_intervals: BTreeMap<String, Interval>,
    /// Difference-bound matrix over atoms + the virtual ZERO atom:
    /// `matrix[a][b]` = best known lower bound of `a - b`.
    matrix: BTreeMap<String, BTreeMap<String, BigInt>>,
    requires_unsatisfiable: bool,
}

const ZERO_ATOM: &str = "\u{0}zero";
const SUBSTITUTION_ROUNDS: usize = 8;

impl<'program> Engine<'program> {
    fn new(program: &'program TypedTrees, machine: &Machine) -> Self {
        let mut parameter_atoms = Vec::new();
        let mut unsigned_atoms = Vec::new();
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if parameter.is_self {
                    continue;
                }
                let name = parameter.name.as_str().to_owned();
                if !parameter_atoms.contains(&name) {
                    let primitive = program
                        .type_reference_table
                        .primitive_type(parameter.type_reference);
                    // `is_signed_integer` is false exactly for the unsigned
                    // integer primitives (floats/bool/string report true), so
                    // this marks precisely the `>= 0` carriers.
                    if let Some(primitive) = primitive {
                        if !primitive.is_signed_integer() {
                            unsigned_atoms.push(name.clone());
                        }
                    }
                    parameter_atoms.push(name);
                }
            }
        }
        Self {
            program,
            machine_symbol: machine.symbol,
            parameter_atoms,
            unsigned_atoms,
            substitutions: BTreeMap::new(),
            bounds: Vec::new(),
            mod_intervals: BTreeMap::new(),
            matrix: BTreeMap::new(),
            requires_unsatisfiable: false,
        }
    }

    /// Like [`Engine::new`], plus the reserved `result` atom for the
    /// machine's return value (unless a real parameter shadows it, matching
    /// the call-site binder rule). Used by the inductive transition path,
    /// where each arm binds or shares `result`.
    fn with_result_atom(
        program: &'program TypedTrees,
        machine: &Machine,
        root: &omega_typed_trees::state::State,
    ) -> Self {
        let mut engine = Self::new(program, machine);
        let shadowed = program
            .state_parameters(root)
            .iter()
            .any(|parameter| !parameter.is_self && parameter.name.as_str() == RESULT_BINDER);
        if !shadowed {
            engine.parameter_atoms.push(RESULT_BINDER.to_owned());
            if root.return_type.is_valid()
                && let Some(primitive) = program
                    .type_reference_table
                    .primitive_type(root.return_type)
                && !primitive.is_signed_integer()
            {
                engine.unsigned_atoms.push(RESULT_BINDER.to_owned());
            }
        }
        engine
    }

    /// Load the requires facts. Returns whether EVERY fact was inside the
    /// engine's language (full visibility is the precondition for rejecting
    /// unproven ensures). The ENTRY-state parameters' declared bracket
    /// ranges join as hypotheses too -- R1's bracket-as-sugar rule (ch12:
    /// `k: u64 [0..=8]` IS `requires k >= 0 && k <= 8`; the range is
    /// caller-discharged, so the callee's contract proofs may assume it).
    fn add_requires(&mut self, facts: &[ExpressionHandle]) -> bool {
        let mut comparisons = Vec::new();
        let mut fully_visible = self.collect_comparisons(facts, &mut comparisons);
        self.collect_entry_range_hypotheses(&mut comparisons);
        fully_visible &= self.install_hypotheses(comparisons);
        fully_visible
    }

    /// The bracket-as-sugar hypotheses: for each ENTRY-state (machine
    /// signature) parameter whose type carries a LITERAL `[a..=b]` range,
    /// push `param >= a` and `param <= b`. Entry-only: sub-state params are
    /// different binders that may reuse names.
    fn collect_entry_range_hypotheses(
        &mut self,
        comparisons: &mut Vec<(BinaryOperator, Polynomial, Polynomial)>,
    ) {
        let Some(machine) = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == self.machine_symbol)
        else {
            return;
        };
        let Some(entry) = self.program.machine_states(machine).first() else {
            return;
        };
        for parameter in self.program.state_parameters(entry) {
            if parameter.is_self {
                continue;
            }
            let Some(interval) = crate::arithmetic_domains::range_constraint_interval(
                self.program,
                parameter.type_reference,
            ) else {
                continue;
            };
            let atom = Polynomial::atom(parameter.name.as_str().to_owned());
            if let Some(low) = interval.low() {
                comparisons.push((
                    BinaryOperator::GreaterOrEqual,
                    atom.clone(),
                    Polynomial::constant(BigInt::from_i64(low)),
                ));
            }
            if let Some(high) = interval.high() {
                comparisons.push((
                    BinaryOperator::LessOrEqual,
                    atom.clone(),
                    Polynomial::constant(BigInt::from_i64(high)),
                ));
            }
        }
    }

    /// First ingestion pass: split facts into conjuncts and normalize each to
    /// a comparison triple. Range membership lowers to `&&` chains
    /// (`x in 1..=10` arrives as `(x >= 1) && (x <= 10)`), so facts split
    /// into conjuncts first. Returns whether every conjunct was readable.
    fn collect_comparisons(
        &mut self,
        facts: &[ExpressionHandle],
        comparisons: &mut Vec<(BinaryOperator, Polynomial, Polynomial)>,
    ) -> bool {
        let mut fully_visible = true;
        for fact in facts {
            for conjunct in self.conjuncts(*fact) {
                match self.comparison_polynomials(conjunct) {
                    Some(comparison) => comparisons.push(comparison),
                    None => fully_visible = false,
                }
            }
        }
        fully_visible
    }

    /// Second ingestion pass: harvest substitutions from equations so every
    /// later normalization sees them, store lower bounds, then seed and close
    /// the difference-bound matrix. Returns whether every hypothesis
    /// installed without arithmetic overflow.
    fn install_hypotheses(
        &mut self,
        comparisons: Vec<(BinaryOperator, Polynomial, Polynomial)>,
    ) -> bool {
        let mut fully_visible = true;
        for (operator, left, right) in &comparisons {
            if *operator == BinaryOperator::Equal {
                self.harvest_substitution(left, right);
            }
        }
        // Second pass: re-normalize under the substitutions and store bounds.
        let mut lower_bounds = Vec::new();
        for (operator, left, right) in comparisons {
            let left = self.substituted(&left);
            let right = self.substituted(&right);
            let difference_rl = right.sub(&left);
            let difference_lr = left.sub(&right);
            match operator {
                BinaryOperator::Less => lower_bounds.push((difference_rl, BigInt::from_i64(1))),
                BinaryOperator::LessOrEqual => lower_bounds.push((difference_rl, BigInt::zero())),
                BinaryOperator::Greater => lower_bounds.push((difference_lr, BigInt::from_i64(1))),
                BinaryOperator::GreaterOrEqual => {
                    lower_bounds.push((difference_lr, BigInt::zero()))
                }
                BinaryOperator::Equal => {
                    lower_bounds.push((difference_rl, BigInt::zero()));
                    lower_bounds.push((difference_lr, BigInt::zero()));
                }
                // A `!=` hypothesis carries no single lower bound; ignore it
                // (sound: dropping hypotheses only weakens proving power).
                BinaryOperator::NotEqual => {}
                _ => fully_visible = false,
            }
        }
        for (polynomial, bound) in lower_bounds {
            if let Some(value) = polynomial.constant_value() {
                if value < bound {
                    self.requires_unsatisfiable = true;
                }
                continue;
            }
            self.bounds.push((polynomial, bound));
        }

        self.seed_matrix();
        self.close_matrix();
        fully_visible
    }

    /// Judge a full ensures fact: an `&&` chain proves when every conjunct
    /// proves, and is disproved when any conjunct is.
    fn judge(&mut self, fact: ExpressionHandle) -> Judgment {
        let conjuncts = self.conjuncts(fact);
        if conjuncts.len() > 1 {
            // A disproved conjunct disproves the chain even if an earlier
            // conjunct was merely unknown, so judge all of them first.
            let mut constant_false = false;
            let mut refuted = false;
            let mut unknown = false;
            let mut all_in_language = true;
            for conjunct in conjuncts {
                match self.judge(conjunct) {
                    Judgment::Proven => {}
                    Judgment::ConstantFalse => constant_false = true,
                    Judgment::Refuted => refuted = true,
                    Judgment::Unknown { goal_in_language } => {
                        unknown = true;
                        all_in_language &= goal_in_language;
                    }
                }
            }
            return if constant_false {
                Judgment::ConstantFalse
            } else if refuted {
                Judgment::Refuted
            } else if unknown {
                Judgment::Unknown {
                    goal_in_language: all_in_language,
                }
            } else {
                Judgment::Proven
            };
        }

        let Some((operator, left, right)) = self.comparison_polynomials(fact) else {
            return Judgment::Unknown {
                goal_in_language: false,
            };
        };
        let left = self.substituted(&left);
        let right = self.substituted(&right);
        let difference_rl = right.sub(&left);
        let difference_lr = left.sub(&right);

        // Constant fold first: it gives the crispest diagnostic.
        if let Some(value) = difference_rl.constant_value() {
            let holds = match operator {
                BinaryOperator::Less => !value.is_negative() && !value.is_zero(),
                BinaryOperator::LessOrEqual => !value.is_negative(),
                BinaryOperator::Greater => value.is_negative(),
                BinaryOperator::GreaterOrEqual => value.is_negative() || value.is_zero(),
                BinaryOperator::Equal => value.is_zero(),
                BinaryOperator::NotEqual => !value.is_zero(),
                _ => {
                    return Judgment::Unknown {
                        goal_in_language: false,
                    };
                }
            };
            return if holds {
                Judgment::Proven
            } else {
                Judgment::ConstantFalse
            };
        }

        let zero = BigInt::zero();
        let one = BigInt::from_i64(1);
        let proved = match operator {
            BinaryOperator::Less => self.prove_at_least(&difference_rl, &one),
            BinaryOperator::LessOrEqual => self.prove_at_least(&difference_rl, &zero),
            BinaryOperator::Greater => self.prove_at_least(&difference_lr, &one),
            BinaryOperator::GreaterOrEqual => self.prove_at_least(&difference_lr, &zero),
            BinaryOperator::Equal => {
                self.prove_at_least(&difference_rl, &zero)
                    && self.prove_at_least(&difference_lr, &zero)
            }
            BinaryOperator::NotEqual => {
                self.prove_at_least(&difference_rl, &one)
                    || self.prove_at_least(&difference_lr, &one)
            }
            _ => {
                return Judgment::Unknown {
                    goal_in_language: false,
                };
            }
        };
        if proved {
            return Judgment::Proven;
        }

        let negation_proved = match operator {
            // not (l < r)  ==  l >= r
            BinaryOperator::Less => self.prove_at_least(&difference_lr, &zero),
            BinaryOperator::LessOrEqual => self.prove_at_least(&difference_lr, &one),
            BinaryOperator::Greater => self.prove_at_least(&difference_rl, &zero),
            BinaryOperator::GreaterOrEqual => self.prove_at_least(&difference_rl, &one),
            BinaryOperator::Equal => {
                self.prove_at_least(&difference_rl, &one)
                    || self.prove_at_least(&difference_lr, &one)
            }
            BinaryOperator::NotEqual => {
                self.prove_at_least(&difference_rl, &zero)
                    && self.prove_at_least(&difference_lr, &zero)
            }
            _ => false,
        };
        if negation_proved {
            return Judgment::Refuted;
        }

        Judgment::Unknown {
            goal_in_language: true,
        }
    }

    /// Prove `polynomial >= bound` via the difference-bound matrix or the
    /// interval evaluator.
    fn prove_at_least(&self, polynomial: &Polynomial, bound: &BigInt) -> bool {
        if let Some((positive, negative, constant)) = polynomial.as_atom_difference() {
            if let Some(best) = self.matrix_bound(&positive, &negative) {
                if best.add(&constant) >= *bound {
                    return true;
                }
            }
        }
        if let Some((atom, sign, constant)) = polynomial.as_single_atom() {
            let other = if sign == 1 {
                self.matrix_bound(&atom, ZERO_ATOM)
            } else {
                self.matrix_bound(ZERO_ATOM, &atom)
            };
            if let Some(best) = other {
                if best.add(&constant) >= *bound {
                    return true;
                }
            }
        }
        // A stored hypothesis bound whose polynomial IS the goal polynomial
        // subsumes it directly. This is the shape induction hypotheses
        // arrive in: general polynomial equations (e.g. `2*result - P >= 0`)
        // that fit neither the difference-bound matrix nor the interval
        // evaluator, but whose canonical form matches the goal exactly.
        for (stored, stored_bound) in &self.bounds {
            if stored == polynomial && stored_bound >= bound {
                return true;
            }
        }
        if let Some(low) = self.polynomial_interval(polynomial).low {
            if low >= *bound {
                return true;
            }
        }
        false
    }

    fn polynomial_interval(&self, polynomial: &Polynomial) -> Interval {
        let mut total = Interval::constant(BigInt::zero());
        for (monomial, coefficient) in &polynomial.terms {
            let mut product = Interval::constant(BigInt::from_i64(1));
            for (atom, power) in monomial {
                let base = self.atom_interval(atom);
                product = product.multiply(&base.correlated_power(*power));
            }
            total = total.add(&product.scale(coefficient));
        }
        total
    }

    fn atom_interval(&self, atom: &str) -> Interval {
        if let Some(interval) = self.mod_intervals.get(atom) {
            return interval.clone();
        }
        Interval {
            low: self.matrix_bound(atom, ZERO_ATOM),
            high: self
                .matrix_bound(ZERO_ATOM, atom)
                .map(|bound| bound.negate()),
        }
    }

    fn matrix_bound(&self, from: &str, to: &str) -> Option<BigInt> {
        if from == to {
            return Some(BigInt::zero());
        }
        self.matrix.get(from).and_then(|row| row.get(to)).cloned()
    }

    fn seed_matrix(&mut self) {
        for atom in self.unsigned_atoms.clone() {
            self.record_difference(&atom, ZERO_ATOM, BigInt::zero());
        }
        let mod_atoms: Vec<(String, Interval)> = self
            .mod_intervals
            .iter()
            .map(|(atom, interval)| (atom.clone(), interval.clone()))
            .collect();
        for (atom, interval) in mod_atoms {
            if let Some(low) = interval.low {
                self.record_difference(&atom, ZERO_ATOM, low);
            }
            if let Some(high) = interval.high {
                self.record_difference(ZERO_ATOM, &atom, high.negate());
            }
        }
        for (polynomial, bound) in self.bounds.clone() {
            if let Some((positive, negative, constant)) = polynomial.as_atom_difference() {
                self.record_difference(&positive, &negative, bound.sub(&constant));
            }
            if let Some((atom, sign, constant)) = polynomial.as_single_atom() {
                let edge = bound.sub(&constant);
                if sign == 1 {
                    self.record_difference(&atom, ZERO_ATOM, edge);
                } else {
                    self.record_difference(ZERO_ATOM, &atom, edge);
                }
            }
        }
    }

    fn record_difference(&mut self, from: &str, to: &str, bound: BigInt) {
        let row = self.matrix.entry(from.to_owned()).or_default();
        match row.entry(to.to_owned()) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(bound);
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                if bound > *slot.get() {
                    slot.insert(bound);
                }
            }
        }
        self.matrix.entry(to.to_owned()).or_default();
    }

    fn close_matrix(&mut self) {
        let atoms: Vec<String> = self.matrix.keys().cloned().collect();
        for via in &atoms {
            for from in &atoms {
                let Some(first) = self.matrix_bound(from, via) else {
                    continue;
                };
                for to in &atoms {
                    let Some(second) = self.matrix_bound(via, to) else {
                        continue;
                    };
                    let combined = first.add(&second);
                    if from == to {
                        if !combined.is_negative() && !combined.is_zero() {
                            self.requires_unsatisfiable = true;
                        }
                        continue;
                    }
                    let current = self.matrix_bound(from, to);
                    if current.is_none() || combined > current.unwrap() {
                        self.record_difference(from, to, combined);
                    }
                }
            }
        }
    }

    fn harvest_substitution(&mut self, left: &Polynomial, right: &Polynomial) {
        for (candidate, replacement) in [(left, right), (right, left)] {
            if let Some((atom, 1, constant)) = candidate.as_single_atom()
                && constant.is_zero()
            {
                let occurs = replacement
                    .terms
                    .keys()
                    .any(|monomial| monomial.contains_key(&atom));
                if !occurs && !self.substitutions.contains_key(&atom) {
                    self.substitutions.insert(atom, replacement.clone());
                    return;
                }
            }
        }
    }

    fn substituted(&self, polynomial: &Polynomial) -> Polynomial {
        let mut current = polynomial.clone();
        for _ in 0..SUBSTITUTION_ROUNDS {
            let mut changed = false;
            let mut next = Polynomial::default();
            let mut overflowed = false;
            for (monomial, coefficient) in &current.terms {
                let mut piece = Polynomial::constant(coefficient.clone());
                for (atom, power) in monomial {
                    let base = match self.substitutions.get(atom) {
                        Some(replacement) => {
                            changed = true;
                            replacement.clone()
                        }
                        None => Polynomial::atom(atom.clone()),
                    };
                    for _ in 0..*power {
                        match piece.checked_mul(&base) {
                            Some(product) => piece = product,
                            None => {
                                overflowed = true;
                                break;
                            }
                        }
                    }
                    if overflowed {
                        break;
                    }
                }
                if overflowed {
                    break;
                }
                next = next.add(&piece);
            }
            if overflowed {
                return current;
            }
            current = next;
            if !changed {
                break;
            }
        }
        current
    }

    /// Flatten nested `&&` chains into conjunct handles (a single
    /// non-conjunction fact returns itself).
    fn conjuncts(&self, fact: ExpressionHandle) -> Vec<ExpressionHandle> {
        let node = self.program.expression_table.expression(fact).clone();
        match node {
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
                let mut left = self.conjuncts(binary.left);
                left.extend(self.conjuncts(binary.right));
                left
            }
            ExpressionNode::Mutable(inner) => self.conjuncts(inner),
            _ => vec![fact],
        }
    }

    /// Split a fact into `(comparison operator, left polynomial, right
    /// polynomial)`.
    fn comparison_polynomials(
        &mut self,
        fact: ExpressionHandle,
    ) -> Option<(BinaryOperator, Polynomial, Polynomial)> {
        let node = self.program.expression_table.expression(fact).clone();
        let ExpressionNode::Binary(binary) = node else {
            return None;
        };
        match binary.operator {
            BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual => {
                let left = self.normalize(binary.left)?;
                let right = self.normalize(binary.right)?;
                Some((binary.operator, left, right))
            }
            _ => None,
        }
    }

    /// Normalize a TERM expression to a polynomial. `None` = outside the
    /// engine's language.
    fn normalize(&mut self, expression: ExpressionHandle) -> Option<Polynomial> {
        let node = self.program.expression_table.expression(expression).clone();
        match node {
            ExpressionNode::Integer(value) => Some(Polynomial::constant(value.value_bignum()?)),
            ExpressionNode::Mutable(inner) => self.normalize(inner),
            ExpressionNode::Name(path) => {
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
                if members.len() != 1 {
                    return None;
                }
                let name = members[0].as_str().to_owned();
                if !self.parameter_atoms.contains(&name) {
                    return None;
                }
                Some(Polynomial::atom(name))
            }
            // The typed-tree unary operator is logical-not only (negative
            // literals fold into Integer), so unary nodes are never terms.
            ExpressionNode::Unary(_) => None,
            ExpressionNode::Binary(binary) => match binary.operator {
                BinaryOperator::Add => {
                    let left = self.normalize(binary.left)?;
                    let right = self.normalize(binary.right)?;
                    Some(left.add(&right))
                }
                BinaryOperator::Subtract => {
                    let left = self.normalize(binary.left)?;
                    let right = self.normalize(binary.right)?;
                    Some(left.sub(&right))
                }
                BinaryOperator::Multiply => {
                    let left = self.normalize(binary.left)?;
                    let right = self.normalize(binary.right)?;
                    left.checked_mul(&right)
                }
                BinaryOperator::Modulo => {
                    let operand = self.normalize(binary.left)?;
                    let modulus = self.normalize(binary.right)?.constant_value()?;
                    if modulus.is_negative() || modulus.is_zero() {
                        return None;
                    }
                    let display = format!("({}) % {}", polynomial_display(&operand), modulus);
                    self.mod_intervals.insert(
                        display.clone(),
                        Interval {
                            low: Some(BigInt::zero()),
                            high: Some(modulus.sub(&BigInt::from_i64(1))),
                        },
                    );
                    Some(Polynomial::atom(display))
                }
                _ => None,
            },
            ExpressionNode::Call(call) => {
                // Proof-view applications are opaque atoms compared by
                // equality only. Anything else is outside the language.
                let target = call.target.as_str();
                if !matches!(target, "Bag" | "Seq" | "Range") {
                    return None;
                }
                if call.receiver.is_valid() {
                    return None;
                }
                let mut rendered = Vec::new();
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec()
                {
                    rendered.push(self.program.expression_table.display_name(argument));
                }
                Some(Polynomial::atom(format!(
                    "{}({})",
                    target,
                    rendered.join(", ")
                )))
            }
            _ => None,
        }
    }
}

fn polynomial_display(polynomial: &Polynomial) -> String {
    let mut parts = Vec::new();
    for (monomial, coefficient) in &polynomial.terms {
        let atoms: Vec<String> = monomial
            .iter()
            .map(|(atom, power)| {
                if *power == 1 {
                    atom.clone()
                } else {
                    format!("{atom}^{power}")
                }
            })
            .collect();
        if atoms.is_empty() {
            parts.push(coefficient.to_string());
        } else if *coefficient == BigInt::from_i64(1) {
            parts.push(atoms.join("*"));
        } else {
            parts.push(format!("{}*{}", coefficient, atoms.join("*")));
        }
    }
    if parts.is_empty() {
        "0".to_owned()
    } else {
        parts.join(" + ")
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
    classification: &omega_typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    expression: ExpressionHandle,
) -> Option<omega_typed_trees::name::Identifier> {
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
        ExpressionNode::Mutable(inner) => recurse(*inner),
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
    }
}

/// Whether a statement is a CITATION: a free (receiver-less) call whose
/// callee resolves to a free PROOF MACHINE other than the enclosing one
/// (self-calls are recursion, owned by the descent checks and the IH).
fn is_citation_statement(
    program: &TypedTrees,
    classification: &omega_typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    call: &omega_typed_trees::statement::TableCall,
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

/// Statement-call CITATIONS (ch10 "Citing Proofs"; the OWNER_QUESTIONS #14
/// answer, 2026-07-18): `add_zero_right(b);` inside a proof body delivers
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
    classification: &omega_typed_trees::proof_only::ProofOnlyClassification,
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
    classification: &omega_typed_trees::proof_only::ProofOnlyClassification,
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
            match contract.kind {
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
                SignatureContractKind::Boundary => {}
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

/// LAW-CONFORMANCE (rearrange rung B, settle 2026-07-18): a trait requirement
/// carrying `ensures` is a LAW -- an obligation every satisfier proves. The
/// satisfier machine must carry a PROVEN ensures conjunct matching the
/// declared law forall-to-forall: the requirement's parameters are pattern
/// variables that must bind to DISTINCT parameters of the satisfier (a weaker
/// instance -- `add(x, x) == add(x, x)` against `add(a, b) == add(b, a)` --
/// does not license the law), and the law's op-slot applications (`add`,
/// `mul` -- the trait's own requirement names) resolve to the CARRIER's bound
/// machines first. This is the N3 shape-match machinery promoted from
/// suggestion-only to load-bearing.
pub(crate) fn check_law_conformance(
    program: &TypedTrees,
    machine: &Machine,
    conformance_alias: Option<&str>,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // The declared law conjuncts (Equal binaries; And-chains split;
    // `result`-mentioning conjuncts are functional specs, not laws -- they
    // stay outside this check, exactly like the suggestion path).
    let mut law_conjuncts: Vec<ExpressionHandle> = Vec::new();
    for contract in program.state_signature_contracts(requirement) {
        if contract.kind != SignatureContractKind::Ensures {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(expression) = fact {
                collect_equality_conjuncts(program, *expression, &mut law_conjuncts);
            }
        }
    }
    if law_conjuncts.is_empty() {
        return; // an OP requirement, not a law
    }

    let requirement_parameters: Vec<String> = program
        .state_signature_parameters(requirement)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect();

    let Some(entry_state) = program.machine_states(machine).first() else {
        return; // the signature check already flagged a stateless machine
    };
    let satisfier_parameters: Vec<String> = program
        .state_parameters(entry_state)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect();

    // The CARRIER is the satisfier's first entry parameter type (law
    // requirements are Self-shaped; the signature check already bound Self
    // there), or its return type for parameterless requirements.
    let carrier = program
        .state_parameters(entry_state)
        .first()
        .map(|parameter| parameter.type_reference)
        .unwrap_or(entry_state.return_type);

    // The trait's op-slot names, and the carrier's bound machine for each.
    let slot_names: Vec<String> = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .map(|signature| signature.name.as_str().to_owned())
        .collect();
    let slot_bindings = carrier_slot_bindings(
        program,
        trait_definition,
        carrier,
        conformance_alias,
        diagnostics,
    );

    // The satisfier's own PROVEN ensures conjuncts (machine-checked by this
    // engine before this point -- compiling means proven).
    let mut proven_conjuncts: Vec<ExpressionHandle> = Vec::new();
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Ensures {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(expression) = fact {
                collect_equality_conjuncts(program, *expression, &mut proven_conjuncts);
            }
        }
    }

    let result_binder = RESULT_BINDER.to_owned();
    for law_conjunct in &law_conjuncts {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*law_conjunct)
        else {
            continue;
        };
        let (Some(law_left), Some(law_right)) = (
            structural_term(program, binary.left),
            structural_term(program, binary.right),
        ) else {
            continue; // out-of-language law conjunct: nothing to enforce yet
        };
        if term_mentions_variable(&law_left, &result_binder)
            || term_mentions_variable(&law_right, &result_binder)
        {
            continue; // a functional spec, not a law conjunct
        }

        // Resolve the law's op-slot applications to the carrier's machines.
        let mut missing_slots: Vec<String> = Vec::new();
        let law_left =
            rewrite_slot_applications(&law_left, &slot_names, &slot_bindings, &mut missing_slots);
        let law_right =
            rewrite_slot_applications(&law_right, &slot_names, &slot_bindings, &mut missing_slots);
        // N4 identity-law bridging: nullary CONSTANT applications
        // (`zero()`, `one()`) normalize to their constructor bodies, so
        // `add(a, zero())` and the proof's `add(a, Nat::Zero)` are one
        // term.
        let law_left = unfold_constant_applications(program, law_left);
        let law_right = unfold_constant_applications(program, law_right);
        if !missing_slots.is_empty() {
            missing_slots.sort();
            missing_slots.dedup();
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}`, whose law mentions `{}` -- but no machine \
                 satisfies that requirement for this carrier (conform the op first; the law \
                 check resolves op slots through the carrier's own conformances)",
                machine.name,
                trait_definition.name,
                requirement.name,
                missing_slots.join("`, `"),
            )));
            continue;
        }

        let matched = proven_conjuncts.iter().any(|proven| {
            let ExpressionNode::Binary(proven_binary) =
                program.expression_table.expression(*proven)
            else {
                return false;
            };
            let (Some(proven_left), Some(proven_right)) = (
                structural_term(program, proven_binary.left),
                structural_term(program, proven_binary.right),
            ) else {
                return false;
            };
            if term_mentions_variable(&proven_left, &result_binder)
                || term_mentions_variable(&proven_right, &result_binder)
            {
                return false;
            }
            let proven_left = unfold_constant_applications(program, proven_left);
            let proven_right = unfold_constant_applications(program, proven_right);
            [(&proven_left, &proven_right), (&proven_right, &proven_left)]
                .into_iter()
                .any(|(first, second)| {
                    let mut bindings: Vec<(String, StructuralTerm)> = Vec::new();
                    diagnostic_shape_match(&law_left, first, &requirement_parameters, &mut bindings)
                        && diagnostic_shape_match(
                            &law_right,
                            second,
                            &requirement_parameters,
                            &mut bindings,
                        )
                        && bindings_are_forall_general(&bindings, &satisfier_parameters)
                })
        });

        if !matched {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}` but proves no ensures matching the declared \
                 law `{} == {}` -- a law requirement's satisfier must carry that equation as a \
                 machine-checked ensures, general in every law parameter",
                machine.name,
                trait_definition.name,
                requirement.name,
                display_structural_term(&law_left),
                display_structural_term(&law_right),
            )));
        }
    }
}

/// Forall-to-forall sharpening: every law parameter must bind to a DISTINCT
/// plain parameter VARIABLE of the satisfier -- binding two law parameters to
/// one satisfier parameter (or to a compound term) proves only a weaker
/// instance of the law.
fn bindings_are_forall_general(
    bindings: &[(String, StructuralTerm)],
    satisfier_parameters: &[String],
) -> bool {
    let mut seen: Vec<&String> = Vec::new();
    for (_, bound) in bindings {
        let StructuralTerm::Variable(name) = bound else {
            return false;
        };
        if !satisfier_parameters
            .iter()
            .any(|parameter| parameter == name)
        {
            return false;
        }
        if seen.iter().any(|previous| *previous == name) {
            return false;
        }
        seen.push(name);
    }
    true
}

/// Split an ensures fact into its `==` conjuncts (And-chains recursively).
fn collect_equality_conjuncts(
    program: &TypedTrees,
    expression: ExpressionHandle,
    out: &mut Vec<ExpressionHandle>,
) {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return;
    };
    match binary.operator {
        BinaryOperator::And => {
            collect_equality_conjuncts(program, binary.left, out);
            collect_equality_conjuncts(program, binary.right, out);
        }
        BinaryOperator::Equal => out.push(expression),
        _ => {}
    }
}

/// The CARRIER's op-slot bindings: for each requirement of the trait, the
/// machine conforming to it whose carrier type matches. Alias preference
/// (plural algebras): a binding sharing the checking conformance's alias
/// wins; otherwise unaliased bindings win; a remaining tie is ambiguous and
/// reported.
fn carrier_slot_bindings(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    carrier: omega_typed_trees::types::TypeReferenceHandle,
    prefer_alias: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<(String, String)> {
    let mut bindings: Vec<(String, String)> = Vec::new();

    for requirement in program.trait_machine_signatures(trait_definition) {
        // (slot machine name, alias) candidates for this carrier.
        let mut candidates: Vec<(String, Option<String>)> = Vec::new();
        for candidate in program.machines() {
            for conformance in program.machine_trait_conformances(candidate) {
                if conformance.symbol != trait_definition.symbol {
                    continue;
                }
                let bound_requirement = conformance
                    .requirement
                    .as_ref()
                    .map(|name| name.as_str().to_owned())
                    .or_else(|| {
                        candidate
                            .attached_data
                            .is_none()
                            .then(|| candidate.name.as_str().to_owned())
                    });
                if bound_requirement.as_deref() != Some(requirement.name.as_str()) {
                    continue;
                }
                let Some(candidate_entry) = program.machine_states(candidate).first() else {
                    continue;
                };
                let candidate_carrier = program
                    .state_parameters(candidate_entry)
                    .first()
                    .map(|parameter| parameter.type_reference)
                    .unwrap_or(candidate_entry.return_type);
                if !crate::type_references::type_references_match(
                    program,
                    candidate_carrier,
                    carrier,
                ) {
                    continue;
                }
                candidates.push((
                    candidate.name.as_str().to_owned(),
                    conformance
                        .alias
                        .as_ref()
                        .map(|alias| alias.as_str().to_owned()),
                ));
            }
        }

        if candidates.is_empty() {
            continue; // an unbound slot only matters if a law mentions it
        }
        let chosen = if let Some(preferred) = candidates
            .iter()
            .filter(|(_, alias)| alias.as_deref() == prefer_alias)
            .collect::<Vec<_>>()
            .split_first()
            .filter(|(_, rest)| rest.is_empty())
            .map(|(first, _)| (*first).clone())
        {
            Some(preferred)
        } else {
            let unaliased: Vec<_> = candidates
                .iter()
                .filter(|(_, alias)| alias.is_none())
                .collect();
            match unaliased.as_slice() {
                [single] => Some((*single).clone()),
                [] if candidates.len() == 1 => Some(candidates[0].clone()),
                [] => None,
                _ => None,
            }
        };
        match chosen {
            Some((machine_name, _)) => {
                bindings.push((requirement.name.as_str().to_owned(), machine_name));
            }
            None => {
                diagnostics.push(Diagnostic::error(format!(
                    "trait `{}` requirement `{}` has AMBIGUOUS satisfiers for this carrier -- \
                     name the family with `as <Alias>` on each conformance so the law check \
                     (and the judge) can pick one",
                    trait_definition.name, requirement.name,
                )));
            }
        }
    }

    bindings
}

/// Rewrite the law's op-slot applications (`add(a, b)` where `add` is a
/// requirement of the SAME trait) to the carrier's bound machine names;
/// slots with no binding are collected for the missing-slot diagnostic.
fn rewrite_slot_applications(
    term: &StructuralTerm,
    slot_names: &[String],
    slot_bindings: &[(String, String)],
    missing: &mut Vec<String>,
) -> StructuralTerm {
    match term {
        StructuralTerm::Application { machine, arguments } => {
            let arguments = arguments
                .iter()
                .map(|argument| {
                    rewrite_slot_applications(argument, slot_names, slot_bindings, missing)
                })
                .collect();
            let machine = if slot_names.iter().any(|slot| slot == machine) {
                match slot_bindings
                    .iter()
                    .find(|(slot, _)| slot == machine)
                    .map(|(_, bound)| bound.clone())
                {
                    Some(bound) => bound,
                    None => {
                        missing.push(machine.clone());
                        machine.clone()
                    }
                }
            } else {
                machine.clone()
            };
            StructuralTerm::Application { machine, arguments }
        }
        StructuralTerm::Constructor { data, case, fields } => StructuralTerm::Constructor {
            data: data.clone(),
            case: case.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        rewrite_slot_applications(value, slot_names, slot_bindings, missing),
                    )
                })
                .collect(),
        },
        other => other.clone(),
    }
}

fn term_mentions_variable(term: &StructuralTerm, variable: &String) -> bool {
    match term {
        StructuralTerm::Variable(name) => name == variable,
        StructuralTerm::Constructor { fields, .. } => fields
            .iter()
            .any(|(_, value)| term_mentions_variable(value, variable)),
        StructuralTerm::Application { arguments, .. } => arguments
            .iter()
            .any(|argument| term_mentions_variable(argument, variable)),
        StructuralTerm::Opaque(_) => false,
    }
}

/// First-order matching for the SUGGESTION diagnostic only (the proving
/// path never pattern-matches -- citations instantiate at written
/// operands): occurrences of `variables` in `pattern` bind consistently
/// against the goal's subterms; everything else must agree exactly.
fn diagnostic_shape_match(
    pattern: &StructuralTerm,
    term: &StructuralTerm,
    variables: &[String],
    bindings: &mut Vec<(String, StructuralTerm)>,
) -> bool {
    match (pattern, term) {
        (StructuralTerm::Variable(name), _) if variables.iter().any(|v| v == name) => {
            if let Some((_, bound)) = bindings.iter().find(|(n, _)| n == name) {
                bound == term
            } else {
                bindings.push((name.clone(), term.clone()));
                true
            }
        }
        (StructuralTerm::Variable(left), StructuralTerm::Variable(right)) => left == right,
        (
            StructuralTerm::Constructor { data, case, fields },
            StructuralTerm::Constructor {
                data: data_t,
                case: case_t,
                fields: fields_t,
            },
        ) => {
            data == data_t
                && case == case_t
                && fields.len() == fields_t.len()
                && fields
                    .iter()
                    .zip(fields_t)
                    .all(|((name, value), (name_t, value_t))| {
                        name == name_t
                            && diagnostic_shape_match(value, value_t, variables, bindings)
                    })
        }
        (
            StructuralTerm::Application { machine, arguments },
            StructuralTerm::Application {
                machine: machine_t,
                arguments: arguments_t,
            },
        ) => {
            machine == machine_t
                && arguments.len() == arguments_t.len()
                && arguments
                    .iter()
                    .zip(arguments_t)
                    .all(|(argument, argument_t)| {
                        diagnostic_shape_match(argument, argument_t, variables, bindings)
                    })
        }
        (StructuralTerm::Opaque(left), StructuralTerm::Opaque(right)) => left == right,
        _ => false,
    }
}

/// Render a term back into citation-argument spelling.
fn display_structural_term(term: &StructuralTerm) -> String {
    match term {
        StructuralTerm::Variable(name) => name.clone(),
        StructuralTerm::Constructor { data, case, fields } => {
            if fields.is_empty() {
                format!("{data}::{case}")
            } else {
                let rendered: Vec<String> = fields
                    .iter()
                    .map(|(name, value)| format!("{name}: {}", display_structural_term(value)))
                    .collect();
                format!("{data}::{case} {{ {} }}", rendered.join(", "))
            }
        }
        StructuralTerm::Application { machine, arguments } => {
            let rendered: Vec<String> = arguments.iter().map(display_structural_term).collect();
            format!("{machine}({})", rendered.join(", "))
        }
        StructuralTerm::Opaque(display) => display.clone(),
    }
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
    &'program omega_typed_trees::name::Identifier,
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
/// obligation `sub(Succ(ARG), MEASURE) == Zero` then judges under the
/// accumulated hypotheses.
pub(crate) fn proof_edge_strict_decrease_judged(
    program: &TypedTrees,
    machine: &Machine,
    state: &omega_typed_trees::state::State,
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
            let TransitionTargetNode::Named { path, arguments } =
                program.statement_table.transition_target(transition.target)
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
                        omega_typed_trees::data::DataMember::Variant(variant)
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
        machine: "sub".to_owned(),
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
    call: &omega_typed_trees::expression::TableCallExpression,
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

    let facts = |kind: omega_typed_trees::signature::SignatureContractKind| {
        program
            .machine_contracts(callee)
            .iter()
            .filter(|contract| contract.kind == kind)
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
            .filter_map(|fact| match fact {
                ProofFact::Expression(expression) => Some(*expression),
                ProofFact::Membership(_) => None,
            })
            .collect::<Vec<_>>()
    };
    for fact in facts(omega_typed_trees::signature::SignatureContractKind::Requires) {
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
    for fact in facts(omega_typed_trees::signature::SignatureContractKind::Ensures) {
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
    call: &omega_typed_trees::statement::TableCall,
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
    let collect = |kind: omega_typed_trees::signature::SignatureContractKind| {
        program
            .machine_contracts(callee)
            .iter()
            .filter(|contract| contract.kind == kind)
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
            .filter_map(|fact| match fact {
                ProofFact::Expression(expression) => Some(*expression),
                ProofFact::Membership(_) => None,
            })
            .collect::<Vec<_>>()
    };
    for fact in collect(omega_typed_trees::signature::SignatureContractKind::Requires) {
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
    for fact in collect(omega_typed_trees::signature::SignatureContractKind::Ensures) {
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
                omega_typed_trees::signature::SignatureContractKind::Requires
            )
        })
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
        .filter_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            ProofFact::Membership(_) => None,
        })
        .collect()
}

fn instantiate_citation(
    program: &TypedTrees,
    classification: &omega_typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    target: &omega_typed_trees::name::Identifier,
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
        match contract.kind {
            SignatureContractKind::Requires => {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    match fact {
                        ProofFact::Expression(expression) => requires_facts.push(*expression),
                        // Membership requires are outside the structural
                        // judge's language: the site cannot discharge them.
                        ProofFact::Membership(_) => requires_out_of_language = true,
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
            SignatureContractKind::Boundary => {}
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
    caller_state: &omega_typed_trees::state::State,
    callee_machine: &Machine,
    callee_state: &omega_typed_trees::state::State,
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
                        ProofFact::Membership(_) => None,
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
                        ProofFact::Membership(_) => None,
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
    /// such as `sub(b, a)` cannot be a substitution, so it is retained as an
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
    let branch = |transition: &omega_typed_trees::statement::TableTransition| {
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

type PendingStructuralCitation = (omega_typed_trees::name::Identifier, Vec<StructuralTerm>);

/// Recognize a proof machine as a tree of structural case states. Each named
/// state can either terminate in a value or refine another subject and hand
/// the branch to a further named state. Unsupported statement order, an
/// unresolved target, or a state cycle fails the whole recognition closed.
fn recognize_structural_case_arms(
    program: &TypedTrees,
    machine: &Machine,
    judge: &StructuralJudge<'_>,
    classification: &omega_typed_trees::proof_only::ProofOnlyClassification,
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
    classification: &omega_typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    states: &[omega_typed_trees::state::State],
    state: &omega_typed_trees::state::State,
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
                        omega_typed_trees::data::DataMember::Variant(variant)
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
    classification: &omega_typed_trees::proof_only::ProofOnlyClassification,
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
            match (contract.kind, fact) {
                (SignatureContractKind::Requires, ProofFact::Expression(expression)) => {
                    requires.push(*expression);
                }
                (SignatureContractKind::Requires, ProofFact::Membership(_)) => {
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

enum StructuralJudgment {
    Proven,
    Refuted,
    Unknown,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum StructuralTerm {
    Variable(String),
    /// data name, case name, named payload fields (sorted by field name;
    /// empty for a nullary classifier like `Nat::Zero`). Payload-carrying
    /// terms spell as parenthesized case literals in fact position
    /// (`(Nat::Succ { prev: a })` -- the parens re-enable struct literals in
    /// the contract grammar), and both lowering fences stand down for
    /// recursive data so the raw Binary reaches this judge.
    Constructor {
        data: String,
        case: String,
        fields: Vec<(String, StructuralTerm)>,
    },
    /// A FREE call whose arguments all term-ify (`add(Nat::Zero, b)`). Static
    /// machine selections are encoded in `machine`, so `f<A>` and `f<B>`
    /// remain distinct terms and generic unfolding can alpha-substitute them.
    /// Resolution UNFOLDS it when the callee is a single-state proof
    /// machine of the case-arm shape and the matched argument resolves to
    /// a constructor -- the compute-mode of N3's operator routing.
    Application {
        machine: String,
        arguments: Vec<StructuralTerm>,
    },
    /// Anything else, compared by canonical display name only.
    Opaque(String),
}

/// REARRANGE-MODE license (settle 2026-07-18, rung C): a carrier EARNS ring
/// canonicalization over an op through EXPLICIT conformance, never
/// scope-sniffing. A license exists for op machine `add_machine` when some
/// trait declares an op slot with BOTH a commutativity law and an
/// associativity law over it (detected by SHAPE, not by name -- `R(x, y) ==
/// R(y, x)` and `R(R(x, y), z) == R(x, R(y, z))` with distinct requirement
/// params), the op slot is conformed by `add_machine`, and BOTH law slots
/// have satisfiers for the same carrier (whose proofs rung B already
/// machine-checked against the declared laws).
#[derive(Clone, Debug)]
struct RingLicense {
    add_machine: String,
}

/// Tier-2 (full polynomial): the PAIRED license -- an add op and a mul op
/// each carrying comm+assoc, connected by a conformed DISTRIBUTIVITY law.
#[derive(Clone)]
struct SemiringLicense {
    add_machine: String,
    mul_machine: String,
}

struct StructuralJudge<'program> {
    program: &'program TypedTrees,
    substitutions: Vec<(String, StructuralTerm)>,
    /// Application REWRITES (`add_zero_right(prev) -> prev`): hypothesis
    /// equations with an application side orient REDUCING -- the inductive
    /// hypothesis rewrites the self-application away instead of expanding a
    /// variable into it, which also serves asymmetric goals.
    rewrites: Vec<(StructuralTerm, StructuralTerm)>,
    hypotheses_contradictory: bool,
    ring_licenses: Vec<RingLicense>,
    /// Tier-2: paired add/mul licenses with a conformed distributivity law.
    semiring_licenses: Vec<SemiringLicense>,
}

impl Clone for StructuralJudge<'_> {
    fn clone(&self) -> Self {
        Self {
            program: self.program,
            substitutions: self.substitutions.clone(),
            rewrites: self.rewrites.clone(),
            hypotheses_contradictory: self.hypotheses_contradictory,
            ring_licenses: self.ring_licenses.clone(),
            semiring_licenses: self.semiring_licenses.clone(),
        }
    }
}

impl<'program> StructuralJudge<'program> {
    fn from_requires(
        program: &'program TypedTrees,
        judged_machine: &Machine,
        requires: &[ExpressionHandle],
    ) -> Self {
        let mut judge = Self {
            program,
            substitutions: Vec::new(),
            rewrites: Vec::new(),
            hypotheses_contradictory: false,
            ring_licenses: compute_ring_licenses(program, judged_machine),
            semiring_licenses: compute_semiring_licenses(program, judged_machine),
        };
        for fact in requires {
            judge.intake(program, *fact);
        }
        judge
    }

    fn intake(&mut self, program: &TypedTrees, fact: ExpressionHandle) {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
            return;
        };
        match binary.operator {
            BinaryOperator::And => {
                self.intake(program, binary.left);
                self.intake(program, binary.right);
            }
            BinaryOperator::Equal => {
                let (Some(left), Some(right)) = (
                    structural_term(program, binary.left),
                    structural_term(program, binary.right),
                ) else {
                    return;
                };
                self.intake_equation(left, right, 0);
            }
            _ => {}
        }
    }

    /// One structural equation: constructor pairs DECOMPOSE (injectivity --
    /// `Succ(a) == Succ(b)` yields `a == b`), distinct cases of one data
    /// make the hypotheses contradictory (disjointness), and a variable side
    /// becomes a directed substitution (first binding wins).
    fn intake_equation(&mut self, left: StructuralTerm, right: StructuralTerm, depth: usize) {
        if depth >= 32 {
            return;
        }
        let left = self.resolve(left);
        let right = self.resolve(right);
        match (&left, &right) {
            (
                StructuralTerm::Constructor {
                    data: data_l,
                    case: case_l,
                    fields: fields_l,
                },
                StructuralTerm::Constructor {
                    data: data_r,
                    case: case_r,
                    fields: fields_r,
                },
            ) if data_l == data_r => {
                if case_l != case_r {
                    self.hypotheses_contradictory = true;
                    return;
                }
                for (name_l, value_l) in fields_l {
                    if let Some((_, value_r)) = fields_r.iter().find(|(name_r, _)| name_r == name_l)
                    {
                        self.intake_equation(value_l.clone(), value_r.clone(), depth + 1);
                    }
                }
            }
            (StructuralTerm::Application { .. }, _) => {
                if !term_contains(&right, &left) {
                    self.rewrites.push((left, right));
                }
            }
            (_, StructuralTerm::Application { .. }) => {
                if !term_contains(&left, &right) {
                    self.rewrites.push((right, left));
                }
            }
            (StructuralTerm::Variable(name), _) => {
                if left != right {
                    self.substitutions.push((name.clone(), right));
                }
            }
            (_, StructuralTerm::Variable(name)) => {
                self.substitutions.push((name.clone(), left));
            }
            _ => {}
        }
    }

    /// Follow variable substitutions to a fixpoint, depth-capped (a cyclic
    /// substitution chain resolves to wherever the cap lands, which only
    /// weakens judgments toward Unknown -- never unsound). Constructor
    /// fields resolve recursively under the same budget.
    fn resolve(&self, term: StructuralTerm) -> StructuralTerm {
        self.resolve_at(term, 0)
    }

    fn resolve_at(&self, mut term: StructuralTerm, depth: usize) -> StructuralTerm {
        if depth >= 32 {
            return term;
        }
        for _ in 0..32 {
            match term {
                StructuralTerm::Variable(ref name) => {
                    let Some((_, replacement)) = self
                        .substitutions
                        .iter()
                        .find(|(variable, _)| variable == name)
                    else {
                        return term;
                    };
                    term = replacement.clone();
                }
                StructuralTerm::Constructor { data, case, fields } => {
                    return StructuralTerm::Constructor {
                        data,
                        case,
                        fields: fields
                            .into_iter()
                            .map(|(name, value)| (name, self.resolve_at(value, depth + 1)))
                            .collect(),
                    };
                }
                StructuralTerm::Application { machine, arguments } => {
                    let arguments: Vec<StructuralTerm> = arguments
                        .into_iter()
                        .map(|argument| self.resolve_at(argument, depth + 1))
                        .collect();
                    let resolved = StructuralTerm::Application { machine, arguments };
                    // Hypothesis rewrites first (the inductive hypothesis
                    // reduces the self-application), then unfolding.
                    if let Some((_, replacement)) = self
                        .rewrites
                        .iter()
                        .find(|(pattern, _)| pattern == &resolved)
                    {
                        term = replacement.clone();
                        continue;
                    }
                    let StructuralTerm::Application { machine, arguments } = &resolved else {
                        unreachable!();
                    };
                    if let Some(unfolded) = self.unfold_application(machine, arguments, depth + 1) {
                        term = unfolded;
                        continue;
                    }
                    return resolved;
                }
                StructuralTerm::Opaque(_) => return term,
            }
        }
        term
    }

    /// COMPUTE-MODE unfolding (N3): apply a single-state proof machine of
    /// the case-arm shape to structural arguments. The desugared arm guard
    /// is `subject == Data::Case` (membership lowers to that exact Binary at
    /// parse/lowering time), so arm selection reads the guard directly: the
    /// matched argument must RESOLVE to a constructor, the arm whose case
    /// matches fires, and its value expression converts to a term under an
    /// environment of callee params -> argument terms (payload bindings are
    /// case-tagged member reads off the subject and resolve to the
    /// constructor's field terms). Any name outside the environment aborts
    /// the unfold -- callee-scope names must never leak into caller-scope
    /// judgments. `None` = no unfold (never unsound; the application just
    /// stays opaque).
    fn unfold_application(
        &self,
        machine_name: &str,
        arguments: &[StructuralTerm],
        depth: usize,
    ) -> Option<StructuralTerm> {
        if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!("STRUCT unfold? {machine_name} args {arguments:?} depth {depth}");
        }
        if depth >= 32 {
            return None;
        }
        let program = self.program;
        let (machine_name, selected_machines) = split_structural_machine_name(machine_name);
        let machine = program.machines().iter().find(|machine| {
            machine.attached_data.is_none() && machine.name.as_str() == machine_name
        })?;
        let machine_parameters: Vec<&omega_typed_trees::data::TypeParameter> = program
            .machine_type_parameters(machine)
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter.kind,
                    omega_typed_trees::data::TypeParameterKind::Machine { .. }
                )
            })
            .collect();
        if machine_parameters.len() != selected_machines.len() {
            return None;
        }
        let machine_environment: Vec<(String, String)> = machine_parameters
            .iter()
            .zip(selected_machines)
            .map(|(parameter, selected)| (parameter.name.as_str().to_owned(), selected.to_owned()))
            .collect();
        let [state] = program.machine_states(machine) else {
            return None;
        };
        let parameters = program.state_parameters(state);
        if parameters.len() != arguments.len() {
            return None;
        }
        let environment: Vec<(String, StructuralTerm)> = parameters
            .iter()
            .zip(arguments.iter())
            .map(|(parameter, argument)| (parameter.name.as_str().to_owned(), argument.clone()))
            .collect();

        // CITE the callee's proven ensures first (extraction into consumer
        // proofs): a lemma with a functional `ensures result == <term>`
        // abstracts its body, so instantiating that ensures under the call
        // environment yields the result directly -- and it is the ONLY route
        // for an INDUCTIVE lemma whose body never finitely unfolds for a
        // symbolic argument (`add_zero_right(a) == a`). Sound because the
        // callee's ensures is proven in the same validation batch (a false
        // one raises its own error, so no compiling program cites an
        // unproven fact). Prefer it over body unfolding. REQUIRES-bearing
        // callees are EXCLUDED: their ensures is conditional and this path
        // has no site to discharge the condition at -- injecting it
        // unconditioned would be unsound (probed 2026-07-16). Their BODY
        // still unfolds below (computation is unconditional).
        let requires_bearing = program.machine_contracts(machine).iter().any(|contract| {
            matches!(
                contract.kind,
                omega_typed_trees::signature::SignatureContractKind::Requires
            ) && !program.proof_facts.span_or_empty(contract.facts).is_empty()
        });
        if !requires_bearing {
            for contract in program.machine_contracts(machine) {
                if !matches!(
                    contract.kind,
                    omega_typed_trees::signature::SignatureContractKind::Ensures
                ) {
                    continue;
                }
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let ProofFact::Expression(expression) = fact else {
                        continue;
                    };
                    if let Some(term) = self.functional_ensures_result(
                        *expression,
                        &environment,
                        &machine_environment,
                        depth + 1,
                    ) {
                        return Some(term);
                    }
                }
            }
        }

        let mut environment = environment;
        for statement in program.statement_table.statements(state.statement_nodes) {
            // A `let` (spelled, or the lowering's __hoist_N of a call-valued
            // terminal -- e.g. a definitional wrapper like
            // `snoc(s, x) = (append(s, [x]))`) BINDS: its initializer
            // termifies under the environment built so far and the local
            // joins it, so the terminal's name resolves. Mirrors the
            // sole-arm and case-arm recognizers.
            if is_arm_pattern_marker(statement) {
                continue; // exhaustiveness carrier, not shape
            }
            if let StatementNode::LocalData(local) = statement {
                let term = self.callee_term_with_machines(
                    local.initial_value,
                    &environment,
                    &machine_environment,
                    depth + 1,
                )?;
                environment.push((local.name.as_str().to_owned(), term));
                continue;
            }
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            if transition.continuation.is_valid() {
                return None;
            }
            let fires = match transition.guard {
                TransitionGuardNode::Always => true,
                TransitionGuardNode::When(guard) => {
                    let ExpressionNode::Binary(comparison) =
                        program.expression_table.expression(guard)
                    else {
                        return None;
                    };
                    if comparison.operator != BinaryOperator::Equal {
                        return None;
                    }
                    let subject = structural_term(program, comparison.left)?;
                    let StructuralTerm::Variable(subject_name) = subject else {
                        return None;
                    };
                    let case = structural_term(program, comparison.right)?;
                    let StructuralTerm::Constructor {
                        case: arm_case,
                        fields: arm_fields,
                        ..
                    } = case
                    else {
                        return None;
                    };
                    if !arm_fields.is_empty() {
                        return None;
                    }
                    let (_, subject_term) =
                        environment.iter().find(|(name, _)| name == &subject_name)?;
                    let StructuralTerm::Constructor { case: got_case, .. } =
                        self.resolve_at(subject_term.clone(), depth + 1)
                    else {
                        // The matched argument is not (yet) a constructor:
                        // arm selection is undecidable, no unfold.
                        return None;
                    };
                    got_case == arm_case
                }
            };
            if !fires {
                continue;
            }
            let TransitionTargetNode::Value(value) =
                program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            return self.callee_term_with_machines(
                *value,
                &environment,
                &machine_environment,
                depth + 1,
            );
        }
        None
    }

    /// If `ensures_fact` is exactly `result == <term>` (either orientation),
    /// convert `<term>` under the call environment -- the functional-result
    /// abstraction of a lemma. `None` for any other ensures shape.
    fn functional_ensures_result(
        &self,
        ensures_fact: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        machine_environment: &[(String, String)],
        depth: usize,
    ) -> Option<StructuralTerm> {
        let program = self.program;
        let ExpressionNode::Binary(binary) = program.expression_table.expression(ensures_fact)
        else {
            return None;
        };
        if binary.operator != BinaryOperator::Equal {
            return None;
        }
        let is_result = |handle: ExpressionHandle| {
            matches!(
                program.expression_table.expression(handle),
                ExpressionNode::Name(path)
                    if matches!(
                        program.expression_table.name_path_members(path.members),
                        [only] if only.as_str() == RESULT_BINDER
                    )
            )
        };
        let value = if is_result(binary.left) {
            binary.right
        } else if is_result(binary.right) {
            binary.left
        } else {
            return None;
        };
        self.callee_term_with_machines(value, environment, machine_environment, depth)
    }

    /// Convert a callee-body expression to a term under the call
    /// environment. Names must be callee parameters; case-tagged member
    /// reads (`a.prev`) index the bound constructor's fields; case literals
    /// and nested free calls recurse. Anything else aborts (None).
    fn callee_term(
        &self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        depth: usize,
    ) -> Option<StructuralTerm> {
        self.callee_term_with_machines(expression, environment, &[], depth)
    }

    fn callee_term_with_machines(
        &self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        machine_environment: &[(String, String)],
        depth: usize,
    ) -> Option<StructuralTerm> {
        if depth >= 32 {
            return None;
        }
        let program = self.program;
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                match members {
                    [single] => environment
                        .iter()
                        .find(|(name, _)| name == single.as_str())
                        .map(|(_, term)| term.clone()),
                    [first, second] => program
                        .data_definitions()
                        .iter()
                        .any(|definition| definition.name.as_str() == first.as_str())
                        .then(|| StructuralTerm::Constructor {
                            data: first.as_str().to_owned(),
                            case: second.as_str().to_owned(),
                            fields: Vec::new(),
                        }),
                    _ => None,
                }
            }
            ExpressionNode::Member(member) => {
                let receiver_term = self.callee_term_with_machines(
                    member.receiver,
                    environment,
                    machine_environment,
                    depth + 1,
                )?;
                match self.resolve_at(receiver_term, depth + 1) {
                    StructuralTerm::Constructor { fields, .. } => fields
                        .iter()
                        .find(|(name, _)| name == member.member.as_str())
                        .map(|(_, term)| term.clone()),
                    // A field read off a SYMBOLIC receiver names the caller's
                    // possibly nested place in the shared Opaque vocabulary --
                    // exactly how the caller-side termifier spells `a.num.neg`
                    // (display name), so citations over the same place line up.
                    StructuralTerm::Variable(name) => Some(StructuralTerm::Opaque(format!(
                        "{name}.{}",
                        member.member.as_str()
                    ))),
                    StructuralTerm::Opaque(inner) => Some(StructuralTerm::Opaque(format!(
                        "{inner}.{}",
                        member.member.as_str()
                    ))),
                    StructuralTerm::Application { .. } => None,
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                // Records (no case name) term as empty-case constructors,
                // mirroring the caller-side termifier.
                let case = literal
                    .case_name
                    .as_ref()
                    .map(|case| case.as_str())
                    .unwrap_or("");
                let mut fields: Vec<(String, StructuralTerm)> = Vec::new();
                for field in program.expression_table.struct_fields(literal.fields) {
                    fields.push((
                        field.name.as_str().to_owned(),
                        self.callee_term_with_machines(
                            field.value,
                            environment,
                            machine_environment,
                            depth + 1,
                        )?,
                    ));
                }
                fields.sort_by(|(left, _), (right, _)| left.cmp(right));
                Some(StructuralTerm::Constructor {
                    data: literal.type_name.as_str().to_owned(),
                    case: case.to_owned(),
                    fields,
                })
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    return None;
                }
                let mut arguments = Vec::new();
                for argument in program.expression_table.expression_handles(call.arguments) {
                    arguments.push(self.callee_term_with_machines(
                        *argument,
                        environment,
                        machine_environment,
                        depth + 1,
                    )?);
                }
                Some(StructuralTerm::Application {
                    machine: structural_call_machine_name(
                        call.target.as_str(),
                        &call.machine_arguments,
                        machine_environment,
                    ),
                    arguments,
                })
            }
            ExpressionNode::Boolean(value) => Some(StructuralTerm::Constructor {
                data: "bool".to_owned(),
                case: value.to_string(),
                fields: Vec::new(),
            }),
            // A structural theorem may recurse on an ordinary scalar
            // measure (`build(n - 1)`) while its result lives in proof data.
            // The structural judge does not interpret that scalar algebra;
            // retain it as an opaque operand so the self-application has the
            // correct arity and identity.  The separate arithmetic recursion
            // validator is solely responsible for proving the edge decreases.
            ExpressionNode::Binary(_) | ExpressionNode::Integer(_) => Some(StructuralTerm::Opaque(
                program.expression_table.display_name(expression),
            )),
            _ => None,
        }
    }

    /// Substitute variables in a term (used to instantiate the machine's
    /// own ensures as the INDUCTIVE HYPOTHESIS at a self-call: params -> the
    /// call's argument terms, `result` -> the application term).
    fn substitute_term(term: &StructuralTerm, map: &[(String, StructuralTerm)]) -> StructuralTerm {
        match term {
            StructuralTerm::Variable(name) => map
                .iter()
                .find(|(variable, _)| variable == name)
                .map(|(_, replacement)| replacement.clone())
                .unwrap_or_else(|| term.clone()),
            StructuralTerm::Constructor { data, case, fields } => StructuralTerm::Constructor {
                data: data.clone(),
                case: case.clone(),
                fields: fields
                    .iter()
                    .map(|(name, value)| (name.clone(), Self::substitute_term(value, map)))
                    .collect(),
            },
            StructuralTerm::Application { machine, arguments } => StructuralTerm::Application {
                machine: machine.clone(),
                arguments: arguments
                    .iter()
                    .map(|argument| Self::substitute_term(argument, map))
                    .collect(),
            },
            StructuralTerm::Opaque(display) => {
                // Symbolic record member places currently share the Opaque
                // vocabulary (`p.den`). Citation instantiation must still
                // alpha-substitute their exact root parameter; otherwise a
                // cited Rat law leaks callee names into the caller frame.
                // Restrict the rewrite to an exact `<parameter>.` prefix.
                // A symbolic static-machine application is also a legitimate
                // place root (`Middle(index).den`); retain that projection in
                // the Opaque place vocabulary.  A concrete constructor can be
                // projected structurally.  Arbitrary opaque arithmetic never
                // gains substring-rewrite semantics.
                for (parameter, replacement) in map {
                    let prefix = format!("{parameter}.");
                    let Some(suffix) = display.strip_prefix(&prefix) else {
                        continue;
                    };
                    return match replacement {
                        StructuralTerm::Variable(root) | StructuralTerm::Opaque(root) => {
                            StructuralTerm::Opaque(format!("{root}.{suffix}"))
                        }
                        StructuralTerm::Application { .. } => StructuralTerm::Opaque(format!(
                            "{}.{suffix}",
                            display_structural_term(replacement)
                        )),
                        StructuralTerm::Constructor { fields, .. } => fields
                            .iter()
                            .find(|(name, _)| name == suffix)
                            .map(|(_, value)| value.clone())
                            .unwrap_or_else(|| term.clone()),
                    };
                }
                term.clone()
            }
        }
    }

    /// Collect every self-application (calls to `machine_name`) in a term.
    fn self_applications<'term>(
        term: &'term StructuralTerm,
        machine_name: &str,
        found: &mut Vec<&'term StructuralTerm>,
    ) {
        match term {
            StructuralTerm::Application { machine, arguments } => {
                if machine == machine_name {
                    found.push(term);
                }
                for argument in arguments {
                    Self::self_applications(argument, machine_name, found);
                }
            }
            StructuralTerm::Constructor { fields, .. } => {
                for (_, value) in fields {
                    Self::self_applications(value, machine_name, found);
                }
            }
            _ => {}
        }
    }

    fn judge(&self, program: &TypedTrees, fact: ExpressionHandle) -> StructuralJudgment {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
            // A boolean-valued proof call is itself a proposition. Resolve a
            // closed checked application exactly as `call == true`; N6
            // equivalence laws use this ordinary contract shape.
            let Some(term) = structural_term(program, fact) else {
                return StructuralJudgment::Unknown;
            };
            return match self.resolve(term) {
                StructuralTerm::Constructor { data, case, fields }
                    if data == "bool" && case == "true" && fields.is_empty() =>
                {
                    StructuralJudgment::Proven
                }
                StructuralTerm::Constructor { data, case, fields }
                    if data == "bool" && case == "false" && fields.is_empty() =>
                {
                    StructuralJudgment::Refuted
                }
                _ => StructuralJudgment::Unknown,
            };
        };
        match binary.operator {
            BinaryOperator::And => {
                match (
                    self.judge(program, binary.left),
                    self.judge(program, binary.right),
                ) {
                    (StructuralJudgment::Proven, StructuralJudgment::Proven) => {
                        StructuralJudgment::Proven
                    }
                    (StructuralJudgment::Refuted, _) | (_, StructuralJudgment::Refuted) => {
                        StructuralJudgment::Refuted
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
                let equality = self.judge_equation(self.resolve(left), self.resolve(right), 0);
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

    /// Judge one resolved structural equation: identical terms prove,
    /// same-case constructors decompose pairwise (all fields prove =>
    /// proven, any refutes => refuted), distinct cases refute. A stuck
    /// equation gets the REARRANGE tier before standing down: under a ring
    /// license, both sides flatten to addend MULTISETS over the licensed op
    /// (the commutativity + associativity closure the carrier's conformance
    /// proved) -- equal multisets prove; unequal ones stay Unknown (atoms may
    /// alias, so rearrangement never refutes).
    fn judge_equation(
        &self,
        left: StructuralTerm,
        right: StructuralTerm,
        depth: usize,
    ) -> StructuralJudgment {
        if depth >= 32 {
            return StructuralJudgment::Unknown;
        }
        if left == right {
            return StructuralJudgment::Proven;
        }
        let (
            StructuralTerm::Constructor {
                data: data_l,
                case: case_l,
                fields: fields_l,
            },
            StructuralTerm::Constructor {
                data: data_r,
                case: case_r,
                fields: fields_r,
            },
        ) = (&left, &right)
        else {
            // RECORD ETA (product extensionality): a record literal that
            // rebuilds EVERY declared field of a variable from that same
            // variable (`IntPair { neg: a.neg, pos: a.pos } == a`) IS the
            // variable -- the shape identity lemmas reduce to. Field values
            // must be the variable's own field reads by name; a permuted
            // rebuild (neg: a.pos) does NOT match.
            if self.record_eta_matches(&left, &right) || self.record_eta_matches(&right, &left) {
                return StructuralJudgment::Proven;
            }
            if self.ring_rearranged_equal(&left, &right) {
                return StructuralJudgment::Proven;
            }
            return StructuralJudgment::Unknown;
        };
        if data_l != data_r {
            return StructuralJudgment::Unknown;
        }
        if case_l != case_r {
            return StructuralJudgment::Refuted;
        }
        let mut verdict = StructuralJudgment::Proven;
        for (name_l, value_l) in fields_l {
            let Some((_, value_r)) = fields_r.iter().find(|(name_r, _)| name_r == name_l) else {
                verdict = StructuralJudgment::Unknown;
                continue;
            };
            match self.judge_equation(value_l.clone(), value_r.clone(), depth + 1) {
                StructuralJudgment::Proven => {}
                StructuralJudgment::Refuted => return StructuralJudgment::Refuted,
                StructuralJudgment::Unknown => verdict = StructuralJudgment::Unknown,
            }
        }
        verdict
    }

    /// RECORD ETA: does `constructor` rebuild every declared field of the
    /// plain-record variable `variable` from that variable's own field
    /// reads? All fields must be present, matched BY NAME to `{v}.{field}`
    /// opaques -- a permutation or a partial rebuild does not match.
    fn record_eta_matches(&self, constructor: &StructuralTerm, variable: &StructuralTerm) -> bool {
        let StructuralTerm::Constructor { data, case, fields } = constructor else {
            return false;
        };
        if !case.is_empty() {
            return false;
        }
        let StructuralTerm::Variable(name) = variable else {
            return false;
        };
        let Some(definition) = self
            .program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == data.as_str())
        else {
            return false;
        };
        let declared: Vec<&str> = self
            .program
            .data_members(definition)
            .iter()
            .filter_map(|member| match member {
                omega_typed_trees::data::DataMember::Field(field) => Some(field.name.as_str()),
                omega_typed_trees::data::DataMember::Variant(_) => None,
            })
            .collect();
        if declared.is_empty() || declared.len() != fields.len() {
            return false;
        }
        declared.iter().all(|field_name| {
            fields.iter().any(|(field, value)| {
                field == field_name
                    && matches!(
                        self.resolve(value.clone()),
                        StructuralTerm::Opaque(opaque)
                            if opaque == format!("{name}.{field_name}")
                    )
            })
        })
    }

    /// The rearrange tier's comparison: for each ring license whose op
    /// appears in the equation, flatten both sides into addend multisets
    /// (nested applications of the licensed op associate away; everything
    /// else is an atom by canonical display) and compare. At least two
    /// addends must appear -- a single atom has nothing to rearrange.
    fn ring_rearranged_equal(&self, left: &StructuralTerm, right: &StructuralTerm) -> bool {
        for license in &self.ring_licenses {
            let op = license.add_machine.as_str();
            if !term_uses_application(left, op) && !term_uses_application(right, op) {
                continue;
            }
            let mut left_addends = Vec::new();
            additive_addends(left, op, &mut left_addends);
            let mut right_addends = Vec::new();
            additive_addends(right, op, &mut right_addends);
            if left_addends.len() < 2 {
                continue;
            }
            left_addends.sort();
            right_addends.sort();
            if left_addends.len() == right_addends.len() && left_addends == right_addends {
                return true;
            }
            // HYPOTHESIS EXCHANGE (bounded, depth 2): a requires / citation /
            // IH equation whose sides flatten over this SAME licensed op
            // licenses swapping that sub-multiset of addends -- sum(left) ==
            // sum(left - from + to) because sum(from) == sum(to) is the
            // hypothesis and the op's comm+assoc closure is exactly what the
            // license's conformance proved. This is what makes QUOTIENT
            // lemmas provable: congruence needs ONE exchange (a.pos + a2.neg
            // exchanges inside a.pos + b.pos + a2.neg + b.neg), transitivity
            // needs TWO (h1 then h2 inside the cancellation citation's
            // requires). Whole-term matches were already rewritten during
            // resolve; this reaches the sub-multisets the rewriter cannot
            // see. Frontier-capped BFS -- over-refusal past the cap, never
            // unsound.
            let mut frontier: Vec<Vec<String>> = vec![left_addends.clone()];
            for _depth in 0..2 {
                let mut next: Vec<Vec<String>> = Vec::new();
                for current in &frontier {
                    for (pattern, replacement) in &self.rewrites {
                        for (from, to) in [(pattern, replacement), (replacement, pattern)] {
                            let mut from_addends = Vec::new();
                            additive_addends(from, op, &mut from_addends);
                            let mut to_addends = Vec::new();
                            additive_addends(to, op, &mut to_addends);
                            from_addends.sort();
                            let Some(mut candidate) =
                                sorted_multiset_subtract(current, &from_addends)
                            else {
                                continue;
                            };
                            candidate.extend(to_addends.iter().cloned());
                            candidate.sort();
                            if candidate == right_addends {
                                return true;
                            }
                            if next.len() < 64 {
                                next.push(candidate);
                            }
                        }
                    }
                }
                frontier = next;
                if frontier.is_empty() {
                    break;
                }
            }
        }
        // Tier-2 FULL POLYNOMIAL: under a paired license, both sides
        // normalize by distributing the licensed mul through the licensed
        // add into a multiset of monomials (each a sorted factor multiset).
        // The distributivity law is CONFORMED (machine-checked), so the
        // normal form is exactly what the carrier proved.
        for license in &self.semiring_licenses {
            if !term_uses_application(left, &license.mul_machine)
                && !term_uses_application(right, &license.mul_machine)
            {
                continue;
            }
            let (Some(mut left_poly), Some(mut right_poly)) = (
                polynomial_normal_form(left, license),
                polynomial_normal_form(right, license),
            ) else {
                continue;
            };
            left_poly.sort();
            right_poly.sort();
            if left_poly == right_poly {
                return true;
            }
            // SCALED-HYPOTHESIS EXCHANGE (tier-2 twin of the addend
            // exchange): a hypothesis equation polynomial-normalizes to a
            // monomial-multiset pair (hl, hr), and multiplying BOTH sides by
            // any monomial factor m keeps it an equation (the semiring's
            // conformed distributivity is exactly that license), so hl*m
            // exchanges for hr*m inside the goal's monomials. Factors are
            // drawn from the goal's own atoms (plus unscaled) -- this is
            // what proves mul-CONGRUENCE over a quotient: the cross-sum
            // hypothesis scaled by b.pos and by b.neg equalizes the product
            // components in two exchanges. Depth-2 frontier-capped BFS.
            let mut atoms: Vec<String> = left_poly
                .iter()
                .chain(right_poly.iter())
                .flatten()
                .cloned()
                .collect();
            atoms.sort();
            atoms.dedup();
            let mut scales: Vec<Vec<String>> = vec![Vec::new()];
            scales.extend(atoms.into_iter().map(|atom| vec![atom]));
            let mut hypothesis_polys: Vec<(Vec<Vec<String>>, Vec<Vec<String>>)> = Vec::new();
            for (pattern, replacement) in &self.rewrites {
                if let (Some(mut hl), Some(mut hr)) = (
                    polynomial_normal_form(pattern, license),
                    polynomial_normal_form(replacement, license),
                ) {
                    hl.sort();
                    hr.sort();
                    hypothesis_polys.push((hl, hr));
                }
            }
            let scaled = |poly: &[Vec<String>], scale: &[String]| -> Vec<Vec<String>> {
                poly.iter()
                    .map(|monomial| {
                        let mut product = monomial.clone();
                        product.extend(scale.iter().cloned());
                        product.sort();
                        product
                    })
                    .collect()
            };
            let mut frontier: Vec<Vec<Vec<String>>> = vec![left_poly.clone()];
            for _depth in 0..2 {
                let mut next: Vec<Vec<Vec<String>>> = Vec::new();
                for current in &frontier {
                    for (hypothesis_left, hypothesis_right) in &hypothesis_polys {
                        for (from, to) in [
                            (hypothesis_left, hypothesis_right),
                            (hypothesis_right, hypothesis_left),
                        ] {
                            for scale in &scales {
                                let from_scaled = scaled(from, scale);
                                let Some(mut candidate) =
                                    sorted_multiset_subtract(current, &from_scaled)
                                else {
                                    continue;
                                };
                                candidate.extend(scaled(to, scale));
                                candidate.sort();
                                if candidate == right_poly {
                                    return true;
                                }
                                if next.len() < 64 {
                                    next.push(candidate);
                                }
                            }
                        }
                    }
                }
                frontier = next;
                if frontier.is_empty() {
                    break;
                }
            }
        }
        false
    }
}

/// Distribute the licensed mul through the licensed add: a term becomes a
/// list of MONOMIALS (sorted factor lists). `None` past the size cap (the
/// cross product is quadratic; a runaway form refuses into the ordinary
/// path rather than stalling).
fn polynomial_normal_form(
    term: &StructuralTerm,
    license: &SemiringLicense,
) -> Option<Vec<Vec<String>>> {
    const MONOMIAL_CAP: usize = 64;
    if let StructuralTerm::Application { machine, arguments } = term
        && arguments.len() == 2
    {
        if *machine == license.add_machine {
            let mut left = polynomial_normal_form(&arguments[0], license)?;
            let right = polynomial_normal_form(&arguments[1], license)?;
            left.extend(right);
            return (left.len() <= MONOMIAL_CAP).then_some(left);
        }
        if *machine == license.mul_machine {
            let left = polynomial_normal_form(&arguments[0], license)?;
            let right = polynomial_normal_form(&arguments[1], license)?;
            let mut product = Vec::new();
            for left_monomial in &left {
                for right_monomial in &right {
                    let mut monomial = left_monomial.clone();
                    monomial.extend(right_monomial.iter().cloned());
                    monomial.sort();
                    product.push(monomial);
                }
            }
            return (product.len() <= MONOMIAL_CAP).then_some(product);
        }
    }
    Some(vec![vec![display_structural_term(term)]])
}

/// `left - from` as multisets; `None` when `from` is not a sub-multiset of
/// `left` (the exchange does not apply). Generic over the element (tier-1
/// addend displays, tier-2 monomial factor lists).
fn sorted_multiset_subtract<T: Clone + PartialEq>(left: &[T], from: &[T]) -> Option<Vec<T>> {
    let mut remaining = left.to_vec();
    for item in from {
        let index = remaining.iter().position(|candidate| candidate == item)?;
        remaining.remove(index);
    }
    Some(remaining)
}

/// Flatten nested applications of the licensed op into its addend list; any
/// other term is one addend, compared by canonical display (the Opaque
/// discipline).
fn additive_addends(term: &StructuralTerm, op: &str, out: &mut Vec<String>) {
    if let StructuralTerm::Application { machine, arguments } = term {
        if machine == op && arguments.len() == 2 {
            additive_addends(&arguments[0], op, out);
            additive_addends(&arguments[1], op, out);
            return;
        }
    }
    out.push(display_structural_term(term));
}

fn term_uses_application(term: &StructuralTerm, op: &str) -> bool {
    match term {
        StructuralTerm::Application { machine, arguments } => {
            machine == op
                || arguments
                    .iter()
                    .any(|argument| term_uses_application(argument, op))
        }
        StructuralTerm::Constructor { fields, .. } => fields
            .iter()
            .any(|(_, value)| term_uses_application(value, op)),
        _ => false,
    }
}

/// Compute the program's REARRANGE licenses (settle 2026-07-18): for every
/// trait, find op slots carrying BOTH a commutativity law and an
/// associativity law (matched by SHAPE over the trait's own requirement
/// names), then license each conforming op machine whose carrier also has
/// satisfiers for both law slots. Conformance is the license -- rung B
/// machine-checked those satisfiers against the declared laws, so the
/// closure the canonicalizer assumes is exactly what the carrier proved.
///
/// NO CIRCULAR LICENSING: a machine that itself binds a comm/assoc LAW slot
/// of a trait gets NO licenses from that trait -- the axiom base always
/// proves ring-free. This kills self-licensing (add_comm rearranging its own
/// goal into triviality) AND multi-machine cycles (two comm satisfiers each
/// licensed by the other's conformance, none carrying a real proof).
fn compute_ring_licenses(program: &TypedTrees, judged_machine: &Machine) -> Vec<RingLicense> {
    let mut licenses = Vec::new();

    for trait_definition in program.traits() {
        // Op slot name -> (has commutativity law named, has associativity law
        // named): the LAW requirement names matter later (their satisfiers
        // must exist for the carrier).
        let mut comm_laws: Vec<(String, String)> = Vec::new(); // (op, law requirement)
        let mut assoc_laws: Vec<(String, String)> = Vec::new();

        for requirement in program.trait_machine_signatures(trait_definition) {
            let parameters: Vec<String> = program
                .state_signature_parameters(requirement)
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect();
            for contract in program.state_signature_contracts(requirement) {
                if contract.kind != SignatureContractKind::Ensures {
                    continue;
                }
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let ProofFact::Expression(expression) = fact else {
                        continue;
                    };
                    let mut conjuncts = Vec::new();
                    collect_equality_conjuncts(program, *expression, &mut conjuncts);
                    for conjunct in conjuncts {
                        let ExpressionNode::Binary(binary) =
                            program.expression_table.expression(conjunct)
                        else {
                            continue;
                        };
                        let (Some(left), Some(right)) = (
                            structural_term(program, binary.left),
                            structural_term(program, binary.right),
                        ) else {
                            continue;
                        };
                        if let Some(op) = commutativity_shape(&left, &right, &parameters) {
                            comm_laws.push((op, requirement.name.as_str().to_owned()));
                        }
                        if let Some(op) = associativity_shape(&left, &right, &parameters) {
                            assoc_laws.push((op, requirement.name.as_str().to_owned()));
                        }
                    }
                }
            }
        }

        // PER-LICENSE circularity break (refined 2026-07-16 from the old
        // trait-wide skip): the judged machine is excluded only from
        // licenses it ITSELF underpins -- the ones whose comm/assoc law
        // slots it binds FOR THE SAME CARRIER. A law lemma's goal is
        // exactly the law shape over its own op, so no other carrier's
        // license can rearrange it -- per-carrier exclusion breaks every
        // cycle while letting IntPair's mul_comm keep using NAT's earned
        // licenses (the trait-wide skip wrongly stripped those).
        let judged_bound_laws: Vec<String> = program
            .machine_trait_conformances(judged_machine)
            .iter()
            .filter(|conformance| conformance.symbol == trait_definition.symbol)
            .filter_map(|conformance| {
                conformance
                    .requirement
                    .as_ref()
                    .map(|name| name.as_str().to_owned())
                    .or_else(|| {
                        judged_machine
                            .attached_data
                            .is_none()
                            .then(|| judged_machine.name.as_str().to_owned())
                    })
            })
            .collect();
        let judged_carrier = program.machine_states(judged_machine).first().map(|entry| {
            program
                .state_parameters(entry)
                .first()
                .map(|parameter| parameter.type_reference)
                .unwrap_or(entry.return_type)
        });

        for (op_slot, comm_law) in &comm_laws {
            let Some((_, assoc_law)) = assoc_laws.iter().find(|(op, _)| op == op_slot) else {
                continue;
            };
            // Every machine conforming the op slot is a candidate license --
            // provided its carrier also conformed BOTH law slots.
            for candidate in program.machines() {
                for conformance in program.machine_trait_conformances(candidate) {
                    if conformance.symbol != trait_definition.symbol {
                        continue;
                    }
                    let bound_requirement = conformance
                        .requirement
                        .as_ref()
                        .map(|name| name.as_str().to_owned())
                        .or_else(|| {
                            candidate
                                .attached_data
                                .is_none()
                                .then(|| candidate.name.as_str().to_owned())
                        });
                    if bound_requirement.as_deref() != Some(op_slot.as_str()) {
                        continue;
                    }
                    let Some(candidate_entry) = program.machine_states(candidate).first() else {
                        continue;
                    };
                    let carrier = program
                        .state_parameters(candidate_entry)
                        .first()
                        .map(|parameter| parameter.type_reference)
                        .unwrap_or(candidate_entry.return_type);
                    let judged_underpins_this_license = judged_bound_laws
                        .iter()
                        .any(|law| law == comm_law || law == assoc_law)
                        && judged_carrier.is_some_and(|judged| {
                            crate::type_references::type_references_match(program, judged, carrier)
                        });
                    if judged_underpins_this_license {
                        continue;
                    }
                    if slot_satisfier_exists(program, trait_definition, comm_law, carrier)
                        && slot_satisfier_exists(program, trait_definition, assoc_law, carrier)
                    {
                        licenses.push(RingLicense {
                            add_machine: candidate.name.as_str().to_owned(),
                        });
                    }
                }
            }
        }
    }

    licenses
}

/// Whether SOME machine conforms `(trait, requirement)` for this carrier.
fn slot_satisfier_exists(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    requirement_name: &str,
    carrier: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    program.machines().iter().any(|candidate| {
        program
            .machine_trait_conformances(candidate)
            .iter()
            .any(|conformance| {
                if conformance.symbol != trait_definition.symbol {
                    return false;
                }
                let bound_requirement = conformance
                    .requirement
                    .as_ref()
                    .map(|name| name.as_str().to_owned())
                    .or_else(|| {
                        candidate
                            .attached_data
                            .is_none()
                            .then(|| candidate.name.as_str().to_owned())
                    });
                if bound_requirement.as_deref() != Some(requirement_name) {
                    return false;
                }
                let Some(candidate_entry) = program.machine_states(candidate).first() else {
                    return false;
                };
                let candidate_carrier = program
                    .state_parameters(candidate_entry)
                    .first()
                    .map(|parameter| parameter.type_reference)
                    .unwrap_or(candidate_entry.return_type);
                crate::type_references::type_references_match(program, candidate_carrier, carrier)
            })
    })
}

/// `R(x, y) == R(y, x)` with `x`/`y` DISTINCT requirement parameters -> the
/// op slot `R` is declared commutative by this law.
/// Tier-2: recognize `mul(a, add(b, c)) == add(mul(a, b), mul(a, c))` up
/// to parameter naming -- returns (mul_op, add_op).
fn distributivity_shape(
    left: &StructuralTerm,
    right: &StructuralTerm,
    parameters: &[String],
) -> Option<(String, String)> {
    // left = mul(a, add(b, c))
    let StructuralTerm::Application {
        machine: mul_op,
        arguments: mul_args,
    } = left
    else {
        return None;
    };
    let [
        StructuralTerm::Variable(a),
        StructuralTerm::Application {
            machine: add_op,
            arguments: add_args,
        },
    ] = mul_args.as_slice()
    else {
        return None;
    };
    let [StructuralTerm::Variable(b), StructuralTerm::Variable(c)] = add_args.as_slice() else {
        return None;
    };
    if mul_op == add_op {
        return None;
    }
    for name in [a, b, c] {
        if !parameters.contains(name) {
            return None;
        }
    }
    // right = add(mul(a, b), mul(a, c))
    let StructuralTerm::Application {
        machine: outer_add,
        arguments: outer_args,
    } = right
    else {
        return None;
    };
    if outer_add != add_op {
        return None;
    }
    let [
        StructuralTerm::Application {
            machine: left_mul,
            arguments: left_args,
        },
        StructuralTerm::Application {
            machine: right_mul,
            arguments: right_args,
        },
    ] = outer_args.as_slice()
    else {
        return None;
    };
    if left_mul != mul_op || right_mul != mul_op {
        return None;
    }
    let (
        [StructuralTerm::Variable(la), StructuralTerm::Variable(lb)],
        [StructuralTerm::Variable(ra), StructuralTerm::Variable(rc)],
    ) = (left_args.as_slice(), right_args.as_slice())
    else {
        return None;
    };
    (la == a && lb == b && ra == a && rc == c).then(|| (mul_op.clone(), add_op.clone()))
}

/// Tier-2 licensing: a trait carrying comm+assoc for BOTH an add op and a
/// mul op, plus a DISTRIBUTIVITY law connecting them, licenses each carrier
/// that conformed ALL FIVE law slots. Same no-circularity rule: the judged
/// machine binding ANY involved law slot gets nothing from this trait.
fn compute_semiring_licenses(
    program: &TypedTrees,
    judged_machine: &Machine,
) -> Vec<SemiringLicense> {
    let mut licenses = Vec::new();
    for trait_definition in program.traits() {
        let mut comm_laws: Vec<(String, String)> = Vec::new();
        let mut assoc_laws: Vec<(String, String)> = Vec::new();
        let mut dist_laws: Vec<(String, String, String)> = Vec::new(); // (mul, add, law)
        for requirement in program.trait_machine_signatures(trait_definition) {
            let parameters: Vec<String> = program
                .state_signature_parameters(requirement)
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect();
            for contract in program.state_signature_contracts(requirement) {
                if contract.kind != SignatureContractKind::Ensures {
                    continue;
                }
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let ProofFact::Expression(expression) = fact else {
                        continue;
                    };
                    let mut conjuncts = Vec::new();
                    collect_equality_conjuncts(program, *expression, &mut conjuncts);
                    for conjunct in conjuncts {
                        let ExpressionNode::Binary(binary) =
                            program.expression_table.expression(conjunct)
                        else {
                            continue;
                        };
                        let (Some(left), Some(right)) = (
                            structural_term(program, binary.left),
                            structural_term(program, binary.right),
                        ) else {
                            continue;
                        };
                        if let Some(op) = commutativity_shape(&left, &right, &parameters) {
                            comm_laws.push((op, requirement.name.as_str().to_owned()));
                        }
                        if let Some(op) = associativity_shape(&left, &right, &parameters) {
                            assoc_laws.push((op, requirement.name.as_str().to_owned()));
                        }
                        if let Some((mul_op, add_op)) =
                            distributivity_shape(&left, &right, &parameters)
                        {
                            dist_laws.push((mul_op, add_op, requirement.name.as_str().to_owned()));
                        }
                    }
                }
            }
        }
        for (mul_op, add_op, dist_law) in &dist_laws {
            let Some((_, add_comm)) = comm_laws.iter().find(|(op, _)| op == add_op) else {
                continue;
            };
            let Some((_, add_assoc)) = assoc_laws.iter().find(|(op, _)| op == add_op) else {
                continue;
            };
            let Some((_, mul_comm)) = comm_laws.iter().find(|(op, _)| op == mul_op) else {
                continue;
            };
            let Some((_, mul_assoc)) = assoc_laws.iter().find(|(op, _)| op == mul_op) else {
                continue;
            };
            let law_slots = [add_comm, add_assoc, mul_comm, mul_assoc, dist_law];
            // PER-LICENSE circularity break (refined 2026-07-16, mirroring
            // compute_ring_licenses): the judged machine is excluded only
            // from paired licenses it underpins -- binding one of the five
            // law slots FOR THE SAME CARRIER. A law lemma's goal is the law
            // shape over its own carrier's ops, so other carriers' licenses
            // cannot rearrange it.
            let judged_bound_laws: Vec<String> = program
                .machine_trait_conformances(judged_machine)
                .iter()
                .filter(|conformance| conformance.symbol == trait_definition.symbol)
                .filter_map(|conformance| {
                    conformance
                        .requirement
                        .as_ref()
                        .map(|name| name.as_str().to_owned())
                        .or_else(|| {
                            judged_machine
                                .attached_data
                                .is_none()
                                .then(|| judged_machine.name.as_str().to_owned())
                        })
                })
                .filter(|name| law_slots.iter().any(|law| law.as_str() == name))
                .collect();
            let judged_carrier = program.machine_states(judged_machine).first().map(|entry| {
                program
                    .state_parameters(entry)
                    .first()
                    .map(|parameter| parameter.type_reference)
                    .unwrap_or(entry.return_type)
            });
            // Each carrier conforming BOTH op slots with all five law slots
            // satisfied earns the paired license.
            for add_candidate in program.machines() {
                for conformance in program.machine_trait_conformances(add_candidate) {
                    if conformance.symbol != trait_definition.symbol {
                        continue;
                    }
                    let bound = conformance
                        .requirement
                        .as_ref()
                        .map(|name| name.as_str().to_owned())
                        .or_else(|| {
                            add_candidate
                                .attached_data
                                .is_none()
                                .then(|| add_candidate.name.as_str().to_owned())
                        });
                    if bound.as_deref() != Some(add_op.as_str()) {
                        continue;
                    }
                    let Some(entry) = program.machine_states(add_candidate).first() else {
                        continue;
                    };
                    let carrier = program
                        .state_parameters(entry)
                        .first()
                        .map(|parameter| parameter.type_reference)
                        .unwrap_or(entry.return_type);
                    if !judged_bound_laws.is_empty()
                        && judged_carrier.is_some_and(|judged| {
                            crate::type_references::type_references_match(program, judged, carrier)
                        })
                    {
                        continue;
                    }
                    if !law_slots
                        .iter()
                        .all(|law| slot_satisfier_exists(program, trait_definition, law, carrier))
                    {
                        continue;
                    }
                    if let Some(mul_machine) =
                        op_slot_satisfier(program, trait_definition, mul_op, carrier)
                    {
                        licenses.push(SemiringLicense {
                            add_machine: add_candidate.name.as_str().to_owned(),
                            mul_machine,
                        });
                    }
                }
            }
        }
    }
    licenses
}

/// The NAME of the machine conforming `op_slot` for the given carrier.
fn op_slot_satisfier(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    op_slot: &str,
    carrier: omega_typed_trees::types::TypeReferenceHandle,
) -> Option<String> {
    for candidate in program.machines() {
        for conformance in program.machine_trait_conformances(candidate) {
            if conformance.symbol != trait_definition.symbol {
                continue;
            }
            let bound = conformance
                .requirement
                .as_ref()
                .map(|name| name.as_str().to_owned())
                .or_else(|| {
                    candidate
                        .attached_data
                        .is_none()
                        .then(|| candidate.name.as_str().to_owned())
                });
            if bound.as_deref() != Some(op_slot) {
                continue;
            }
            let Some(entry) = program.machine_states(candidate).first() else {
                continue;
            };
            let candidate_carrier = program
                .state_parameters(entry)
                .first()
                .map(|parameter| parameter.type_reference)
                .unwrap_or(entry.return_type);
            if crate::type_references::type_references_match(program, candidate_carrier, carrier) {
                return Some(candidate.name.as_str().to_owned());
            }
        }
    }
    None
}

fn commutativity_shape(
    left: &StructuralTerm,
    right: &StructuralTerm,
    parameters: &[String],
) -> Option<String> {
    let StructuralTerm::Application {
        machine: op_l,
        arguments: args_l,
    } = left
    else {
        return None;
    };
    let StructuralTerm::Application {
        machine: op_r,
        arguments: args_r,
    } = right
    else {
        return None;
    };
    if op_l != op_r || args_l.len() != 2 || args_r.len() != 2 {
        return None;
    }
    let [StructuralTerm::Variable(x), StructuralTerm::Variable(y)] = args_l.as_slice() else {
        return None;
    };
    let [StructuralTerm::Variable(rx), StructuralTerm::Variable(ry)] = args_r.as_slice() else {
        return None;
    };
    let is_parameter = |name: &String| parameters.iter().any(|parameter| parameter == name);
    (x != y && rx == y && ry == x && is_parameter(x) && is_parameter(y)).then(|| op_l.clone())
}

/// `R(R(x, y), z) == R(x, R(y, z))` (either orientation) with distinct
/// requirement parameters -> the op slot `R` is declared associative.
fn associativity_shape(
    left: &StructuralTerm,
    right: &StructuralTerm,
    parameters: &[String],
) -> Option<String> {
    for (first, second) in [(left, right), (right, left)] {
        let StructuralTerm::Application {
            machine: op_outer,
            arguments: outer_args,
        } = first
        else {
            continue;
        };
        if outer_args.len() != 2 {
            continue;
        }
        let StructuralTerm::Application {
            machine: op_inner,
            arguments: inner_args,
        } = &outer_args[0]
        else {
            continue;
        };
        if op_inner != op_outer || inner_args.len() != 2 {
            continue;
        }
        let (StructuralTerm::Variable(x), StructuralTerm::Variable(y), StructuralTerm::Variable(z)) =
            (&inner_args[0], &inner_args[1], &outer_args[1])
        else {
            continue;
        };
        let StructuralTerm::Application {
            machine: op_right,
            arguments: right_args,
        } = second
        else {
            continue;
        };
        if op_right != op_outer || right_args.len() != 2 {
            continue;
        }
        let StructuralTerm::Variable(rx) = &right_args[0] else {
            continue;
        };
        let StructuralTerm::Application {
            machine: op_right_inner,
            arguments: right_inner_args,
        } = &right_args[1]
        else {
            continue;
        };
        if op_right_inner != op_outer || right_inner_args.len() != 2 {
            continue;
        }
        let (StructuralTerm::Variable(ry), StructuralTerm::Variable(rz)) =
            (&right_inner_args[0], &right_inner_args[1])
        else {
            continue;
        };
        let is_parameter = |name: &String| parameters.iter().any(|parameter| parameter == name);
        let distinct = x != y && y != z && x != z;
        if distinct
            && rx == x
            && ry == y
            && rz == z
            && is_parameter(x)
            && is_parameter(y)
            && is_parameter(z)
        {
            return Some(op_outer.clone());
        }
    }
    None
}

/// Whether `haystack` contains `needle` as a subterm (occurs check for the
/// rewrite orientation: a rewrite whose replacement contains its own pattern
/// would loop; the resolution cap would still bound it, but skipping keeps
/// resolution productive).
fn term_contains(haystack: &StructuralTerm, needle: &StructuralTerm) -> bool {
    if haystack == needle {
        return true;
    }
    match haystack {
        StructuralTerm::Constructor { fields, .. } => {
            fields.iter().any(|(_, value)| term_contains(value, needle))
        }
        StructuralTerm::Application { arguments, .. } => arguments
            .iter()
            .any(|argument| term_contains(argument, needle)),
        _ => false,
    }
}

/// Read an expression as a structural term. Single-segment names are
/// variables; a two-segment path whose head names a data definition is a
/// nullary case constructor; a case literal (`(Nat::Succ { prev: a })`) is a
/// payload-carrying constructor with fields sorted by name; everything else
/// is opaque by display name (identical applications still prove reflexively
/// through term equality).
/// N4 identity-law bridging: a NULLARY application of a trivial CONSTANT
/// machine (single state, a single un-guarded transition to a constructor
/// terminal -- the settled zero/one shape) normalizes to that constructor.
fn unfold_constant_applications(program: &TypedTrees, term: StructuralTerm) -> StructuralTerm {
    match term {
        StructuralTerm::Application { machine, arguments } if arguments.is_empty() => {
            match constant_machine_constructor(program, &machine) {
                Some(constructor) => constructor,
                None => StructuralTerm::Application { machine, arguments },
            }
        }
        StructuralTerm::Application { machine, arguments } => StructuralTerm::Application {
            machine,
            arguments: arguments
                .into_iter()
                .map(|argument| unfold_constant_applications(program, argument))
                .collect(),
        },
        StructuralTerm::Constructor { data, case, fields } => StructuralTerm::Constructor {
            data,
            case,
            fields: fields
                .into_iter()
                .map(|(name, value)| (name, unfold_constant_applications(program, value)))
                .collect(),
        },
        other => other,
    }
}

/// The constructor value a trivial CONSTANT machine returns, when its shape
/// is exactly the settled one: a single state whose only statement is an
/// un-guarded transition to a VALUE target that is a closed constructor.
fn constant_machine_constructor(program: &TypedTrees, name: &str) -> Option<StructuralTerm> {
    use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    let machine = program.machines().iter().find(|machine| {
        machine.name.as_str() == name
            || machine
                .name
                .as_str()
                .rsplit("::")
                .next()
                .is_some_and(|simple| simple == name)
    })?;
    let states = program.machine_states(machine);
    let [state] = states else {
        return None;
    };
    let non_marker: Vec<&StatementNode> = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter(|statement| !is_arm_pattern_marker(statement))
        .collect();
    let [statement] = non_marker[..] else {
        return None;
    };
    let StatementNode::Transition(transition) = statement else {
        return None;
    };
    if !matches!(transition.guard, TransitionGuardNode::Always) {
        return None;
    }
    let TransitionTargetNode::Value(value) =
        program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    let term = structural_term(program, *value)?;
    // Closed constructors only (no variables -- a constant).
    fn is_closed(term: &StructuralTerm) -> bool {
        match term {
            StructuralTerm::Constructor { fields, .. } => {
                fields.iter().all(|(_, value)| is_closed(value))
            }
            _ => false,
        }
    }
    is_closed(&term).then_some(term)
}

/// Preserve compile-time machine selections in structural application
/// identity. Static machine arguments are part of a call's meaning:
/// `f<A>(x)` and `f<B>(x)` must never collapse to the same proof term.
/// During generic body unfolding, selections are alpha-substituted through
/// `machine_environment` (`Sequence` -> `unit_sample`) exactly as value
/// parameters are substituted through the ordinary term environment.
fn structural_call_machine_name(
    target: &str,
    machine_arguments: &[omega_typed_trees::expression::StaticMachineArgument],
    machine_environment: &[(String, String)],
) -> String {
    let substitute = |name: String| {
        machine_environment
            .iter()
            .find(|(parameter, _)| parameter == &name)
            .map(|(_, selected)| selected.clone())
            .unwrap_or(name)
    };
    let target = substitute(target.to_owned());
    if machine_arguments.is_empty() {
        return target;
    }
    let selected: Vec<String> = machine_arguments
        .iter()
        .map(|argument| {
            let name = argument
                .path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            substitute(name)
        })
        .collect();
    format!("{target}<{}>", selected.join(","))
}

fn split_structural_machine_name(name: &str) -> (&str, Vec<&str>) {
    let Some((base, selected)) = name.split_once('<') else {
        return (name, Vec::new());
    };
    let Some(selected) = selected.strip_suffix('>') else {
        return (name, Vec::new());
    };
    if selected.is_empty() {
        (base, Vec::new())
    } else {
        (base, selected.split(',').collect())
    }
}

fn structural_term(program: &TypedTrees, expression: ExpressionHandle) -> Option<StructuralTerm> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            match members {
                [single] => Some(StructuralTerm::Variable(single.as_str().to_owned())),
                [first, second] => {
                    if program
                        .data_definitions()
                        .iter()
                        .any(|definition| definition.name.as_str() == first.as_str())
                    {
                        Some(StructuralTerm::Constructor {
                            data: first.as_str().to_owned(),
                            case: second.as_str().to_owned(),
                            fields: Vec::new(),
                        })
                    } else {
                        Some(StructuralTerm::Opaque(
                            program.expression_table.display_name(expression),
                        ))
                    }
                }
                _ => Some(StructuralTerm::Opaque(
                    program.expression_table.display_name(expression),
                )),
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            // A RECORD literal (no case name) is a single-constructor data:
            // it terms as a Constructor with the EMPTY case (both termifiers
            // spell it identically, so congruence decomposes it; the
            // case-disjointness refutation never fires on "" == "").
            let case = literal
                .case_name
                .as_ref()
                .map(|case| case.as_str())
                .unwrap_or("");
            let mut fields: Vec<(String, StructuralTerm)> = Vec::new();
            for field in program.expression_table.struct_fields(literal.fields) {
                fields.push((
                    field.name.as_str().to_owned(),
                    structural_term(program, field.value)?,
                ));
            }
            fields.sort_by(|(left, _), (right, _)| left.cmp(right));
            Some(StructuralTerm::Constructor {
                data: literal.type_name.as_str().to_owned(),
                case: case.to_owned(),
                fields,
            })
        }
        ExpressionNode::Call(call) => {
            if !call.receiver.is_valid() {
                let handles = program.expression_table.expression_handles(call.arguments);
                let arguments: Vec<StructuralTerm> = handles
                    .iter()
                    .filter_map(|argument| structural_term(program, *argument))
                    .collect();
                if arguments.len() == handles.len() {
                    return Some(StructuralTerm::Application {
                        machine: structural_call_machine_name(
                            call.target.as_str(),
                            &call.machine_arguments,
                            &[],
                        ),
                        arguments,
                    });
                }
            }
            Some(StructuralTerm::Opaque(
                program.expression_table.display_name(expression),
            ))
        }
        ExpressionNode::Boolean(value) => Some(StructuralTerm::Constructor {
            data: "bool".to_owned(),
            case: value.to_string(),
            fields: Vec::new(),
        }),
        ExpressionNode::Member(_) => Some(StructuralTerm::Opaque(
            program.expression_table.display_name(expression),
        )),
        _ => None,
    }
}

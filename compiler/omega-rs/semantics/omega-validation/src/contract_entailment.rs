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
use omega_core::symbols::SymbolHandle;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::SignatureContractKind;
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

/// The reserved binder naming a machine's return value inside `ensures`
/// facts. Matches the call-site substitution rule in the checked-trees
/// contract prover: a single-segment `result` that does not shadow a real
/// parameter denotes the produced value.
const RESULT_BINDER: &str = "result";

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
    let structural = StructuralJudge::from_requires(program, &requires);
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
            match statement {
                StatementNode::LocalData(local_data) => {
                    let term =
                        structural.callee_term(local_data.initial_value, &environment, 0)?;
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
            let ExpressionNode::Binary(binary) = program.expression_table.expression(*fact)
            else {
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
        recognize_structural_case_arms(program, machine, &structural)
    };
    let judge_structural = |fact: ExpressionHandle| -> StructuralJudgment {
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
            if let Some((subject, constructor)) = &arm.case_hypothesis {
                bound
                    .substitutions
                    .insert(0, (subject.clone(), constructor.clone()));
            }
            // Inductive hypotheses: instantiate every ensures conjunct for
            // each self-application in the arm's value term.
            let mut applications = Vec::new();
            StructuralJudge::self_applications(&arm.value, &arm.machine_name, &mut applications);
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
            let Some(held) = fact_mentions_proof_only_data(program, &proof_only, machine, *fact)
            else {
                // Not structural: stays with the polynomial engine below.
                return true;
            };
            any_structural = true;
            if structural.hypotheses_contradictory {
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
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` ensures contract proof fact `{}` speaks about proof-only \
                         `{held}`, which no entailment tier judges yet -- accepting it would \
                         certify an unproven structural claim. Spell the fact over integer \
                         measures, or wait for the structural extraction tier (math roster N3)",
                        machine.name,
                        program.expression_table.display_name(*fact),
                    )));
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
        || requires
            .iter()
            .any(|fact| fact_mentions_proof_only_data(program, &proof_only, machine, *fact).is_some())
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
    if !machine.terminates {
        return false;
    }
    let decreases = program
        .expression_table
        .expression_handles(machine.decreases);
    let order = program.machine_decrease_order(machine.decrease_order);
    let polynomial_order = order.is_empty()
        || (order.len() == 2
            && order[0].as_str() == "Nat"
            && matches!(order[1].as_str(), "Descending" | "BoundedDistance"));
    if !polynomial_order {
        return false;
    }
    let measure = match decreases {
        [single] => engine.normalize(*single),
        // The two-subject bounded distance: the subjects bind in order to the
        // view's (lower, upper) parameters and the measure polynomial is the
        // distance `upper - lower`.
        [lower, upper] => engine
            .normalize(*upper)
            .zip(engine.normalize(*lower))
            .map(|(upper, lower)| upper.sub(&lower)),
        _ => None,
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
    /// `Some((subject, constructor))` for a case arm: the subject parameter
    /// equals a constructor over FRESH payload variables. `None` for an
    /// Always arm.
    case_hypothesis: Option<(String, StructuralTerm)>,
    /// The arm's value term, converted under the case environment (payload
    /// member reads resolve against the fresh-variable constructor).
    value: StructuralTerm,
}

/// Recognize a single-state proof machine whose statements are case arms
/// over its parameters (`transition a { Nat::Zero -> .. Nat::Succ { prev }
/// -> .. }` desugars to per-arm transitions guarded by `a == Nat::Case`).
/// Returns `None` for anything out of shape -- judging then proceeds
/// without result binding, which only weakens toward Unknown.
fn recognize_structural_case_arms(
    program: &TypedTrees,
    machine: &Machine,
    judge: &StructuralJudge<'_>,
) -> Option<Vec<StructuralCaseArm>> {
    let [root] = program.machine_states(machine) else {
        return None;
    };
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
    let statements = program.statement_table.statements(root.statement_nodes);
    if statements.is_empty() {
        return None;
    }
    let mut arms = Vec::new();
    for statement in statements {
        let StatementNode::Transition(transition) = statement else {
            return None;
        };
        if transition.continuation.is_valid() {
            return None;
        }
        let case_hypothesis = match transition.guard {
            TransitionGuardNode::Always => None,
            TransitionGuardNode::When(guard) => {
                let ExpressionNode::Binary(comparison) =
                    program.expression_table.expression(guard)
                else {
                    return None;
                };
                if comparison.operator != BinaryOperator::Equal {
                    return None;
                }
                let Some(StructuralTerm::Variable(subject)) =
                    structural_term(program, comparison.left)
                else {
                    return None;
                };
                if !parameter_names.iter().any(|name| name == &subject) {
                    return None;
                }
                let Some(StructuralTerm::Constructor { data, case, fields }) =
                    structural_term(program, comparison.right)
                else {
                    return None;
                };
                if !fields.is_empty() {
                    return None;
                }
                // Fresh payload variables from the case's declared fields.
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
                let mut fresh: Vec<(String, StructuralTerm)> = variant_fields
                    .into_iter()
                    .map(|field| {
                        let variable = format!("__ih_{subject}_{field}");
                        (field, StructuralTerm::Variable(variable))
                    })
                    .collect();
                fresh.sort_by(|(left, _), (right, _)| left.cmp(right));
                Some((
                    subject,
                    StructuralTerm::Constructor {
                        data,
                        case,
                        fields: fresh,
                    },
                ))
            }
        };
        let TransitionTargetNode::Value(value) =
            program.statement_table.transition_target(transition.target)
        else {
            return None;
        };
        // The arm environment: the subject maps to its constructor (so
        // payload member reads index the fresh variables); every other
        // parameter maps to itself.
        let environment: Vec<(String, StructuralTerm)> = parameter_names
            .iter()
            .map(|name| {
                if let Some((subject, constructor)) = &case_hypothesis
                    && subject == name
                {
                    (name.clone(), constructor.clone())
                } else {
                    (name.clone(), StructuralTerm::Variable(name.clone()))
                }
            })
            .collect();
        let value = judge.callee_term(*value, &environment, 0)?;
        arms.push(StructuralCaseArm {
            machine_name: machine_name.clone(),
            parameter_names: parameter_names.clone(),
            case_hypothesis,
            value,
        });
    }
    Some(arms)
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
    /// A FREE call whose arguments all term-ify (`add(Nat::Zero, b)`).
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

struct StructuralJudge<'program> {
    program: &'program TypedTrees,
    substitutions: Vec<(String, StructuralTerm)>,
    /// Application REWRITES (`add_zero_right(prev) -> prev`): hypothesis
    /// equations with an application side orient REDUCING -- the inductive
    /// hypothesis rewrites the self-application away instead of expanding a
    /// variable into it, which also serves asymmetric goals.
    rewrites: Vec<(StructuralTerm, StructuralTerm)>,
    hypotheses_contradictory: bool,
}

impl Clone for StructuralJudge<'_> {
    fn clone(&self) -> Self {
        Self {
            program: self.program,
            substitutions: self.substitutions.clone(),
            rewrites: self.rewrites.clone(),
            hypotheses_contradictory: self.hypotheses_contradictory,
        }
    }
}

impl<'program> StructuralJudge<'program> {
    fn from_requires(program: &'program TypedTrees, requires: &[ExpressionHandle]) -> Self {
        let mut judge = Self {
            program,
            substitutions: Vec::new(),
            rewrites: Vec::new(),
            hypotheses_contradictory: false,
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
                    if let Some((_, value_r)) =
                        fields_r.iter().find(|(name_r, _)| name_r == name_l)
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
                    if let Some(unfolded) =
                        self.unfold_application(machine, arguments, depth + 1)
                    {
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
        let machine = program
            .machines()
            .iter()
            .find(|machine| {
                machine.attached_data.is_none() && machine.name.as_str() == machine_name
            })?;
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

        for statement in program.statement_table.statements(state.statement_nodes) {
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
                    let (_, subject_term) = environment
                        .iter()
                        .find(|(name, _)| name == &subject_name)?;
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
            return self.callee_term(*value, &environment, depth + 1);
        }
        None
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
                let ExpressionNode::Name(path) =
                    program.expression_table.expression(member.receiver)
                else {
                    return None;
                };
                let [single] = program.expression_table.name_path_members(path.members) else {
                    return None;
                };
                let (_, receiver_term) = environment
                    .iter()
                    .find(|(name, _)| name == single.as_str())?;
                let StructuralTerm::Constructor { fields, .. } =
                    self.resolve_at(receiver_term.clone(), depth + 1)
                else {
                    return None;
                };
                fields
                    .iter()
                    .find(|(name, _)| name == member.member.as_str())
                    .map(|(_, term)| term.clone())
            }
            ExpressionNode::StructLiteral(literal) => {
                let case = literal.case_name.as_ref()?;
                let mut fields: Vec<(String, StructuralTerm)> = Vec::new();
                for field in program.expression_table.struct_fields(literal.fields) {
                    fields.push((
                        field.name.as_str().to_owned(),
                        self.callee_term(field.value, environment, depth + 1)?,
                    ));
                }
                fields.sort_by(|(left, _), (right, _)| left.cmp(right));
                Some(StructuralTerm::Constructor {
                    data: literal.type_name.as_str().to_owned(),
                    case: case.as_str().to_owned(),
                    fields,
                })
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    return None;
                }
                let mut arguments = Vec::new();
                for argument in program.expression_table.expression_handles(call.arguments) {
                    arguments.push(self.callee_term(*argument, environment, depth + 1)?);
                }
                Some(StructuralTerm::Application {
                    machine: call.target.as_str().to_owned(),
                    arguments,
                })
            }
            _ => None,
        }
    }

    /// Substitute variables in a term (used to instantiate the machine's
    /// own ensures as the INDUCTIVE HYPOTHESIS at a self-call: params -> the
    /// call's argument terms, `result` -> the application term).
    fn substitute_term(
        term: &StructuralTerm,
        map: &[(String, StructuralTerm)],
    ) -> StructuralTerm {
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
            StructuralTerm::Opaque(_) => term.clone(),
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
            return StructuralJudgment::Unknown;
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
                let equality =
                    self.judge_equation(self.resolve(left), self.resolve(right), 0);
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
    /// proven, any refutes => refuted), distinct cases refute.
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
        StructuralTerm::Constructor { fields, .. } => fields
            .iter()
            .any(|(_, value)| term_contains(value, needle)),
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
            let case = literal.case_name.as_ref()?;
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
                case: case.as_str().to_owned(),
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
                        machine: call.target.as_str().to_owned(),
                        arguments,
                    });
                }
            }
            Some(StructuralTerm::Opaque(
                program.expression_table.display_name(expression),
            ))
        }
        ExpressionNode::Member(_) => Some(StructuralTerm::Opaque(
            program.expression_table.display_name(expression),
        )),
        _ => None,
    }
}


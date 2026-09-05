use super::*;

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
pub(super) fn inductive_transition_entailment(
    program: &TypedTrees,
    machine: &Machine,
    requires: &[ExpressionHandle],
    ensures: &[ExpressionHandle],
    all_facts_are_expressions: bool,
    diagnostics: &mut Vec<Diagnostic>,
    account_stand_downs: bool,
    ensures_coordinates: &[(ExpressionHandle, usize, usize)],
    stand_downs: &mut Vec<crate::ContractEntailmentStandDown>,
    proven: &mut Vec<ExpressionHandle>,
) {
    // Single-state machines only: the state graph IS the recursion structure,
    // and a tail self-call is a transition back to the root state.
    let states = program.machine_states(machine);
    let [root] = states else {
        record_inductive_stand_downs(
            account_stand_downs,
            machine,
            ensures,
            crate::ContractEntailmentStandDownReason::UnrecognizedInductiveBody,
            ensures_coordinates,
            stand_downs,
        );
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
            record_inductive_stand_downs(
                account_stand_downs,
                machine,
                ensures,
                crate::ContractEntailmentStandDownReason::UnrecognizedInductiveBody,
                ensures_coordinates,
                stand_downs,
            );
            return; // assignments / locals / calls: out of shape, stand down
        };
        if transition.continuation.is_valid() {
            record_inductive_stand_downs(
                account_stand_downs,
                machine,
                ensures,
                crate::ContractEntailmentStandDownReason::UnrecognizedInductiveBody,
                ensures_coordinates,
                stand_downs,
            );
            return;
        }
        let guard = match transition.guard {
            TransitionGuardNode::When(guard) => Some(guard),
            TransitionGuardNode::Always => None,
        };
        let target = program.statement_table.transition_target(transition.target);
        let kind = match target {
            TransitionTargetNode::Value(value) => ArmKind::Value(*value),
            TransitionTargetNode::Named {
                path, arguments, ..
            } if path.symbol == root.symbol => ArmKind::TailSelfCall(
                program
                    .statement_table
                    .expression_handles(*arguments)
                    .to_vec(),
            ),
            // Transitions to other states (or `self` / terminal targets) are
            // outside the recognized inductive shape.
            _ => {
                record_inductive_stand_downs(
                    account_stand_downs,
                    machine,
                    ensures,
                    crate::ContractEntailmentStandDownReason::UnrecognizedInductiveBody,
                    ensures_coordinates,
                    stand_downs,
                );
                return;
            }
        };
        arms.push(TransitionArm { guard, kind });
    }

    if arms.is_empty() {
        record_inductive_stand_downs(
            account_stand_downs,
            machine,
            ensures,
            crate::ContractEntailmentStandDownReason::UnrecognizedInductiveBody,
            ensures_coordinates,
            stand_downs,
        );
        return;
    }

    let trace = std::env::var("OMEGA_ENTAILMENT_TRACE").is_ok();
    let mut judged_arms = Vec::new();
    let mut every_arm_visible = true;
    for arm in &arms {
        let Some(judged) = prepare_arm(program, machine, root, requires, ensures, arm) else {
            // The arm's value or argument list is unreadable: the whole body
            // cannot be anchored, so nothing can be judged or rejected.
            record_inductive_stand_downs(
                account_stand_downs,
                machine,
                ensures,
                crate::ContractEntailmentStandDownReason::OutsideEntailmentLanguage,
                ensures_coordinates,
                stand_downs,
            );
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
        } else if let Some(ref guard) = unknown_arm
            && goal_always_in_language
            && machine_fully_visible
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` cannot prove ensures contract proof fact `{}` on the transition arm guarded by `{}`",
                machine.name,
                program.expression_table.display_name(*fact),
                guard
            )));
        } else if unknown_arm.is_some() && account_stand_downs {
            record_expression_stand_down(
                machine,
                *fact,
                crate::ContractEntailmentStandDownReason::OutsideEntailmentLanguage,
                ensures_coordinates,
                stand_downs,
            );
        } else if unknown_arm.is_none() {
            proven.push(*fact);
        }
        // An unknown that is not fully visible means some fact lies outside
        // the engine's language: stand down rather than reject what we
        // cannot fully read.
    }
}

fn record_inductive_stand_downs(
    account: bool,
    machine: &Machine,
    ensures: &[ExpressionHandle],
    reason: crate::ContractEntailmentStandDownReason,
    coordinates: &[(ExpressionHandle, usize, usize)],
    stand_downs: &mut Vec<crate::ContractEntailmentStandDown>,
) {
    if account {
        record_all_expression_stand_downs(machine, ensures, reason, coordinates, stand_downs);
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
    root: &typed_trees::state::State,
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
    root: &typed_trees::state::State,
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
    // The hypothesis gate keys exclusively on the normalized private witness.
    let Some(witness) = machine.termination_plan.implementation_witness.as_ref() else {
        return false;
    };
    let Some(decreases) = typed_trees::ranking::resolve_machine_witness_subjects(program, machine)
    else {
        return false;
    };
    let order = witness
        .view_path
        .split("::")
        .filter(|member| !member.is_empty())
        .collect::<Vec<_>>();
    // TPR3: the argumented `Nat::IncreasingTo(limit)` is polynomial too --
    // its measure is the distance `limit - subject` with the bound taken
    // from the view's argument.
    let increasing_to = order.as_slice() == ["Nat", "IncreasingTo"];
    let polynomial_order = increasing_to
        || order.is_empty()
        || (order.len() == 2
            && order[0] == "Nat"
            && matches!(order[1], "Descending" | "BoundedDistance"));
    if !polynomial_order {
        return false;
    }
    let measure = if increasing_to {
        let Some(arguments) =
            typed_trees::ranking::resolve_machine_witness_view_arguments(program, machine)
        else {
            return false;
        };
        match (decreases.as_slice(), arguments.as_slice()) {
            ([subject], [limit]) => engine
                .normalize(*limit)
                .zip(engine.normalize(*subject))
                .map(|(limit, subject)| limit.sub(&subject)),
            _ => None,
        }
    } else {
        match decreases.as_slice() {
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

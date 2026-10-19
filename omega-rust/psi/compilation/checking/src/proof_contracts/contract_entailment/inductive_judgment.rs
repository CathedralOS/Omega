use super::{
    BTreeMap, BigInt, BinaryOperator, Diagnostic, Engine, ExpressionHandle, ExpressionNode,
    Judgment, Machine, Polynomial, RESULT_BINDER, TransitionGuardNode, TransitionTargetNode,
    is_arm_pattern_marker,
};

/// One recognized transition arm of an inductive machine body.
struct TransitionArm {
    /// The arm's own guard. Earlier arms must also have failed, including
    /// when this arm has no additional test.
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
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    requires: &[ExpressionHandle],
    ensures: &[ExpressionHandle],
    all_facts_are_expressions: bool,
    diagnostics: &mut Vec<Diagnostic>,
    proven: &mut Vec<ExpressionHandle>,
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
        let Some(transition) = statement.as_transition() else {
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
            TransitionTargetNode::Named {
                path, arguments, ..
            } if path.symbol == root.symbol || path.symbol == machine.symbol => {
                // A tail call naming the machine itself re-enters its entry
                // state: resolution binds the machine symbol as the canonical
                // coordinate for a free machine's self-recursion, and the
                // single-state precondition above makes that coordinate
                // unambiguous with the root state.
                ArmKind::TailSelfCall(
                    program
                        .statement_table
                        .expression_handles(*arguments)
                        .to_vec(),
                )
            }
            // Transitions to other states (or `self` / terminal targets) are
            // outside the recognized inductive shape.
            _ => {
                return;
            }
        };
        arms.push(TransitionArm { guard, kind });
    }

    if arms.is_empty() {
        return;
    }

    // This judgment uses one parameter namespace for the entire dispatch.
    // A writing or opaque guard needs versioned facts; unreadable comparisons
    // alone are not enough to suppress a stale proof or vacuity conclusion.
    let call_frames = crate::CallFrameResolver::new(program);
    if arms.iter().filter_map(|arm| arm.guard).any(|guard| {
        !call_frames.as_ref().is_some_and(|frames| {
            frames
                .expression_write_frame(machine, guard)
                .into_complete_paths()
                .is_some_and(|paths| paths.is_empty())
        })
    }) {
        return;
    }

    let trace = std::env::var("OMEGA_ENTAILMENT_TRACE").is_ok();
    let mut judged_arms = Vec::new();
    let mut every_arm_visible = true;
    for (index, arm) in arms.iter().enumerate() {
        let Some(judged) = prepare_arm(
            program,
            machine,
            root,
            requires,
            ensures,
            arm,
            &arms[..index],
        ) else {
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
        } else if unknown_arm.is_none() {
            proven.push(*fact);
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
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    root: &typed_trees::state::State,
    requires: &[ExpressionHandle],
    ensures: &[ExpressionHandle],
    arm: &TransitionArm,
    preceding: &[TransitionArm],
) -> Option<JudgedArm<'program>> {
    let guard_display = match arm.guard {
        Some(guard) => program.expression_table.display_name(guard),
        None if preceding.is_empty() => "<always>".to_owned(),
        None => preceding
            .iter()
            .filter_map(|arm| arm.guard)
            .map(|guard| failed_guard_display(program, guard))
            .collect::<Vec<_>>()
            .join(" && "),
    };

    let mut engine = Engine::with_result_atom(program, machine, root);
    if preceding.iter().any(|arm| arm.guard.is_none()) {
        return Some(JudgedArm {
            engine,
            vacuous: true,
            fully_visible: true,
            guard_display,
        });
    }
    let mut comparisons = Vec::new();
    let mut fully_visible = engine.add_requires(requires);

    // Dispatch is ordered: preceding guards failed before this arm's own
    // guard succeeded. In particular, an Always fallback is not unconstrained.
    let guards = preceding
        .iter()
        .filter_map(|arm| arm.guard.map(|guard| (guard, false)))
        .chain(arm.guard.map(|guard| (guard, true)))
        .collect::<Vec<_>>();
    for &(guard, polarity) in &guards {
        fully_visible &=
            super::ranking_range::collect_guard(&mut engine, guard, polarity, &mut comparisons, 0)
                .unwrap_or(false);
        fully_visible &= engine.install_hypotheses(std::mem::take(&mut comparisons));
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
            // site. The installed context contains requires + guard only;
            // normalizing under it cannot borrow the recursive conclusion.
            let actuals = self_call_arguments(&mut engine, root, arguments)?;
            if engine.requires_unsatisfiable {
                // Unreachable arm: vacuous regardless of the hypothesis.
            } else if discharges_strict_decrease(program, machine, &mut engine, &actuals) {
                // Instantiate the machine's own ensures over the call's
                // arguments; `result` stays shared (the arm's result IS the
                // call's result). Conjuncts the engine cannot instantiate are
                // dropped -- a weaker hypothesis is sound but blocks
                // rejection.
                let (hypotheses, visible) =
                    engine.with_formal_arguments(machine.symbol, &actuals, |engine| {
                        let mut hypotheses = Vec::new();
                        let mut visible = true;
                        for fact in ensures {
                            for conjunct in engine.conjuncts(*fact) {
                                match engine.comparison_polynomials(conjunct) {
                                    Some(comparison) => hypotheses.push(comparison),
                                    None => visible = false,
                                }
                            }
                        }
                        Some((hypotheses, visible))
                    })?;
                comparisons.extend(hypotheses);
                fully_visible &= visible;
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
pub(super) fn guard_arm_comparison(
    engine: &mut Engine<'_>,
    guard: ExpressionHandle,
    expected: bool,
) -> Option<(BinaryOperator, Polynomial, Polynomial)> {
    let node = engine.program.expression_table.expression(guard).clone();
    if let ExpressionNode::Binary(binary) = &node
        && binary.operator == BinaryOperator::Equal
        && let ExpressionNode::Boolean(polarity) =
            engine.program.expression_table.expression(binary.right)
    {
        let polarity = *polarity == expected;
        let (operator, left, right) = engine.comparison_polynomials(binary.left)?;
        let operator = if polarity {
            operator
        } else {
            operator.complement()?
        };
        return Some((operator, left, right));
    }
    let (operator, left, right) = engine.comparison_polynomials(guard)?;
    Some((
        if expected {
            operator
        } else {
            operator.complement()?
        },
        left,
        right,
    ))
}

fn failed_guard_display(program: &dyn crate::CheckingInput, guard: ExpressionHandle) -> String {
    if let Some(binary) = program.expression_table.binary_node(guard)
        && binary.operator == BinaryOperator::Equal
        && let ExpressionNode::Boolean(polarity) = program.expression_table.expression(binary.right)
    {
        return format!(
            "{} == {}",
            program.expression_table.display_name(binary.left),
            !polarity
        );
    }
    format!("!({})", program.expression_table.display_name(guard))
}

/// Complete the recursive tuple in the caller's namespace before publishing
/// any formal. Resolved symbols carry the association into both consumers.
fn self_call_arguments(
    engine: &mut Engine<'_>,
    root: &typed_trees::state::State,
    arguments: &[ExpressionHandle],
) -> Option<Vec<(symbols::SymbolHandle, Polynomial)>> {
    let program = engine.program;
    let parameters = program.state_parameters(root);
    if parameters.len() != arguments.len() {
        return None;
    }
    let mut actuals = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let polynomial = engine.normalize(*argument)?;
        actuals.push((parameter.symbol, polynomial));
    }
    Some(actuals)
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
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    engine: &mut Engine<'_>,
    actuals: &[(symbols::SymbolHandle, Polynomial)],
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
    let limits = if increasing_to {
        match typed_trees::ranking::resolve_machine_witness_view_arguments(program, machine) {
            Some(arguments) => Some(arguments),
            None => return false,
        }
    } else {
        None
    };
    let read_measure = |engine: &mut Engine<'_>| {
        match (decreases.as_slice(), limits.as_deref()) {
            ([subject], Some([limit])) => engine
                .normalize(*limit)
                .zip(engine.normalize(*subject))
                .map(|(limit, subject)| limit.sub(&subject)),
            ([single], None) => engine.normalize(*single),
            // The two-subject bounded distance: the subjects bind in order to the
            // view's (lower, upper) parameters and the measure polynomial is the
            // distance `upper - lower`.
            ([lower, upper], None) => engine
                .normalize(*upper)
                .zip(engine.normalize(*lower))
                .map(|(upper, lower)| upper.sub(&lower)),
            _ => None,
        }
    };
    let Some(measure) = read_measure(engine) else {
        return false;
    };
    let Some(measure_after) = engine.with_formal_arguments(machine.symbol, actuals, read_measure)
    else {
        return false;
    };
    let measure_now = engine.substituted(&measure);
    let measure_after = engine.substituted(&measure_after);
    let difference = measure_now.sub(&measure_after);
    engine.prove_at_least(&difference, &BigInt::from_i64(1))
        && engine.prove_at_least(&measure_after, &BigInt::zero())
}

/// Simultaneous single-pass substitution of parameter atoms by argument
/// polynomials. Single-pass is essential: arguments mention the same
/// parameters (`n -> n - 1`), so a fixpoint application would telescope.
/// Atoms that are neither mapped parameters nor `result` (mod-term and
/// proof-view atoms embed parameter names in their rendered form) cannot be
/// instantiated and fail the substitution.
pub(super) fn apply_argument_map(
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

//! Path-conditioned crash-route coverage.
//!
//! A state reached only through a retained incoming guard has that guard in
//! every execution path reaching its body. The same is true of the negated
//! guard on a fallthrough/continuation edge. When that exact normalized
//! predicate is one alternative in a same-cause published bucket, the site's
//! derived condition implies the bucket guard. This is checked implementation
//! evidence and never enters the public contract fingerprint.

use checked_trees::{CheckFacts, CrashRouteGuard};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

mod entry_guards;
mod entry_requirements;
mod source_fallthrough;

/// A crash transition is recorded unconditionally at its site: contract exits,
/// returned values, and state joins attach only to `Ordinary` exits, and the
/// flow readers skip every edge on a non-ordinary transition. A `Crash` exit
/// that still carries an ordinary successor target, a continuation, or a
/// conditional `when` guard therefore holds dead structure whose obligations
/// (an `ensures` on the returned value, a fallthrough join, a guarded arm)
/// are silently absorbed rather than discharged. Parsed `crash` statements
/// always arrive with a `Terminal` target, no continuation, and an `Always`
/// guard; anything else is a malformed tree and must reject here instead of
/// being skipped by every ordinary-edge reader downstream. Outcome proof
/// selectors need no clause here: a nonempty selector list on a non-`When`
/// arm already rejects in `bind_outcome_specific_arm_facts`, and a `When`
/// guard on a crash exit rejects above regardless.
pub(crate) fn check_crash_exit_edge_isolation(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    use typed_trees::statement::{
        StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
    };

    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_ordinal, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let StatementNode::Transition(transition) = statement else {
                    continue;
                };
                let TransitionExit::Crash(cause) = transition.exit else {
                    continue;
                };
                let mut reject = |edge: &str| {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{state_name}` statement {statement_ordinal} is an unconditional {cause:?} crash exit; it cannot keep {edge}",
                        machine.name.as_str(),
                        state_name = state.name.as_str(),
                    )));
                };
                if transition.target.is_valid()
                    && !matches!(
                        program.statement_table.transition_target(transition.target),
                        TransitionTargetNode::Terminal
                    )
                {
                    reject("an ordinary successor edge");
                }
                if transition.continuation.is_valid() {
                    reject("a continuation edge");
                }
                if matches!(transition.guard, TransitionGuardNode::When(_)) {
                    reject("a conditional `when` guard");
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Reconstruct the exact selected occurrence roster before relying on source
/// crash summaries. Missing discharge rows must not look like crash-free uses;
/// substitutions, operands, and selected requirement buckets remain checkable.
/// Portable products still need their own invocation/certificate representation.
pub(crate) fn check_operator_invocation_custody(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let expected = crate::facts::operator_crashes::build(
        program,
        &facts.operators,
        &facts.flow,
        &facts.semantic,
    )?;
    let actual = facts
        .contract_plans
        .machines
        .iter()
        .flat_map(|machine| {
            machine
                .crash
                .checked_operators()
                .iter()
                .map(move |site| (machine.machine, site))
        })
        .collect::<Vec<_>>();
    if expected.len() != actual.len()
        || expected.iter().any(|(machine, site)| {
            actual
                .iter()
                .filter(|(owner, candidate)| owner == machine && *candidate == site)
                .count()
                != 1
        })
    {
        return Err(vec![Diagnostic::error(
            "selected operator crash invocation evidence does not match its captured source occurrence",
        )]);
    }
    Ok(())
}

pub(crate) fn infer_path_conditioned_guard_coverage(
    program: &TypedTrees,
    facts: &mut CheckFacts,
    incoming_guards: &super::ranges::incoming_guards::IncomingGuardIndex,
) {
    let content_conservation = validation::build_content_conservation_plans(program);
    let mut integer_types = None;
    for machine in program.machines() {
        let incoming = incoming_guards.for_machine(machine.symbol);
        let parameter_names = program
            .machine_states(machine)
            .first()
            .map(|entry| {
                program
                    .state_parameters(entry)
                    .iter()
                    .map(|parameter| parameter.name.as_str().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let Some(contract_index) = facts
            .contract_plans
            .machines
            .iter()
            .position(|contract| contract.machine == machine.symbol)
        else {
            continue;
        };
        let crash_plan = &facts.contract_plans.machines[contract_index].crash;
        if crash_plan.checked_sites().is_empty() && crash_plan.checked_calls().is_empty() {
            continue;
        }
        // The declaration classification is a program roster and the mutable
        // primitive parameters are machine facts; the guard-consequence
        // derivations below reread both per checked site and per checked
        // call. Build each once, on first need, because neither answer can
        // change during this pass.
        let integer_types =
            integer_types.get_or_insert_with(|| IntegerTypeClassification::build(program));
        // Each retained `when` guard is re-resolved against the state and
        // statement where it was evaluated, not the site where its edge
        // arrives. That is the point where current storage can still be
        // related to the invocation-entry operand a published route names.
        let eval_sites = entry_guards::guard_eval_sites(program, machine);
        let source_fallthrough =
            source_fallthrough::collect(program, machine, &parameter_names, &content_conservation);
        let entry_requirements = entry_requirements::collect(
            program,
            machine,
            &facts.operators,
            &parameter_names,
            &content_conservation,
            integer_types,
        );

        let checked_sites = crash_plan
            .checked_sites()
            .iter()
            .map(|site| {
                let mut covering = site.guard_covering_buckets().to_vec();
                let applicable_guards = incoming
                    .iter()
                    .filter(|guard| guard.applies_at(site.location().state()))
                    .flat_map(|guard| {
                        entry_guards::entry_meaning_conjuncts(
                            program,
                            machine,
                            &eval_sites,
                            guard.guard(),
                            guard.is_negated(),
                            &parameter_names,
                            &content_conservation,
                        )
                    })
                    .collect::<Vec<_>>();
                let mut path_guard_conjuncts = applicable_guards
                    .iter()
                    .map(|&(guard, negated)| {
                        crate::facts::canonical_crash_path_predicate(
                            program,
                            guard,
                            negated,
                            &parameter_names,
                            &content_conservation,
                        )
                    })
                    .collect::<Vec<_>>();
                path_guard_conjuncts.extend(entry_requirements.conjuncts.iter().cloned());
                let mut path_predicates = entry_requirements.consequences.clone();
                let mut order_relations = Vec::new();
                let mut integer_disequalities = Vec::new();
                for (guard, negated) in applicable_guards {
                    collect_structural_guard_consequences(
                        program,
                        guard,
                        negated,
                        &parameter_names,
                        &content_conservation,
                        integer_types,
                        &mut path_predicates,
                    );
                    collect_integer_order_relations(
                        program,
                        guard,
                        negated,
                        &parameter_names,
                        &content_conservation,
                        integer_types,
                        &mut order_relations,
                        &mut integer_disequalities,
                    );
                }
                if let Some(fallthrough) = source_fallthrough
                    .iter()
                    .find(|fallthrough| fallthrough.location == site.location())
                {
                    for &(guard, negated) in &fallthrough.guards {
                        path_guard_conjuncts.push(crate::facts::canonical_crash_path_predicate(
                            program,
                            guard,
                            negated,
                            &parameter_names,
                            &content_conservation,
                        ));
                        collect_structural_guard_consequences(
                            program,
                            guard,
                            negated,
                            &parameter_names,
                            &content_conservation,
                            integer_types,
                            &mut path_predicates,
                        );
                        collect_integer_order_relations(
                            program,
                            guard,
                            negated,
                            &parameter_names,
                            &content_conservation,
                            integer_types,
                            &mut order_relations,
                            &mut integer_disequalities,
                        );
                    }
                }
                push_transitive_integer_order_consequences(
                    program,
                    &mut order_relations,
                    &integer_disequalities,
                    &parameter_names,
                    &content_conservation,
                    &mut path_predicates,
                );
                path_predicates.sort();
                path_predicates.dedup();

                for (bucket_id, bucket) in crash_plan.published_with_ids() {
                    if bucket.cause() != site.cause()
                        || covering.contains(&bucket_id)
                        || !bucket.alternative_guards().iter().any(|route| {
                            matches!(route, CrashRouteGuard::Predicate(predicate)
                                if path_predicates.contains(predicate))
                        })
                    {
                        continue;
                    }
                    covering.push(bucket_id);
                }

                site.clone()
                    .with_path_guard_conjuncts(path_guard_conjuncts)
                    .with_path_guard_consequences(path_predicates)
                    .with_guard_covering_buckets(covering)
            })
            .collect();
        let checked_calls = crash_plan
            .checked_calls()
            .iter()
            .map(|call| {
                let applicable_guards = incoming
                    .iter()
                    .filter(|guard| guard.applies_at(call.location().state()))
                    .flat_map(|guard| {
                        entry_guards::entry_meaning_conjuncts(
                            program,
                            machine,
                            &eval_sites,
                            guard.guard(),
                            guard.is_negated(),
                            &parameter_names,
                            &content_conservation,
                        )
                    })
                    .collect::<Vec<_>>();
                let mut path_guard_conjuncts = applicable_guards
                    .iter()
                    .map(|&(guard, negated)| {
                        crate::facts::canonical_crash_path_predicate(
                            program,
                            guard,
                            negated,
                            &parameter_names,
                            &content_conservation,
                        )
                    })
                    .collect::<Vec<_>>();
                path_guard_conjuncts.extend(entry_requirements.conjuncts.iter().cloned());
                let mut path_guard_consequences = entry_requirements.consequences.clone();
                let mut order_relations = Vec::new();
                let mut integer_disequalities = Vec::new();
                for (guard, negated) in applicable_guards {
                    collect_structural_guard_consequences(
                        program,
                        guard,
                        negated,
                        &parameter_names,
                        &content_conservation,
                        integer_types,
                        &mut path_guard_consequences,
                    );
                    collect_integer_order_relations(
                        program,
                        guard,
                        negated,
                        &parameter_names,
                        &content_conservation,
                        integer_types,
                        &mut order_relations,
                        &mut integer_disequalities,
                    );
                }
                push_transitive_integer_order_consequences(
                    program,
                    &mut order_relations,
                    &integer_disequalities,
                    &parameter_names,
                    &content_conservation,
                    &mut path_guard_consequences,
                );
                call.clone()
                    .with_path_guard_conjuncts(path_guard_conjuncts)
                    .with_path_guard_consequences(path_guard_consequences)
            })
            .collect();
        facts.contract_plans.machines[contract_index].crash = crash_plan
            .clone()
            .with_checked_sites(checked_sites)
            .expect("path-conditioned coverage retains valid checked-site identity")
            .with_checked_calls(checked_calls)
            .expect("path-conditioned coverage retains valid checked-call identity");
    }
}

pub(crate) fn check_published_ceiling_coverage(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for caller in
        facts.contract_plans.machines.iter().filter(|plan| {
            plan.crash.interface() == checked_trees::CrashInterface::PublishedCeiling
        })
    {
        let caller_machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == caller.machine);
        let caller_name = caller_machine
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>");
        for site in caller
            .crash
            .checked_sites()
            .iter()
            .filter(|site| site.guard_covering_buckets().is_empty())
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{caller_name}` has an uncovered {:?} crash at statement {}; publish a same-cause `crashes` route whose guard covers this site",
                site.cause(),
                site.location().statement_ordinal(),
            )));
        }
        for call in caller.crash.checked_calls() {
            for surviving in call.surviving_buckets() {
                let covered = surviving.alternative_guards().iter().all(|route| {
                    caller.crash.published().iter().any(|published| {
                        published.cause() == surviving.cause()
                            && published.alternative_guards().iter().any(|cover| {
                                call_route_guard_covers(
                                    cover,
                                    route,
                                    call.path_guard_consequences(),
                                )
                            })
                    })
                });
                if covered {
                    continue;
                }
                let target_name = program
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == call.target_machine())
                    .map(|machine| machine.name.as_str())
                    .unwrap_or("<unknown>");
                diagnostics.push(Diagnostic::error(format!(
                    "call from `{caller_name}` to `{target_name}` at statement {} call {} has an uncovered {:?} crash route; publish a same-cause route whose guard covers this invocation",
                    call.location().statement_ordinal(),
                    call.location().call_ordinal(),
                    surviving.cause(),
                )));
            }
        }
        for invocation in caller.crash.checked_operators() {
            for surviving in &invocation.surviving {
                let covered = surviving.alternative_guards().iter().all(|route| {
                    caller.crash.published().iter().any(|published| {
                        published.cause() == surviving.cause()
                            && published.alternative_guards().iter().any(|cover| {
                                // These routes already use proven entry values.
                                // Statement-entry/path facts are not an implicit
                                // bridge from mutated operand storage to entry.
                                call_route_guard_covers(cover, route, &[])
                            })
                    })
                });
                if !covered {
                    diagnostics.push(Diagnostic::error(format!(
                        "selected operator `{}` in `{caller_name}` has an uncovered {:?} crash route; publish a same-cause route whose guard covers this invocation",
                        crate::labels::symbol_name(program, invocation.selected_operator),
                        surviving.cause(),
                    )));
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn call_route_guard_covers(
    published: &CrashRouteGuard,
    surviving: &CrashRouteGuard,
    path_guard_consequences: &[checked_trees::CrashPredicateIdentity],
) -> bool {
    match published {
        CrashRouteGuard::Truth => true,
        CrashRouteGuard::Predicate(published) => {
            matches!(surviving, CrashRouteGuard::Predicate(surviving) if surviving == published)
                || path_guard_consequences.contains(published)
        }
    }
}

/// Retain only propositional consequences that follow structurally from one
/// incoming predicate. A positive conjunction entails each conjunct; a
/// negated disjunction entails each negated disjunct; logical negation flips
/// polarity; equality/inequality normalize under negation; and comparisons
/// retain operand-reversed equivalents. Everything else remains an opaque
/// canonical atom. This is deliberately incomplete but sound, and does not
/// rewrite public routes.
fn collect_structural_guard_consequences(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    negated: bool,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    integer_types: &IntegerTypeClassification,
    output: &mut Vec<checked_trees::CrashPredicateIdentity>,
) {
    use typed_trees::expression::{BinaryOperator, ExpressionNode, UnaryOperator};

    output.push(crate::facts::canonical_crash_path_predicate(
        program,
        expression,
        negated,
        parameter_names,
        content_conservation,
    ));
    match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            collect_structural_guard_consequences(
                program,
                unary.operand,
                !negated,
                parameter_names,
                content_conservation,
                integer_types,
                output,
            );
        }
        ExpressionNode::Binary(binary)
            if (!negated && binary.operator == BinaryOperator::And)
                || (negated && binary.operator == BinaryOperator::Or) =>
        {
            collect_structural_guard_consequences(
                program,
                binary.left,
                negated,
                parameter_names,
                content_conservation,
                integer_types,
                output,
            );
            collect_structural_guard_consequences(
                program,
                binary.right,
                negated,
                parameter_names,
                content_conservation,
                integer_types,
                output,
            );
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            let operand_and_literal = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(literal), _) => Some((binary.right, *literal)),
                (_, ExpressionNode::Boolean(literal)) => Some((binary.left, *literal)),
                _ => None,
            };
            if let Some((operand, literal)) = operand_and_literal {
                // Normalize `x == true`, `x == false`, `x != true`, and
                // `x != false`, including a negated/fallthrough relation, to
                // the exact polarity of x that the edge establishes.
                let equality_is_negated = if binary.operator == BinaryOperator::Equal {
                    negated
                } else {
                    !negated
                };
                collect_structural_guard_consequences(
                    program,
                    operand,
                    equality_is_negated == literal,
                    parameter_names,
                    content_conservation,
                    integer_types,
                    output,
                );
            }
            let operands_are_integers =
                comparison_operands_are_integers(program, binary.left, binary.right, integer_types);
            let normalized = normalized_comparison(binary.operator, negated, operands_are_integers)
                .expect("equality operators are comparisons");
            push_comparison_consequences(
                program,
                normalized,
                binary.left,
                binary.right,
                operands_are_integers,
                parameter_names,
                content_conservation,
                output,
            );
        }
        ExpressionNode::Binary(binary) => {
            let operands_are_integers =
                comparison_operands_are_integers(program, binary.left, binary.right, integer_types);
            if let Some(normalized) =
                normalized_comparison(binary.operator, negated, operands_are_integers)
            {
                push_comparison_consequences(
                    program,
                    normalized,
                    binary.left,
                    binary.right,
                    operands_are_integers,
                    parameter_names,
                    content_conservation,
                    output,
                );
            }
        }
        _ => {}
    }
}

#[derive(Clone, PartialEq, Eq)]
struct IntegerOrderRelation {
    left_identity: checked_trees::CrashPredicateIdentity,
    right_identity: checked_trees::CrashPredicateIdentity,
    left: typed_trees::expression::ExpressionHandle,
    right: typed_trees::expression::ExpressionHandle,
    strict: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct IntegerDisequality {
    left_identity: checked_trees::CrashPredicateIdentity,
    right_identity: checked_trees::CrashPredicateIdentity,
}

fn collect_integer_order_relations(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    negated: bool,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    integer_types: &IntegerTypeClassification,
    output: &mut Vec<IntegerOrderRelation>,
    disequalities: &mut Vec<IntegerDisequality>,
) {
    use typed_trees::expression::{BinaryOperator, ExpressionNode, UnaryOperator};

    match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            collect_integer_order_relations(
                program,
                unary.operand,
                !negated,
                parameter_names,
                content_conservation,
                integer_types,
                output,
                disequalities,
            );
        }
        ExpressionNode::Binary(binary)
            if (!negated && binary.operator == BinaryOperator::And)
                || (negated && binary.operator == BinaryOperator::Or) =>
        {
            collect_integer_order_relations(
                program,
                binary.left,
                negated,
                parameter_names,
                content_conservation,
                integer_types,
                output,
                disequalities,
            );
            collect_integer_order_relations(
                program,
                binary.right,
                negated,
                parameter_names,
                content_conservation,
                integer_types,
                output,
                disequalities,
            );
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            let operand_and_literal = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(literal), _) => Some((binary.right, *literal)),
                (_, ExpressionNode::Boolean(literal)) => Some((binary.left, *literal)),
                _ => None,
            };
            if let Some((operand, literal)) = operand_and_literal {
                let equality_is_negated = if binary.operator == BinaryOperator::Equal {
                    negated
                } else {
                    !negated
                };
                collect_integer_order_relations(
                    program,
                    operand,
                    equality_is_negated == literal,
                    parameter_names,
                    content_conservation,
                    integer_types,
                    output,
                    disequalities,
                );
            }
            collect_normalized_integer_order_relation(
                program,
                binary.operator,
                binary.left,
                binary.right,
                negated,
                parameter_names,
                content_conservation,
                integer_types,
                output,
                disequalities,
            );
        }
        ExpressionNode::Binary(binary) => collect_normalized_integer_order_relation(
            program,
            binary.operator,
            binary.left,
            binary.right,
            negated,
            parameter_names,
            content_conservation,
            integer_types,
            output,
            disequalities,
        ),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_normalized_integer_order_relation(
    program: &TypedTrees,
    operator: typed_trees::expression::BinaryOperator,
    left: typed_trees::expression::ExpressionHandle,
    right: typed_trees::expression::ExpressionHandle,
    negated: bool,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    integer_types: &IntegerTypeClassification,
    output: &mut Vec<IntegerOrderRelation>,
    disequalities: &mut Vec<IntegerDisequality>,
) {
    use typed_trees::expression::BinaryOperator;

    if !comparison_operands_are_integers(program, left, right, integer_types) {
        return;
    }
    let Some(normalized) = normalized_comparison(operator, negated, true) else {
        return;
    };
    let mut push = |left, right, strict| {
        let relation = IntegerOrderRelation {
            left_identity: crate::facts::canonical_crash_operand_identity(
                program,
                left,
                parameter_names,
                content_conservation,
            ),
            right_identity: crate::facts::canonical_crash_operand_identity(
                program,
                right,
                parameter_names,
                content_conservation,
            ),
            left,
            right,
            strict,
        };
        if !output.contains(&relation) {
            output.push(relation);
        }
    };
    match normalized {
        BinaryOperator::Less => push(left, right, true),
        BinaryOperator::LessOrEqual => push(left, right, false),
        BinaryOperator::Greater => push(right, left, true),
        BinaryOperator::GreaterOrEqual => push(right, left, false),
        BinaryOperator::Equal => {
            push(left, right, false);
            push(right, left, false);
        }
        BinaryOperator::NotEqual => {
            let disequality = IntegerDisequality {
                left_identity: crate::facts::canonical_crash_operand_identity(
                    program,
                    left,
                    parameter_names,
                    content_conservation,
                ),
                right_identity: crate::facts::canonical_crash_operand_identity(
                    program,
                    right,
                    parameter_names,
                    content_conservation,
                ),
            };
            if !disequalities.contains(&disequality) {
                disequalities.push(disequality);
            }
        }
        _ => unreachable!("normalized comparisons use only comparison operators"),
    }
}

fn push_transitive_integer_order_consequences(
    program: &TypedTrees,
    relations: &mut Vec<IntegerOrderRelation>,
    disequalities: &[IntegerDisequality],
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    output: &mut Vec<checked_trees::CrashPredicateIdentity>,
) {
    let strict_refinements = relations
        .iter()
        .filter(|relation| {
            !relation.strict
                && disequalities.iter().any(|disequality| {
                    (disequality.left_identity == relation.left_identity
                        && disequality.right_identity == relation.right_identity)
                        || (disequality.left_identity == relation.right_identity
                            && disequality.right_identity == relation.left_identity)
                })
        })
        .map(|relation| IntegerOrderRelation {
            strict: true,
            ..relation.clone()
        })
        .collect::<Vec<_>>();
    for refinement in strict_refinements {
        if !relations.contains(&refinement) {
            relations.push(refinement);
        }
    }

    loop {
        let existing = relations.clone();
        let mut added = Vec::new();
        for left in &existing {
            for right in &existing {
                if left.right_identity != right.left_identity {
                    continue;
                }
                let relation = IntegerOrderRelation {
                    left_identity: left.left_identity.clone(),
                    right_identity: right.right_identity.clone(),
                    left: left.left,
                    right: right.right,
                    strict: left.strict || right.strict,
                };
                if !relations.contains(&relation) && !added.contains(&relation) {
                    added.push(relation);
                }
            }
        }
        if added.is_empty() {
            break;
        }
        relations.extend(added);
    }

    let nonstrict_equalities = relations
        .iter()
        .filter(|relation| {
            !relation.strict
                && relations.iter().any(|reverse| {
                    !reverse.strict
                        && reverse.left_identity == relation.right_identity
                        && reverse.right_identity == relation.left_identity
                })
        })
        .map(|relation| (relation.left, relation.right))
        .collect::<Vec<_>>();
    for (left, right) in nonstrict_equalities {
        push_comparison_consequences(
            program,
            typed_trees::expression::BinaryOperator::Equal,
            left,
            right,
            true,
            parameter_names,
            content_conservation,
            output,
        );
    }

    for relation in relations {
        push_comparison_consequences(
            program,
            if relation.strict {
                typed_trees::expression::BinaryOperator::Less
            } else {
                typed_trees::expression::BinaryOperator::LessOrEqual
            },
            relation.left,
            relation.right,
            true,
            parameter_names,
            content_conservation,
            output,
        );
    }
}

fn push_comparison_consequences(
    program: &TypedTrees,
    normalized: typed_trees::expression::BinaryOperator,
    left: typed_trees::expression::ExpressionHandle,
    right: typed_trees::expression::ExpressionHandle,
    operands_are_integers: bool,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    output: &mut Vec<checked_trees::CrashPredicateIdentity>,
) {
    use typed_trees::expression::BinaryOperator;

    let mut consequences = vec![normalized];
    if operands_are_integers {
        match normalized {
            BinaryOperator::Less => {
                consequences.extend([BinaryOperator::LessOrEqual, BinaryOperator::NotEqual]);
            }
            BinaryOperator::Greater => {
                consequences.extend([BinaryOperator::GreaterOrEqual, BinaryOperator::NotEqual]);
            }
            BinaryOperator::Equal => {
                consequences.extend([BinaryOperator::LessOrEqual, BinaryOperator::GreaterOrEqual]);
            }
            _ => {}
        }
    }
    for operator in consequences {
        output.push(crate::facts::canonical_crash_binary_path_predicate(
            program,
            operator,
            left,
            right,
            parameter_names,
            content_conservation,
        ));
        output.push(crate::facts::canonical_crash_binary_path_predicate(
            program,
            reversed_comparison(operator),
            right,
            left,
            parameter_names,
            content_conservation,
        ));
    }
}

fn normalized_comparison(
    operator: typed_trees::expression::BinaryOperator,
    negated: bool,
    operands_have_total_order: bool,
) -> Option<typed_trees::expression::BinaryOperator> {
    use typed_trees::expression::BinaryOperator;

    Some(match (operator, negated) {
        (BinaryOperator::Equal, false) => BinaryOperator::Equal,
        (BinaryOperator::Equal, true) => BinaryOperator::NotEqual,
        (BinaryOperator::NotEqual, false) => BinaryOperator::NotEqual,
        (BinaryOperator::NotEqual, true) => BinaryOperator::Equal,
        (BinaryOperator::Less, false) => BinaryOperator::Less,
        // Ordered negation is not portable to unordered float values. The
        // caller enables the complement only for checked integer operands.
        (BinaryOperator::Less, true) if operands_have_total_order => BinaryOperator::GreaterOrEqual,
        (BinaryOperator::LessOrEqual, false) => BinaryOperator::LessOrEqual,
        (BinaryOperator::LessOrEqual, true) if operands_have_total_order => BinaryOperator::Greater,
        (BinaryOperator::Greater, false) => BinaryOperator::Greater,
        (BinaryOperator::Greater, true) if operands_have_total_order => BinaryOperator::LessOrEqual,
        (BinaryOperator::GreaterOrEqual, false) => BinaryOperator::GreaterOrEqual,
        (BinaryOperator::GreaterOrEqual, true) if operands_have_total_order => BinaryOperator::Less,
        _ => return None,
    })
}

fn comparison_operands_are_integers(
    program: &TypedTrees,
    left: typed_trees::expression::ExpressionHandle,
    right: typed_trees::expression::ExpressionHandle,
    integer_types: &IntegerTypeClassification,
) -> bool {
    expression_is_integer_typed(program, left, integer_types)
        && expression_is_integer_typed(program, right, integer_types)
}

fn expression_is_integer_typed(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    integer_types: &IntegerTypeClassification,
) -> bool {
    use typed_trees::expression::ExpressionNode;

    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => true,
        ExpressionNode::Atomic(atomic) => {
            expression_is_integer_typed(program, atomic.value, integer_types)
        }
        ExpressionNode::Borrow(inner) => {
            expression_is_integer_typed(program, inner.target, integer_types)
        }
        ExpressionNode::Cast(cast) => type_reference_is_integer(program, cast.target_type),
        ExpressionNode::Name(path) => {
            crate::lookup::first_valid_name_path_symbol(path, &program.expression_table)
                .is_some_and(|symbol| integer_types.is_integer_typed(symbol))
        }
        ExpressionNode::Member(member) => {
            let symbol = crate::flow::effective_member_symbol(program, member.receiver, member);
            integer_types.is_integer_typed(symbol)
        }
        _ => false,
    }
}

fn type_reference_is_integer(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    program
        .type_reference_table
        .primitive_type(type_reference)
        .is_some_and(typed_trees::types::PrimitiveType::accepts_integer_literal)
}

/// Whole-program integer-type classification built once per path-conditioned
/// coverage pass. The typed program is immutable for the duration of the pass,
/// so one traversal over the declaration roster in the same order the replaced
/// per-query scan used retains each symbol's first-match answer: state
/// parameters, state locals, machine-owned data, then data members. Slots are
/// keyed by the declaring symbol's full generational identity; a symbol absent
/// from the roster, or one whose stale generation no longer occupies the slot,
/// reports not integer-typed exactly as the exhausted scan did.
pub(super) struct IntegerTypeClassification {
    /// Indexed by symbol arena index: `(generation, is_integer)` recorded by
    /// the declaring symbol. A zero generation means no roster declaration
    /// owns the slot.
    slots: Vec<(u32, bool)>,
}

impl IntegerTypeClassification {
    pub(super) fn build(program: &TypedTrees) -> Self {
        // Symbol arena indices are one-based, so slot 0 stays unwritten.
        let mut classification = Self {
            slots: vec![(0, false); program.symbols.symbols().len() + 1],
        };
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for parameter in program.state_parameters(state) {
                    classification.record(program, parameter.symbol, parameter.type_reference);
                }
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let typed_trees::statement::StatementNode::LocalData(local) = statement {
                        classification.record(program, local.symbol, local.type_reference);
                    }
                }
            }
            for owned in program.machine_owned_data(machine) {
                classification.record(program, owned.symbol, owned.type_reference);
            }
        }
        for data in program.data_definitions() {
            for member in program.data_members(data) {
                match member {
                    typed_trees::data::DataMember::Field(field) => {
                        classification.record(program, field.symbol, field.type_reference);
                    }
                    typed_trees::data::DataMember::Variant(variant) => {
                        for field in program.data_payload_fields(variant) {
                            classification.record(program, field.symbol, field.type_reference);
                        }
                    }
                }
            }
        }
        classification
    }

    /// The roster records a symbol only at its first position, matching the
    /// replaced scan's early return on the first declaration that named it.
    fn record(
        &mut self,
        program: &TypedTrees,
        symbol: symbols::SymbolHandle,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) {
        if !symbol.is_valid() {
            return;
        }
        if let Some(slot) = self.slots.get_mut(symbol.arena_index() as usize)
            && slot.0 == 0
        {
            *slot = (
                symbol.generation(),
                type_reference_is_integer(program, type_reference),
            );
        }
    }

    fn is_integer_typed(&self, symbol: symbols::SymbolHandle) -> bool {
        symbol.is_valid()
            && self.slots.get(symbol.arena_index() as usize).is_some_and(
                |&(generation, is_integer)| generation == symbol.generation() && is_integer,
            )
    }
}

fn reversed_comparison(
    operator: typed_trees::expression::BinaryOperator,
) -> typed_trees::expression::BinaryOperator {
    use typed_trees::expression::BinaryOperator;

    match operator {
        BinaryOperator::Equal => BinaryOperator::Equal,
        BinaryOperator::NotEqual => BinaryOperator::NotEqual,
        BinaryOperator::Less => BinaryOperator::Greater,
        BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
        BinaryOperator::Greater => BinaryOperator::Less,
        BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
        _ => unreachable!("only normalized comparisons reach operand reversal"),
    }
}

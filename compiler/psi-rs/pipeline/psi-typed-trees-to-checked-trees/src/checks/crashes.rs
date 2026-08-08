//! Path-conditioned crash-route coverage.
//!
//! A state reached only through a retained incoming guard has that guard in
//! every execution path reaching its body. The same is true of the negated
//! guard on a fallthrough/continuation edge. When that exact normalized
//! predicate is one alternative in a same-cause published bucket, the site's
//! derived condition implies the bucket guard. This is checked implementation
//! evidence and never enters the public contract fingerprint.

use psi_checked_trees::{CheckFacts, CrashRouteGuard};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;

pub(crate) fn infer_path_conditioned_guard_coverage(program: &TypedTrees, facts: &mut CheckFacts) {
    let content_conservation = psi_validation::build_content_conservation_plans(program);
    for machine in program.machines() {
        let incoming =
            super::ranges::incoming_guards::collect_incoming_guard_facts(program, machine);
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

        let checked_sites = crash_plan
            .checked_sites()
            .iter()
            .map(|site| {
                let mut covering = site.guard_covering_buckets().to_vec();
                let path_guard_conjuncts = incoming
                    .iter()
                    .filter(|guard| guard.applies_at(site.location().state()))
                    .map(|guard| {
                        crate::facts::canonical_crash_path_predicate(
                            program,
                            guard.guard(),
                            guard.is_negated(),
                            &parameter_names,
                            &content_conservation,
                        )
                    })
                    .collect::<Vec<_>>();
                let mut path_predicates = Vec::new();
                for guard in incoming
                    .iter()
                    .filter(|guard| guard.applies_at(site.location().state()))
                {
                    collect_structural_guard_consequences(
                        program,
                        guard.guard(),
                        guard.is_negated(),
                        &parameter_names,
                        &content_conservation,
                        &mut path_predicates,
                    );
                }
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
                    .collect::<Vec<_>>();
                let path_guard_conjuncts = applicable_guards
                    .iter()
                    .map(|guard| {
                        crate::facts::canonical_crash_path_predicate(
                            program,
                            guard.guard(),
                            guard.is_negated(),
                            &parameter_names,
                            &content_conservation,
                        )
                    })
                    .collect::<Vec<_>>();
                let mut path_guard_consequences = Vec::new();
                for guard in applicable_guards {
                    collect_structural_guard_consequences(
                        program,
                        guard.guard(),
                        guard.is_negated(),
                        &parameter_names,
                        &content_conservation,
                        &mut path_guard_consequences,
                    );
                }
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

pub(crate) fn check_call_ceiling_coverage(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for caller in facts.contract_plans.machines.iter().filter(|plan| {
        plan.crash.interface() == psi_checked_trees::CrashInterface::PublishedCeiling
    }) {
        for call in caller.crash.checked_calls() {
            for surviving in call.surviving_buckets() {
                let covered = surviving.alternative_guards().iter().all(|route| {
                    caller.crash.published().iter().any(|published| {
                        published.cause() == surviving.cause()
                            && psi_checked_trees::crash_scope_covers_minimum(
                                surviving.containment_demand(),
                                published.containment_demand(),
                            )
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
                let caller_name = program
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == caller.machine)
                    .map(|machine| machine.name.as_str())
                    .unwrap_or("<unknown>");
                let target_name = program
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == call.target_machine())
                    .map(|machine| machine.name.as_str())
                    .unwrap_or("<unknown>");
                diagnostics.push(Diagnostic::error(format!(
                    "call from `{caller_name}` to `{target_name}` at statement {} call {} has an uncovered {:?} crash route requiring `{}` containment; publish a same-cause route whose guard and containment demand cover this invocation",
                    call.location().statement_ordinal(),
                    call.location().call_ordinal(),
                    surviving.cause(),
                    surviving.containment_demand(),
                )));
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
    path_guard_consequences: &[psi_checked_trees::CrashPredicateIdentity],
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
    expression: psi_typed_trees::expression::ExpressionHandle,
    negated: bool,
    parameter_names: &[String],
    content_conservation: &[psi_validation::ContentConservationSourcePlan],
    output: &mut Vec<psi_checked_trees::CrashPredicateIdentity>,
) {
    use psi_typed_trees::expression::{BinaryOperator, ExpressionNode, UnaryOperator};

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
                output,
            );
            collect_structural_guard_consequences(
                program,
                binary.right,
                negated,
                parameter_names,
                content_conservation,
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
                    output,
                );
            }
            let normalized = normalized_comparison(binary.operator, negated)
                .expect("equality operators are comparisons");
            output.push(crate::facts::canonical_crash_binary_path_predicate(
                program,
                normalized,
                binary.left,
                binary.right,
                parameter_names,
                content_conservation,
            ));
            output.push(crate::facts::canonical_crash_binary_path_predicate(
                program,
                reversed_comparison(normalized),
                binary.right,
                binary.left,
                parameter_names,
                content_conservation,
            ));
        }
        ExpressionNode::Binary(binary)
            if normalized_comparison(binary.operator, negated).is_some() =>
        {
            let normalized = normalized_comparison(binary.operator, negated)
                .expect("comparison operator was matched above");
            output.push(crate::facts::canonical_crash_binary_path_predicate(
                program,
                normalized,
                binary.left,
                binary.right,
                parameter_names,
                content_conservation,
            ));
            output.push(crate::facts::canonical_crash_binary_path_predicate(
                program,
                reversed_comparison(normalized),
                binary.right,
                binary.left,
                parameter_names,
                content_conservation,
            ));
        }
        _ => {}
    }
}

fn normalized_comparison(
    operator: psi_typed_trees::expression::BinaryOperator,
    negated: bool,
) -> Option<psi_typed_trees::expression::BinaryOperator> {
    use psi_typed_trees::expression::BinaryOperator;

    Some(match (operator, negated) {
        (BinaryOperator::Equal, false) => BinaryOperator::Equal,
        (BinaryOperator::Equal, true) => BinaryOperator::NotEqual,
        (BinaryOperator::NotEqual, false) => BinaryOperator::NotEqual,
        (BinaryOperator::NotEqual, true) => BinaryOperator::Equal,
        (BinaryOperator::Less, false) => BinaryOperator::Less,
        // Ordered negation is not portable to unordered float values:
        // `!(x < y)` does not imply `x >= y` when either operand is NaN.
        (BinaryOperator::Less, true) => return None,
        (BinaryOperator::LessOrEqual, false) => BinaryOperator::LessOrEqual,
        (BinaryOperator::LessOrEqual, true) => return None,
        (BinaryOperator::Greater, false) => BinaryOperator::Greater,
        (BinaryOperator::Greater, true) => return None,
        (BinaryOperator::GreaterOrEqual, false) => BinaryOperator::GreaterOrEqual,
        (BinaryOperator::GreaterOrEqual, true) => return None,
        _ => return None,
    })
}

fn reversed_comparison(
    operator: psi_typed_trees::expression::BinaryOperator,
) -> psi_typed_trees::expression::BinaryOperator {
    use psi_typed_trees::expression::BinaryOperator;

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

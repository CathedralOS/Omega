//! Path-conditioned crash-route coverage.
//!
//! A state reached only through a retained incoming guard has that guard in
//! every execution path reaching its body. The same is true of the negated
//! guard on a fallthrough/continuation edge. When that exact normalized
//! predicate is one alternative in a same-cause published bucket, the site's
//! derived condition implies the bucket guard. This is checked implementation
//! evidence and never enters the public contract fingerprint.

use psi_checked_trees::{CheckFacts, CrashRouteGuard};
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
        if crash_plan.checked_sites().is_empty() {
            continue;
        }

        let checked_sites = crash_plan
            .checked_sites()
            .iter()
            .map(|site| {
                let mut covering = site.guard_covering_buckets().to_vec();
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

                site.clone().with_guard_covering_buckets(covering)
            })
            .collect();
        facts.contract_plans.machines[contract_index].crash = crash_plan
            .clone()
            .with_checked_sites(checked_sites)
            .expect("path-conditioned coverage retains valid checked-site identity");
    }
}

/// Retain only propositional consequences that follow structurally from one
/// incoming predicate. A positive conjunction entails each conjunct; a
/// negated disjunction entails each negated disjunct; and logical negation
/// flips polarity. Everything else remains an opaque canonical atom. This is
/// deliberately incomplete but sound, and does not rewrite public routes.
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
        _ => {}
    }
}

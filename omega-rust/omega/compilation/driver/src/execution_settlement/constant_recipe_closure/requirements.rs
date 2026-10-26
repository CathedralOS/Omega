//! Retain each discharged judgment's joint, qualified observation assignment.
//! Materialization transfers a policy judgment to the exact carrier it made;
//! erasing that policy call cannot erase the condition of its result.

use checked_trees::{
    CheckedLayoutAlternative, CheckedLayoutObservation, CheckedLayoutOwner,
    CheckedLayoutRequirement, CheckedTrees,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(super) fn retain(
    shared: &CheckedTrees,
    closed: &typed_trees::TypedTrees,
    target: target::TargetProfile,
    providers: &[provider_planning::SelectedProviderReviewProvenance],
    inputs: &super::query_inputs::Inputs,
    requirements: &mut Vec<CheckedLayoutRequirement>,
) -> Result<(), Vec<Diagnostic>> {
    for (proof_owner, expressions) in &inputs.owners {
        let mut observations = Vec::new();
        for expression in expressions {
            let subjects: Vec<_> = shared
                .facts
                .layout_query_subjects
                .iter()
                .filter(|query| query.expression == *expression)
                .collect();
            if subjects.is_empty() {
                return Err(vec![
                    Diagnostic::error("closed observation lost its checked subject")
                        .with_source_span(closed.expression_table.source_span(*expression)),
                ]);
            }
            for query in subjects {
                let value = closed
                    .closed_integer_expression_value_in(query.expression, query.owner)
                    .and_then(|value| value.to_u64())
                    .ok_or_else(|| {
                        vec![
                            Diagnostic::error(
                                "discharged layout observation has no closed u64 value",
                            )
                            .with_source_span(closed.expression_table.source_span(*expression)),
                        ]
                    })?;
                observations.push(CheckedLayoutObservation {
                    expression: query.expression,
                    owner: query.owner,
                    value,
                });
            }
        }
        observations.sort_by_key(|observation| {
            (
                observation.expression.arena_index(),
                observation.expression.generation(),
                observation.owner.arena_index(),
                observation.owner.generation(),
            )
        });
        observations.dedup();
        let alternative = CheckedLayoutAlternative {
            target: typed_trees::name::Identifier::from(target.target_name()),
            observations,
        };
        append(
            requirements,
            *proof_owner,
            CheckedLayoutOwner::Machine(*proof_owner),
            &alternative,
        );
        // A type-position recipe produces geometry even when its evaluator
        // has no runtime body. The checked array shape retains the recipe's
        // exact identity; transfer its complete observation assignment to that
        // surviving carrier rather than inventing a callable for the evaluator.
        for recipe in &inputs.recipes {
            let selected = shared.typed.constant_recipes.get(*recipe);
            if !provider_planning::target_machines::proof_application_dependencies(
                shared,
                target,
                selected.machine,
                providers,
            )
            .contains(proof_owner)
            {
                continue;
            }
            for shape in &shared.facts.flow.terminal_unit_effects.structural_types {
                if matches!(shape.shape,
                    checked_trees::CheckedUnitStructuralTypeShape::FixedArray {
                        length: checked_trees::CheckedExtent::Recipe(handle), ..
                    } if handle == *recipe)
                {
                    for obligation in &shared.facts.constant_recipe_obligations {
                        if obligation.recipe == *recipe
                            && inputs.applications.contains(&obligation.owner)
                        {
                            append(
                                requirements,
                                *proof_owner,
                                CheckedLayoutOwner::Structural {
                                    machine: obligation.owner,
                                    identity: shape.identity,
                                },
                                &alternative,
                            );
                        }
                    }
                }
            }
        }
        for layout in &shared.typed.plan_laid_layouts {
            let roots: Vec<_> = layout
                .target_layouts(&shared.typed)
                .iter()
                .filter(|row| row.target.as_str() == target.target_name())
                .map(|row| row.machine)
                .chain(
                    matches!(
                        layout.realizations,
                        typed_trees::PlanLaidRealizations::Invariant(_)
                    )
                    .then_some(layout.policy_plan_machine_symbol),
                )
                .collect();
            if roots.into_iter().any(|root| {
                provider_planning::target_machines::proof_application_dependencies(
                    shared, target, root, providers,
                )
                .contains(proof_owner)
            }) {
                append(
                    requirements,
                    *proof_owner,
                    CheckedLayoutOwner::Carrier(layout.data_symbol),
                    &alternative,
                );
            }
        }
    }
    Ok(())
}

fn append(
    requirements: &mut Vec<CheckedLayoutRequirement>,
    proof_owner: SymbolHandle,
    owner: CheckedLayoutOwner,
    alternative: &CheckedLayoutAlternative,
) {
    if let Some(requirement) = requirements
        .iter_mut()
        .find(|requirement| requirement.proof_owner == proof_owner && requirement.owner == owner)
    {
        if !requirement.alternatives.contains(alternative) {
            requirement.alternatives.push(alternative.clone());
        }
    } else {
        requirements.push(CheckedLayoutRequirement {
            proof_owner,
            owner,
            alternatives: vec![alternative.clone()],
        });
    }
}

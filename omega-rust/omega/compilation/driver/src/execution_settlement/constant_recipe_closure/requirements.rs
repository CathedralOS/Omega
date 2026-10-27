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
    let mut storage_types: std::collections::HashMap<
        SymbolHandle,
        Vec<typed_trees::type_identity::TypeIdentityId>,
    > = std::collections::HashMap::new();
    for (owner, root) in &inputs.storage_roots {
        let mut extent_failure = None;
        let types = shared
            .facts
            .flow
            .terminal_unit_effects
            .stored_type_closure(*root, |length| match length {
                checked_trees::CheckedExtent::Literal(length) => length != 0,
                checked_trees::CheckedExtent::Recipe(recipe) => {
                    match closed.closed_array_extent_value(recipe) {
                        Ok(Some(value)) => match value.to_u64() {
                            Some(length) => length != 0,
                            None => {
                                extent_failure = Some("materialized storage has a non-u64 array extent");
                                false
                            }
                        },
                        Ok(None) => {
                            extent_failure = Some("materialized storage lost its closed array extent in the selected target context");
                            false
                        }
                        Err(reason) => { extent_failure = Some(reason); false }
                    }
                }
            });
        if let Some(reason) = extent_failure {
            return Err(vec![Diagnostic::error(reason)]);
        }
        storage_types.entry(*owner).or_default().extend(types);
    }
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
        append_machine(
            shared,
            requirements,
            *proof_owner,
            *proof_owner,
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
            for (owner, consumed) in &inputs.recipe_consumers {
                if *consumed == *recipe {
                    append_machine(shared, requirements, *proof_owner, *owner, &alternative);
                }
            }
            for formation in &inputs.array_extent_uses {
                if formation.recipe != *recipe {
                    continue;
                }
                if storage_types
                    .get(&formation.owner)
                    .is_some_and(|types| types.contains(&formation.identity))
                {
                    append(
                        requirements,
                        *proof_owner,
                        CheckedLayoutOwner::Structural {
                            machine: formation.owner,
                            identity: formation.identity,
                        },
                        &alternative,
                    );
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

/// Formation owns a canonical value rather than an executable body. Retain its
/// assignment as well as every runtime consumer's assignment, including when a
/// type-position recipe transfers the judgment to that declaration value.
fn append_machine(
    shared: &CheckedTrees,
    requirements: &mut Vec<CheckedLayoutRequirement>,
    proof_owner: SymbolHandle,
    receiver: SymbolHandle,
    alternative: &CheckedLayoutAlternative,
) {
    let owner = if shared.typed.const_declaration_by_symbol(receiver).is_some() {
        CheckedLayoutOwner::Constant(receiver)
    } else {
        CheckedLayoutOwner::Machine(receiver)
    };
    append(requirements, proof_owner, owner, alternative);
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

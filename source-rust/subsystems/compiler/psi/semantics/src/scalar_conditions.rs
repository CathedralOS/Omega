//! Read condition subjects and their scalar application dependencies from a published Psi catalog.
use numerics::integer_bounds::IntegerBoundStep;
use numerics::scalar_predicates::PredicateNode;
use semantic_vocabulary::{BoundaryMachineId, MachineId, ScalarApplicationId};
use terminal_psi::{
    ProofApplicationScope, ProofDeclarationId, ScalarCompletionCondition, ScalarConditionSubject,
    ScalarConditions, ScalarRangeCondition, ScalarRangeObservation, ScalarRangeRequirement,
    TargetMachineCallable,
};

pub fn proof_provider_edges(
    scope: &ProofApplicationScope,
    providers: impl IntoIterator<Item = (BoundaryMachineId, MachineId)>,
) -> Result<Vec<(ProofDeclarationId, ProofDeclarationId)>, String> {
    providers
        .into_iter()
        .map(|(boundary, body)| {
            let requirement = scope
                .callable_declaration(TargetMachineCallable::Boundary(boundary))
                .ok_or("selected provider requirement lost its proof declaration")?;
            let realization = scope
                .callable_declaration(TargetMachineCallable::Machine(body))
                .ok_or("selected provider body lost its proof declaration")?;
            Ok((requirement, realization))
        })
        .collect()
}

pub fn invocation_declarations(
    scope: &ProofApplicationScope,
    roots: &[ProofDeclarationId],
    machines: &[ProofDeclarationId],
) -> Vec<ProofDeclarationId> {
    let mut dependencies = roots.to_vec();
    dependencies.extend_from_slice(machines);
    scope.plan.declaration_closure(&dependencies)
}

#[derive(Clone, Copy)]
pub enum ConditionPosition {
    Range(usize),
    Predicate(usize),
    Completion(usize),
}

pub fn visit_condition_rows(
    conditions: &ScalarConditions,
    mut visit: impl FnMut(ScalarConditionSubject, bool, ConditionPosition),
) -> Result<(), String> {
    for (position, condition) in conditions.ranges().iter().enumerate() {
        let ScalarRangeCondition::Range {
            subject,
            requirement,
            ..
        } = condition
        else {
            return Err("scalar scope contains an absent range condition".into());
        };
        visit(
            *subject,
            matches!(requirement, ScalarRangeRequirement::AddressModel),
            ConditionPosition::Range(position),
        );
    }
    for (position, condition) in conditions.predicates().iter().enumerate() {
        visit(
            condition.subject,
            false,
            ConditionPosition::Predicate(position),
        );
    }
    for (position, condition) in conditions.completions().iter().enumerate() {
        let ScalarCompletionCondition::Complete { subject, .. } = condition else {
            return Err("scalar scope contains an absent completion condition".into());
        };
        visit(*subject, true, ConditionPosition::Completion(position));
    }
    Ok(())
}

pub fn visit_condition_applications(
    conditions: &ScalarConditions,
    row: ConditionPosition,
    mut visit: impl FnMut(ScalarApplicationId),
) -> Result<(), String> {
    let mut observation = |observation: &ScalarRangeObservation| -> Result<(), String> {
        match observation {
            ScalarRangeObservation::Application(application)
            | ScalarRangeObservation::EntryResult { application, .. } => {
                visit(*application);
                Ok(())
            }
            ScalarRangeObservation::Literal(_) => Ok(()),
            ScalarRangeObservation::Missing => Err("scalar condition observation is absent".into()),
        }
    };
    match row {
        ConditionPosition::Completion(position) => {
            let ScalarCompletionCondition::Complete { application, .. } = *conditions
                .completions()
                .get(position)
                .ok_or("scalar completion condition is absent")?
            else {
                return Err("scalar completion condition is absent".into());
            };
            visit(application);
        }
        ConditionPosition::Range(position) => {
            for step in conditions.steps(
                conditions
                    .ranges()
                    .get(position)
                    .ok_or("scalar range condition is absent")?,
            )? {
                if let IntegerBoundStep::Observation(value) = step {
                    observation(value)?;
                }
            }
        }
        ConditionPosition::Predicate(position) => {
            for (_, node) in conditions
                .predicates()
                .get(position)
                .ok_or("scalar predicate condition is absent")?
                .nodes()
                .iter()
            {
                if let PredicateNode::Leaf(value) = &node.0 {
                    observation(value)?;
                }
            }
        }
    }
    Ok(())
}

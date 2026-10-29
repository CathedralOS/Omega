//! Select retained proof debt using the same declaration membership as source
//! checking. Available provider candidates are not selected provider edges.
//! This is a work selection, never evidence that any condition is true.

use numerics::integer_bounds::IntegerBoundStep;
use numerics::scalar_predicates::PredicateNode;
use semantic_vocabulary::{BoundaryMachineId, MachineId, ScalarApplicationId};
use terminal_psi::{
    ProofApplicationScope, ScalarConditionSubject, ScalarConditions, ScalarRangeCondition,
    ScalarRangeObservation, ScalarRangeRequirement, TargetMachineCallable,
};

/// Invocation-local rows selected from one immutable condition roster.
pub struct ScalarConditionSelection<'artifact> {
    conditions: &'artifact ScalarConditions,
    target: String,
    primary: bool,
    providers: Vec<(BoundaryMachineId, MachineId)>,
    ranges: Vec<usize>,
    predicates: Vec<usize>,
}

impl<'artifact> ScalarConditionSelection<'artifact> {
    pub(super) fn select(
        scope: &ProofApplicationScope,
        conditions: &'artifact ScalarConditions,
        target: &str,
        primary: bool,
        providers: impl IntoIterator<Item = (BoundaryMachineId, MachineId)>,
        demanded: &[ScalarApplicationId],
    ) -> Result<Self, String> {
        let providers: Vec<_> = providers.into_iter().collect();
        let mut selected = Vec::new();
        for &(boundary, body) in &providers {
            let requirement = scope
                .callable_declaration(TargetMachineCallable::Boundary(boundary))
                .ok_or("selected provider requirement lost its proof declaration")?;
            let realization = scope
                .callable_declaration(TargetMachineCallable::Machine(body))
                .ok_or("selected provider body lost its proof declaration")?;
            selected.push((requirement, realization));
        }
        let applications = scope.applications();
        let (machines, declarations) = applications.membership(target, primary, &selected);
        let all_declarations = applications.declarations();
        let applies = |subject, address_model| match subject {
            ScalarConditionSubject::Module => true,
            ScalarConditionSubject::Declaration(owner) => {
                machines.contains(&owner)
                    || (address_model
                        && (declarations.contains(&owner)
                            || (primary && all_declarations.contains(&owner))))
            }
            // Keeping an entry in the catalog is not a demand to execute it.
            ScalarConditionSubject::Application(application) => demanded.contains(&application),
        };
        let mut ranges = Vec::new();
        for (position, condition) in conditions.ranges().iter().enumerate() {
            let ScalarRangeCondition::Range {
                subject,
                requirement,
                ..
            } = condition
            else {
                return Err("scalar scope contains an absent range condition".into());
            };
            if applies(
                *subject,
                matches!(requirement, ScalarRangeRequirement::AddressModel),
            ) {
                ranges.push(position);
            }
        }
        let predicates = conditions
            .predicates()
            .iter()
            .enumerate()
            .filter_map(|(position, condition)| {
                applies(condition.subject, false).then_some(position)
            })
            .collect();
        Ok(Self {
            conditions,
            target: target.to_owned(),
            primary,
            providers,
            ranges,
            predicates,
        })
    }

    pub fn ranges(&self) -> &[usize] {
        &self.ranges
    }
    pub fn predicates(&self) -> &[usize] {
        &self.predicates
    }

    /// Selection is bound to the conditions and actual provider edges it read.
    /// This is an input join, not a second membership or proof traversal.
    pub(super) fn matches(
        &self,
        observations: &super::ScalarApplicationObservations<'_>,
        primary: &str,
    ) -> bool {
        std::ptr::eq(
            self.conditions,
            &observations
                .artifact()
                .module()
                .scalar_applications
                .conditions,
        ) && self.target == observations.targets().target()
            && self.primary == (self.target == primary)
            && self
                .providers
                .iter()
                .copied()
                .eq(observations.providers().providers().chain(
                    observations
                        .providers()
                        .assumed_bindings()
                        .iter()
                        .map(|binding| (binding.required, binding.implementation)),
                ))
    }

    /// Grow entry-owned debt as dependency discovery demands more jobs. The
    /// target/declaration membership prepared at selection remains unchanged.
    pub fn include_applications(&mut self, demanded: &[ScalarApplicationId]) {
        for (position, condition) in self.conditions.ranges().iter().enumerate() {
            if matches!(condition, ScalarRangeCondition::Range { subject: ScalarConditionSubject::Application(application), .. } if demanded.contains(application))
                && !self.ranges.contains(&position)
            {
                self.ranges.push(position);
            }
        }
        for (position, condition) in self.conditions.predicates().iter().enumerate() {
            if matches!(condition.subject, ScalarConditionSubject::Application(application) if demanded.contains(&application))
                && !self.predicates.contains(&position)
            {
                self.predicates.push(position);
            }
        }
        self.ranges.sort_unstable();
        self.predicates.sort_unstable();
    }

    /// Observe only selected debts. These include intermediate entry results,
    /// not just each application's returned scalar.
    pub fn observations(&self) -> Result<Vec<&'artifact ScalarRangeObservation>, String> {
        let mut observations = Vec::new();
        for &position in &self.ranges {
            for step in self.conditions.steps(&self.conditions.ranges()[position])? {
                if let IntegerBoundStep::Observation(observation) = step {
                    observations.push(observation);
                }
            }
        }
        for &position in &self.predicates {
            for (_, node) in self.conditions.predicates()[position].nodes().iter() {
                if let PredicateNode::Leaf(observation) = &node.0 {
                    observations.push(observation);
                }
            }
        }
        Ok(observations)
    }

    pub fn applications(&self) -> Result<Vec<ScalarApplicationId>, String> {
        let mut applications = Vec::new();
        for observation in self.observations()? {
            let application = match observation {
                ScalarRangeObservation::Application(application)
                | ScalarRangeObservation::EntryResult { application, .. } => *application,
                ScalarRangeObservation::Literal(_) => continue,
                ScalarRangeObservation::Missing => {
                    return Err("scalar condition observation is absent".into());
                }
            };
            if !applications.contains(&application) {
                applications.push(application);
            }
        }
        Ok(applications)
    }
}

#[cfg(test)]
mod tests;

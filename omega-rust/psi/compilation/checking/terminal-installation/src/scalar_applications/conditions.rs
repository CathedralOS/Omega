//! Select retained proof debt using the same declaration membership as source
//! checking. Available provider candidates are not selected provider edges.
//! This is a work selection, never evidence that any condition is true.

use super::ScalarApplicationPurpose;
use numerics::integer_bounds::IntegerBoundStep;
use numerics::scalar_predicates::PredicateNode;
use semantic_vocabulary::{BoundaryMachineId, MachineId, ScalarApplicationId};
use terminal_psi::{
    ProofApplicationScope, ScalarCompletionCondition, ScalarConditionSubject, ScalarConditions,
    ScalarRangeCondition, ScalarRangeObservation, ScalarRangeRequirement, TargetMachineCallable,
};

/// Invocation-local rows selected from one immutable condition roster.
pub struct ScalarConditionSelection<'artifact> {
    conditions: &'artifact ScalarConditions,
    target: String,
    primary: bool,
    purpose: ScalarApplicationPurpose,
    providers: Vec<(BoundaryMachineId, MachineId)>,
    ranges: Vec<usize>,
    predicates: Vec<usize>,
    completions: Vec<usize>,
    application_machines: Option<Vec<terminal_psi::ProofDeclarationId>>,
    application_roots: Option<Vec<terminal_psi::ProofDeclarationId>>,
}

impl<'artifact> ScalarConditionSelection<'artifact> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn select(
        scope: &ProofApplicationScope,
        conditions: &'artifact ScalarConditions,
        target: &str,
        primary: bool,
        providers: impl IntoIterator<Item = (BoundaryMachineId, MachineId)>,
        demanded: &[ScalarApplicationId],
        roots: Option<&[terminal_psi::ProofDeclarationId]>,
        purpose: ScalarApplicationPurpose,
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
        let (machines, declarations) = match purpose {
            ScalarApplicationPurpose::Product => {
                applications.membership(target, primary, &selected, roots)
            }
            ScalarApplicationPurpose::Invocation => {
                let roots = roots.ok_or("scalar invocation has no exact entry roots")?;
                let machines = applications
                    .invocation_reachability(target, &selected, roots)
                    .machines;
                let declarations = scope.plan.declaration_closure(&machines);
                (machines, declarations)
            }
        };
        let all_declarations = applications.declarations();
        let applies = |subject, address_model| match subject {
            ScalarConditionSubject::Module => true,
            ScalarConditionSubject::Declaration(owner) => {
                machines.contains(&owner)
                    || (address_model
                        && (declarations.contains(&owner)
                            || (primary
                                && purpose == ScalarApplicationPurpose::Product
                                && all_declarations.contains(&owner))))
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
        let mut completions = Vec::new();
        for (position, condition) in conditions.completions().iter().enumerate() {
            let ScalarCompletionCondition::Complete { subject, .. } = condition else {
                return Err("scalar scope contains an absent completion condition".into());
            };
            // Constant root completion is declaration formation, including
            // bodyless signatures, not just a predicate in an executable body.
            if applies(*subject, true) {
                completions.push(position);
            }
        }
        Ok(Self {
            conditions,
            target: target.to_owned(),
            primary,
            purpose,
            providers,
            ranges,
            predicates,
            completions,
            application_machines: roots.map(|_| machines),
            application_roots: roots.map(<[_]>::to_vec),
        })
    }

    pub(super) fn application_machines(&self) -> Option<&[terminal_psi::ProofDeclarationId]> {
        self.application_machines.as_deref()
    }

    pub fn ranges(&self) -> &[usize] {
        &self.ranges
    }
    pub fn predicates(&self) -> &[usize] {
        &self.predicates
    }

    pub fn completions(&self) -> &[usize] {
        &self.completions
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
            && self.purpose == observations.purpose
            && self.application_roots == observations.application_roots
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
        for (position, condition) in self.conditions.completions().iter().enumerate() {
            if matches!(condition, ScalarCompletionCondition::Complete { subject: ScalarConditionSubject::Application(application), .. } if demanded.contains(application))
                && !self.completions.contains(&position)
            {
                self.completions.push(position);
            }
        }
        self.completions.sort_unstable();
    }

    /// Refuse independently ready debts before executing more scalar entries.
    /// Entry-result debts remain pending until their complete activation returns;
    /// final closure still discharges every selected row and the address model.
    pub fn discharge_ready(
        &self,
        observations: &super::ScalarApplicationObservations<'_>,
    ) -> Result<(), String> {
        if !std::ptr::eq(
            self.conditions,
            &observations
                .artifact()
                .module()
                .scalar_applications
                .conditions,
        ) {
            return Err("scalar condition selection names another artifact".into());
        }
        for &position in &self.predicates {
            let mut ready = true;
            for (_, node) in self.conditions.predicates()[position].nodes().iter() {
                if let PredicateNode::Leaf(observation) = &node.0 {
                    ready &= observations.observation_ready(observation)?;
                }
            }
            if ready {
                observations.discharge_predicate(position)?;
            }
        }
        for &position in &self.ranges {
            let condition = &self.conditions.ranges()[position];
            let ScalarRangeCondition::Range { requirement, .. } = condition else {
                return Err("scalar range condition is absent".into());
            };
            if matches!(requirement, ScalarRangeRequirement::AddressModel) {
                continue;
            }
            let mut ready = true;
            for step in self.conditions.steps(condition)? {
                if let IntegerBoundStep::Observation(observation) = step {
                    ready &= observations.observation_ready(observation)?;
                }
            }
            if ready {
                observations.discharge_range(position, None)?;
            }
        }
        Ok(())
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
        for &position in &self.completions {
            let ScalarCompletionCondition::Complete { application, .. } =
                self.conditions.completions()[position]
            else {
                return Err("scalar completion condition is absent".into());
            };
            if !applications.contains(&application) {
                applications.push(application);
            }
        }
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

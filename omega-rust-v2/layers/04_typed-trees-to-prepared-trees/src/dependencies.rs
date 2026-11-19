use std::collections::HashMap;

use omega_resolver::DeclarationId;
use omega_typer::StaticValueDemand;

use crate::TargetObservation;

/// One frozen value or one specialized use of an open source expression.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum PreparedValueOrigin {
    Constant(DeclarationId),
    StaticIndex(StaticValueDemand),
}

/// The evaluator records the named values and target inputs it actually read.
/// A consumer can follow a named constant further without copying its inputs
/// into every use or mistaking equal output values for equal dependencies.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ValueInputs {
    pub constants: Vec<DeclarationId>,
    pub target_observations: Vec<TargetObservation>,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct PreparationDependencies {
    values: HashMap<PreparedValueOrigin, ValueInputs>,
}

impl PreparationDependencies {
    pub fn inputs(
        &self,
        origin: &PreparedValueOrigin,
    ) -> Option<&ValueInputs> {
        self.values.get(origin)
    }

    pub(crate) fn record_constant(
        &mut self,
        origin: &PreparedValueOrigin,
        input: &DeclarationId,
    ) {
        let dependencies = self.values.entry(origin.clone()).or_default();
        if !dependencies.constants.contains(input) {
            dependencies.constants.push(input.clone());
        }
    }

    pub(crate) fn record_observation(
        &mut self,
        origin: &PreparedValueOrigin,
        input: &TargetObservation,
    ) {
        let dependencies = self.values.entry(origin.clone()).or_default();
        if !dependencies.target_observations.contains(input) {
            dependencies.target_observations.push(input.clone());
        }
    }

    pub(crate) fn extend(
        &mut self,
        pass: Self,
    ) {
        for (origin, inputs) in pass.values {
            match self.values.entry(origin) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(inputs);
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    let existing = entry.get_mut();
                    for constant in inputs.constants {
                        if !existing.constants.contains(&constant) {
                            existing.constants.push(constant);
                        }
                    }
                    for observation in inputs.target_observations {
                        if !existing.target_observations.contains(&observation) {
                            existing.target_observations.push(observation);
                        }
                    }
                }
            }
        }
    }
}

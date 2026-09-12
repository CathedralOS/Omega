//! Publication replays the dependency rather than trusting its retained arena
//! coordinates. Nominal variables are then projected into the existing exact
//! static telescope; private helper names never enter package identity.

use crate::capture::behavior::project_service_row;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::facts::exactly_one;
use crate::record::{
    PackagePolicyMachineParameterContract, PackagePolicyServiceReachDependency,
    PackagePolicyTypeParameter, PackagePolicyTypeParameterKind,
};
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use flow_effects::ServiceReachInferencePlan;

pub(super) fn project(
    compilation: &CheckedCompilation,
    machine: &typed_trees::machine::Machine,
    inferred: &ServiceReachInferencePlan,
    projected_parameters: &[PackagePolicyTypeParameter],
) -> Result<PackagePolicyServiceReachDependency, Vec<Diagnostic>> {
    let checked = exactly_one(
        compilation
            .facts
            .service_reaches
            .machines()
            .iter()
            .filter(|row| row.machine == machine.symbol),
        machine.name.as_str(),
        "service-reach dependency",
    )?;
    let expected = exactly_one(
        inferred
            .machines()
            .iter()
            .filter(|row| row.machine == machine.symbol),
        machine.name.as_str(),
        "replayed service-reach dependency",
    )?;
    let concrete = project_service_row(compilation, checked.dependency.concrete)?;
    let mut expected_concrete = inferred
        .rows
        .services(expected.dependency.concrete)
        .iter()
        .map(|service| {
            let definition = compilation
                .service_reaches
                .definition(*service)
                .ok_or_else(|| rejected("replayed dependency has an unknown service"))?;
            nominal_identity(compilation, definition.symbol)
        })
        .collect::<Result<Vec<_>, _>>()?;
    expected_concrete.sort();
    expected_concrete.dedup();
    let checked_parameters = compilation
        .facts
        .service_reaches
        .dependency_parameters
        .span_or_empty(checked.dependency.parameters);
    let expected_parameters = inferred
        .dependency_parameters
        .span_or_empty(expected.dependency.parameters);
    if concrete != expected_concrete
        || checked_parameters != expected_parameters
        || checked_parameters.len() != checked.dependency.parameters.len()
        || expected_parameters.len() != expected.dependency.parameters.len()
    {
        return Err(rejected(
            "checked service-reach dependency differs from independent inference",
        ));
    }
    let parameters = compilation.machine_type_parameters(machine);
    let mut ordinals = Vec::with_capacity(checked_parameters.len());
    for symbol in checked_parameters {
        let mut matches = parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| parameter.symbol == *symbol);
        let Some((ordinal, parameter)) = matches.next() else {
            return Err(rejected(
                "service-reach dependency binder is outside its callable telescope",
            ));
        };
        if matches.next().is_some()
            || !matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Machine {
                    contract: typed_trees::data::MachineParameterContract::Nominal { .. }
                }
            )
        {
            return Err(rejected(
                "service-reach dependency requires one exact nominal machine binder",
            ));
        }
        if !matches!(
            projected_parameters
                .get(ordinal)
                .map(PackagePolicyTypeParameter::kind),
            Some(PackagePolicyTypeParameterKind::Machine(
                PackagePolicyMachineParameterContract::Nominal { .. }
            ))
        ) {
            return Err(rejected(
                "service-reach dependency lost its projected nominal requirement",
            ));
        }
        ordinals.push(u32::try_from(ordinal).map_err(|_| {
            rejected("service-reach dependency ordinal exceeds the policy vocabulary")
        })?);
    }
    ordinals.sort_unstable();
    if ordinals.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(rejected(
            "service-reach dependency repeats a nominal binder",
        ));
    }
    Ok(PackagePolicyServiceReachDependency {
        concrete,
        parameters: ordinals,
    })
}

fn rejected(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}

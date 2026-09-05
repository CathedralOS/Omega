use super::rejected;
use crate::capture::semantics::declarations::{
    top_level_requirement_identity, trait_requirement_identity,
    trait_requirement_identity_from_symbols,
};
use crate::capture::semantics::encoding::framed_identity;
use crate::record::PackagePolicyCallbackBinder;
use omega_compiler::CheckedCompilation;
use omega_provider_planning::calling_policy_plans::{
    BoundaryCallbackBinder, MaterializedBoundarySignature,
};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::data::{MachineParameterContract, TypeParameterKind};

pub(super) fn project(
    compilation: &CheckedCompilation,
    signature: &MaterializedBoundarySignature,
    binder: &BoundaryCallbackBinder,
) -> Result<PackagePolicyCallbackBinder, Vec<Diagnostic>> {
    let mut owners = Vec::new();
    for owner in compilation.traits() {
        for requirement in compilation.trait_machine_signatures(owner) {
            let parameters = compilation.state_signature_type_parameters(requirement);
            if let Some(ordinal) = parameters
                .iter()
                .position(|parameter| parameter.symbol == binder.parameter_symbol)
            {
                if compilation
                    .normalized_trait_requirement_overload_identity(owner, requirement)
                    .identity()
                    != signature.owner_requirement_identity()
                {
                    return Err(rejected("callback binder belongs to another requirement"));
                }
                owners.push((
                    trait_requirement_identity(compilation, owner, requirement)?,
                    parameters,
                    ordinal,
                ));
            }
        }
    }
    for owner in compilation.machines() {
        let parameters = compilation.machine_type_parameters(owner);
        if let Some(ordinal) = parameters
            .iter()
            .position(|parameter| parameter.symbol == binder.parameter_symbol)
        {
            if compilation
                .normalized_machine_overload_identity(owner)
                .is_none_or(|identity| {
                    identity.identity() != signature.owner_requirement_identity()
                })
            {
                return Err(rejected(
                    "callback binder belongs to another requirement machine",
                ));
            }
            owners.push((
                top_level_requirement_identity(compilation, owner)?,
                parameters,
                ordinal,
            ));
        }
    }
    let [(owner, parameters, ordinal)] = owners.as_slice() else {
        return Err(rejected(
            "callback binder has no unique exact declaring telescope",
        ));
    };
    let TypeParameterKind::Machine {
        contract:
            MachineParameterContract::Nominal {
                trait_definition,
                requirement,
            },
    } = &parameters[*ordinal].kind
    else {
        return Err(rejected(
            "callback binder is not an exact nominal machine parameter",
        ));
    };
    let machine_ordinal = parameters[..*ordinal]
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Machine { .. }))
        .count();
    if *trait_definition != binder.requirement_trait
        || *requirement != binder.requirement_machine
        || u32::try_from(machine_ordinal).ok() != Some(binder.static_machine_ordinal)
    {
        return Err(rejected(
            "callback binder changed its requirement or telescope ordinal",
        ));
    }
    let static_parameter_ordinal =
        u32::try_from(*ordinal).map_err(|_| rejected("callback parameter ordinal exceeds u32"))?;
    let mut parameter = owner.clone();
    parameter.path = framed_identity(
        "callback-static-parameter",
        &[owner.path.clone(), static_parameter_ordinal.to_string()],
    );
    Ok(PackagePolicyCallbackBinder {
        parameter,
        static_parameter_ordinal,
        static_machine_ordinal: binder.static_machine_ordinal,
        requirement: trait_requirement_identity_from_symbols(
            compilation,
            binder.requirement_trait,
            binder.requirement_machine,
            "callback policy binder",
        )?,
    })
}

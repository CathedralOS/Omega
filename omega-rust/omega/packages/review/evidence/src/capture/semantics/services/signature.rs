//! Typed source signatures are retained even without a selected calling plan.

use super::*;
use crate::capture::semantics::signatures::policy::project_type_parameters;
use crate::capture::semantics::types::review_signature_type_identity_with_binders;
use crate::record::{
    PackagePolicyServiceSignature, PackageReviewNominalIdentity,
    PackageReviewTraitRequirementParameter,
};

pub(super) fn project_declaration(
    compilation: &CheckedCompilation,
    schema: SymbolHandle,
    requirement: SymbolHandle,
    identity: &PackageReviewNominalIdentity,
    calling: Option<&crate::record::PackagePolicyCallingPlan>,
) -> Result<PackagePolicyServiceSignature, Vec<Diagnostic>> {
    let generic = compilation.traits().iter().any(|owner| {
        owner.symbol == schema && !compilation.trait_type_parameters(owner).is_empty()
    });
    if !generic {
        return project(
            compilation,
            ProviderSchemaDeclaration::BoundaryTrait(schema),
            None,
            requirement,
            identity,
            calling,
        );
    }
    if calling.is_some() {
        return Err(rejected(
            "uninstantiated generic service carries a closed calling application",
        ));
    }
    let projected = crate::capture::calling::application::signature::project_declaration(
        compilation,
        schema,
        requirement,
    )?;
    from_application(projected, identity)
}

fn from_application(
    projected: crate::capture::calling::application::signature::CallingSignatureProjection,
    identity: &PackageReviewNominalIdentity,
) -> Result<PackagePolicyServiceSignature, Vec<Diagnostic>> {
    if projected.requirement != *identity {
        return Err(rejected("service signature changes the exact requirement"));
    }
    Ok(PackagePolicyServiceSignature {
        schema_arguments: projected.boundary_arguments,
        schema_lifetime_parameter_count: projected.boundary_lifetime_parameter_count,
        requirement_arguments: projected.requirement_arguments,
        requirement_lifetime_arguments: projected.requirement_lifetime_arguments,
        requirement_lifetime_parameter_count: projected.requirement_lifetime_parameter_count,
        static_parameters: projected.static_parameters,
        parameters: projected.semantic_parameters,
        result: projected.semantic_result,
    })
}

pub(super) fn project(
    compilation: &CheckedCompilation,
    schema: ProviderSchemaDeclaration,
    provider_type: Option<SymbolHandle>,
    requirement: SymbolHandle,
    identity: &PackageReviewNominalIdentity,
    calling: Option<&crate::record::PackagePolicyCallingPlan>,
) -> Result<PackagePolicyServiceSignature, Vec<Diagnostic>> {
    if let Some(calling) = calling {
        // This value was just projected from the exact selected live calling
        // association. Reuse its semantic fields, not an old review receipt.
        return Ok(PackagePolicyServiceSignature {
            schema_arguments: calling.boundary_arguments.clone(),
            schema_lifetime_parameter_count: calling.boundary_lifetime_parameter_count,
            requirement_arguments: calling.requirement_arguments.clone(),
            requirement_lifetime_arguments: calling.requirement_lifetime_arguments.clone(),
            requirement_lifetime_parameter_count: calling.requirement_lifetime_parameter_count,
            static_parameters: calling.static_parameters.clone(),
            parameters: calling
                .semantic_parameters
                .iter()
                .map(|parameter| PackageReviewTraitRequirementParameter {
                    name: parameter.name.clone(),
                    type_identity: parameter.value_type.clone(),
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                    is_self: false,
                })
                .collect(),
            result: calling.semantic_result.clone(),
        });
    }
    if let ProviderSchemaDeclaration::BoundaryTrait(symbol) = schema {
        let arguments = calling::boundary_arguments(compilation, schema, provider_type)?;
        let projected = crate::capture::calling::application::signature::project_application(
            compilation,
            symbol,
            arguments,
            requirement,
        )?;
        return from_application(projected, identity);
    }
    let (lifetimes, parameters, values, result) = match schema {
        ProviderSchemaDeclaration::BoundaryOperator(symbol) => {
            let matches = compilation
                .operators()
                .iter()
                .chain(
                    compilation
                        .domain_definitions()
                        .iter()
                        .flat_map(|domain| compilation.domain_operators(domain)),
                )
                .filter(|operator| {
                    operator.symbol == symbol && symbol == requirement && operator.is_boundary
                })
                .collect::<Vec<_>>();
            let [operator] = matches.as_slice() else {
                return Err(rejected(
                    "service signature has no unique boundary operator",
                ));
            };
            (
                &operator.lifetime_parameters,
                compilation.operator_type_parameters(operator),
                compilation.operator_parameters(operator),
                operator.return_type,
            )
        }
        ProviderSchemaDeclaration::BoundaryRequirement(symbol) => {
            let matches = compilation
                .machines()
                .iter()
                .filter(|machine| {
                    machine.symbol == symbol
                        && symbol == requirement
                        && machine.supply_mode
                            == psi_language_semantics::MachineSupplyMode::TopLevelRequirement
                })
                .collect::<Vec<_>>();
            let [machine] = matches.as_slice() else {
                return Err(rejected(
                    "service signature has no unique top-level requirement",
                ));
            };
            let entry = compilation
                .machine_states(machine)
                .first()
                .ok_or_else(|| rejected("service requirement has no exact entry state"))?;
            (
                &machine.lifetime_parameters,
                compilation.machine_type_parameters(machine),
                compilation.state_parameters(entry),
                entry.return_type,
            )
        }
        ProviderSchemaDeclaration::BoundaryTrait(_) => unreachable!(),
    };
    if lifetimes
        .iter()
        .enumerate()
        .any(|(index, name)| lifetimes[..index].contains(name))
    {
        return Err(rejected("service requirement repeats a lifetime binder"));
    }
    let mut projected = compilation.clone();
    let source_parameters = parameters;
    let mut parameters = parameters.to_vec();
    let mut scopes = Vec::new();
    let lifetime_substitutions = lifetimes
        .iter()
        .cloned()
        .map(|name| (name.clone(), name))
        .collect::<Vec<_>>();
    crate::capture::calling::application::signature::instantiate_static_parameters(
        &mut projected,
        &mut parameters,
        &[],
        &lifetime_substitutions,
        lifetimes,
        &mut scopes,
        0,
    )?;
    let (binders, static_parameters) = project_type_parameters(
        &projected,
        compilation,
        &parameters,
        source_parameters,
        identity.path(),
        &[],
        0,
        lifetimes,
        &[],
        &scopes,
        false,
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
    )?;
    let parameters = values
        .iter()
        .filter(|parameter| !parameter.is_self)
        .map(|parameter| {
            Ok(PackageReviewTraitRequirementParameter {
                name: parameter.name.as_str().to_owned(),
                type_identity: review_signature_type_identity_with_binders(
                    compilation,
                    parameter.type_reference,
                    &binders,
                    lifetimes,
                )?,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                is_self: parameter.is_self,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    Ok(PackagePolicyServiceSignature {
        schema_arguments: Vec::new(),
        schema_lifetime_parameter_count: 0,
        requirement_arguments: Vec::new(),
        requirement_lifetime_arguments: Vec::new(),
        requirement_lifetime_parameter_count: u32::try_from(lifetimes.len())
            .map_err(|_| rejected("service lifetime count exceeds u32"))?,
        static_parameters,
        parameters,
        result: result
            .is_valid()
            .then(|| {
                review_signature_type_identity_with_binders(
                    compilation,
                    result,
                    &binders,
                    lifetimes,
                )
            })
            .transpose()?,
    })
}

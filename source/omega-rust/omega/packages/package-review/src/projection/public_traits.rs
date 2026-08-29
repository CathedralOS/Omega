use super::contracts::metadata::contracts::{
    ContractProjectionContext, project_trait_requirement_contracts,
};
use super::contracts::metadata::operations::{
    project_signature_invocation_source_locations, project_signature_operational_source_locations,
};
use super::contracts::metadata::parameters::{
    collect_callable_parameter_source_locations, collect_type_parameter_source_locations,
};
use super::contracts::metadata::service_reach::project_signature_service_reach_source_locations;
use super::contracts::metadata::source_locations::project_contract_source_locations;
use super::contracts::propositions::evidence::collect_evidence_requirements;
use super::evidence::source_locations::project_nested_declaration_source_location;
use super::exact_identity::conformances::project_conformance_bounds;
use super::exact_identity::lifetime_identities::lifetime_binder_ordinal;
use super::exact_identity::nominal_identities::{
    nominal_identity, reviewed_package_owns, trait_requirement_identity,
};
use super::exact_identity::parameter_contracts::{
    project_type_parameters, project_type_parameters_after,
};
use super::exact_identity::type_identities::review_signature_type_identity_with_binders;
use super::operational::{
    project_crash_routes, project_service_row, project_synchronous_invocations,
    project_trait_requirement_termination,
};
use crate::evidence::projection::{ProjectedNestedSourceLocation, ProjectedReviewRow};
use crate::evidence::{
    PackageReviewConformanceShape, PackageReviewConformanceSubject, PackageReviewEvidenceInterface,
    PackageReviewSourceLocationRole, PackageReviewTraitParent, PackageReviewTraitRequirement,
    PackageReviewTraitRequirementParameter, PackageReviewTraitShape,
};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_public_traits(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewTraitShape>>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for definition in compilation.traits().iter().filter(|row| row.is_public) {
        let identity = nominal_identity(compilation, definition.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let parameters = compilation.trait_type_parameters(definition);
        let (mut trait_binders, type_parameters) = project_type_parameters(
            compilation,
            parameters,
            "trait",
            &identity.path,
            &definition.lifetime_parameters,
        )?;
        trait_binders.insert(0, (definition.symbol, "trait-self".to_owned()));
        let conformance_bounds = project_conformance_bounds(
            compilation,
            &definition.conformance_bounds,
            parameters,
            &trait_binders,
            &definition.lifetime_parameters,
            "public trait",
            &identity.path,
        )?;
        let parents = compilation
            .trait_requirements(definition)
            .iter()
            .map(|parent| {
                project_trait_parent(
                    compilation,
                    parent,
                    &trait_binders,
                    &definition.lifetime_parameters,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let requirements = compilation
            .trait_machine_signatures(definition)
            .iter()
            .map(|requirement| {
                project_trait_requirement(
                    compilation,
                    definition.symbol,
                    requirement,
                    &trait_binders,
                    parameters.len(),
                    &definition.lifetime_parameters,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut nested_source_locations = Vec::new();
        collect_type_parameter_source_locations(
            compilation,
            parameters,
            &mut nested_source_locations,
        )?;
        nested_source_locations.extend(compilation.trait_requirements(definition).iter().map(
            |parent| ProjectedNestedSourceLocation {
                source_span: parent.source_span,
                role: PackageReviewSourceLocationRole::TraitParent,
            },
        ));
        for requirement in compilation.trait_machine_signatures(definition) {
            nested_source_locations.push(project_nested_declaration_source_location(
                compilation,
                requirement.symbol,
                PackageReviewSourceLocationRole::TraitRequirement,
                "public trait requirement",
            )?);
            collect_callable_parameter_source_locations(
                compilation,
                compilation.state_signature_parameters(requirement),
                "public trait requirement parameter",
                &mut nested_source_locations,
            )?;
            nested_source_locations.extend(project_contract_source_locations(
                compilation,
                compilation.state_signature_contracts(requirement),
            )?);
            nested_source_locations.extend(project_signature_invocation_source_locations(
                compilation,
                requirement,
            )?);
            nested_source_locations.extend(project_signature_service_reach_source_locations(
                compilation,
                definition.symbol,
                requirement,
            )?);
            nested_source_locations.extend(project_signature_operational_source_locations(
                compilation,
                definition.symbol,
                requirement,
            )?);
            collect_type_parameter_source_locations(
                compilation,
                compilation.state_signature_type_parameters(requirement),
                &mut nested_source_locations,
            )?;
        }
        rows.push(ProjectedReviewRow {
            row: PackageReviewTraitShape {
                identity,
                is_boundary: definition.is_boundary,
                lifetime_parameter_count: definition.lifetime_parameters.len(),
                type_parameters,
                conformance_bounds,
                parents,
                requirements,
            },
            declaration: definition.symbol,
            nested_source_locations,
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}

pub(crate) fn project_public_conformances(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewConformanceShape>>, Vec<Diagnostic>> {
    use psi_typed_trees::trait_definition::{ConformanceImplementation, ConformanceSubject};

    let mut projected = Vec::new();
    for conformance in compilation
        .conformances()
        .iter()
        .filter(|conformance| conformance.is_public)
    {
        let identity = nominal_identity(compilation, conformance.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let parameters = compilation.conformance_type_parameters(conformance);
        let (binders, type_parameters) = project_type_parameters(
            compilation,
            parameters,
            "conformance",
            &identity.path,
            &conformance.lifetime_parameters,
        )?;
        let subject = match &conformance.subject {
            ConformanceSubject::Subjectless => PackageReviewConformanceSubject::Subjectless,
            ConformanceSubject::Carrier(_) => {
                if let Some(ordinal) = parameters
                    .iter()
                    .position(|parameter| parameter.symbol == conformance.carrier_symbol)
                {
                    if !matches!(
                        parameters[ordinal].kind,
                        psi_typed_trees::data::TypeParameterKind::Type
                    ) {
                        return Err(vec![Diagnostic::error(format!(
                            "public conformance `{}` uses a non-type static parameter as its subject",
                            identity.path
                        ))]);
                    }
                    PackageReviewConformanceSubject::TypeParameter(
                        u32::try_from(ordinal).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "public conformance `{}` subject exceeds the portable review parameter range",
                                identity.path
                            ))]
                        })?,
                    )
                } else {
                    let carriers = compilation
                        .data_definitions()
                        .iter()
                        .filter(|definition| definition.symbol == conformance.carrier_symbol)
                        .collect::<Vec<_>>();
                    let [carrier] = carriers.as_slice() else {
                        return Err(vec![Diagnostic::error(format!(
                            "public conformance `{}` resolves its subject to {} data declarations; expected exactly one nominal data carrier or one telescope parameter",
                            identity.path,
                            carriers.len()
                        ))]);
                    };
                    if !carrier.is_public {
                        return Err(vec![Diagnostic::error(format!(
                            "public conformance `{}` exposes private carrier `{}`",
                            identity.path, carrier.name
                        ))]);
                    }
                    PackageReviewConformanceSubject::Nominal(nominal_identity(
                        compilation,
                        conformance.carrier_symbol,
                    )?)
                }
            }
        };
        let traits = compilation
            .traits()
            .iter()
            .filter(|definition| definition.symbol == conformance.trait_symbol)
            .collect::<Vec<_>>();
        let [trait_definition] = traits.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "public conformance `{}` resolves its trait to {} declarations; expected exactly one",
                identity.path,
                traits.len()
            ))]);
        };
        if !trait_definition.is_public {
            return Err(vec![Diagnostic::error(format!(
                "public conformance `{}` exposes private trait `{}`",
                identity.path, trait_definition.name
            ))]);
        }
        if !trait_definition.lifetime_parameters.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "public conformance `{}` selects lifetime-parameterized trait `{}` without retained lifetime arguments",
                identity.path, trait_definition.name
            ))]);
        }
        let trait_arguments = compilation
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .to_vec();
        let mut requirements = Vec::new();
        collect_evidence_requirements(
            compilation,
            conformance.trait_symbol,
            &trait_arguments,
            &binders,
            Some(&conformance.lifetime_parameters),
            &[],
            &mut Vec::new(),
            &mut requirements,
        )?;
        requirements.sort();
        requirements.dedup();
        let trait_identity = nominal_identity(compilation, conformance.trait_symbol)?;
        let interface_arguments = trait_arguments
            .iter()
            .map(|argument| {
                review_signature_type_identity_with_binders(
                    compilation,
                    *argument,
                    &binders,
                    &conformance.lifetime_parameters,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let interface = PackageReviewEvidenceInterface {
            trait_identity,
            arguments: interface_arguments,
            requirements,
        };
        let mut realized = match &conformance.implementation {
            ConformanceImplementation::AttachedRequirementMachines => {
                interface.requirements.clone()
            }
            ConformanceImplementation::Closed { rows } => Vec::with_capacity(rows.len()),
        };
        let closed_rows = match &conformance.implementation {
            ConformanceImplementation::AttachedRequirementMachines => &[][..],
            ConformanceImplementation::Closed { rows } => rows.as_slice(),
        };
        for row in closed_rows {
            if !row.realization_machine.is_valid() || !row.realization_state.is_valid() {
                return Err(vec![Diagnostic::error(format!(
                    "public conformance `{}` has an incomplete checked realization row",
                    identity.path
                ))]);
            }
            let declaring_trait = nominal_identity(compilation, row.declaring_trait)?;
            let definition = compilation
                .traits()
                .iter()
                .find(|candidate| candidate.symbol == row.declaring_trait)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "public conformance `{}` has a row with an unresolved declaring trait",
                        identity.path
                    ))]
                })?;
            let requirement = compilation
                .trait_machine_signatures(definition)
                .iter()
                .find(|candidate| candidate.symbol == row.requirement)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "public conformance `{}` has a row outside its declaring trait's requirement sequence",
                        identity.path
                    ))]
                })?;
            let requirement = trait_requirement_identity(compilation, definition, requirement)?;
            let matching = interface
                .requirements
                .iter()
                .filter(|candidate| {
                    candidate.declaring_trait == declaring_trait
                        && candidate.requirement == requirement
                })
                .collect::<Vec<_>>();
            let [matching] = matching.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "public conformance `{}` cannot assign one normalized row uniquely to its inherited evidence interface",
                    identity.path
                ))]);
            };
            realized.push((*matching).clone());
        }
        realized.sort();
        if realized.windows(2).any(|pair| pair[0] == pair[1]) || realized != interface.requirements
        {
            return Err(vec![Diagnostic::error(format!(
                "public conformance `{}` does not retain one complete normalized row for every inherited requirement",
                identity.path
            ))]);
        }
        projected.push(ProjectedReviewRow {
            row: PackageReviewConformanceShape {
                identity,
                lifetime_parameter_count: conformance.lifetime_parameters.len(),
                type_parameters,
                subject,
                interface,
            },
            declaration: conformance.symbol,
            nested_source_locations: {
                let mut locations = Vec::new();
                collect_type_parameter_source_locations(compilation, parameters, &mut locations)?;
                locations
            },
        });
    }
    projected.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(projected)
}

pub(crate) fn project_trait_parent(
    compilation: &CheckedCompilation,
    parent: &psi_typed_trees::trait_definition::TraitRequirement,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
) -> Result<PackageReviewTraitParent, Vec<Diagnostic>> {
    let matches = compilation
        .traits()
        .iter()
        .filter(|candidate| candidate.symbol == parent.symbol)
        .collect::<Vec<_>>();
    let [definition] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "package review trait parent resolves to {} declarations; expected exactly one",
            matches.len()
        ))]);
    };
    if parent.lifetime_arguments.len() != definition.lifetime_parameters.len() {
        return Err(vec![Diagnostic::error(format!(
            "package review trait parent `{}` has {} resolved lifetime arguments; expected {}",
            parent.name,
            parent.lifetime_arguments.len(),
            definition.lifetime_parameters.len(),
        ))]);
    }
    Ok(PackageReviewTraitParent {
        kind: if definition.is_boundary {
            psi_typed_trees::trait_definition::TraitCompositionKind::ServiceReach
        } else {
            psi_typed_trees::trait_definition::TraitCompositionKind::Policy
        },
        identity: nominal_identity(compilation, definition.symbol)?,
        lifetime_arguments: parent
            .lifetime_arguments
            .iter()
            .map(|argument| lifetime_binder_ordinal(argument, lifetime_binders, "trait parent"))
            .collect::<Result<Vec<_>, _>>()?,
        arguments: compilation
            .type_reference_table
            .type_reference_handles(parent.arguments)
            .iter()
            .map(|argument| {
                review_signature_type_identity_with_binders(
                    compilation,
                    *argument,
                    binders,
                    lifetime_binders,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub(crate) fn project_trait_requirement(
    compilation: &CheckedCompilation,
    trait_symbol: SymbolHandle,
    requirement: &psi_typed_trees::signature::StateSignature,
    trait_binders: &[(SymbolHandle, String)],
    trait_parameter_count: usize,
    trait_lifetime_parameters: &[psi_typed_trees::name::Identifier],
) -> Result<PackageReviewTraitRequirement, Vec<Diagnostic>> {
    let owner = compilation
        .traits()
        .iter()
        .find(|definition| definition.symbol == trait_symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "package review trait requirement has no exact declaring trait",
            )]
        })?;
    let identity = trait_requirement_identity(compilation, owner, requirement)?;
    let parameters = compilation.state_signature_type_parameters(requirement);
    let mut lifetime_binders = trait_lifetime_parameters.to_vec();
    lifetime_binders.extend(requirement.lifetime_parameters.iter().cloned());
    let (binders, type_parameters) = project_type_parameters_after(
        compilation,
        parameters,
        "trait requirement",
        &identity.path,
        trait_binders,
        trait_parameter_count,
        &lifetime_binders,
        0,
    )?;
    // Preserve the more specific public-progress validation before the same
    // membership facts enter the general contract lane.
    let termination = project_trait_requirement_termination(compilation, requirement)?;
    let contract_parameters = compilation.state_signature_parameters(requirement);
    let contract_context = ContractProjectionContext {
        subject_kind: "public trait requirement",
        subject_name: &identity.path,
        owner: psi_checked_trees::ContractProofFactOwner::StateSignature {
            owner_symbol: trait_symbol,
            state_symbol: requirement.symbol,
        },
        point: psi_facts::ProgramPoint::State {
            machine_symbol: trait_symbol,
            state_symbol: requirement.symbol,
        },
        parameters: contract_parameters,
        domain_symbol: None,
        data_symbol: None,
        lifetime_binders: &lifetime_binders,
    };
    let contracts =
        project_trait_requirement_contracts(compilation, requirement, &contract_context, &binders)?;
    let mut crash_capsules = compilation
        .facts
        .contract_plans
        .crash_capsules
        .iter()
        .filter(|capsule| {
            capsule.target_machine() == trait_symbol && capsule.target_state() == requirement.symbol
        });
    let crash_capsule = crash_capsules.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "public trait requirement `{}` has no exact checked crash capsule",
            identity.path
        ))]
    })?;
    if crash_capsules.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "public trait requirement `{}` has duplicate checked crash capsules",
            identity.path
        ))]);
    }
    let published_crash = project_crash_routes(crash_capsule.published_buckets());
    Ok(PackageReviewTraitRequirement {
        identity,
        spelling: requirement.spelling,
        has_default_realization: requirement.is_default,
        lifetime_parameter_count: requirement.lifetime_parameters.len(),
        type_parameters,
        parameters: compilation
            .state_signature_parameters(requirement)
            .iter()
            .map(|parameter| {
                Ok(PackageReviewTraitRequirementParameter {
                    name: parameter.name.as_str().to_owned(),
                    type_identity: review_signature_type_identity_with_binders(
                        compilation,
                        parameter.type_reference,
                        &binders,
                        &lifetime_binders,
                    )?,
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                    is_self: parameter.is_self,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?,
        return_type: review_signature_type_identity_with_binders(
            compilation,
            requirement.return_type,
            &binders,
            &lifetime_binders,
        )?,
        contracts,
        published_crash,
        service_reach: project_service_row(compilation, requirement.service_reach_row)?,
        service_reach_is_installation_bound: requirement.service_reach_is_installation_bound,
        synchronous_invocations: project_synchronous_invocations(
            compilation,
            &psi_effects::declared_signature_invocations(compilation, requirement),
        )?,
        suspends: requirement.suspends,
        blocks: requirement.blocks,
        termination,
    })
}

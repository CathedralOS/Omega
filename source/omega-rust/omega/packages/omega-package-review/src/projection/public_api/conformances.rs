use super::super::checked_semantics::declarations::{
    nominal_identity, reviewed_package_owns, trait_requirement_identity,
};
use super::super::checked_semantics::signatures::parameters::project_type_parameters;
use super::super::checked_semantics::types::review_signature_type_identity_with_binders;
use super::super::contracts::checked::parameters::collect_type_parameter_source_locations;
use super::super::contracts::propositions::evidence::collect_evidence_requirements;
use crate::evidence::projection::ProjectedReviewRow;
use crate::evidence::{
    PackageReviewConformanceShape, PackageReviewConformanceSubject, PackageReviewEvidenceInterface,
};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::trait_definition::{ConformanceImplementation, ConformanceSubject};

pub(crate) fn project_public_conformances(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewConformanceShape>>, Vec<Diagnostic>> {
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

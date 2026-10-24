use super::super::contracts::propositions::evidence::collect_evidence_requirements;
use super::super::semantics::declarations::{
    is_product_scope_instance, nominal_identity, reviewed_package_owns, trait_requirement_identity,
};
use super::super::semantics::signatures::parameters::project_type_parameters;
use super::super::semantics::types::review_signature_type_identity_with_binders;
use crate::capture::PackageReviewInput;
use crate::capture::source::ProjectedReviewRow;
use crate::record::{PackageReviewConformanceShape, PackageReviewEvidenceInterface};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use typed_trees::trait_definition::Conformance;

pub(crate) fn project_public_conformances<'a>(
    compilation: &'a PackageReviewInput<'_>,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<'a, PackageReviewConformanceShape, Conformance>>, Vec<Diagnostic>>
{
    let mut projected = Vec::new();
    for conformance in compilation.conformances().iter().filter(|conformance| {
        conformance.is_public
            && is_product_scope_instance(&compilation.typed.symbols, conformance.symbol)
    }) {
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
        if conformance.trait_lifetime_arguments.len() != trait_definition.lifetime_parameters.len()
        {
            return Err(vec![Diagnostic::error(format!(
                "public conformance `{}` retains {} target-trait lifetime arguments for trait `{}`, expected {}",
                identity.path,
                conformance.trait_lifetime_arguments.len(),
                trait_definition.name,
                trait_definition.lifetime_parameters.len(),
            ))]);
        }
        let target_trait_lifetime_arguments = conformance
            .trait_lifetime_arguments
            .iter()
            .map(|ordinal| {
                usize::try_from(*ordinal)
                    .ok()
                    .and_then(|ordinal| conformance.lifetime_parameters.get(ordinal))
                    .cloned()
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "public conformance `{}` retains a target-trait lifetime outside its checked telescope",
                            identity.path
                        ))]
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let trait_arguments = compilation
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .to_vec();
        let mut requirements = Vec::new();
        collect_evidence_requirements(
            compilation,
            conformance.trait_symbol,
            &trait_arguments,
            &target_trait_lifetime_arguments,
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
            lifetime_arguments: conformance.trait_lifetime_arguments.clone(),
            arguments: interface_arguments,
            requirements,
        };
        let mut realized = Vec::with_capacity(conformance.rows.len());
        for row in &conformance.rows {
            if !row.realization_machine.is_valid() || !row.realization_state.is_valid() {
                return Err(vec![Diagnostic::error(format!(
                    "public conformance `{}` has an incomplete checked realization row",
                    identity.path
                ))]);
            }
            let declaring_trait = nominal_identity(compilation, row.declaring_trait)?;
            let definition = compilation
                .trait_definition_by_symbol(row.declaring_trait)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "public conformance `{}` has a row with an unresolved declaring trait",
                        identity.path
                    ))]
                })?;
            let requirement = compilation
                .trait_machine_signature_by_symbol(definition, row.requirement)
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
                interface,
            },
            declaration: conformance,
        });
    }
    projected.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(projected)
}

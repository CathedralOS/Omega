use super::aliases::{project_domain_alias_expansion, project_domain_establishment_route};
use super::facts::project_domain_predicate_facts;
use crate::evidence::{
    PackageReviewDomainClassification, PackageReviewDomainSemanticRole, PackageReviewDomainShape,
    PackageReviewNominalIdentity,
};
use crate::projection::contracts::parameters::collect_type_parameter_source_locations;
use crate::projection::contracts::source_locations::project_required_proof_fact_source_locations;
use crate::projection::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::projection::semantics::signatures::parameters::project_type_parameters;
use crate::projection::semantics::types::review_type_identity_with_binders;
use crate::projection::source::ProjectedReviewRow;
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(crate) fn project_public_domains(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewDomainShape>>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for definition in compilation
        .domain_definitions()
        .iter()
        .filter(|row| row.is_public)
    {
        let identity = nominal_identity(compilation, definition.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let parameters = compilation.domain_type_parameters(definition);
        let (binders, type_parameters) =
            project_type_parameters(compilation, parameters, "domain", &identity.path, &[])?;
        let predicate_facts =
            project_domain_predicate_facts(compilation, definition, &identity, &binders)?;
        let alias_expansion = definition
            .alias
            .as_ref()
            .map(|_| project_domain_alias_expansion(compilation, definition.symbol))
            .transpose()?;
        let classification = definition
            .classification
            .map(|classification| match classification {
                psi_language_semantics::DomainClassification::ProgressProfile => {
                    PackageReviewDomainClassification::ProgressProfile
                }
            });
        let mut establishment_routes = definition
            .establishment_routes
            .iter()
            .map(|route| project_domain_establishment_route(compilation, *route))
            .collect::<Result<Vec<_>, _>>()?;
        establishment_routes.sort();
        establishment_routes.dedup();
        let semantic_roles = project_domain_semantic_roles(definition, &identity)?;
        rows.push(ProjectedReviewRow {
            row: PackageReviewDomainShape {
                identity,
                type_parameters,
                target_type: review_type_identity_with_binders(
                    compilation,
                    definition.target_type,
                    &binders,
                )?,
                index_arguments: definition
                    .index_arguments
                    .iter()
                    .map(|argument| {
                        review_type_identity_with_binders(compilation, *argument, &binders)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                predicate_body: definition.predicate_body,
                predicate_facts,
                alias_expansion,
                classification,
                semantic_roles,
                establishment_routes,
            },
            declaration: definition.symbol,
            nested_source_locations: {
                let mut locations = Vec::new();
                collect_type_parameter_source_locations(compilation, parameters, &mut locations)?;
                locations.extend(project_required_proof_fact_source_locations(
                    compilation,
                    definition.facts,
                    "public domain predicate",
                )?);
                locations
            },
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}

pub(crate) fn project_domain_semantic_roles(
    definition: &psi_typed_trees::domain::DomainDefinition,
    identity: &PackageReviewNominalIdentity,
) -> Result<Vec<PackageReviewDomainSemanticRole>, Vec<Diagnostic>> {
    let mut roles = Vec::new();
    for (role, semantic_identity) in [
        (
            PackageReviewDomainSemanticRole::DenotationDimension,
            definition.semantic_roles.denotation_dimension,
        ),
        (
            PackageReviewDomainSemanticRole::ArithmeticPolicy,
            definition.semantic_roles.arithmetic_policy,
        ),
    ] {
        let Some(semantic_identity) = semantic_identity else {
            continue;
        };
        if semantic_identity != definition.semantic_id {
            return Err(vec![Diagnostic::error(format!(
                "public domain `{}` semantic role does not name its exact typed semantic identity",
                identity.path
            ))]);
        }
        roles.push(role);
    }
    Ok(roles)
}

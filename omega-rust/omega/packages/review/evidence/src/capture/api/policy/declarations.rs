//! Non-trait public declaration policy with source-owned static telescopes.

use super::{signatures, values};
use crate::capture::api;
use crate::capture::semantics::facts::exactly_one;
use crate::record::*;
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(super) fn data(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<PackagePolicyDataShape>, Vec<Diagnostic>> {
    api::data::projection::project_public_data(compilation, package)?
        .into_iter()
        .map(|projected| {
            let source = exactly_one(
                compilation
                    .data_definitions()
                    .iter()
                    .filter(|definition| definition.symbol == projected.declaration),
                "public data policy",
                "data declaration",
            )?;
            let row = projected.row;
            let (_, type_parameters) = signatures::parameters(
                compilation,
                compilation.data_type_parameters(source),
                &row.identity.path,
                &[],
                0,
                &source.lifetime_parameters,
            )?;
            Ok(PackagePolicyDataShape {
                identity: row.identity,
                kind: row.kind,
                supply: row.supply,
                lifetime_parameter_count: row.lifetime_parameter_count,
                type_parameters,
                properties: row.properties,
                zero_gated: row.zero_gated,
                invariants: row.invariants,
                retired_identities: row.retired_identities,
                members: row.members,
            })
        })
        .collect()
}

pub(super) fn domains(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<PackagePolicyDomainShape>, Vec<Diagnostic>> {
    api::domains::projection::project_public_domains(compilation, package)?
        .into_iter()
        .map(|projected| {
            let source = exactly_one(
                compilation
                    .domain_definitions()
                    .iter()
                    .filter(|definition| definition.symbol == projected.declaration),
                "public domain policy",
                "domain declaration",
            )?;
            let row = projected.row;
            let (_, type_parameters) = signatures::parameters(
                compilation,
                compilation.domain_type_parameters(source),
                &row.identity.path,
                &[],
                0,
                &[],
            )?;
            Ok(PackagePolicyDomainShape {
                identity: row.identity,
                type_parameters,
                target_type: row.target_type,
                index_arguments: row.index_arguments,
                predicate_body: row.predicate_body,
                predicate_facts: row.predicate_facts,
                alias_expansion: row.alias_expansion,
                classification: row.classification,
                semantic_roles: row.semantic_roles,
                establishment_routes: row.establishment_routes,
            })
        })
        .collect()
}

pub(crate) fn conformances(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<PackagePolicyConformanceShape>, Vec<Diagnostic>> {
    api::conformances::project_public_conformances(compilation, package)?
        .into_iter()
        .map(|projected| {
            let source = exactly_one(
                compilation
                    .conformances()
                    .iter()
                    .filter(|definition| definition.symbol == projected.declaration),
                "public conformance policy",
                "conformance declaration",
            )?;
            let row = projected.row;
            let (_, type_parameters) = signatures::parameters(
                compilation,
                compilation.conformance_type_parameters(source),
                &row.identity.path,
                &[],
                0,
                &source.lifetime_parameters,
            )?;
            Ok(PackagePolicyConformanceShape {
                identity: row.identity,
                lifetime_parameter_count: row.lifetime_parameter_count,
                type_parameters,
                subject: row.subject,
                interface: row.interface,
            })
        })
        .collect()
}

pub(super) fn operators(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<PackagePolicyOperatorShape>, Vec<Diagnostic>> {
    api::operators::project_public_operators(compilation, package)?
        .into_iter()
        .map(|projected| {
            let source = exactly_one(
                compilation
                    .operators()
                    .iter()
                    .chain(
                        compilation
                            .domain_definitions()
                            .iter()
                            .flat_map(|domain| compilation.domain_operators(domain)),
                    )
                    .filter(|definition| definition.symbol == projected.declaration),
                "public operator policy",
                "operator declaration",
            )?;
            let row = projected.row;
            let (_, type_parameters) = signatures::parameters(
                compilation,
                compilation.operator_type_parameters(source),
                row.coordinate.identity().path(),
                &[],
                0,
                &source.lifetime_parameters,
            )?;
            Ok(PackagePolicyOperatorShape {
                coordinate: row.coordinate,
                is_boundary: row.is_boundary,
                spelling: row.spelling,
                lifetime_parameter_count: row.lifetime_parameter_count,
                type_parameters,
                parameters: row.parameters,
                return_type: source.return_type.is_valid().then_some(row.return_type),
                contracts: row.contracts,
                published_crash: values::crashes(row.published_crash)?,
            })
        })
        .collect()
}

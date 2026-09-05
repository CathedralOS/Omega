//! Exact public trait requirements with policy-owned behavioral signatures.

use super::{rejected, signatures, values};
use crate::capture::api;
use crate::capture::contracts::facts::{
    ContractProjectionContext, project_trait_requirement_contracts,
};
use crate::capture::semantics::facts::exactly_one;
use crate::capture::semantics::signatures::policy_crashes;
use crate::capture::semantics::types::review_signature_type_identity_with_binders;
use crate::record::*;
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(super) fn project(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<PackagePolicyTraitShape>, Vec<Diagnostic>> {
    api::traits::project_public_traits(compilation, package)?
        .into_iter()
        .map(|projected| {
            let source = exactly_one(
                compilation
                    .traits()
                    .iter()
                    .filter(|definition| definition.symbol == projected.declaration),
                "public trait policy",
                "trait declaration",
            )?;
            let row = projected.row;
            let parameters = compilation.trait_type_parameters(source);
            let (mut binders, type_parameters) = signatures::parameters(
                compilation,
                parameters,
                &row.identity.path,
                &[],
                0,
                &source.lifetime_parameters,
            )?;
            binders.insert(0, (source.symbol, "trait-self".to_owned()));
            let source_requirements = compilation.trait_machine_signatures(source);
            if source_requirements.len() != row.requirements.len() {
                return Err(rejected(
                    "trait requirement roster loses its source declaration",
                ));
            }
            let requirements = source_requirements
                .iter()
                .zip(row.requirements)
                .map(|(requirement, value)| {
                    project_requirement(
                        compilation,
                        source.symbol,
                        requirement,
                        value,
                        &binders,
                        parameters.len(),
                        &source.lifetime_parameters,
                    )
                })
                .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
            Ok(PackagePolicyTraitShape {
                identity: row.identity,
                is_boundary: row.is_boundary,
                lifetime_parameter_count: row.lifetime_parameter_count,
                type_parameters,
                conformance_bounds: row.conformance_bounds,
                parents: row.parents,
                requirements,
            })
        })
        .collect()
}

fn project_requirement(
    compilation: &CheckedCompilation,
    owner: SymbolHandle,
    source: &psi_typed_trees::signature::StateSignature,
    row: PackageReviewTraitRequirement,
    outer_binders: &[(SymbolHandle, String)],
    offset: usize,
    outer_lifetimes: &[psi_typed_trees::name::Identifier],
) -> Result<PackagePolicyTraitRequirement, Vec<Diagnostic>> {
    let (prepared, binders, type_parameters) = signatures::requirement(
        compilation,
        owner,
        source,
        &row.identity.path,
        outer_binders,
        offset,
        outer_lifetimes,
    )?;
    let context = ContractProjectionContext {
        subject_kind: "public trait policy requirement",
        subject_name: &row.identity.path,
        owner: psi_checked_trees::ContractProofFactOwner::StateSignature { owner_symbol: owner, state_symbol: source.symbol },
        point: psi_facts::ProgramPoint::State { machine_symbol: owner, state_symbol: source.symbol },
        parameters: compilation.state_signature_parameters(source),
        domain_symbol: None, data_symbol: None,
        lifetime_binders: &prepared.lifetimes, lifetime_substitutions: &prepared.scopes[0].lifetime_substitutions,
        selection_exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
    };
    let published_crash = values::crashes(policy_crashes::project(
        compilation,
        owner,
        source,
        &context,
        &binders,
    )?)?;
    let contracts = project_trait_requirement_contracts(compilation, source, &context, &binders)?;
    let parameters = prepared
        .compilation
        .state_signature_parameters(&prepared.signature)
        .iter()
        .map(|parameter| {
            Ok(PackageReviewTraitRequirementParameter {
                name: parameter.name.as_str().to_owned(),
                type_identity: review_signature_type_identity_with_binders(
                    &prepared.compilation,
                    parameter.type_reference,
                    &binders,
                    &prepared.lifetimes,
                )?,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                is_self: parameter.is_self,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let return_type = source
        .return_type
        .is_valid()
        .then(|| {
            review_signature_type_identity_with_binders(
                &prepared.compilation,
                prepared.signature.return_type,
                &binders,
                &prepared.lifetimes,
            )
        })
        .transpose()?;
    Ok(PackagePolicyTraitRequirement {
        identity: row.identity,
        spelling: row.spelling,
        has_default_realization: row.has_default_realization,
        lifetime_parameter_count: row.lifetime_parameter_count,
        type_parameters,
        parameters,
        return_type,
        contracts,
        published_crash,
        service_reach: row.service_reach,
        service_reach_is_installation_bound: row.service_reach_is_installation_bound,
        synchronous_invocations: row.synchronous_invocations,
        suspends: row.suspends,
        blocks: row.blocks,
        termination: values::termination(
            compilation,
            &source.termination_guarantee,
            row.termination,
        )?,
    })
}

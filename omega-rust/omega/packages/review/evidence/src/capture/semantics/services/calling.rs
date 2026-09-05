//! Report coordinates are only a checked join to the complete calling value.

use super::*;
use crate::record::PackagePolicyCallingPlan;

pub(super) fn project(
    compilation: &CheckedCompilation,
    schema: ProviderSchemaDeclaration,
    provider_type: Option<SymbolHandle>,
    requirement: SymbolHandle,
    method: &ServiceMethod,
) -> Result<Option<PackagePolicyCallingPlan>, Vec<Diagnostic>> {
    let ProviderSchemaDeclaration::BoundaryTrait(schema_symbol) = schema else {
        return if method.calling_plan_report_fingerprint.is_none()
            && method.calling_plan_commitment.is_none()
        {
            Ok(None)
        } else {
            Err(rejected(
                "non-trait requirement carries a detached calling application",
            ))
        };
    };
    let arguments = boundary_arguments(compilation, schema, provider_type)?;
    let retained = compilation.boundary_calling_plan_identity_for_arguments(
        schema_symbol,
        arguments,
        requirement,
    );
    if retained.map(|identity| identity.report_fingerprint)
        != method.calling_plan_report_fingerprint
        || retained.map(|identity| identity.commitment) != method.calling_plan_commitment
    {
        return Err(rejected(
            "service method changes its exact typed calling application",
        ));
    }
    let candidates = compilation
        .boundary_calling_plan_realizations()
        .iter()
        .filter(|candidate| {
            candidate.boundary_trait == schema_symbol
                && candidate.requirement_machine == requirement
                && candidate.boundary_arguments == arguments
        })
        .collect::<Vec<_>>();
    let Some(retained) = retained else {
        return if candidates.is_empty() {
            Ok(None)
        } else {
            Err(rejected(
                "calling realization has no typed service application",
            ))
        };
    };
    let Some(candidate) = candidates.first().copied() else {
        return Err(rejected(
            "service method has no exact target-closed calling realization",
        ));
    };
    if candidates.iter().any(|other| *other != candidate)
        || candidate.report_fingerprint != retained.report_fingerprint
        || candidate.commitment != retained.commitment
    {
        return Err(rejected(
            "service method has ambiguous or stale calling realization",
        ));
    }
    crate::capture::calling::project_checked_calling_policy(compilation, candidate).map(Some)
}

pub(super) fn boundary_arguments(
    compilation: &CheckedCompilation,
    schema: ProviderSchemaDeclaration,
    provider_type: Option<SymbolHandle>,
) -> Result<&[typed_trees::types::TypeReferenceHandle], Vec<Diagnostic>> {
    let ProviderSchemaDeclaration::BoundaryTrait(schema_symbol) = schema else {
        return Ok(&[]);
    };
    let mut applications = compilation
        .conformances()
        .iter()
        .filter(|candidate| {
            candidate.trait_symbol == schema_symbol
                && Some(candidate.carrier_symbol) == provider_type
        })
        .map(|candidate| {
            compilation
                .type_reference_table
                .type_reference_handles(candidate.arguments)
        });
    let arguments = applications.next().unwrap_or(&[]);
    if applications.any(|candidate| candidate != arguments) {
        return Err(rejected("provider has ambiguous exact boundary arguments"));
    }
    Ok(arguments)
}

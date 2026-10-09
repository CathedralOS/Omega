//! Report coordinates are only a checked join to the complete calling value.
use crate::capture::PackageReviewInput;
use crate::capture::semantics::services::rejected;
use crate::record::PackagePolicyCallingPlan;
use diagnostics::Diagnostic;
use effects::provider_plan::ServiceMethod;
use provider_planning::ProviderSchemaDeclaration;
use symbols::SymbolHandle;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
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
    let candidates = compilation
        .custody
        .providers()
        .boundary_calling_plan_realizations
        .iter()
        .filter(|candidate| {
            candidate.boundary_trait == schema_symbol
                && candidate.requirement_machine == requirement
                && candidate.boundary_arguments == arguments
        })
        .collect::<Vec<_>>();
    if candidates.is_empty()
        && method.calling_plan_report_fingerprint.is_none()
        && method.calling_plan_commitment.is_none()
    {
        return Ok(None);
    }
    let Some(candidate) = candidates.first().copied() else {
        return Err(rejected(
            "service method has no exact target-closed calling realization",
        ));
    };
    if candidates.iter().any(|other| *other != candidate)
        || Some(candidate.report_fingerprint) != method.calling_plan_report_fingerprint
        || Some(candidate.commitment) != method.calling_plan_commitment
    {
        return Err(rejected(
            "service method has ambiguous or stale calling realization",
        ));
    }
    crate::capture::calling::project_checked_calling_policy(compilation, candidate).map(Some)
}

pub(super) fn boundary_arguments<'a>(
    compilation: &'a PackageReviewInput<'_>,
    schema: ProviderSchemaDeclaration,
    provider_type: Option<SymbolHandle>,
) -> Result<&'a [typed_trees::types::TypeReferenceHandle], Vec<Diagnostic>> {
    let ProviderSchemaDeclaration::BoundaryTrait(schema_symbol) = schema else {
        return Ok(&[]);
    };
    let mut applications = compilation
        .conformances()
        .iter()
        .filter(|candidate| {
            candidate.trait_symbol == schema_symbol && Some(candidate.symbol) == provider_type
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

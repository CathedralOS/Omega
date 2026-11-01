//! Report coordinates are only a checked join to the complete calling value.
use crate::capture::PackageReviewInput;
use crate::capture::semantics::services::rejected;
use crate::record::PackagePolicyCallingPlan;
use diagnostics::Diagnostic;
use effects::provider_plan::ServiceSchema;
use provider_planning::ProviderSchemaDeclaration;
use symbols::SymbolHandle;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    schema: ProviderSchemaDeclaration,
    provider_type: Option<SymbolHandle>,
    requirement: SymbolHandle,
    service_schema: &ServiceSchema,
) -> Result<Option<PackagePolicyCallingPlan>, Vec<Diagnostic>> {
    // Service-method review describes this exact prepared declaration. Closed
    // local applications are reviewed individually in the calling-policy roster.
    let census = compilation.typed.boundary_signatures();
    let abi = census
        .abi_for_boundary(requirement)
        .and_then(|coordinate| coordinate.as_ref().ok())
        .and_then(|coordinate| {
            census
                .abi_signatures()
                .iter()
                .find(|row| row.id == *coordinate)
        })
        .map(|row| semantic_vocabulary::CallingAbiIdentity {
            requirement_identity: row.requirement_identity.clone(),
            declaring_application: row.declaring_application.clone(),
            operands: row.operands.clone(),
        });
    let calling = abi
        .as_ref()
        .and_then(|abi| service_schema.calling_application(abi));
    let ProviderSchemaDeclaration::BoundaryTrait(schema_symbol) = schema else {
        return if calling.map(|plan| plan.report_fingerprint).is_none()
            && calling.map(|plan| plan.commitment).is_none()
        {
            Ok(None)
        } else {
            Err(rejected(
                "non-trait requirement carries a detached calling application",
            ))
        };
    };
    let arguments = boundary_arguments(compilation, schema, provider_type)?;
    let argument_owner = argument_owner(compilation, schema_symbol, provider_type)?;
    let candidates = compilation
        .custody
        .providers()
        .boundary_calling_plan_realizations
        .iter()
        .filter(|candidate| {
            candidate.boundary_trait == schema_symbol
                && abi.as_ref() == Some(candidate.materialized_signature().abi())
                && candidate.requirement_machine == requirement
                && candidate.boundary_arguments == arguments
                && candidate.argument_owner() == argument_owner
        })
        .collect::<Vec<_>>();
    if candidates.is_empty()
        && calling.map(|plan| plan.report_fingerprint).is_none()
        && calling.map(|plan| plan.commitment).is_none()
    {
        return Ok(None);
    }
    let Some(candidate) = candidates.first().copied() else {
        return Err(rejected(
            "service method has no exact target-closed calling realization",
        ));
    };
    if candidates.iter().any(|other| *other != candidate)
        || Some(candidate.report_fingerprint) != calling.map(|plan| plan.report_fingerprint)
        || Some(candidate.commitment) != calling.map(|plan| plan.commitment)
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

/// The ABI producer retains generic operands under their supplying conformance.
/// A nongeneric boundary has no supplied type operands and uses its declaration.
pub(super) fn argument_owner(
    compilation: &PackageReviewInput<'_>,
    schema: SymbolHandle,
    provider: Option<SymbolHandle>,
) -> Result<SymbolHandle, Vec<Diagnostic>> {
    let definition = compilation
        .trait_definition_by_symbol(schema)
        .ok_or_else(|| rejected("calling application has no exact boundary declaration"))?;
    if compilation.trait_type_parameters(definition).is_empty() {
        return Ok(schema);
    }
    let conformance = provider
        .and_then(|symbol| compilation.conformance_by_symbol(symbol))
        .filter(|conformance| conformance.trait_symbol == schema)
        .ok_or_else(|| rejected("calling application has no exact supplying conformance"))?;
    Ok(conformance.symbol)
}

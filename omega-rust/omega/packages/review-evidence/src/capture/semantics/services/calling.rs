//! Report coordinates are only a checked join to the complete calling value.
use crate::capture::PackageReviewInput;
use crate::capture::semantics::services::rejected;
use crate::capture::semantics::signatures::SignatureProjection;
use crate::record::PackagePolicyCallingPlan;
use diagnostics::Diagnostic;
use effects::provider_plan::ServiceSchema;
use provider_planning::ProviderSchemaDeclaration;
use symbols::SymbolHandle;

pub(super) fn project(
    projection: &mut SignatureProjection<'_>,
    schema: ProviderSchemaDeclaration,
    provider_type: Option<SymbolHandle>,
    requirement: SymbolHandle,
    service_schema: &ServiceSchema,
    requirement_coordinate: effects::provider_plan::ProviderIdentityId,
) -> Result<Vec<PackagePolicyCallingPlan>, Vec<Diagnostic>> {
    let input = projection.input();
    let compilation = &input;
    let applications = service_schema.calling_for_requirement(requirement_coordinate);
    let ProviderSchemaDeclaration::BoundaryTrait(schema_symbol) = schema else {
        return if applications.is_empty() {
            Ok(Vec::new())
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
                && candidate.requirement_machine == requirement
                && candidate.boundary_arguments == arguments
                && candidate.argument_owner() == argument_owner
        })
        .collect::<Vec<_>>();
    if candidates.iter().any(|candidate| {
        !applications
            .iter()
            .any(|application| &application.abi == candidate.materialized_signature().abi())
    }) {
        return Err(rejected(
            "service method omitted a source-owned exact calling application",
        ));
    }
    applications
        .into_iter()
        .map(|application| {
            let mut matching = candidates
                .iter()
                .copied()
                .filter(|candidate| candidate.materialized_signature().abi() == &application.abi);
            let candidate = matching.next().ok_or_else(|| {
                rejected("service method lost its source-qualified exact calling application")
            })?;
            if matching.any(|other| other != candidate)
                || candidate.report_fingerprint != application.plan.report_fingerprint
                || candidate.commitment != application.plan.commitment
            {
                return Err(rejected(
                    "service method has ambiguous or stale exact calling application",
                ));
            }
            crate::capture::calling::project_checked_calling_policy(projection, candidate)
        })
        .collect()
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

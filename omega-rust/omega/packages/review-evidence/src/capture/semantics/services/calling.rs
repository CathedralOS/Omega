//! Report coordinates are only a checked join to the complete calling value.
use super::application::ApplicationSource;
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
    application: ApplicationSource<'_>,
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
    let (arguments, argument_owner, closed_requirement) = match application {
        ApplicationSource::Provider(provider_type) => (
            boundary_arguments(compilation, schema, provider_type)?,
            argument_owner(compilation, schema_symbol, provider_type)?,
            None,
        ),
        ApplicationSource::Requirement(source) => {
            if let Some(closed) = source.closed_requirement {
                let closed = compilation
                    .closed_requirement_application(closed)
                    .filter(|closed| {
                        closed.template == requirement && closed.declaring_trait == schema_symbol
                    })
                    .ok_or_else(|| {
                        rejected("native calling application lost its exact closed requirement")
                    })?;
                let owner = compilation
                    .expression_table
                    .static_machine_application(closed.application)
                    .lexical_owner;
                (closed.trait_arguments, owner, Some(closed.signature.symbol))
            } else if compilation
                .trait_definition_by_symbol(schema_symbol)
                .is_some_and(|owner| compilation.trait_type_parameters(owner).is_empty())
            {
                (&[][..], schema_symbol, None)
            } else if method.calling_plan_report_fingerprint.is_none()
                && method.calling_plan_commitment.is_none()
            {
                // The exact open or outer-only source is independently projected
                // below. It cannot borrow a physical application's identity.
                return Ok(None);
            } else {
                return Err(rejected(
                    "native calling application omits its complete source telescope",
                ));
            }
        }
    };
    let candidates = compilation
        .custody
        .providers()
        .boundary_calling_plan_realizations
        .iter()
        .filter(|candidate| {
            candidate.boundary_trait == schema_symbol
                && (candidate.requirement_machine == requirement
                    || Some(candidate.requirement_machine) == closed_requirement)
                && candidate.boundary_arguments == arguments
                && candidate.argument_owner() == argument_owner
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
    match application {
        ApplicationSource::Provider(_) => {
            crate::capture::calling::project_checked_calling_policy(compilation, candidate)
                .map(Some)
        }
        ApplicationSource::Requirement(source) => {
            let semantic =
                crate::capture::calling::application::signature::project_requirement_application(
                    &mut super::SignatureProjection::new(*compilation),
                    schema_symbol,
                    requirement,
                    source,
                )?;
            crate::capture::calling::application::project_checked_calling_policy_with_source(
                compilation,
                candidate,
                semantic,
            )
            .map(Some)
        }
    }
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

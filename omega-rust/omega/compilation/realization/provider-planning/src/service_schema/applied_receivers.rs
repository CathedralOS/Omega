//! Omega interns portable receiver content in its own schema table.
//!
//! Psi owns source scope projection and complete requirement rosters. Native
//! schemas retain their own table; joins compare issued content, including
//! receiver regions, rather than process-local table identifiers.

pub(super) use build_time_evaluation::provider_declarations::receivers::collect_scopes;
use checked_trees::typed_trees::trait_definition::AppliedTraitScope;
use effects::provider_plan::{ProviderIdentityTable, ServiceReceiverApplication};

pub(super) fn receiver_applications(
    program: &checked_trees::typed_trees::TypedTrees,
    scopes: &[AppliedTraitScope<'_>],
    identities: &mut ProviderIdentityTable,
) -> Option<Vec<ServiceReceiverApplication>> {
    let applications =
        build_time_evaluation::provider_declarations::receivers::receiver_applications(
            program, scopes,
        )?;
    Some(
        applications
            .into_iter()
            .map(|application| ServiceReceiverApplication {
                trait_name: identities.intern(&application.declaration),
                trait_package_identity: application.package,
                argument_identities: application
                    .arguments
                    .iter()
                    .map(|identity| identities.intern(identity))
                    .collect(),
                lifetime_argument_identities: application
                    .lifetimes
                    .iter()
                    .map(|identity| identities.intern(identity))
                    .collect(),
                requirement_identities: application
                    .requirements
                    .iter()
                    .map(|identity| identities.intern(identity))
                    .collect(),
            })
            .collect(),
    )
}

pub(crate) fn receiver_identity(
    table: &ProviderIdentityTable,
    application: &ServiceReceiverApplication,
) -> Option<build_time_evaluation::provider_declarations::receivers::ProviderSelectionReceiver> {
    let text = |identity| {
        let content = table.text(identity);
        (!content.is_empty()).then(|| content.to_owned())
    };
    Some(
        build_time_evaluation::provider_declarations::receivers::ProviderSelectionReceiver {
            package: application.trait_package_identity,
            declaration: text(application.trait_name)?,
            arguments: application
                .argument_identities
                .iter()
                .map(|identity| text(*identity))
                .collect::<Option<Vec<_>>>()?,
            lifetimes: application
                .lifetime_argument_identities
                .iter()
                .map(|identity| text(*identity))
                .collect::<Option<Vec<_>>>()?,
            requirements: application
                .requirement_identities
                .iter()
                .map(|identity| text(*identity))
                .collect::<Option<Vec<_>>>()?,
        },
    )
}

pub(crate) fn selection_matches_schema(
    subject: &crate::ProviderSelectionSubject,
    symbol: symbols::SymbolHandle,
    requirement: &str,
    schema: &effects::provider_plan::ServiceSchema,
) -> bool {
    if !subject.selects_declaration(symbol, requirement) {
        return false;
    }
    let receiver = match subject {
        crate::ProviderSelectionSubject::BoundaryTrait(identity)
        | crate::ProviderSelectionSubject::BoundaryRequirement(identity) => {
            identity.receiver.as_ref()
        }
        crate::ProviderSelectionSubject::BoundaryOperatorFamily(_) => None,
    };
    receiver.is_none_or(|receiver| {
        schema.receiver_applications.iter().any(|application| {
            receiver_identity(&schema.identities, application)
                .is_some_and(|candidate| candidate == *receiver)
        })
    })
}

/// Compare Psi-issued source receiver content with Omega's schema roster.
/// Scope projection remains in the shared Psi provider declaration facade.
pub(crate) fn source_receiver_matches_schema(
    program: &checked_trees::typed_trees::TypedTrees,
    scope: AppliedTraitScope<'_>,
    schema: &effects::provider_plan::ServiceSchema,
) -> bool {
    let Some(scopes) = collect_scopes(program, scope) else {
        return false;
    };
    let Some(applications) =
        build_time_evaluation::provider_declarations::receivers::receiver_applications(
            program, &scopes,
        )
    else {
        return false;
    };
    !applications.is_empty()
        && applications.len() == schema.receiver_applications.len()
        && applications
            .iter()
            .zip(&schema.receiver_applications)
            .all(|(actual, required)| {
                receiver_identity(&schema.identities, required)
                    .is_some_and(|required| *actual == required)
            })
}

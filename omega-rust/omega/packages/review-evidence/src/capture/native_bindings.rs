//! Native review retains evaluated Build producers and exact binding receipts.
use super::published_input::{PublishedPackageReviewInput, rejected};
use crate::record::*;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use provider_planning::evaluated_via_bindings::EvaluatedNativeBinding;

pub(super) fn native_producer(
    compilation: &PublishedPackageReviewInput<'_>,
    native: &provider_planning::native_bindings::NativeBuildRowOrigin,
) -> Result<PackagePolicyEvaluatedBindingProducer, Vec<Diagnostic>> {
    if native.application.is_some_and(|handle| {
        !compilation
            .candidate
            .product_operand_applications
            .is_valid(handle)
    }) {
        return Err(rejected(
            "native Build producer has a stale or foreign applied requirement coordinate",
        ));
    }
    let production = &native.origin.primary;
    let receipt = production.evaluated.receipt();
    let origin = receipt
        .build_origin()
        .ok_or_else(|| rejected("native Build row has no Build-origin receipt"))?;
    let requirement = compilation
        .candidate
        .trait_machine_signature(native.requirement)
        .map(|(owner, signature)| {
            let selected = native.application.map(|handle| {
                compilation
                    .candidate
                    .product_operand_applications
                    .get(handle)
                    .declaration
            });
            let selected = selected
                .and_then(|symbol| compilation.candidate.closed_requirement_application(symbol));
            compilation
                .candidate
                .trait_requirement_callable_identity_text(
                    owner,
                    selected
                        .as_ref()
                        .map_or(signature, |application| application.signature),
                )
        })
        .ok_or_else(|| rejected("native Build producer lost its exact requirement"))?;
    let identities = compilation
        .custody
        .providers()
        .evaluated_via_bindings
        .identities();
    if identities.text(origin.requirement_identity) != Some(requirement.as_str())
        || receipt.producer_package() != Some(production.producer.package)
        || identities.text(receipt.producer_callable_identity())
            != Some(production.producer.callable_identity.as_str())
    {
        return Err(rejected(
            "native Build producer receipt differs from its checked package source",
        ));
    }
    Ok(PackagePolicyEvaluatedBindingProducer {
        declaration: crate::record::PackageReviewNominalIdentity {
            owner: crate::record::PackageReviewNominalOwner::Package(production.producer.package),
            path: production.producer.declaration_path.clone(),
        },
        package: Some(production.producer.package),
        callable_identity: production.producer.callable_identity.clone(),
    })
}

pub(crate) fn project_native(
    compilation: &PublishedPackageReviewInput<'_>,
    native: &provider_planning::native_bindings::NativeBuildRowOrigin,
    selected: &ProviderBinding,
) -> Result<PackagePolicyProviderBinding, Vec<Diagnostic>> {
    if native.origin.primary.evaluated.provider_binding() != *selected {
        return Err(rejected(
            "native Build binding differs from its retained evaluated intent",
        ));
    }
    let producer = native_producer(compilation, native)?;
    match &native.origin.primary.evaluated {
        EvaluatedNativeBinding::Import(import) => Ok(PackagePolicyProviderBinding::Import {
            target: import.locator().target().identity().as_str().to_owned(),
            locator: PackageReviewForeignLocator::from_candidate(import.locator().locator()),
            producer,
        }),
        EvaluatedNativeBinding::Syscall(syscall) => Ok(PackagePolicyProviderBinding::Syscall {
            number: syscall.number(),
            evaluated: Some(PackagePolicyProviderEvaluatedSyscall {
                target: syscall.target().identity().as_str().to_owned(),
                producer,
            }),
        }),
    }
}

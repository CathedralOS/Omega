//! An unused generic import has an authored requirement and a completed Build
//! evaluation. Only a concrete application can supply a physical calling plan.
use super::{PolicySource, ServiceSchema, unavailable};
use build_providers::plans::EvaluatedForeignImport;
use diagnostics::Diagnostic;
use review_evidence::record::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyNativeApplication,
    PackagePolicyNativeBinding, PackagePolicyServiceAuthority, PackagePolicyServiceSignature,
    PackageReviewNominalIdentity,
};
use symbols::SymbolHandle;

pub(super) fn project(
    input: &PolicySource<'_>,
    schema: &ServiceSchema,
    requirement: SymbolHandle,
    source_occurrence: [u8; 32],
    import: &EvaluatedForeignImport,
    producer: PackagePolicyEvaluatedBindingProducer,
) -> Result<PackagePolicyNativeBinding, Vec<Diagnostic>> {
    let typed = input.typed();
    let (service, signature) = typed
        .trait_machine_signature(requirement)
        .ok_or_else(|| unavailable("the generic import's authored signature"))?;
    let declaration = typed.trait_requirement_callable_identity_text(service, signature);
    let mut matches = schema
        .declaration
        .methods
        .iter()
        .filter(|method| schema.identity_text(method.requirement_identity) == declaration);
    let (Some(method), None) = (matches.next(), matches.next()) else {
        return Err(unavailable("the generic import's exact schema method"));
    };
    let requirement = input.requirement(service, signature)?;
    let custom_calling_policy = selected_calling_policy(input, schema)?;
    let method = super::super::providers::service_method_policy(
        method,
        schema,
        requirement.identity,
        input.nominal(service.symbol)?,
        PackagePolicyServiceSignature {
            schema_arguments: Vec::new(),
            requirement_arguments: Vec::new(),
            schema_lifetime_parameter_count: 0,
            schema_lifetime_arguments: Vec::new(),
            application_lifetime_parameter_count: 0,
            requirement_lifetime_arguments: Vec::new(),
            requirement_lifetime_parameter_count: u32::try_from(
                requirement.lifetime_parameter_count,
            )
            .map_err(|_| unavailable("the generic import's lifetime parameter count"))?,
            static_parameters: requirement.type_parameters,
            parameters: requirement.parameters,
            result: requirement.return_type,
        },
        PackagePolicyServiceAuthority {
            service_reach: requirement
                .service_reach
                .ok_or_else(|| unavailable("the generic import's reach ceiling"))?,
            synchronous_invocations: requirement.synchronous_invocations,
            progress_premises: Vec::new(),
        },
    );
    Ok(PackagePolicyNativeBinding {
        application: PackagePolicyNativeApplication::UnusedGeneric {
            declaration,
            unbound_parameters: typed
                .state_signature_type_parameters(signature)
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect(),
            custom_calling_policy,
            source_occurrence,
            evaluation: import.receipt().identity_digest(),
        },
        method,
        binding: super::import_policy(import, producer),
    })
}

fn selected_calling_policy(
    input: &PolicySource<'_>,
    schema: &ServiceSchema,
) -> Result<Option<PackageReviewNominalIdentity>, Vec<Diagnostic>> {
    let accepted = input.accepted();
    let module = accepted.module();
    let application = schema
        .root_application_identity()
        .ok_or_else(|| unavailable("the unused import's declaring application"))?;
    let mut policy = None;
    for request in &module.interfaces.calling_policy_requests {
        for instance in request
            .instances
            .as_ref()
            .map_err(|reasons| unavailable(&reasons.join(", ")))?
        {
            if module.identities.text(instance.declaring_application) != Some(application) {
                continue;
            }
            let terminal_psi::TargetMachineCallable::Machine(id) = *instance
                .policy
                .as_ref()
                .map_err(|reason| unavailable(reason))?
            else {
                return Err(unavailable(
                    "the generic import's selected calling policy machine",
                ));
            };
            let symbol = accepted
                .closure()
                .elaboration()
                .machines
                .get(id)
                .ok_or_else(|| {
                    unavailable("the generic import's checked calling policy declaration")
                })?;
            if policy.replace(input.nominal(*symbol)?).is_some() {
                return Err(unavailable("one calling policy for the generic import"));
            }
        }
    }
    Ok(policy)
}

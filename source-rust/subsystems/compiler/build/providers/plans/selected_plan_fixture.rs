//! The provider-plan and provider-selection retained-input tests both pin
//! digests of this exact plan, so one copy keeps them in step.

use super::*;

pub(crate) fn selected_plan() -> ProviderPlan {
    let mut plan = ProviderPlan {
        name: "selected-provider".into(),
        provider_type: "Provider".into(),
        target: "linux_x86_64".into(),
        ..ProviderPlan::default()
    };
    plan.provider_application = plan.identities.intern("Provider<opaque>");
    let declaration = std::sync::Arc::make_mut(&mut plan.schema.declaration);
    declaration.trait_name = declaration.identities.intern("Boundary");
    let requirement = declaration.identities.intern("Boundary::consume(opaque)");
    let application = declaration.identities.intern("Boundary<opaque>");
    let argument = declaration.identities.intern("opaque");
    declaration
        .receiver_applications
        .push(ServiceReceiverApplication {
            application,
            trait_name: declaration.trait_name,
            trait_package_identity: None,
            argument_identities: vec![argument],
            lifetime_argument_identities: Vec::new(),
            requirement_identities: vec![requirement],
        });
    declaration.methods.push(ServiceMethod {
        name: "consume".into(),
        requirement_owner: "Boundary".into(),
        requirement_identity: requirement,
        ..ServiceMethod::default()
    });
    let requirement_identity = plan.identities.intern("Boundary::consume(opaque)");
    let machine_identity = plan.identities.intern("Provider::consume(opaque)");
    plan.rows.push(ProviderPlanRow {
        method: "consume".into(),
        requirement_identity,
        declared_requirement_identity: requirement_identity,
        calling_signature: None,
        requirement_lifetime_partition: Vec::new(),
        admitted_same_stack_bytes: 0,
        binding: ProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity: None,
        },
    });
    assert!(plan.validate_against_schema().is_empty());
    plan
}

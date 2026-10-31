//! Whole inherited receiver contracts retain calling and claim distinctions.
use super::*;

fn schema(root: &str, calling: u8, shift_identity_slots: bool) -> ServiceSchema {
    let mut identities = ProviderIdentityTable::default();
    if shift_identity_slots {
        identities.intern("unrelated mint order");
    }
    let root = identities.intern(root);
    let parent = identities.intern("Parent");
    let requirement = identities.intern("Parent::enter(u64)");
    let parameter = identities.intern("u64");
    let receiver = |name| ServiceReceiverApplication {
        application: Default::default(),
        trait_name: name,
        trait_package_identity: None,
        argument_identities: vec![],
        lifetime_argument_identities: vec![],
        requirement_identities: vec![requirement],
    };
    let declaration = std::sync::Arc::new(ServiceSurface {
        trait_name: root,
        identities,
        trait_package_identity: None,
        receiver_applications: vec![receiver(root), receiver(parent)],
        methods: vec![ServiceMethod {
            name: "enter".into(),
            requirement_owner: "Parent".into(),
            requirement_identity: requirement,
            parameter_count: 1,
            parameter_type_identities: vec![parameter],
            ..Default::default()
        }],
    });
    ServiceSchema {
        declaration,
        calling_applications: vec![ServiceCallingApplication {
            requirement,
            plan: ServiceCallingPlan {
                report_fingerprint: u64::from(calling),
                commitment: BoundaryCallingPlanCommitment::from_digest([calling; 32]),
            },
        }],
    }
}

#[test]
fn inherited_views_require_their_complete_effective_calling_contract() {
    let first = schema("FirstRoot", 1, false);
    let second = schema("SecondRoot", 2, false);
    assert!(!first.shares_receiver_application(&second));
    assert!(!second.shares_receiver_application(&first));
    let demanded = schema("Parent", 1, true);
    assert_eq!(first.receiver_application_index(&demanded), Some(1));
    assert!(!second.supplies_receiver_schema(&demanded));
}

#[test]
fn equal_shared_parent_contract_still_competes_across_choosing_owners() {
    let first = schema("FirstRoot", 1, false);
    let second = schema("SecondRoot", 1, true);
    assert!(first.shares_receiver_application(&second));
    assert!(second.shares_receiver_application(&first));
}

#[test]
fn compact_calling_coordinate_cannot_replace_its_exact_commitment() {
    let first = schema("FirstRoot", 1, false);
    let mut changed = schema("Parent", 1, true);
    changed.calling_applications[0].plan.commitment =
        BoundaryCallingPlanCommitment::from_digest([2; 32]);
    assert!(!first.supplies_receiver_schema(&changed));
}

#[test]
fn incomplete_or_weakened_method_roster_cannot_supply_a_receiver() {
    let first = schema("FirstRoot", 1, false);
    let mut changed = schema("Parent", 1, true);
    let declaration = std::sync::Arc::make_mut(&mut changed.declaration);
    declaration.methods[0].parameter_type_identities[0] = declaration.identities.intern("addr");
    assert!(!first.supplies_receiver_schema(&changed));
    std::sync::Arc::make_mut(&mut changed.declaration)
        .methods
        .clear();
    assert!(!first.supplies_receiver_schema(&changed));
}

#[test]
fn target_attachments_do_not_copy_or_change_the_neutral_declaration() {
    let first = schema("Root", 1, false);
    let mut second = first.clone();
    second.calling_applications[0].plan.commitment =
        BoundaryCallingPlanCommitment::from_digest([2; 32]);
    assert!(std::sync::Arc::ptr_eq(
        &first.declaration,
        &second.declaration
    ));
    assert_ne!(first.identity_digest(), second.identity_digest());
    assert_eq!(
        first.declaration_identity_digest(),
        second.declaration_identity_digest()
    );
    let unselected = ServiceSchema::from_declaration(first.declaration.clone());
    assert_eq!(
        first.declaration_identity_digest(),
        unselected.identity_digest()
    );
}

#[test]
fn attachment_roster_refuses_duplicate_and_foreign_coordinates() {
    let first = schema("Root", 1, false);
    assert!(first.validate_calling_applications().is_ok());
    let mut duplicate = first.clone();
    duplicate
        .calling_applications
        .push(duplicate.calling_applications[0].clone());
    assert!(duplicate.validate_calling_applications().is_err());
    let mut foreign = first;
    foreign.calling_applications[0].requirement = ProviderIdentityId::from_index(u32::MAX);
    assert!(foreign.validate_calling_applications().is_err());
}

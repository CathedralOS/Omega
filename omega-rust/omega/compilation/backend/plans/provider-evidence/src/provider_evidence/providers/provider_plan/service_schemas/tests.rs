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
        trait_name: name,
        trait_package_identity: None,
        argument_identities: vec![],
        lifetime_argument_identities: vec![],
        requirement_identities: vec![requirement],
    };
    ServiceSchema {
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
            calling_plan_report_fingerprint: Some(u64::from(calling)),
            calling_plan_commitment: Some(BoundaryCallingPlanCommitment::from_digest(
                [calling; 32],
            )),
            ..Default::default()
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
    changed.methods[0].calling_plan_commitment =
        Some(BoundaryCallingPlanCommitment::from_digest([2; 32]));
    assert!(!first.supplies_receiver_schema(&changed));
}

#[test]
fn incomplete_or_weakened_method_roster_cannot_supply_a_receiver() {
    let first = schema("FirstRoot", 1, false);
    let mut changed = schema("Parent", 1, true);
    changed.methods[0].parameter_type_identities[0] = changed.identities.intern("addr");
    assert!(!first.supplies_receiver_schema(&changed));
    changed.methods.clear();
    assert!(!first.supplies_receiver_schema(&changed));
}

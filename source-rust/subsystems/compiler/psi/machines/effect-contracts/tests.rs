#[test]
fn capability_flow_kind_names_are_stable() {
    use super::CapabilityFlowKind;

    let names = [
        CapabilityFlowKind::Uses,
        CapabilityFlowKind::Returns,
        CapabilityFlowKind::Acquires,
        CapabilityFlowKind::Stores,
        CapabilityFlowKind::Derives,
    ]
    .map(CapabilityFlowKind::as_str);

    assert_eq!(names, ["uses", "returns", "acquires", "stores", "derives"]);
}

#[test]
fn template_commitments_bind_the_base_and_both_callback_dependencies() {
    use super::{ContractDependency, template_contract_commitment};

    let selected = ContractDependency::parameter(0, true);
    let absent = false.into();
    let commit = |base, count, suspension, blocking| {
        template_contract_commitment(base, count, suspension, blocking)
    };
    let expected = commit([7; 32], 1, &selected, &absent).unwrap();
    assert_ne!(expected, commit([6; 32], 1, &selected, &absent).unwrap());
    assert_ne!(expected, commit([7; 32], 2, &selected, &absent).unwrap());
    assert_ne!(expected, commit([7; 32], 1, &absent, &selected).unwrap());
    assert!(commit([7; 32], 0, &selected, &absent).is_err());
}

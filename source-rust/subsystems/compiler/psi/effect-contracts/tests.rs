#[test]
fn published_carry_commits_exact_guard_subject_and_acquisition() {
    use super::{ConditionalCarryRequirement, template_contract_commitment};
    use crate::PermissionArrival;
    use crate::ScopedPermissionClaimIdentity;
    use crate::ScopedPermissionEventSource;
    use crate::ScopedPermissionProvenance;
    use crate::SuspensionValueOrigin;
    let condition = super::ContractDependency {
        concrete: false,
        parameters: vec![0],
    };
    let row = ConditionalCarryRequirement {
        state: 0,
        statement_index: 2,
        call_ordinal: 1,
        subject: SuspensionValueOrigin::Local {
            symbol: "helper::kept".into(),
            statement_index: 0,
            environment_position: 1,
        },
        value_type: "Kept".into(),
        claims: vec![PermissionArrival {
            claim_identity: ScopedPermissionClaimIdentity::Established {
                machine_symbol: "helper".into(),
                state_symbol: "helper::entry".into(),
                source: ScopedPermissionEventSource::Statement { statement_index: 0 },
                ordinal: 3,
            },
            provenance: ScopedPermissionProvenance::Joined {
                machine_symbol: "helper".into(),
                state_symbol: "helper::entry".into(),
                source: ScopedPermissionEventSource::Call {
                    statement_index: 0,
                    call_ordinal: 0,
                    target_symbol: "acquire".into(),
                },
                ordinal: 3,
            },
        }],
        condition: condition.clone(),
    };
    let commit = |rows: &[ConditionalCarryRequirement]| {
        template_contract_commitment([7; 32], 2, &condition, &false.into(), rows)
    };
    let expected = commit(std::slice::from_ref(&row)).unwrap();
    assert_ne!(expected, commit(&[]).unwrap());
    let mut changed = row.clone();
    changed.condition.parameters = vec![1];
    assert_ne!(expected, commit(&[changed]).unwrap());
    let mut changed = row.clone();
    changed.subject = SuspensionValueOrigin::CallArgument { position: 1 };
    assert_ne!(expected, commit(&[changed]).unwrap());
    let mut changed = row.clone();
    let ScopedPermissionClaimIdentity::Established { ordinal, .. } =
        &mut changed.claims[0].claim_identity
    else {
        unreachable!()
    };
    *ordinal += 1;
    assert_ne!(expected, commit(&[changed]).unwrap());
    let mut changed = row.clone();
    changed.condition.parameters = vec![2];
    assert!(commit(&[changed]).is_err());
    let mut changed = row.clone();
    changed.claims[0].claim_identity = ScopedPermissionClaimIdentity::Unknown;
    assert!(commit(&[changed]).is_err());
    let mut changed = row.clone();
    changed.claims[0].provenance = ScopedPermissionProvenance::Unknown;
    assert!(commit(&[changed]).is_err());
    for change_target in [false, true] {
        let mut changed = row.clone();
        let ScopedPermissionProvenance::Joined {
            source:
                ScopedPermissionEventSource::Call {
                    target_symbol,
                    call_ordinal,
                    ..
                },
            ..
        } = &mut changed.claims[0].provenance
        else {
            unreachable!()
        };
        if change_target {
            *target_symbol = "other_acquire".into();
        } else {
            *call_ordinal += 1;
        }
        assert_ne!(expected, commit(&[changed]).unwrap());
    }
    assert!(commit(&[row.clone(), row]).is_err());
}

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

use super::*;

fn description<'identity>(
    lanes: &'identity [Vec<String>; 4],
) -> ConformanceIdentityDescription<'identity> {
    ConformanceIdentityDescription {
        declaration: "Pick",
        trait_name: "Slot",
        lifetime_arguments: &lanes[0],
        type_arguments: &lanes[1],
        trait_lifetime_arguments: &lanes[2],
        trait_arguments: &lanes[3],
        const_arguments: vec![
            ConformanceConstIdentity::Evaluated {
                parameter_carrier: "u64".into(),
                declared_carrier: "u32".into(),
                value_type: "u32",
                value_encoding: "7",
            },
            ConformanceConstIdentity::CallerBinder {
                parameter_carrier: "u64".into(),
                binder_carrier: "u64".into(),
                binder_identity: "Owner::N".into(),
            },
            ConformanceConstIdentity::CallerExpression {
                parameter_carrier: "u64".into(),
                expression_identity: "add(N,1)".into(),
            },
        ],
        machine_arguments: vec!["Handler::call".into(), "Other::entry".into()],
        machine_operands: vec![MachineOperandIdentity {
            selection: MachineOperandSelectionIdentity::Conformance("pkg::Dispatch".into()),
            application: Some(MachineOperandApplicationIdentity {
                lifetime_arguments: vec!["'a".into()],
                arguments: vec![
                    MachineOperandIdentity {
                        selection: MachineOperandSelectionIdentity::Type("u8".into()),
                        application: None,
                    },
                    MachineOperandIdentity {
                        selection: MachineOperandSelectionIdentity::Const("u64:7".into()),
                        application: None,
                    },
                    MachineOperandIdentity {
                        selection: MachineOperandSelectionIdentity::Conformance(
                            "pkg::Inner".into(),
                        ),
                        application: Some(MachineOperandApplicationIdentity {
                            lifetime_arguments: Vec::new(),
                            arguments: Vec::new(),
                        }),
                    },
                ],
            }),
        }],
        rows: vec![
            ConformanceRowIdentity {
                declaring_trait: "Slot",
                requirement: "invoke",
                realization: "handler",
                requirement_signature: Some(vec![0, 255, 254]),
            },
            ConformanceRowIdentity {
                declaring_trait: "Slot",
                requirement: "empty",
                realization: "none",
                requirement_signature: None,
            },
        ],
    }
}

fn lanes() -> [Vec<String>; 4] {
    [
        vec!["'a".into(), "'b".into()],
        vec!["Pair<u8>".into()],
        vec!["'b".into()],
        vec!["Pair<u8>".into(), "Layout".into(), "Callback".into()],
    ]
}

#[test]
fn source_v4_snapshot_keeps_all_lanes_and_historical_framing() {
    let lanes = lanes();
    let description = description(&lanes);
    // Frozen from the pre-extraction v4 traversal, including all three const
    // tags, all recursive operand tags, and present/absent signature lanes.
    assert_eq!(description.encoded_bytes().len(), 545);
    assert_eq!(
        description.fingerprint_and_commitment(),
        (
            0xb83bfa9b72729933,
            [
                0x93, 0x93, 0x25, 0xb5, 0xfa, 0xe8, 0x93, 0x96, 0xcc, 0x8e, 0xe3, 0x13, 0x43, 0x71,
                0xf2, 0xd0, 0xd5, 0x14, 0xc6, 0x63, 0x1a, 0x00, 0x69, 0x52, 0x19, 0xb7, 0x23, 0x62,
                0x3f, 0x30, 0x7d, 0x88,
            ]
        )
    );
}

#[test]
fn role_carriers_recursive_selection_and_application_presence_are_distinct() {
    let lanes = lanes();
    let original = description(&lanes);
    let expected = original.fingerprint_and_commitment();
    let changed = |candidate: ConformanceIdentityDescription<'_>| {
        let observed = candidate.fingerprint_and_commitment();
        assert_ne!(observed.0, expected.0);
        assert_ne!(observed.1, expected.1);
    };
    let mut candidate = original.clone();
    let ConformanceConstIdentity::Evaluated {
        declared_carrier, ..
    } = &mut candidate.const_arguments[0]
    else {
        panic!("fixture must contain an evaluated const");
    };
    *declared_carrier = "u64".into();
    changed(candidate);
    let mut candidate = original.clone();
    let application = candidate.machine_operands[0].application.as_mut().unwrap();
    application.arguments[0].selection = MachineOperandSelectionIdentity::Const("u8".into());
    changed(candidate);
    let mut candidate = original.clone();
    candidate.machine_operands[0]
        .application
        .as_mut()
        .unwrap()
        .arguments[2]
        .application = None;
    changed(candidate);
    let mut candidate = original.clone();
    candidate.rows[1].requirement_signature = Some(Vec::new());
    changed(candidate);
    let mut candidate = original.clone();
    candidate.machine_arguments.reverse();
    changed(candidate);

    let mut terminal_domain =
        DomainDigestEncoder::new(b"omega.psi.terminal.closed-conformance-application.v7\0");
    terminal_domain.raw(&original.encoded_bytes());
    assert_ne!(expected.1, terminal_domain.finish());
}

#[test]
fn selected_slot_record_preserves_roles_and_source_preimage() {
    use crate::callback_declarations::PrivateCallbackSlotSelection;
    let lanes = lanes();
    let description = description(&lanes);
    let application_encoding: std::sync::Arc<[u8]> = description.encoded_bytes().into();
    let (application_report_fingerprint, application_commitment) =
        source_identity_commitments(&application_encoding);
    let selection = PrivateCallbackSlotSelection {
        declaration_identity: "package::Pick".into(),
        trait_identity: "core::layout::PrivateCallbackSlot".into(),
        semantic_identity: "package::Pair<u8>".into(),
        layout_identity: "package::Layout".into(),
        requirement_identity: "package::Callback::call".into(),
        application_report_fingerprint,
        application_commitment,
        application_encoding: application_encoding.clone(),
    };
    selection.validate_identity_encoding().unwrap();
    let bytes = compiler_storage::encode(&selection, Default::default()).unwrap();
    let restored: PrivateCallbackSlotSelection =
        compiler_storage::decode(&bytes, Default::default()).unwrap();
    assert_eq!(restored, selection);
    restored.validate_identity_encoding().unwrap();
    let cloned = selection.clone();
    assert!(std::sync::Arc::ptr_eq(
        &cloned.application_encoding,
        &application_encoding
    ));
    let mut changed = selection.clone();
    std::sync::Arc::make_mut(&mut changed.application_encoding)[0] ^= 1;
    assert!(
        changed
            .validate_identity_encoding()
            .unwrap_err()
            .contains("Source-v4 identity preimage")
    );
    let mut changed = selection;
    changed.requirement_identity.clear();
    assert!(
        changed
            .validate_identity_encoding()
            .unwrap_err()
            .contains("exact selected application identity")
    );
}

use crate::plans::digest_encoder::PROVIDER_PLAN_DOMAIN;
use crate::plans::{
    BoundaryCallingPlanCommitment, BoundaryCallingPlanPromise, ProviderBinding, ProviderPlan,
    ServiceCallingApplication, ServiceCallingPlan,
};
use component_description::retained_plan::ProviderPlanInput;
use semantic_vocabulary::{CallingAbiIdentity, CallingOperandKind, CallingStaticOperand};
use sha2::{Digest, Sha256};

use crate::plans::selected_plan_fixture::selected_plan;

#[test]
fn table_field_correspondence_is_committed_and_received_in_full() {
    use crate::plans::TableFieldStorage;
    let storage = TableFieldStorage {
        table_application: "closed-table<word>".into(),
        field_declaration: "exact-package::table::invoke".into(),
    };
    for storage in [Ok(storage), Err("field is not address".into())] {
        let mut plan = selected_plan();
        plan.rows[0].binding = ProviderBinding::TableFunction {
            table: "Table".into(),
            field: "invoke".into(),
            storage,
        };
        let bytes = plan.identity_preimage();
        let received = ProviderPlanInput::read(&bytes, plan.identity_digest().as_bytes())
            .expect("complete table-field preimage");
        assert_eq!(received.rows.len(), 1);
        assert!(received.rows[0].checked_adapter.is_none());
        let mut changed = plan.clone();
        let ProviderBinding::TableFunction { storage, .. } = &mut changed.rows[0].binding else {
            panic!("table fixture");
        };
        match storage {
            Ok(storage) => storage.field_declaration.push_str("-different-field"),
            Err(reason) => reason.push_str("-different-refusal"),
        }
        assert_ne!(plan.identity_digest(), changed.identity_digest());
    }
}

fn read(bytes: &[u8]) -> Result<ProviderPlanInput<'_>, &'static str> {
    ProviderPlanInput::read(bytes, &Sha256::digest(bytes).into())
}

fn plan_with_calling_applications(count: usize) -> ProviderPlan {
    let mut plan = selected_plan();
    let kinds = [
        CallingOperandKind::Type,
        CallingOperandKind::Const,
        CallingOperandKind::Value,
        CallingOperandKind::Machine,
        CallingOperandKind::Implementation,
        CallingOperandKind::Proposition,
    ];
    for index in 0..count {
        plan.schema
            .calling_applications
            .push(ServiceCallingApplication {
                requirement: plan.schema.declaration.methods[0].requirement_identity,
                abi: CallingAbiIdentity {
                    requirement_identity: "Boundary::consume(opaque)".into(),
                    declaring_application: format!("Boundary<application-{index}>"),
                    operands: kinds
                        .iter()
                        .enumerate()
                        .map(|(ordinal, kind)| CallingStaticOperand {
                            ordinal: ordinal as u32 * 2 + 1,
                            kind: *kind,
                            identity: format!("static-operand-{ordinal}"),
                        })
                        .collect(),
                },
                plan: ServiceCallingPlan {
                    report_fingerprint: index as u64 + 1,
                    commitment: BoundaryCallingPlanCommitment::from_digest([index as u8 + 17; 32]),
                    promise: BoundaryCallingPlanPromise::from_digest([index as u8 + 41; 32]),
                },
            });
    }
    assert!(plan.validate_against_schema().is_empty());
    plan
}

fn position(bytes: &[u8], value: &[u8]) -> usize {
    let matches = bytes
        .windows(value.len())
        .enumerate()
        .filter_map(|(position, candidate)| (candidate == value).then_some(position))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1);
    matches[0]
}

#[test]
fn complete_calling_application_rosters_round_trip_in_canonical_order() {
    for count in 0..=2 {
        let mut plan = plan_with_calling_applications(count);
        let bytes = plan.identity_preimage();
        let input = ProviderPlanInput::read(&bytes, plan.identity_digest().as_bytes()).unwrap();
        assert_eq!(input.rows[0].requirement, "Boundary::consume(opaque)");
        plan.schema.calling_applications.reverse();
        assert_eq!(bytes, plan.identity_preimage());
    }
}

#[test]
fn distinct_static_applications_can_share_one_physical_calling_plan() {
    let mut plan = plan_with_calling_applications(2);
    let first = plan.schema.calling_applications[0].clone();
    let second = &mut plan.schema.calling_applications[1];
    second.abi.declaring_application = first.abi.declaring_application;
    second.abi.operands[0].identity = "static-operand-0-other".into();
    second.plan = first.plan;
    assert!(plan.validate_against_schema().is_empty());
    let bytes = plan.identity_preimage();
    let input = ProviderPlanInput::read(&bytes, plan.identity_digest().as_bytes()).unwrap();
    assert_eq!(input.rows[0].requirement, "Boundary::consume(opaque)");
    plan.schema.calling_applications.reverse();
    assert_eq!(bytes, plan.identity_preimage());
}

#[test]
fn calling_application_keys_reject_foreign_or_incomplete_coordinates() {
    let original = plan_with_calling_applications(1);
    let mut changed = original.clone();
    changed.schema.calling_applications[0]
        .abi
        .requirement_identity = "Other::consume".into();
    assert_eq!(
        read(&changed.identity_preimage()).unwrap_err(),
        "provider calling application differs from its method requirement"
    );
    let mut changed = original.clone();
    changed.schema.calling_applications[0]
        .abi
        .declaring_application
        .clear();
    assert_eq!(
        read(&changed.identity_preimage()).unwrap_err(),
        "provider plan identity is empty"
    );
    let mut changed = original.clone();
    changed.schema.calling_applications[0].abi.operands[0]
        .identity
        .clear();
    assert_eq!(
        read(&changed.identity_preimage()).unwrap_err(),
        "provider plan identity is empty"
    );
    let mut changed = original.clone();
    changed.schema.calling_applications[0].abi.operands[1].ordinal = 0;
    assert_eq!(
        read(&changed.identity_preimage()).unwrap_err(),
        "provider calling operand ordinals repeat or reorder"
    );
    let mut changed = original.clone();
    changed.schema.calling_applications[0]
        .abi
        .operands
        .reverse();
    assert_eq!(
        read(&changed.identity_preimage()).unwrap_err(),
        "provider calling operand ordinals repeat or reorder"
    );
    let mut changed = original.clone();
    changed
        .schema
        .calling_applications
        .push(changed.schema.calling_applications[0].clone());
    assert_eq!(
        read(&changed.identity_preimage()).unwrap_err(),
        "provider calling applications repeat or reorder"
    );
    let mut changed = original.clone();
    changed.schema.calling_applications[0]
        .plan
        .report_fingerprint = 0;
    assert_eq!(
        read(&changed.identity_preimage()).unwrap_err(),
        "provider calling report coordinate is zero"
    );
    let mut changed = original;
    changed.schema.calling_applications[0].plan.commitment =
        BoundaryCallingPlanCommitment::from_digest([0; 32]);
    assert_eq!(
        read(&changed.identity_preimage()).unwrap_err(),
        "provider calling commitment is zero"
    );
}

#[test]
fn calling_application_bytes_reject_malformed_lengths_tags_and_order() {
    let plan = plan_with_calling_applications(1);
    let bytes = plan.identity_preimage();
    let operand = position(&bytes, b"static-operand-0");
    let mut changed = bytes.clone();
    changed[operand - 9] = 6;
    assert_eq!(
        read(&changed).unwrap_err(),
        "provider plan input has an unknown tag"
    );
    let mut changed = bytes.clone();
    changed[operand - 17..operand - 9].copy_from_slice(&(u64::from(u32::MAX) + 1).to_le_bytes());
    assert_eq!(
        read(&changed).unwrap_err(),
        "provider calling operand ordinal exceeds u32"
    );
    let mut changed = bytes.clone();
    let commitment = position(&bytes, &[17; 32]);
    changed[commitment - 8..commitment].copy_from_slice(&31_u64.to_le_bytes());
    assert_eq!(
        read(&changed).unwrap_err(),
        "provider calling commitment has the wrong width"
    );
    let mut changed = bytes.clone();
    let application = position(&bytes, b"Boundary<application-0>");
    let count = application - 24 - b"Boundary::consume(opaque)".len();
    changed[count..count + 8].copy_from_slice(&4097_u64.to_le_bytes());
    assert_eq!(
        read(&changed).unwrap_err(),
        "provider plan roster exceeds its bound"
    );
    for end in 0..bytes.len() {
        assert!(
            read(&bytes[..end]).is_err(),
            "truncated calling roster at {end}"
        );
    }
    assert!(ProviderPlanInput::read(&bytes, &[0; 32]).is_err());
    let mut changed = bytes;
    changed.push(0);
    assert_eq!(
        read(&changed).unwrap_err(),
        "provider plan input has trailing bytes"
    );

    let bytes = plan_with_calling_applications(2).identity_preimage();
    let requirement_bytes = b"Boundary::consume(opaque)".len() + 16;
    let first = position(&bytes, b"Boundary<application-0>") - requirement_bytes;
    let second = position(&bytes, b"Boundary<application-1>") - requirement_bytes;
    let end = position(&bytes, &[18; 32]) + 32;
    let changed = [
        &bytes[..first],
        &bytes[second..end],
        &bytes[first..second],
        &bytes[end..],
    ]
    .concat();
    assert_eq!(
        read(&changed).unwrap_err(),
        "provider calling applications repeat or reorder"
    );
}

#[test]
fn original_full_plan_input_exposes_exact_checked_row() {
    let plan = selected_plan();
    let bytes = plan.identity_preimage();
    let input = ProviderPlanInput::read(&bytes, plan.identity_digest().as_bytes()).unwrap();
    assert_eq!(input.provider, "Provider");
    assert_eq!(input.target, "linux_x86_64");
    assert_eq!(input.rows.len(), 1);
    assert_eq!(input.rows[0].requirement, "Boundary::consume(opaque)");
    assert_eq!(
        input.rows[0].checked_adapter,
        Some(("Provider::consume(opaque)", None))
    );
}

#[test]
fn applied_provider_and_receiver_remain_in_the_complete_commitment() {
    let mut plan = selected_plan();
    plan.provider_application = plan.identities.intern("Provider<u64>");
    let declaration = std::sync::Arc::make_mut(&mut plan.schema.declaration);
    let application = declaration.identities.intern("Boundary<u64>");
    declaration
        .receiver_applications
        .push(crate::plans::ServiceReceiverApplication {
            application,
            trait_name: declaration.trait_name,
            trait_package_identity: None,
            argument_identities: vec![declaration.identities.intern("u64")],
            lifetime_argument_identities: vec![],
            requirement_identities: vec![declaration.methods[0].requirement_identity],
        });
    assert!(plan.validate_against_schema().is_empty());
    let bytes = plan.identity_preimage();
    let digest = Sha256::digest(&bytes).into();
    assert!(ProviderPlanInput::read(&bytes, &digest).is_ok());
    for spelling in [b"Provider<u64>".as_slice(), b"Boundary<u64>".as_slice()] {
        let mut changed = bytes.clone();
        let offset = changed
            .windows(spelling.len())
            .position(|part| part == spelling)
            .unwrap();
        changed[offset] ^= 1;
        assert!(ProviderPlanInput::read(&changed, &digest).is_err());
    }
    for end in 0..bytes.len() {
        assert!(
            read(&bytes[..end]).is_err(),
            "truncated application prefix {end}"
        );
    }
    let version_offset = PROVIDER_PLAN_DOMAIN.len() - 2;
    let current_version = bytes[version_offset];
    // Derive corruptions from the current schema so a format update cannot
    // accidentally turn one of the rejected versions into the valid input.
    for version in [
        current_version.wrapping_sub(1),
        current_version.wrapping_add(1),
    ] {
        let mut other_schema = bytes.clone();
        other_schema[version_offset] = version;
        assert_eq!(
            read(&other_schema).unwrap_err(),
            "unknown provider plan input schema"
        );
    }
}

#[test]
fn retained_plan_rejects_changed_commitment_and_malformed_complete_inputs() {
    let plan = selected_plan();
    let bytes = plan.identity_preimage();
    assert!(ProviderPlanInput::read(&bytes, &[0; 32]).is_err());
    for end in 0..bytes.len() {
        assert!(read(&bytes[..end]).is_err(), "truncated prefix {end}");
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(read(&trailing).is_err());
    let mut unknown_schema = bytes.clone();
    unknown_schema[0] ^= 1;
    assert!(read(&unknown_schema).is_err());
    let mut retired_schema = bytes.clone();
    retired_schema[PROVIDER_PLAN_DOMAIN.len() - 2] = b'4';
    assert!(read(&retired_schema).is_err());
    let mut overflowing_string = bytes;
    overflowing_string[PROVIDER_PLAN_DOMAIN.len()..PROVIDER_PLAN_DOMAIN.len() + 8]
        .copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(read(&overflowing_string).is_err());
    let mut repeated = plan.clone();
    repeated.rows.push(repeated.rows[0].clone());
    assert!(read(&repeated.identity_preimage()).is_err());
    let mut outside_schema = plan.clone();
    outside_schema.rows[0].requirement_identity =
        outside_schema.identities.intern("Other::consume(opaque)");
    assert!(read(&outside_schema.identity_preimage()).is_err());
    let mut missing = plan;
    missing.rows.clear();
    assert!(read(&missing.identity_preimage()).is_err());
}
#[test]
fn retained_plan_rejects_retired_schema_with_a_matching_commitment() {
    let mut bytes = selected_plan().identity_preimage();
    let retired_domain = b"omega.provider-plan.sha256.v4\0";
    assert_eq!(retired_domain.len(), PROVIDER_PLAN_DOMAIN.len());
    bytes[..retired_domain.len()].copy_from_slice(retired_domain);
    assert_eq!(
        read(&bytes).unwrap_err(),
        "unknown provider plan input schema"
    );
}

#[test]
fn retained_plan_reads_complete_applied_identity_framing() {
    let mut plan = selected_plan();
    plan.provider_application = plan.identities.intern("Provider<Payload,'caller>");
    let declaration = std::sync::Arc::make_mut(&mut plan.schema.declaration);
    let application = declaration.identities.intern("Boundary<Payload,'caller>");
    let argument = declaration.identities.intern("Payload");
    let lifetime = declaration.identities.intern("'caller");
    declaration
        .receiver_applications
        .push(crate::plans::ServiceReceiverApplication {
            application,
            trait_name: declaration.trait_name,
            trait_package_identity: declaration.trait_package_identity,
            argument_identities: vec![argument],
            lifetime_argument_identities: vec![lifetime],
            requirement_identities: vec![declaration.methods[0].requirement_identity],
        });
    assert!(plan.validate_against_schema().is_empty());
    let bytes = plan.identity_preimage();
    let input = ProviderPlanInput::read(&bytes, plan.identity_digest().as_bytes()).unwrap();
    assert_eq!(input.rows[0].requirement, "Boundary::consume(opaque)");
    let mut changed = bytes;
    let position = changed
        .windows(b"Provider<Payload,'caller>".len())
        .position(|window| window == b"Provider<Payload,'caller>")
        .unwrap();
    changed[position] ^= 1;
    assert_eq!(
        ProviderPlanInput::read(&changed, plan.identity_digest().as_bytes()).unwrap_err(),
        "provider plan input differs from its complete commitment"
    );
}

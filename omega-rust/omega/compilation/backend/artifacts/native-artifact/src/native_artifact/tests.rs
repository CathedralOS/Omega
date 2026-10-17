use super::provider_execution::{
    validate_foreign_stack_contribution, validate_provider_execution_reports,
};
use super::{
    BTreeSet, NativeArtifactIdentity, NativeArtifactIdentityFields, NativePhysicalEvidenceScope,
    NativeProviderExecution, NativeSelectedProviderPlan, NativeSelectedProviderPlanDigest,
    TerminalAuthorityPermissionPolicyIdentity, TerminalAuthorityPolicyIdentity,
    derive_native_artifact_identity,
};
use effects::provider_plan::{
    ProviderBinding, ProviderIdentityTable, ProviderPlan, ProviderPlanRow, ServiceMethod,
    ServiceSchema,
};
use effects::{SelectedProviderPlanFacts, SelectedRequirementSpecialization};

fn kernel_call(profile: target::TargetProfile) -> image_emission::ObjectForeignCall {
    let native = profile.native_target();
    let signature = calling_conventions::CallSignature {
        parameters: vec![calling_conventions::ValueShape::integer(4, 4)],
        result: Some(calling_conventions::ValueShape::integer(4, 4)),
    };
    image_emission::ObjectForeignCall {
        machine: semantic_vocabulary::MachineId::new(1).unwrap(),
        owner: target_operations::CallSiteOwner::Operation(
            semantic_vocabulary::OperationId::new(2).unwrap(),
        ),
        operation_ordinal: 0,
        binding: target_operations::NormalizedForeignCallBinding {
            boundary_entry_plan: calling_conventions::evaluate_ordinary_boundary_entry_plan(
                calling_conventions::CallingPolicy::native_syscall_for_target(native).unwrap(),
                &signature,
            )
            .unwrap()
            .plan()
            .clone(),
            mechanism: target_operations::NormalizedForeignCallMechanism::SystemCall(
                target_operations::NormalizedSystemCallBinding {
                    target: profile,
                    number: 0,
                    requirement_identity: "Input::read".into(),
                    provider_plan_report_identity: 7,
                    provider_plan_commitment: [23; 32],
                },
            ),
        },
        provider_execution: machine_code::ProviderExecutionRecord::new(7, 11, 13, 17, 19).unwrap(),
        caller_live_bytes: 32,
        scalar_arguments: Vec::new(),
        callback_addresses: Vec::new(),
        scalar_result: None,
        x86_floating_control: None,
        aarch64_floating_control: None,
        text_offset: 64,
    }
}

#[test]
fn kernel_provider_custody_rejects_substituted_plans_and_user_control_records() {
    use super::provider_execution::validate_foreign_provider_binding;
    let plans = [NativeSelectedProviderPlan::new(
        7,
        NativeSelectedProviderPlanDigest::from_digest([23; 32]),
        vec!["Input::read".into()],
    )];
    for profile in [
        target::TargetProfile::LinuxX64,
        target::TargetProfile::LinuxArm64,
    ] {
        let call = kernel_call(profile);
        let identities = terminal_psi::TerminalIdentityTable::default();
        let validate = |call: &image_emission::ObjectForeignCall| {
            validate_foreign_provider_binding("Input::read", call, &plans, &identities)
        };
        // Zero is a valid syscall number; no user-space stack receipt is invented.
        assert_eq!(validate(&call), Ok(()));
        for mutation in 0..5 {
            let mut changed = call.clone();
            let target_operations::NormalizedForeignCallMechanism::SystemCall(kernel) =
                &mut changed.binding.mechanism
            else {
                unreachable!()
            };
            match mutation {
                0 => kernel.provider_plan_commitment[0] ^= 1,
                1 => kernel.provider_plan_report_identity += 1,
                2 => kernel.requirement_identity = "Input::other".into(),
                3 => kernel.number = -1,
                _ => changed.provider_execution.provider_plan_report_identity += 1,
            }
            assert!(
                validate(&changed).is_err(),
                "{profile:?} mutation {mutation}"
            );
        }
        let mut changed = call.clone();
        changed.binding.boundary_entry_plan.call.ordinary_clobbers =
            calling_conventions::RegisterSet::new(Vec::new());
        assert!(validate(&changed).is_err());
        let mut changed = call.clone();
        changed.x86_floating_control = Some(machine_code::X86ForeignCallFloatingControlRecord {
            target: target::NativeTarget::linux_x64(),
            saved_slot_byte_offset: 0,
            save_offset: 1,
            save_byte_count: 4,
            restore_offset: 8,
            restore_byte_count: 4,
        });
        assert!(validate(&changed).is_err());
        let mut changed = call.clone();
        changed.aarch64_floating_control =
            Some(machine_code::Aarch64ForeignCallFloatingControlRecord {
                target: target::NativeTarget::linux_arm64(),
                saved_slot_byte_offset: 0,
                save_offset: 1,
                save_byte_count: 4,
                restore_offset: 8,
                restore_byte_count: 4,
            });
        assert!(validate(&changed).is_err());
    }
}

#[test]
fn kernel_custody_identity_binds_target_number_requirement_and_provider() {
    use super::identity::foreign_call_custody_digest;
    let call = kernel_call(target::TargetProfile::LinuxX64);
    let expected = foreign_call_custody_digest(std::slice::from_ref(&call));
    for mutation in 0..5 {
        let mut changed = call.clone();
        let target_operations::NormalizedForeignCallMechanism::SystemCall(kernel) =
            &mut changed.binding.mechanism
        else {
            unreachable!()
        };
        match mutation {
            0 => kernel.target = target::TargetProfile::LinuxArm64,
            1 => kernel.number = 211,
            2 => kernel.requirement_identity = "Input::other".into(),
            3 => kernel.provider_plan_report_identity += 1,
            _ => kernel.provider_plan_commitment[0] ^= 1,
        }
        assert_ne!(
            expected,
            foreign_call_custody_digest(&[changed]),
            "mutation {mutation}"
        );
    }
}

#[test]
fn selected_plan_projection_retains_closed_and_unspecialized_requirements() {
    let plans = ["Generic", "Ordinary"].map(|name| {
        let mut schema_identities = ProviderIdentityTable::default();
        let trait_name = schema_identities.intern(name);
        let requirement_identity = schema_identities.intern(&format!("{name}::read"));
        let mut identities = ProviderIdentityTable::default();
        let machine_identity = identities.intern(&format!("{name}Provider::read"));
        let row_requirement_identity = identities.intern(&format!("{name}::read"));
        ProviderPlan {
            name: name.into(),
            provider_type: format!("{name}Provider"),
            target: "macos_arm64".into(),
            schema: ServiceSchema {
                trait_name,
                identities: schema_identities,
                methods: vec![ServiceMethod {
                    completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                    name: "read".into(),
                    requirement_owner: name.into(),
                    requirement_identity,
                    ..ServiceMethod::default()
                }],
                ..ServiceSchema::default()
            },
            identities,
            rows: vec![ProviderPlanRow {
                method: "read".into(),
                requirement_identity: row_requirement_identity,
                requirement_lifetime_partition: Vec::new(),
                admitted_same_stack_bytes: 0,
                binding: ProviderBinding::CheckedAdapter {
                    machine_identity,
                    machine_package_identity: None,
                },
            }],
            ..ProviderPlan::default()
        }
    });
    let generic_report_identity = plans[0].report_fingerprint();
    let row_identities = terminal_psi::TerminalIdentityTable::from(vec![
        String::new(),
        "Generic::read".to_owned(),
        "GenericProvider::read".to_owned(),
        "Generic::read<u64>".to_owned(),
        "GenericProvider::read<u64>".to_owned(),
        "Generic::read<i32>".to_owned(),
        "GenericProvider::read<i32>".to_owned(),
    ]);
    let selection = SelectedProviderPlanFacts::from_selected_plans(plans.into())
        .expect("fully covering plans")
        .with_specialized_requirement_rows(
            ["u64", "i32"]
                .map(|element| SelectedRequirementSpecialization {
                    provider_plan_report_identity: generic_report_identity,
                    selected_requirement_identity: row_identities
                        .index_of("Generic::read")
                        .unwrap(),
                    selected_machine_identity: row_identities
                        .index_of("GenericProvider::read")
                        .unwrap(),
                    requirement_identity: row_identities
                        .index_of(format!("Generic::read<{element}>").as_str())
                        .unwrap(),
                    machine_identity: row_identities
                        .index_of(format!("GenericProvider::read<{element}>").as_str())
                        .unwrap(),
                })
                .to_vec(),
            &row_identities,
        )
        .expect("two exact closed instances of the selected adapter");
    let projected = NativeSelectedProviderPlan::project_selection(&selection);

    assert_eq!(projected.len(), 2);
    assert!(
        projected
            .windows(2)
            .all(|pair| pair[0].report_identity() < pair[1].report_identity())
    );
    for plan in selection.plans() {
        let projection = projected
            .iter()
            .find(|projection| projection.report_identity() == plan.report_fingerprint())
            .expect("selected plan retained");
        assert_eq!(
            projection.plan_digest(),
            NativeSelectedProviderPlanDigest::from_digest(*plan.identity_digest().as_bytes())
        );
        let expected: &[&str] = if plan.report_fingerprint() == generic_report_identity {
            &["Generic::read<i32>", "Generic::read<u64>"]
        } else {
            &["Ordinary::read"]
        };
        assert_eq!(projection.requirement_identities(), expected);
    }
}

#[test]
fn closed_import_reports_rejoin_original_execution_and_reject_plan_substitution() {
    // Synthetic report data tests replay addresses, never mints provider authority.
    let selected = "Registrar::register";
    let closed = "Registrar::register<u64, Comparator::compare>";
    let other = "Registrar::register<i32, Comparator::compare>";
    let identities = terminal_psi::TerminalIdentityTable::from(vec![
        String::new(),
        selected.into(),
        closed.into(),
        other.into(),
    ]);
    let digest = effects::provider_plan::ProviderPlanDigest::from_digest([2; 32]);
    let mut plan = NativeSelectedProviderPlan::new(
        7,
        NativeSelectedProviderPlanDigest::from_digest(*digest.as_bytes()),
        vec![closed.into()],
    );
    plan.family_realization_identities = identities.clone();
    plan.external_requirement_applications
        .push(effects::SelectedExternalRequirementApplication {
            provider_plan_report_identity: 7,
            provider_plan_digest: digest,
            selected_requirement_identity: identities.index_of(selected).unwrap(),
            requirement_identity: identities.index_of(closed).unwrap(),
        });
    let execution = NativeProviderExecution {
        requirement_identity: selected.into(),
        provider_plan_report_identity: 7,
        provider_execution_report_identity: 11,
        provider_execution_report_fingerprint: 12,
        normalized_root_report_identity: 13,
        boundary_contract_report_fingerprint: 14,
    };
    let required = BTreeSet::from([(selected.into(), 7, 11, 12, 13, 14)]);
    assert_eq!(plan.selected_requirement_identity(closed), selected);
    assert_eq!(plan.selected_requirement_identity(other), other);
    validate_provider_execution_reports(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&execution),
        &required,
    )
    .unwrap();
    validate_foreign_stack_contribution(
        closed,
        machine_code::ProviderExecutionRecord {
            provider_plan_report_identity: 7,
            provider_execution_report_identity: 11,
            provider_execution_report_fingerprint: 12,
            normalized_root_report_identity: 13,
            boundary_contract_report_fingerprint: 14,
        },
        selected,
        7,
        [2; 32],
        std::slice::from_ref(&plan),
    )
    .unwrap();
    let mut changed = plan.clone();
    changed.external_requirement_applications[0].provider_plan_digest =
        effects::provider_plan::ProviderPlanDigest::from_digest([3; 32]);
    assert!(
        validate_provider_execution_reports(
            &[changed],
            std::slice::from_ref(&execution),
            &required
        )
        .is_err()
    );
    let mut changed = plan.clone();
    changed.external_requirement_applications[0].requirement_identity =
        identities.index_of(other).unwrap();
    assert!(
        validate_provider_execution_reports(
            &[changed],
            std::slice::from_ref(&execution),
            &required
        )
        .is_err()
    );
    let mut changed_execution = execution;
    changed_execution.requirement_identity = other.into();
    assert!(validate_provider_execution_reports(&[plan], &[changed_execution], &required).is_err());
}

struct IdentityFixture<'a> {
    terminal_marker: u8,
    target: target::NativeTarget,
    object_text: &'a [u8],
    image_bytes: &'a [u8],
    file_name: &'a str,
    callback_report_fingerprint: u64,
    inventory_marker: u8,
    provider_closure_marker: u8,
    foreign_stack_marker: u8,
    provider_plan_marker: u8,
    requirement: &'a str,
    execution_report_fingerprint: u64,
    terminal_policy_marker: u8,
    permission_policy_present: bool,
    boundary_application_marker: Option<u8>,
    physical_evidence_scope: NativePhysicalEvidenceScope,
    physical_evidence_marker: u8,
    physical_evidence_gap_marker: Option<u8>,
    with_validation_evidence: bool,
}

impl Default for IdentityFixture<'static> {
    fn default() -> Self {
        Self {
            terminal_marker: 1,
            target: target::NativeTarget::linux_x64(),
            object_text: b"object text",
            image_bytes: b"executable image",
            file_name: "omega-program",
            callback_report_fingerprint: 29,
            inventory_marker: 2,
            provider_closure_marker: 3,
            foreign_stack_marker: 71,
            provider_plan_marker: 59,
            requirement: "core::Console::write",
            execution_report_fingerprint: 41,
            terminal_policy_marker: 67,
            permission_policy_present: true,
            boundary_application_marker: Some(73),
            physical_evidence_scope:
                NativePhysicalEvidenceScope::UnoptimizedCompleteBoundaryEvidence,
            physical_evidence_marker: 61,
            physical_evidence_gap_marker: None,
            with_validation_evidence: true,
        }
    }
}

fn fixture_identity(fixture: IdentityFixture<'_>) -> NativeArtifactIdentity {
    let plans = vec![NativeSelectedProviderPlan::new(
        7,
        NativeSelectedProviderPlanDigest::from_digest([fixture.provider_plan_marker; 32]),
        vec![fixture.requirement.to_owned()],
    )];
    let executions = vec![NativeProviderExecution {
        requirement_identity: fixture.requirement.to_owned(),
        provider_plan_report_identity: 7,
        provider_execution_report_identity: 11,
        provider_execution_report_fingerprint: fixture.execution_report_fingerprint,
        normalized_root_report_identity: 17,
        boundary_contract_report_fingerprint: 19,
    }];
    let evidence = fixture.with_validation_evidence.then_some(([5; 32], 43));
    derive_native_artifact_identity(NativeArtifactIdentityFields {
        terminal_artifact_identity: [fixture.terminal_marker; 32],
        target: fixture.target,
        object_text_bytes: fixture.object_text,
        image_bytes: fixture.image_bytes,
        final_text_bytes: b"final text",
        image_subsystem: None,
        output_file_name: fixture.file_name,
        output_format: "elf",
        output_counts: [13, 17, 19, 23, 29, 31, 37, 41],
        callback_placement_identity_report_fingerprint: fixture.callback_report_fingerprint,
        final_image_symbol_digest: [47; 32],
        executable_region_inventory_digest: [fixture.inventory_marker; 32],
        executable_region_inventory_report_fingerprint: 53,
        compiler_text_validation_digest: evidence.map(|(digest, _)| digest),
        compiler_function_validation: evidence,
        compiler_entry_region_binding: evidence,
        compiler_entry_footprint_binding: evidence,
        selected_provider_closure_digest: [fixture.provider_closure_marker; 32],
        foreign_call_custody_digest: [fixture.foreign_stack_marker; 32],
        selected_provider_plans: &plans,
        provider_executions: &executions,
        terminal_authority_policy_identity: TerminalAuthorityPolicyIdentity::from_parts(
            1,
            [fixture.terminal_policy_marker; 32],
        ),
        terminal_authority_permission_policy_identity: fixture.permission_policy_present.then(
            || {
                TerminalAuthorityPermissionPolicyIdentity::from_parts(
                    1,
                    [fixture.terminal_policy_marker.wrapping_add(1); 32],
                )
            },
        ),
        terminal_authority_closure_review_identity: [fixture.terminal_policy_marker.wrapping_add(2);
            32],
        boundary_application_coverage_identity: fixture
            .boundary_application_marker
            .map(|marker| [marker; 32]),
        physical_evidence_scope: &fixture.physical_evidence_scope,
        physical_evidence_identity: Some([fixture.physical_evidence_marker; 32]),
        physical_evidence_gap_identity: fixture
            .physical_evidence_gap_marker
            .map(|marker| [marker; 32]),
    })
}

#[test]
fn compact_equal_execution_cannot_substitute_an_exact_requirement() {
    let selected = vec![NativeSelectedProviderPlan::new(
        7,
        NativeSelectedProviderPlanDigest::from_digest([23; 32]),
        vec!["core::Expected".to_owned()],
    )];
    let required = BTreeSet::from([("core::Expected".to_owned(), 7, 11, 13, 17, 19)]);
    let substituted = vec![NativeProviderExecution {
        requirement_identity: "core::Substitute".to_owned(),
        provider_plan_report_identity: 7,
        provider_execution_report_identity: 11,
        provider_execution_report_fingerprint: 13,
        normalized_root_report_identity: 17,
        boundary_contract_report_fingerprint: 19,
    }];

    assert_eq!(
        validate_provider_execution_reports(&selected, &substituted, &required),
        Err("native artifact provider execution is absent from its selected plan"),
    );
}

#[test]
fn compact_equal_foreign_stack_plan_cannot_substitute_a_strong_commitment() {
    let execution = machine_code::ProviderExecutionRecord::new(7, 11, 13, 17, 19)
        .expect("nonzero execution record");
    let selected = vec![NativeSelectedProviderPlan::new(
        7,
        NativeSelectedProviderPlanDigest::from_digest([29; 32]),
        vec!["core::Expected".to_owned()],
    )];

    assert_eq!(
        validate_foreign_stack_contribution(
            "core::Expected",
            execution,
            "core::Expected",
            7,
            [23; 32],
            &selected,
        ),
        Err(
            "native artifact foreign stack contribution disagrees with the exact selected provider plan"
        ),
    );
}

#[test]
fn native_artifact_identity_binds_terminal_target_and_exact_native_bytes() {
    let baseline = fixture_identity(IdentityFixture::default());
    for mutation in [
        fixture_identity(IdentityFixture {
            terminal_marker: 2,
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            target: target::NativeTarget::macos_arm64(),
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            object_text: b"changed object text",
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            image_bytes: b"changed executable image",
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            file_name: "renamed-program",
            ..IdentityFixture::default()
        }),
    ] {
        assert_ne!(mutation, baseline);
    }
}

#[test]
fn native_artifact_identity_binds_evidence_and_provider_realization() {
    let baseline = fixture_identity(IdentityFixture::default());
    for mutation in [
        fixture_identity(IdentityFixture {
            callback_report_fingerprint: 31,
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            inventory_marker: 7,
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            provider_closure_marker: 11,
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            foreign_stack_marker: 73,
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            provider_plan_marker: 13,
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            requirement: "core::Console::substitute",
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            execution_report_fingerprint: 43,
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            terminal_policy_marker: 71,
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            permission_policy_present: false,
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            boundary_application_marker: None,
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            with_validation_evidence: false,
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            physical_evidence_scope: NativePhysicalEvidenceScope::Unavailable,
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            physical_evidence_marker: 17,
            ..IdentityFixture::default()
        }),
        fixture_identity(IdentityFixture {
            physical_evidence_gap_marker: Some(23),
            ..IdentityFixture::default()
        }),
    ] {
        assert_ne!(mutation, baseline);
    }
}

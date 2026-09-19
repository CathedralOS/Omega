use super::{
    BTreeSet, NativeArtifactIdentity, NativeArtifactIdentityFields, NativePhysicalEvidenceScope,
    NativeProviderExecution, NativeSelectedProviderPlan, NativeSelectedProviderPlanDigest,
    TerminalAuthorityPermissionPolicyIdentity, TerminalAuthorityPolicyIdentity,
    derive_native_artifact_identity, validate_foreign_stack_contribution,
    validate_provider_execution_reports,
};
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
fn native_artifact_identity_is_stable_and_canonical() {
    let first = fixture_identity(IdentityFixture::default());
    let replay = fixture_identity(IdentityFixture::default());

    assert_eq!(first, replay);
    assert_eq!(first.to_string().len(), 64);
    assert_eq!(format!("{first:?}"), first.to_string());
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

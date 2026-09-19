use super::artifacts::RetainedContainerProof;
use super::{
    AdmissionReceiptId, Architecture, Artifact, ArtifactAdmissionEvidence, ArtifactEntry,
    ArtifactId, ArtifactRelocationKind, CodePlacement, CodePlacementAuthority, CodePlacementId,
    DecodedArtifactRelocation, EntrySetId, FinalValidationCertificate, FinalValidationId,
    InstallAuthority, InstallationAudience, InstallationReceipt, InstallationScopeId,
    InstalledCode, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MappingQuarantineCause, MappingQuarantineId, MappingQuarantineReceipt, MaterializationReceipt,
    PlacementConstraints, PlacementPlanId, RelocationSetId, RelocationTarget, RetirementAuthority,
    RetirementFactDigest, RetirementReceipt, UninstallOutcome, ValidatedPlacement, WxEnforcement,
    admit_executable, install_validated, materialize_admitted_artifact, materialize_and_freeze,
    normalized_proof_payload_digest, quarantine_installed, retire_installed, uninstall_installed,
    validate_final_placement,
};
use extents::{Extent, ExtentLineageId, ExtentRootGrant, MappingEraId};
use layout_plans::{ArtifactInstallationScopeId, PlacementPhase};

use super::test_support::*;
use extents::{AddressSpaceId, ExtentProvenanceId, ExtentRights};
use layout_plans::{MachineRegimeId, PlacementAddressRange, PlacementSite};

#[test]
fn canonical_materializer_patches_x86_relative_targets_and_binds_the_receipt() {
    let target = RelocationTarget::Entry(entry_id(1001));
    let mut code = vec![0x90; 64];
    code[0] = 0xe8;
    code[1..5].fill(0);
    let candidate = relocatable_artifact(
        1,
        Architecture::X86_64,
        code,
        vec![DecodedArtifactRelocation {
            kind: ArtifactRelocationKind::X86Relative32,
            destination_offset: 1,
            target,
            addend: -4,
        }],
    );
    let admitted = admit(&candidate);
    let placement = placement_authority(100, 0x1000, 4096)
        .claim(placement_extent(100, 0x1000, 4096))
        .expect("placement");

    let materialized = materialize_admitted_artifact(&admitted, &placement, |candidate| {
        (candidate == target).then_some(0x1024)
    })
    .expect("materialized bytes");
    assert_eq!(
        i32::from_le_bytes(materialized.bytes()[1..5].try_into().unwrap()),
        0x1b
    );

    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            id(31, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("receipt is bound to canonical materializer output");
    assert_eq!(frozen.bytes(), materialized.bytes());
    assert_eq!(frozen.final_bytes(), materialized.final_bytes());

    let certificate = FinalValidationCertificate::from_validator(
        id(181, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated = validate_final_placement(frozen, &certificate).unwrap();
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        id(281, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    let installed = install_validated(validated, authority, receipt).unwrap();
    assert!(
        installed.binds_exact_materialized_artifact_bytes(candidate.code(), materialized.bytes())
    );
    let mut changed_source = candidate.code().to_vec();
    changed_source[0] ^= 1;
    assert!(
        !installed.binds_exact_materialized_artifact_bytes(&changed_source, materialized.bytes())
    );
    let mut changed_final = materialized.bytes().to_vec();
    changed_final[1] ^= 1;
    assert!(!installed.binds_exact_materialized_artifact_bytes(candidate.code(), &changed_final));
    assert!(
        installed.binds_exact_materialized_entry_bytes(entry_id(1001), &materialized.bytes()[..8])
    );
    let mut changed_entry = materialized.bytes()[..8].to_vec();
    changed_entry[1] ^= 1;
    assert!(!installed.binds_exact_materialized_entry_bytes(entry_id(1001), &changed_entry));
    assert!(
        !installed.binds_exact_materialized_entry_bytes(entry_id(1002), &materialized.bytes()[..8])
    );
    assert!(!installed.binds_exact_materialized_entry_bytes(entry_id(1001), &[]));
    assert!(!installed.binds_exact_materialized_entry_bytes(entry_id(1001), &[0; 65]));
}

#[test]
fn aarch64_materialization_validates_the_relocated_instruction_shape() {
    let target = RelocationTarget::Entry(entry_id(1002));
    let mut branch = vec![0; 64];
    branch[..4].copy_from_slice(&0x9400_0000u32.to_le_bytes());
    let candidate = relocatable_artifact(
        2,
        Architecture::Aarch64,
        branch,
        vec![DecodedArtifactRelocation {
            kind: ArtifactRelocationKind::Aarch64Branch26,
            destination_offset: 0,
            target,
            addend: 0,
        }],
    );
    let admitted = admit(&candidate);
    let placement = placement_authority(101, 0x1000, 4096)
        .claim(placement_extent(101, 0x1000, 4096))
        .expect("placement");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| Some(0x1010))
        .expect("AArch64 branch materialization");
    assert_eq!(
        u32::from_le_bytes(materialized.bytes()[..4].try_into().unwrap()),
        0x9400_0004
    );

    let invalid = relocatable_artifact(
        3,
        Architecture::Aarch64,
        vec![0; 64],
        vec![DecodedArtifactRelocation {
            kind: ArtifactRelocationKind::Aarch64Branch26,
            destination_offset: 0,
            target,
            addend: 0,
        }],
    );
    let invalid = admit(&invalid);
    let error = materialize_admitted_artifact(&invalid, &placement, |_| Some(0x1010))
        .expect_err("relocation cannot rewrite an arbitrary instruction");
    assert!(error.0.contains("B/BL"));
}

#[test]
fn aarch64_materialization_patches_page_pairs_and_absolute_data() {
    let target = RelocationTarget::Entry(entry_id(1004));
    let mut code = vec![0; 64];
    code[..4].copy_from_slice(&0x9000_0000u32.to_le_bytes());
    code[4..8].copy_from_slice(&0x9100_0000u32.to_le_bytes());
    let candidate = relocatable_artifact(
        4,
        Architecture::Aarch64,
        code,
        vec![
            DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::Aarch64Page21,
                destination_offset: 0,
                target,
                addend: 0,
            },
            DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::Aarch64PageOffset12,
                destination_offset: 4,
                target,
                addend: 0,
            },
            DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::Absolute64,
                destination_offset: 8,
                target,
                addend: -6,
            },
        ],
    );
    let admitted = admit(&candidate);
    let placement = placement_authority(102, 0x1000, 4096)
        .claim(placement_extent(102, 0x1000, 4096))
        .expect("placement");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| Some(0x3456))
        .expect("AArch64 page-pair materialization");

    assert_eq!(
        u32::from_le_bytes(materialized.bytes()[..4].try_into().unwrap()),
        0xd000_0000
    );
    assert_eq!(
        u32::from_le_bytes(materialized.bytes()[4..8].try_into().unwrap()),
        0x9111_5800
    );
    assert_eq!(
        u64::from_le_bytes(materialized.bytes()[8..16].try_into().unwrap()),
        0x3450
    );
}

#[test]
fn admitted_artifact_is_reusable_but_each_placement_is_linear() {
    let candidate = artifact(1);
    let admitted = admit(&candidate);
    let second_reference = admitted.clone();

    let frozen = frozen(&admitted, 100, 0x1000);
    let certificate = certificate(&frozen, 180);
    let validated = validate_final_placement(frozen, &certificate).expect("validated placement");
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        id(200, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    let installed = install_validated(validated, authority, receipt).expect("installed code");
    assert_eq!(installed.artifact(), second_reference.artifact().identity());
    assert_eq!(installed.wx(), WxEnforcement::HardwareEnforced);
}

#[test]
fn installation_receipt_cannot_substitute_colliding_normalized_artifact() {
    let first = admit(&colliding_artifact(1, 0x90));
    let second = admit(&colliding_artifact(1, 0xcc));

    let first_frozen = frozen(&first, 114, 0xc000);
    let first_certificate = certificate(&first_frozen, 194);
    let first_validated = validate_final_placement(first_frozen, &first_certificate)
        .expect("first validated placement");

    let second_frozen = frozen(&second, 114, 0xc000);
    let second_certificate = certificate(&second_frozen, 194);
    let second_validated = validate_final_placement(second_frozen, &second_certificate)
        .expect("second validated placement");

    let authority = InstallAuthority::from_admitted_provider(&first_validated);
    let substituted_receipt = InstallationReceipt::from_provider(
        id(314, InstalledCodeId::from_normalized_identity),
        &second_validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    let error = install_validated(first_validated, authority, substituted_receipt)
        .expect_err("exact frozen bytes must outrank colliding report identities");
    assert!(error.diagnostic().0.contains("receipt"));
}

#[test]
fn artifact_content_digest_is_derived_from_exact_semantics() {
    let first = colliding_artifact(1, 0x90);
    let second = colliding_artifact(1, 0xcc);

    assert_eq!(first.identity(), second.identity());
    assert_ne!(first.code(), second.code());
    assert_ne!(first.content(), second.content());
    assert_ne!(first.content().digest(), second.content().digest());
}

#[test]
fn local_fnv_sites_are_explicitly_non_authoritative() {
    let root_source = [
        include_str!("../executable_installation.rs"),
        include_str!("authority_digests.rs"),
        include_str!("artifacts.rs"),
        include_str!("code_placement.rs"),
        include_str!("installation.rs"),
        include_str!("retirement.rs"),
    ]
    .concat();
    let root = root_source.as_str();
    let writer = include_str!("post_handoff_writer.rs");
    let container = include_str!("container.rs");
    let container_bytes = include_str!("container_bytes.rs");
    let materializer = include_str!("materializer.rs");

    for forbidden in [
        ["normalized_id!", "(ArtifactContent"].concat(),
        ["normalized_id!", "(ProofPayload"].concat(),
        ["normalized_id!", "(FinalBytes"].concat(),
        ["pub const fn", " from_digest"].concat(),
    ] {
        assert!(!root.contains(&forbidden));
    }
    assert!(writer.contains("non_authoritative_post_handoff_entry_writer_context_fingerprint"));
    assert!(container.contains("NonAuthoritativeContainerFingerprint64"));
    assert!(container_bytes.contains("non_authoritative_informational_section_fingerprint"));
    assert!(materializer.contains("FinalBytesDigest"));

    let fnv_offset_basis = ["0x", "cbf"].concat();
    let fnv_offset_basis_count = [root, writer, container, container_bytes, materializer]
        .into_iter()
        .map(|source| source.matches(&fnv_offset_basis).count())
        .sum::<usize>();
    assert_eq!(
        fnv_offset_basis_count, 4,
        "new FNV sites require explicit non-authoritative classification"
    );
}

#[test]
fn materialization_cannot_substitute_another_artifact() {
    let first = admit(&artifact(1));
    let second = admit(&artifact(2));
    let placement = placement_authority(101, 0x2000, 4096)
        .claim(placement_extent(101, 0x2000, 4096))
        .expect("placement");
    let first_materialized = materialize_admitted_artifact(&first, &placement, |_| None)
        .expect("first artifact materialization");
    let second_materialized = materialize_admitted_artifact(&second, &placement, |_| None)
        .expect("second artifact materialization");
    let error = materialize_and_freeze(
        &first,
        placement,
        first_materialized,
        MaterializationReceipt::from_materialized(
            &second_materialized,
            id(71, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect_err("artifact substitution rejects");
    assert!(error.diagnostic().0.contains("exact canonical output"));
    let (_placement, _materialized, _receipt) = (*error).into_parts();
}

#[test]
fn canonical_materializer_output_cannot_substitute_another_artifact() {
    let first = admit(&colliding_artifact(1, 0x90));
    let second = admit(&colliding_artifact(1, 0xcc));
    let placement = placement_authority(111, 0x9000, 4096)
        .claim(placement_extent(111, 0x9000, 4096))
        .expect("placement");
    let materialized = materialize_admitted_artifact(&second, &placement, |_| None)
        .expect("second artifact materialization");
    let receipt = MaterializationReceipt::from_materialized(
        &materialized,
        id(71, MachineFootprintId::from_normalized_identity),
        true,
    );
    let error = materialize_and_freeze(&first, placement, materialized, receipt)
        .expect_err("canonical output substitution rejects");
    assert!(
        error
            .diagnostic()
            .0
            .contains("materializer output does not retain the exact admitted artifact")
    );
    let (_placement, _materialized, _receipt) = (*error).into_parts();
}

#[test]
fn final_bytes_digest_binds_content_and_placement_base() {
    let admitted = admit(&artifact(1));
    let first_placement = placement_authority(114, 0x4000, 4096)
        .claim(placement_extent(114, 0x4000, 4096))
        .expect("first placement");
    let second_placement = placement_authority(115, 0x8000, 4096)
        .claim(placement_extent(115, 0x8000, 4096))
        .expect("second placement");
    let first = materialize_admitted_artifact(&admitted, &first_placement, |_| None)
        .expect("first materialization");
    let second = materialize_admitted_artifact(&admitted, &second_placement, |_| None)
        .expect("second materialization");

    assert_eq!(first.bytes(), second.bytes());
    assert_ne!(first.base_address(), second.base_address());
    assert_ne!(first.final_bytes(), second.final_bytes());
    assert_ne!(first.final_bytes().digest(), second.final_bytes().digest());
}

#[test]
fn admission_evidence_cannot_substitute_placement_constraints() {
    let candidate = artifact(1);
    let weaker = PlacementConstraints::unconstrained(PlacementPhase::PostHandoff);
    let substituted = artifact_with(
        1,
        weaker,
        candidate.0.entry_set,
        candidate.0.entries[0].identity,
    );
    let error = admit_executable(
        &candidate,
        ArtifactAdmissionEvidence::from_validator(
            id(40, AdmissionReceiptId::from_normalized_identity),
            &substituted,
            true,
        ),
    )
    .expect_err("admission evidence must pin the decoded placement constraints");
    assert!(error.0.contains("does not match canonical candidate"));
}

#[test]
fn admission_evidence_cannot_substitute_the_selected_entry_set() {
    let candidate = artifact(1);
    let substituted = artifact_with(
        1,
        candidate.0.placement_constraints,
        id(34, EntrySetId::from_normalized_identity),
        candidate.0.entries[0].identity,
    );
    let error = admit_executable(
        &candidate,
        ArtifactAdmissionEvidence::from_validator(
            id(40, AdmissionReceiptId::from_normalized_identity),
            &substituted,
            true,
        ),
    )
    .expect_err("admission evidence must pin the decoded entry set");
    assert!(error.0.contains("does not match canonical candidate"));
}

#[test]
fn admitted_artifact_selects_only_its_canonical_entry_targets() {
    let candidate = artifact(1);
    let selected = entry_id(1001);
    let admitted = admit(&candidate);
    assert_eq!(
        admitted
            .selected_entry_target(selected)
            .expect("selected entry target"),
        RelocationTarget::Entry(selected)
    );
    let foreign = entry_id(1002);
    assert!(admitted.selected_entry_target(foreign).is_err());
}

#[test]
fn final_certificate_is_bound_to_one_placement_and_final_bytes() {
    let admitted = admit(&artifact(1));
    let target = frozen(&admitted, 102, 0x3000);
    let foreign = frozen(&admitted, 103, 0x3000);
    let certificate = certificate(&foreign, 183);
    let error =
        validate_final_placement(target, &certificate).expect_err("certificate transplant rejects");
    assert!(error.diagnostic().0.contains("does not match"));
}

#[test]
fn final_certificate_retains_the_exact_frozen_byte_snapshot() {
    let admitted = admit(&artifact(1));
    let frozen = frozen(&admitted, 113, 0xb000);
    let mut certificate = certificate(&frozen, 193);
    certificate.final_bytes[0] ^= 1;

    let error = validate_final_placement(frozen, &certificate)
        .expect_err("a certificate for substituted exact bytes must reject");
    assert!(error.diagnostic().0.contains("does not match"));
}

#[test]
fn unsupported_execute_transition_preserves_all_linear_inputs() {
    let admitted = admit(&artifact(1));
    let frozen = frozen(&admitted, 104, 0x4000);
    let certificate = certificate(&frozen, 184);
    let validated = validate_final_placement(frozen, &certificate).expect("validated placement");
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        id(204, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::Unsupported,
    );
    let error =
        install_validated(validated, authority, receipt).expect_err("unsupported provider rejects");
    assert!(error.diagnostic().0.contains("does not support"));
    let (_validated, _authority, _receipt) = (*error).into_parts();
}

#[test]
fn materialization_uses_admitted_artifact_size_not_a_caller_hint() {
    let admitted = admit(&artifact(1));
    let placement = placement_authority(105, 0x5000, 32)
        .claim(placement_extent(105, 0x5000, 32))
        .expect("qualified but undersized destination");
    let error = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect_err("artifact cannot fit");
    assert!(error.0.contains("smaller"));
}

#[test]
fn materialization_rejects_placement_constraint_substitution() {
    let admitted = admit(&artifact(1));
    let substituted = PlacementConstraints::new(None, 1, PlacementPhase::PostHandoff, None, None)
        .expect("weaker substituted constraints");
    let placement = placement_authority_with_constraints(109, 0x8000, 4096, substituted)
        .claim(placement_extent(109, 0x8000, 4096))
        .expect("substituted constraints independently accept the site");
    let error = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect_err("provider cannot substitute weaker placement constraints");
    assert!(error.0.contains("constraints do not match"));
}

#[test]
fn failed_placement_claim_returns_extent_and_one_shot_authority() {
    let extent = ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(106),
        extent_id(106, ExtentLineageId::from_normalized_identity),
        extent_id(50, AddressSpaceId::from_normalized_identity),
        ExtentRights::none(),
        extent_id(52, ExtentProvenanceId::from_normalized_identity),
        extent_id(53, MappingEraId::from_normalized_identity),
    )
    .mint(0x6000, 4096)
    .expect("placement extent without required rights");
    let error = placement_authority(106, 0x6000, 4096)
        .claim(extent)
        .expect_err("missing placement right");
    assert!(error.diagnostic().0.contains("exact range"));
    let (_authority, extent) = (*error).into_parts();
    assert_eq!(extent.base(), 0x6000);
}

#[test]
fn placement_authority_rejects_same_address_from_another_lineage() {
    let admitted_extent = placement_extent(116, 0xe000, 4096);
    let substituted_extent = placement_extent(117, 0xe000, 4096);
    let authority = CodePlacementAuthority::from_admitted_provider(
        id(116, CodePlacementId::from_normalized_identity),
        id(61, InstallationScopeId::from_normalized_identity),
        InstallationAudience::FutureFetcher,
        &admitted_extent,
        rights(&[51]),
        artifact_placement_constraints(),
        PlacementSite {
            base_address: 0xe000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: Some(
                ArtifactInstallationScopeId::from_normalized_identity(61)
                    .expect("installation scope"),
            ),
        },
    );

    let error = authority
        .claim(substituted_extent)
        .expect_err("same address and rights do not imply the same range authority");
    assert!(error.diagnostic().0.contains("lineage"));
}

#[test]
fn retirement_requires_quiescence_then_returns_writable_placement() {
    let admitted = admit(&artifact(1));
    let installed = installed_code(&admitted, 107, 0x7000);
    let retirement_fact =
        RetirementFactDigest::from_canonical_bytes(b"provider.timer-drain.complete.v1");
    let authority = RetirementAuthority::from_admitted_provider(&installed, [retirement_fact]);
    let receipt =
        RetirementReceipt::from_provider(&installed, false, true, true, [retirement_fact]);
    let error =
        retire_installed(installed, authority, receipt).expect_err("visibility is not quiescence");
    assert!(error.diagnostic().0.contains("quiescence"));
    let (installed, authority, _) = (*error).into_parts();

    let receipt = RetirementReceipt::from_provider(&installed, true, true, true, [retirement_fact]);
    let retired = retire_installed(installed, authority, receipt).expect("retired code");
    assert_eq!(
        retired.previous_artifact().artifact().identity(),
        admitted.artifact().identity()
    );

    let replacement = admit(&artifact(2));
    let placement = retired.into_placement();
    let materialized = materialize_admitted_artifact(&replacement, &placement, |_| None)
        .expect("replacement materialization");
    materialize_and_freeze(
        &replacement,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            id(71, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("placement reusable only after quiescent retirement");
}

#[test]
fn uninstall_join_retires_a_complete_drain() {
    let spec = authentic_spec();
    let outcome = uninstall_installed(
        realize(&spec),
        RetirementAuthority::from_admitted_provider(&realize(&spec), std::iter::empty()),
        RetirementReceipt::from_provider(&realize(&spec), true, true, true, std::iter::empty()),
        None,
    )
    .expect("a complete drain retires through the join");
    let UninstallOutcome::Retired(retired) = outcome else {
        panic!("a complete drain retires rather than quarantining")
    };
    assert_eq!(
        retired.previous_artifact().artifact().identity(),
        id(spec.artifact, ArtifactId::from_normalized_identity)
    );
    let _placement = retired.into_placement();
}

#[test]
fn uninstall_join_quarantines_an_incomplete_drain() {
    let spec = authentic_spec();
    let installed = realize(&spec);
    let installed_identity = installed.identity();
    let installed_context = installed.receipt_context();
    let quarantine = id(401, MappingQuarantineId::from_normalized_identity);
    let outcome = uninstall_installed(
        installed,
        RetirementAuthority::from_admitted_provider(&realize(&spec), std::iter::empty()),
        RetirementReceipt::from_provider(&realize(&spec), false, true, true, std::iter::empty()),
        Some(MappingQuarantineReceipt::from_provider(
            &realize(&spec),
            quarantine,
            true,
            true,
            true,
            MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 2,
            },
        )),
    )
    .expect("an incomplete drain parks in quarantine");
    let UninstallOutcome::Quarantined(quarantined) = outcome else {
        panic!("an incomplete drain must not return the placement")
    };
    assert_eq!(quarantined.installed_code(), installed_identity);
    assert_eq!(quarantined.attributed_capacity_loss(), 4096);
    assert!(matches!(
        quarantined.cause(),
        MappingQuarantineCause::IncompleteDrain {
            residual_authority_count: 2
        }
    ));
    let fault = quarantined
        .stale_entry_fault(&installed_context)
        .expect("a stale entry names the quarantined realization");
    assert_eq!(fault.quarantine(), quarantine);
    assert!(!fault.discharged_obligations());
}

#[test]
fn uninstall_join_returns_every_input_without_quarantine_evidence() {
    let spec = authentic_spec();
    let error = uninstall_installed(
        realize(&spec),
        RetirementAuthority::from_admitted_provider(&realize(&spec), std::iter::empty()),
        RetirementReceipt::from_provider(&realize(&spec), false, true, true, std::iter::empty()),
        None,
    )
    .expect_err("a failed drain without quarantine evidence keeps every input");
    assert!(error.retirement_diagnostic().0.contains("quiescence"));
    assert!(error.quarantine_diagnostic().is_none());
    let (installed, authority, _retirement, quarantine) = (*error).into_parts();
    assert!(quarantine.is_none());

    // The returned custody is intact: the same authority with a complete
    // drain retires the returned installed code.
    let receipt =
        RetirementReceipt::from_provider(&installed, true, true, true, std::iter::empty());
    retire_installed(installed, authority, receipt).expect("returned custody still retires");
}

#[test]
fn uninstall_join_rejects_a_quarantine_receipt_naming_another_realization() {
    let spec = authentic_spec();
    let mut foreign_spec = authentic_spec();
    foreign_spec.placement = 108;
    let error = uninstall_installed(
        realize(&spec),
        RetirementAuthority::from_admitted_provider(&realize(&spec), std::iter::empty()),
        RetirementReceipt::from_provider(&realize(&spec), false, true, true, std::iter::empty()),
        Some(MappingQuarantineReceipt::from_provider(
            &realize(&foreign_spec),
            id(401, MappingQuarantineId::from_normalized_identity),
            true,
            true,
            true,
            MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 1,
            },
        )),
    )
    .expect_err("a foreign quarantine receipt cannot park this realization");
    assert!(error.retirement_diagnostic().0.contains("quiescence"));
    assert!(
        error
            .quarantine_diagnostic()
            .expect("the quarantine leg ran and refused")
            .0
            .contains("does not match")
    );
    let (installed, authority, _retirement, quarantine) = (*error).into_parts();
    assert!(quarantine.is_some());

    let receipt =
        RetirementReceipt::from_provider(&installed, true, true, true, std::iter::empty());
    retire_installed(installed, authority, receipt).expect("returned custody still retires");
}

#[test]
fn incomplete_drain_quarantines_capacity_without_returning_placement() {
    let admitted = admit(&artifact(1));
    let installed = installed_code(&admitted, 117, 0xe000);
    let installed_identity = installed.identity();
    let installed_context = installed.receipt_context();
    let quarantine = id(401, MappingQuarantineId::from_normalized_identity);
    let receipt = MappingQuarantineReceipt::from_provider(
        &installed,
        quarantine,
        true,
        true,
        true,
        MappingQuarantineCause::IncompleteDrain {
            residual_authority_count: 2,
        },
    );

    let quarantined = quarantine_installed(installed, receipt).expect("fail-closed quarantine");
    assert_eq!(quarantined.installed_code(), installed_identity);
    assert_eq!(quarantined.attributed_capacity_loss(), 4096);
    assert!(matches!(
        quarantined.cause(),
        MappingQuarantineCause::IncompleteDrain {
            residual_authority_count: 2
        }
    ));
    let fault = quarantined
        .stale_entry_fault(&installed_context)
        .expect("stale entry names quarantined realization");
    assert_eq!(fault.quarantine(), quarantine);
    assert!(!fault.discharged_obligations());
}

#[test]
fn quarantine_requires_execute_removal_unmapping_and_reservation() {
    let admitted = admit(&artifact(1));
    let installed = installed_code(&admitted, 118, 0xf000);
    let quarantine = id(402, MappingQuarantineId::from_normalized_identity);
    let incomplete = MappingQuarantineReceipt::from_provider(
        &installed,
        quarantine,
        true,
        false,
        true,
        MappingQuarantineCause::PossibleOpaqueHolder {
            provider_identity: "OpaqueCodec".into(),
        },
    );
    let error = quarantine_installed(installed, incomplete)
        .expect_err("still-mapped range cannot become quarantine");
    assert!(error.diagnostic().0.contains("unmapped/trapping"));
    let (installed, _) = (*error).into_parts();

    let complete = MappingQuarantineReceipt::from_provider(
        &installed,
        quarantine,
        true,
        true,
        true,
        MappingQuarantineCause::PossibleOpaqueHolder {
            provider_identity: "OpaqueCodec".into(),
        },
    );
    let quarantined =
        quarantine_installed(installed, complete).expect("opaque holder stays quarantined");
    assert!(matches!(
        quarantined.cause(),
        MappingQuarantineCause::PossibleOpaqueHolder { provider_identity }
            if provider_identity == "OpaqueCodec"
    ));
}

#[test]
fn quarantine_fault_rejects_an_unrelated_installed_identity() {
    let admitted = admit(&artifact(1));
    let installed = installed_code(&admitted, 119, 0xc000);
    let receipt = MappingQuarantineReceipt::from_provider(
        &installed,
        id(403, MappingQuarantineId::from_normalized_identity),
        true,
        true,
        true,
        MappingQuarantineCause::IncompleteDrain {
            residual_authority_count: 1,
        },
    );
    let quarantined = quarantine_installed(installed, receipt).expect("quarantined");
    let unrelated = installed_code(&admit(&artifact(2)), 999, 0xd000).receipt_context();
    assert!(
        quarantined
            .stale_entry_fault(&unrelated)
            .expect_err("unrelated identity is not this stale entry")
            .0
            .contains("does not name")
    );
}

#[test]
fn quarantine_fault_rejects_a_collision_equal_report_identity() {
    let first = admit(&colliding_artifact(1, 0x90));
    let second = admit(&colliding_artifact(1, 0xcc));
    let first_installed = installed_code(&first, 120, 0xd000);
    let second_installed = installed_code(&second, 120, 0xd000);
    let exact_context = first_installed.receipt_context();
    let colliding_context = second_installed.receipt_context();
    assert_eq!(
        first_installed.identity(),
        second_installed.identity(),
        "the adversary controls a collision-equal compact report identity"
    );

    let receipt = MappingQuarantineReceipt::from_provider(
        &first_installed,
        id(404, MappingQuarantineId::from_normalized_identity),
        true,
        true,
        true,
        MappingQuarantineCause::IncompleteDrain {
            residual_authority_count: 1,
        },
    );
    let quarantined = quarantine_installed(first_installed, receipt).expect("exact quarantine");

    quarantined
        .stale_entry_fault(&exact_context)
        .expect("the exact quarantined realization faults");
    let error = quarantined
        .stale_entry_fault(&colliding_context)
        .expect_err("a compact-ID collision must not forge stale-entry evidence");
    assert!(error.0.contains("does not name"));
}

#[test]
fn quarantine_receipt_rejects_a_collision_equal_installed_realization() {
    let first = admit(&colliding_artifact(1, 0x90));
    let second = admit(&colliding_artifact(1, 0xcc));
    let first_installed = installed_code(&first, 122, 0xd000);
    let second_installed = installed_code(&second, 122, 0xd000);
    assert_eq!(first_installed.identity(), second_installed.identity());

    let substituted_receipt = MappingQuarantineReceipt::from_provider(
        &second_installed,
        id(405, MappingQuarantineId::from_normalized_identity),
        true,
        true,
        true,
        MappingQuarantineCause::IncompleteDrain {
            residual_authority_count: 1,
        },
    );
    let error = quarantine_installed(first_installed, substituted_receipt)
        .expect_err("quarantine must bind the complete installed realization");
    assert!(error.diagnostic().0.contains("does not match"));
    let (first_installed, _) = (*error).into_parts();
    assert!(first_installed.binds_exact_unrelocated_artifact_bytes(&[0x90; 64]));
}

#[test]
fn retirement_completion_facts_are_strong_domain_separated_commitments() {
    let fact = RetirementFactDigest::from_canonical_bytes(b"provider.timer-drain.complete.v1");
    let changed = RetirementFactDigest::from_canonical_bytes(b"provider.timer-drain.complete.v2");
    let proof = normalized_proof_payload_digest(b"provider.timer-drain.complete.v1");

    assert_ne!(
        fact, changed,
        "fact-byte mutation must change the commitment"
    );
    assert_ne!(
        fact.digest(),
        proof.digest(),
        "equal payload bytes in another authority domain must not collide by construction"
    );
}

#[test]
fn retirement_rejects_a_different_exact_completion_fact() {
    let admitted = admit(&artifact(1));
    let installed = installed_code(&admitted, 121, 0xd000);
    let required = RetirementFactDigest::from_canonical_bytes(b"provider.drain.complete.v1");
    let substituted =
        RetirementFactDigest::from_canonical_bytes(b"provider.cache-flush.complete.v1");
    let authority = RetirementAuthority::from_admitted_provider(&installed, [required]);
    let receipt = RetirementReceipt::from_provider(&installed, true, true, true, [substituted]);

    let error = retire_installed(installed, authority, receipt)
        .expect_err("another provider fact cannot discharge retirement");
    assert!(error.diagnostic().0.contains("completion facts"));
}

#[test]
fn placement_claim_validates_actual_extent_site_against_plan_constraints() {
    let error = placement_authority(108, 0x7101, 4096)
        .claim(placement_extent(108, 0x7101, 4096))
        .expect_err("misaligned site rejects before materialization");
    assert!(error.diagnostic().0.contains("not aligned"));
    let (_authority, extent) = (*error).into_parts();
    assert_eq!(extent.base(), 0x7101);
}

#[test]
fn retirement_receipt_cannot_substitute_colliding_installed_realization() {
    let first = admit(&colliding_artifact(1, 0x90));
    let second = admit(&colliding_artifact(1, 0xcc));
    let first_installed = installed_code(&first, 115, 0xd000);
    let second_installed = installed_code(&second, 115, 0xd000);

    let authority =
        RetirementAuthority::from_admitted_provider(&first_installed, std::iter::empty());
    let substituted_receipt =
        RetirementReceipt::from_provider(&second_installed, true, true, true, std::iter::empty());
    let error = retire_installed(first_installed, authority, substituted_receipt)
        .expect_err("retirement must bind the exact installed realization");
    assert!(error.diagnostic().0.contains("receipt"));
}

/// Every provider-visible choice behind one installed realization: each
/// retained record field the occurrence digest or a lifecycle receipt binds
/// is reconstructible from this spec, so each substitution below is exactly
/// one honest field change replayed through the same ladder.
#[derive(Clone)]
struct RealizationSpec {
    artifact: u64,
    architecture: Architecture,
    code: Vec<u8>,
    entry_set: u64,
    entry: u64,
    entry_offset: u64,
    relocations: Vec<DecodedArtifactRelocation>,
    resolve: fn(RelocationTarget) -> Option<u64>,
    constraints: PlacementConstraints,
    admission_receipt: u64,
    container_proof: Option<RetainedContainerProof>,
    placement: u64,
    scope: u64,
    audience: InstallationAudience,
    extent_issuance: u64,
    extent_lineage: u64,
    extent_space: u64,
    extent_rights: Vec<u64>,
    extent_provenance: u64,
    extent_era: u64,
    extent_base: u64,
    extent_length: u64,
    realized_footprint: u64,
    validation: u64,
    installed: u64,
    wx: WxEnforcement,
}

fn authentic_spec() -> RealizationSpec {
    RealizationSpec {
        artifact: 1,
        architecture: Architecture::X86_64,
        code: vec![0; 64],
        entry_set: 33,
        entry: 1001,
        entry_offset: 16,
        relocations: Vec::new(),
        resolve: |_| None,
        constraints: artifact_placement_constraints(),
        admission_receipt: 40,
        container_proof: Some(RetainedContainerProof {
            digest: normalized_proof_payload_digest(b"container proof payload"),
            bytes: b"container proof payload".to_vec(),
        }),
        placement: 107,
        scope: 61,
        audience: InstallationAudience::FutureFetcher,
        extent_issuance: 107,
        extent_lineage: 107,
        extent_space: 50,
        extent_rights: vec![51],
        extent_provenance: 52,
        extent_era: 53,
        extent_base: 0x7000,
        extent_length: 4096,
        realized_footprint: 71,
        validation: 180,
        installed: 281,
        wx: WxEnforcement::HardwareEnforced,
    }
}

fn spec_constraints(
    range: Option<(u64, u64)>,
    alignment: u64,
    phase: PlacementPhase,
    regime: Option<u64>,
    scope: Option<u64>,
) -> PlacementConstraints {
    PlacementConstraints::new(
        range.map(|(start, end)| PlacementAddressRange::new(start, end).expect("range")),
        alignment,
        phase,
        regime.map(|identity| MachineRegimeId::from_normalized_identity(identity).expect("regime")),
        scope.map(|identity| {
            ArtifactInstallationScopeId::from_normalized_identity(identity).expect("scope")
        }),
    )
    .expect("spec constraints")
}

fn spec_artifact(spec: &RealizationSpec) -> Artifact {
    Artifact::from_canonical_decode(
        id(spec.artifact, ArtifactId::from_normalized_identity),
        spec.architecture,
        spec.code.clone(),
        id(30, MachineContractSetId::from_normalized_identity),
        id(31, MachineFootprintId::from_normalized_identity),
        id(32, PlacementPlanId::from_normalized_identity),
        spec.constraints,
        id(spec.entry_set, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(
            entry_id(spec.entry),
            spec.entry_offset,
        )],
        id(34, RelocationSetId::from_normalized_identity),
        spec.relocations.clone(),
        authority_commitments(spec.constraints),
    )
    .expect("spec artifact")
}

fn spec_extent(spec: &RealizationSpec) -> Extent {
    ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(spec.extent_issuance),
        extent_id(
            spec.extent_lineage,
            ExtentLineageId::from_normalized_identity,
        ),
        extent_id(spec.extent_space, AddressSpaceId::from_normalized_identity),
        rights(&spec.extent_rights),
        extent_id(
            spec.extent_provenance,
            ExtentProvenanceId::from_normalized_identity,
        ),
        extent_id(spec.extent_era, MappingEraId::from_normalized_identity),
    )
    .mint(spec.extent_base, spec.extent_length)
    .expect("spec extent")
}

fn spec_placement(spec: &RealizationSpec) -> CodePlacement {
    // The authority binds the exact extent evidence plus its declared site;
    // the site follows the constraint/spec coordinates so each honest
    // substitution stays internally consistent.
    CodePlacementAuthority::from_admitted_provider(
        id(spec.placement, CodePlacementId::from_normalized_identity),
        id(spec.scope, InstallationScopeId::from_normalized_identity),
        spec.audience,
        &spec_extent(spec),
        rights(&[51]),
        spec.constraints,
        PlacementSite {
            base_address: spec.extent_base,
            phase: spec.constraints.phase(),
            machine_regime: spec.constraints.machine_regime(),
            installation_scope: Some(
                ArtifactInstallationScopeId::from_normalized_identity(spec.scope)
                    .expect("site scope"),
            ),
        },
    )
    .claim(spec_extent(spec))
    .expect("spec placement")
}

fn spec_validated(spec: &RealizationSpec) -> ValidatedPlacement {
    let candidate = spec_artifact(spec);
    let mut admitted = admit_executable(
        &candidate,
        ArtifactAdmissionEvidence::from_validator(
            id(
                spec.admission_receipt,
                AdmissionReceiptId::from_normalized_identity,
            ),
            &candidate,
            true,
        ),
    )
    .expect("spec admission");
    admitted.container_proof = spec.container_proof.clone();
    let placement = spec_placement(spec);
    let materialized = materialize_admitted_artifact(&admitted, &placement, spec.resolve)
        .expect("spec materialization");
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            id(
                spec.realized_footprint,
                MachineFootprintId::from_normalized_identity,
            ),
            true,
        ),
    )
    .expect("spec frozen placement");
    let certificate = FinalValidationCertificate::from_validator(
        id(spec.validation, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    validate_final_placement(frozen, &certificate).expect("spec validated placement")
}

fn realize(spec: &RealizationSpec) -> InstalledCode {
    let validated = spec_validated(spec);
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        id(spec.installed, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        spec.wx,
    );
    install_validated(validated, authority, receipt).expect("installed realization")
}

#[test]
fn installed_realization_rejects_every_one_field_substitution() {
    let spec = authentic_spec();
    let mut authentic = realize(&spec);
    let authentic_digest = authentic.occurrence_digest();
    assert_eq!(
        authentic.receipt_context().occurrence_digest(),
        authentic_digest,
        "the retained context replays the same occurrence identity"
    );

    // The one-shot registry authority burns on issuance and replays the
    // complete installed evidence rather than the compact report identity.
    let registry = authentic
        .claim_installation_registry()
        .expect("sole registry authority");
    assert!(
        authentic.claim_installation_registry().is_err(),
        "a second registry claim must reject the burned one-shot authority"
    );
    assert_eq!(
        registry.installation_scope(),
        id(61, InstallationScopeId::from_normalized_identity)
    );
    assert!(registry.matches(&authentic));

    // The authentic quarantined installation replays stale-entry contexts
    // against the complete retained receipt evidence.
    let quarantine_receipt = MappingQuarantineReceipt::from_provider(
        &authentic,
        id(401, MappingQuarantineId::from_normalized_identity),
        true,
        true,
        true,
        MappingQuarantineCause::IncompleteDrain {
            residual_authority_count: 1,
        },
    );
    let quarantined =
        quarantine_installed(authentic, quarantine_receipt).expect("authentic quarantine");
    quarantined
        .stale_entry_fault(&realize(&spec).receipt_context())
        .expect("the authentic context names the quarantined realization");

    /// Every independent replay bound to the authentic realization must
    /// reject a substituted one: the recomputed occurrence digest differs,
    /// the registry authority does not match, the quarantined fault rejects
    /// the substituted context, and each lifecycle gate refuses a receipt or
    /// authority carrying the substituted evidence in either direction.
    fn assert_realization_diverged(
        axis: &str,
        substituted: &InstalledCode,
        authentic_digest: installation_evidence::InstalledArtifactOccurrenceDigest,
        registry: &super::InstallationRegistryAuthority,
        quarantined: &super::QuarantinedInstallation,
        spec: &RealizationSpec,
    ) {
        assert_ne!(
            substituted.occurrence_digest(),
            authentic_digest,
            "{axis}: the substituted realization must recompute to a different occurrence identity"
        );
        assert!(
            !registry.matches(substituted),
            "{axis}: the registry authority must not match the substituted realization"
        );
        assert!(
            quarantined
                .stale_entry_fault(&substituted.receipt_context())
                .is_err(),
            "{axis}: the quarantined realization must reject the substituted stale-entry context"
        );
        let error = retire_installed(
            realize(spec),
            RetirementAuthority::from_admitted_provider(substituted, std::iter::empty()),
            RetirementReceipt::from_provider(&realize(spec), true, true, true, std::iter::empty()),
        )
        .expect_err("retirement authority bound to the substituted realization must reject");
        assert!(
            error.diagnostic().0.contains("not scoped"),
            "{axis}: unexpected authority rejection: {}",
            error.diagnostic().0
        );
        let error = retire_installed(
            realize(spec),
            RetirementAuthority::from_admitted_provider(&realize(spec), std::iter::empty()),
            RetirementReceipt::from_provider(substituted, true, true, true, std::iter::empty()),
        )
        .expect_err("retirement receipt bound to the substituted realization must reject");
        assert!(
            error.diagnostic().0.contains("does not match"),
            "{axis}: unexpected receipt rejection: {}",
            error.diagnostic().0
        );
        let error = quarantine_installed(
            realize(spec),
            MappingQuarantineReceipt::from_provider(
                substituted,
                id(401, MappingQuarantineId::from_normalized_identity),
                true,
                true,
                true,
                MappingQuarantineCause::IncompleteDrain {
                    residual_authority_count: 1,
                },
            ),
        )
        .expect_err("quarantine receipt bound to the substituted realization must reject");
        assert!(
            error.diagnostic().0.contains("does not match"),
            "{axis}: unexpected quarantine rejection: {}",
            error.diagnostic().0
        );
    }

    // Every field retained in the installed-occurrence evidence substitutes
    // independently: the honest containing identity is recomputed by the
    // digest and each independent replay still rejects the substitution.
    let axes: Vec<(&str, fn(&mut RealizationSpec))> = vec![
        ("artifact code bytes", |s| s.code[0] ^= 1),
        ("artifact architecture", |s| {
            s.architecture = Architecture::Aarch64;
        }),
        ("artifact identity", |s| s.artifact = 2),
        ("artifact entry set", |s| s.entry_set = 35),
        ("artifact entry identity", |s| s.entry = 1002),
        ("artifact entry code offset", |s| s.entry_offset = 24),
        ("artifact relocation roster", |s| {
            s.relocations.push(DecodedArtifactRelocation {
                kind: ArtifactRelocationKind::Absolute64,
                destination_offset: 8,
                target: RelocationTarget::Entry(entry_id(1001)),
                addend: 0,
            });
            s.resolve = |_| Some(0x9000);
        }),
        ("admission receipt", |s| s.admission_receipt = 41),
        ("container proof presence", |s| s.container_proof = None),
        ("container proof digest", |s| {
            s.container_proof = Some(RetainedContainerProof {
                digest: normalized_proof_payload_digest(b"forged proof payload"),
                bytes: b"container proof payload".to_vec(),
            });
        }),
        ("container proof bytes", |s| {
            s.container_proof = Some(RetainedContainerProof {
                digest: normalized_proof_payload_digest(b"container proof payload"),
                bytes: b"forged proof payload".to_vec(),
            });
        }),
        ("installed identity", |s| s.installed = 282),
        ("placement identity", |s| s.placement = 108),
        ("installation scope", |s| {
            s.scope = 62;
            s.constraints = spec_constraints(
                Some((0x1000, 0x1_0000)),
                4096,
                PlacementPhase::PostHandoff,
                None,
                Some(62),
            );
        }),
        ("constraint installation scope", |s| {
            s.constraints = spec_constraints(
                Some((0x1000, 0x1_0000)),
                4096,
                PlacementPhase::PostHandoff,
                None,
                None,
            );
        }),
        ("installation audience", |s| {
            s.audience = InstallationAudience::DormantLocal;
        }),
        ("permitted range dropped", |s| {
            s.constraints =
                spec_constraints(None, 4096, PlacementPhase::PostHandoff, None, Some(61));
        }),
        ("permitted range start", |s| {
            s.constraints = spec_constraints(
                Some((0x800, 0x1_0000)),
                4096,
                PlacementPhase::PostHandoff,
                None,
                Some(61),
            );
        }),
        ("permitted range end", |s| {
            s.constraints = spec_constraints(
                Some((0x1000, 0x2_0000)),
                4096,
                PlacementPhase::PostHandoff,
                None,
                Some(61),
            );
        }),
        ("placement alignment", |s| {
            s.constraints = spec_constraints(
                Some((0x1000, 0x1_0000)),
                2048,
                PlacementPhase::PostHandoff,
                None,
                Some(61),
            );
        }),
        ("placement phase", |s| {
            s.constraints = spec_constraints(
                Some((0x1000, 0x1_0000)),
                4096,
                PlacementPhase::Load,
                None,
                Some(61),
            );
        }),
        ("placement machine regime", |s| {
            s.constraints = spec_constraints(
                Some((0x1000, 0x1_0000)),
                4096,
                PlacementPhase::PostHandoff,
                Some(10),
                Some(61),
            );
        }),
        ("placement base", |s| s.extent_base = 0x8000),
        ("placement length", |s| s.extent_length = 8192),
        ("placement address space", |s| s.extent_space = 54),
        ("placement rights roster", |s| {
            s.extent_rights = vec![51, 55]
        }),
        ("placement provenance", |s| s.extent_provenance = 56),
        ("placement mapping era", |s| s.extent_era = 57),
        ("extent lineage", |s| s.extent_lineage = 117),
        ("realized footprint", |s| s.realized_footprint = 72),
        ("final validation identity", |s| s.validation = 181),
        ("W^X enforcement", |s| s.wx = WxEnforcement::ConventionOnly),
    ];
    for (name, mutate) in &axes {
        let mut changed = spec.clone();
        mutate(&mut changed);
        let substituted = realize(&changed);
        assert_realization_diverged(
            name,
            &substituted,
            authentic_digest,
            &registry,
            &quarantined,
            &spec,
        );
    }

    // The exact materialized byte interval is bound independently of the
    // content identity: one relocation resolved to two different targets
    // keeps the same artifact, placement, and report identities yet yields a
    // different final-bytes digest, so the realizations diverge.
    let mut resolved_a = spec.clone();
    resolved_a.relocations.push(DecodedArtifactRelocation {
        kind: ArtifactRelocationKind::Absolute64,
        destination_offset: 8,
        target: RelocationTarget::Entry(entry_id(1001)),
        addend: 0,
    });
    resolved_a.resolve = |_| Some(0x9000);
    let mut resolved_b = resolved_a.clone();
    resolved_b.resolve = |_| Some(0x9008);
    let realization_a = realize(&resolved_a);
    let realization_b = realize(&resolved_b);
    assert_eq!(
        realization_a.artifact(),
        realization_b.artifact(),
        "the resolver outcome rides outside the artifact content identity"
    );
    assert_ne!(
        realization_a.occurrence_digest(),
        realization_b.occurrence_digest(),
        "the exact final bytes are bound by the occurrence identity"
    );

    // The provider-issuance origin is not retained in placement evidence:
    // substituting it canonicalizes to the identical realization and replays
    // as the same custody, exactly like an informational wire axis.
    let mut envelope = spec.clone();
    envelope.extent_issuance = 900;
    let envelope_realization = realize(&envelope);
    assert_eq!(
        envelope_realization.occurrence_digest(),
        authentic_digest,
        "the provider issuance origin is not retained in the realization identity"
    );
    assert!(registry.matches(&envelope_realization));

    // The claimed installed report identity is adopted into the realization
    // and bound by the occurrence digest: the substitution is representable
    // but every downstream replay names a different realization.
    let mut claimed = spec.clone();
    claimed.installed = 299;
    let claimed_installed = realize(&claimed);
    assert_eq!(
        claimed_installed.identity(),
        id(299, InstalledCodeId::from_normalized_identity)
    );
    assert_realization_diverged(
        "claimed installed identity",
        &claimed_installed,
        authentic_digest,
        &registry,
        &quarantined,
        &spec,
    );

    // The install gate rejects a receipt or authority bound to a different
    // validated placement, an incomplete visibility claim, and an
    // unsupported execute transition, returning all linear inputs.
    let mut foreign_spec = spec.clone();
    foreign_spec.placement = 108;
    let foreign_validated = spec_validated(&foreign_spec);
    let validated = spec_validated(&spec);
    let receipt = InstallationReceipt::from_provider(
        id(spec.installed, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    let error = install_validated(
        validated,
        InstallAuthority::from_admitted_provider(&foreign_validated),
        receipt,
    )
    .expect_err("an install authority bound to another validated placement must reject");
    assert!(error.diagnostic().0.contains("not scoped"));
    let (validated, _foreign_authority, _receipt) = (*error).into_parts();
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let foreign_receipt = InstallationReceipt::from_provider(
        id(spec.installed, InstalledCodeId::from_normalized_identity),
        &foreign_validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    let error = install_validated(validated, authority, foreign_receipt)
        .expect_err("a receipt bound to another validated placement must reject");
    assert!(error.diagnostic().0.contains("does not match"));
    let (validated, authority, _receipt) = (*error).into_parts();
    let incomplete = InstallationReceipt::from_provider(
        id(spec.installed, InstalledCodeId::from_normalized_identity),
        &validated,
        false,
        WxEnforcement::HardwareEnforced,
    );
    let error = install_validated(validated, authority, incomplete)
        .expect_err("an incomplete visibility claim must reject");
    assert!(
        error
            .diagnostic()
            .0
            .contains("instruction-fetch visibility")
    );
    let (validated, authority, _receipt) = (*error).into_parts();
    let unsupported = InstallationReceipt::from_provider(
        id(spec.installed, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::Unsupported,
    );
    let error = install_validated(validated, authority, unsupported)
        .expect_err("an unsupported execute transition must reject");
    assert!(error.diagnostic().0.contains("does not support"));
    let (_validated, _authority, _receipt) = (*error).into_parts();

    // The retirement gate replays both records against the exact installed
    // evidence and requires the quiescence, execute-removal, write-restore,
    // and required-fact claims; a completion-fact superset remains admitted
    // because required facts are provider-open vocabulary.
    let fact_a = RetirementFactDigest::from_canonical_bytes(b"provider.drain.complete.v1");
    let fact_b = RetirementFactDigest::from_canonical_bytes(b"provider.cache-flush.complete.v1");
    let fact_c = RetirementFactDigest::from_canonical_bytes(b"provider.quiesce.complete.v1");
    let retired = retire_installed(
        realize(&spec),
        RetirementAuthority::from_admitted_provider(&realize(&spec), [fact_a, fact_c]),
        RetirementReceipt::from_provider(
            &realize(&spec),
            true,
            true,
            true,
            [fact_a, fact_b, fact_c],
        ),
    )
    .expect("a receipt establishing a superset of the required facts retires");
    assert_eq!(
        retired.previous_artifact().artifact().identity(),
        id(spec.artifact, ArtifactId::from_normalized_identity)
    );
    let _placement = retired.into_placement();

    let rejected_retirement: Vec<(
        &str,
        fn(&RealizationSpec) -> RetirementAuthority,
        fn(&RealizationSpec) -> RetirementReceipt,
        &str,
    )> = vec![
        (
            "executors not quiesced",
            |s| RetirementAuthority::from_admitted_provider(&realize(s), std::iter::empty()),
            |s| {
                RetirementReceipt::from_provider(&realize(s), false, true, true, std::iter::empty())
            },
            "quiescence",
        ),
        (
            "execute not disabled",
            |s| RetirementAuthority::from_admitted_provider(&realize(s), std::iter::empty()),
            |s| {
                RetirementReceipt::from_provider(&realize(s), true, false, true, std::iter::empty())
            },
            "execute removal",
        ),
        (
            "write authority not restored",
            |s| RetirementAuthority::from_admitted_provider(&realize(s), std::iter::empty()),
            |s| {
                RetirementReceipt::from_provider(&realize(s), true, true, false, std::iter::empty())
            },
            "write authority",
        ),
        (
            "required fact dropped",
            |s| {
                RetirementAuthority::from_admitted_provider(
                    &realize(s),
                    [
                        RetirementFactDigest::from_canonical_bytes(b"provider.drain.complete.v1"),
                        RetirementFactDigest::from_canonical_bytes(b"provider.quiesce.complete.v1"),
                    ],
                )
            },
            |s| {
                RetirementReceipt::from_provider(
                    &realize(s),
                    true,
                    true,
                    true,
                    [RetirementFactDigest::from_canonical_bytes(
                        b"provider.drain.complete.v1",
                    )],
                )
            },
            "completion facts",
        ),
        (
            "required fact renamed",
            |s| {
                RetirementAuthority::from_admitted_provider(
                    &realize(s),
                    [RetirementFactDigest::from_canonical_bytes(
                        b"provider.drain.complete.v1",
                    )],
                )
            },
            |s| {
                RetirementReceipt::from_provider(
                    &realize(s),
                    true,
                    true,
                    true,
                    [RetirementFactDigest::from_canonical_bytes(
                        b"provider.cache-flush.complete.v1",
                    )],
                )
            },
            "completion facts",
        ),
    ];
    for (name, build_authority, build_receipt, fragment) in &rejected_retirement {
        let error = retire_installed(realize(&spec), build_authority(&spec), build_receipt(&spec))
            .expect_err("retirement gate must reject the substituted record");
        assert!(
            error.diagnostic().0.contains(fragment),
            "{name}: unexpected retirement rejection: {}",
            error.diagnostic().0
        );
        let (_installed, _authority, _receipt) = (*error).into_parts();
    }

    // The quarantine gate replays the complete installed evidence and the
    // fail-closed claims; the claimed compact quarantine identity and the
    // attributed cause are adopted verbatim into the retained record.
    let quarantine = id(401, MappingQuarantineId::from_normalized_identity);
    let rejected_quarantine: Vec<(&str, fn(&InstalledCode) -> MappingQuarantineReceipt, &str)> = vec![
        (
            "foreign installed evidence",
            |_| {
                let mut foreign = authentic_spec();
                foreign.placement = 108;
                MappingQuarantineReceipt::from_provider(
                    &realize(&foreign),
                    id(401, MappingQuarantineId::from_normalized_identity),
                    true,
                    true,
                    true,
                    MappingQuarantineCause::IncompleteDrain {
                        residual_authority_count: 1,
                    },
                )
            },
            "does not match",
        ),
        (
            "execute not disabled",
            |installed| {
                MappingQuarantineReceipt::from_provider(
                    installed,
                    id(401, MappingQuarantineId::from_normalized_identity),
                    false,
                    true,
                    true,
                    MappingQuarantineCause::IncompleteDrain {
                        residual_authority_count: 1,
                    },
                )
            },
            "execute removal",
        ),
        (
            "range still mapped",
            |installed| {
                MappingQuarantineReceipt::from_provider(
                    installed,
                    id(401, MappingQuarantineId::from_normalized_identity),
                    true,
                    false,
                    true,
                    MappingQuarantineCause::IncompleteDrain {
                        residual_authority_count: 1,
                    },
                )
            },
            "unmapped/trapping",
        ),
        (
            "range not reserved",
            |installed| {
                MappingQuarantineReceipt::from_provider(
                    installed,
                    id(401, MappingQuarantineId::from_normalized_identity),
                    true,
                    true,
                    false,
                    MappingQuarantineCause::IncompleteDrain {
                        residual_authority_count: 1,
                    },
                )
            },
            "reserve",
        ),
        (
            "drained cause",
            |installed| {
                MappingQuarantineReceipt::from_provider(
                    installed,
                    id(401, MappingQuarantineId::from_normalized_identity),
                    true,
                    true,
                    true,
                    MappingQuarantineCause::IncompleteDrain {
                        residual_authority_count: 0,
                    },
                )
            },
            "no attributed residual holder",
        ),
        (
            "anonymous holder cause",
            |installed| {
                MappingQuarantineReceipt::from_provider(
                    installed,
                    id(401, MappingQuarantineId::from_normalized_identity),
                    true,
                    true,
                    true,
                    MappingQuarantineCause::PossibleOpaqueHolder {
                        provider_identity: "   ".into(),
                    },
                )
            },
            "no attributed residual holder",
        ),
    ];
    for (name, build_receipt, fragment) in &rejected_quarantine {
        let installed = realize(&spec);
        let receipt = build_receipt(&installed);
        let error = quarantine_installed(installed, receipt)
            .expect_err("quarantine gate must reject the substituted record");
        assert!(
            error.diagnostic().0.contains(fragment),
            "{name}: unexpected quarantine rejection: {}",
            error.diagnostic().0
        );
        let (_installed, _receipt) = (*error).into_parts();
    }

    let substituted_claim = quarantine_installed(
        realize(&spec),
        MappingQuarantineReceipt::from_provider(
            &realize(&spec),
            id(409, MappingQuarantineId::from_normalized_identity),
            true,
            true,
            true,
            MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 1,
            },
        ),
    )
    .expect("the claimed quarantine report identity is adopted verbatim");
    assert_ne!(
        substituted_claim.quarantine(),
        quarantined.quarantine(),
        "the retained record reports the substituted quarantine identity"
    );
    let fault = substituted_claim
        .stale_entry_fault(&realize(&spec).receipt_context())
        .expect("the unchanged installed evidence still faults");
    assert_eq!(
        fault.quarantine(),
        id(409, MappingQuarantineId::from_normalized_identity),
        "the stale-entry fault replays the claimed report identity"
    );
    assert!(!fault.discharged_obligations());

    let substituted_cause = quarantine_installed(
        realize(&spec),
        MappingQuarantineReceipt::from_provider(
            &realize(&spec),
            quarantine,
            true,
            true,
            true,
            MappingQuarantineCause::PossibleOpaqueHolder {
                provider_identity: "OtherProvider".into(),
            },
        ),
    )
    .expect("a different attributed cause is adopted verbatim");
    assert!(
        matches!(
            substituted_cause.cause(),
            MappingQuarantineCause::PossibleOpaqueHolder { provider_identity }
                if provider_identity == "OtherProvider"
        ),
        "the retained record reports the substituted cause"
    );

    // A stale-entry attempt naming any other realization rejects, including
    // one whose compact installed report identity collides.
    let unrelated = realize(&{
        let mut foreign = spec.clone();
        foreign.placement = 108;
        foreign.installed = 282;
        foreign
    });
    assert!(
        quarantined
            .stale_entry_fault(&unrelated.receipt_context())
            .is_err(),
        "an unrelated realization is not this stale entry"
    );
    let mut colliding = spec.clone();
    colliding.code = vec![0xcc; 64];
    let colliding_installed = realize(&colliding);
    assert_eq!(
        colliding_installed.identity(),
        realize(&spec).identity(),
        "the adversary controls a collision-equal compact report identity"
    );
    assert!(
        quarantined
            .stale_entry_fault(&colliding_installed.receipt_context())
            .is_err(),
        "a compact-ID collision must not forge stale-entry evidence"
    );

    // Zero is never a representable normalized identity: every identity
    // constructor in this family rejects it before any substitution can be
    // encoded.
    assert!(InstalledCodeId::from_normalized_identity(0).is_err());
    assert!(MappingQuarantineId::from_normalized_identity(0).is_err());
    assert!(FinalValidationId::from_normalized_identity(0).is_err());
    assert!(CodePlacementId::from_normalized_identity(0).is_err());
    assert!(InstallationScopeId::from_normalized_identity(0).is_err());
    assert!(AdmissionReceiptId::from_normalized_identity(0).is_err());
}

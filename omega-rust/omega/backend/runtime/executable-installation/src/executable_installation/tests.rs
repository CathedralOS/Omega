use super::*;
use extents::{ExtentLineageId, ExtentRootGrant, MappingEraId};
use layout_plans::{ArtifactInstallationScopeId, PlacementPhase};

use super::test_support::*;

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
    let root = include_str!("../executable_installation.rs");
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

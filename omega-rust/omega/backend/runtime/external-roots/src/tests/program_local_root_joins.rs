use super::{
    entry_id, install_program_local_required_root, install_test_root_pair_with_ids, installed_code,
    join_program_local, program_local_claim, program_local_epoch_lease, program_local_lifecycle,
    program_local_root_catalog, program_local_root_module, program_local_terminal_object,
    publish_program_local_era, sole_rejected_cohort_lease,
};
use crate::{ExternalRootEntryClaim, ProgramLocalRootCohortMember};
use proof_admission::AdmissionProfile;
use terminal_psi::{
    StructuralContentProjection, VocabularyMarker,
    program_local_root_introduction_compatibility_report_identity,
};

#[test]
fn program_local_root_schemas_derive_exact_installed_slots_without_minting() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, first, second) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut bindings = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");

    // The enumerable set is the sealed required closure itself: omitting the
    // sealed slot or presenting a runtime-open root rejects before anything
    // commits, so the aggregate cannot silently understate this installation.
    assert!(
        bindings
            .derive_eligible_prebindings(&catalog, &terminal, [])
            .expect_err("omitting the sealed required root rejects enumeration")
            .0
            .contains("omits a sealed required root slot")
    );
    assert!(
        bindings
            .derive_eligible_prebindings(&catalog, &terminal, [&second])
            .expect_err("a runtime-open root is outside the required cohort")
            .0
            .contains("outside the sealed required closure")
    );
    assert!(
        bindings
            .derive_eligible_prebindings(&catalog, &terminal, [&first, &second])
            .expect_err("an extra root cannot join the sealed enumeration")
            .0
            .contains("outside the sealed required closure")
    );
    assert_eq!(bindings.prebindings().count(), 0);

    let first_occurrences = bindings
        .derive_eligible_prebindings(&catalog, &terminal, [&first])
        .expect("the exact required set derives the eligible prebindings");
    let [first_occurrence] = first_occurrences.as_slice() else {
        panic!("one producer schema")
    };
    let counts = bindings.counts();
    let [count] = counts.as_slice() else {
        panic!("one exact installed schema count")
    };
    assert_eq!(count.installed_slot_count.get(), 1);
    assert_eq!(count.prebinding_identities, [first_occurrence.identity()]);
    assert_eq!(
        count.per_occurrence_capacity,
        module.boundary_machines[0].program_local_root_introductions[0].capacity
    );
    assert!(
        bindings
            .derive_eligible_prebindings(&catalog, &terminal, [&first])
            .expect_err("the complete eligible set is derived exactly once")
            .0
            .contains("already derived")
    );
}

#[test]
fn program_local_root_prebinding_rejects_catalog_object_and_claim_substitution() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let mut narrowed_module = module.clone();
    let boundary_identity = narrowed_module.boundary_machines[0].identity.clone();
    let qualification_identity = narrowed_module.structural_domains[0].identity.clone();
    let carrier_identity = narrowed_module.structural_types[0].identity.clone();
    let narrowed_schema = {
        let schema = &mut narrowed_module.boundary_machines[0].program_local_root_introductions[0];
        schema.capacity = semantic_vocabulary::ContentProjectionExpression::CountedQuantity(
            semantic_vocabulary::ContentProjectionScalar::Natural("1".into()),
        );
        schema.projection.projection_report_fingerprint =
            language_semantics::content::terminal_projection_report_fingerprint(
                &schema.algebra,
                &schema.capacity,
            );
        schema.compatibility_report_identity =
            program_local_root_introduction_compatibility_report_identity(
                &boundary_identity,
                &qualification_identity,
                &carrier_identity,
                schema,
            );
        schema.clone()
    };
    narrowed_module.structural_domains[0].content_projection = Some(StructuralContentProjection {
        identity: narrowed_schema.projection,
        algebra: narrowed_schema.algebra.clone(),
        expression: narrowed_schema.capacity.clone(),
    });
    let narrowed_catalog = program_local_root_catalog(&narrowed_module);
    let narrowed_terminal = program_local_terminal_object(&narrowed_module);
    assert_ne!(catalog.terminal_psi(), narrowed_catalog.terminal_psi());
    let (mut root_ledger, root, wrong_claim_root) = install_test_root_pair_with_ids(
        &mut code,
        (1, 20, 21, 22, vec![program_local_claim()]),
        (
            201,
            220,
            221,
            222,
            vec![ExternalRootEntryClaim {
                domain: "Region::Other".into(),
                ..program_local_claim()
            }],
        ),
        entry,
    );

    let mut tampered_module = module.clone();
    tampered_module.boundary_machines[0].program_local_root_introductions[0]
        .compatibility_report_identity ^= 1;
    assert!(
        terminal_verifier::verify_module(
            &tampered_module,
            &terminal_verifier::ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .is_err()
    );

    let mut wrong_object = program_local_terminal_object(&module);
    wrong_object.identity = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([9; 32]),
    };
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let original_catalog_narrowed_artifact = installation
        .derive_eligible_prebindings(&catalog, &narrowed_terminal, [&root])
        .expect_err("a narrowed artifact cannot carry the original producer catalog");
    assert_eq!(
        original_catalog_narrowed_artifact.0,
        "program-local root catalog does not match the terminal artifact identity"
    );
    assert_eq!(installation.prebindings().count(), 0);

    let narrowed_catalog_original_artifact = installation
        .derive_eligible_prebindings(&narrowed_catalog, &terminal, [&root])
        .expect_err("a narrowed producer catalog cannot describe the original artifact");
    assert_eq!(
        narrowed_catalog_original_artifact.0,
        "program-local root catalog does not match the terminal artifact identity"
    );
    assert_eq!(installation.prebindings().count(), 0);

    assert!(
        installation
            .derive_eligible_prebindings(&catalog, &wrong_object, [&root])
            .is_err()
    );

    assert!(
        installation
            .derive_eligible_prebindings(&catalog, &terminal, [&root, &wrong_claim_root])
            .expect_err("an out-of-closure runtime-open root cannot join the enumeration")
            .0
            .contains("outside the sealed required closure")
    );

    let mut wrong_claim_code = installed_code(2, entry);
    let (mut wrong_claim_ledger, wrong_claim_required, _wrong_claim_open) =
        install_test_root_pair_with_ids(
            &mut wrong_claim_code,
            (
                1,
                20,
                21,
                22,
                vec![ExternalRootEntryClaim {
                    domain: "Region::Other".into(),
                    ..program_local_claim()
                }],
            ),
            (101, 120, 121, 122, vec![program_local_claim()]),
            entry,
        );
    let mut wrong_claim_installation = wrong_claim_ledger
        .claim_program_local_root_installation_ledger()
        .expect("claim-substituted program-local cohort verifier");
    let claim_substitution = wrong_claim_installation
        .derive_eligible_prebindings(&catalog, &terminal, [&wrong_claim_required])
        .expect_err("a substituted installed entry claim rejects the derived schema");
    assert!(
        claim_substitution
            .0
            .contains("does not match an exact installed entry claim")
    );

    installation
        .derive_eligible_prebindings(&catalog, &terminal, [&root])
        .expect("failed substitutions leave the exact prebinding available");
}

#[test]
fn program_local_root_join_pins_exact_root_artifact_contract_and_epoch_once() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, root, _open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let [prebinding] = installation
        .derive_eligible_prebindings(&catalog, &terminal, [&root])
        .expect("verified installed prebinding")
        .try_into()
        .expect("one program-local prebinding");
    let prebinding = prebinding.identity();
    let mut lifecycle = program_local_lifecycle(
        700,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let occurrence = join_program_local(
        &mut installation,
        prebinding,
        &root,
        &mut lifecycle,
        800,
        10,
        "TestRoot::entry",
    )
    .expect("exact lifecycle join");
    assert_eq!(occurrence.identity().prebinding(), prebinding);
    assert_eq!(occurrence.identity().lifecycle_epoch(), 10);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));

    let mut foreign_lifecycle = program_local_lifecycle(
        701,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let foreign = join_program_local(
        &mut installation,
        prebinding,
        &root,
        &mut foreign_lifecycle,
        850,
        10,
        "TestRoot::entry",
    )
    .expect_err("one installed prebinding family has one lifecycle ledger");
    assert!(foreign.diagnostic().0.contains("another lifecycle ledger"));
    let foreign_lease = sole_rejected_cohort_lease(*foreign);
    foreign_lifecycle
        .release_program_local_root_epoch_lease(foreign_lease)
        .expect("lifecycle substitution returns its lease");

    let replay = join_program_local(
        &mut installation,
        prebinding,
        &root,
        &mut lifecycle,
        801,
        10,
        "TestRoot::entry",
    )
    .expect_err("one exact occurrence cannot join twice in one era");
    let replay_lease = sole_rejected_cohort_lease(*replay);
    lifecycle
        .release_program_local_root_epoch_lease(replay_lease)
        .expect("rejected join returns its lease intact");

    let retired = installation
        .retire(occurrence, &mut lifecycle)
        .expect("exact occurrence retirement");
    assert_eq!(retired.identity().prebinding(), prebinding);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));

    let replay = join_program_local(
        &mut installation,
        prebinding,
        &root,
        &mut lifecycle,
        802,
        10,
        "TestRoot::entry",
    )
    .expect_err("retirement never makes the same-era origin reusable");
    let replay_lease = sole_rejected_cohort_lease(*replay);
    lifecycle
        .release_program_local_root_epoch_lease(replay_lease)
        .expect("used-key rejection returns its lease");

    publish_program_local_era(
        &mut lifecycle,
        20,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
        120,
        true,
    );
    let next_epoch = join_program_local(
        &mut installation,
        prebinding,
        &root,
        &mut lifecycle,
        803,
        20,
        "TestRoot::entry",
    )
    .expect("a later lifecycle epoch is a fresh occurrence");
    assert_eq!(next_epoch.identity().lifecycle_epoch(), 20);
    installation
        .retire(next_epoch, &mut lifecycle)
        .expect("later epoch occurrence retires independently");
}

#[test]
fn program_local_root_failed_join_returns_lease_without_burning_the_occurrence() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, first, other) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let [prebinding] = installation
        .derive_eligible_prebindings(&catalog, &terminal, [&first])
        .expect("verified installed prebinding")
        .try_into()
        .expect("one program-local prebinding");
    let prebinding = prebinding.identity();

    let mut lifecycle = program_local_lifecycle(
        710,
        10,
        first.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let substituted = join_program_local(
        &mut installation,
        prebinding,
        &other,
        &mut lifecycle,
        810,
        10,
        "TestRoot::entry",
    )
    .expect_err("a different installed root cannot satisfy the prebinding");
    let lease = sole_rejected_cohort_lease(*substituted);
    lifecycle
        .release_program_local_root_epoch_lease(lease)
        .expect("root substitution returns the lease");

    let mut wrong_artifact = program_local_lifecycle(
        711,
        10,
        installation_evidence::InstalledArtifactOccurrenceDigest::from_sha256([0xaa; 32]),
        code_identity,
        "TestRoot::entry",
    );
    let substituted = join_program_local(
        &mut installation,
        prebinding,
        &first,
        &mut wrong_artifact,
        811,
        10,
        "TestRoot::entry",
    )
    .expect_err("artifact occurrence substitution rejects");
    let lease = sole_rejected_cohort_lease(*substituted);
    wrong_artifact
        .release_program_local_root_epoch_lease(lease)
        .expect("artifact substitution returns the lease");

    let mut wrong_contract = program_local_lifecycle(
        712,
        10,
        first.installed_artifact_occurrence_digest(),
        code_identity,
        "OtherRoot::entry",
    );
    let substituted = join_program_local(
        &mut installation,
        prebinding,
        &first,
        &mut wrong_contract,
        812,
        10,
        "OtherRoot::entry",
    )
    .expect_err("entry-contract substitution rejects");
    let lease = sole_rejected_cohort_lease(*substituted);
    wrong_contract
        .release_program_local_root_epoch_lease(lease)
        .expect("contract substitution returns the lease");

    let mut stale_lifecycle = program_local_lifecycle(
        713,
        10,
        first.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let stale_lease = program_local_epoch_lease(&mut stale_lifecycle, 814, 10, "TestRoot::entry");
    publish_program_local_era(
        &mut stale_lifecycle,
        20,
        first.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
        121,
        true,
    );
    let stale = installation
        .seal_epoch_cohort(
            &stale_lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding,
                &first,
                stale_lease,
            )],
        )
        .expect_err("a lease from a now-closing era cannot establish new authority");
    assert!(stale.diagnostic().0.contains("current epoch ledger"));
    let stale_lease = sole_rejected_cohort_lease(*stale);
    stale_lifecycle
        .release_program_local_root_epoch_lease(stale_lease)
        .expect("stale establishment rejection returns the lifecycle hold");

    let exact = join_program_local(
        &mut installation,
        prebinding,
        &first,
        &mut lifecycle,
        813,
        10,
        "TestRoot::entry",
    )
    .expect("failed joins did not burn the exact occurrence");
    installation
        .retire(exact, &mut lifecycle)
        .expect("exact occurrence remains retireable");
}

#[test]
fn program_local_root_failed_retirement_returns_the_complete_occurrence() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, root, _open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let [prebinding] = installation
        .derive_eligible_prebindings(&catalog, &terminal, [&root])
        .expect("verified installed prebinding")
        .try_into()
        .expect("one program-local prebinding");
    let mut rightful = program_local_lifecycle(
        720,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let mut substituted = program_local_lifecycle(
        721,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let occurrence = join_program_local(
        &mut installation,
        prebinding.identity(),
        &root,
        &mut rightful,
        820,
        10,
        "TestRoot::entry",
    )
    .expect("exact lifecycle join");

    let error = installation
        .retire(occurrence, &mut substituted)
        .expect_err("another lifecycle ledger cannot release the occurrence");
    assert!(error.diagnostic().0.contains("lifecycle lease"));
    let occurrence = (*error).into_occurrence();
    assert_eq!(rightful.program_local_root_authority_holds(10), Some(1));
    installation
        .retire(occurrence, &mut rightful)
        .expect("returned occurrence retires through the rightful ledger");
    assert_eq!(rightful.program_local_root_authority_holds(10), Some(0));
}

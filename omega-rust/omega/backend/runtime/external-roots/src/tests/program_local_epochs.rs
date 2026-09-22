use super::{
    entry_id, install_program_local_required_root, install_program_local_two_parameter_roots,
    installed_code, installed_code_with_fill_and_installation_identity, program_local_activation,
    program_local_claim, program_local_claim_at, program_local_epoch_lease,
    program_local_extent_module, program_local_extent_subject, program_local_lifecycle,
    program_local_root_catalog, program_local_root_module, program_local_subject,
    program_local_subject_at, program_local_terminal_object, program_local_two_schema_module,
    publish_program_local_era,
};
use crate::{
    EstablishedProgramLocalRoot, EstablishedProgramLocalRootCapacity,
    InstalledProgramLocalRootOccurrence, ProgramLocalRootCohortMember,
    compose_program_local_root_coexistence_report,
};

#[test]
fn epoch_cohort_cannot_seal_before_the_eligible_set_is_derived() {
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
    let mut lifecycle = program_local_lifecycle(
        731,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );

    // Before derivation the eligible set is empty, so an empty member roster
    // would silently seal a cohort missing this artifact instance's exact
    // enumerable occurrence. The seal must wait for the derived closure.
    let premature = installation
        .seal_epoch_cohort(&lifecycle, std::iter::empty())
        .expect_err("a cohort cannot seal before the eligible set is derived");
    assert!(
        premature
            .diagnostic()
            .0
            .contains("precedes the derived eligible prebinding set")
    );

    let [prebinding] = installation
        .derive_eligible_prebindings(&catalog, &terminal, [&root])
        .expect("derived eligible prebinding")
        .try_into()
        .expect("one producer schema");
    let lease = program_local_epoch_lease(&mut lifecycle, 832, 10, "TestRoot::entry");
    let cohort = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding.identity(),
                &root,
                lease,
            )],
        )
        .expect("the derived eligible set seals the exact cohort");
    assert_eq!(cohort.occurrences().len(), 1);
    assert_eq!(cohort.aggregates().len(), 1);
}

#[test]
fn epoch_cohort_seals_exact_members_and_derives_aggregate_schema() {
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
        .expect("required root prebinding")
        .try_into()
        .expect("one producer schema");
    let prebinding = prebinding.identity();
    let mut lifecycle = program_local_lifecycle(
        730,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );

    let omitted = installation
        .seal_epoch_cohort(&lifecycle, std::iter::empty())
        .expect_err("omitting the eligible prebinding rejects");
    assert!(omitted.diagnostic().0.contains("omits or adds"));
    assert!(omitted.into_members().is_empty());

    let duplicate_lease_a = program_local_epoch_lease(&mut lifecycle, 830, 10, "TestRoot::entry");
    let duplicate_lease_b = program_local_epoch_lease(&mut lifecycle, 831, 10, "TestRoot::entry");
    let duplicate = installation
        .seal_epoch_cohort(
            &lifecycle,
            [
                ProgramLocalRootCohortMember::new(prebinding, &root, duplicate_lease_a),
                ProgramLocalRootCohortMember::new(prebinding, &root, duplicate_lease_b),
            ],
        )
        .expect_err("duplicate cohort members reject transactionally");
    assert!(duplicate.diagnostic().0.contains("repeats one prebinding"));
    for member in duplicate.into_members() {
        lifecycle
            .release_program_local_root_epoch_lease(member.into_parts().2)
            .expect("duplicate rejection returns every lease");
    }

    let lease = program_local_epoch_lease(&mut lifecycle, 832, 10, "TestRoot::entry");
    let cohort = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(prebinding, &root, lease)],
        )
        .expect("exact epoch cohort");
    assert_eq!(cohort.identity().lifecycle_epoch(), 10);
    assert_eq!(cohort.installed_required_slots().slots().len(), 1);
    assert_eq!(cohort.occurrences().len(), 1);
    let aggregates = cohort.aggregates().collect::<Vec<_>>();
    let [aggregate] = aggregates.as_slice() else {
        panic!("one closed aggregate schema")
    };
    assert_eq!(aggregate.cardinality().get(), 1);
    assert_eq!(
        aggregate.per_occurrence_capacity(),
        &module.boundary_machines[0].program_local_root_introductions[0].capacity
    );
    let snapshot = cohort.aggregate_snapshot();
    assert_eq!(snapshot.identity(), cohort.identity());
    assert_eq!(
        snapshot.installed_required_slots(),
        cohort.installed_required_slots()
    );
    let snapshot_rows = snapshot.aggregates().collect::<Vec<_>>();
    let [snapshot_row] = snapshot_rows.as_slice() else {
        panic!("one snapshotted aggregate row")
    };
    assert_eq!(snapshot_row, aggregate);
    assert_eq!(snapshot_row.cardinality().get(), 1);
    assert_eq!(
        snapshot_row.per_occurrence_capacity(),
        &module.boundary_machines[0].program_local_root_introductions[0].capacity
    );
    let cloned_snapshot = snapshot.clone();
    assert_eq!(cloned_snapshot, snapshot);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));
    assert!(
        installation
            .derive_eligible_prebindings(&catalog, &terminal, [&root])
            .expect_err("sealing freezes the eligible prebinding set")
            .0
            .contains("prebindings are frozen")
    );
    let runtime = cohort.into_runtime();
    assert_eq!(runtime.aggregate_snapshot(), snapshot);
    assert_eq!(runtime.pending_occurrences().len(), 1);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));
    let [occurrence]: [InstalledProgramLocalRootOccurrence<'_, '_>; 1] =
        runtime.cancel().try_into().expect("one cohort occurrence");
    installation
        .retire(occurrence, &mut lifecycle)
        .expect("sealed occurrence remains retireable");
}

#[test]
fn coexistence_report_requires_every_exact_live_epoch_without_reducing_rows() {
    let entry = entry_id(1);
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);

    let mut first_code = installed_code_with_fill_and_installation_identity(1, entry, 0, 300);
    let first_code_identity = first_code.identity().normalized_identity();
    let (mut first_root_ledger, first_root, _first_open_root) =
        install_program_local_required_root(&mut first_code, entry, vec![program_local_claim()]);
    let mut first_installation = first_root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("first program-local cohort verifier");
    let [first_prebinding] = first_installation
        .derive_eligible_prebindings(&catalog, &terminal, [&first_root])
        .expect("first required root prebinding")
        .try_into()
        .expect("one first producer schema");

    let mut lifecycle = program_local_lifecycle(
        760,
        10,
        first_root.installed_artifact_occurrence_digest(),
        first_code_identity,
        "TestRoot::entry",
    );
    let first_lease = program_local_epoch_lease(&mut lifecycle, 860, 10, "TestRoot::entry");
    let first_cohort = first_installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                first_prebinding.identity(),
                &first_root,
                first_lease,
            )],
        )
        .expect("first exact epoch cohort");
    let first_snapshot = first_cohort.aggregate_snapshot();

    let mut second_code = installed_code_with_fill_and_installation_identity(2, entry, 0, 301);
    let second_code_identity = second_code.identity().normalized_identity();
    publish_program_local_era(
        &mut lifecycle,
        20,
        second_code.occurrence_digest(),
        second_code_identity,
        "TestRoot::entry",
        120,
        true,
    );
    let (mut second_root_ledger, second_root, _second_open_root) =
        install_program_local_required_root(&mut second_code, entry, vec![program_local_claim()]);
    let mut second_installation = second_root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("second program-local cohort verifier");
    let [second_prebinding] = second_installation
        .derive_eligible_prebindings(&catalog, &terminal, [&second_root])
        .expect("second required root prebinding")
        .try_into()
        .expect("one second producer schema");
    let second_lease = program_local_epoch_lease(&mut lifecycle, 861, 20, "TestRoot::entry");
    let second_cohort = second_installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                second_prebinding.identity(),
                &second_root,
                second_lease,
            )],
        )
        .expect("second exact epoch cohort");
    let second_snapshot = second_cohort.aggregate_snapshot();

    let missing =
        compose_program_local_root_coexistence_report(&lifecycle, std::iter::once(&first_snapshot))
            .expect_err("one of two live epochs cannot be omitted");
    assert!(missing.0.contains("omits or adds"));

    let duplicate = compose_program_local_root_coexistence_report(
        &lifecycle,
        [&first_snapshot, &first_snapshot, &second_snapshot],
    )
    .expect_err("one live epoch cannot be repeated");
    assert!(duplicate.0.contains("repeats one lifecycle epoch"));

    let report = compose_program_local_root_coexistence_report(
        &lifecycle,
        [&second_snapshot, &first_snapshot],
    )
    .expect("both exact coexisting epochs compose");
    assert_eq!(report.lifecycle_ledger(), lifecycle.identity());
    assert_eq!(
        report
            .epoch_snapshots()
            .map(|snapshot| snapshot.identity().lifecycle_epoch())
            .collect::<Vec<_>>(),
        vec![10, 20]
    );
    assert_eq!(
        report
            .aggregates()
            .map(|(epoch, aggregate)| (epoch, aggregate.cardinality().get()))
            .collect::<Vec<_>>(),
        vec![(10, 1), (20, 1)]
    );
    assert!(report.aggregates().all(|(_, aggregate)| {
        aggregate.per_occurrence_capacity()
            == &module.boundary_machines[0].program_local_root_introductions[0].capacity
    }));

    let other_lifecycle = program_local_lifecycle(
        761,
        10,
        first_root.installed_artifact_occurrence_digest(),
        first_code_identity,
        "TestRoot::entry",
    );
    let substituted = compose_program_local_root_coexistence_report(
        &other_lifecycle,
        std::iter::once(&first_snapshot),
    )
    .expect_err("a snapshot from another lifecycle ledger rejects");
    assert!(substituted.0.contains("another lifecycle ledger"));

    let later_only_lifecycle = program_local_lifecycle(
        760,
        20,
        second_root.installed_artifact_occurrence_digest(),
        second_code_identity,
        "TestRoot::entry",
    );
    let stale = compose_program_local_root_coexistence_report(
        &later_only_lifecycle,
        std::iter::once(&first_snapshot),
    )
    .expect_err("an old snapshot cannot stand in for the current live epoch");
    assert!(stale.0.contains("non-live lifecycle epoch"));

    let [first_occurrence]: [InstalledProgramLocalRootOccurrence<'_, '_>; 1] = first_cohort
        .into_runtime()
        .cancel()
        .try_into()
        .expect("one first occurrence");
    first_installation
        .retire(first_occurrence, &mut lifecycle)
        .expect("first coexistence occurrence remains retireable");
    let [second_occurrence]: [InstalledProgramLocalRootOccurrence<'_, '_>; 1] = second_cohort
        .into_runtime()
        .cancel()
        .try_into()
        .expect("one second occurrence");
    second_installation
        .retire(second_occurrence, &mut lifecycle)
        .expect("second coexistence occurrence remains retireable");
}

#[test]
fn installed_subject_establishes_exact_capacity_lineage_once_and_pins_the_epoch() {
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
        .expect("one producer schema");
    let mut lifecycle = program_local_lifecycle(
        740,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 840, 10, "TestRoot::entry");
    let cohort = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding.identity(),
                &root,
                lease,
            )],
        )
        .expect("exact epoch cohort");
    let mut runtime = cohort.into_runtime();
    let activation = program_local_activation(&mut lifecycle, 940, 10);

    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_subject(&root, &activation, 1040, Some(8)),
        )
        .expect("exact installed subject establishes its root");
    assert_eq!(
        established
            .capacity()
            .counted_quantity()
            .expect("counted capacity"),
        &numerics::bignum::BigInt::from_u64(9)
    );
    assert_eq!(
        established.lineage().occurrence(),
        established.occurrence_identity()
    );
    assert_eq!(established.prebinding().argument_index(), 0);
    assert_eq!(established.scalar_observations().len(), 1);
    assert_eq!(runtime.pending_occurrences().len(), 0);
    assert_eq!(runtime.aggregates().len(), 1);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));

    let replay_activation = program_local_activation(&mut lifecycle, 941, 10);
    let replay = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &replay_activation,
            program_local_subject(&root, &replay_activation, 1041, Some(8)),
        )
        .expect_err("the exact installed occurrence establishes at most once");
    assert!(replay.diagnostic().0.contains("no pending exact cohort"));
    assert_eq!(
        replay.into_subject().invocation().normalized_identity(),
        941
    );

    let mut substituted = program_local_lifecycle(
        741,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let retirement = installation
        .retire_established(established, &mut substituted)
        .expect_err("a foreign lifecycle cannot retire the established root");
    assert!(retirement.diagnostic().0.contains("lifecycle lease"));
    let established = (*retirement).into_root();
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));
    installation
        .retire_established(established, &mut lifecycle)
        .expect("the exact lifecycle retires the root");
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

#[test]
fn receiver_cleanup_occupancy_tracks_the_active_extent_until_retirement() {
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
        .expect("one producer schema");
    let mut lifecycle = program_local_lifecycle(
        750,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 850, 10, "TestRoot::entry");
    let cohort = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding.identity(),
                &root,
                lease,
            )],
        )
        .expect("exact epoch cohort");
    let mut runtime = cohort.into_runtime();
    let activation = program_local_activation(&mut lifecycle, 950, 10);

    // A still-pending occurrence has not proven its extent: cleanup cannot
    // occupy it through the ledger.
    let pending_identity = runtime
        .pending_occurrences()
        .next()
        .expect("the sealed cohort carries a pending occurrence")
        .identity;
    assert!(
        installation
            .track_receiver_cleanup_occupancy(pending_identity)
            .is_err(),
        "cleanup occupancy cannot attach to a pending occurrence"
    );

    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_subject(&root, &activation, 1050, Some(8)),
        )
        .expect("exact installed subject establishes its root");
    let identity = established.occurrence_identity();

    // A receiver whose cleanup must occupy its hosted extent keeps that
    // extent tracked through the ledger for the occupancy's active lifetime.
    installation
        .track_receiver_cleanup_occupancy(identity)
        .expect("the established occurrence tracks its cleanup occupancy");
    assert!(
        installation
            .cleanup_occupancies()
            .any(|occupancy| *occupancy == identity),
        "the occupancy is tracked against the exact occurrence"
    );
    // Retracking the same extent records no second occupancy row.
    installation
        .track_receiver_cleanup_occupancy(identity)
        .expect("occupancy tracking is idempotent");
    assert_eq!(installation.cleanup_occupancies().count(), 1);

    installation
        .retire_established(established, &mut lifecycle)
        .expect("the exact lifecycle retires the root");
    // Completion discharges the occupancy; a retired extent cannot occupy.
    assert_eq!(installation.cleanup_occupancies().count(), 0);
    assert!(
        installation
            .track_receiver_cleanup_occupancy(identity)
            .is_err(),
        "a retired extent cannot host cleanup occupancy"
    );
}

#[test]
fn aggregate_capacity_reconstruction_sums_the_live_group_for_one_epoch() {
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
        .expect("one producer schema");
    let mut lifecycle = program_local_lifecycle(
        790,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 890, 10, "TestRoot::entry");
    let mut runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding.identity(),
                &root,
                lease,
            )],
        )
        .expect("exact epoch cohort")
        .into_runtime();
    let activation = program_local_activation(&mut lifecycle, 990, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_subject(&root, &activation, 1090, Some(8)),
        )
        .expect("exact counted subject establishes its root");
    let reconstructed = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&established])
        .expect("the complete live group reconstructs its aggregate capacity");
    assert_eq!(
        reconstructed
            .cohort()
            .installed_code()
            .normalized_identity(),
        code_identity
    );
    assert_eq!(
        reconstructed.cohort().lifecycle_ledger(),
        lifecycle.identity()
    );
    assert_eq!(reconstructed.cohort().lifecycle_epoch(), 10);
    assert_eq!(
        reconstructed
            .aggregate()
            .occurrence_identities()
            .collect::<Vec<_>>(),
        vec![established.occurrence_identity()]
    );
    assert_eq!(reconstructed.aggregate().cardinality().get(), 1);
    assert_eq!(
        reconstructed.capacity(),
        &EstablishedProgramLocalRootCapacity::CountedQuantity(numerics::bignum::BigInt::from_u64(
            9
        ))
    );

    let substituted_lifecycle = program_local_lifecycle(
        791,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let rejected = installation
        .reconstruct_aggregate_capacity(&substituted_lifecycle, [&established])
        .expect_err("a foreign lifecycle cannot reconstruct the epoch aggregate");
    assert!(rejected.0.contains("live lease"));

    let retired = installation
        .retire_established(established, &mut lifecycle)
        .expect("the exact lifecycle retires the root");
    assert_eq!(
        retired.identity(),
        reconstructed
            .aggregate()
            .occurrence_identities()
            .next()
            .expect("single live member")
    );
    let rejected = installation
        .reconstruct_aggregate_capacity(
            &lifecycle,
            std::iter::empty::<&EstablishedProgramLocalRoot>(),
        )
        .expect_err("an empty roster cannot reconstruct the epoch aggregate");
    assert!(rejected.0.contains("at least one"));
}

#[test]
fn aggregate_capacity_reconstruction_composes_the_interval_member_set() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_extent_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, root, _open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let [prebinding] = installation
        .derive_eligible_prebindings(&catalog, &terminal, [&root])
        .expect("verified installed Extent prebinding")
        .try_into()
        .expect("one producer schema");
    let mut lifecycle = program_local_lifecycle(
        790,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 890, 10, "TestRoot::entry");
    let mut runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding.identity(),
                &root,
                lease,
            )],
        )
        .expect("exact Extent epoch cohort")
        .into_runtime();
    let activation = program_local_activation(&mut lifecycle, 990, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1090, 0x4000, 0x100),
        )
        .expect("exact interval subject establishes its root");
    let reconstructed = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&established])
        .expect("the live interval group reconstructs its separated set");
    assert_eq!(
        reconstructed.capacity(),
        &EstablishedProgramLocalRootCapacity::IntervalSet(
            language_semantics::content::CanonicalIntervalSet::singleton(
                numerics::bignum::BigInt::from_u64(0x4000),
                numerics::bignum::BigInt::from_u64(0x4100),
            )
            .expect("exact member interval")
        )
    );
    let retired = installation
        .retire_established(established, &mut lifecycle)
        .expect("the exact lifecycle retires the root");
    assert_eq!(retired.identity().lifecycle_epoch(), 10);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

#[test]
fn aggregate_capacity_reconstruction_rejects_mixed_schemas_cohorts_and_installations() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_two_schema_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let claims = vec![program_local_claim_at(0), program_local_claim_at(1)];
    let (mut root_ledger, root, _open_root) =
        install_program_local_two_parameter_roots(&mut code, entry, claims);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let [first_prebinding, second_prebinding] = installation
        .derive_eligible_prebindings(&catalog, &terminal, [&root])
        .expect("two verified installed prebindings")
        .try_into()
        .expect("two producer schemas");
    let mut lifecycle = program_local_lifecycle(
        790,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let first_lease = program_local_epoch_lease(&mut lifecycle, 890, 10, "TestRoot::entry");
    let second_lease = program_local_epoch_lease(&mut lifecycle, 891, 10, "TestRoot::entry");
    let mut runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [
                ProgramLocalRootCohortMember::new(first_prebinding.identity(), &root, first_lease),
                ProgramLocalRootCohortMember::new(
                    second_prebinding.identity(),
                    &root,
                    second_lease,
                ),
            ],
        )
        .expect("exact two-schema epoch cohort")
        .into_runtime();
    let activation = program_local_activation(&mut lifecycle, 990, 10);
    let first = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_subject_at(&root, &activation, 1090, 0, 0, Some(8)),
        )
        .expect("first schema establishes on its exact parameter subject");
    let second = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_subject_at(&root, &activation, 1091, 1, 1, Some(4)),
        )
        .expect("second schema establishes on its exact parameter subject");

    let mixed = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&first, &second])
        .expect_err("two aggregate schema groups cannot merge into one roster");
    assert!(mixed.0.contains("distinct aggregate schemas"));
    let repeated = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&first, &first])
        .expect_err("one occurrence cannot supply two roster members");
    assert!(repeated.0.contains("repeats one exact occurrence"));

    let first_group = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&first])
        .expect("the first exact group reconstructs alone");
    assert_eq!(
        first_group.capacity(),
        &EstablishedProgramLocalRootCapacity::CountedQuantity(numerics::bignum::BigInt::from_u64(
            9
        ))
    );
    let second_group = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&second])
        .expect("the second exact group reconstructs alone");
    assert_eq!(
        second_group.capacity(),
        &EstablishedProgramLocalRootCapacity::CountedQuantity(numerics::bignum::BigInt::from_u64(
            5
        ))
    );
    assert_ne!(
        first_group
            .aggregate()
            .occurrence_identities()
            .collect::<Vec<_>>(),
        second_group
            .aggregate()
            .occurrence_identities()
            .collect::<Vec<_>>()
    );

    publish_program_local_era(
        &mut lifecycle,
        11,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
        111,
        true,
    );
    let next_first_lease = program_local_epoch_lease(&mut lifecycle, 892, 11, "TestRoot::entry");
    let next_second_lease = program_local_epoch_lease(&mut lifecycle, 893, 11, "TestRoot::entry");
    let mut next_runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [
                ProgramLocalRootCohortMember::new(
                    first_prebinding.identity(),
                    &root,
                    next_first_lease,
                ),
                ProgramLocalRootCohortMember::new(
                    second_prebinding.identity(),
                    &root,
                    next_second_lease,
                ),
            ],
        )
        .expect("the next exact epoch cohort")
        .into_runtime();
    let next_activation = program_local_activation(&mut lifecycle, 992, 11);
    let next_first = installation
        .establish(
            &mut next_runtime,
            &lifecycle,
            &next_activation,
            program_local_subject_at(&root, &next_activation, 1092, 0, 0, Some(6)),
        )
        .expect("the same schema establishes in the next epoch");
    let spanned = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&next_first, &first])
        .expect_err("two lifecycle cohorts cannot merge into one roster");
    assert!(spanned.0.contains("distinct lifecycle cohorts"));
    let stale = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&first])
        .expect_err("the closed epoch's roster no longer reconstructs");
    assert!(stale.0.contains("live lease"));
    let next_group = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&next_first])
        .expect("the next epoch's exact group reconstructs alone");
    assert_eq!(next_group.cohort().lifecycle_epoch(), 11);
    assert_eq!(
        next_group.capacity(),
        &EstablishedProgramLocalRootCapacity::CountedQuantity(numerics::bignum::BigInt::from_u64(
            7
        ))
    );

    let mut foreign_code = installed_code(2, entry);
    let foreign_code_identity = foreign_code.identity().normalized_identity();
    let (mut foreign_root_ledger, foreign_root, _foreign_open) =
        install_program_local_two_parameter_roots(
            &mut foreign_code,
            entry,
            vec![program_local_claim_at(0), program_local_claim_at(1)],
        );
    let mut foreign_installation = foreign_root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("foreign program-local cohort verifier");
    let [foreign_first_prebinding, foreign_second_prebinding] = foreign_installation
        .derive_eligible_prebindings(&catalog, &terminal, [&foreign_root])
        .expect("foreign verified installed prebindings")
        .try_into()
        .expect("two foreign producer schemas");
    let mut foreign_lifecycle = program_local_lifecycle(
        792,
        10,
        foreign_root.installed_artifact_occurrence_digest(),
        foreign_code_identity,
        "TestRoot::entry",
    );
    let foreign_first_lease =
        program_local_epoch_lease(&mut foreign_lifecycle, 894, 10, "TestRoot::entry");
    let foreign_second_lease =
        program_local_epoch_lease(&mut foreign_lifecycle, 895, 10, "TestRoot::entry");
    let mut foreign_runtime = foreign_installation
        .seal_epoch_cohort(
            &foreign_lifecycle,
            [
                ProgramLocalRootCohortMember::new(
                    foreign_first_prebinding.identity(),
                    &foreign_root,
                    foreign_first_lease,
                ),
                ProgramLocalRootCohortMember::new(
                    foreign_second_prebinding.identity(),
                    &foreign_root,
                    foreign_second_lease,
                ),
            ],
        )
        .expect("foreign exact epoch cohort")
        .into_runtime();
    let foreign_activation = program_local_activation(&mut foreign_lifecycle, 993, 10);
    let foreign_first = foreign_installation
        .establish(
            &mut foreign_runtime,
            &foreign_lifecycle,
            &foreign_activation,
            program_local_subject_at(&foreign_root, &foreign_activation, 1093, 0, 0, Some(3)),
        )
        .expect("the foreign schema establishes in its own installation");
    let foreign = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&foreign_first])
        .expect_err("a foreign installation's occurrence cannot enter this aggregate");
    assert!(foreign.0.contains("no exact established occurrence"));
    let foreign_group = foreign_installation
        .reconstruct_aggregate_capacity(&foreign_lifecycle, [&foreign_first])
        .expect("the foreign installation reconstructs its own exact group");
    assert_eq!(
        foreign_group
            .cohort()
            .installed_code()
            .normalized_identity(),
        foreign_code_identity
    );
}

#[test]
fn entry_activation_is_the_only_invocation_authority_for_establishment() {
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
        .expect("one producer schema");
    // The cohort ledger starts in era 9 so one activation can be entered there
    // and held while era 10 publishes: the held token then carries a stale era.
    let mut lifecycle = program_local_lifecycle(
        750,
        9,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let stale_activation = program_local_activation(&mut lifecycle, 950, 9);
    publish_program_local_era(
        &mut lifecycle,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
        850,
        true,
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 851, 10, "TestRoot::entry");
    let mut runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding.identity(),
                &root,
                lease,
            )],
        )
        .expect("exact epoch cohort")
        .into_runtime();
    let activation = program_local_activation(&mut lifecycle, 951, 10);

    // A subject observed under an activation that entered era 9 cannot
    // establish the era-10 cohort even though it presents the same ledger.
    let stale_subject = program_local_subject(&root, &stale_activation, 1050, Some(8));
    let stale = installation
        .establish(&mut runtime, &lifecycle, &stale_activation, stale_subject)
        .expect_err("a stale-era activation cannot establish the exact epoch");
    assert!(
        stale
            .diagnostic()
            .0
            .contains("not entered on the cohort's exact lifecycle ledger and epoch")
    );
    assert_eq!(stale.into_subject().invocation().normalized_identity(), 950);
    assert_eq!(runtime.pending_occurrences().len(), 1);

    // A subject observed under an activation of a different lifecycle ledger —
    // with the same era number and entry contract — is not this cohort's entry.
    let mut foreign_lifecycle = program_local_lifecycle(
        751,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let foreign_activation = program_local_activation(&mut foreign_lifecycle, 952, 10);
    let foreign_subject = program_local_subject(&root, &foreign_activation, 1051, Some(8));
    let foreign = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &foreign_activation,
            foreign_subject,
        )
        .expect_err("a foreign ledger's activation cannot establish this cohort");
    assert!(
        foreign
            .diagnostic()
            .0
            .contains("not entered on the cohort's exact lifecycle ledger and epoch")
    );
    assert_eq!(
        foreign.into_subject().invocation().normalized_identity(),
        952
    );
    assert_eq!(runtime.pending_occurrences().len(), 1);

    // A subject stamped by one live activation cannot be established under
    // another activation of the same era: the invocation identity is minted by
    // the exact presented scope, not by the caller.
    let other_activation = program_local_activation(&mut lifecycle, 953, 10);
    let redirected = program_local_subject(&root, &other_activation, 1052, Some(8));
    let rejected = installation
        .establish(&mut runtime, &lifecycle, &activation, redirected)
        .expect_err("a subject stamped by another activation cannot establish under this one");
    assert!(
        rejected
            .diagnostic()
            .0
            .contains("not observed under the presented entry activation")
    );
    assert_eq!(
        rejected.into_subject().invocation().normalized_identity(),
        953
    );
    assert_eq!(runtime.pending_occurrences().len(), 1);

    // The matching activation establishes, and the same activation completes
    // its scope through the ledger that minted it.
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_subject(&root, &activation, 1053, Some(8)),
        )
        .expect("the presented activation's own subject establishes");
    assert_eq!(runtime.pending_occurrences().len(), 0);
    let receipt = activation.leave_receipt(true);
    activation
        .leave(&mut lifecycle, receipt)
        .expect("the exact activation leaves through its minting ledger");

    // Leave through a foreign ledger rejects and returns the scope intact so
    // the rightful ledger can still complete it.
    let receipt = other_activation.leave_receipt(true);
    let leave = other_activation
        .leave(&mut foreign_lifecycle, receipt)
        .expect_err("an activation cannot leave through a different ledger");
    assert!(
        leave
            .diagnostic()
            .0
            .contains("cannot leave through a different lifecycle ledger")
    );
    let (other_activation, receipt) = leave.into_parts();
    other_activation
        .leave(&mut lifecycle, receipt)
        .expect("the returned activation still completes on its own ledger");

    // The stale-era token's entry hold also settles on the same ledger.
    let receipt = stale_activation.leave_receipt(true);
    stale_activation
        .leave(&mut lifecycle, receipt)
        .expect("a held stale-era activation still leaves its own era");
    let receipt = foreign_activation.leave_receipt(true);
    foreign_activation
        .leave(&mut foreign_lifecycle, receipt)
        .expect("the foreign activation leaves its own ledger");

    installation
        .retire_established(established, &mut lifecycle)
        .expect("the exact lifecycle retires the root");
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

use super::{
    entry_id, extent_id, extent_provider_issuance, install_program_local_required_root,
    install_program_local_two_parameter_roots, installed_backing_extent, installed_code,
    installed_code_with_fill_and_installation_identity, program_local_activation,
    program_local_claim, program_local_claim_at, program_local_epoch_lease,
    program_local_extent_module, program_local_extent_subject, program_local_extent_subject_at,
    program_local_lifecycle, program_local_root_catalog, program_local_root_module,
    program_local_subject, program_local_terminal_object, program_local_two_schema_extent_module,
    publish_program_local_era,
};
use crate::{
    EstablishedProgramLocalRoot, ProgramLocalExtentRegistry, ProgramLocalRootCohortMember,
    RetainedForeignAccess, RetainedForeignArgumentDisposition, RetainedForeignArgumentRange,
    RetainedForeignArgumentRequest,
};
use extents::{
    AddressSpaceId, Extent, ExtentLineageId, ExtentProvenanceId, ExtentRightId, ExtentRights,
    ExtentRootGrant, MappingEraId,
};

#[test]
fn program_local_extent_registry_retains_exact_account_through_split_and_retirement() {
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
        780,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 880, 10, "TestRoot::entry");
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
    let activation = program_local_activation(&mut lifecycle, 980, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1080, 0x4000, 0x100),
        )
        .expect("exact interval subject establishes its root");
    let mut registry = ProgramLocalExtentRegistry::new();
    let extent = registry
        .materialize(
            established,
            installed_backing_extent(700, 0x4000, 0x100, 30),
        )
        .expect("established interval materializes over its installed backing");
    assert_eq!(extent.base(), 0x4000);
    assert_eq!(extent.length(), 0x100);
    assert_eq!(
        extent.address_space(),
        extent_id(10, AddressSpaceId::from_normalized_identity)
    );
    assert_eq!(
        extent.provenance(),
        extent_id(20, ExtentProvenanceId::from_normalized_identity)
    );
    assert_eq!(
        extent.era(),
        extent_id(30, MappingEraId::from_normalized_identity)
    );
    let origin = extent
        .program_local_origin()
        .expect("passive program-local origin");
    assert_eq!(origin.installed_code(), code_identity);
    assert_eq!(origin.lifecycle_ledger(), 780);
    assert_eq!(origin.lifecycle_epoch(), 10);
    assert_eq!(origin.entry_invocation(), 980);
    assert_eq!(origin.subject_place(), 1080);
    assert_eq!(registry.held_accounts(), 1);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));

    let (lower, upper) = extent.split_at(0x40).expect("split program-local Extent");
    let rejected = registry
        .retire(lower, &mut installation, &mut lifecycle)
        .expect_err("a split descendant cannot retire the account");
    assert!(rejected.diagnostic().0.contains("recombined root"));
    let lower = (*rejected).into_extent();
    assert_eq!(registry.held_accounts(), 1);
    let extent = lower.merge(upper).expect("recombine exact root Extent");

    let mut substituted = program_local_lifecycle(
        781,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let rejected = registry
        .retire(extent, &mut installation, &mut substituted)
        .expect_err("a foreign lifecycle cannot release the retained occurrence");
    assert!(rejected.diagnostic().0.contains("lifecycle lease"));
    let extent = (*rejected).into_extent();
    assert_eq!(registry.held_accounts(), 1);
    let retired = registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect("exact recombined root releases its lifecycle account");
    assert_eq!(registry.held_accounts(), 0);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
    let backing = retired.into_backing();
    assert_eq!(backing.base(), 0x4000);
    assert_eq!(backing.length(), 0x100);
    assert_eq!(
        backing.lineage_root(),
        extent_id(710, ExtentLineageId::from_normalized_identity)
    );
    assert!(
        backing
            .provider_issuance()
            .is_some_and(|issuance| issuance == extent_provider_issuance(700))
    );
}

#[test]
fn aggregate_materialization_discharges_reconstructed_capacity_over_installed_partitions() {
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
        780,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 880, 10, "TestRoot::entry");
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
    let activation = program_local_activation(&mut lifecycle, 980, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1080, 0x4040, 0x100),
        )
        .expect("exact interval subject establishes its root");
    let aggregate = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&established])
        .expect("the live group reconstructs its aggregate capacity");

    // The receiver partition is carved out of provider-issued installed
    // image backing; the before/after residuals stay outside the member's
    // authority for the activation's duration.
    let partition = installed_backing_extent(700, 0x4000, 0x200, 30)
        .partition_owned(0x40, 0x100)
        .expect("exact receiver partition");
    let (before, backing, after) = partition.into_parts();
    let before = before.expect("lower installed residual");
    let after = after.expect("upper installed residual");

    let mut registry = ProgramLocalExtentRegistry::new();
    let [mut extent] = registry
        .materialize_aggregate(
            &installation,
            &lifecycle,
            &aggregate,
            vec![(established, backing)],
        )
        .expect("reconstructed capacity discharges over the exact partition")
        .try_into()
        .expect("one minted program-local Extent");
    assert_eq!(extent.base(), 0x4040);
    assert_eq!(extent.length(), 0x100);
    let origin = extent.program_local_origin().expect("program-local origin");
    assert_eq!(origin.installed_code(), code_identity);
    assert_eq!(origin.lifecycle_epoch(), 10);
    assert_eq!(origin.entry_invocation(), 980);
    assert_eq!(registry.held_accounts(), 1);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));

    // The activation borrows a partition subrange rather than owning it; the
    // checked loan carries the same program-local origin and ends before
    // completion.
    {
        let loan = registry
            .loan_mut_under_activation(&activation, &mut extent, 0x10, 0x20)
            .expect("exclusive activation loan");
        assert_eq!(loan.base(), 0x4050);
        assert_eq!(loan.length(), 0x20);
        assert_eq!(loan.program_local_origin(), Some(origin));
    }

    let retired = registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect("recombined root completes the occurrence");
    assert_eq!(registry.held_accounts(), 0);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));

    // Completion returns the exact receiver partition, which rejoins the
    // installed image it was carved from.
    let tail = retired
        .into_backing()
        .merge(after)
        .expect("receiver partition rejoins its upper residual");
    let restored = before
        .merge(tail)
        .expect("installed image restores around the discharged range");
    assert!(restored.is_lineage_root());
    assert_eq!(restored.base(), 0x4000);
    assert_eq!(restored.length(), 0x200);
    assert!(
        restored
            .provider_issuance()
            .is_some_and(|issuance| issuance == extent_provider_issuance(700))
    );
}

#[test]
fn aggregate_materialization_rejects_stale_substituted_and_inexact_discharge() {
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
        780,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 880, 10, "TestRoot::entry");
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
    let activation = program_local_activation(&mut lifecycle, 980, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1080, 0x4040, 0x100),
        )
        .expect("exact interval subject establishes its root");
    let stale_aggregate = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&established])
        .expect("the live group reconstructs its aggregate capacity");

    publish_program_local_era(
        &mut lifecycle,
        11,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
        111,
        true,
    );
    let next_lease = program_local_epoch_lease(&mut lifecycle, 881, 11, "TestRoot::entry");
    let mut next_runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding.identity(),
                &root,
                next_lease,
            )],
        )
        .expect("the next exact Extent epoch cohort")
        .into_runtime();
    let next_activation = program_local_activation(&mut lifecycle, 981, 11);
    let next_established = installation
        .establish(
            &mut next_runtime,
            &lifecycle,
            &next_activation,
            program_local_extent_subject(&root, &next_activation, 1081, 0x4040, 0x100),
        )
        .expect("the same schema establishes in the next epoch");
    let next_identity = next_established.occurrence_identity();
    let aggregate = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&next_established])
        .expect("the next epoch's group reconstructs its aggregate capacity");

    let mut registry = ProgramLocalExtentRegistry::new();
    let stale = registry
        .materialize_aggregate(
            &installation,
            &lifecycle,
            &stale_aggregate,
            vec![(
                next_established,
                installed_backing_extent(700, 0x4040, 0x100, 30),
            )],
        )
        .expect_err("a closed epoch's aggregate cannot discharge the live membership");
    assert!(stale.diagnostic().0.contains("stale or substituted"));
    let next_established = stale
        .into_inputs()
        .pop()
        .expect("rejection returns the presented member")
        .0;
    assert_eq!(next_established.occurrence_identity(), next_identity);

    let substituted_lifecycle = program_local_lifecycle(
        781,
        11,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let foreign = registry
        .materialize_aggregate(
            &installation,
            &substituted_lifecycle,
            &aggregate,
            vec![(
                next_established,
                installed_backing_extent(700, 0x4040, 0x100, 30),
            )],
        )
        .expect_err("a foreign lifecycle cannot reconstruct the discharge requirement");
    assert!(foreign.diagnostic().0.contains("live lease"));
    let next_established = foreign
        .into_inputs()
        .pop()
        .expect("rejection returns the presented member")
        .0;

    let inexact = registry
        .materialize_aggregate(
            &installation,
            &lifecycle,
            &aggregate,
            vec![(
                next_established,
                installed_backing_extent(700, 0x5000, 0x100, 30),
            )],
        )
        .expect_err("backing outside the member's evaluated interval rejects");
    assert!(
        inexact
            .diagnostic()
            .0
            .contains("does not equal its established interval capacity")
    );
    let next_established = inexact
        .into_inputs()
        .pop()
        .expect("rejection returns the presented member")
        .0;
    assert_eq!(next_established.occurrence_identity(), next_identity);
    assert_eq!(registry.held_accounts(), 0);
}

#[test]
fn counted_aggregate_capacity_cannot_discharge_extent_partitions() {
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
        780,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 880, 10, "TestRoot::entry");
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
    let activation = program_local_activation(&mut lifecycle, 980, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_subject(&root, &activation, 1080, Some(8)),
        )
        .expect("exact counted subject establishes its root");
    let aggregate = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&established])
        .expect("the live counted group reconstructs its aggregate capacity");

    let mut registry = ProgramLocalExtentRegistry::new();
    let rejected = registry
        .materialize_aggregate(
            &installation,
            &lifecycle,
            &aggregate,
            vec![(
                established,
                installed_backing_extent(700, 0x4000, 0x100, 30),
            )],
        )
        .expect_err("a counted aggregate names no interval requirement to partition");
    assert!(
        rejected
            .diagnostic()
            .0
            .contains("counted program-local aggregate")
    );

    let rejected = registry
        .retire_aggregate(&mut installation, &mut lifecycle, &aggregate, Vec::new())
        .expect_err("a counted aggregate names no Extent-partitioned membership to complete");
    assert!(
        rejected
            .diagnostic()
            .0
            .contains("counted program-local aggregate")
    );
}

#[test]
fn program_local_extent_loans_require_the_establishing_activation() {
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
    // The cohort ledger starts in era 9 so one activation can be entered
    // there and held while era 10 publishes: the held token then carries a
    // stale epoch for the era-10 occurrence.
    let mut lifecycle = program_local_lifecycle(
        780,
        9,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let stale_activation = program_local_activation(&mut lifecycle, 979, 9);
    publish_program_local_era(
        &mut lifecycle,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
        110,
        true,
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 880, 10, "TestRoot::entry");
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
    let activation = program_local_activation(&mut lifecycle, 980, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1080, 0x4000, 0x100),
        )
        .expect("exact interval subject establishes its root");
    let mut registry = ProgramLocalExtentRegistry::new();
    let mut extent = registry
        .materialize(
            established,
            installed_backing_extent(700, 0x4000, 0x100, 30),
        )
        .expect("established interval materializes over its installed backing");
    let origin = extent.program_local_origin().expect("program-local origin");
    assert_eq!(origin.entry_invocation(), 980);

    // The establishing activation borrows shared and exclusive subranges of
    // its own introduced root.
    {
        let shared = registry
            .loan_under_activation(&activation, &extent, 0x10, 0x20)
            .expect("shared activation loan");
        assert_eq!(shared.polarity(), extents::LoanPolarity::Shared);
        assert_eq!(shared.base(), 0x4010);
        assert_eq!(shared.length(), 0x20);
        assert_eq!(shared.program_local_origin(), Some(origin));
    }
    {
        let exclusive = registry
            .loan_mut_under_activation(&activation, &mut extent, 0x30, 0x10)
            .expect("exclusive activation loan");
        assert_eq!(exclusive.polarity(), extents::LoanPolarity::Exclusive);
        assert_eq!(exclusive.base(), 0x4030);
    }

    // A split descendant remains inside the same held account, so its
    // subrange still loans under the establishing activation.
    let (lower, upper) = extent.split_at(0x40).expect("split program-local Extent");
    {
        let descendant = registry
            .loan_under_activation(&activation, &lower, 0x10, 0x10)
            .expect("a split descendant borrows under the same activation");
        assert_eq!(descendant.base(), 0x4010);
        assert_eq!(descendant.length(), 0x10);
    }
    let extent = lower.merge(upper).expect("recombine exact root Extent");

    // Another activation entered on the same ledger and epoch is a distinct
    // invocation: it did not observe this occurrence's subject.
    let other_activation = program_local_activation(&mut lifecycle, 981, 10);
    let rejected = registry
        .loan_under_activation(&other_activation, &extent, 0x10, 0x20)
        .expect_err("another invocation of the same epoch cannot borrow the extent");
    assert!(rejected.0.contains("established the exact occurrence"));

    // An activation entered on a stale era of the same ledger is not the
    // occurrence's epoch, even though it is still live.
    let rejected = registry
        .loan_under_activation(&stale_activation, &extent, 0x10, 0x20)
        .expect_err("a stale-epoch activation cannot borrow the extent");
    assert!(
        rejected
            .0
            .contains("exact occurrence lifecycle ledger and epoch")
    );

    // A foreign ledger's activation is not this installation's occurrence
    // even with a coincidentally matching invocation identity.
    let mut foreign_lifecycle = program_local_lifecycle(
        781,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let foreign_activation = program_local_activation(&mut foreign_lifecycle, 980, 10);
    let rejected = registry
        .loan_under_activation(&foreign_activation, &extent, 0x10, 0x20)
        .expect_err("a foreign ledger's activation cannot borrow the extent");
    assert!(
        rejected
            .0
            .contains("exact occurrence lifecycle ledger and epoch")
    );

    // Provider-issued installed backing and an account held by another
    // registry are both ambient authority here.
    let ambient = installed_backing_extent(701, 0x4000, 0x100, 30);
    let rejected = registry
        .loan_under_activation(&activation, &ambient, 0x10, 0x20)
        .expect_err("provider-issued backing is not a held program-local account");
    assert!(
        rejected
            .0
            .contains("not rooted in an established program-local account")
    );
    let empty_registry = ProgramLocalExtentRegistry::new();
    let rejected = empty_registry
        .loan_under_activation(&activation, &extent, 0x10, 0x20)
        .expect_err("another registry holds no account for the extent");
    assert!(rejected.0.contains("no held program-local account"));

    // The checked route still enforces the Extent's own subrange geometry.
    let rejected = registry
        .loan_under_activation(&activation, &extent, 0x200, 0x10)
        .expect_err("a loan outside the extent range rejects");
    assert!(rejected.0.contains("exceeds"));

    // Completion for the same occurrence and epoch: the recombined root
    // retires through the ledger and the establishing activation leaves.
    let retired = registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect("exact recombined root completes the occurrence");
    assert_eq!(registry.held_accounts(), 0);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
    assert_eq!(retired.backing().base(), 0x4000);
    assert_eq!(retired.backing().length(), 0x100);

    let receipt = activation.leave_receipt(true);
    activation
        .leave(&mut lifecycle, receipt)
        .expect("the establishing activation completes its scope");
    let receipt = other_activation.leave_receipt(true);
    other_activation
        .leave(&mut lifecycle, receipt)
        .expect("the same-epoch activation completes its scope");
    let receipt = stale_activation.leave_receipt(true);
    stale_activation
        .leave(&mut lifecycle, receipt)
        .expect("the held stale-era activation completes its scope");
    let receipt = foreign_activation.leave_receipt(true);
    foreign_activation
        .leave(&mut foreign_lifecycle, receipt)
        .expect("the foreign activation completes on its own ledger");
}

#[test]
fn retained_foreign_argument_borrowed_pins_the_account_until_release() {
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
    let mut registry = ProgramLocalExtentRegistry::new();
    let extent = registry
        .materialize(
            established,
            installed_backing_extent(700, 0x4000, 0x100, 30),
        )
        .expect("established interval materializes over its installed backing");
    let exclusive = RetainedForeignArgumentRequest::new(
        0x10,
        0x20,
        RetainedForeignAccess::Exclusive,
        ExtentRights::from_normalized_identities([extent_id(
            100,
            ExtentRightId::from_normalized_identity,
        )]),
    )
    .expect("nonempty exclusive request");
    let retained = registry
        .retain_foreign_argument_borrowed(&extent, exclusive)
        .expect("exclusive borrowed retention");
    assert_eq!(retained.identity().normalized_identity(), 1);
    assert_eq!(retained.origin(), extent.program_local_origin().unwrap());
    assert_eq!(retained.base(), 0x4010);
    assert_eq!(retained.length(), 0x20);
    assert_eq!(retained.era().normalized_identity(), 30);
    assert_eq!(
        retained.disposition(),
        RetainedForeignArgumentDisposition::LifetimeBorrowed
    );

    let overlapping_exclusive = RetainedForeignArgumentRequest::new(
        0x20,
        0x10,
        RetainedForeignAccess::Exclusive,
        ExtentRights::from_normalized_identities([extent_id(
            100,
            ExtentRightId::from_normalized_identity,
        )]),
    )
    .expect("nonempty overlapping request");
    assert!(
        registry
            .retain_foreign_argument_borrowed(&extent, overlapping_exclusive)
            .expect_err("exclusive overlap")
            .0
            .contains("overlaps")
    );

    let disjoint_shared = RetainedForeignArgumentRequest::new(
        0x80,
        0x10,
        RetainedForeignAccess::Shared,
        ExtentRights::from_normalized_identities([extent_id(
            100,
            ExtentRightId::from_normalized_identity,
        )]),
    )
    .expect("nonempty disjoint request");
    let disjoint = registry
        .retain_foreign_argument_borrowed(&extent, disjoint_shared)
        .expect("disjoint shared retention");
    let overlapping_shared = RetainedForeignArgumentRequest::new(
        0x20,
        0x10,
        RetainedForeignAccess::Shared,
        ExtentRights::from_normalized_identities([extent_id(
            100,
            ExtentRightId::from_normalized_identity,
        )]),
    )
    .expect("nonempty shared overlap request");
    assert!(
        registry
            .retain_foreign_argument_borrowed(&extent, overlapping_shared)
            .expect_err("shared overlap with exclusive")
            .0
            .contains("overlaps")
    );

    let rejected = registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect_err("live retained arguments block retirement");
    assert!(rejected.diagnostic().0.contains("live retained"));
    let extent = (*rejected).into_extent();
    registry
        .release_retained_foreign_argument(retained)
        .expect("release exclusive borrowed retention");
    registry
        .release_retained_foreign_argument(disjoint)
        .expect("release shared borrowed retention");
    registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect("retirement succeeds after releases");
    assert_eq!(registry.live_retained_foreign_arguments(), 0);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

fn retained_borrow_custody(
    source_position: u32,
    access: terminal_psi::StructuralAccess,
) -> terminal_psi::BoundaryContentGuarantee {
    let semantic_domain = semantic_vocabulary::DomainSemanticId::new(41).expect("domain");
    let content_domain = semantic_vocabulary::ContentDomainId::new(41).expect("content domain");
    let projection = move |carrier: &str| terminal_psi::RetainedBorrowContentProjection {
        semantic_domain,
        carrier_identity: carrier.to_owned(),
        projection: terminal_psi::StructuralContentProjection {
            identity: semantic_vocabulary::ContentProjectionIdentity {
                domain: content_domain,
                projection_report_fingerprint: 7,
            },
            algebra: semantic_vocabulary::ContentAlgebra {
                kind: semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
                parameter: "named(name(BufferSlot))".to_owned(),
            },
            expression: semantic_vocabulary::ContentProjectionExpression::CountedQuantity(
                semantic_vocabulary::ContentProjectionScalar::Natural("1".to_owned()),
            ),
        },
    };
    terminal_psi::BoundaryContentGuarantee::RetainedBorrow(terminal_psi::RetainedBorrowCustody {
        callable_identity: "Reader::submit".to_owned(),
        source: terminal_psi::RetainedBorrowPlace {
            version: semantic_vocabulary::ContentPlaceVersion::Entry,
            root: terminal_psi::RetainedBorrowPlaceRoot::Parameter {
                position: source_position,
                identity: "buffer".to_owned(),
                is_self: false,
            },
            segments: Vec::new(),
        },
        result: terminal_psi::RetainedBorrowPlace {
            version: semantic_vocabulary::ContentPlaceVersion::Current,
            root: terminal_psi::RetainedBorrowPlaceRoot::Result,
            segments: Vec::new(),
        },
        access,
        callable_lifetime_parameter_count: 1,
        callable_lifetime_parameter_ordinal: 0,
        result_nominal_identity: "PendingRead".to_owned(),
        result_multiplicity: terminal_psi::StructuralMultiplicity::Linear,
        result_lifetime_argument_count: 1,
        result_lifetime_argument_ordinal: 0,
        result_lifetime_slot_is_erased: true,
        retained_semantic_domain: semantic_domain,
        source_projection: projection("Buffer"),
        result_projection: projection("PendingRead"),
    })
}

#[test]
fn retained_foreign_argument_under_custody_uses_the_authored_row() {
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
    let mut registry = ProgramLocalExtentRegistry::new();
    let extent = registry
        .materialize(
            established,
            installed_backing_extent(700, 0x4000, 0x100, 30),
        )
        .expect("established interval materializes over its installed backing");

    let rights = ExtentRights::from_normalized_identities([extent_id(
        100,
        ExtentRightId::from_normalized_identity,
    )]);

    // The authored row's SharedBorrow access and source parameter position
    // drive the retention; the caller supplies only the retained range and
    // the boundary's guarantee row.
    let guarantee = retained_borrow_custody(0, terminal_psi::StructuralAccess::SharedBorrow);
    let retained = registry
        .retain_foreign_argument_under_custody(
            &[&extent],
            RetainedForeignArgumentRange::new(0x10, 0x20, rights.clone()).unwrap(),
            &guarantee,
        )
        .expect("authored custody row selects the shared borrow retention");
    assert_eq!(retained.base(), 0x4010);
    assert_eq!(retained.length(), 0x20);
    assert_eq!(retained.era().normalized_identity(), 30);
    assert_eq!(
        retained.disposition(),
        RetainedForeignArgumentDisposition::LifetimeBorrowed
    );

    // A conservation row authorizes no retention.
    let conservation = terminal_psi::BoundaryContentGuarantee::Conservation(
        terminal_psi::ContentConservationGuarantee {
            structural_places: Vec::new(),
            conservation: semantic_vocabulary::ContentConservation::new(
                semantic_vocabulary::ContentAlgebra {
                    kind: semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
                    parameter: "named(name(BufferSlot))".to_owned(),
                },
                semantic_vocabulary::ContentTerm::Separate(Vec::new()),
                semantic_vocabulary::ContentTerm::Separate(Vec::new()),
            ),
            report_fingerprint: 7,
        },
    );
    assert!(
        registry
            .retain_foreign_argument_under_custody(
                &[&extent],
                RetainedForeignArgumentRange::new(0x80, 0x10, rights.clone()).unwrap(),
                &conservation,
            )
            .expect_err("conservation row authorizes no retention")
            .0
            .contains("does not authorize")
    );

    // A row whose authored access is not the shared borrow cannot retain.
    let mutable = retained_borrow_custody(0, terminal_psi::StructuralAccess::MutableBorrow);
    assert!(
        registry
            .retain_foreign_argument_under_custody(
                &[&extent],
                RetainedForeignArgumentRange::new(0x80, 0x10, rights.clone()).unwrap(),
                &mutable,
            )
            .expect_err("non-shared authored access rejects")
            .0
            .contains("shared borrow")
    );

    // The row's own source position selects the argument; a position with no
    // argument rejects rather than falling back to a caller's pick.
    let absent_source = retained_borrow_custody(3, terminal_psi::StructuralAccess::SharedBorrow);
    assert!(
        registry
            .retain_foreign_argument_under_custody(
                &[&extent],
                RetainedForeignArgumentRange::new(0x80, 0x10, rights.clone()).unwrap(),
                &absent_source,
            )
            .expect_err("source position beyond the argument list rejects")
            .0
            .contains("no argument")
    );

    // The retained source must be the authored entry-time parameter itself.
    // A `Current`-version place, a `Result` root, or the `self` receiver all
    // drift off the authored entry-parameter shape and must reject rather
    // than silently re-binding whichever argument the caller prefers.
    let mutated_sources = [
        {
            let terminal_psi::BoundaryContentGuarantee::RetainedBorrow(mut custody) =
                retained_borrow_custody(0, terminal_psi::StructuralAccess::SharedBorrow)
            else {
                unreachable!()
            };
            custody.source.version = semantic_vocabulary::ContentPlaceVersion::Current;
            custody
        },
        {
            let terminal_psi::BoundaryContentGuarantee::RetainedBorrow(mut custody) =
                retained_borrow_custody(0, terminal_psi::StructuralAccess::SharedBorrow)
            else {
                unreachable!()
            };
            custody.source.root = terminal_psi::RetainedBorrowPlaceRoot::Result;
            custody
        },
        {
            let terminal_psi::BoundaryContentGuarantee::RetainedBorrow(mut custody) =
                retained_borrow_custody(0, terminal_psi::StructuralAccess::SharedBorrow)
            else {
                unreachable!()
            };
            custody.source.root = terminal_psi::RetainedBorrowPlaceRoot::Parameter {
                position: 0,
                identity: "self".to_owned(),
                is_self: true,
            };
            custody
        },
    ];
    for custody in mutated_sources {
        assert!(
            registry
                .retain_foreign_argument_under_custody(
                    &[&extent],
                    RetainedForeignArgumentRange::new(0x80, 0x10, rights.clone()).unwrap(),
                    &terminal_psi::BoundaryContentGuarantee::RetainedBorrow(custody),
                )
                .expect_err("a non-entry-parameter custody source rejects")
                .0
                .contains("not an entry parameter")
        );
    }

    // A projected source (`buffer.field`) names interior storage, not the
    // entry parameter place the authored loan binds.
    let terminal_psi::BoundaryContentGuarantee::RetainedBorrow(mut projected) =
        retained_borrow_custody(0, terminal_psi::StructuralAccess::SharedBorrow)
    else {
        unreachable!()
    };
    projected
        .source
        .segments
        .push(semantic_vocabulary::ContentPlaceSegment::Field(
            "field".to_owned(),
        ));
    assert!(
        registry
            .retain_foreign_argument_under_custody(
                &[&extent],
                RetainedForeignArgumentRange::new(0x80, 0x10, rights.clone()).unwrap(),
                &terminal_psi::BoundaryContentGuarantee::RetainedBorrow(projected),
            )
            .expect_err("a projected custody source rejects")
            .0
            .contains("not a direct parameter")
    );

    // A range outside the argument's backing still rejects against the
    // ledger's own backing bounds.
    assert!(
        registry
            .retain_foreign_argument_under_custody(
                &[&extent],
                RetainedForeignArgumentRange::new(0x80, 0x100, rights.clone()).unwrap(),
                &guarantee,
            )
            .expect_err("range outside the argument backing rejects")
            .0
            .contains("exceeds")
    );

    // The held loan pins the account exactly like a borrowed retention.
    let rejected = registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect_err("live custody retention blocks retirement");
    assert!(rejected.diagnostic().0.contains("live retained"));
    let extent = (*rejected).into_extent();
    let released = registry
        .release_retained_foreign_argument(retained)
        .expect("release custody retention");
    assert!(released.returned().is_none());
    assert_eq!(
        released.disposition(),
        RetainedForeignArgumentDisposition::LifetimeBorrowed
    );
    registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect("retirement succeeds after release");
    assert_eq!(registry.live_retained_foreign_arguments(), 0);
}

#[test]
fn retained_foreign_argument_rejects_unknown_or_stale_backing() {
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
        791,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 891, 10, "TestRoot::entry");
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
    let activation = program_local_activation(&mut lifecycle, 991, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1091, 0x4000, 0x100),
        )
        .expect("exact interval subject establishes its root");
    let mut registry = ProgramLocalExtentRegistry::new();
    let extent = registry
        .materialize(
            established,
            installed_backing_extent(700, 0x4000, 0x100, 30),
        )
        .expect("established interval materializes over its installed backing");
    let request = RetainedForeignArgumentRequest::new(
        0,
        1,
        RetainedForeignAccess::Shared,
        ExtentRights::from_normalized_identities([extent_id(
            100,
            ExtentRightId::from_normalized_identity,
        )]),
    )
    .expect("nonempty request");
    let provider = ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(100),
        extent_id(2, ExtentLineageId::from_normalized_identity),
        extent.address_space(),
        extent.rights().clone(),
        extent.provenance(),
        extent.era(),
    )
    .mint(0x5000, 0x20)
    .expect("provider extent");
    let error = registry
        .retain_foreign_argument_borrowed(&provider, request.clone())
        .expect_err("provider backing is unknown ambient backing");
    assert!(error.0.contains("unknown ambient backing"));

    let stale = ExtentRootGrant::from_established_program_local(
        extent.program_local_origin().unwrap(),
        extent.lineage_root(),
        extent.address_space(),
        extent.rights().clone(),
        extent.provenance(),
        extent_id(31, MappingEraId::from_normalized_identity),
    )
    .mint(extent.base(), extent.length())
    .expect("stale same-origin extent");
    let error = registry
        .retain_foreign_argument_borrowed(&stale, request)
        .expect_err("stale mapping era");
    assert!(error.0.contains("revision provenance"));

    let out_of_range = RetainedForeignArgumentRequest::new(
        0xf0,
        0x20,
        RetainedForeignAccess::Shared,
        ExtentRights::from_normalized_identities([extent_id(
            100,
            ExtentRightId::from_normalized_identity,
        )]),
    )
    .expect("nonempty out-of-range request");
    assert!(
        registry
            .retain_foreign_argument_borrowed(&extent, out_of_range)
            .expect_err("range exceeds extent")
            .0
            .contains("exceeds")
    );
    let wrong_rights = RetainedForeignArgumentRequest::new(
        0,
        1,
        RetainedForeignAccess::Shared,
        ExtentRights::from_normalized_identities([extent_id(
            102,
            ExtentRightId::from_normalized_identity,
        )]),
    )
    .expect("nonempty rights request");
    assert!(
        registry
            .retain_foreign_argument_borrowed(&extent, wrong_rights)
            .expect_err("rights exceed extent")
            .0
            .contains("rights")
    );
    assert!(
        RetainedForeignArgumentRequest::new(
            0,
            0,
            RetainedForeignAccess::Shared,
            ExtentRights::none(),
        )
        .expect_err("zero-length request")
        .0
        .contains("nonempty")
    );
}

#[test]
fn retained_foreign_argument_moved_returns_exact_authority_on_release() {
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
        792,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 892, 10, "TestRoot::entry");
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
    let activation = program_local_activation(&mut lifecycle, 992, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1092, 0x4000, 0x100),
        )
        .expect("exact interval subject establishes its root");
    let mut registry = ProgramLocalExtentRegistry::new();
    let extent = registry
        .materialize(
            established,
            installed_backing_extent(700, 0x4000, 0x100, 30),
        )
        .expect("established interval materializes over its installed backing");
    let (lower, upper) = extent.split_at(0x40).expect("split program-local Extent");
    let retained = registry
        .retain_foreign_argument_moved(
            lower,
            RetainedForeignAccess::Shared,
            ExtentRights::from_normalized_identities([extent_id(
                100,
                ExtentRightId::from_normalized_identity,
            )]),
        )
        .expect("move lower extent into retention");
    assert_eq!(retained.base(), 0x4000);
    assert_eq!(retained.length(), 0x40);
    assert_eq!(
        retained.disposition(),
        RetainedForeignArgumentDisposition::Moved
    );
    let released = registry
        .release_retained_foreign_argument(retained)
        .expect("release moved retention");
    let returned = released.into_returned().expect("moved authority returns");
    assert_eq!(returned.base(), 0x4000);
    assert_eq!(returned.length(), 0x40);
    let extent = returned.merge(upper).expect("recombine exact root Extent");
    registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect("retirement succeeds after moved release");
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

#[test]
fn retained_foreign_argument_snapshot_pins_only_private_backing() {
    let entry = entry_id(1);
    let mut first_code = installed_code(1, entry);
    let first_code_identity = first_code.identity().normalized_identity();
    let module = program_local_extent_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut first_root_ledger, first_root, _first_open_root) =
        install_program_local_required_root(&mut first_code, entry, vec![program_local_claim()]);
    let mut first_installation = first_root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("first program-local cohort verifier");
    let [first_prebinding] = first_installation
        .derive_eligible_prebindings(&catalog, &terminal, [&first_root])
        .expect("first Extent prebinding")
        .try_into()
        .expect("one first producer schema");
    let mut lifecycle = program_local_lifecycle(
        793,
        10,
        first_root.installed_artifact_occurrence_digest(),
        first_code_identity,
        "TestRoot::entry",
    );
    let first_lease = program_local_epoch_lease(&mut lifecycle, 893, 10, "TestRoot::entry");
    let mut first_runtime = first_installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                first_prebinding.identity(),
                &first_root,
                first_lease,
            )],
        )
        .expect("first exact Extent epoch cohort")
        .into_runtime();
    let first_activation = program_local_activation(&mut lifecycle, 993, 10);
    let first_established = first_installation
        .establish(
            &mut first_runtime,
            &lifecycle,
            &first_activation,
            program_local_extent_subject(&first_root, &first_activation, 1093, 0x4000, 0x100),
        )
        .expect("first interval subject establishes its root");

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
        .expect("second Extent prebinding")
        .try_into()
        .expect("one second producer schema");
    let second_lease = program_local_epoch_lease(&mut lifecycle, 894, 20, "TestRoot::entry");
    let mut second_runtime = second_installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                second_prebinding.identity(),
                &second_root,
                second_lease,
            )],
        )
        .expect("second exact Extent epoch cohort")
        .into_runtime();
    let second_activation = program_local_activation(&mut lifecycle, 994, 20);
    let second_established = second_installation
        .establish(
            &mut second_runtime,
            &lifecycle,
            &second_activation,
            program_local_extent_subject(&second_root, &second_activation, 1094, 0x5000, 0x20),
        )
        .expect("second interval subject establishes its root");

    let mut registry = ProgramLocalExtentRegistry::new();
    let source = registry
        .materialize(
            first_established,
            installed_backing_extent(700, 0x4000, 0x100, 30),
        )
        .expect("source interval materializes over its installed backing");
    let backing = registry
        .materialize(
            second_established,
            installed_backing_extent(701, 0x5000, 0x20, 20),
        )
        .expect("snapshot backing materializes over its installed backing");
    let backing_origin = backing.program_local_origin().unwrap();
    let request = RetainedForeignArgumentRequest::new(
        0x10,
        0x20,
        RetainedForeignAccess::Shared,
        ExtentRights::from_normalized_identities([extent_id(
            100,
            ExtentRightId::from_normalized_identity,
        )]),
    )
    .expect("snapshot request");
    let retained = registry
        .retain_foreign_argument_snapshot(&source, request, backing)
        .expect("private backing snapshot retention");
    assert_eq!(retained.origin(), backing_origin);
    assert_eq!(
        retained.disposition(),
        RetainedForeignArgumentDisposition::Snapshot
    );
    assert_eq!(registry.live_retained_foreign_arguments(), 1);
    registry
        .retire(source, &mut first_installation, &mut lifecycle)
        .expect("snapshot does not pin source account");
    let released = registry
        .release_retained_foreign_argument(retained)
        .expect("release snapshot backing");
    let backing = released.into_returned().expect("snapshot backing returns");
    assert_eq!(backing.base(), 0x5000);
    assert_eq!(backing.length(), 0x20);
    registry
        .retire(backing, &mut second_installation, &mut lifecycle)
        .expect("retire snapshot backing");
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
    assert_eq!(lifecycle.program_local_root_authority_holds(20), Some(0));
}

#[test]
fn counted_program_local_capacity_cannot_mint_an_extent() {
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
        .expect("verified installed counted prebinding")
        .try_into()
        .expect("one producer schema");
    let mut lifecycle = program_local_lifecycle(
        782,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 882, 10, "TestRoot::entry");
    let mut runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding.identity(),
                &root,
                lease,
            )],
        )
        .expect("counted epoch cohort")
        .into_runtime();
    let activation = program_local_activation(&mut lifecycle, 982, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_subject(&root, &activation, 1082, Some(8)),
        )
        .expect("counted root establishes");
    let mut registry = ProgramLocalExtentRegistry::new();
    let rejected = registry
        .materialize(established, installed_backing_extent(702, 0x5000, 9, 30))
        .expect_err("counted authority has no one-Extent interpretation");
    assert!(rejected.diagnostic().0.contains("counted"));
    let [(established, _backing)]: [(EstablishedProgramLocalRoot<'_, '_>, Extent); 1] = (*rejected)
        .into_inputs()
        .try_into()
        .expect("rejection returns the exact account and backing");
    assert_eq!(registry.held_accounts(), 0);
    installation
        .retire_established(established, &mut lifecycle)
        .expect("rejected materialization returns retireable account");
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

#[test]
fn program_local_extent_materialization_requires_actual_installed_backing() {
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
        783,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 883, 10, "TestRoot::entry");
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
    let mut registry = ProgramLocalExtentRegistry::new();

    // A backing range that does not equal the evaluated interval capacity
    // rejects transactionally and returns the complete account plus backing.
    let activation = program_local_activation(&mut lifecycle, 983, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1083, 0x4000, 0x100),
        )
        .expect("exact interval subject establishes its root");
    let rejected = registry
        .materialize(established, installed_backing_extent(703, 0x4000, 0x80, 30))
        .expect_err("backing smaller than the established interval rejects");
    assert!(
        rejected
            .diagnostic()
            .0
            .contains("does not equal its established interval capacity")
    );
    let [(established, _backing)]: [(EstablishedProgramLocalRoot<'_, '_>, Extent); 1] = (*rejected)
        .into_inputs()
        .try_into()
        .expect("rejection returns the exact account and backing");
    assert_eq!(registry.held_accounts(), 0);

    // Another program-local account's authority is not installed backing:
    // consuming it would reticket that authority under a second occurrence
    // while stranding the held account's lifecycle lease.
    let program_local_backing = ExtentRootGrant::from_established_program_local(
        extents::ExtentProgramLocalOrigin::from_normalized_identities([
            11, 12, 13, 14, 15, 16, 17, 18,
        ])
        .expect("program-local origin identities"),
        extent_id(30, ExtentLineageId::from_normalized_identity),
        extent_id(10, AddressSpaceId::from_normalized_identity),
        ExtentRights::from_normalized_identities([extent_id(
            100,
            ExtentRightId::from_normalized_identity,
        )]),
        extent_id(20, ExtentProvenanceId::from_normalized_identity),
        extent_id(30, MappingEraId::from_normalized_identity),
    )
    .mint(0x4000, 0x100)
    .expect("program-local authority extent");
    let rejected = registry
        .materialize(established, program_local_backing)
        .expect_err("held program-local authority is not installed backing");
    assert!(rejected.diagnostic().0.contains("actual installed backing"));
    let [(established, _backing)]: [(EstablishedProgramLocalRoot<'_, '_>, Extent); 1] = (*rejected)
        .into_inputs()
        .try_into()
        .expect("rejection returns the exact account and backing");
    assert_eq!(registry.held_accounts(), 0);

    // The minted Extent derives every runtime fact from the exact consumed
    // backing; no ambient plan can substitute a different space, provenance,
    // era, or rights set.
    let extent = registry
        .materialize(
            established,
            installed_backing_extent(704, 0x4000, 0x100, 30),
        )
        .expect("established interval materializes over its installed backing");
    assert_eq!(extent.base(), 0x4000);
    assert_eq!(extent.length(), 0x100);
    assert_eq!(
        extent.address_space(),
        extent_id(10, AddressSpaceId::from_normalized_identity)
    );
    assert_eq!(
        extent.provenance(),
        extent_id(20, ExtentProvenanceId::from_normalized_identity)
    );
    assert_eq!(
        extent.era(),
        extent_id(30, MappingEraId::from_normalized_identity)
    );
    assert_eq!(
        extent.rights().identities().collect::<Vec<_>>(),
        vec![
            extent_id(100, ExtentRightId::from_normalized_identity),
            extent_id(101, ExtentRightId::from_normalized_identity),
        ]
    );

    // Retirement completes the account's custody by returning the exact
    // installed backing that was consumed at materialization.
    let retired = registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect("retirement returns the consumed installed backing");
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
    assert_eq!(
        retired.occurrence().epoch_lease().normalized_identity(),
        883
    );
    let backing = retired.into_backing();
    assert_eq!(backing.base(), 0x4000);
    assert_eq!(backing.length(), 0x100);
    assert_eq!(
        backing.lineage_root(),
        extent_id(714, ExtentLineageId::from_normalized_identity)
    );
    assert_eq!(
        backing.provider_issuance(),
        Some(extent_provider_issuance(704))
    );
}

#[test]
fn subject_capacity_rejection_is_transactional_and_a_later_epoch_is_fresh() {
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
    let prebinding = prebinding.identity();
    let mut lifecycle = program_local_lifecycle(
        750,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 850, 10, "TestRoot::entry");
    let mut runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(prebinding, &root, lease)],
        )
        .expect("first epoch cohort")
        .into_runtime();

    let batch_activation = program_local_activation(&mut lifecycle, 948, 10);
    let rejected_batch = installation
        .establish_batch(
            &mut runtime,
            &lifecycle,
            &batch_activation,
            [
                program_local_subject(&root, &batch_activation, 1048, Some(3)),
                program_local_subject(&root, &batch_activation, 1049, Some(3)),
            ],
        )
        .expect_err("a batch cannot establish one pending occurrence twice");
    assert!(rejected_batch.diagnostic().0.contains("repeats one exact"));
    let returned = rejected_batch.into_subjects();
    assert_eq!(returned.len(), 2);
    assert_eq!(returned[0].invocation().normalized_identity(), 948);
    assert_eq!(returned[1].invocation().normalized_identity(), 948);
    assert_eq!(runtime.pending_occurrences().len(), 1);

    let rejected = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &batch_activation,
            program_local_subject(&root, &batch_activation, 1050, None),
        )
        .expect_err("missing subject-dependent capacity rejects");
    assert!(rejected.diagnostic().0.contains("omits or adds"));
    assert_eq!(runtime.pending_occurrences().len(), 1);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));
    let first = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &batch_activation,
            program_local_subject(&root, &batch_activation, 1051, Some(3)),
        )
        .expect("failed evaluation did not burn the pending occurrence");
    let first_lineage = first.lineage();
    installation
        .retire_established(first, &mut lifecycle)
        .expect("first epoch root retirement");

    publish_program_local_era(
        &mut lifecycle,
        20,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
        125,
        true,
    );
    let next_lease = program_local_epoch_lease(&mut lifecycle, 851, 20, "TestRoot::entry");
    let mut next_runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding, &root, next_lease,
            )],
        )
        .expect("later epoch cohort")
        .into_runtime();
    let next_activation = program_local_activation(&mut lifecycle, 952, 20);
    let next = installation
        .establish(
            &mut next_runtime,
            &lifecycle,
            &next_activation,
            program_local_subject(&root, &next_activation, 1052, Some(3)),
        )
        .expect("later epoch establishes a fresh root");
    assert_ne!(first_lineage, next.lineage());
    assert_eq!(next.lineage().occurrence().lifecycle_epoch(), 20);
    installation
        .retire_established(next, &mut lifecycle)
        .expect("later epoch root retirement");
}

#[test]
fn aggregate_retirement_completes_the_epoch_membership_and_rejoins_installed_backing() {
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
        784,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 884, 10, "TestRoot::entry");
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
    let activation = program_local_activation(&mut lifecycle, 984, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1084, 0x4040, 0x100),
        )
        .expect("exact interval subject establishes its root");
    let aggregate = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&established])
        .expect("the live group reconstructs its aggregate capacity");

    // The receiver partition is carved out of provider-issued installed
    // image backing; the residuals stay outside the member's authority.
    let partition = installed_backing_extent(720, 0x4000, 0x200, 30)
        .partition_owned(0x40, 0x100)
        .expect("exact receiver partition");
    let (before, backing, after) = partition.into_parts();
    let before = before.expect("lower installed residual");
    let after = after.expect("upper installed residual");

    let mut registry = ProgramLocalExtentRegistry::new();
    let [extent] = registry
        .materialize_aggregate(
            &installation,
            &lifecycle,
            &aggregate,
            vec![(established, backing)],
        )
        .expect("reconstructed capacity discharges over the exact partition")
        .try_into()
        .expect("one minted program-local Extent");
    assert_eq!(registry.held_accounts(), 1);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));

    // Aggregate completion is the discharge's counterpart: the complete live
    // membership retires in one transaction inside the cohort's epoch, and
    // each member's installed backing partition returns for rejoin.
    let [retired]: [_; 1] = registry
        .retire_aggregate(&mut installation, &mut lifecycle, &aggregate, vec![extent])
        .expect("the complete live membership completes in its epoch")
        .try_into()
        .expect("one retired member");
    assert_eq!(registry.held_accounts(), 0);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
    assert_eq!(
        retired.occurrence().epoch_lease().normalized_identity(),
        884
    );

    let tail = retired
        .into_backing()
        .merge(after)
        .expect("receiver partition rejoins its upper residual");
    let restored = before
        .merge(tail)
        .expect("installed image restores around the discharged range");
    assert!(restored.is_lineage_root());
    assert_eq!(restored.base(), 0x4000);
    assert_eq!(restored.length(), 0x200);
    assert!(
        restored
            .provider_issuance()
            .is_some_and(|issuance| issuance == extent_provider_issuance(720))
    );

    let receipt = activation.leave_receipt(true);
    activation
        .leave(&mut lifecycle, receipt)
        .expect("the establishing activation completes its scope");
}

#[test]
fn aggregate_retirement_is_epoch_bound_and_rejects_stale_aggregates() {
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
        785,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 885, 10, "TestRoot::entry");
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
    let activation = program_local_activation(&mut lifecycle, 985, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1085, 0x4000, 0x100),
        )
        .expect("exact interval subject establishes its root");
    let aggregate = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&established])
        .expect("the live group reconstructs its aggregate capacity");
    let mut registry = ProgramLocalExtentRegistry::new();
    let extent = registry
        .materialize(
            established,
            installed_backing_extent(722, 0x4000, 0x100, 30),
        )
        .expect("established interval materializes over its installed backing");

    // The epoch rolls: aggregate completion replays each member's live epoch
    // lease, so a closed cohort cannot complete through the aggregate route.
    publish_program_local_era(
        &mut lifecycle,
        11,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
        121,
        true,
    );
    let rejected = registry
        .retire_aggregate(&mut installation, &mut lifecycle, &aggregate, vec![extent])
        .expect_err("a closed epoch's membership cannot reconstruct its live group");
    assert!(rejected.diagnostic().0.contains("live lease"));
    let (uncommitted, retired_prefix) = (*rejected).into_parts();
    assert!(retired_prefix.is_empty());
    let [extent]: [Extent; 1] = uncommitted
        .try_into()
        .expect("rejection returns the uncommitted member");
    assert_eq!(registry.held_accounts(), 1);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));

    // The single-account route still completes: the stale-era lease releases
    // without replaying the aggregate's live-epoch requirement.
    registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect("single retirement releases the stale-era lease");
    assert_eq!(registry.held_accounts(), 0);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));

    // A stale aggregate cannot complete the next epoch's live membership.
    let next_lease = program_local_epoch_lease(&mut lifecycle, 886, 11, "TestRoot::entry");
    let mut next_runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding.identity(),
                &root,
                next_lease,
            )],
        )
        .expect("next epoch cohort")
        .into_runtime();
    let next_activation = program_local_activation(&mut lifecycle, 986, 11);
    let next_established = installation
        .establish(
            &mut next_runtime,
            &lifecycle,
            &next_activation,
            program_local_extent_subject(&root, &next_activation, 1086, 0x4000, 0x100),
        )
        .expect("next epoch establishes a fresh root");
    let next_aggregate = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&next_established])
        .expect("the next epoch's group reconstructs");
    let next_extent = registry
        .materialize(
            next_established,
            installed_backing_extent(723, 0x4000, 0x100, 30),
        )
        .expect("next epoch materializes over installed backing");

    let rejected = registry
        .retire_aggregate(
            &mut installation,
            &mut lifecycle,
            &aggregate,
            vec![next_extent],
        )
        .expect_err("a stale aggregate cannot discharge the new epoch's membership");
    assert!(rejected.diagnostic().0.contains("stale or substituted"));
    let (uncommitted, _) = (*rejected).into_parts();
    let [next_extent]: [Extent; 1] = uncommitted
        .try_into()
        .expect("rejection returns the uncommitted member");

    let [retired]: [_; 1] = registry
        .retire_aggregate(
            &mut installation,
            &mut lifecycle,
            &next_aggregate,
            vec![next_extent],
        )
        .expect("the next epoch's membership completes in its epoch")
        .try_into()
        .expect("one retired member");
    assert_eq!(
        retired.occurrence().epoch_lease().normalized_identity(),
        886
    );
    assert_eq!(lifecycle.program_local_root_authority_holds(11), Some(0));
    assert_eq!(registry.held_accounts(), 0);
}

#[test]
fn aggregate_retirement_rejects_cross_group_blocked_and_foreign_members() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_two_schema_extent_module();
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
        .expect("two verified installed Extent prebindings")
        .try_into()
        .expect("two producer schemas");
    let mut lifecycle = program_local_lifecycle(
        786,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let first_lease = program_local_epoch_lease(&mut lifecycle, 886, 10, "TestRoot::entry");
    let second_lease = program_local_epoch_lease(&mut lifecycle, 887, 10, "TestRoot::entry");
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
        .expect("exact two-schema Extent epoch cohort")
        .into_runtime();
    let activation = program_local_activation(&mut lifecycle, 986, 10);
    let first = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject_at(&root, &activation, 1086, 0, 0, 0x4040, 0x100),
        )
        .expect("first schema establishes on its exact parameter subject");
    let second = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject_at(&root, &activation, 1087, 1, 1, 0x5040, 0x100),
        )
        .expect("second schema establishes on its exact parameter subject");
    let first_aggregate = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&first])
        .expect("the first group reconstructs its aggregate capacity");
    let second_aggregate = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&second])
        .expect("the second group reconstructs its aggregate capacity");

    let mut registry = ProgramLocalExtentRegistry::new();
    let mut extents = registry
        .materialize_batch(vec![
            (first, installed_backing_extent(730, 0x4040, 0x100, 30)),
            (second, installed_backing_extent(731, 0x5040, 0x100, 30)),
        ])
        .expect("both members materialize over installed backing");
    let extent_b = extents.pop().expect("second member extent");
    let [extent_a]: [_; 1] = extents.try_into().expect("first member extent");
    assert_eq!(registry.held_accounts(), 2);

    // Members of two distinct aggregate groups cannot complete in one
    // transaction: reconstruction refuses the mixed roster and both Extents
    // return uncommitted.
    let rejected = registry
        .retire_aggregate(
            &mut installation,
            &mut lifecycle,
            &first_aggregate,
            vec![extent_a, extent_b],
        )
        .expect_err("a mixed membership cannot reconstruct one aggregate group");
    assert!(
        rejected
            .diagnostic()
            .0
            .contains("distinct aggregate schemas")
    );
    let (uncommitted, retired_prefix) = (*rejected).into_parts();
    assert!(retired_prefix.is_empty());
    let [extent_a, extent_b]: [Extent; 2] = uncommitted
        .try_into()
        .expect("rejection returns both members");
    assert_eq!(registry.held_accounts(), 2);

    // A live retained foreign argument pins its account out of completion.
    let retained = registry
        .retain_foreign_argument_borrowed(
            &extent_a,
            RetainedForeignArgumentRequest::new(
                0x10,
                0x20,
                RetainedForeignAccess::Shared,
                extent_a.rights().clone(),
            )
            .expect("retained foreign argument request"),
        )
        .expect("borrowed retention");
    let rejected = registry
        .retire_aggregate(
            &mut installation,
            &mut lifecycle,
            &first_aggregate,
            vec![extent_a],
        )
        .expect_err("a retained foreign argument blocks aggregate completion");
    assert!(
        rejected
            .diagnostic()
            .0
            .contains("live retained foreign argument")
    );
    let (uncommitted, _) = (*rejected).into_parts();
    let [extent_a]: [Extent; 1] = uncommitted
        .try_into()
        .expect("rejection returns the member");
    registry
        .release_retained_foreign_argument(retained)
        .expect("release the pinned foreign argument");

    // Only the exact recombined lineage root completes the account.
    let (lower, upper) = extent_a.split_at(0x40).expect("split program-local Extent");
    let rejected = registry
        .retire_aggregate(
            &mut installation,
            &mut lifecycle,
            &first_aggregate,
            vec![lower],
        )
        .expect_err("a split descendant cannot complete the aggregate");
    assert!(rejected.diagnostic().0.contains("recombined root"));
    let (uncommitted, _) = (*rejected).into_parts();
    let [lower]: [Extent; 1] = uncommitted
        .try_into()
        .expect("descendant returns uncommitted");
    let extent_a = lower.merge(upper).expect("recombine exact root Extent");

    // Ambient provider-issued backing is not a held member, and an account
    // held by another registry cannot complete through this one.
    let rejected = registry
        .retire_aggregate(
            &mut installation,
            &mut lifecycle,
            &first_aggregate,
            vec![installed_backing_extent(732, 0x4040, 0x100, 30)],
        )
        .expect_err("provider-issued backing is not a held account");
    assert!(rejected.diagnostic().0.contains("provider-issued root"));
    let mut empty_registry = ProgramLocalExtentRegistry::new();
    let rejected = empty_registry
        .retire_aggregate(
            &mut installation,
            &mut lifecycle,
            &first_aggregate,
            vec![extent_a],
        )
        .expect_err("another registry holds no account for the member extent");
    assert!(rejected.diagnostic().0.contains("no held exact occurrence"));
    let (uncommitted, _) = (*rejected).into_parts();
    let [extent_a]: [Extent; 1] = uncommitted.try_into().expect("member returns uncommitted");

    // An empty roster cannot complete an aggregate.
    let rejected = registry
        .retire_aggregate(
            &mut installation,
            &mut lifecycle,
            &first_aggregate,
            Vec::new(),
        )
        .expect_err("aggregate completion requires at least one member");
    assert!(rejected.diagnostic().0.contains("at least one"));

    // Each group then completes in its own transaction within the epoch.
    let [retired_a]: [_; 1] = registry
        .retire_aggregate(
            &mut installation,
            &mut lifecycle,
            &first_aggregate,
            vec![extent_a],
        )
        .expect("the first group completes")
        .try_into()
        .expect("one retired member");
    let [retired_b]: [_; 1] = registry
        .retire_aggregate(
            &mut installation,
            &mut lifecycle,
            &second_aggregate,
            vec![extent_b],
        )
        .expect("the second group completes")
        .try_into()
        .expect("one retired member");
    assert_eq!(retired_a.backing().base(), 0x4040);
    assert_eq!(retired_b.backing().base(), 0x5040);
    assert_eq!(registry.held_accounts(), 0);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

#[test]
fn receiver_partition_materialization_holds_residuals_until_completion_restores_the_receiver() {
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
        787,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 887, 10, "TestRoot::entry");
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
    let activation = program_local_activation(&mut lifecycle, 987, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1087, 0x4040, 0x100),
        )
        .expect("exact interval subject establishes its root");

    // The registry consumes the whole receiver partition: the selected
    // member backs the account and the conserved residuals stay held inside
    // it for the account's epoch rather than ambient outside the ledger.
    let partition = installed_backing_extent(733, 0x4000, 0x200, 30)
        .partition_owned(0x40, 0x100)
        .expect("exact receiver partition");

    let mut registry = ProgramLocalExtentRegistry::new();
    let mut extent = registry
        .materialize_over_receiver(established, partition)
        .expect("receiver partition materializes the exact occurrence");
    assert_eq!(extent.base(), 0x4040);
    assert_eq!(extent.length(), 0x100);
    assert_eq!(registry.held_accounts(), 1);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));

    // The establishing activation still borrows a subrange of the partition.
    {
        let loan = registry
            .loan_mut_under_activation(&activation, &mut extent, 0x10, 0x20)
            .expect("exclusive activation loan over the receiver partition");
        assert_eq!(loan.base(), 0x4050);
        assert_eq!(loan.length(), 0x20);
    }

    // Completion rejoins the retained residuals: the returned backing is the
    // restored receiver extent, not a detached partition range.
    let retired = registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect("recombined root completes the receiver account");
    let restored = retired.into_backing();
    assert!(restored.is_lineage_root());
    assert_eq!(restored.base(), 0x4000);
    assert_eq!(restored.length(), 0x200);
    assert_eq!(
        restored.era(),
        extent_id(30, MappingEraId::from_normalized_identity)
    );
    assert!(
        restored
            .provider_issuance()
            .is_some_and(|issuance| issuance == extent_provider_issuance(733))
    );
}

#[test]
fn receiver_partition_materialization_rejects_and_restores_the_receiver() {
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
        788,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 888, 10, "TestRoot::entry");
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
    let activation = program_local_activation(&mut lifecycle, 988, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1088, 0x4040, 0x100),
        )
        .expect("exact interval subject establishes its root");

    // A partition whose selected member does not equal the evaluated
    // interval capacity rejects, and the returned input is the receiver
    // extent restored whole — no residual is left carved outside the ledger.
    let mismatched = installed_backing_extent(734, 0x4000, 0x200, 30)
        .partition_owned(0x60, 0x100)
        .expect("mismatched receiver partition");
    let mut registry = ProgramLocalExtentRegistry::new();
    let rejected = registry
        .materialize_over_receiver(established, mismatched)
        .expect_err("a mismatched selected partition cannot back the account");
    assert!(
        rejected
            .diagnostic()
            .0
            .contains("does not equal its established interval capacity")
    );
    let [(established, receiver)] = (*rejected)
        .into_inputs()
        .try_into()
        .expect("rejection returns the member input");
    assert!(receiver.is_lineage_root());
    assert_eq!(receiver.base(), 0x4000);
    assert_eq!(receiver.length(), 0x200);
    assert_eq!(registry.held_accounts(), 0);

    // The restored receiver partitions again and discharges the same member.
    let partition = receiver
        .partition_owned(0x40, 0x100)
        .expect("exact receiver partition");
    let extent = registry
        .materialize_over_receiver(established, partition)
        .expect("the restored receiver discharges the same member");
    let retired = registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect("recombined root completes the receiver account");
    let restored = retired.into_backing();
    assert!(restored.is_lineage_root());
    assert_eq!(restored.base(), 0x4000);
    assert_eq!(restored.length(), 0x200);
}

#[test]
fn receiver_partition_materialization_cannot_reticket_held_program_local_authority() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_two_schema_extent_module();
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
        .expect("two verified installed Extent prebindings")
        .try_into()
        .expect("two producer schemas");
    let mut lifecycle = program_local_lifecycle(
        789,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let first_lease = program_local_epoch_lease(&mut lifecycle, 889, 10, "TestRoot::entry");
    let second_lease = program_local_epoch_lease(&mut lifecycle, 890, 10, "TestRoot::entry");
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
        .expect("exact two-schema Extent epoch cohort")
        .into_runtime();
    let activation = program_local_activation(&mut lifecycle, 989, 10);
    let first = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject_at(&root, &activation, 1089, 0, 0, 0x4040, 0x100),
        )
        .expect("first schema establishes on its exact parameter subject");
    let second = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject_at(&root, &activation, 1090, 1, 1, 0x5040, 0x100),
        )
        .expect("second schema establishes on its exact parameter subject");

    let mut registry = ProgramLocalExtentRegistry::new();
    let extent_a = registry
        .materialize_over_receiver(
            first,
            installed_backing_extent(735, 0x4000, 0x200, 30)
                .partition_owned(0x40, 0x100)
                .expect("first receiver partition"),
        )
        .expect("first receiver partition materializes");

    // A partition carved out of a held program-local Extent is not installed
    // backing: it cannot reticket the same range under a second occurrence,
    // and the rejection rejoins it into the exact minted Extent.
    let reticket = extent_a
        .partition_owned(0x10, 0x40)
        .expect("held account partition");
    let rejected = registry
        .materialize_over_receiver(second, reticket)
        .expect_err("a held program-local Extent is not installed backing");
    assert!(
        rejected
            .diagnostic()
            .0
            .contains("requires actual installed backing")
    );
    let [(second, extent_a)] = (*rejected)
        .into_inputs()
        .try_into()
        .expect("rejection returns the member input");
    assert!(extent_a.is_lineage_root());
    assert_eq!(registry.held_accounts(), 1);

    // The returned minted Extent still completes its own account, restoring
    // the first receiver, and the second member materializes over its own
    // receiver partition.
    let retired = registry
        .retire(extent_a, &mut installation, &mut lifecycle)
        .expect("first receiver account completes");
    assert_eq!(retired.backing().base(), 0x4000);
    assert_eq!(retired.backing().length(), 0x200);
    let extent_b = registry
        .materialize_over_receiver(
            second,
            installed_backing_extent(736, 0x5000, 0x200, 30)
                .partition_owned(0x40, 0x100)
                .expect("second receiver partition"),
        )
        .expect("second receiver partition materializes");
    let retired = registry
        .retire(extent_b, &mut installation, &mut lifecycle)
        .expect("second receiver account completes");
    assert!(retired.backing().is_lineage_root());
    assert_eq!(retired.backing().base(), 0x5000);
    assert_eq!(registry.held_accounts(), 0);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

#[test]
fn aggregate_over_receiver_partitions_complete_with_restored_receivers() {
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
    let lease = program_local_epoch_lease(&mut lifecycle, 891, 10, "TestRoot::entry");
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
    let activation = program_local_activation(&mut lifecycle, 991, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1091, 0x4040, 0x100),
        )
        .expect("exact interval subject establishes its root");
    let aggregate = installation
        .reconstruct_aggregate_capacity(&lifecycle, [&established])
        .expect("the live group reconstructs its aggregate capacity");

    let mut registry = ProgramLocalExtentRegistry::new();
    let [extent] = registry
        .materialize_aggregate_over_receiver(
            &installation,
            &lifecycle,
            &aggregate,
            vec![(
                established,
                installed_backing_extent(737, 0x4000, 0x200, 30)
                    .partition_owned(0x40, 0x100)
                    .expect("exact receiver partition"),
            )],
        )
        .expect("the aggregate discharges over the receiver partition")
        .try_into()
        .expect("one minted program-local Extent");
    assert_eq!(extent.base(), 0x4040);
    assert_eq!(registry.held_accounts(), 1);

    let [retired]: [_; 1] = registry
        .retire_aggregate(&mut installation, &mut lifecycle, &aggregate, vec![extent])
        .expect("the complete live membership completes in its epoch")
        .try_into()
        .expect("one retired member");
    assert_eq!(registry.held_accounts(), 0);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));

    // Aggregate completion rejoined the retained residuals: the member's
    // returned backing is the receiver extent restored whole.
    let restored = retired.into_backing();
    assert!(restored.is_lineage_root());
    assert_eq!(restored.base(), 0x4000);
    assert_eq!(restored.length(), 0x200);
    assert!(
        restored
            .provider_issuance()
            .is_some_and(|issuance| issuance == extent_provider_issuance(737))
    );
}

#[test]
fn batch_over_receiver_partitions_retains_each_member_residuals() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_two_schema_extent_module();
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
        .expect("two verified installed Extent prebindings")
        .try_into()
        .expect("two producer schemas");
    let mut lifecycle = program_local_lifecycle(
        791,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let first_lease = program_local_epoch_lease(&mut lifecycle, 892, 10, "TestRoot::entry");
    let second_lease = program_local_epoch_lease(&mut lifecycle, 893, 10, "TestRoot::entry");
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
        .expect("exact two-schema Extent epoch cohort")
        .into_runtime();
    let activation = program_local_activation(&mut lifecycle, 992, 10);
    let first = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject_at(&root, &activation, 1092, 0, 0, 0x4040, 0x100),
        )
        .expect("first schema establishes on its exact parameter subject");
    let second = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject_at(&root, &activation, 1093, 1, 1, 0x5040, 0x100),
        )
        .expect("second schema establishes on its exact parameter subject");

    // Each member's receiver partition retains its own residuals under the
    // account; a member whose partition does not match its interval rejects
    // the whole batch and restores every receiver.
    let mut registry = ProgramLocalExtentRegistry::new();
    let rejected = registry
        .materialize_batch_over_receiver(vec![
            (
                first,
                installed_backing_extent(738, 0x4000, 0x200, 30)
                    .partition_owned(0x40, 0x100)
                    .expect("first receiver partition"),
            ),
            (
                second,
                installed_backing_extent(739, 0x5000, 0x200, 30)
                    .partition_owned(0x80, 0x100)
                    .expect("mismatched second receiver partition"),
            ),
        ])
        .expect_err("a mismatched member partition rejects the whole batch");
    assert!(
        rejected
            .diagnostic()
            .0
            .contains("does not equal its established interval capacity")
    );
    let receivers = (*rejected).into_inputs();
    assert_eq!(receivers.len(), 2);
    for (_, receiver) in &receivers {
        assert!(receiver.is_lineage_root());
        assert_eq!(receiver.length(), 0x200);
    }
    assert_eq!(registry.held_accounts(), 0);

    // The restored receivers discharge the batch; each account's completion
    // rejoins its residuals into the exact receiver extent.
    let mut members = receivers.into_iter();
    let (first, first_receiver) = members.next().expect("first member input");
    let (second, second_receiver) = members.next().expect("second member input");
    let mut extents = registry
        .materialize_batch_over_receiver(vec![
            (
                first,
                first_receiver
                    .partition_owned(0x40, 0x100)
                    .expect("first receiver partition"),
            ),
            (
                second,
                second_receiver
                    .partition_owned(0x40, 0x100)
                    .expect("second receiver partition"),
            ),
        ])
        .expect("both members materialize over their receiver partitions");
    let extent_b = extents.pop().expect("second member extent");
    let [extent_a]: [_; 1] = extents.try_into().expect("first member extent");
    assert_eq!(registry.held_accounts(), 2);

    let retired_a = registry
        .retire(extent_a, &mut installation, &mut lifecycle)
        .expect("first receiver account completes");
    let retired_b = registry
        .retire(extent_b, &mut installation, &mut lifecycle)
        .expect("second receiver account completes");
    for (retired, base) in [(retired_a, 0x4000), (retired_b, 0x5000)] {
        let restored = retired.into_backing();
        assert!(restored.is_lineage_root());
        assert_eq!(restored.base(), base);
        assert_eq!(restored.length(), 0x200);
    }
    assert_eq!(registry.held_accounts(), 0);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

#[test]
fn program_local_extent_registry_remap_reseats_backing_under_new_era() {
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
        985,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 1085, 10, "TestRoot::entry");
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
    let activation = program_local_activation(&mut lifecycle, 1185, 10);
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            &activation,
            program_local_extent_subject(&root, &activation, 1285, 0x4000, 0x100),
        )
        .expect("exact interval subject establishes its root");
    let mut registry = ProgramLocalExtentRegistry::new();
    let extent = registry
        .materialize(
            established,
            installed_backing_extent(700, 0x4000, 0x100, 30),
        )
        .expect("established interval materializes over its installed backing");

    // Only the exact recombined root can drive a remap: a split descendant
    // names partial authority and rejects, returning both Extents.
    let (lower, upper) = extent.split_at(0x40).expect("split program-local Extent");
    let rejected = registry
        .remap(lower, installed_backing_extent(701, 0x4000, 0x100, 31))
        .expect_err("a split descendant cannot remap the account");
    assert!(rejected.diagnostic().0.contains("recombined root"));
    let (lower, _backing) = rejected.into_parts();
    let extent = lower.merge(upper).expect("recombine exact root Extent");

    // A same-era replacement is indistinguishable from ambient substitution:
    // a remap must advance the mapping era.
    let rejected = registry
        .remap(extent, installed_backing_extent(701, 0x4000, 0x100, 30))
        .expect_err("a same-era replacement is not a remap");
    assert!(rejected.diagnostic().0.contains("new mapping era"));
    let (extent, _backing) = rejected.into_parts();

    // The replacement covers the same geometry in the same address space;
    // relocation is retire+materialize, not a remap.
    let rejected = registry
        .remap(extent, installed_backing_extent(701, 0x4000, 0x80, 31))
        .expect_err("a geometry change cannot ride the remap route");
    assert!(rejected.diagnostic().0.contains("same address space"));
    let (extent, _backing) = rejected.into_parts();

    // A foreign side still retaining the old range pins the stale revision:
    // the remap is refused until the peer releases its retention.
    let request = RetainedForeignArgumentRequest::new(
        0x10,
        0x20,
        RetainedForeignAccess::Shared,
        ExtentRights::from_normalized_identities([extent_id(
            100,
            ExtentRightId::from_normalized_identity,
        )]),
    )
    .expect("nonempty shared request");
    let retained = registry
        .retain_foreign_argument_borrowed(&extent, request)
        .expect("shared borrowed retention");
    let rejected = registry
        .remap(extent, installed_backing_extent(701, 0x4000, 0x100, 31))
        .expect_err("a live retained foreign argument blocks the remap");
    assert!(
        rejected
            .diagnostic()
            .0
            .contains("retained foreign argument")
    );
    let (extent, _backing) = rejected.into_parts();
    registry
        .release_retained_foreign_argument(retained)
        .expect("peer retention releases");

    // The checked remap consumes the stale root and mints a fresh
    // program-local Extent over the re-seated backing's own runtime facts —
    // same origin and lineage, new provenance and era.
    let stale_era = extent.era();
    let remapped = registry
        .remap(extent, installed_backing_extent(701, 0x4000, 0x100, 31))
        .expect("the recombined root re-seats the account's backing");
    assert_eq!(remapped.base(), 0x4000);
    assert_eq!(remapped.length(), 0x100);
    assert_eq!(remapped.era().normalized_identity(), 31);
    assert_ne!(remapped.era(), stale_era);
    assert_eq!(registry.held_accounts(), 1);

    // The account completes under the new revision: retirement returns the
    // remapped backing, not the stale one.
    let retired = registry
        .retire(remapped, &mut installation, &mut lifecycle)
        .expect("the remapped account completes under its new revision");
    let backing = retired.into_backing();
    assert_eq!(backing.base(), 0x4000);
    assert_eq!(backing.era().normalized_identity(), 31);
    assert!(
        backing
            .provider_issuance()
            .is_some_and(|issuance| issuance == extent_provider_issuance(701))
    );
    assert_eq!(registry.held_accounts(), 0);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

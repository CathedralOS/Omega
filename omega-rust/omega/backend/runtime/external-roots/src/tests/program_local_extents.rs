use super::{
    entry_id, extent_id, extent_provider_issuance, install_program_local_required_root,
    installed_backing_extent, installed_code, installed_code_with_fill_and_installation_identity,
    program_local_activation, program_local_claim, program_local_epoch_lease,
    program_local_extent_module, program_local_extent_subject, program_local_lifecycle,
    program_local_root_catalog, program_local_root_module, program_local_subject,
    program_local_terminal_object, publish_program_local_era,
};
use crate::{
    EstablishedProgramLocalRoot, ProgramLocalExtentRegistry, ProgramLocalRootCohortMember,
    RetainedForeignAccess, RetainedForeignArgumentDisposition, RetainedForeignArgumentRequest,
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

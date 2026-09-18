//! Machine contract plan tests.

use super::{
    BlockingInterface, CheckedCrashCallSite, CheckedCrashSite, CheckedEntryResourceEnvelope,
    CheckedMachineResourceEnvelopes, CheckedResourceDerivationObligation,
    ClosedScalarValueContractPlan, CrashCallSiteLocation, CrashCause, CrashContractCapsule,
    CrashInterface, CrashPlan, CrashPredicateIdentity, CrashRouteBucket, CrashRouteGuard,
    CrashSiteLocation, MachineContractCommitment, MachineContractPlan, MachineContractPlans,
    MachineSupplyMode, RealizedMachineContractEnvelope, SuspensionInterface, SymbolHandle,
    SynchronousInvocationInterface, TerminationGuarantee, TerminationInterface,
    contract_report_fingerprint,
};

#[test]
fn compact_equal_machine_contract_substitution_rejects() {
    let machine = SymbolHandle::from_arena_index(17);
    let entry = SymbolHandle::from_arena_index(18);
    let compact = 0xfeed;
    let expected = MachineContractCommitment::from_digest([0x11; 32]);
    let substituted = MachineContractCommitment::from_digest([0x22; 32]);
    let plans = MachineContractPlans {
        machines: vec![MachineContractPlan {
            machine,
            closed_scalar_values: ClosedScalarValueContractPlan::default(),
            crash: CrashPlan::default(),
            report_fingerprint: compact,
            commitment: expected,
        }],
        crash_capsules: Vec::new(),
        realized_envelopes: vec![RealizedMachineContractEnvelope {
            machine,
            contract_report_fingerprint: compact,
            contract_commitment: substituted,
            effective_service_reach: Vec::new(),
            concrete_service_reach: Vec::new(),
            unresolved_installation_reaches: Vec::new(),
            effective_synchronous_invocations: Vec::new(),
            checked_may_suspend: false,
            checked_may_block: false,
            checked_termination: TerminationGuarantee::NoGuarantee,
            checked_crash: CrashPlan::default(),
            mutation: Vec::new(),
            capabilities: Vec::new(),
            resources: CheckedMachineResourceEnvelopes::from_checked_contract_entries(
                machine,
                compact,
                substituted,
                [entry],
            ),
        }],
    };

    assert!(
        plans
            .validate_resource_envelopes()
            .expect_err("compact-equal structural contract substitution must reject")
            .contains("contract identity disagrees")
    );
}

#[test]
fn zero_machine_contract_commitment_rejects_even_with_nonzero_report_coordinate() {
    let machine = SymbolHandle::from_arena_index(17);
    let entry = SymbolHandle::from_arena_index(18);
    let compact = 0xfeed;
    let plans = MachineContractPlans {
        machines: vec![MachineContractPlan {
            machine,
            closed_scalar_values: ClosedScalarValueContractPlan::default(),
            crash: CrashPlan::default(),
            report_fingerprint: compact,
            commitment: MachineContractCommitment::from_digest([0; 32]),
        }],
        crash_capsules: Vec::new(),
        realized_envelopes: vec![RealizedMachineContractEnvelope {
            machine,
            contract_report_fingerprint: compact,
            contract_commitment: MachineContractCommitment::from_digest([0; 32]),
            effective_service_reach: Vec::new(),
            concrete_service_reach: Vec::new(),
            unresolved_installation_reaches: Vec::new(),
            effective_synchronous_invocations: Vec::new(),
            checked_may_suspend: false,
            checked_may_block: false,
            checked_termination: TerminationGuarantee::NoGuarantee,
            checked_crash: CrashPlan::default(),
            mutation: Vec::new(),
            capabilities: Vec::new(),
            resources: CheckedMachineResourceEnvelopes::from_checked_contract_entries(
                machine,
                compact,
                MachineContractCommitment::from_digest([0; 32]),
                [entry],
            ),
        }],
    };

    assert_eq!(
        plans
            .validate_resource_envelopes()
            .expect_err("zero strong authority must reject before compact replay"),
        "checked resource envelope requires a nonzero machine contract commitment",
    );
}

#[test]
fn machine_contract_fnv_is_report_only_and_strong_identity_is_domain_separated() {
    let source = [
        include_str!("../contract_plans.rs"),
        include_str!("crash_plans.rs"),
        include_str!("crash_predicates.rs"),
        include_str!("crash_sites.rs"),
        include_str!("resource_envelopes.rs"),
        include_str!("scalar_contracts.rs"),
    ]
    .concat();
    let compact_marker = ["const OFFSET: u64 = 0x", "cbf29ce484222325"].concat();
    assert_eq!(
        source.matches(compact_marker.as_str()).count(),
        2,
        "the reviewed checked-contract/resource compact inventory changed"
    );
    assert!(
        source.contains("omega.checked-machine-public-contract.v1\\0")
            && source.contains("pub fn contract_identity(")
            && source.contains("pub commitment: MachineContractCommitment"),
        "public machine contracts must retain a domain-separated strong commitment"
    );
}

#[test]
fn checked_resource_envelope_replays_three_distinct_derivation_obligations() {
    let machine = SymbolHandle::from_arena_index(21);
    let entry = SymbolHandle::from_arena_index(22);
    let commitment = MachineContractCommitment::from_digest([0x11; 32]);
    let envelope =
        CheckedEntryResourceEnvelope::from_checked_contract(machine, entry, 0xfeed, commitment);

    envelope.validate().expect("canonical resource anchor");
    assert_eq!(envelope.machine(), machine);
    assert_eq!(envelope.entry(), entry);
    assert_eq!(envelope.contract_report_fingerprint(), 0xfeed);
    assert_eq!(
        envelope.stack().derivation_obligation(),
        CheckedResourceDerivationObligation::TerminalAndTargetStackClosure
    );
    assert_eq!(
        envelope.logical_structural_work().derivation_obligation(),
        CheckedResourceDerivationObligation::TerminalControlAndFuelSchedule
    );
    assert_eq!(
        envelope.machine_state().derivation_obligation(),
        CheckedResourceDerivationObligation::SelectedInstructionMachineStateFootprint
    );
    assert_ne!(
        envelope.stack().report_fingerprint(),
        envelope.logical_structural_work().report_fingerprint()
    );
    assert_ne!(
        envelope.logical_structural_work().report_fingerprint(),
        envelope.machine_state().report_fingerprint()
    );

    let mut fused = envelope.clone();
    fused.stack = fused.logical_structural_work.clone();
    assert!(
        fused
            .validate()
            .expect_err("one resource axis cannot substitute for another")
            .contains("substituted or fused")
    );

    let mut tampered = envelope;
    tampered.machine_state.report_fingerprint ^= 1;
    assert!(
        tampered
            .validate()
            .expect_err("axis identity must independently replay")
            .contains("axis report fingerprint")
    );

    let other = CheckedEntryResourceEnvelope::from_checked_contract(
        machine,
        SymbolHandle::from_arena_index(23),
        0xfeed,
        commitment,
    );
    tampered = other.clone();
    tampered.stack =
        CheckedEntryResourceEnvelope::from_checked_contract(machine, entry, 0xfeed, commitment)
            .stack;
    assert!(
        tampered
            .validate()
            .expect_err("an axis cannot move between entries")
            .contains("exact machine entry contract")
    );
}

#[test]
fn checked_resource_envelope_rejects_compact_equal_contract_commitment_substitution() {
    let machine = SymbolHandle::from_arena_index(24);
    let entry = SymbolHandle::from_arena_index(25);
    let expected = MachineContractCommitment::from_digest([0x31; 32]);
    let substituted = MachineContractCommitment::from_digest([0x32; 32]);
    let canonical =
        CheckedEntryResourceEnvelope::from_checked_contract(machine, entry, 0xfeed, expected);
    let foreign =
        CheckedEntryResourceEnvelope::from_checked_contract(machine, entry, 0xfeed, substituted);

    canonical.validate().expect("canonical resource envelope");
    foreign
        .validate()
        .expect("independently canonical foreign envelope");
    assert_ne!(canonical.report_fingerprint(), foreign.report_fingerprint());

    let mut mixed = canonical;
    mixed.stack = foreign.stack;
    assert!(
        mixed
            .validate()
            .expect_err("compact-equal contract substitution must reject")
            .contains("exact machine entry contract")
    );
}

#[test]
fn checked_resource_roster_rejects_entry_deletion_and_reordering() {
    let machine = SymbolHandle::from_arena_index(31);
    let entries = [
        SymbolHandle::from_arena_index(32),
        SymbolHandle::from_arena_index(33),
        SymbolHandle::from_arena_index(34),
    ];
    let commitment = MachineContractCommitment::from_digest([0x22; 32]);
    let roster = CheckedMachineResourceEnvelopes::from_checked_contract_entries(
        machine, 0xbeef, commitment, entries,
    );
    roster.validate().expect("canonical resource roster");
    assert_eq!(roster.len(), 3);
    assert_eq!(
        roster
            .iter()
            .map(CheckedEntryResourceEnvelope::entry)
            .collect::<Vec<_>>(),
        entries
    );

    let mut deleted = roster.clone();
    deleted.entries.remove(1);
    assert!(
        deleted
            .validate()
            .expect_err("entry deletion must invalidate the sealed roster")
            .contains("roster report fingerprint")
    );

    let mut reordered = roster;
    reordered.entries.swap(0, 2);
    assert!(
        reordered
            .validate()
            .expect_err("entry reordering must invalidate the sealed roster")
            .contains("roster report fingerprint")
    );
}

#[test]
fn crash_route_carriers_enforce_canonical_nonempty_sets() {
    assert!(CrashRouteBucket::new(CrashCause::Trap, Vec::new()).is_none());

    let predicate = CrashPredicateIdentity::from_canonical_bytes(vec![1, 2, 3]);
    let guarded = CrashRouteBucket::new(
        CrashCause::Trap,
        vec![
            CrashRouteGuard::Predicate(predicate.clone()),
            CrashRouteGuard::Predicate(predicate),
        ],
    )
    .expect("a guarded bucket is nonempty");
    assert_eq!(guarded.alternative_guards().len(), 1);

    let unconditional = CrashRouteBucket::new(
        CrashCause::Trap,
        vec![
            CrashRouteGuard::Predicate(CrashPredicateIdentity::from_canonical_bytes(vec![4])),
            CrashRouteGuard::Truth,
        ],
    )
    .expect("truth contributes a route");
    assert!(unconditional.is_unconditional());

    let plan = CrashPlan::published_ceiling(vec![unconditional.clone(), unconditional]);
    assert_eq!(plan.published().len(), 1);
}

#[test]
fn crash_sites_are_canonical_implementation_evidence() {
    let first_state = SymbolHandle::from_arena_index(4);
    let second_state = SymbolHandle::from_arena_index(9);
    let first_claim = language_semantics::PermissionClaimIdentity::Established {
        machine_symbol: SymbolHandle::from_arena_index(2),
        state_symbol: first_state,
        source: language_semantics::PermissionEventSource::StateEntry,
        ordinal: 0,
    };
    let second_claim = language_semantics::PermissionClaimIdentity::Established {
        machine_symbol: SymbolHandle::from_arena_index(2),
        state_symbol: first_state,
        source: language_semantics::PermissionEventSource::Statement { statement_index: 1 },
        ordinal: 1,
    };
    let path_guard = CrashPredicateIdentity::from_canonical_bytes(vec![1, 9, 0, 0, 0, 0]);
    let first = CheckedCrashSite::new(
        CrashSiteLocation::new(first_state, 2),
        CrashCause::Abort,
        Vec::new(),
        vec![second_claim, first_claim, second_claim],
    )
    .with_path_guard_conjuncts(vec![path_guard.clone(), path_guard.clone()]);
    let second = CheckedCrashSite::new(
        CrashSiteLocation::new(second_state, 0),
        CrashCause::Trap,
        Vec::new(),
        Vec::new(),
    );
    let plan = CrashPlan::default()
        .with_checked_sites(vec![second.clone(), first.clone(), first.clone()])
        .expect("one crash cause occupies each source site");

    assert_eq!(plan.checked_sites(), &[first.clone(), second]);
    assert_eq!(
        plan.checked_sites()[0].frontier_lower_bound(),
        &[first_claim, second_claim],
        "frontier identity is canonical and duplicate-free"
    );
    assert_eq!(
        plan.checked_sites()[0].path_guard_conjuncts(),
        &[path_guard]
    );
    assert_eq!(
        plan.checked_site_at(first_state, 2)
            .map(|site| site.cause()),
        Some(CrashCause::Abort)
    );
    assert_eq!(plan.interface(), CrashInterface::InternalInferred);

    assert!(
        CrashPlan::default()
            .with_checked_sites(vec![
                first.clone(),
                CheckedCrashSite::new(first.location(), CrashCause::Trap, Vec::new(), Vec::new(),),
            ])
            .is_none()
    );
    assert!(
        CrashPlan::default()
            .with_checked_sites(vec![CheckedCrashSite::new(
                CrashSiteLocation::new(first_state, 3),
                CrashCause::Abort,
                Vec::new(),
                vec![language_semantics::PermissionClaimIdentity::Unknown],
            )])
            .is_none(),
        "an unknown claim identity cannot enter checked crash evidence"
    );
}

#[test]
fn crash_calls_retain_empty_refinements_and_reject_coordinate_collisions() {
    let machine = SymbolHandle::from_arena_index(2);
    let state = SymbolHandle::from_arena_index(3);
    let location = CrashCallSiteLocation::new(state, 4, 1);
    let call = CheckedCrashCallSite::new(location, machine, state, 17, Vec::new());
    let plan = CrashPlan::default()
        .with_checked_calls(vec![call.clone(), call.clone()])
        .expect("an identical duplicate canonicalizes away");
    assert_eq!(plan.checked_calls(), std::slice::from_ref(&call));
    assert!(plan.checked_calls()[0].surviving_buckets().is_empty());
    assert!(plan.checked_call_at(state, 4, 1).is_some());

    let conflicting = CheckedCrashCallSite::new(
        location,
        SymbolHandle::from_arena_index(8),
        state,
        18,
        vec![CrashRouteBucket::unconditional(CrashCause::Abort)],
    );
    assert!(
        CrashPlan::default()
            .with_checked_calls(vec![call, conflicting])
            .is_none(),
        "one invocation coordinate cannot name two checked crash refinements"
    );
}

#[test]
fn crash_contract_capsules_are_canonical_and_addressable() {
    let target_machine = SymbolHandle::from_arena_index(11);
    let target_state = SymbolHandle::from_arena_index(12);
    let capsule = CrashContractCapsule::new(
        target_machine,
        target_state,
        0xfeed,
        vec![
            CrashRouteBucket::unconditional(CrashCause::Abort),
            CrashRouteBucket::unconditional(CrashCause::Trap),
            CrashRouteBucket::unconditional(CrashCause::Abort),
        ],
    )
    .with_operational_envelope(
        vec!["Window".to_owned(), "Clock".to_owned(), "Window".to_owned()],
        vec!["service:Events".to_owned(), "service:Events".to_owned()],
        true,
        false,
        TerminationGuarantee::Terminates {
            premises: Vec::new(),
        },
    );
    assert_eq!(capsule.published_buckets().len(), 2);
    assert_eq!(capsule.published_service_reach(), ["Clock", "Window"]);
    assert_eq!(
        capsule.published_synchronous_invocations(),
        ["service:Events"]
    );
    assert!(capsule.published_may_suspend());
    assert!(!capsule.published_may_block());
    assert!(matches!(
        capsule.published_termination(),
        TerminationGuarantee::Terminates { .. }
    ));
    let plans = MachineContractPlans {
        machines: Vec::new(),
        crash_capsules: vec![capsule],
        realized_envelopes: Vec::new(),
    };
    assert_eq!(
        plans
            .crash_capsule(target_machine, target_state)
            .map(CrashContractCapsule::target_contract_report_fingerprint),
        Some(0xfeed)
    );
}

#[test]
fn crash_bucket_ids_join_checked_sites_to_their_published_contract() {
    let plan = CrashPlan::published_ceiling(vec![
        CrashRouteBucket::unconditional(CrashCause::Abort),
        CrashRouteBucket::unconditional(CrashCause::Trap),
    ]);
    let ids = plan
        .published_with_ids()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    assert_eq!(ids.iter().map(|id| id.get()).collect::<Vec<_>>(), [1, 2]);
    for (id, bucket) in plan.published_with_ids() {
        assert_eq!(plan.published_bucket(id), Some(bucket));
    }

    let abort_id = plan
        .published_with_ids()
        .find_map(|(id, bucket)| (bucket.cause() == CrashCause::Abort).then_some(id))
        .expect("published abort bucket");
    let site = CheckedCrashSite::new(
        CrashSiteLocation::new(SymbolHandle::from_arena_index(4), 0),
        CrashCause::Abort,
        vec![abort_id, abort_id],
        Vec::new(),
    );
    let plan = plan
        .with_checked_sites(vec![site])
        .expect("site coverage cites a same-cause bucket");
    assert_eq!(
        plan.checked_sites()[0].guard_covering_buckets(),
        &[abort_id]
    );
}

#[test]
fn operational_interfaces_participate_independently_in_contract_identity() {
    let fingerprint = |suspension, blocking| {
        contract_report_fingerprint(
            MachineSupplyMode::Boundary,
            &[],
            SynchronousInvocationInterface::PublishedCeiling,
            &[],
            suspension,
            blocking,
            &CrashPlan::default(),
            &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
            &[],
        )
    };
    let neither = fingerprint(
        SuspensionInterface::PublishedMaySuspend(false),
        BlockingInterface::PublishedMayBlock(false),
    );
    let suspending = fingerprint(
        SuspensionInterface::PublishedMaySuspend(true),
        BlockingInterface::PublishedMayBlock(false),
    );
    let blocking = fingerprint(
        SuspensionInterface::PublishedMaySuspend(false),
        BlockingInterface::PublishedMayBlock(true),
    );
    assert_ne!(neither, suspending);
    assert_ne!(neither, blocking);
    assert_ne!(suspending, blocking);
}

#[test]
fn machine_supply_vocabulary_participates_independently_in_contract_identity() {
    let fingerprint = |supply_mode| {
        contract_report_fingerprint(
            supply_mode,
            &[],
            SynchronousInvocationInterface::PublishedCeiling,
            &[],
            SuspensionInterface::PublishedMaySuspend(false),
            BlockingInterface::PublishedMayBlock(false),
            &CrashPlan::default(),
            &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
            &[],
        )
    };
    let fingerprints = [
        MachineSupplyMode::CheckedBody,
        MachineSupplyMode::Requirement,
        MachineSupplyMode::Boundary,
        MachineSupplyMode::AdmissionClaim,
        MachineSupplyMode::ExternalRealization {
            binding: Some(language_semantics::ExternalBindingId(1)),
            mechanism: Some(language_semantics::ExternalBindingMechanism::CompilerIntrinsic),
        },
        MachineSupplyMode::TopLevelRequirement,
    ]
    .map(fingerprint);

    for (index, fingerprint) in fingerprints.iter().enumerate() {
        assert!(
            fingerprints[..index]
                .iter()
                .all(|earlier| earlier != fingerprint),
            "machine supply modes must have distinct contract identities"
        );
    }
}

#[test]
fn external_binding_mechanism_participates_in_contract_identity() {
    let fingerprint = |mechanism| {
        contract_report_fingerprint(
            MachineSupplyMode::ExternalRealization {
                binding: Some(language_semantics::ExternalBindingId(1)),
                mechanism: Some(mechanism),
            },
            &[],
            SynchronousInvocationInterface::PublishedCeiling,
            &[],
            SuspensionInterface::PublishedMaySuspend(false),
            BlockingInterface::PublishedMayBlock(false),
            &CrashPlan::default(),
            &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
            &[],
        )
    };

    assert_ne!(
        fingerprint(language_semantics::ExternalBindingMechanism::CompilerIntrinsic),
        fingerprint(language_semantics::ExternalBindingMechanism::Syscall),
        "one per-program binding ordinal must not collapse distinct mechanisms"
    );
}

#[test]
fn termination_implementation_evidence_is_contract_invisible() {
    let interface = TerminationInterface::Published(TerminationGuarantee::Terminates {
        premises: Vec::new(),
    });
    let unresolved = language_semantics::MachineTerminationPlan {
        interface: interface.clone(),
        checked_summary: TerminationGuarantee::NoGuarantee,
        implementation_witness: None,
    };
    let established = language_semantics::MachineTerminationPlan {
        interface,
        checked_summary: TerminationGuarantee::Terminates {
            premises: Vec::new(),
        },
        implementation_witness: Some(language_semantics::RankingWitness {
            subjects: vec!["remaining".to_owned()],
            ranking_view: language_semantics::RankingViewId::NAT_DESCENDING,
            view_path: "Nat::Descending".to_owned(),
            view_arguments: Vec::new(),
            rank_range: None,
        }),
    };
    let fingerprint = |plan: &language_semantics::MachineTerminationPlan| {
        contract_report_fingerprint(
            MachineSupplyMode::CheckedBody,
            &[],
            SynchronousInvocationInterface::InternalInferred,
            &[],
            SuspensionInterface::InternalInferred,
            BlockingInterface::InternalInferred,
            &CrashPlan::default(),
            &plan.interface,
            &[],
        )
    };

    assert_ne!(unresolved, established);
    assert_eq!(fingerprint(&unresolved), fingerprint(&established));
}

#[test]
fn symbol_resolved_service_names_participate_in_contract_identity() {
    let fingerprint = |services: &[String]| {
        contract_report_fingerprint(
            MachineSupplyMode::Boundary,
            services,
            SynchronousInvocationInterface::PublishedCeiling,
            &[],
            SuspensionInterface::PublishedMaySuspend(false),
            BlockingInterface::PublishedMayBlock(false),
            &CrashPlan::default(),
            &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
            &[],
        )
    };
    let empty = fingerprint(&[]);
    let readable = fingerprint(&["Readable".to_owned()]);
    let queryable = fingerprint(&["Queryable".to_owned()]);
    let composite = fingerprint(&["Readable".to_owned(), "Queryable".to_owned()]);
    let reordered = fingerprint(&["Queryable".to_owned(), "Readable".to_owned()]);
    assert_ne!(empty, readable);
    assert_ne!(readable, queryable);
    assert_eq!(composite, reordered);
}

#[test]
fn synchronous_invocation_ceiling_participates_in_contract_identity() {
    let fingerprint = |interface, invocations: &[String]| {
        contract_report_fingerprint(
            MachineSupplyMode::Boundary,
            &[],
            interface,
            invocations,
            SuspensionInterface::PublishedMaySuspend(false),
            BlockingInterface::PublishedMayBlock(false),
            &CrashPlan::default(),
            &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
            &[],
        )
    };
    let omitted = fingerprint(SynchronousInvocationInterface::PublishedCeiling, &[]);
    let handler = fingerprint(
        SynchronousInvocationInterface::PublishedCeiling,
        &["parameter:0".to_owned()],
    );
    let composite = fingerprint(
        SynchronousInvocationInterface::PublishedCeiling,
        &["service:Clock".to_owned(), "parameter:0".to_owned()],
    );
    let reordered = fingerprint(
        SynchronousInvocationInterface::PublishedCeiling,
        &["parameter:0".to_owned(), "service:Clock".to_owned()],
    );
    let private = fingerprint(SynchronousInvocationInterface::InternalInferred, &[]);
    assert_ne!(omitted, handler);
    assert_ne!(omitted, private);
    assert_eq!(composite, reordered);
}

#[test]
fn internal_derivation_differs_from_published_omission() {
    let fingerprint = |termination| {
        contract_report_fingerprint(
            MachineSupplyMode::CheckedBody,
            &[],
            SynchronousInvocationInterface::InternalInferred,
            &[],
            SuspensionInterface::InternalInferred,
            BlockingInterface::InternalInferred,
            &CrashPlan::default(),
            termination,
            &[],
        )
    };
    assert_ne!(
        fingerprint(&TerminationInterface::InternalDerived),
        fingerprint(&TerminationInterface::Published(
            TerminationGuarantee::NoGuarantee
        ))
    );
}

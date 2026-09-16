//! Trust report reconstruction tests.

use super::{
    accepted_instance_contract_plan, accepted_machine_crash_routes, accepted_machine_may_block,
    accepted_machine_may_suspend, accepted_machine_service_reach,
    accepted_machine_synchronous_invocations, accepted_machine_terminates_guarantee,
    exact_machine_contract_plan, reconstruct_trust_report, trust_provider_realization,
};
use artifacts::{
    TrustCrashCause, TrustCrashRouteBucket, TrustCrashRouteGuard, TrustProviderRealization,
};
use checked_trees::{
    CheckedTrees, CrashCause, CrashPlan, CrashPredicateIdentity, CrashRouteBucket, CrashRouteGuard,
    MachineBlockingFact, MachineContractPlan, MachineContractPlans, MachineServiceReachRows,
    MachineSuspensionFact, MachineSynchronousInvocationFact, MachineTerminationFact,
    ServiceReachFacts,
};
use language_semantics::{
    BlockingInterface, BlockingPlan, MachineTerminationPlan, ProgressPremise, ProgressSubject,
    RankingWitness, SemanticDomainId, ServiceReachInterface, ServiceReachRowId,
    ServiceReachRowTable, ServiceReachTable, SuspensionInterface, SuspensionPlan,
    SynchronousInvocationInterface, SynchronousInvocationPlan, TerminationGuarantee,
    TerminationInterface,
};
use symbols::SymbolHandle;

#[test]
fn trust_report_rejects_compact_equal_selected_plan_substitution() {
    let checked = CheckedTrees::default();
    let classifications = crate::AcceptedTemplateClassifications::capture(&checked.typed);
    let candidate = effects::provider_plan::ProviderPlan {
        name: "Provider".to_owned(),
        provider_type: "Provider".to_owned(),
        schema: effects::provider_plan::ServiceSchema {
            trait_name: "Pair".to_owned(),
            methods: vec![effects::provider_plan::ServiceMethod {
                name: "run".to_owned(),
                requirement_owner: "Pair".to_owned(),
                requirement_identity: "Pair::run".to_owned(),
                service_reach: vec!["Pair".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        },
        rows: vec![effects::provider_plan::ProviderPlanRow {
            method: "run".to_owned(),
            requirement_identity: "Pair::run".to_owned(),
            requirement_lifetime_partition: Vec::new(),
            binding: effects::provider_plan::ProviderBinding::VtableSlot { index: 0 },
        }],
        ..Default::default()
    };
    let mut substituted = candidate.clone();
    substituted.schema.methods[0].requirement_owner = "OtherPair".to_owned();
    assert_eq!(
        candidate.report_fingerprint(),
        substituted.report_fingerprint()
    );
    assert_ne!(candidate.identity_digest(), substituted.identity_digest());
    let selected = effects::SelectedProviderPlanFacts::from_selected_plans(vec![substituted])
        .expect("compact-equal selected closure");

    let diagnostics =
        reconstruct_trust_report(&checked, &[], &[candidate], &selected, &classifications)
            .expect_err("exact selected-plan substitution must not enter the trust report");
    assert!(
        diagnostics[0]
            .message
            .contains("replays against 0 exact candidate plans")
    );
}

#[test]
fn trust_realization_retains_normalized_locator() {
    let locator = effects::normalize_foreign_locator(
        effects::ForeignLocatorCandidate::ElfVersioned {
            object: b"libopaque.so".to_vec(),
            symbol: b"invoke_raw".to_vec(),
            version: b"OPAQUE_2.0".to_vec(),
        },
        target::TargetProfile::LinuxX64,
    )
    .expect("valid normalized Linux import");
    let usage = effects::provider_plan::EvaluatedBindingUsage::from_evaluator(
        7, 1, 10, 1_000, 0, 0, 4, 12, 3, 0,
    )
    .expect("valid fixture usage");
    let receipt = effects::provider_plan::EvaluatedBindingReceipt::from_evaluation(
        None,
        "fixture::producer".to_owned(),
        effects::provider_plan::EvaluatedBindingProducerClosureDigest::from_bytes([11; 32])
            .unwrap(),
        1,
        usage,
        effects::provider_plan::EvaluatedBindingEvaluationDigest::from_bytes([12; 32]).unwrap(),
        1,
        effects::provider_plan::EvaluatedBindingMaterializationDigest::from_bytes([13; 32])
            .unwrap(),
        locator.identity_digest(),
    )
    .expect("valid fixture receipt");
    let evaluated = effects::provider_plan::EvaluatedForeignImport::from_retained_evidence(
        locator.clone(),
        receipt,
    )
    .expect("receipt matches locator");
    let normalized = trust_provider_realization(&effects::provider_plan::ProviderBinding::Import {
        evaluated: evaluated.clone(),
    });
    assert_eq!(normalized, TrustProviderRealization::Import { evaluated });
    assert_eq!(
        normalized.foreign_locator_compatibility_report_identity(),
        Some(locator.non_authoritative_compatibility_fingerprint()),
    );
}

#[test]
fn accepted_instance_contract_identity_copies_exact_plan_and_fails_closed_when_missing() {
    let machine = SymbolHandle::from_arena_index(1);
    let missing = accepted_instance_contract_plan(&CheckedTrees::default(), machine, "admitted")
        .expect_err("missing checked instance plan must fail closed");
    assert!(
        missing
            .message
            .contains("has no exact checked contract plan")
    );

    let mut checked = CheckedTrees::default();
    checked
        .facts
        .contract_plans
        .machines
        .push(MachineContractPlan {
            machine,
            closed_scalar_values: Default::default(),
            crash: CrashPlan::default(),
            report_fingerprint: 0x1234_5678_9abc_def0,
            commitment: checked_trees::MachineContractCommitment::from_digest([1; 32]),
        });
    let identity = accepted_instance_contract_plan(&checked, machine, "admitted")
        .expect("exact checked instance plan");
    assert_eq!(identity.report_fingerprint, 0x1234_5678_9abc_def0);
    assert_eq!(
        identity.commitment,
        checked_trees::MachineContractCommitment::from_digest([1; 32])
    );

    let duplicate = checked.facts.contract_plans.machines[0].clone();
    checked.facts.contract_plans.machines.push(duplicate);
    let duplicate = accepted_instance_contract_plan(&checked, machine, "admitted")
        .expect_err("duplicate checked instance plans must fail closed");
    assert!(
        duplicate
            .message
            .contains("duplicate exact checked contract plans")
    );
}

fn checked_with_reach(
    machine: SymbolHandle,
    interface: ServiceReachInterface,
    services: ServiceReachTable,
    rows: ServiceReachRowTable,
) -> CheckedTrees {
    let mut service_reaches = ServiceReachFacts {
        services,
        rows,
        ..Default::default()
    };
    service_reaches.machines.append_to_span(
        &mut service_reaches.root_machines,
        MachineServiceReachRows {
            machine,
            interface,
            ..Default::default()
        },
    );
    let mut checked = CheckedTrees::default();
    checked.facts.service_reaches = service_reaches;
    checked
}

fn checked_with_crash(machine: SymbolHandle, crash: CrashPlan) -> CheckedTrees {
    let mut checked = CheckedTrees::default();
    checked.facts.contract_plans = MachineContractPlans {
        machines: vec![MachineContractPlan {
            machine,
            closed_scalar_values: Default::default(),
            crash,
            report_fingerprint: 0,
            commitment: checked_trees::MachineContractCommitment::from_digest([0; 32]),
        }],
        crash_capsules: Vec::new(),
        realized_envelopes: Vec::new(),
    };
    checked
}

#[derive(Clone, Copy)]
enum AcceptedTrustAxis {
    Contract,
    ServiceReach,
    SynchronousInvocation,
    Suspension,
    Blocking,
    Termination,
}

impl AcceptedTrustAxis {
    const ALL: [Self; 6] = [
        Self::Contract,
        Self::ServiceReach,
        Self::SynchronousInvocation,
        Self::Suspension,
        Self::Blocking,
        Self::Termination,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Contract => "contract",
            Self::ServiceReach => "service reach",
            Self::SynchronousInvocation => "synchronous invocation",
            Self::Suspension => "suspension",
            Self::Blocking => "blocking",
            Self::Termination => "termination",
        }
    }
}

fn accepted_exact_rows_fixture(machine: SymbolHandle) -> CheckedTrees {
    let mut checked = CheckedTrees::default();
    checked
        .facts
        .contract_plans
        .machines
        .push(MachineContractPlan {
            machine,
            closed_scalar_values: Default::default(),
            crash: CrashPlan::published_ceiling(Vec::new()),
            report_fingerprint: 0x1234,
            commitment: checked_trees::MachineContractCommitment::from_digest([1; 32]),
        });
    let empty_reach = checked.facts.service_reaches.rows.intern(Vec::new());
    checked.facts.service_reaches.machines.append_to_span(
        &mut checked.facts.service_reaches.root_machines,
        MachineServiceReachRows {
            machine,
            interface: ServiceReachInterface::PublishedCeiling(empty_reach),
            ..Default::default()
        },
    );
    checked
        .facts
        .synchronous_invocations
        .machines
        .push(MachineSynchronousInvocationFact {
            machine,
            published_targets: Vec::new(),
            checked_inferred_targets: Vec::new(),
            plan: SynchronousInvocationPlan {
                interface: SynchronousInvocationInterface::PublishedCeiling,
                published: Vec::new(),
                checked_inferred: vec!["service:Private".to_owned()],
            },
        });
    checked
        .facts
        .suspensions
        .machines
        .push(MachineSuspensionFact {
            machine,
            plan: SuspensionPlan {
                interface: SuspensionInterface::PublishedMaySuspend(false),
                checked_may_suspend: true,
            },
        });
    checked.facts.blocking.machines.push(MachineBlockingFact {
        machine,
        plan: BlockingPlan {
            interface: BlockingInterface::PublishedMayBlock(false),
            checked_may_block: true,
        },
    });
    checked
        .facts
        .termination
        .machines
        .push(MachineTerminationFact {
            machine,
            plan: MachineTerminationPlan {
                interface: TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
                checked_summary: TerminationGuarantee::Terminates {
                    premises: Vec::new(),
                },
                implementation_witness: Some(RankingWitness {
                    view_path: "Private::Witness".to_owned(),
                    ..Default::default()
                }),
            },
        });
    checked
}

fn validate_accepted_axis(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    axis: AcceptedTrustAxis,
) -> Result<(), String> {
    let result = match axis {
        AcceptedTrustAxis::Contract => {
            exact_machine_contract_plan(checked, machine, "accepted machine `accepted`").map(|_| ())
        }
        AcceptedTrustAxis::ServiceReach => {
            accepted_machine_service_reach(checked, machine, "accepted").map(|_| ())
        }
        AcceptedTrustAxis::SynchronousInvocation => {
            accepted_machine_synchronous_invocations(checked, machine, "accepted").map(|_| ())
        }
        AcceptedTrustAxis::Suspension => {
            accepted_machine_may_suspend(checked, machine, "accepted").map(|_| ())
        }
        AcceptedTrustAxis::Blocking => {
            accepted_machine_may_block(checked, machine, "accepted").map(|_| ())
        }
        AcceptedTrustAxis::Termination => {
            accepted_machine_terminates_guarantee(checked, machine, "accepted").map(|_| ())
        }
    };
    result.map_err(|diagnostic| diagnostic.message)
}

fn remove_accepted_axis(
    checked: &mut CheckedTrees,
    machine: SymbolHandle,
    axis: AcceptedTrustAxis,
) {
    match axis {
        AcceptedTrustAxis::Contract => checked
            .facts
            .contract_plans
            .machines
            .retain(|row| row.machine != machine),
        AcceptedTrustAxis::ServiceReach => {
            checked
                .facts
                .service_reaches
                .machines
                .for_each_mut(|_, row| {
                    if row.machine == machine {
                        row.machine = SymbolHandle::invalid();
                    }
                });
        }
        AcceptedTrustAxis::SynchronousInvocation => checked
            .facts
            .synchronous_invocations
            .machines
            .retain(|row| row.machine != machine),
        AcceptedTrustAxis::Suspension => checked
            .facts
            .suspensions
            .machines
            .retain(|row| row.machine != machine),
        AcceptedTrustAxis::Blocking => checked
            .facts
            .blocking
            .machines
            .retain(|row| row.machine != machine),
        AcceptedTrustAxis::Termination => checked
            .facts
            .termination
            .machines
            .retain(|row| row.machine != machine),
    }
}

fn append_accepted_axis_copy(
    checked: &mut CheckedTrees,
    source: SymbolHandle,
    owner: SymbolHandle,
    axis: AcceptedTrustAxis,
) {
    match axis {
        AcceptedTrustAxis::Contract => {
            let mut row = checked
                .facts
                .contract_plans
                .machines
                .iter()
                .find(|row| row.machine == source)
                .expect("source contract row")
                .clone();
            row.machine = owner;
            checked.facts.contract_plans.machines.push(row);
        }
        AcceptedTrustAxis::ServiceReach => {
            let mut row = checked
                .facts
                .service_reaches
                .machines()
                .iter()
                .find(|row| row.machine == source)
                .expect("source service-reach row")
                .clone();
            row.machine = owner;
            checked
                .facts
                .service_reaches
                .machines
                .append_to_span(&mut checked.facts.service_reaches.root_machines, row);
        }
        AcceptedTrustAxis::SynchronousInvocation => {
            let mut row = checked
                .facts
                .synchronous_invocations
                .machines
                .iter()
                .find(|row| row.machine == source)
                .expect("source synchronous-invocation row")
                .clone();
            row.machine = owner;
            checked.facts.synchronous_invocations.machines.push(row);
        }
        AcceptedTrustAxis::Suspension => {
            let mut row = *checked
                .facts
                .suspensions
                .machines
                .iter()
                .find(|row| row.machine == source)
                .expect("source suspension row");
            row.machine = owner;
            checked.facts.suspensions.machines.push(row);
        }
        AcceptedTrustAxis::Blocking => {
            let mut row = *checked
                .facts
                .blocking
                .machines
                .iter()
                .find(|row| row.machine == source)
                .expect("source blocking row");
            row.machine = owner;
            checked.facts.blocking.machines.push(row);
        }
        AcceptedTrustAxis::Termination => {
            let mut row = checked
                .facts
                .termination
                .machines
                .iter()
                .find(|row| row.machine == source)
                .expect("source termination row")
                .clone();
            row.machine = owner;
            checked.facts.termination.machines.push(row);
        }
    }
}

#[test]
fn accepted_trust_axes_require_one_exact_row_without_global_poisoning() {
    let machine = SymbolHandle::from_arena_index(1);
    let unrelated = SymbolHandle::from_arena_index(2);
    for axis in AcceptedTrustAxis::ALL {
        let mut missing = accepted_exact_rows_fixture(machine);
        remove_accepted_axis(&mut missing, machine, axis);
        let error = validate_accepted_axis(&missing, machine, axis)
            .expect_err("missing exact row must fail closed");
        assert!(
            error.contains("no exact checked"),
            "{} missing diagnostic: {error}",
            axis.label()
        );

        let mut duplicate = accepted_exact_rows_fixture(machine);
        append_accepted_axis_copy(&mut duplicate, machine, machine, axis);
        let error = validate_accepted_axis(&duplicate, machine, axis)
            .expect_err("duplicate exact row must fail closed");
        assert!(
            error.contains("duplicate exact checked"),
            "{} duplicate diagnostic: {error}",
            axis.label()
        );

        let mut unrelated_duplicates = accepted_exact_rows_fixture(machine);
        append_accepted_axis_copy(&mut unrelated_duplicates, machine, unrelated, axis);
        append_accepted_axis_copy(&mut unrelated_duplicates, machine, unrelated, axis);
        validate_accepted_axis(&unrelated_duplicates, machine, axis).unwrap_or_else(|error| {
            panic!("{} unrelated rows must be ignored: {error}", axis.label())
        });
    }
}

#[test]
fn accepted_trust_axes_preserve_explicit_public_negatives() {
    let machine = SymbolHandle::from_arena_index(1);
    let checked = accepted_exact_rows_fixture(machine);
    assert_eq!(
        accepted_machine_service_reach(&checked, machine, "accepted"),
        Ok(Vec::new())
    );
    assert_eq!(
        accepted_machine_synchronous_invocations(&checked, machine, "accepted"),
        Ok(Vec::new())
    );
    assert_eq!(
        accepted_machine_may_suspend(&checked, machine, "accepted"),
        Ok(false)
    );
    assert_eq!(
        accepted_machine_may_block(&checked, machine, "accepted"),
        Ok(false)
    );
    assert_eq!(
        accepted_machine_terminates_guarantee(&checked, machine, "accepted"),
        Ok(false)
    );
    let contract = exact_machine_contract_plan(&checked, machine, "accepted machine `accepted`")
        .expect("one exact contract");
    assert_eq!(
        accepted_machine_crash_routes(contract, "accepted"),
        Ok(Vec::new())
    );
}

#[test]
fn accepted_service_reach_projects_only_exact_published_registry_rows() {
    let machine = SymbolHandle::from_arena_index(1);
    let service_symbol = SymbolHandle::from_arena_index(2);
    let mut services = ServiceReachTable::default();
    let service = services.intern(service_symbol, "Clock");
    let mut rows = ServiceReachRowTable::default();
    let row = rows.intern(vec![service]);
    let checked = checked_with_reach(
        machine,
        ServiceReachInterface::PublishedCeiling(row),
        services,
        rows,
    );

    assert_eq!(
        accepted_machine_service_reach(&checked, machine, "accepted"),
        Ok(vec!["Clock".to_owned()])
    );
}

#[test]
fn accepted_service_reach_fails_closed_on_missing_internal_and_unknown_facts() {
    let machine = SymbolHandle::from_arena_index(1);
    let missing = accepted_machine_service_reach(&CheckedTrees::default(), machine, "missing")
        .expect_err("missing facts reject")
        .to_string();
    assert!(missing.contains("has no exact checked service-reach facts"));

    let internal = checked_with_reach(
        machine,
        ServiceReachInterface::InternalInferred,
        ServiceReachTable::default(),
        ServiceReachRowTable::default(),
    );
    let internal = accepted_machine_service_reach(&internal, machine, "internal")
        .expect_err("private inference rejects")
        .to_string();
    assert!(internal.contains("has no published service-reach ceiling"));

    let mut foreign_services = ServiceReachTable::default();
    let unknown_service = foreign_services.intern(SymbolHandle::from_arena_index(2), "Unknown");
    let mut rows = ServiceReachRowTable::default();
    let row = rows.intern(vec![unknown_service]);
    let unknown = checked_with_reach(
        machine,
        ServiceReachInterface::PublishedCeiling(row),
        ServiceReachTable::default(),
        rows,
    );
    let unknown = accepted_machine_service_reach(&unknown, machine, "unknown")
        .expect_err("unregistered service rejects")
        .to_string();
    assert!(unknown.contains("references an unknown service-reach identity"));

    let invalid_row = checked_with_reach(
        machine,
        ServiceReachInterface::PublishedCeiling(ServiceReachRowId(99)),
        ServiceReachTable::default(),
        ServiceReachRowTable::default(),
    );
    let invalid_row = accepted_machine_service_reach(&invalid_row, machine, "invalid-row")
        .expect_err("unknown row identity rejects")
        .to_string();
    assert!(invalid_row.contains("references an unknown service-reach row identity"));
}

#[test]
fn accepted_synchronous_invocations_copy_only_the_exact_published_vector() {
    let machine = SymbolHandle::from_arena_index(1);
    let mut checked = CheckedTrees::default();
    checked
        .facts
        .synchronous_invocations
        .machines
        .push(MachineSynchronousInvocationFact {
            machine,
            published_targets: Vec::new(),
            checked_inferred_targets: Vec::new(),
            plan: SynchronousInvocationPlan {
                interface: SynchronousInvocationInterface::PublishedCeiling,
                published: vec!["parameter:0".to_owned(), "service:Clock".to_owned()],
                checked_inferred: vec!["service:Private".to_owned()],
            },
        });

    assert_eq!(
        accepted_machine_synchronous_invocations(&checked, machine, "accepted"),
        Ok(vec!["parameter:0".to_owned(), "service:Clock".to_owned()])
    );
}

#[test]
fn accepted_synchronous_invocations_fail_closed_on_missing_and_internal_facts() {
    let machine = SymbolHandle::from_arena_index(1);
    let missing =
        accepted_machine_synchronous_invocations(&CheckedTrees::default(), machine, "missing")
            .expect_err("missing facts reject")
            .to_string();
    assert!(missing.contains("has no exact checked synchronous-invocation facts"));

    let mut internal = CheckedTrees::default();
    internal
        .facts
        .synchronous_invocations
        .machines
        .push(MachineSynchronousInvocationFact {
            machine,
            published_targets: Vec::new(),
            checked_inferred_targets: Vec::new(),
            plan: SynchronousInvocationPlan {
                interface: SynchronousInvocationInterface::InternalInferred,
                published: Vec::new(),
                checked_inferred: vec!["parameter:0".to_owned()],
            },
        });
    let internal = accepted_machine_synchronous_invocations(&internal, machine, "internal")
        .expect_err("private inference rejects")
        .to_string();
    assert!(internal.contains("has no published synchronous-invocation ceiling"));
}

#[test]
fn accepted_suspension_copies_only_the_published_interface_bit() {
    let machine = SymbolHandle::from_arena_index(1);
    let mut checked = CheckedTrees::default();
    checked
        .facts
        .suspensions
        .machines
        .push(MachineSuspensionFact {
            machine,
            plan: SuspensionPlan {
                interface: SuspensionInterface::PublishedMaySuspend(true),
                checked_may_suspend: false,
            },
        });

    assert_eq!(
        accepted_machine_may_suspend(&checked, machine, "accepted"),
        Ok(true)
    );
}

#[test]
fn accepted_suspension_fails_closed_on_missing_and_internal_facts() {
    let machine = SymbolHandle::from_arena_index(1);
    let missing = accepted_machine_may_suspend(&CheckedTrees::default(), machine, "missing")
        .expect_err("missing facts reject")
        .to_string();
    assert!(missing.contains("has no exact checked suspension facts"));

    let mut internal = CheckedTrees::default();
    internal
        .facts
        .suspensions
        .machines
        .push(MachineSuspensionFact {
            machine,
            plan: SuspensionPlan {
                interface: SuspensionInterface::InternalInferred,
                checked_may_suspend: true,
            },
        });
    let internal = accepted_machine_may_suspend(&internal, machine, "internal")
        .expect_err("private inference rejects")
        .to_string();
    assert!(internal.contains("has no published suspension ceiling"));
}

#[test]
fn accepted_blocking_copies_only_the_published_interface_bit() {
    let machine = SymbolHandle::from_arena_index(1);
    let mut checked = CheckedTrees::default();
    checked.facts.blocking.machines.push(MachineBlockingFact {
        machine,
        plan: BlockingPlan {
            interface: BlockingInterface::PublishedMayBlock(true),
            checked_may_block: false,
        },
    });

    assert_eq!(
        accepted_machine_may_block(&checked, machine, "accepted"),
        Ok(true)
    );
}

#[test]
fn accepted_blocking_fails_closed_on_missing_and_internal_facts() {
    let machine = SymbolHandle::from_arena_index(1);
    let missing = accepted_machine_may_block(&CheckedTrees::default(), machine, "missing")
        .expect_err("missing facts reject")
        .to_string();
    assert!(missing.contains("has no exact checked blocking facts"));

    let mut internal = CheckedTrees::default();
    internal.facts.blocking.machines.push(MachineBlockingFact {
        machine,
        plan: BlockingPlan {
            interface: BlockingInterface::InternalInferred,
            checked_may_block: true,
        },
    });
    let internal = accepted_machine_may_block(&internal, machine, "internal")
        .expect_err("private inference rejects")
        .to_string();
    assert!(internal.contains("has no published blocking ceiling"));
}

#[test]
fn accepted_termination_copies_only_the_premise_free_published_interface() {
    let machine = SymbolHandle::from_arena_index(1);
    let mut checked = CheckedTrees::default();
    checked
        .facts
        .termination
        .machines
        .push(MachineTerminationFact {
            machine,
            plan: MachineTerminationPlan {
                interface: TerminationInterface::Published(TerminationGuarantee::Terminates {
                    premises: Vec::new(),
                }),
                checked_summary: TerminationGuarantee::NoGuarantee,
                implementation_witness: Some(RankingWitness {
                    view_path: "Private::Witness".to_owned(),
                    ..Default::default()
                }),
            },
        });

    assert_eq!(
        accepted_machine_terminates_guarantee(&checked, machine, "accepted"),
        Ok(true)
    );
}

#[test]
fn accepted_termination_fails_closed_on_missing_internal_and_premised_facts() {
    let machine = SymbolHandle::from_arena_index(1);
    let missing =
        accepted_machine_terminates_guarantee(&CheckedTrees::default(), machine, "missing")
            .expect_err("missing facts reject")
            .to_string();
    assert!(missing.contains("has no exact checked termination facts"));

    let mut internal = CheckedTrees::default();
    internal
        .facts
        .termination
        .machines
        .push(MachineTerminationFact {
            machine,
            plan: MachineTerminationPlan {
                interface: TerminationInterface::InternalDerived,
                checked_summary: TerminationGuarantee::Terminates {
                    premises: Vec::new(),
                },
                implementation_witness: None,
            },
        });
    let internal = accepted_machine_terminates_guarantee(&internal, machine, "internal")
        .expect_err("private derivation rejects")
        .to_string();
    assert!(internal.contains("has no published termination interface"));

    let mut premised = CheckedTrees::default();
    premised
        .facts
        .termination
        .machines
        .push(MachineTerminationFact {
            machine,
            plan: MachineTerminationPlan {
                interface: TerminationInterface::Published(TerminationGuarantee::Terminates {
                    premises: vec![ProgressPremise {
                        profile: SemanticDomainId(1),
                        subject: ProgressSubject {
                            root: machine,
                            projections: Vec::new(),
                        },
                    }],
                }),
                checked_summary: TerminationGuarantee::NoGuarantee,
                implementation_witness: None,
            },
        });
    let premised = accepted_machine_terminates_guarantee(&premised, machine, "premised")
        .expect_err("progress-premised guarantee rejects")
        .to_string();
    assert!(premised.contains("cannot enter the premise-free trust row"));
}

#[test]
fn accepted_crash_routes_copy_exact_published_bucket_and_guard_identity() {
    let machine = SymbolHandle::from_arena_index(1);
    let trap = CrashRouteBucket::new(
        CrashCause::Trap,
        vec![CrashRouteGuard::Predicate(
            CrashPredicateIdentity::from_canonical_bytes(vec![1, 2]),
        )],
    )
    .expect("nonempty guarded bucket");
    let abort = CrashRouteBucket::unconditional(CrashCause::Abort);
    let checked = checked_with_crash(machine, CrashPlan::published_ceiling(vec![abort, trap]));
    let contract = exact_machine_contract_plan(&checked, machine, "accepted machine `accepted`")
        .expect("one exact contract");

    assert_eq!(
        accepted_machine_crash_routes(contract, "accepted"),
        Ok(vec![
            TrustCrashRouteBucket {
                cause: TrustCrashCause::Trap,
                alternative_guards: vec![TrustCrashRouteGuard::PredicateIdentity(vec![1, 2])],
            },
            TrustCrashRouteBucket {
                cause: TrustCrashCause::Abort,
                alternative_guards: vec![TrustCrashRouteGuard::Truth],
            },
        ])
    );
}

#[test]
fn accepted_crash_routes_fail_closed_on_missing_and_internal_plans() {
    let machine = SymbolHandle::from_arena_index(1);
    let missing = exact_machine_contract_plan(
        &CheckedTrees::default(),
        machine,
        "accepted machine `missing`",
    )
    .expect_err("missing plan rejects")
    .to_string();
    assert!(missing.contains("has no exact checked contract plan"));

    let internal = checked_with_crash(machine, CrashPlan::default());
    let contract = exact_machine_contract_plan(&internal, machine, "accepted machine `internal`")
        .expect("one exact contract");
    let internal = accepted_machine_crash_routes(contract, "internal")
        .expect_err("private inference rejects")
        .to_string();
    assert!(internal.contains("has no published crash ceiling"));

    assert_eq!(
        CrashRouteBucket::new(CrashCause::Trap, Vec::new()),
        None,
        "the checked owner seals the empty-guard state before trust projection"
    );
}

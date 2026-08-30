use super::*;

#[test]
fn machine_contract_manifest_keeps_interface_and_witness_separate() {
    let symbol = SymbolHandle::from_arena_index(2);
    let state_symbol = SymbolHandle::from_arena_index(3);
    let capsule_machine_symbol = SymbolHandle::from_arena_index(4);
    let capsule_state_symbol = SymbolHandle::from_arena_index(5);
    let service_symbol = SymbolHandle::from_arena_index(1);
    let mut program = CheckedTrees::default();
    let service = program
        .facts
        .service_reaches
        .services
        .intern(service_symbol, "Readable");
    let service_row = program.facts.service_reaches.rows.intern(vec![service]);
    let crash = psi_checked_trees::CrashPlan::published_ceiling(vec![
        psi_checked_trees::CrashRouteBucket::unconditional(psi_checked_trees::CrashCause::Abort),
    ]);
    let abort_bucket = crash
        .published_with_ids()
        .next()
        .map(|(id, _)| id)
        .expect("published abort bucket");
    let abandoned_claim = psi_language_semantics::PermissionClaimIdentity::Established {
        machine_symbol: symbol,
        state_symbol,
        source: psi_language_semantics::PermissionEventSource::StateEntry,
        ordinal: 0,
    };
    let crash = crash
        .with_checked_sites(vec![
            psi_checked_trees::CheckedCrashSite::new(
                psi_checked_trees::CrashSiteLocation::new(state_symbol, 4),
                psi_checked_trees::CrashCause::Abort,
                vec![abort_bucket],
                vec![abandoned_claim],
            )
            .with_path_guard_conjuncts(vec![
                psi_checked_trees::CrashPredicateIdentity::from_canonical_bytes(vec![
                    1, 9, 0, 0, 0, 0,
                ]),
            ])
            .with_path_guard_consequences(vec![
                psi_checked_trees::CrashPredicateIdentity::from_canonical_bytes(vec![1, 4, 1]),
            ]),
        ])
        .expect("one crash site per source location")
        .with_checked_calls(vec![
            psi_checked_trees::CheckedCrashCallSite::new(
                psi_checked_trees::CrashCallSiteLocation::new(state_symbol, 7, 2),
                symbol,
                state_symbol,
                0x1234,
                vec![psi_checked_trees::CrashRouteBucket::unconditional(
                    psi_checked_trees::CrashCause::Trap,
                )],
            )
            .with_path_guard_conjuncts(vec![
                psi_checked_trees::CrashPredicateIdentity::from_canonical_bytes(vec![1, 4, 1]),
            ]),
            psi_checked_trees::CheckedCrashCallSite::new(
                psi_checked_trees::CrashCallSiteLocation::new(state_symbol, 8, 0),
                capsule_machine_symbol,
                capsule_state_symbol,
                0x5678,
                vec![psi_checked_trees::CrashRouteBucket::unconditional(
                    psi_checked_trees::CrashCause::Trap,
                )],
            ),
        ])
        .expect("one crash call per invocation coordinate");
    let mut machine = Machine {
        symbol,
        name: Identifier::generated("Worker::run"),
        termination_plan: MachineTerminationPlan {
            implementation_witness: Some(RankingWitness {
                subjects: vec!["remaining".to_string()],
                ranking_view: RankingViewId::NAT_DESCENDING,
                view_path: "Nat::Descending".to_string(),
                view_arguments: Vec::new(),
                rank_range: None,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("entry"),
        ..Default::default()
    };
    for _ in 0..9 {
        program
            .typed
            .statement_table
            .push_statement(&mut state.statement_nodes, Default::default());
    }
    program.typed.push_machine_state(&mut machine, state);
    program.typed.push_machine(machine);
    let mut capsule_trait = TraitDefinition {
        symbol: capsule_machine_symbol,
        name: Identifier::generated("Firmware"),
        ..Default::default()
    };
    program.typed.push_trait_machine_signature(
        &mut capsule_trait,
        StateSignature {
            symbol: capsule_state_symbol,
            name: Identifier::generated("read"),
            ..Default::default()
        },
    );
    program.typed.push_trait_definition(capsule_trait);
    let calls = program.facts.flow.control.calls.insert_many([
        FlowCallFact {
            statement_index: 7,
            call_ordinal: 2,
            target_symbol: state_symbol,
            ..Default::default()
        },
        FlowCallFact {
            statement_index: 8,
            call_ordinal: 0,
            target_symbol: capsule_state_symbol,
            ..Default::default()
        },
    ]);
    program.facts.flow.control.states.insert(FlowStateFact {
        machine_symbol: symbol,
        state_symbol,
        calls,
        ..Default::default()
    });
    program.facts.service_reaches.machines.append_to_span(
        &mut program.facts.service_reaches.root_machines,
        MachineServiceReachRows {
            machine: symbol,
            interface: psi_language_semantics::ServiceReachInterface::PublishedCeiling(service_row),
            published_ceiling: service_row,
            inferred_direct: service_row,
            inferred_transitive: service_row,
            concrete_transitive: service_row,
            effective: service_row,
            concrete_effective: service_row,
            unresolved_installation_reaches: Vec::new(),
            states: Default::default(),
        },
    );
    program.facts.synchronous_invocations.machines.push(
        psi_checked_trees::MachineSynchronousInvocationFact {
            machine: symbol,
            published_targets: vec![psi_effects::InvocationTarget::Parameter(0)],
            checked_inferred_targets: vec![psi_effects::InvocationTarget::Parameter(0)],
            plan: psi_language_semantics::SynchronousInvocationPlan {
                interface: psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling,
                published: vec!["parameter:0".to_owned()],
                checked_inferred: vec!["parameter:0".to_owned()],
            },
        },
    );
    program
        .facts
        .suspensions
        .machines
        .push(psi_checked_trees::MachineSuspensionFact {
            machine: symbol,
            plan: SuspensionPlan {
                interface: SuspensionInterface::PublishedMaySuspend(false),
                checked_may_suspend: false,
            },
        });
    program
        .facts
        .blocking
        .machines
        .push(psi_checked_trees::MachineBlockingFact {
            machine: symbol,
            plan: BlockingPlan {
                interface: BlockingInterface::PublishedMayBlock(true),
                checked_may_block: true,
            },
        });
    program
        .facts
        .termination
        .machines
        .push(psi_checked_trees::MachineTerminationFact {
            machine: symbol,
            plan: MachineTerminationPlan {
                interface: psi_language_semantics::TerminationInterface::Published(
                    TerminationGuarantee::NoGuarantee,
                ),
                checked_summary: TerminationGuarantee::Terminates {
                    premises: Vec::new(),
                },
                implementation_witness: Some(RankingWitness {
                    subjects: vec!["remaining".to_string()],
                    ranking_view: RankingViewId::NAT_DESCENDING,
                    view_path: "Nat::Descending".to_string(),
                    view_arguments: Vec::new(),
                    rank_range: None,
                }),
            },
        });
    program.facts.mutation.machines.push(MachineMutationFact {
        machine: symbol,
        state_write_frames: vec![StateWriteFramePlan {
            state: state_symbol,
            frame: psi_facts::NormalizedWriteFrame::opaque(),
        }],
    });
    program
        .facts
        .contract_plans
        .machines
        .push(MachineContractPlan {
            machine: symbol,
            closed_scalar_values: Default::default(),
            crash,
            report_fingerprint: 0x1234,
            commitment: psi_checked_trees::MachineContractCommitment::from_digest([1; 32]),
        });
    program
        .facts
        .contract_plans
        .crash_capsules
        .push(psi_checked_trees::CrashContractCapsule::new(
            capsule_machine_symbol,
            capsule_state_symbol,
            0x5678,
            vec![psi_checked_trees::CrashRouteBucket::unconditional(
                psi_checked_trees::CrashCause::Trap,
            )],
        ));
    let json = machine_contract_manifest_json(&program);
    let contract_start = json.find("\"contract\"").expect("contract object");
    let implementation_start = json
        .find("\"implementation\"")
        .expect("implementation object");
    let contract = &json[contract_start..implementation_start];

    assert!(contract.contains("\"report_fingerprint\": \"0x0000000000001234\""));
    assert!(contract.contains("\"supply\": \"checked_body\""));
    assert!(!contract.contains("\"supply\": \"accepted\""));
    assert!(json.contains("\"machine_overload_identity\": \"named-callable(path(Worker::run)"));
    assert!(contract.contains(
        "\"service_reach\": {\"interface\": \"published_ceiling\", \"services\": [\"Readable\"]}"
    ));
    assert!(contract.contains(
        "\"synchronous_invocation\": {\"interface\": \"published_ceiling\", \"targets\": [\"parameter:0\"]}"
    ));
    assert!(contract.contains(
        "\"suspension\": {\"interface\": \"published_ceiling\", \"may_suspend\": false}"
    ));
    assert!(
        contract
            .contains("\"blocking\": {\"interface\": \"published_ceiling\", \"may_block\": true}")
    );
    assert!(contract.contains(
        "\"crashes\": {\"interface\": \"published_ceiling\", \"buckets\": [{\"cause\": \"Abort\", \"alternative_guards\": [\"true\"]}]}"
    ));
    assert!(contract.contains(
        "\"termination\": {\"interface\": \"published\", \"guarantee\": {\"kind\": \"no_guarantee\"}}"
    ));
    assert!(!contract.contains("inferred_write_frames"));
    assert!(!contract.contains("remaining"));
    assert!(json[implementation_start..].contains(
        "\"inferred_write_frames\": [\n          {\"state\": \"entry\", \"completeness\": \"opaque\""
    ));
    assert!(json[implementation_start..].contains("\"paths\": []"));
    assert!(json[implementation_start..].contains(
        "\"checked_crash_sites\": [\n          {\"state\": \"entry\", \"statement_ordinal\": 4, \"cause\": \"Abort\", \"path_guard_conjuncts\": [\"0x010900000000\"], \"path_guard_consequences\": [\"0x010401\"], \"guard_covering_buckets\": [1], \"covering_buckets\": [1], \"frontier_lower_bound\": [{\"kind\": \"established\""
    ));
    assert!(json[implementation_start..].contains(
        "\"checked_crash_calls\": [\n          {\"state\": \"entry\", \"statement_ordinal\": 7, \"call_ordinal\": 2, \"target_machine\": \"Worker::run\", \"target_callable_overload_identity\": \"named-callable(path(Worker::run)"
    ));
    assert!(json[implementation_start..].contains(
        "\"target_state\": \"entry\", \"target_contract_report_fingerprint\": \"0x0000000000001234\", \"path_guard_conjuncts\": [\"0x010401\"], \"path_guard_consequences\": [], \"surviving_buckets\": [{\"cause\": \"Trap\", \"alternative_guards\": [\"true\"]}]"
    ));
    assert!(json[implementation_start..].contains("\"statement_ordinal\": 8, \"call_ordinal\": 0"));
    assert!(
        json[implementation_start..].contains(
            "\"target_callable_overload_identity\": \"named-callable(path(Firmware::read)"
        )
    );
    assert!(json.contains("\"crash_contract_capsules\": [\n    {\"target_machine\":"));
    assert!(
        json.contains(
            "\"target_callable_overload_identity\": \"named-callable(path(Firmware::read)"
        )
    );
    assert!(json.contains("\"target_contract_report_fingerprint\": \"0x0000000000005678\""));
    assert!(
        json[implementation_start..]
            .contains("\"source\": {\"kind\": \"state_entry\"}, \"ordinal\": 0}]")
    );
    assert!(json[implementation_start..].contains("\"checked_may_suspend\": false"));
    assert!(json[implementation_start..].contains("\"checked_may_block\": true"));
    assert!(json[implementation_start..].contains("\"checked_service_reach\": [\"Readable\"]"));
    assert!(
        json[implementation_start..]
            .contains("\"checked_synchronous_invocations\": [\"parameter:0\"]")
    );
    assert!(json[implementation_start..].contains("\"kind\": \"terminates\""));
    assert!(json[implementation_start..].contains("\"subjects\": [\"remaining\"]"));
    assert!(json[implementation_start..].contains("\"view\": \"Nat::Descending\""));
}

#[test]
fn termination_manifest_distinguishes_private_derivation_from_public_omission() {
    let mut internal = String::new();
    push_termination_interface_json(&mut internal, &TerminationInterface::InternalDerived);
    assert_eq!(internal, "{\"interface\": \"internal_derived\"}");

    let mut omitted = String::new();
    push_termination_interface_json(
        &mut omitted,
        &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
    );
    assert_eq!(
        omitted,
        "{\"interface\": \"published\", \"guarantee\": {\"kind\": \"no_guarantee\"}}"
    );
}

fn specialization_coordinate_fixture() -> CheckedTrees {
    let template_symbol = SymbolHandle::from_arena_index(70);
    let clone_symbol = SymbolHandle::from_arena_index(71);
    let mut program = CheckedTrees::default();
    for (machine_symbol, state_symbol, name, fingerprint) in [
        (
            template_symbol,
            SymbolHandle::from_arena_index(72),
            "Template::run",
            0x1111,
        ),
        (
            clone_symbol,
            SymbolHandle::from_arena_index(73),
            "Template::run#clone",
            0x2222,
        ),
    ] {
        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated(name),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: state_symbol,
                name: Identifier::generated("entry"),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);
        push_behavior_contract(&mut program, machine_symbol, false, false);
        program
            .facts
            .contract_plans
            .machines
            .last_mut()
            .expect("specialization coordinate contract")
            .report_fingerprint = fingerprint;
    }
    program.typed.machine_specializations = vec![
        MachineSpecialization {
            template: template_symbol,
            instance: template_symbol,
            type_arguments: vec!["ReadableReuse".to_owned()],
            type_argument_identities: vec!["type(reuse)".to_owned()],
            report_fingerprint: 0xaaaa,
            ..Default::default()
        },
        MachineSpecialization {
            template: template_symbol,
            instance: clone_symbol,
            type_arguments: vec!["ReadableClone".to_owned()],
            type_argument_identities: vec!["type(clone)".to_owned()],
            report_fingerprint: 0xbbbb,
            ..Default::default()
        },
    ];
    program
}

#[test]
fn machine_contract_manifest_specialization_coordinates_accept_reuse_and_clone_in_order() {
    let program = specialization_coordinate_fixture();

    let json = machine_contract_manifest_json(&program);
    let reuse = json
        .find("\"instance\": \"Template::run\"")
        .expect("reused template specialization");
    let cloned = json
        .find("\"instance\": \"Template::run#clone\"")
        .expect("cloned specialization");

    assert!(reuse < cloned);
    assert!(
        json[reuse..cloned].contains("\"instance_report_fingerprint\": \"0x000000000000aaaa\"")
    );
    assert!(
        json[reuse..cloned]
            .contains("\"instance_contract_report_fingerprint\": \"0x0000000000001111\"")
    );
    assert!(json[cloned..].contains("\"instance_report_fingerprint\": \"0x000000000000bbbb\""));
    assert!(
        json[cloned..].contains("\"instance_contract_report_fingerprint\": \"0x0000000000002222\"")
    );
}

#[test]
fn machine_contract_manifest_specialization_coordinates_keep_rows_orthogonal() {
    let mut program = specialization_coordinate_fixture();
    let baseline = machine_contract_manifest_json(&program);
    program.typed.machine_specializations[1].type_arguments = vec!["ChangedDisplay".to_owned()];
    program.typed.machine_specializations[1].type_argument_identities =
        vec!["type(changed)".to_owned()];
    let changed = machine_contract_manifest_json(&program);
    let specialization_start = baseline.find("\"specializations\"").expect("section");
    let baseline_clone = baseline
        .find("\"instance\": \"Template::run#clone\"")
        .expect("baseline clone");
    let changed_clone = changed
        .find("\"instance\": \"Template::run#clone\"")
        .expect("changed clone");

    assert_eq!(
        &baseline[specialization_start..baseline_clone],
        &changed[specialization_start..changed_clone]
    );
    assert!(changed[changed_clone..].contains("\"type_arguments\": [\"ChangedDisplay\"]"));
    assert!(changed[changed_clone..].contains("\"type_argument_identities\": [\"type(changed)\"]"));
}

#[test]
#[should_panic(expected = "missing its exact typed template machine")]
fn machine_contract_manifest_specialization_coordinates_reject_missing_template() {
    let mut program = specialization_coordinate_fixture();
    program.typed.machine_specializations[0].template = SymbolHandle::invalid();
    machine_contract_manifest_json(&program);
}

#[test]
#[should_panic(expected = "duplicate exact typed template machines")]
fn machine_contract_manifest_specialization_coordinates_reject_duplicate_template() {
    let mut program = specialization_coordinate_fixture();
    let duplicate = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == program.machine_specializations[0].template)
        .expect("template machine")
        .clone();
    program.typed.push_machine(duplicate);
    machine_contract_manifest_json(&program);
}

#[test]
#[should_panic(expected = "missing its exact typed instance machine")]
fn machine_contract_manifest_specialization_coordinates_reject_missing_instance() {
    let mut program = specialization_coordinate_fixture();
    program.typed.machine_specializations[1].instance = SymbolHandle::invalid();
    machine_contract_manifest_json(&program);
}

#[test]
#[should_panic(expected = "duplicate exact typed instance machines")]
fn machine_contract_manifest_specialization_coordinates_reject_duplicate_instance() {
    let mut program = specialization_coordinate_fixture();
    let duplicate = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == program.machine_specializations[1].instance)
        .expect("instance machine")
        .clone();
    program.typed.push_machine(duplicate);
    machine_contract_manifest_json(&program);
}

#[test]
#[should_panic(expected = "duplicate exact instance rows")]
fn machine_contract_manifest_specialization_coordinates_reject_duplicate_instance_row() {
    let mut program = specialization_coordinate_fixture();
    let duplicate = program.machine_specializations[0].clone();
    program.typed.machine_specializations.push(duplicate);
    machine_contract_manifest_json(&program);
}

#[test]
fn machine_contract_manifest_records_specialization_trust_and_contract_ids() {
    let symbol = SymbolHandle::from_arena_index(3);
    let state_symbol = SymbolHandle::from_arena_index(4);
    let mut program = CheckedTrees::default();
    let mut machine = Machine {
        symbol,
        name: Identifier::generated("accepted_map"),
        supply_mode: MachineSupplyMode::AdmissionClaim,
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut machine,
        State {
            symbol: state_symbol,
            name: Identifier::generated("entry"),
            ..Default::default()
        },
    );
    program.typed.push_machine(machine);
    let argument_symbol = SymbolHandle::from_arena_index(8);
    let mut argument_machine = Machine {
        symbol: argument_symbol,
        name: Identifier::generated("selected_argument"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut argument_machine,
        State {
            symbol: SymbolHandle::from_arena_index(9),
            name: Identifier::generated("entry"),
            ..Default::default()
        },
    );
    program.typed.push_machine(argument_machine);
    push_behavior_contract(&mut program, argument_symbol, false, false);
    program
        .facts
        .contract_plans
        .machines
        .last_mut()
        .expect("selected argument contract fixture")
        .report_fingerprint = 0x2222;
    program
        .typed
        .machine_specializations
        .push(MachineSpecialization {
            template: symbol,
            instance: symbol,
            type_arguments: vec!["Card".to_owned()],
            const_arguments: vec!["1".to_owned()],
            type_argument_identities: vec!["named(name(Card))".to_owned()],
            const_argument_identities: vec!["named(name(1))".to_owned()],
            machine_arguments: vec![argument_symbol],
            conformance_arguments: Vec::new(),
            inferred_conformance_arguments: Vec::new(),
            conformance_applications: Vec::new(),
            template_contract_report_fingerprint: 0x1111,
            template_contract_commitment:
                psi_typed_trees::typed_trees::MachineTemplateCommitment::from_digest([0x11; 32]),
            accepted_template_commitment: Some("accepted_map".to_owned()),
            machine_argument_contract_report_fingerprints: vec![0x2222],
            conformance_argument_report_fingerprints: vec![0x4444, 0x5555],
            report_fingerprint: 0x3333,
            commitment: psi_typed_trees::typed_trees::MachineSpecializationCommitment::from_digest(
                [0x33; 32],
            ),
            ..Default::default()
        });
    push_behavior_contract(&mut program, symbol, false, false);
    program
        .facts
        .contract_plans
        .machines
        .last_mut()
        .expect("specialization contract fixture")
        .report_fingerprint = 0xaaaa;

    let json = machine_contract_manifest_json(&program);
    assert!(json.contains("\"template\": \"accepted_map\""));
    assert!(json.contains("\"accepted_template_commitment\": \"accepted_map\""));
    assert!(json.contains("\"template_contract_report_fingerprint\": \"0x0000000000001111\""));
    assert!(json.contains(&format!(
        "\"template_contract_commitment\": \"{}\"",
        "11".repeat(32)
    )));
    assert!(json.contains("\"type_arguments\": [\"Card\"]"));
    assert!(json.contains("\"const_arguments\": [\"1\"]"));
    assert!(json.contains("\"type_argument_identities\": [\"named(name(Card))\"]"));
    assert!(json.contains("\"const_argument_identities\": [\"named(name(1))\"]"));
    assert!(
        json.contains(
            "\"machine_argument_contract_report_fingerprints\": [\"0x0000000000002222\"]"
        )
    );
    assert!(json.contains(
        "\"conformance_argument_report_fingerprints\": [\"0x0000000000004444\", \"0x0000000000005555\"]"
    ));
    assert!(json.contains("\"instance_report_fingerprint\": \"0x0000000000003333\""));
    assert!(json.contains("\"instance_contract_report_fingerprint\": \"0x000000000000aaaa\""));
}

#[test]
#[should_panic(expected = "missing its exact machine contract row")]
fn specialization_manifest_fails_closed_without_exact_instance_contract() {
    let instance = Machine {
        symbol: SymbolHandle::from_arena_index(1),
        name: Identifier::generated("Missing::instance"),
        ..Default::default()
    };
    specialization_instance_contract_report_fingerprint(&CheckedTrees::default(), &instance);
}

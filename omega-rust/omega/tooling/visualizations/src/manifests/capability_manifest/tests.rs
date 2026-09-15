//! Capability manifest tests.

use super::{
    capability_manifest_json, capability_manifest_json_with_composition,
    capability_manifest_json_with_selection,
};
use checked_trees::{CheckedTrees, MachineContractPlan, MachineServiceReachRows};
use effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceProgressPremise,
    ServiceProgressSubject, ServiceSchema,
};
use flow_effects::InstallationReachRequirement;
use language_semantics::{
    BlockingInterface, BlockingPlan, ServiceReachId, ServiceReachInterface, ServiceReachRowId,
    ServiceReachRowTable, SuspensionInterface, SuspensionPlan, TerminationGuarantee,
};
use symbols::SymbolHandle;
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::signature::StateSignature;
use typed_trees::state::State;
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::TypeReferenceNode;

fn minimal_manifest_program() -> (CheckedTrees, SymbolHandle) {
    let machine_symbol = SymbolHandle::from_arena_index(30);
    let mut program = CheckedTrees::default();
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Application::launch"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut machine,
        State {
            symbol: SymbolHandle::from_arena_index(31),
            name: Identifier::generated("main"),
            ..Default::default()
        },
    );
    program.typed.push_machine(machine);
    program.facts.service_reaches.machines.append_to_span(
        &mut program.facts.service_reaches.root_machines,
        MachineServiceReachRows {
            machine: machine_symbol,
            inferred_transitive: ServiceReachRowTable::EMPTY_ROW,
            ..Default::default()
        },
    );
    program
        .facts
        .suspensions
        .machines
        .push(checked_trees::MachineSuspensionFact {
            machine: machine_symbol,
            plan: SuspensionPlan {
                interface: SuspensionInterface::InternalInferred,
                checked_may_suspend: false,
            },
        });
    program
        .facts
        .blocking
        .machines
        .push(checked_trees::MachineBlockingFact {
            machine: machine_symbol,
            plan: BlockingPlan {
                interface: BlockingInterface::InternalInferred,
                checked_may_block: false,
            },
        });
    (program, machine_symbol)
}

fn manifest_panic(program: &CheckedTrees) -> String {
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        capability_manifest_json(program, Some("Application::launch"))
    }))
    .expect_err("invalid manifest facts must fail closed");
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            panic
                .downcast_ref::<&str>()
                .map(|message| (*message).to_owned())
        })
        .expect("invariant panic has a string diagnostic")
}

fn selected_manifest_panic(
    program: &CheckedTrees,
    selected: &effects::SelectedProviderPlanFacts,
) -> String {
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        capability_manifest_json_with_selection(
            program,
            Some("Application::launch"),
            Some(selected),
        )
    }))
    .expect_err("invalid selected manifest facts must fail closed");
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            panic
                .downcast_ref::<&str>()
                .map(|message| (*message).to_owned())
        })
        .expect("invariant panic has a string diagnostic")
}

#[test]
fn executable_manifest_uses_normalized_split_behavior_axes() {
    let machine_symbol = SymbolHandle::from_arena_index(10);
    let state_symbol = SymbolHandle::from_arena_index(11);
    let mut program = CheckedTrees::default();

    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Application::launch"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut machine,
        State {
            symbol: state_symbol,
            name: Identifier::generated("main"),
            ..Default::default()
        },
    );
    program.typed.push_machine(machine);
    let mut fallback = Machine {
        symbol: SymbolHandle::from_arena_index(12),
        name: Identifier::generated("Main::main"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut fallback,
        State {
            symbol: SymbolHandle::from_arena_index(13),
            name: Identifier::generated("main"),
            ..Default::default()
        },
    );
    program.typed.push_machine(fallback);

    let services = &mut program.facts.service_reaches;
    let machine_control = services
        .services
        .intern(SymbolHandle::from_arena_index(20), "MachineControl");
    let port_io = services
        .services
        .intern(SymbolHandle::from_arena_index(21), "PortIo");
    let service_row = services.rows.intern(vec![machine_control, port_io]);
    services.machines.append_to_span(
        &mut services.root_machines,
        MachineServiceReachRows {
            machine: machine_symbol,
            dependency: flow_effects::ServiceReachDependency {
                concrete: ServiceReachRowTable::EMPTY_ROW,
                parameters: Default::default(),
            },
            interface: ServiceReachInterface::InternalInferred,
            published_ceiling: language_semantics::ServiceReachRowTable::EMPTY_ROW,
            inferred_direct: service_row,
            inferred_transitive: service_row,
            concrete_transitive: service_row,
            effective: service_row,
            concrete_effective: service_row,
            unresolved_installation_reaches: Vec::new(),
            states: Default::default(),
        },
    );

    program
        .facts
        .suspensions
        .machines
        .push(checked_trees::MachineSuspensionFact {
            machine: machine_symbol,
            plan: SuspensionPlan {
                interface: SuspensionInterface::InternalInferred,
                checked_may_suspend: true,
            },
        });
    program
        .facts
        .blocking
        .machines
        .push(checked_trees::MachineBlockingFact {
            machine: machine_symbol,
            plan: BlockingPlan {
                interface: BlockingInterface::InternalInferred,
                checked_may_block: false,
            },
        });
    program
        .facts
        .termination
        .machines
        .push(checked_trees::MachineTerminationFact {
            machine: machine_symbol,
            plan: language_semantics::MachineTerminationPlan {
                interface: language_semantics::TerminationInterface::Published(
                    TerminationGuarantee::NoGuarantee,
                ),
                ..Default::default()
            },
        });
    program
        .facts
        .contract_plans
        .machines
        .push(MachineContractPlan {
            machine: machine_symbol,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: 0,
            commitment: checked_trees::MachineContractCommitment::from_digest([0; 32]),
        });

    let json = capability_manifest_json(&program, Some("Application::launch"));
    assert!(json.contains("\"entry_machine\": \"Application::launch\""));
    assert!(json.contains("\"service_reach\": [\"MachineControl\", \"PortIo\"]"));
    assert!(json.contains("\"may_suspend\": true"));
    assert!(json.contains("\"may_block\": false"));
    assert!(!json.contains("\"effect_bits\""));
    assert!(!json.contains("\"effects\""));
    let missing = capability_manifest_json(&program, None);
    assert!(missing.contains("\"entry_machine\": \"<missing>\""));
    assert!(missing.contains("\"entry_state\": \"<missing>\""));
    assert!(!missing.contains("\"entry_machine\": \"Main::main\""));
}

#[test]
fn executable_manifest_preserves_explicit_empty_and_negative_axes() {
    let (program, _) = minimal_manifest_program();
    let json = capability_manifest_json(&program, Some("Application::launch"));

    assert!(json.contains("\"service_reach\": []"));
    assert!(json.contains("\"may_suspend\": false"));
    assert!(json.contains("\"may_block\": false"));
}

#[test]
fn executable_manifest_renders_canonical_build_bound_progress_rows() {
    let (program, _) = minimal_manifest_program();
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x5a; 32])
        .expect("nonzero package identity");
    let provider = ProviderPlan {
        name: "scheduler".into(),
        provider_type: "SchedulerProvider".into(),
        provider_type_package_identity: Some(package),
        target: "test".into(),
        schema: ServiceSchema {
            trait_name: "Scheduler".into(),
            trait_package_identity: Some(package),
            methods: vec![ServiceMethod {
                name: "wait".into(),
                requirement_owner: "Scheduler".into(),
                requirement_owner_package_identity: Some(package),
                requirement_identity: "Scheduler::wait#exact".into(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec!["Scheduler".into()],
                synchronous_invocations: Vec::new(),
                may_suspend: true,
                may_block: false,
                terminates_guarantee: true,
                termination_premises: vec![ServiceProgressPremise {
                    profile: "WeakFair".into(),
                    subject: ServiceProgressSubject::ProviderReceiver,
                    subject_projections: Vec::new(),
                    establishment_routes: vec![
                        effects::provider_plan::ServiceProgressEstablishmentRoute {
                            kind: effects::provider_plan::ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                            requirement_identity: "SchedulerAdmission::grant#exact".into(),
                        },
                    ],
                }],
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "wait".into(),
            requirement_identity: "Scheduler::wait#exact".into(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CheckedAdapter {
                machine_identity: "SchedulerProvider::wait".into(),
                machine_package_identity: Some(package),
            },
        }],
        origin_package_identity: Some(package),
        origin_package: "test".into(),
    };
    let selected =
        effects::SelectedProviderPlanFacts::from_selection(&[provider], &["scheduler".into()])
            .expect("selected provider");
    let component = effects::ComponentProgressManifest::bind(
        "Application::launch#exact".into(),
        &selected,
        vec![effects::CheckedComponentProgressDemand {
            provider_service_identity: "Scheduler".into(),
            provider_service_package_identity: Some(package),
            requirement_identity: "Scheduler::wait#exact".into(),
            requirement_owner_package_identity: Some(package),
            profile_identity: "WeakFair".into(),
            subject_projections: Vec::new(),
            origin_callable_identity: "Application::launch#exact".into(),
            origin_state_identity: "Application::launch::main".into(),
            statement_ordinal: 2,
            call_ordinal: 1,
        }],
    )
    .expect("canonical component manifest");

    let json = capability_manifest_json_with_composition(
        &program,
        Some("Application::launch"),
        Some(&selected),
        Some(&component),
    );
    assert!(json.contains("\"provider_service\": \"Scheduler\""));
    assert!(json.contains(&format!(
        "\"provider_service_package\": \"{}\"",
        "5a".repeat(32)
    )));
    assert!(json.contains("\"requirement\": \"Scheduler::wait#exact\""));
    assert!(json.contains(&format!(
        "\"requirement_owner_package\": \"{}\"",
        "5a".repeat(32)
    )));
    assert!(json.contains("\"profile\": \"WeakFair\""));
    assert!(json.contains("\"authorized_establishment_routes\""));
    assert!(json.contains("\"requirement\": \"SchedulerAdmission::grant#exact\""));
    assert!(json.contains("\"provider_plan_report_identity\""));
    assert!(!json.contains("\"provider_plan_identity\""));
    assert!(!json.contains("\"provider_plan_report_fingerprint\""));
    assert!(json.contains("\"component_progress_status\": \"pending\""));
    assert!(json.contains("\"component_progress_manifest_report_identity\""));
    assert!(!json.contains("\"component_progress_manifest_identity\""));
}

#[test]
fn executable_manifest_publishes_unresolved_installation_reach_bounds() {
    let (mut program, machine) = minimal_manifest_program();
    let requirement_symbol = SymbolHandle::from_arena_index(41);
    let unit = program
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Unit);
    let mut controller = TraitDefinition {
        symbol: SymbolHandle::from_arena_index(40),
        is_boundary: true,
        name: Identifier::generated("InterruptCompletion"),
        ..Default::default()
    };
    program.typed.push_trait_machine_signature(
        &mut controller,
        StateSignature {
            symbol: requirement_symbol,
            name: Identifier::generated("complete"),
            return_type: unit,
            ..Default::default()
        },
    );
    program.typed.push_trait_definition(controller);
    let requirement_identity = {
        let owner = program
            .typed
            .traits()
            .iter()
            .find(|owner| owner.symbol == SymbolHandle::from_arena_index(40))
            .expect("controller trait exists");
        let requirement = program
            .typed
            .trait_machine_signatures(owner)
            .first()
            .expect("completion requirement exists");
        program
            .typed
            .normalized_trait_requirement_overload_identity(owner, requirement)
            .identity()
    };

    program.facts.service_reaches = Default::default();
    let reaches = &mut program.facts.service_reaches;
    let machine_control = reaches
        .services
        .intern(SymbolHandle::from_arena_index(42), "MachineControl");
    let port_io = reaches
        .services
        .intern(SymbolHandle::from_arena_index(43), "PortIo");
    let bound = reaches.rows.intern(vec![machine_control, port_io]);
    reaches.machines.append_to_span(
        &mut reaches.root_machines,
        MachineServiceReachRows {
            machine,
            inferred_transitive: bound,
            unresolved_installation_reaches: vec![InstallationReachRequirement {
                requirement: requirement_symbol,
                upper_bound: bound,
            }],
            ..Default::default()
        },
    );

    let json = capability_manifest_json(&program, Some("Application::launch"));
    assert!(
        json.contains("InterruptCompletion::complete"),
        "manifest omitted exact requirement identity:\n{json}"
    );
    assert!(
        json.contains("\"upper_bound\": [\"MachineControl\", \"PortIo\"]"),
        "manifest omitted installation-bound ceiling:\n{json}"
    );
    let provider = ProviderPlan {
        name: "pic".into(),
        provider_type: "LegacyPic".into(),
        provider_type_package_identity: None,
        target: "test-target".into(),
        schema: ServiceSchema {
            trait_name: "InterruptCompletion".into(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "complete".into(),
                requirement_owner: "InterruptCompletion".into(),
                requirement_owner_package_identity: None,
                requirement_identity: requirement_identity.clone(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec!["PortIo".into()],
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "complete".into(),
            requirement_identity: requirement_identity.clone(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CompilerIntrinsic {
                machine: "LegacyPic::complete".into(),
            },
        }],
        origin_package_identity: None,
        origin_package: "test".into(),
    };
    let provider_identity = provider.report_fingerprint();
    let selected_without_reach =
        effects::SelectedProviderPlanFacts::from_selection(&[provider], &["pic".into()])
            .expect("provider selection is valid");
    assert!(
        selected_manifest_panic(&program, &selected_without_reach)
            .contains("does not resolve installation-bound requirement")
    );
    let selected = selected_without_reach
        .with_installation_reach_resolutions(vec![effects::InstallationReachResolution {
            requirement_identity,
            provider_plan_report_identity: provider_identity,
            upper_bound: vec!["MachineControl".into(), "PortIo".into()],
            resolved_row: vec!["PortIo".into()],
        }])
        .expect("installation reach resolves inside its bound");
    let selected_json = capability_manifest_json_with_selection(
        &program,
        Some("Application::launch"),
        Some(&selected),
    );
    assert!(selected_json.contains("\"selected_row\": [\"PortIo\"]"));
    assert!(selected_json.contains(&format!("{provider_identity:#018x}")));
}

#[test]
fn executable_manifest_rejects_unresolved_reach_without_exact_requirement() {
    let (mut program, machine) = minimal_manifest_program();
    let reaches = &mut program.facts.service_reaches;
    reaches.machines = Default::default();
    reaches.root_machines = Default::default();
    reaches.machines.append_to_span(
        &mut reaches.root_machines,
        MachineServiceReachRows {
            machine,
            inferred_transitive: ServiceReachRowTable::EMPTY_ROW,
            unresolved_installation_reaches: vec![InstallationReachRequirement {
                requirement: SymbolHandle::from_arena_index(99),
                upper_bound: ServiceReachRowTable::EMPTY_ROW,
            }],
            ..Default::default()
        },
    );

    assert!(manifest_panic(&program).contains("resolves to 0 typed declarations"));
}

#[test]
fn executable_manifest_rejects_missing_and_duplicate_service_reach() {
    let (mut missing, _) = minimal_manifest_program();
    missing.facts.service_reaches = Default::default();
    assert!(manifest_panic(&missing).contains("no service-reach row"));

    let (mut duplicate, machine) = minimal_manifest_program();
    duplicate.facts.service_reaches.machines.append_to_span(
        &mut duplicate.facts.service_reaches.root_machines,
        MachineServiceReachRows {
            machine,
            inferred_transitive: ServiceReachRowTable::EMPTY_ROW,
            ..Default::default()
        },
    );
    assert!(manifest_panic(&duplicate).contains("duplicate service-reach rows"));
}

#[test]
fn executable_manifest_rejects_missing_and_duplicate_suspension() {
    let (mut missing, _) = minimal_manifest_program();
    missing.facts.suspensions.machines.clear();
    assert!(manifest_panic(&missing).contains("no suspension row"));

    let (mut duplicate, machine) = minimal_manifest_program();
    duplicate
        .facts
        .suspensions
        .machines
        .push(checked_trees::MachineSuspensionFact {
            machine,
            plan: SuspensionPlan {
                interface: SuspensionInterface::PublishedMaySuspend(true),
                checked_may_suspend: true,
            },
        });
    assert!(manifest_panic(&duplicate).contains("duplicate suspension rows"));
}

#[test]
fn executable_manifest_rejects_missing_and_duplicate_blocking() {
    let (mut missing, _) = minimal_manifest_program();
    missing.facts.blocking.machines.clear();
    assert!(manifest_panic(&missing).contains("no blocking row"));

    let (mut duplicate, machine) = minimal_manifest_program();
    duplicate
        .facts
        .blocking
        .machines
        .push(checked_trees::MachineBlockingFact {
            machine,
            plan: BlockingPlan {
                interface: BlockingInterface::PublishedMayBlock(true),
                checked_may_block: true,
            },
        });
    assert!(manifest_panic(&duplicate).contains("duplicate blocking rows"));
}

#[test]
fn executable_manifest_rejects_noncanonical_and_unregistered_service_rows() {
    let (mut noncanonical, machine) = minimal_manifest_program();
    noncanonical.facts.service_reaches = Default::default();
    noncanonical.facts.service_reaches.machines.append_to_span(
        &mut noncanonical.facts.service_reaches.root_machines,
        MachineServiceReachRows {
            machine,
            inferred_transitive: ServiceReachRowId(99),
            ..Default::default()
        },
    );
    assert!(manifest_panic(&noncanonical).contains("noncanonical inferred service-reach row"));

    let (mut unregistered, machine) = minimal_manifest_program();
    unregistered.facts.service_reaches = Default::default();
    let row = unregistered
        .facts
        .service_reaches
        .rows
        .intern(vec![ServiceReachId(99)]);
    unregistered.facts.service_reaches.machines.append_to_span(
        &mut unregistered.facts.service_reaches.root_machines,
        MachineServiceReachRows {
            machine,
            inferred_transitive: row,
            ..Default::default()
        },
    );
    assert!(manifest_panic(&unregistered).contains("unregistered service"));
}

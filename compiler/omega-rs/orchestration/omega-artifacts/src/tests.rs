//! Artifact construction and projection regression tests.

use std::collections::BTreeSet;

use omega_calling_conventions::{
    CallSignature, CallingPolicy, EntryStack, MachineRegister, MachineState, MachineStateSet,
    ProviderExitRealization, RegisterSet, StateFootprintEvidence, ValueShape,
    evaluate_ordinary_boundary_entry_plan,
};
use omega_executable_installation::{
    Artifact, ArtifactContentId, ArtifactEntry, ArtifactId, ContainerLimits,
    DecodedArtifactContainer, EntrySetId, InstallationDiagnostic, InstalledCodeId,
    MachineContractSetId, MachineFootprintId, PlacementPlanId, RelocationSetId,
    decode_executable_container, normalized_decoded_content_identity,
};
use omega_external_roots::{
    AcknowledgementPolicyId, ComponentArtifactId, ComponentContractId, ComponentProviderId,
    ComponentVersionPin, ComponentVersionPinId, ExternalRootDiagnostic, ExternalRootId,
    FixedFuelCall, FixedFuelProviderSummary, FuelProvisionId, FuelScheduleIdentity,
    FuelValidationReceiptId, InstalledRootRecord, LogicalFuelResourceColumn,
    MachineStateResourceColumn, NativeFuelRealizationKind, NestingRelationId,
    OpaqueProviderExitAssurance, ProviderExecutionId, ProviderFuelSummaryId,
    ProviderFuelValidationReceiptId, ProviderPlanId, ProviderStackSummary, RootAdmissionId,
    RootEffectId, RootProviderId, RootSlotId, RootSlotOwnerId, StackNestingRelation,
    StackResourceColumn, StackValidationReceiptId, StateValidationReceiptId, TrustReceiptId,
    compose_artifact_stacks, compose_fixed_fuel,
};
use omega_target::Architecture;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::machine::Machine;
use psi_checked_trees::name::Identifier;
use psi_checked_trees::state::State;
use psi_layout_plans::{EntryStubId, PlacementConstraints, PlacementPhase};
use psi_symbols::SymbolHandle;

use super::external_root_report::external_root_records_manifest_json;
use super::{
    ArtifactWriter, TrustCrashCause, TrustCrashRouteBucket, TrustCrashRouteGuard,
    TrustGenericAcceptedInstanceRow, TrustProviderRealization, TrustProviderRequirementRow,
    TrustQualificationRow, TrustReport, TrustReportRow, build_backend_surface_report,
    value_placement_json,
};

#[test]
fn accepted_machine_service_reach_distinguishes_public_empty_from_non_machine_rows() {
    let root = std::env::temp_dir().join(format!(
        "omega-trust-accepted-reach-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let writer = ArtifactWriter::new(&root).expect("artifact writer");
    let report = TrustReport {
        rows: vec![
            TrustReportRow {
                commitment: "accepted fact: quiet_axiom".to_owned(),
                provenance: "root grant (build.omg)".to_owned(),
                machine_contract_fingerprint: Some(0xabcd),
                machine_template_fingerprint: None,
                machine_service_reach: Some(Vec::new()),
                machine_synchronous_invocations: Some(Vec::new()),
                machine_may_suspend: Some(false),
                machine_may_block: Some(false),
                machine_terminates_guarantee: Some(false),
                machine_crash_routes: Some(Vec::new()),
                standing_warning: false,
            },
            TrustReportRow {
                commitment: "domain introduction: Meters".to_owned(),
                provenance: "root grant (build.omg)".to_owned(),
                machine_contract_fingerprint: None,
                machine_template_fingerprint: None,
                machine_service_reach: None,
                machine_synchronous_invocations: None,
                machine_may_suspend: None,
                machine_may_block: None,
                machine_terminates_guarantee: None,
                machine_crash_routes: None,
                standing_warning: false,
            },
            TrustReportRow {
                commitment: "accepted fact: guarded_axiom".to_owned(),
                provenance: "root grant (build.omg)".to_owned(),
                machine_contract_fingerprint: Some(0xbcde),
                machine_template_fingerprint: None,
                machine_service_reach: Some(Vec::new()),
                machine_synchronous_invocations: Some(Vec::new()),
                machine_may_suspend: Some(false),
                machine_may_block: Some(false),
                machine_terminates_guarantee: Some(false),
                machine_crash_routes: Some(vec![
                    TrustCrashRouteBucket {
                        cause: TrustCrashCause::Trap,
                        alternative_guards: vec![
                            TrustCrashRouteGuard::PredicateIdentity(vec![1]),
                            TrustCrashRouteGuard::PredicateIdentity(vec![2]),
                        ],
                    },
                    TrustCrashRouteBucket {
                        cause: TrustCrashCause::Abort,
                        alternative_guards: vec![TrustCrashRouteGuard::Truth],
                    },
                ]),
                standing_warning: false,
            },
        ],
        generic_accepted_instances: vec![TrustGenericAcceptedInstanceRow {
            template_commitment: "admitted".to_owned(),
            template_fingerprint: 0x1111,
            instance_fingerprint: 0x2222,
            instance_contract_fingerprint: 0xaaaa,
            type_argument_identities: vec!["named(name(Card))".to_owned()],
            const_argument_identities: vec!["named(name(1))".to_owned()],
            machine_argument_contract_fingerprints: vec![0x3333],
            conformance_argument_fingerprints: vec![0x4444],
        }],
        ..Default::default()
    };

    writer
        .write_trust_report(&report)
        .expect("trust report output");
    let output =
        std::fs::read_to_string(root.join("trust_report.md")).expect("written trust report");
    let accepted = output
        .lines()
        .find(|line| line.contains("accepted fact: quiet_axiom"))
        .expect("accepted fact row");
    let domain = output
        .lines()
        .find(|line| line.contains("domain introduction: Meters"))
        .expect("domain row");
    let guarded = output
        .lines()
        .find(|line| line.contains("accepted fact: guarded_axiom"))
        .expect("guarded accepted fact row");

    assert!(accepted.contains("service reach: none"));
    assert!(accepted.contains("synchronous invocations: none"));
    assert!(accepted.contains("may suspend: no"));
    assert!(accepted.contains("may block: no"));
    assert!(accepted.contains("termination guarantee: no"));
    assert!(accepted.contains("crash routes: none"));
    assert!(guarded.contains("crash routes: Trap[0x01 | 0x02], Abort[true]"));
    assert!(output.contains("accepted template: admitted [0000000000001111]"));
    assert!(output.contains("instance: 0000000000002222"));
    assert!(output.contains("instance contract: 000000000000aaaa"));
    assert!(output.contains("type argument identities: named(name(Card))"));
    assert!(output.contains("const argument identities: named(name(1))"));
    assert!(output.contains("machine argument contracts: 0000000000003333"));
    assert!(output.contains("conformance arguments: 0000000000004444"));
    assert!(!domain.contains("service reach:"));
    assert!(!domain.contains("synchronous invocations:"));
    assert!(!domain.contains("may suspend:"));
    assert!(!domain.contains("may block:"));
    assert!(!domain.contains("termination guarantee:"));
    assert!(!domain.contains("crash routes:"));
    std::fs::remove_dir_all(root).expect("remove test artifact directory");
}

#[test]
fn trust_report_keeps_inherited_requirement_owner_separate_from_overload_identity() {
    let root = std::env::temp_dir().join(format!(
        "omega-trust-owner-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let writer = ArtifactWriter::new(&root).expect("artifact writer");
    let report = TrustReport {
        selected_provider_closure_fingerprint: 0xabcd,
        rows: Vec::new(),
        generic_accepted_instances: Vec::new(),
        provider_requirements: Vec::new(),
        qualifications: vec![TrustQualificationRow {
            provider_plan: "RootProvider::satisfies::Root".to_owned(),
            provider_plan_fingerprint: 0x1234,
            provider_type: "RootProvider".to_owned(),
            target: "windows_x64".to_owned(),
            provider_origin_package: "omega::providers::root".to_owned(),
            service_schema: "Root".to_owned(),
            calling_plan_fingerprint: Some(0xfeed),
            selected: false,
            requirement_owner: "Base".to_owned(),
            requirement_identity: "named-callable(path(Base::enter), parameters(), result(none))"
                .to_owned(),
            method: "enter".to_owned(),
            subject: "parameter:0".to_owned(),
            authority_flow: "accepts".to_owned(),
            domain: "Token::Granted".to_owned(),
            effective_carry: "strict".to_owned(),
            predicate_discharge_required: false,
            provenance: "own-package (dev-active)".to_owned(),
            grant_selectors: Vec::new(),
            standing_warning: true,
        }],
    };

    writer
        .write_trust_report(&report)
        .expect("trust report output");
    let output =
        std::fs::read_to_string(root.join("trust_report.md")).expect("written trust report");

    assert!(output.contains("requirement owner: Base"));
    assert!(output.contains("selected provider closure: 000000000000abcd"));
    assert!(output.contains("provider type: RootProvider"));
    assert!(output.contains("target: windows_x64"));
    assert!(output.contains("provider origin package: omega::providers::root"));
    assert!(output.contains("own-package (dev-active)"));
    assert!(output.contains("service schema: Root"));
    assert!(output.contains("calling plan: 000000000000feed"));
    assert!(output.contains("selected: no"));
    assert!(output.contains("requirement identity: named-callable(path(Base::enter)"));
    assert!(!output.contains("requirement owner: Root"));
    std::fs::remove_dir_all(root).expect("remove test artifact directory");
}

#[test]
fn trust_report_keeps_claim_free_provider_requirement_blast_radius_exact() {
    let root = std::env::temp_dir().join(format!(
        "omega-trust-provider-requirement-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let writer = ArtifactWriter::new(&root).expect("artifact writer");
    let report = TrustReport {
        selected_provider_closure_fingerprint: 0x5678,
        rows: Vec::new(),
        generic_accepted_instances: Vec::new(),
        provider_requirements: vec![TrustProviderRequirementRow {
            provider_plan: "RootProvider::satisfies::Root".to_owned(),
            provider_plan_fingerprint: 0x1234,
            provider_type: String::new(),
            target: String::new(),
            provider_origin_package: String::new(),
            service_schema: "Root".to_owned(),
            calling_plan_fingerprint: None,
            selected: true,
            requirement_owner: "Base".to_owned(),
            requirement_identity: "named-callable(path(Base::enter), parameters(), result(none))"
                .to_owned(),
            method: "enter".to_owned(),
            parameter_type_identities: vec!["Token in Granted".to_owned()],
            result_type_identity: Some("Token".to_owned()),
            service_reach: vec!["Clock".to_owned(), "Storage".to_owned()],
            synchronous_invocations: vec!["Callback".to_owned()],
            may_suspend: true,
            may_block: false,
            terminates_guarantee: true,
            realization: TrustProviderRealization::VtableSlot { index: 4 },
            provenance: "root grant (build.omg)".to_owned(),
            grant_selectors: vec!["Root".to_owned()],
            standing_warning: false,
        }],
        qualifications: Vec::new(),
    };

    writer
        .write_trust_report(&report)
        .expect("trust report output");
    let output =
        std::fs::read_to_string(root.join("trust_report.md")).expect("written trust report");

    assert!(output.contains("provider requirements: 1"));
    assert!(output.contains("selected provider closure: 0000000000005678"));
    assert!(output.contains("provider plan: RootProvider::satisfies::Root [0000000000001234]"));
    assert!(output.contains("provider type: <free external>"));
    assert!(output.contains("target: <all>"));
    assert!(output.contains("provider origin package: <none>"));
    assert!(output.contains("service schema: Root"));
    assert!(output.contains("calling plan: <none>"));
    assert!(output.contains("selected: yes"));
    assert!(output.contains("requirement owner: Base"));
    assert!(output.contains("requirement identity: named-callable(path(Base::enter)"));
    assert!(output.contains("method: enter"));
    assert!(output.contains("parameter types: Token in Granted"));
    assert!(output.contains("result type: Token"));
    assert!(output.contains("service reach: Clock, Storage"));
    assert!(output.contains("synchronous invocations: Callback"));
    assert!(output.contains("may suspend: yes"));
    assert!(output.contains("may block: no"));
    assert!(output.contains("termination guarantee: yes"));
    assert!(output.contains("realization: vtable slot 4"));
    assert!(output.contains("root grant (build.omg)"));
    assert!(output.contains("grant selectors: Root"));
    assert!(!output.contains("requirement owner: Root"));
    assert!(!output.contains("STANDING WARNING"));
    std::fs::remove_dir_all(root).expect("remove test artifact directory");
}

#[test]
fn trust_provider_realizations_distinguish_checked_and_opaque_leaves() {
    assert_eq!(
        TrustProviderRealization::CheckedAdapter {
            machine: "ConsoleProvider::write".to_owned(),
        }
        .report_text(),
        "checked adapter `ConsoleProvider::write`"
    );
    assert_eq!(
        TrustProviderRealization::Syscall { number: 60 }.report_text(),
        "syscall 60"
    );
}

#[test]
fn value_placement_json_retains_indirect_copy_geometry() {
    let boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![ValueShape::integer(16, 8)],
            result: None,
        },
    )
    .expect("Microsoft x64 aggregate placement");
    let json = value_placement_json(&boundary.plan().call.parameters[0]);

    assert!(json.contains("\"indirect\""));
    assert!(json.contains("\"copy_stack_byte_offset\": 32"));
    assert!(json.contains("\"byte_size\": 16"));
}

fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(identity).expect("normalized root identity")
}

fn fuel_schedule() -> FuelScheduleIdentity {
    FuelScheduleIdentity::new(1).expect("canonical test fuel schedule")
}

fn install_id<T>(identity: u64, constructor: fn(u64) -> Result<T, InstallationDiagnostic>) -> T {
    constructor(identity).expect("normalized installation identity")
}

fn entry_id(identity: u64) -> EntryStubId {
    EntryStubId::from_normalized_identity(identity).expect("normalized entry identity")
}

fn executable_container_fixture() -> Artifact {
    let artifact_id = install_id(900, ArtifactId::from_normalized_identity);
    let contracts = install_id(901, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(902, MachineFootprintId::from_normalized_identity);
    let placement_plan = install_id(903, PlacementPlanId::from_normalized_identity);
    let entry_set = install_id(904, EntrySetId::from_normalized_identity);
    let relocation_set = install_id(905, RelocationSetId::from_normalized_identity);
    let code = vec![0xc3];
    let entries = vec![ArtifactEntry::from_canonical_decode(entry_id(906), 0)];
    let placement_constraints =
        PlacementConstraints::new(None, 1, PlacementPhase::Load, None, None)
            .expect("placement constraints");
    let decoded = DecodedArtifactContainer {
        format_marker: omega_executable_installation::OMEGA_EXECUTABLE_CONTAINER_MARKER,
        total_length: 1,
        artifact: artifact_id,
        content: install_id(907, ArtifactContentId::from_normalized_identity),
        architecture: Architecture::X86_64,
        code_length: code.len() as u64,
        code: code.clone(),
        contracts,
        declared_footprint: footprint,
        placement_plan,
        placement_constraints,
        entry_set,
        entries: entries.clone(),
        relocation_set,
        relocations: Vec::new(),
        proof_payload: omega_executable_installation::normalized_proof_payload_identity(b""),
        proof: Vec::new(),
        sections: Vec::new(),
    };
    let content =
        normalized_decoded_content_identity(&decoded).expect("normalized content identity");
    Artifact::from_canonical_decode(
        artifact_id,
        content,
        Architecture::X86_64,
        code,
        contracts,
        footprint,
        placement_plan,
        placement_constraints,
        entry_set,
        entries,
        relocation_set,
        Vec::new(),
    )
    .expect("canonical artifact")
}

#[test]
fn writes_canonical_executable_container_atomically() {
    let root = std::env::temp_dir().join(format!(
        "omega-artifact-container-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let writer = ArtifactWriter::new(&root).expect("artifact writer");
    let limits = ContainerLimits {
        max_total_bytes: 64 * 1024,
        max_sections: 16,
        max_section_bytes: 32 * 1024,
        max_relocations: 64,
    };
    let artifact = executable_container_fixture();

    let path = writer
        .write_executable_container("program.omega-artifact", &artifact, b"proof", limits)
        .expect("canonical artifact output");
    let bytes = std::fs::read(&path).expect("written artifact bytes");
    let decoded =
        decode_executable_container(&bytes, limits).expect("written bytes remain canonical");

    assert_eq!(decoded.artifact(), &artifact);
    assert_eq!(decoded.proof(), b"proof");
    assert!(!root.join(".program.omega-artifact.tmp").exists());
    std::fs::remove_dir_all(root).expect("remove test artifact directory");
}

#[test]
fn missing_entry_selection_does_not_infer_main_machine() {
    let mut program = CheckedTrees::default();
    let mut machine = Machine {
        symbol: SymbolHandle::default(),
        name: Identifier::generated("Main::main"),
        attached_data: None,
        owned_data: Default::default(),
        satisfies: Default::default(),
        states: Default::default(),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut machine,
        State {
            symbol: SymbolHandle::default(),
            name: Identifier::generated("main"),
            parameters: Default::default(),
            return_type: Default::default(),
            ..Default::default()
        },
    );
    program.typed.push_machine(machine);

    let report = build_backend_surface_report(&program, None);

    assert!(report.entry_points.is_empty());
    assert_eq!(report.machines.len(), 1);
}

#[test]
fn explicit_entry_selection_controls_surface_report() {
    let mut program = CheckedTrees::default();
    for (machine_name, state_name) in [("Main::main", "main"), ("Application::launch", "start")] {
        let mut machine = Machine {
            name: Identifier::generated(machine_name),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                name: Identifier::generated(state_name),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);
    }

    let report = build_backend_surface_report(&program, Some("Application::launch"));
    let entries = report
        .entry_points
        .iter()
        .map(|(_, entry)| (entry.machine.as_str(), entry.state.as_str()))
        .collect::<Vec<_>>();

    assert_eq!(entries, [("Application::launch", "start")]);
}

#[test]
fn counts_contained_machines_from_attached_data_fields() {
    let worker_data_symbol = SymbolHandle::from_arena_index(1);
    let main_data_symbol = SymbolHandle::from_arena_index(2);
    let worker_machine_symbol = SymbolHandle::from_arena_index(3);
    let main_machine_symbol = SymbolHandle::from_arena_index(4);
    let worker_field_symbol = SymbolHandle::from_arena_index(5);
    let mut program = CheckedTrees::default();
    let worker_type = program.typed.type_reference_table.insert(
        psi_checked_trees::types::TypeReferenceNode::Named {
            symbol: worker_data_symbol,
            name: Identifier::generated("Worker"),
        },
    );

    program
        .typed
        .push_data_definition(psi_checked_trees::data::DataDefinition {
            symbol: worker_data_symbol,
            name: Identifier::generated("Worker"),
            ..Default::default()
        });
    let mut main_data = psi_checked_trees::data::DataDefinition {
        symbol: main_data_symbol,
        name: Identifier::generated("Main"),
        ..Default::default()
    };
    program.typed.push_data_member(
        &mut main_data,
        psi_checked_trees::data::DataMember::Field(psi_checked_trees::data::DataField {
            identity: None,
            symbol: worker_field_symbol,
            name: Identifier::generated("worker"),
            relevance: Default::default(),
            type_reference: worker_type,
        }),
    );
    program.typed.push_data_definition(main_data);
    program.typed.push_machine(Machine {
        symbol: worker_machine_symbol,
        name: Identifier::generated("Worker::run"),
        attached_data: Some(Identifier::generated("Worker")),
        ..Default::default()
    });
    program.typed.push_machine(Machine {
        symbol: main_machine_symbol,
        name: Identifier::generated("Main::main"),
        attached_data: Some(Identifier::generated("Main")),
        ..Default::default()
    });
    let targets = program.facts.carry.contained_targets.insert_many([
        psi_checked_trees::ContainedMachineTargetFact {
            machine: worker_machine_symbol,
        },
    ]);
    let fields = program.facts.carry.contained_fields.insert_many([
        psi_checked_trees::ContainedMachineFieldFact {
            field: worker_field_symbol,
            data: worker_data_symbol,
            type_reference: worker_type,
            targets,
        },
    ]);
    program
        .facts
        .carry
        .machine_topologies
        .insert(psi_checked_trees::MachineCarryTopologyFact {
            machine: worker_machine_symbol,
            fields: psi_arena::HandleSpan::empty(),
        });
    program
        .facts
        .carry
        .machine_topologies
        .insert(psi_checked_trees::MachineCarryTopologyFact {
            machine: main_machine_symbol,
            fields,
        });

    let report = build_backend_surface_report(&program, None);
    let main = report
        .machines
        .iter()
        .find_map(|(_, machine)| (machine.name == "Main::main").then_some(machine))
        .expect("main machine surface");

    assert_eq!(main.contained_machines, 1);
}

#[test]
fn external_root_manifest_is_complete_normalized_and_address_free() {
    let boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("boundary plan");
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        root_id(21, ProviderFuelSummaryId::from_normalized_identity),
        root_id(22, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        4,
        BTreeSet::new(),
        root_id(
            23,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let work_root = FixedFuelProviderSummary::from_admitted_provider(
        root_id(20, ProviderFuelSummaryId::from_normalized_identity),
        root_id(8, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        3,
        BTreeSet::from([FixedFuelCall {
            callee: leaf.identity,
            maximum_invocations: 2,
        }]),
        root_id(
            24,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let composed_fuel =
        compose_fixed_fuel(work_root.identity, [&work_root, &leaf]).expect("fixed fuel");
    let root_identity = root_id(1, ExternalRootId::from_normalized_identity);
    let nesting_identity = root_id(11, NestingRelationId::from_normalized_identity);
    let stack_summary = ProviderStackSummary::from_admitted_provider(
        root_identity,
        root_id(8, RootProviderId::from_normalized_identity),
        EntryStack::ProviderSelected,
        2048,
        16,
        root_id(29, StackValidationReceiptId::from_normalized_identity),
    );
    let composed_stack = compose_artifact_stacks(
        &StackNestingRelation {
            identity: nesting_identity,
            edges: BTreeSet::new(),
        },
        [&stack_summary],
    )
    .expect("stack composition")
    .demand(root_identity)
    .expect("root stack demand")
    .clone();
    let record = InstalledRootRecord {
        root: root_identity,
        normalized_root_identity: 0x101,
        entry: entry_id(2),
        installed_code: install_id(3, InstalledCodeId::from_normalized_identity),
        artifact: install_id(4, ArtifactId::from_normalized_identity),
        slot: root_id(5, RootSlotId::from_normalized_identity),
        owner: root_id(6, RootSlotOwnerId::from_normalized_identity),
        admission: root_id(7, RootAdmissionId::from_normalized_identity),
        provider_execution: root_id(30, ProviderExecutionId::from_normalized_identity),
        provider_execution_fingerprint: 0x3030,
        provider_exit_assurance: OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: boundary.plan().call.entry_control,
                restored_state: boundary.plan().state.restored_state,
            },
            validation_receipt: root_id(10, TrustReceiptId::from_normalized_identity),
        },
        provider_exit_assurance_fingerprint: 0x3031,
        provider_plan: root_id(31, ProviderPlanId::from_normalized_identity),
        native_fuel_kind: NativeFuelRealizationKind::FixedProvision,
        native_fuel_fingerprint: 0x3032,
        requirement_identity: "TestRoot::entry".into(),
        entry_claims: Vec::new(),
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
        boundary_contract_fingerprint: boundary.contract_fingerprint(),
        boundary: boundary.plan().clone(),
        provider: root_id(8, RootProviderId::from_normalized_identity),
        effects: BTreeSet::from([root_id(9, RootEffectId::from_normalized_identity)]),
        trust_receipts: BTreeSet::from([root_id(10, TrustReceiptId::from_normalized_identity)]),
        nesting_relation: nesting_identity,
        acknowledgement_policy: Some(root_id(
            12,
            AcknowledgementPolicyId::from_normalized_identity,
        )),
        stack: StackResourceColumn {
            ceiling_bytes: 8192,
            realization: composed_stack,
            validation_receipt: root_id(25, StackValidationReceiptId::from_normalized_identity),
        },
        logical_fuel: LogicalFuelResourceColumn {
            schedule: fuel_schedule(),
            provision: root_id(28, FuelProvisionId::from_normalized_identity),
            ceiling_units: 64,
            realization: composed_fuel,
            validation_receipt: root_id(26, FuelValidationReceiptId::from_normalized_identity),
        },
        machine_state: MachineStateResourceColumn {
            realization: StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax]),
                MachineStateSet::new([MachineState::Flags]),
            ),
            validation_receipt: root_id(27, StateValidationReceiptId::from_normalized_identity),
        },
        component_pins: BTreeSet::from([ComponentVersionPin {
            contract: root_id(13, ComponentContractId::from_normalized_identity),
            artifact: root_id(14, ComponentArtifactId::from_normalized_identity),
            provider: root_id(15, ComponentProviderId::from_normalized_identity),
            version: root_id(16, ComponentVersionPinId::from_normalized_identity),
        }]),
    };

    let first = external_root_records_manifest_json(0x202, &[&record]);
    let second = external_root_records_manifest_json(0x202, &[&record]);
    let parsed: serde_json::Value = serde_json::from_str(&first).expect("valid JSON manifest");

    assert_eq!(first, second);
    assert_eq!(parsed["root_count"], 1);
    assert_eq!(
        parsed["roots"][0]["normalized_root_identity"],
        "0x0000000000000101"
    );
    assert_eq!(parsed["roots"][0]["entry"], "0x0000000000000002");
    assert_eq!(
        parsed["roots"][0]["provider_execution"],
        "0x000000000000001e"
    );
    assert_eq!(parsed["roots"][0]["provider_plan"], "0x000000000000001f");
    assert_eq!(parsed["roots"][0]["native_fuel"]["kind"], "fixed_provision");
    assert_eq!(
        parsed["roots"][0]["native_fuel"]["fingerprint"],
        "0x0000000000003032"
    );
    assert_eq!(
        parsed["roots"][0]["boundary_plan"]["call"]["policy"],
        "system_v_amd64"
    );
    assert_eq!(
        parsed["roots"][0]["boundary_plan"]["state"]["stack"],
        "provider_selected"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["composed_wcsu_bytes"],
        2048
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["origin"],
        "admitted_provider"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["composed_units"],
        11
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["schedule_marker"],
        1
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["provision"],
        "0x000000000000001c"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["summary_evidence"][0]["origin"],
        "admitted_provider"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["summary_evidence"][1]["origin"],
        "admitted_provider"
    );
    assert!(
        parsed["roots"][0]["resources"]
            .get("structural_work")
            .is_none()
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["machine_state"]["realized_registers"][0],
        "x86_rax"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["machine_state"]["ceiling"]["permitted_transitive_use_bits"],
        "0x0007"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["validation_receipt"],
        "0x0000000000000019"
    );
    assert_eq!(parsed["roots"][0]["effects"][0], "0x0000000000000009");
    assert_eq!(
        parsed["roots"][0]["component_pins"][0]["version"],
        "0x0000000000000010"
    );
    assert!(!first.contains("entry_address"));
    assert!(!first.contains("code_address"));
    assert!(!first.contains("ranking"));
    assert!(!first.contains("codegen"));
}

use super::*;

#[test]
fn carry_manifest_keeps_authored_and_effective_policies_separate() {
    let symbol = SymbolHandle::from_arena_index(7);
    let state_symbol = SymbolHandle::from_arena_index(9);
    let declared = CarryPolicy {
        suspension: CarrySuspension::Forbidden,
        cpu: CarryCpu::Origin,
        host_thread: CarryHostThread::Any,
        address: CarryAddress::Stable,
    };
    let mut program = CheckedTrees::default();
    program
        .typed
        .push_data_definition(typed_trees::data::DataDefinition {
            symbol,
            name: Identifier::generated("PerCpuLease"),
            ..Default::default()
        });
    program.facts.carry.data.push(DataCarryFact {
        data: symbol,
        declared: Some(declared),
        effective: CarryPolicy::PERMISSIVE,
    });
    let machine = SymbolHandle::from_arena_index(8);
    let mut machine_definition = Machine {
        symbol: machine,
        name: Identifier::generated("Worker::run"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut machine_definition,
        State {
            symbol: state_symbol,
            name: Identifier::generated("entry"),
            ..Default::default()
        },
    );
    program.typed.push_machine(machine_definition);
    program
        .facts
        .carry
        .suspension_crossings
        .push(SuspensionCrossingCarryFact {
            machine,
            state: state_symbol,
            statement_index: 3,
            call_ordinal: 1,
            target: machine,
            receiver: None,
            effective: CarryPolicy::STRICT,
            live_values: Vec::new(),
        });
    program
        .facts
        .carry
        .activation_wide_carry
        .push(MachineActivationCarryFact {
            machine,
            effective: CarryPolicy::STRICT,
            analysis_complete: true,
            contributing_types: Vec::new(),
            unnamed_strict_values: 1,
        });
    program
        .facts
        .carry
        .claim_policies
        .push(ClaimCarryPolicyFact {
            claim_identity: language_semantics::PermissionClaimIdentity::Unknown,
            effective: CarryPolicy::STRICT,
            contributing_origins: 2,
        });

    let json = carry_manifest_json(&program);

    assert!(json.contains("\"type\": \"PerCpuLease\""));
    assert!(json.contains(
        "\"declared\": {\"suspension\": \"forbidden\", \"cpu\": \"same\", \"thread\": \"any\", \"address\": \"stable\"}"
    ));
    assert!(json.contains(
        "\"effective\": {\"suspension\": \"allowed\", \"cpu\": \"any\", \"thread\": \"any\", \"address\": \"movable\"}"
    ));
    assert!(json.contains("\"machine\": \"Worker::run\""));
    assert!(json.contains("\"machine_overload_identity\": \"named-callable(path(Worker::run)"));
    assert!(json.contains(
        "\"safe_point_crossings\": [\n    {\n      \"machine\": \"Worker::run\",\n      \"machine_overload_identity\": \"named-callable(path(Worker::run)"
    ));
    assert!(json.contains("\"analysis_complete\": true"));
    assert!(json.contains("\"subtree_machine_count\": 1"));
    assert!(json.contains("\"unnamed_strict_values\": 1"));
    assert!(json.contains("\"claim_policies\": ["));
    assert!(json.contains("\"claim_identity\": {\"kind\": \"unknown\"}"));
    assert!(json.contains("\"contributing_origins\": 2"));
}

#[test]
fn task_activation_manifest_retains_exact_target_overload_identity() {
    let machine_symbol = SymbolHandle::from_arena_index(8);
    let state_symbol = SymbolHandle::from_arena_index(9);
    let mut program = CheckedTrees::default();
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Worker::run"),
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

    let normalized_id = |identity| {
        task_plans::MachineContractId::from_normalized_identity(identity)
            .expect("nonzero normalized identity")
    };
    let plan = task_plans::validate_activation_plan(task_plans::ActivationPlanCandidate {
        machine_contract: normalized_id(1),
        entry: task_plans::MachineEntryId::from_normalized_identity(2)
            .expect("nonzero entry identity"),
        argument_layout: task_plans::ValueLayoutId::from_normalized_identity(3)
            .expect("nonzero argument layout identity"),
        terminal_outcome_layout: task_plans::ValueLayoutId::from_normalized_identity(4)
            .expect("nonzero result layout identity"),
        calling_plan: task_plans::CallingPlanId::from_normalized_identity(5)
            .expect("nonzero calling-plan identity"),
        stack_plan: task_plans::StackPlan {
            bytes: 4096,
            alignment: 16,
            representation: task_plans::StackRepresentationId::from_normalized_identity(6)
                .expect("nonzero stack representation identity"),
        },
        may_suspend: false,
        may_block: false,
        canonical_suspension_crossings: Vec::new(),
        carry_obligations: task_plans::ActivationCarryObligations::none(),
        cancellation_required: false,
    })
    .expect("valid non-suspending activation plan");
    let activations = task_plans::TaskActivationPlanSet {
        activations: vec![task_plans::TaskActivationPlanFact {
            start_requirement: SymbolHandle::invalid(),
            target_machine: machine_symbol,
            target_entry: state_symbol,
            specialization_report_fingerprint: 0x1234,
            specialization_commitment: task_plans::TaskSpecializationCommitment::from_digest(
                [0x12; 32],
            ),
            operation: task_plans::TaskStartOperation::Start,
            selected_runtime: task_plans::SelectedTaskRuntimeProviderFact {
                runtime: task_plans::TaskRuntimeId::from_normalized_identity(7)
                    .expect("nonzero runtime identity"),
                provider_plan_name: "Runtime::selected".to_owned(),
                requirement_identity: "TaskRuntime::start#exact".to_owned(),
            },
            plan,
        }],
    };

    let json = task_activation_manifest_json(&program, &activations);

    assert!(json.contains("\"target_machine\": \"Worker::run\""));
    assert!(
        json.contains("\"target_machine_overload_identity\": \"named-callable(path(Worker::run)")
    );
    assert!(json.contains("\"specialization_report_fingerprint\": \"0x0000000000001234\""));
    assert!(json.contains(&format!(
        "\"specialization_commitment\": \"{}\"",
        "12".repeat(32)
    )));
    assert!(!json.contains("\"specialization_fingerprint\""));
    assert!(json.contains("\"activation_plan_id\": \"0x"));
}

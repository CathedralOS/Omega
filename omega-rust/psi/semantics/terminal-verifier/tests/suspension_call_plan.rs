use language_semantics::{CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalSuspensionCallPlan, TerminalSuspensionLiveValue,
    TerminalSuspensionPlace, TerminalSuspensionStorage, TerminalSuspensionValueType, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ModuleError, SuspensionCallPlanError, validate_module};

fn id<T: semantic_vocabulary::PsiSemanticId>(raw: u64) -> T {
    T::new(raw).expect("fixture identities are nonzero")
}

fn permissive() -> CarryPolicy {
    CarryPolicy::PERMISSIVE
}

fn cpu_local() -> CarryPolicy {
    CarryPolicy {
        suspension: CarrySuspension::Allowed,
        cpu: CarryCpu::Origin,
        host_thread: CarryHostThread::Any,
        address: CarryAddress::Movable,
    }
}

fn fixture() -> TerminalModule {
    let boolean = semantic_vocabulary::ScalarType::Boolean;
    let caller_parameter = ValueDeclaration {
        id: id(1),
        scalar_type: boolean,
    };
    let second_parameter = ValueDeclaration {
        id: id(2),
        scalar_type: boolean,
    };
    let caller_result = ValueDeclaration {
        id: id(3),
        scalar_type: boolean,
    };
    let local = ValueDeclaration {
        id: id(4),
        scalar_type: boolean,
    };
    let call_result = ValueDeclaration {
        id: id(5),
        scalar_type: boolean,
    };
    let callee_parameter = ValueDeclaration {
        id: id(6),
        scalar_type: boolean,
    };
    let callee_result = ValueDeclaration {
        id: id(7),
        scalar_type: boolean,
    };
    let second_call_result = ValueDeclaration {
        id: id(8),
        scalar_type: boolean,
    };
    let live = |value, storage, effective| TerminalSuspensionLiveValue {
        place: TerminalSuspensionPlace::Scalar(value),
        value_type: TerminalSuspensionValueType::Scalar(boolean),
        storage,
        claim_count: 0,
        claims: Vec::new(),
        effective,
    };
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 1,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: vec![TerminalSuspensionCallPlan {
            operation: id(2),
            crossing: id(41),
            target: terminal_psi::TerminalSuspensionCallTarget::Machine(id(2)),
            effective: cpu_local(),
            live_value_count: 3,
            live_values: vec![
                live(
                    caller_parameter.id,
                    TerminalSuspensionStorage::Parameter,
                    cpu_local(),
                ),
                live(
                    caller_parameter.id,
                    TerminalSuspensionStorage::CallArgument,
                    permissive(),
                ),
                live(local.id, TerminalSuspensionStorage::Local, permissive()),
            ],
        }],
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: id(1),
                attachment: None,
                parameters: vec![caller_parameter, second_parameter],
                structural_parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(caller_result),
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: id(1),
                blocks: vec![Block {
                    id: id(1),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: id(1),
                            result: OperationResult::Scalar(local),
                            kind: OperationKind::BooleanNot {
                                operand: second_parameter.id,
                            },
                        },
                        Operation {
                            id: id(2),
                            result: OperationResult::Scalar(call_result),
                            kind: OperationKind::Call {
                                callee: id(2),
                                arguments: vec![caller_parameter.id],
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        },
                        Operation {
                            id: id(3),
                            result: OperationResult::Scalar(second_call_result),
                            kind: OperationKind::Call {
                                callee: id(2),
                                arguments: vec![caller_parameter.id],
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        },
                    ],
                    terminator: Terminator::Return {
                        edge: id(1),
                        value: call_result.id,
                        cleanup_actions: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: id(1),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
            TerminalMachine {
                id: id(2),
                attachment: None,
                parameters: vec![callee_parameter],
                structural_parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(callee_result),
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: id(2),
                blocks: vec![Block {
                    id: id(2),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: id(2),
                        value: callee_parameter.id,
                        cleanup_actions: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: id(2),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
        ],
    };
    let plan = &module.suspension_call_plans[0];
    module
        .suspension_call_sites
        .push(terminal_psi::TerminalSuspensionCallSite {
            operation: plan.operation,
            crossing: plan.crossing,
            target: plan.target,
            frontier_commitment: terminal_psi::suspension_frontier_commitment(plan),
        });
    module
}

fn assert_reason(module: &TerminalModule, expected: SuspensionCallPlanError) {
    assert!(matches!(
        validate_module(module),
        Err(ModuleError::InvalidSuspensionCallPlan { reason, .. }) if reason == expected
    ));
}

fn refresh_site(module: &mut TerminalModule) {
    let [plan] = module.suspension_call_plans.as_slice() else {
        panic!("fixture has one suspension plan")
    };
    module.suspension_call_sites[0] = terminal_psi::TerminalSuspensionCallSite {
        operation: plan.operation,
        crossing: plan.crossing,
        target: plan.target,
        frontier_commitment: terminal_psi::suspension_frontier_commitment(plan),
    };
}

#[test]
fn suspension_call_plan_round_trips_and_verifies() {
    let module = fixture();
    validate_module(&module).expect("exact suspension frontier verifies");
}

#[test]
fn suspension_call_plan_mutations_fail_independently() {
    let mut missing = fixture();
    missing.suspension_call_plans.clear();
    missing.suspension_call_plan_count = 0;
    assert_reason(&missing, SuspensionCallPlanError::CountMismatch);

    let mut duplicate = fixture();
    duplicate.suspension_call_plan_count = 2;
    duplicate
        .suspension_call_plans
        .push(duplicate.suspension_call_plans[0].clone());
    assert!(validate_module(&duplicate).is_err());

    let mut redirected = fixture();
    redirected.suspension_call_plans[0].operation = id(3);
    assert!(validate_module(&redirected).is_err());

    let mut understated = fixture();
    understated.suspension_call_plans[0].live_values.remove(0);
    understated.suspension_call_plans[0].live_value_count -= 1;
    understated.suspension_call_plans[0].effective = permissive();
    assert!(validate_module(&understated).is_err());

    let mut wrong_type = fixture();
    wrong_type.suspension_call_plans[0].live_values[0].value_type =
        TerminalSuspensionValueType::Scalar(semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
                .unwrap(),
        ));
    refresh_site(&mut wrong_type);
    assert_reason(&wrong_type, SuspensionCallPlanError::TypeMismatch);

    let mut wrong_policy = fixture();
    wrong_policy.suspension_call_plans[0].live_values[0].effective = permissive();
    wrong_policy.suspension_call_plans[0].effective = permissive();
    assert!(validate_module(&wrong_policy).is_err());

    let mut wrong_crossing = fixture();
    wrong_crossing.suspension_call_plans[0].crossing = id(42);
    assert!(validate_module(&wrong_crossing).is_err());

    let mut wrong_storage = fixture();
    wrong_storage.suspension_call_plans[0].live_values[0].storage =
        TerminalSuspensionStorage::Local;
    assert!(validate_module(&wrong_storage).is_err());
}

#[test]
fn suspension_call_plan_rejects_coordinated_carry_axis_drift() {
    let policies = [
        CarryPolicy {
            suspension: CarrySuspension::Forbidden,
            ..cpu_local()
        },
        CarryPolicy {
            cpu: CarryCpu::Any,
            ..cpu_local()
        },
        CarryPolicy {
            host_thread: CarryHostThread::Origin,
            ..cpu_local()
        },
        CarryPolicy {
            address: CarryAddress::Stable,
            ..cpu_local()
        },
    ];
    for policy in policies {
        let mut module = fixture();
        module.suspension_call_plans[0].live_values[0].effective = policy;
        module.suspension_call_plans[0].effective = module.suspension_call_plans[0]
            .live_values
            .iter()
            .map(|live| live.effective)
            .fold(CarryPolicy::PERMISSIVE, CarryPolicy::intersect);
        assert!(validate_module(&module).is_err());
    }
}

#[test]
fn suspension_call_plan_rejects_argument_redirection() {
    let mut module = fixture();
    module.suspension_call_plans[0].live_values[1].place = TerminalSuspensionPlace::Scalar(id(2));
    refresh_site(&mut module);
    assert_reason(&module, SuspensionCallPlanError::InvalidCallArgument);
}

#[test]
fn suspension_call_plan_rejoins_operation_target_after_site_validation() {
    let mut module = fixture();
    module.suspension_call_plans[0].target =
        terminal_psi::TerminalSuspensionCallTarget::Machine(id(1));
    refresh_site(&mut module);
    assert_reason(&module, SuspensionCallPlanError::RedirectedToNonCall);
}

#[test]
fn suspension_plans_add_no_control_edge() {
    let module = fixture();
    assert_eq!(module.machines[0].blocks[0].operations.len(), 3);
    assert!(matches!(
        module.machines[0].blocks[0].terminator,
        Terminator::Return { .. }
    ));
}

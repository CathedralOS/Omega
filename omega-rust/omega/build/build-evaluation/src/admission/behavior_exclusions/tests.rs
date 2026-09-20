use super::{
    AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind, BehaviorExclusion,
    BehaviorExclusionReport, BehaviorExclusionVerdict, BehaviorExclusions, BoundaryServiceOwners,
    EvidenceGap, EvidenceGapKind, ProhibitedBehavior, ProhibitedSite,
    authored_behavior_exclusion_set, authored_behavior_exclusion_set_in,
    establish_behavior_exclusions, establish_behavior_exclusions_with_owners,
};
use effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
};
use effects::{SelectedProviderPlanFacts, TerminalAuthorityClass};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, OperationId, ServiceId,
    StructuralTypeId, ValueId,
};
use symbols::SymbolHandle;
use terminal_psi::{
    Block, BoundaryMachineDeclaration, CrashCause, CrashRouteBucket, CrashRouteGuard,
    MachineContract, Operation, OperationKind, OperationResult, ProviderCandidateConformance,
    ProviderRefinement, ProviderSignature, TerminalIndirectDynamicDispatch, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalParameterDynamicDispatch, Terminator,
    VocabularyMarker,
};

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}

fn boundary_id(raw: u64) -> BoundaryMachineId {
    BoundaryMachineId::new(raw).unwrap()
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).unwrap()
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

fn contract_id(raw: u64) -> ContractId {
    ContractId::new(raw).unwrap()
}

fn service_id(raw: u64) -> ServiceId {
    ServiceId::new(raw).unwrap()
}

fn empty_contract(raw: u64) -> MachineContract {
    MachineContract {
        erased_scalar_formals: Vec::new(),
        id: contract_id(raw),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn return_unit_block(raw: u64) -> Block {
    Block {
        erased_scalar_formals: Vec::new(),
        id: block_id(raw),
        structural_parameters: Vec::new(),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: edge_id(raw),
            trivial_affine_discards: Vec::new(),
        },
    }
}

/// One Unit machine whose executable content is exactly `blocks`.
fn unit_machine(raw: u64, blocks: Vec<Block>) -> TerminalMachine {
    TerminalMachine {
        id: machine_id(raw),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: blocks[0].id,
        blocks,
        contract: empty_contract(raw),
    }
}

fn unit_operation(raw: u64, kind: OperationKind) -> Operation {
    Operation {
        static_reach_binding: None,
        id: operation_id(raw),
        result: OperationResult::Unit,
        kind,
    }
}

fn call_unit(callee: MachineId) -> OperationKind {
    OperationKind::CallUnit {
        erased_arguments: Vec::new(),
        callee,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    }
}

fn boundary_declaration(raw: u64) -> BoundaryMachineDeclaration {
    BoundaryMachineDeclaration {
        id: boundary_id(raw),
        identity: format!("test::boundary_{raw}"),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: Vec::new(),
    }
}

fn terminal_module(
    machines: Vec<TerminalMachine>,
    boundaries: Vec<BoundaryMachineDeclaration>,
) -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machines[0].id,
        scalar_qualifications: Default::default(),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: boundaries,
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines,
    }
}

fn entries() -> Vec<MachineId> {
    vec![machine_id(1)]
}

fn trap_exclusions() -> BehaviorExclusions {
    BehaviorExclusions::from_selections([BehaviorExclusion::CrashCause(CrashCause::Trap)])
}

fn empty_plans() -> SelectedProviderPlanFacts {
    SelectedProviderPlanFacts::default()
}

/// One retained provider candidate: `machine` is the exact checked adapter
/// body selected plans may name by `candidate_identity`.
fn provider_candidate(
    boundary: BoundaryMachineId,
    requirement_identity: &str,
    candidate_identity: &str,
    candidate: MachineId,
) -> ProviderCandidateConformance {
    ProviderCandidateConformance {
        boundary,
        requirement_identity: requirement_identity.to_owned(),
        provider_identity: "TestProvider".to_owned(),
        candidate_identity: candidate_identity.to_owned(),
        candidate,
        signature: ProviderSignature {
            parameters: Vec::new(),
        },
        refinement: ProviderRefinement {
            positional_parameters: Vec::new(),
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    }
}

/// A fully-covering one-method plan whose only row binds `requirement_identity`
/// to `binding`.
fn selected_plan(
    requirement_identity: &str,
    binding: ProviderBinding,
) -> SelectedProviderPlanFacts {
    let plan = ProviderPlan {
        name: "test_plan".to_owned(),
        provider_type: "TestProvider".to_owned(),
        provider_type_package_identity: None,
        target: String::new(),
        schema: ServiceSchema {
            trait_name: "TestService".to_owned(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "call".to_owned(),
                requirement_owner: "TestService".to_owned(),
                requirement_owner_package_identity: None,
                requirement_identity: requirement_identity.to_owned(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: Vec::new(),
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
            method: "call".to_owned(),
            requirement_identity: requirement_identity.to_owned(),
            requirement_lifetime_partition: Vec::new(),
            binding,
        }],
        origin_package_identity: None,
        origin_package: "test".to_owned(),
    };
    SelectedProviderPlanFacts::from_selected_plans(vec![plan]).expect("test plan is fully covering")
}

fn checked_adapter_plans(
    requirement_identity: &str,
    machine_identity: &str,
) -> SelectedProviderPlanFacts {
    selected_plan(
        requirement_identity,
        ProviderBinding::CheckedAdapter {
            machine_identity: machine_identity.to_owned(),
            machine_package_identity: None,
        },
    )
}

/// Entry calls helper 2; helper's published contract permits Trap but the
/// selected body cannot produce it.
fn no_op_assertion_module() -> TerminalModule {
    let mut helper = unit_machine(2, vec![return_unit_block(2)]);
    helper.contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(1, call_unit(machine_id(2)))],
            ..return_unit_block(1)
        }],
    );
    terminal_module(vec![entry, helper], Vec::new())
}

/// Same public ceiling, but the selected helper body retains a possible Trap.
fn checking_assertion_module() -> TerminalModule {
    let mut helper = unit_machine(
        2,
        vec![Block {
            terminator: Terminator::Crash {
                edge: edge_id(2),
                cause: CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
            ..return_unit_block(2)
        }],
    );
    helper.contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(1, call_unit(machine_id(2)))],
            ..return_unit_block(1)
        }],
    );
    terminal_module(vec![entry, helper], Vec::new())
}

#[test]
fn no_op_assertion_passes_trap_exclusion_despite_public_ceiling() {
    let module = no_op_assertion_module();
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report, BehaviorExclusionReport::default());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Satisfied);
}

#[test]
fn checking_assertion_is_prohibited_under_trap_exclusion() {
    let module = checking_assertion_module();
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    assert_eq!(
        report.prohibited,
        vec![ProhibitedBehavior {
            exclusion: BehaviorExclusion::CrashCause(CrashCause::Trap),
            entry: machine_id(1),
            machine: machine_id(2),
            site: ProhibitedSite::CrashTerminator { block: block_id(2) },
        }]
    );
    assert!(report.gaps.is_empty());

    let abort_only =
        BehaviorExclusions::from_selections([BehaviorExclusion::CrashCause(CrashCause::Abort)]);
    let report = establish_behavior_exclusions(&module, &entries(), &abort_only, &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Satisfied);
}

#[test]
fn silent_logger_passes_console_exclusion() {
    let console = service_id(1);
    let mut logger = unit_machine(2, vec![return_unit_block(2)]);
    logger.published_service_ceiling = vec![console];
    logger.declared_service_reach = vec![console];
    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(1, call_unit(machine_id(2)))],
            ..return_unit_block(1)
        }],
    );
    let module = terminal_module(vec![entry, logger], Vec::new());
    let exclusions = BehaviorExclusions::from_selections([BehaviorExclusion::Service(console)]);
    let report = establish_behavior_exclusions(&module, &entries(), &exclusions, &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Satisfied);
    assert_eq!(report, BehaviorExclusionReport::default());
}

#[test]
fn actual_console_invocation_is_prohibited_even_with_silent_provider() {
    let console = service_id(1);
    let mut boundary = boundary_declaration(1);
    boundary.fixed_service_reach = vec![console];
    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(
                1,
                OperationKind::BoundaryCall {
                    boundary: boundary_id(1),
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    completion_receipts: Vec::new(),
                },
            )],
            ..return_unit_block(1)
        }],
    );
    // A selected silent provider does not erase the abstract invocation.
    let provider = unit_machine(2, vec![return_unit_block(2)]);
    let module = terminal_module(vec![entry, provider], vec![boundary]);
    let exclusions = BehaviorExclusions::from_selections([BehaviorExclusion::Service(console)]);
    let report = establish_behavior_exclusions(&module, &entries(), &exclusions, &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    assert_eq!(
        report.prohibited,
        vec![ProhibitedBehavior {
            exclusion: BehaviorExclusion::Service(console),
            entry: machine_id(1),
            machine: machine_id(1),
            site: ProhibitedSite::BoundaryCall {
                block: block_id(1),
                operation: operation_id(1),
                boundary: boundary_id(1),
            },
        }]
    );

    let unrelated = service_id(9);
    let exclusions = BehaviorExclusions::from_selections([BehaviorExclusion::Service(unrelated)]);
    let report = establish_behavior_exclusions(&module, &entries(), &exclusions, &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Satisfied);
}

#[test]
fn boundary_declared_trap_route_is_prohibited_conservatively() {
    let mut boundary = boundary_declaration(1);
    boundary.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(
                1,
                OperationKind::BoundaryCall {
                    boundary: boundary_id(1),
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    completion_receipts: Vec::new(),
                },
            )],
            ..return_unit_block(1)
        }],
    );
    let module = terminal_module(vec![entry], vec![boundary]);
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    assert_eq!(
        report.prohibited,
        vec![ProhibitedBehavior {
            exclusion: BehaviorExclusion::CrashCause(CrashCause::Trap),
            entry: machine_id(1),
            machine: machine_id(1),
            site: ProhibitedSite::BoundaryCall {
                block: block_id(1),
                operation: operation_id(1),
                boundary: boundary_id(1),
            },
        }]
    );
}

#[test]
fn dynamic_call_is_insufficient_evidence_not_prohibition() {
    let dynamic_entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(
                1,
                OperationKind::CallDynamicUnit {
                    descriptor_ordinal: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            )],
            ..return_unit_block(1)
        }],
    );
    let module = terminal_module(vec![dynamic_entry], Vec::new());
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(
        report.verdict(),
        BehaviorExclusionVerdict::InsufficientEvidence
    );
    assert_eq!(
        report.gaps,
        vec![EvidenceGap {
            entry: machine_id(1),
            machine: machine_id(1),
            block: Some(block_id(1)),
            operation: Some(operation_id(1)),
            kind: EvidenceGapKind::DynamicCall,
        }]
    );
    assert!(report.prohibited.is_empty());

    // A prohibited site elsewhere in the closure wins the verdict while the
    // gap stays reported.
    let mut dynamic_entry = unit_machine(
        1,
        vec![Block {
            operations: vec![
                unit_operation(
                    1,
                    OperationKind::CallDynamicUnit {
                        descriptor_ordinal: 0,
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                ),
                unit_operation(2, call_unit(machine_id(2))),
            ],
            ..return_unit_block(1)
        }],
    );
    let trapper = unit_machine(
        2,
        vec![Block {
            terminator: Terminator::Crash {
                edge: edge_id(2),
                cause: CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
            ..return_unit_block(2)
        }],
    );
    dynamic_entry.contract = empty_contract(1);
    let module = terminal_module(vec![dynamic_entry, trapper], Vec::new());
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    assert_eq!(report.prohibited.len(), 1);
    assert_eq!(report.gaps.len(), 1);
}

#[test]
fn exclusions_are_a_canonical_union() {
    let left = BehaviorExclusions::from_selections([
        BehaviorExclusion::Service(service_id(2)),
        BehaviorExclusion::CrashCause(CrashCause::Trap),
        BehaviorExclusion::Service(service_id(1)),
        BehaviorExclusion::CrashCause(CrashCause::Trap),
    ]);
    let right = BehaviorExclusions::from_selections([
        BehaviorExclusion::CrashCause(CrashCause::Trap),
        BehaviorExclusion::Service(service_id(1)),
        BehaviorExclusion::Service(service_id(2)),
    ]);
    assert_eq!(left, right);
    assert_eq!(left.crash_causes(), &[CrashCause::Trap]);
    assert_eq!(left.services(), &[service_id(1), service_id(2)]);

    // Union never removes an earlier restriction.
    let mut union = left.clone();
    union.union(&BehaviorExclusions::default());
    assert_eq!(union, left);
    let mut union = left.clone();
    union.union(&BehaviorExclusions::from_selections([
        BehaviorExclusion::CrashCause(CrashCause::Abort),
    ]));
    assert_eq!(union.crash_causes(), &[CrashCause::Trap, CrashCause::Abort]);

    assert!(BehaviorExclusions::default().is_empty());
    let module = checking_assertion_module();
    let report = establish_behavior_exclusions(
        &module,
        &entries(),
        &BehaviorExclusions::default(),
        &empty_plans(),
    );
    assert_eq!(report, BehaviorExclusionReport::default());
}

#[test]
fn unknown_entry_or_callee_is_an_evidence_gap() {
    let module = no_op_assertion_module();
    let report = establish_behavior_exclusions(
        &module,
        &[machine_id(9)],
        &trap_exclusions(),
        &empty_plans(),
    );
    assert_eq!(
        report.gaps,
        vec![EvidenceGap {
            entry: machine_id(9),
            machine: machine_id(9),
            block: None,
            operation: None,
            kind: EvidenceGapKind::UnknownEntry,
        }]
    );
    assert_eq!(
        report.verdict(),
        BehaviorExclusionVerdict::InsufficientEvidence
    );

    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(1, call_unit(machine_id(7)))],
            ..return_unit_block(1)
        }],
    );
    let module = terminal_module(vec![entry], Vec::new());
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(
        report.gaps,
        vec![EvidenceGap {
            entry: machine_id(1),
            machine: machine_id(1),
            block: Some(block_id(1)),
            operation: Some(operation_id(1)),
            kind: EvidenceGapKind::UnknownCallee(machine_id(7)),
        }]
    );
    assert_eq!(
        report.verdict(),
        BehaviorExclusionVerdict::InsufficientEvidence
    );
}

/// Entry invokes boundary 1 which declares a broad Trap contract; machine 2 is
/// a retained candidate named by `candidate_identity`.
fn boundary_call_module(candidate_bodies: Vec<TerminalMachine>) -> TerminalModule {
    let mut boundary = boundary_declaration(1);
    boundary.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(
                1,
                OperationKind::BoundaryCall {
                    boundary: boundary_id(1),
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    completion_receipts: Vec::new(),
                },
            )],
            ..return_unit_block(1)
        }],
    );
    let mut machines = vec![entry];
    machines.extend(candidate_bodies.clone());
    let mut module = terminal_module(machines, vec![boundary]);
    module.provider_candidates = candidate_bodies
        .iter()
        .map(|machine| {
            provider_candidate(
                boundary_id(1),
                "test::service::call",
                &format!("TestProvider::candidate_{}", machine.id.get()),
                machine.id,
            )
        })
        .collect();
    module
}

fn trapping_machine(raw: u64) -> TerminalMachine {
    unit_machine(
        raw,
        vec![Block {
            terminator: Terminator::Crash {
                edge: edge_id(raw),
                cause: CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
            ..return_unit_block(raw)
        }],
    )
}

#[test]
fn selected_silent_adapter_satisfies_exclusion_beneath_broad_contract() {
    let module = boundary_call_module(vec![unit_machine(2, vec![return_unit_block(2)])]);
    let plans = checked_adapter_plans("test::service::call", "TestProvider::candidate_2");
    let report = establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &plans);
    // The selected verified body is stronger evidence than the boundary's
    // declared Trap ceiling.
    assert_eq!(report, BehaviorExclusionReport::default());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Satisfied);
}

#[test]
fn selected_trapping_adapter_is_prohibited_by_its_body() {
    let module = boundary_call_module(vec![trapping_machine(2)]);
    let plans = checked_adapter_plans("test::service::call", "TestProvider::candidate_2");
    let report = establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &plans);
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    assert_eq!(
        report.prohibited,
        vec![ProhibitedBehavior {
            exclusion: BehaviorExclusion::CrashCause(CrashCause::Trap),
            entry: machine_id(1),
            machine: machine_id(2),
            site: ProhibitedSite::CrashTerminator { block: block_id(2) },
        }]
    );
}

#[test]
fn unconstrained_boundary_walks_every_candidate_and_the_contract() {
    let module = boundary_call_module(vec![
        unit_machine(2, vec![return_unit_block(2)]),
        trapping_machine(3),
    ]);
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    // The unselected trapping candidate is possible under an unconstrained
    // slot, and the declared contract still bounds any external realization.
    assert_eq!(report.prohibited.len(), 2);
    assert_eq!(
        report.prohibited[0],
        ProhibitedBehavior {
            exclusion: BehaviorExclusion::CrashCause(CrashCause::Trap),
            entry: machine_id(1),
            machine: machine_id(1),
            site: ProhibitedSite::BoundaryCall {
                block: block_id(1),
                operation: operation_id(1),
                boundary: boundary_id(1),
            },
        }
    );
    assert_eq!(
        report.prohibited[1].machine,
        machine_id(3),
        "the trapping retained candidate must be covered",
    );
}

#[test]
fn external_binding_keeps_the_declared_contract_not_the_candidates() {
    let module = boundary_call_module(vec![trapping_machine(2)]);
    let plans = selected_plan(
        "test::service::call",
        ProviderBinding::Syscall { number: 3 },
    );
    let report = establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &plans);
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    // Only the declared contract row: the unselected trapping candidate is not
    // part of this composition, and the external realization cannot be walked.
    assert_eq!(
        report.prohibited,
        vec![ProhibitedBehavior {
            exclusion: BehaviorExclusion::CrashCause(CrashCause::Trap),
            entry: machine_id(1),
            machine: machine_id(1),
            site: ProhibitedSite::BoundaryCall {
                block: block_id(1),
                operation: operation_id(1),
                boundary: boundary_id(1),
            },
        }]
    );
}

#[test]
fn checked_adapter_absent_from_candidates_uses_the_contract() {
    let module = boundary_call_module(vec![unit_machine(2, vec![return_unit_block(2)])]);
    let plans = checked_adapter_plans("test::service::call", "OtherProvider::call");
    let report = establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &plans);
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    assert_eq!(report.prohibited.len(), 1);
    assert_eq!(report.prohibited[0].machine, machine_id(1));
}

#[test]
fn bounded_dynamic_dispatch_covers_its_exact_realization() {
    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(
                1,
                OperationKind::CallDynamicUnit {
                    descriptor_ordinal: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            )],
            ..return_unit_block(1)
        }],
    );
    let mut module = terminal_module(vec![entry, trapping_machine(2)], Vec::new());
    module.dynamic_dispatch.indirect_dispatches = vec![TerminalIndirectDynamicDispatch {
        owner: machine_id(1),
        operation: operation_id(1),
        descriptor_ordinal: 0,
        declaring_trait_identity: "test::Dynamic".to_owned(),
        public_requirement_identity: "test::Dynamic::run".to_owned(),
        family_tuple: Vec::new(),
        requirement_identity: "test::Dynamic::run".to_owned(),
        realization_identity: "test::Trapper::run".to_owned(),
        realization_callable_identity: "test::Trapper::run".to_owned(),
        realization: machine_id(2),
    }];
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    assert_eq!(report.prohibited[0].machine, machine_id(2));

    // The same dispatch bound to a silent realization satisfies the exclusion.
    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(
                1,
                OperationKind::CallDynamicUnit {
                    descriptor_ordinal: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            )],
            ..return_unit_block(1)
        }],
    );
    let mut module = terminal_module(
        vec![entry, unit_machine(2, vec![return_unit_block(2)])],
        Vec::new(),
    );
    module.dynamic_dispatch.indirect_dispatches = vec![TerminalIndirectDynamicDispatch {
        owner: machine_id(1),
        operation: operation_id(1),
        descriptor_ordinal: 0,
        declaring_trait_identity: "test::Dynamic".to_owned(),
        public_requirement_identity: "test::Dynamic::run".to_owned(),
        family_tuple: Vec::new(),
        requirement_identity: "test::Dynamic::run".to_owned(),
        realization_identity: "test::Silent::run".to_owned(),
        realization_callable_identity: "test::Silent::run".to_owned(),
        realization: machine_id(2),
    }];
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Satisfied);
}

#[test]
fn parameter_dispatch_remains_insufficient_evidence() {
    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(
                1,
                OperationKind::CallDynamicParameterUnit {
                    parameter_ordinal: 0,
                    requirement_slot: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            )],
            ..return_unit_block(1)
        }],
    );
    let mut module = terminal_module(vec![entry], Vec::new());
    module.dynamic_dispatch.parameter_dispatches = vec![TerminalParameterDynamicDispatch {
        owner: machine_id(1),
        operation: operation_id(1),
        parameter_ordinal: 0,
        requirement_slot: 0,
    }];
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(
        report.verdict(),
        BehaviorExclusionVerdict::InsufficientEvidence
    );
    assert_eq!(report.gaps[0].kind, EvidenceGapKind::DynamicCall);
}

#[test]
fn service_rows_resolve_against_the_module_catalog_and_crash_rows_stand_alone() {
    let symbol = SymbolHandle::invalid();
    let rows = [
        AuthoredBehaviorExclusion {
            kind: AuthoredBehaviorExclusionKind::Service {
                trait_symbol: symbol,
            },
            selecting_machine: symbol,
            source_span: source::SourceSpan::default(),
        },
        AuthoredBehaviorExclusion {
            kind: AuthoredBehaviorExclusionKind::CrashCause {
                cause: CrashCause::Trap,
                case_symbol: symbol,
            },
            selecting_machine: symbol,
            source_span: source::SourceSpan::default(),
        },
    ];
    let mut module = terminal_module(vec![unit_machine(1, vec![return_unit_block(1)])], vec![]);
    module.services.push(terminal_psi::ServiceDeclaration {
        id: service_id(7),
        identity: "Sink".to_owned(),
        parents: Vec::new(),
    });
    let sink = |_: SymbolHandle| Some("Sink".to_owned());
    let other = |_: SymbolHandle| Some("Console".to_owned());
    let unresolved = |_: SymbolHandle| None;

    // In a module that declares the service, the row names its exact id.
    let resolved = authored_behavior_exclusion_set_in(&rows, &module, &sink);
    assert!(resolved.excludes_service(service_id(7)));
    assert!(resolved.excludes_crash_cause(CrashCause::Trap));
    // A service the module never declares imposes nothing there; the crash
    // row still stands.
    for identity in [
        &other as &dyn Fn(SymbolHandle) -> Option<String>,
        &unresolved,
    ] {
        let resolved = authored_behavior_exclusion_set_in(&rows, &module, identity);
        assert!(resolved.services().is_empty());
        assert!(resolved.excludes_crash_cause(CrashCause::Trap));
    }
    // The module-free set carries crash causes only.
    let crash_only = authored_behavior_exclusion_set(&rows);
    assert!(crash_only.services().is_empty());
    assert!(crash_only.excludes_crash_cause(CrashCause::Trap));
}

#[test]
fn boundary_ownership_counts_the_owning_service_and_its_parents_without_fixed_reach() {
    // `Sink::emit` spelled no `reaches`, so its fixed reach is empty; the
    // requirement still belongs to Sink, whose parent is Output. The
    // canonical requirement identity retains the owner, so the plain
    // source-free entry reconstructs the same join the producing admission
    // supplies from the checked trait declarations.
    let mut boundary = boundary_declaration(1);
    boundary.identity =
        "named-callable(path(Sink::emit),parameters(),result-dispatch())".to_owned();
    let machine = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(
                1,
                OperationKind::BoundaryCall {
                    boundary: boundary_id(1),
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    completion_receipts: Vec::new(),
                },
            )],
            ..return_unit_block(1)
        }],
    );
    let mut module = terminal_module(vec![machine], vec![boundary]);
    assert!(module.boundary_machines[0].fixed_service_reach.is_empty());
    module.services.push(terminal_psi::ServiceDeclaration {
        id: service_id(2),
        identity: "Output".to_owned(),
        parents: Vec::new(),
    });
    module.services.push(terminal_psi::ServiceDeclaration {
        id: service_id(1),
        identity: "Sink".to_owned(),
        parents: vec![service_id(2)],
    });
    let owners = BoundaryServiceOwners::from_module_identities(&module);
    assert_eq!(owners.owner(boundary_id(1)), Some(service_id(1)));

    for excluded in [service_id(1), service_id(2)] {
        let exclusions =
            BehaviorExclusions::from_selections([BehaviorExclusion::Service(excluded)]);
        // The source-free entry reconstructs ownership from the canonical
        // identity: the boundary call invokes Sink and its parent Output.
        let report =
            establish_behavior_exclusions(&module, &entries(), &exclusions, &empty_plans());
        assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
        assert_eq!(report.prohibited.len(), 1);
        assert_eq!(
            report.prohibited[0].exclusion,
            BehaviorExclusion::Service(excluded)
        );
        assert_eq!(
            report.prohibited[0].site,
            ProhibitedSite::BoundaryCall {
                block: block_id(1),
                operation: operation_id(1),
                boundary: boundary_id(1),
            }
        );
        // An explicit owner join reaches the identical verdict.
        let owned = establish_behavior_exclusions_with_owners(
            &module,
            &entries(),
            &exclusions,
            &empty_plans(),
            &owners,
        );
        assert_eq!(owned, report);
    }
}

#[test]
fn boundary_without_a_canonical_requirement_identity_contributes_no_owner() {
    // Identities outside `named-callable(path(Owner::requirement),...)`
    // carry no decodable owner: the boundary call counts only its spelled
    // fixed reach.
    let boundary = boundary_declaration(1); // identity "test::boundary_1"
    let machine = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(
                1,
                OperationKind::BoundaryCall {
                    boundary: boundary_id(1),
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    completion_receipts: Vec::new(),
                },
            )],
            ..return_unit_block(1)
        }],
    );
    let mut module = terminal_module(vec![machine], vec![boundary]);
    module.services.push(terminal_psi::ServiceDeclaration {
        id: service_id(1),
        identity: "test".to_owned(),
        parents: Vec::new(),
    });
    assert_eq!(
        BoundaryServiceOwners::from_module_identities(&module).owner(boundary_id(1)),
        None
    );
    let exclusions =
        BehaviorExclusions::from_selections([BehaviorExclusion::Service(service_id(1))]);
    let report = establish_behavior_exclusions(&module, &entries(), &exclusions, &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Satisfied);
}

#[test]
fn qualified_and_escaped_owner_paths_rejoin_the_declaring_service() {
    for (identity, service_identity) in [
        (
            "named-callable(path(a::b::Sink::emit),parameters(),result-dispatch())",
            "a::b::Sink",
        ),
        (
            "named-callable(path(Sink\\(v2\\)::emit),parameters(),result-dispatch())",
            "Sink(v2)",
        ),
    ] {
        let mut boundary = boundary_declaration(1);
        boundary.identity = identity.to_owned();
        let machine = unit_machine(
            1,
            vec![Block {
                operations: vec![unit_operation(
                    1,
                    OperationKind::BoundaryCall {
                        boundary: boundary_id(1),
                        arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                )],
                ..return_unit_block(1)
            }],
        );
        let mut module = terminal_module(vec![machine], vec![boundary]);
        module.services.push(terminal_psi::ServiceDeclaration {
            id: service_id(1),
            identity: service_identity.to_owned(),
            parents: Vec::new(),
        });
        let exclusions =
            BehaviorExclusions::from_selections([BehaviorExclusion::Service(service_id(1))]);
        let report =
            establish_behavior_exclusions(&module, &entries(), &exclusions, &empty_plans());
        assert_eq!(
            report.verdict(),
            BehaviorExclusionVerdict::Prohibited,
            "boundary identity {identity} must rejoin service {service_identity}"
        );
    }
}

#[test]
fn canonical_machine_overload_sharing_a_service_name_counts_conservatively() {
    // A canonical identity cannot distinguish a trait requirement from an
    // exact machine overload: `Endpoint::step` reads as owned by a declared
    // service `Endpoint`. The join counts that service — rejecting rather
    // than dropping a possible invocation, the direction incomplete
    // evidence already takes.
    let mut boundary = boundary_declaration(1);
    boundary.identity =
        "named-callable(path(Endpoint::step),parameters(),result-dispatch())".to_owned();
    let machine = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(
                1,
                OperationKind::BoundaryCall {
                    boundary: boundary_id(1),
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    completion_receipts: Vec::new(),
                },
            )],
            ..return_unit_block(1)
        }],
    );
    let mut module = terminal_module(vec![machine], vec![boundary]);
    module.services.push(terminal_psi::ServiceDeclaration {
        id: service_id(1),
        identity: "Endpoint".to_owned(),
        parents: Vec::new(),
    });
    let exclusions =
        BehaviorExclusions::from_selections([BehaviorExclusion::Service(service_id(1))]);
    let report = establish_behavior_exclusions(&module, &entries(), &exclusions, &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
}

/// An entry whose `Return` edge commits `cleanup_actions`: the nominal
/// invocations carry executable cleanup machines, while the claim-free
/// discards carry no code and must not join the closure.
fn cleanup_edge_entry(
    raw: u64,
    cleanups: Vec<terminal_psi::NominalAffineCleanup>,
) -> TerminalMachine {
    let cleanup_actions = cleanups
        .into_iter()
        .map(terminal_psi::TerminalAffineCleanupAction::InvokeNominal)
        .chain([terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
            place_id(raw),
        )])
        .collect();
    unit_machine(
        raw,
        vec![Block {
            terminator: Terminator::Return {
                edge: edge_id(raw),
                value: ValueId::new(raw).expect("nonzero value identity"),
                cleanup_actions,
            },
            ..return_unit_block(raw)
        }],
    )
}

fn nominal_cleanup(raw: u64, cleanup_machine: MachineId) -> terminal_psi::NominalAffineCleanup {
    terminal_psi::NominalAffineCleanup {
        place: place_id(raw),
        structural_type: StructuralTypeId::new(raw).expect("nonzero structural type identity"),
        cleanup_machine,
        cleanup_receiver: None,
        requirement_obligations: Vec::new(),
    }
}

fn place_id(raw: u64) -> semantic_vocabulary::PlaceId {
    semantic_vocabulary::PlaceId::new(raw).expect("nonzero place identity")
}

#[test]
fn nominal_cleanup_edge_joins_the_cleanup_machine_to_the_closure() {
    // A cleanup machine that can Trap is prohibited possible behavior:
    // the Return edge's InvokeNominal action runs it even though no call
    // operation names it.
    let entry = cleanup_edge_entry(1, vec![nominal_cleanup(1, machine_id(2))]);
    let module = terminal_module(vec![entry, trapping_machine(2)], Vec::new());
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    assert_eq!(
        report.prohibited,
        vec![ProhibitedBehavior {
            exclusion: BehaviorExclusion::CrashCause(CrashCause::Trap),
            entry: machine_id(1),
            machine: machine_id(2),
            site: ProhibitedSite::CrashTerminator { block: block_id(2) },
        }]
    );
    assert!(report.gaps.is_empty());
}

#[test]
fn nominal_unit_cleanup_edge_joins_the_cleanup_machine_to_the_closure() {
    // The nominal-affine ReturnUnit variant carries the same executable
    // cleanup invocations directly, without the ordered action list.
    let entry = unit_machine(
        1,
        vec![Block {
            terminator: Terminator::ReturnUnitNominalAffine {
                edge: edge_id(1),
                cleanups: vec![nominal_cleanup(1, machine_id(2))],
            },
            ..return_unit_block(1)
        }],
    );
    let module = terminal_module(vec![entry, trapping_machine(2)], Vec::new());
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    assert_eq!(report.prohibited[0].machine, machine_id(2));
}

#[test]
fn nominal_cleanup_reaching_an_excluded_service_is_prohibited() {
    // The cleanup machine's own body is walked: a boundary call inside it
    // invokes the service exactly as an ordinary call would.
    let console = service_id(1);
    let mut boundary = boundary_declaration(1);
    boundary.fixed_service_reach = vec![console];
    let cleanup_body = unit_machine(
        2,
        vec![Block {
            operations: vec![unit_operation(
                2,
                OperationKind::BoundaryCall {
                    boundary: boundary_id(1),
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    completion_receipts: Vec::new(),
                },
            )],
            ..return_unit_block(2)
        }],
    );
    let entry = cleanup_edge_entry(1, vec![nominal_cleanup(1, machine_id(2))]);
    let module = terminal_module(vec![entry, cleanup_body], vec![boundary]);
    let exclusions = BehaviorExclusions::from_selections([BehaviorExclusion::Service(console)]);
    let report = establish_behavior_exclusions(&module, &entries(), &exclusions, &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    assert_eq!(
        report.prohibited,
        vec![ProhibitedBehavior {
            exclusion: BehaviorExclusion::Service(console),
            entry: machine_id(1),
            machine: machine_id(2),
            site: ProhibitedSite::BoundaryCall {
                block: block_id(2),
                operation: operation_id(2),
                boundary: boundary_id(1),
            },
        }]
    );
}

#[test]
fn an_absent_cleanup_machine_is_an_evidence_gap_not_a_pass() {
    // A cleanup edge whose selected machine is not retained cannot certify
    // absence: the edge still commits a body the module does not show.
    let entry = cleanup_edge_entry(1, vec![nominal_cleanup(1, machine_id(9))]);
    let module = terminal_module(vec![entry], Vec::new());
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(
        report.gaps,
        vec![EvidenceGap {
            entry: machine_id(1),
            machine: machine_id(1),
            block: Some(block_id(1)),
            operation: None,
            kind: EvidenceGapKind::UnknownCallee(machine_id(9)),
        }]
    );
    assert_eq!(
        report.verdict(),
        BehaviorExclusionVerdict::InsufficientEvidence
    );
}

#[test]
fn inert_nominal_cleanup_satisfies_the_exclusion() {
    // A verified-shape cleanup machine — a Unit body with no retained
    // excluded behavior — joins the closure without changing the verdict.
    let entry = cleanup_edge_entry(1, vec![nominal_cleanup(1, machine_id(2))]);
    let cleanup_body = unit_machine(2, vec![return_unit_block(2)]);
    let module = terminal_module(vec![entry, cleanup_body], Vec::new());
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report, BehaviorExclusionReport::default());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Satisfied);
}

fn physical_exclusions(classes: &[TerminalAuthorityClass]) -> BehaviorExclusions {
    BehaviorExclusions::from_selections(
        classes
            .iter()
            .map(|&class| BehaviorExclusion::PhysicalAuthorityClass(class)),
    )
}

/// Physical-class selections union canonically alongside the semantic rows:
/// the retained set orders and deduplicates all three axes, and only an
/// entirely empty union is trivial.
#[test]
fn physical_authority_classes_are_a_canonical_union_axis() {
    let left = BehaviorExclusions::from_selections([
        BehaviorExclusion::PhysicalAuthorityClass(TerminalAuthorityClass::PortIo),
        BehaviorExclusion::CrashCause(CrashCause::Trap),
        BehaviorExclusion::PhysicalAuthorityClass(TerminalAuthorityClass::ProcessOutput),
        BehaviorExclusion::PhysicalAuthorityClass(TerminalAuthorityClass::PortIo),
    ]);
    assert_eq!(
        left.physical_authority_classes(),
        &[
            TerminalAuthorityClass::ProcessOutput,
            TerminalAuthorityClass::PortIo,
        ]
    );
    assert!(left.excludes_physical_authority_class(TerminalAuthorityClass::PortIo));
    assert!(!left.excludes_physical_authority_class(TerminalAuthorityClass::ProcessInput));
    assert_eq!(left.crash_causes(), &[CrashCause::Trap]);
    assert!(!left.is_empty());

    // Union never removes an earlier physical restriction.
    let mut union = left.clone();
    union.union(&physical_exclusions(&[
        TerminalAuthorityClass::MachineControl,
    ]));
    assert_eq!(
        union.physical_authority_classes(),
        &[
            TerminalAuthorityClass::ProcessOutput,
            TerminalAuthorityClass::MachineControl,
            TerminalAuthorityClass::PortIo,
        ]
    );
}

/// A physical-only exclusion set is not vacuous at the semantic stage: it
/// still demands the bounded entry/call closure, so an unresolvable call is
/// an evidence gap rather than a silent pass. Over a bounded module the same
/// set is satisfied — mechanism adjudication waits for native realization.
#[test]
fn physical_exclusion_demands_bounded_closure_but_yields_no_site() {
    let bounded = no_op_assertion_module();
    let exclusions = physical_exclusions(&[TerminalAuthorityClass::ProcessOutput]);
    let report = establish_behavior_exclusions(&bounded, &entries(), &exclusions, &empty_plans());
    assert_eq!(report, BehaviorExclusionReport::default());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Satisfied);

    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(1, call_unit(machine_id(7)))],
            ..return_unit_block(1)
        }],
    );
    let module = terminal_module(vec![entry], Vec::new());
    let report = establish_behavior_exclusions(&module, &entries(), &exclusions, &empty_plans());
    assert_eq!(
        report.gaps,
        vec![EvidenceGap {
            entry: machine_id(1),
            machine: machine_id(1),
            block: Some(block_id(1)),
            operation: Some(operation_id(1)),
            kind: EvidenceGapKind::UnknownCallee(machine_id(7)),
        }]
    );
    assert_eq!(
        report.verdict(),
        BehaviorExclusionVerdict::InsufficientEvidence
    );
}

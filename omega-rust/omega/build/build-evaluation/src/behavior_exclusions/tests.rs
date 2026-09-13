use super::{
    BehaviorExclusion, BehaviorExclusionReport, BehaviorExclusionVerdict, BehaviorExclusions,
    EvidenceGap, EvidenceGapKind, ProhibitedBehavior, ProhibitedSite,
    establish_behavior_exclusions,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, OperationId, ServiceId,
};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, CrashCause, CrashRouteBucket, CrashRouteGuard,
    MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, VocabularyMarker,
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
        id: contract_id(raw),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn return_unit_block(raw: u64) -> Block {
    Block {
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
    let report = establish_behavior_exclusions(&module, &entries(), &trap_exclusions());
    assert_eq!(report, BehaviorExclusionReport::default());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Satisfied);
}

#[test]
fn checking_assertion_is_prohibited_under_trap_exclusion() {
    let module = checking_assertion_module();
    let report = establish_behavior_exclusions(&module, &entries(), &trap_exclusions());
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
    let report = establish_behavior_exclusions(&module, &entries(), &abort_only);
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
    let report = establish_behavior_exclusions(&module, &entries(), &exclusions);
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
    let report = establish_behavior_exclusions(&module, &entries(), &exclusions);
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
    let report = establish_behavior_exclusions(&module, &entries(), &exclusions);
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
    let report = establish_behavior_exclusions(&module, &entries(), &trap_exclusions());
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
    let report = establish_behavior_exclusions(&module, &entries(), &trap_exclusions());
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
    let report = establish_behavior_exclusions(&module, &entries(), &trap_exclusions());
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
    let report = establish_behavior_exclusions(&module, &entries(), &BehaviorExclusions::default());
    assert_eq!(report, BehaviorExclusionReport::default());
}

#[test]
fn unknown_entry_or_callee_is_an_evidence_gap() {
    let module = no_op_assertion_module();
    let report = establish_behavior_exclusions(&module, &[machine_id(9)], &trap_exclusions());
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
    let report = establish_behavior_exclusions(&module, &entries(), &trap_exclusions());
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

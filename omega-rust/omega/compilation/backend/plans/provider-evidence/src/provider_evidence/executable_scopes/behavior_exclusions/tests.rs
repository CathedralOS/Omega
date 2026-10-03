use super::{
    BehaviorExclusion, BehaviorExclusionReport, BehaviorExclusionVerdict, BehaviorExclusions,
    BoundaryServiceOwners, EvidenceGap, EvidenceGapKind, ProhibitedBehavior, ProhibitedSite,
    establish_behavior_exclusions,
};
use crate::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
};
use crate::{SelectedProviderPlanFacts, TerminalAuthorityClass};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, OperationId, ServiceId,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, ClosedConformanceApplication,
    ClosedConformanceApplicationCommitment, ClosedConformanceCallableResult,
    ClosedConformanceRealizationCallable, ClosedConformanceRow, CrashCause, CrashRouteBucket,
    CrashRouteGuard, MachineContract, Operation, OperationKind, OperationResult,
    ProviderCandidateConformance, ProviderRefinement, ProviderSignature, StructuralAccess,
    StructuralArgument, TerminalDynamicConformanceSelection, TerminalDynamicDescriptorArgument,
    TerminalDynamicDescriptorParameter, TerminalDynamicDescriptorSource,
    TerminalDynamicRequirement, TerminalIndirectDynamicDispatch, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalReboundDynamicDescriptor, Terminator,
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
        erased_proof_formals: Vec::new(),
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
        erased_proof_formals: Vec::new(),
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
        suspension_crossing: None,
        id: operation_id(raw),
        result: OperationResult::Unit,
        kind,
    }
}

fn call_unit(callee: MachineId) -> OperationKind {
    OperationKind::CallUnit {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
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
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
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
                completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
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
                    requirement_obligations: Vec::new(),
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
fn parameter_dispatch_crash_contract_counts_excluded_causes() {
    // The requirement's retained crash contract bounds the unenumerated
    // parameter target: an excluded cause in it is a prohibited site.
    let parameter_entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(
                1,
                OperationKind::CallDynamicParameterUnit {
                    parameter_ordinal: 0,
                    requirement_slot: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: vec![CrashRouteBucket {
                        cause: CrashCause::Trap,
                        alternatives: vec![CrashRouteGuard::Truth],
                    }],
                },
            )],
            ..return_unit_block(1)
        }],
    );
    let module = terminal_module(vec![parameter_entry], Vec::new());
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    assert_eq!(
        report.prohibited,
        vec![ProhibitedBehavior {
            exclusion: BehaviorExclusion::CrashCause(CrashCause::Trap),
            entry: machine_id(1),
            machine: machine_id(1),
            site: ProhibitedSite::DynamicCall {
                block: block_id(1),
                operation: operation_id(1),
            },
        }]
    );
    assert!(report.gaps.is_empty());
}

#[test]
fn parameter_dispatch_crash_contract_suffices_for_crash_only_exclusions() {
    // A non-empty contract lacking the excluded cause decides the site for a
    // crash-only exclusion set — the closed bucket list attests absence.
    let parameter_entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(
                1,
                OperationKind::CallDynamicParameterScalar {
                    parameter_ordinal: 0,
                    requirement_slot: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: vec![CrashRouteBucket {
                        cause: CrashCause::Abort,
                        alternatives: vec![CrashRouteGuard::Truth],
                    }],
                },
            )],
            ..return_unit_block(1)
        }],
    );
    let module = terminal_module(vec![parameter_entry], Vec::new());
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Satisfied);
    assert_eq!(report, BehaviorExclusionReport::default());
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
                    requirement_obligations: Vec::new(),
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

/// The descriptor interface one worker parameter declares: a single
/// `test::Dynamic::run` requirement in slot zero.
fn dynamic_parameter(machine: MachineId) -> TerminalDynamicDescriptorParameter {
    TerminalDynamicDescriptorParameter {
        owner: machine,
        ordinal: 0,
        source_position: 0,
        trait_identity: "test::Dynamic".to_owned(),
        access: StructuralAccess::SharedBorrow,
        requirements: vec![TerminalDynamicRequirement {
            slot: 0,
            declaring_trait_identity: "test::Dynamic".to_owned(),
            public_requirement_identity: "test::Dynamic::run".to_owned(),
            family_tuple: Vec::new(),
            result: ClosedConformanceCallableResult::Unit,
        }],
    }
}

/// One retained conformance application whose single row realizes
/// `test::Dynamic::run` through `realization`.
fn dynamic_conformance_application(
    realization: MachineId,
    commitment_byte: u8,
) -> ClosedConformanceApplication {
    ClosedConformanceApplication {
        owner: machine_id(900),
        declaration_identity: format!("test::application_{commitment_byte}"),
        telescope: Vec::new(),
        subject_identity: None,
        trait_identity: "test::Dynamic".to_owned(),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: vec![ClosedConformanceRealizationCallable {
            source_callable_identity: "test::Selected::run".to_owned(),
            machine: realization,
            result: ClosedConformanceCallableResult::Unit,
        }],
        rows: vec![ClosedConformanceRow {
            declaring_trait_identity: "test::Dynamic".to_owned(),
            public_requirement_identity: "test::Dynamic::run".to_owned(),
            family_tuple: Vec::new(),
            requirement_identity: "test::Dynamic::run".to_owned(),
            realization_identity: "test::Selected::run".to_owned(),
            realization_callable_identity: Some("test::Selected::run".to_owned()),
        }],
        report_fingerprint: 0,
        commitment: ClosedConformanceApplicationCommitment::from_digest([commitment_byte; 32]),
    }
}

/// One caller-local dynamic selection bound to the application's commitment.
fn dynamic_selection(owner: MachineId, commitment_byte: u8) -> TerminalDynamicConformanceSelection {
    TerminalDynamicConformanceSelection {
        owner,
        ordinal: 0,
        source: StructuralArgument {
            place: place_id(7000 + commitment_byte as u64),
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        },
        conformance_application_report_fingerprint: 0,
        conformance_application_commitment: ClosedConformanceApplicationCommitment::from_digest(
            [commitment_byte; 32],
        ),
    }
}

fn parameter_dispatch_worker(raw: u64) -> TerminalMachine {
    unit_machine(
        raw,
        vec![Block {
            operations: vec![unit_operation(
                raw,
                OperationKind::CallDynamicParameterUnit {
                    parameter_ordinal: 0,
                    requirement_slot: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            )],
            ..return_unit_block(raw)
        }],
    )
}

#[test]
fn parameter_dispatch_missing_descriptor_row_stays_a_gap() {
    // The worker declares the parameter and the entry calls it, but no
    // catalog argument supplies the descriptor: missing evidence, not a
    // proven-empty target set.
    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(1, call_unit(machine_id(2)))],
            ..return_unit_block(1)
        }],
    );
    let mut module = terminal_module(vec![entry, parameter_dispatch_worker(2)], Vec::new());
    module.dynamic_dispatch.parameters = vec![dynamic_parameter(machine_id(2))];
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(
        report.verdict(),
        BehaviorExclusionVerdict::InsufficientEvidence
    );
    assert_eq!(report.gaps[0].kind, EvidenceGapKind::DynamicCall);
}

#[test]
fn parameter_dispatch_follows_a_forwarded_parameter() {
    // entry(1) -> call forwarder(2) passing selection; forwarder forwards
    // its own parameter 0 to worker(3); worker dispatches the parameter.
    // The forwarded chain rejoins the selection's exact realization (4).
    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(1, call_unit(machine_id(2)))],
            ..return_unit_block(1)
        }],
    );
    let forwarder = unit_machine(
        2,
        vec![Block {
            operations: vec![unit_operation(2, call_unit(machine_id(3)))],
            ..return_unit_block(2)
        }],
    );
    let mut module = terminal_module(
        vec![
            entry,
            forwarder,
            parameter_dispatch_worker(3),
            trapping_machine(4),
        ],
        Vec::new(),
    );
    module.dynamic_dispatch.parameters = vec![
        dynamic_parameter(machine_id(2)),
        dynamic_parameter(machine_id(3)),
    ];
    module.dynamic_dispatch.selections = vec![dynamic_selection(machine_id(1), 9)];
    module.dynamic_dispatch.arguments = vec![
        TerminalDynamicDescriptorArgument {
            owner: machine_id(1),
            operation: operation_id(1),
            parameter_ordinal: 0,
            source: TerminalDynamicDescriptorSource::Selection { ordinal: 0 },
        },
        TerminalDynamicDescriptorArgument {
            owner: machine_id(2),
            operation: operation_id(2),
            parameter_ordinal: 0,
            source: TerminalDynamicDescriptorSource::Parameter { ordinal: 0 },
        },
    ];
    module.closed_conformance_applications =
        vec![dynamic_conformance_application(machine_id(4), 9)];
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    assert_eq!(report.prohibited[0].machine, machine_id(4));
    assert!(report.gaps.is_empty(), "{:?}", report.gaps);
}

#[test]
fn parameter_dispatch_covers_a_rebound_descriptor() {
    // The caller dispatches through a rebound descriptor: both the initial
    // and rebound selections are admitted bodies, so the trap in the
    // rebound realization is witnessed.
    let entry = unit_machine(
        1,
        vec![Block {
            operations: vec![unit_operation(1, call_unit(machine_id(2)))],
            ..return_unit_block(1)
        }],
    );
    let mut module = terminal_module(
        vec![
            entry,
            parameter_dispatch_worker(2),
            unit_machine(3, vec![return_unit_block(3)]),
            trapping_machine(4),
        ],
        Vec::new(),
    );
    module.dynamic_dispatch.parameters = vec![dynamic_parameter(machine_id(2))];
    module.dynamic_dispatch.selections = vec![
        dynamic_selection(machine_id(1), 11),
        TerminalDynamicConformanceSelection {
            ordinal: 1,
            ..dynamic_selection(machine_id(1), 12)
        },
    ];
    module.dynamic_dispatch.rebound_descriptors = vec![TerminalReboundDynamicDescriptor {
        owner: machine_id(1),
        ordinal: 0,
        initial_selection_ordinal: 0,
        rebound_selection_ordinal: 1,
    }];
    module.dynamic_dispatch.arguments = vec![TerminalDynamicDescriptorArgument {
        owner: machine_id(1),
        operation: operation_id(1),
        parameter_ordinal: 0,
        source: TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal: 0 },
    }];
    module.closed_conformance_applications = vec![
        dynamic_conformance_application(machine_id(3), 11),
        dynamic_conformance_application(machine_id(4), 12),
    ];
    let report =
        establish_behavior_exclusions(&module, &entries(), &trap_exclusions(), &empty_plans());
    assert_eq!(report.verdict(), BehaviorExclusionVerdict::Prohibited);
    assert_eq!(report.prohibited[0].machine, machine_id(4));
    assert!(report.gaps.is_empty(), "{:?}", report.gaps);
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
                    requirement_obligations: Vec::new(),
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
                        requirement_obligations: Vec::new(),
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
                    requirement_obligations: Vec::new(),
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

/// A build exclusion is independent of permission to exercise the mechanism:
/// both a receiver-less review and an explicitly admitted one must refuse it.
#[test]
fn physical_exclusions_use_exercised_authority_not_receiver_permission() {
    let disposition = crate::TerminalAuthorityDisposition::from_classes([
        TerminalAuthorityClass::ProcessTermination,
    ]);
    let mut rejected_messages = Vec::new();
    for permission_policy in [
        None,
        Some(crate::TerminalAuthorityPermissionPolicyIdentity::from_parts(1, [17; 32])),
    ] {
        let leaf = crate::TerminalAuthorityClosureLeaf::new(
            crate::provider_plan::ServiceSchemaDigest::from_digest([41; 32]),
            "test::Console::exit()".to_owned(),
            crate::provider_plan::ProviderPlanDigest::from_digest([43; 32]),
            crate::CompilerIntrinsicExecutionIdentity::HostedExitProcessI32.into(),
            Some(disposition.clone()),
            permission_policy.map(|_| disposition.clone()),
        )
        .unwrap();
        let receipt = crate::TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
            [7; 32],
            target::NativeTarget::linux_x64(),
            empty_plans().identity_digest(),
            crate::TerminalAuthorityPolicyIdentity::from_parts(1, [11; 32]),
            permission_policy,
            vec![leaf],
        )
        .unwrap();
        for exclusions in [
            BehaviorExclusions::default(),
            trap_exclusions(),
            physical_exclusions(&[TerminalAuthorityClass::ProcessOutput]),
        ] {
            assert!(
                exclusions
                    .validate_physical_authority_closure(&receipt)
                    .is_ok()
            );
        }
        let diagnostics = physical_exclusions(&[TerminalAuthorityClass::ProcessTermination])
            .validate_physical_authority_closure(&receipt)
            .expect_err("the exercised class violates either permission mode");
        let messages = diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>();
        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0],
            "requested behavior exclusions could not be satisfied by the reviewed mechanism closure"
        );
        assert!(messages[1].contains("test::Console::exit()"));
        assert!(messages[1].contains("excluded physical authority class ProcessTermination"));
        rejected_messages.push(messages);
    }
    assert_eq!(rejected_messages[0], rejected_messages[1]);
}

#[test]
fn unclassified_mechanisms_cannot_establish_physical_exclusions() {
    let leaf = crate::TerminalAuthorityClosureLeaf::new(
        crate::provider_plan::ServiceSchemaDigest::from_digest([41; 32]),
        "test::NativeExit::finish".to_owned(),
        crate::provider_plan::ProviderPlanDigest::from_digest([43; 32]),
        crate::CompilerIntrinsicExecutionIdentity::HostedExitProcessI32.into(),
        None,
        None,
    )
    .unwrap();
    let receipt = crate::TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
        [7; 32],
        target::NativeTarget::linux_x64(),
        empty_plans().identity_digest(),
        crate::TerminalAuthorityPolicyIdentity::from_parts(1, [11; 32]),
        None,
        vec![leaf],
    )
    .unwrap();
    receipt.validate().unwrap();
    trap_exclusions()
        .validate_physical_authority_closure(&receipt)
        .unwrap();
    let errors = physical_exclusions(&[TerminalAuthorityClass::ProcessOutput])
        .validate_physical_authority_closure(&receipt)
        .expect_err("unknown authority is not proof of an absent class");
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("no physical authority classification")
    }));
}

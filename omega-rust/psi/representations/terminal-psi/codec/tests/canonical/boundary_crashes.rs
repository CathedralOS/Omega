use super::{
    Block, BoundaryMachineDeclaration, CrashCause, CrashRouteBucket, CrashRouteGuard,
    MachineContract, Operation, OperationKind, OperationResult, Proposition, ScalarTerm,
    ScalarType, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, block_id,
    boundary_machine_id, contract_id, edge_id, encode_module, fixture, i32_type, operation_id,
    value_id,
};
use terminal_psi::CrashPredicateTerm;
use terminal_verifier::ProofBundle;

fn routes(value: u64) -> Vec<CrashRouteBucket> {
    vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(
                ScalarTerm::value(value_id(value), ScalarType::Boolean),
                ScalarTerm::boolean(true),
            ),
        ))],
    }]
}

fn boundary_fixture() -> TerminalModule {
    let mut module = fixture();
    module.boundary_machines = vec![BoundaryMachineDeclaration {
        scalar_requires: Vec::new(),
        fixed_service_reach: Vec::new(),
        id: boundary_machine_id(1),
        identity: "test::guarded_boundary".into(),
        attachment: None,
        parameter_order: vec![
            terminal_psi::BoundaryParameterKind::Scalar,
            terminal_psi::BoundaryParameterKind::Scalar,
        ],
        scalar_parameters: vec![ScalarType::Boolean; 2],
        crash_routes: routes(1),
        structural_parameters: Vec::new(),
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    }];
    let machine = &mut module.machines[0];
    machine.parameters = [2, 3]
        .into_iter()
        .map(|value| ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(value),
            scalar_type: ScalarType::Boolean,
        })
        .collect();
    machine.result = TerminalMachineResult::Unit;
    machine.contract = MachineContract {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        id: contract_id(1),
        crash_routes: routes(3),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    };
    machine.blocks = vec![Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        id: block_id(1),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(1),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
                requirement_obligations: Vec::new(),
                boundary: boundary_machine_id(1),
                arguments: vec![value_id(3), value_id(2)],
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
            },
        }],
        terminator: Terminator::ReturnUnit {
            edge: edge_id(1),
            trivial_affine_discards: Vec::new(),
        },
    }];
    module
}

#[test]
fn boundary_crash_encoding_rejects_cause_actual_formal_type_and_order_tampering() {
    for mutation in 0..5 {
        let mut changed = boundary_fixture();
        match mutation {
            0 => changed.boundary_machines[0].crash_routes[0].cause = CrashCause::Abort,
            1 => {
                let OperationKind::BoundaryCall { arguments, .. } =
                    &mut changed.machines[0].blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                arguments.swap(0, 1);
            }
            2 => changed.boundary_machines[0].crash_routes = routes(3),
            3 => {
                changed.boundary_machines[0].scalar_parameters[0] = ScalarType::Integer(i32_type())
            }
            4 => {
                let duplicate = changed.boundary_machines[0].crash_routes[0].clone();
                changed.boundary_machines[0].crash_routes.push(duplicate);
            }
            _ => unreachable!(),
        }
        if mutation <= 1 {
            // Caller coverage is a supplied-certificate question now, not a
            // module-validity one: the tampered module still encodes
            // canonically, and verification rejects the reconstructed
            // continuation obligation that names no supplied roster row.
            encode_module(&changed).expect("coverage failures still encode; coverage is proved");
            assert!(
                matches!(
                    terminal_verifier::verify_module(
                        &changed,
                        &ProofBundle::default(),
                        &proof_admission::AdmissionProfile::default(),
                    ),
                    Err(
                        terminal_verifier::VerificationError::MissingCrashObligationEvidence(
                            terminal_psi::CrashObligationOwner::Continuation { .. }
                        )
                    )
                ),
                "mutation {mutation}"
            );
        } else {
            assert!(encode_module(&changed).is_err(), "mutation {mutation}");
        }
    }
}

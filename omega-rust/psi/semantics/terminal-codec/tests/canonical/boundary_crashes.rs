use super::*;
use terminal_codec::{
    CanonicalTerminalArtifact, build_identity_optimization_execution_record, decode_proof_bundle,
    encode_proof_bundle,
};
use terminal_psi::CrashPredicateTerm;
use terminal_verifier::{ModuleError, ProofBundle};

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
        fixed_service_reach: Vec::new(),
        id: boundary_machine_id(1),
        identity: "test::guarded_boundary".into(),
        attachment: None,
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
        id: contract_id(1),
        crash_routes: routes(3),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    };
    machine.blocks = vec![Block {
        id: block_id(1),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![Operation {
            static_reach_binding: None,
            id: operation_id(1),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
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
fn boundary_crash_envelope_round_trips_without_source_or_provider_body() {
    let module = boundary_fixture();
    let encoded = encode_module(&module).expect("declaration-local guard encodes");
    let decoded = decode_module(&encoded).expect("guard independently validates after decoding");
    assert_eq!(decoded, module);
    assert_eq!(encode_module(&decoded).unwrap(), encoded);
    let proof = ProofBundle::default();
    let optimization = build_identity_optimization_execution_record(&module, &proof).unwrap();
    let artifact = CanonicalTerminalArtifact::from_parts(&module, &proof, &optimization, None)
        .expect("opaque boundary requirement needs no provider-body assumptions");
    artifact.validate().expect("source-free artifact replay");
}

#[test]
fn boundary_crash_envelope_deletion_cannot_reuse_the_original_artifact_binding() {
    let module = boundary_fixture();
    let proof_bytes = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let proof = decode_proof_bundle(&proof_bytes).unwrap();
    let optimization = build_identity_optimization_execution_record(&module, &proof).unwrap();
    let identity = terminal_psi_identity(&module).unwrap();

    let mut deleted = module.clone();
    deleted.boundary_machines[0].crash_routes.clear();
    assert_ne!(terminal_psi_identity(&deleted).unwrap(), identity);
    assert!(CanonicalTerminalArtifact::from_parts(&deleted, &proof, &optimization, None).is_err());

    let mut uncovered = module;
    uncovered.machines[0].contract.crash_routes.clear();
    assert!(matches!(
        encode_module(&uncovered),
        Err(CodecError::InvalidModule(
            ModuleError::CallCrashContinuationUncovered { .. }
        ))
    ));
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
        assert!(encode_module(&changed).is_err(), "mutation {mutation}");
    }
}

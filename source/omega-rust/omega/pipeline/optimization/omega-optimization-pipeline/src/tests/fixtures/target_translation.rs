use crate::tests::*;

pub(crate) fn integer_literal_return_artifact(
    integer_type: IntegerType,
    value: IntegerValue,
) -> (Vec<u8>, Vec<u8>) {
    scalar_terminal_artifact(
        ScalarType::Integer(integer_type),
        ScalarTerminal::Literal(OperationKind::IntegerConstant { value }),
    )
}

pub(crate) fn boolean_literal_return_artifact(value: bool) -> (Vec<u8>, Vec<u8>) {
    scalar_terminal_artifact(
        ScalarType::Boolean,
        ScalarTerminal::Literal(OperationKind::BooleanConstant { value }),
    )
}

pub(crate) fn scalar_crash_artifact(
    scalar_type: ScalarType,
    cause: CrashCause,
) -> (Vec<u8>, Vec<u8>) {
    scalar_terminal_artifact(scalar_type, ScalarTerminal::Crash(cause))
}

enum ScalarTerminal {
    Literal(OperationKind),
    Crash(CrashCause),
}

fn scalar_terminal_artifact(
    scalar_type: ScalarType,
    terminal: ScalarTerminal,
) -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(30_001).unwrap();
    let entry = BlockId::new(30_002).unwrap();
    let constant_value = ValueId::new(30_003).unwrap();
    let function_result = ValueId::new(30_004).unwrap();
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let edge = EdgeId::new(30_006).unwrap();
    let (operations, terminator, crash_routes) = match terminal {
        ScalarTerminal::Literal(literal) => (
            vec![Operation {
                id: OperationId::new(30_005).unwrap(),
                result: OperationResult::Scalar(declaration(constant_value)),
                kind: literal,
            }],
            Terminator::Return {
                edge,
                value: constant_value,
                cleanup_actions: Vec::new(),
            },
            Vec::new(),
        ),
        ScalarTerminal::Crash(cause) => (
            Vec::new(),
            Terminator::Crash {
                edge,
                cause,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
            vec![CrashRouteBucket {
                cause,
                alternatives: vec![CrashRouteGuard::Truth],
            }],
        ),
    };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(function_result)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![Block {
                id: entry,
                parameters: Vec::new(),
                operations,
                terminator,
            }],
            contract: MachineContract {
                id: ContractId::new(30_007).unwrap(),
                crash_routes,
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

//! Independently decoded Terminal Psi enforces retained authored floating
//! entry ranges: the roster is replayed from the artifact, never from
//! source-side state. A canonical artifact re-encodes byte-for-byte, and a
//! delivery the authored range excludes rejects after decode.
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IeeeFloatValue, MachineId, OperationId, ScalarType, ValueId,
};
use terminal_codec::{decode_module, encode_module};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, ScalarFloatRange,
    ScalarQualificationCatalog, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ModuleError, ProofBundle, verify_module};

fn module(argument_bits: u64) -> TerminalModule {
    let caller_constant = value_id(1);
    let call_result = value_id(2);
    let caller_result = value_id(3);
    let callee_parameter = value_id(4);
    let callee_result = value_id(5);
    TerminalModule {
        scalar_qualifications: ScalarQualificationCatalog {
            // The authored `f64[0.0..1.5)` survives publication as exact
            // interchange bits; the exclusive endpoint is preserved verbatim.
            float_entry_ranges: vec![ScalarFloatRange {
                machine: machine_id(2),
                parameter: callee_parameter,
                minimum: IeeeFloatValue::Binary64(0.0f64.to_bits()),
                maximum: IeeeFloatValue::Binary64(1.5f64.to_bits()),
                maximum_inclusive: false,
            }],
            ..Default::default()
        },
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
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
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(1),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(declaration(caller_result)),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(1),
                            result: OperationResult::Scalar(declaration(caller_constant)),
                            kind: OperationKind::IeeeFloatConstant {
                                value: IeeeFloatValue::Binary64(argument_bits),
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: operation_id(2),
                            result: OperationResult::Scalar(declaration(call_result)),
                            kind: OperationKind::Call {
                                erased_arguments: Vec::new(),
                                callee: machine_id(2),
                                arguments: vec![caller_constant],
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        },
                    ],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(1),
                        value: call_result,
                    },
                }],
                contract: empty_contract(1),
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(2),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![declaration(callee_parameter)],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(declaration(callee_result)),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(2),
                        value: callee_parameter,
                    },
                }],
                contract: empty_contract(2),
            },
        ],
    }
}

#[test]
fn decoded_artifact_replays_below_endpoint_delivery() {
    let bytes = encode_module(&module(1.0f64.to_bits())).expect("encode ranged module");
    let decoded = decode_module(&bytes).expect("decode ranged module");
    assert_eq!(
        encode_module(&decoded),
        Ok(bytes.clone()),
        "canonical ranged artifact re-encodes byte-for-byte"
    );
    verify_module(
        &decoded,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("decoded artifact admits the below-endpoint delivery");
}

#[test]
fn decoded_artifact_replays_exclusive_endpoint_and_nan_rejection() {
    // A non-conforming artifact cannot be encoded at all: encode runs the
    // module validator. Tamper with a conforming artifact instead — replace
    // the delivered constant's retained bits — then decode and replay.
    let mut bytes = encode_module(&module(1.0f64.to_bits())).expect("encode ranged module");
    let needle = 1.0f64.to_bits().to_le_bytes();
    let offset = bytes
        .windows(8)
        .position(|window| window == needle)
        .expect("the delivered constant is retained on the wire");
    for bits in [
        1.5f64.to_bits(),
        f64::NAN.to_bits(),
        2.0f64.to_bits(),
        f64::INFINITY.to_bits(),
    ] {
        bytes[offset..offset + 8].copy_from_slice(&bits.to_le_bytes());
        // Decode replays module validation: the tampered delivery is refused
        // by the decoder itself, before verification even runs.
        let error = decode_module(&bytes).expect_err("tampered delivery must reject at decode");
        assert!(
            matches!(
                error,
                terminal_codec::CodecError::InvalidModule(
                    ModuleError::ScalarFloatRangeDelivery { .. }
                )
            ),
            "excluded delivery must reject as a range delivery fault: {error:?}"
        );
    }
}

#[test]
fn roster_row_survives_decode_with_exact_bits() {
    let bytes = encode_module(&module(1.0f64.to_bits())).expect("encode ranged module");
    let decoded = decode_module(&bytes).expect("decode ranged module");
    let [range] = decoded.scalar_qualifications.float_entry_ranges[..] else {
        panic!("decoded artifact lost the retained float entry range")
    };
    assert_eq!(range.minimum, IeeeFloatValue::Binary64(0.0f64.to_bits()));
    assert_eq!(range.maximum, IeeeFloatValue::Binary64(1.5f64.to_bits()));
    assert!(!range.maximum_inclusive);
}

fn empty_contract(raw: u64) -> MachineContract {
    MachineContract {
        erased_scalar_formals: Vec::new(),
        id: ContractId::new(raw).unwrap(),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn declaration(id: ValueId) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type: ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64),
    }
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
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

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}

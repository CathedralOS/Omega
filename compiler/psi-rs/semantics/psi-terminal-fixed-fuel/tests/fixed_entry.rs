use psi_core::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use psi_proof_kernel::{AdmissionProfile, EvidenceRoute, PrimitiveJudgment};
use psi_terminal::{
    Block, ContractClause, MachineContract, Operation, OperationKind, SemanticVersion,
    TerminalMachine, TerminalModule, Terminator, ValueDeclaration,
};
use psi_terminal_codec::{CodecError, decode_module, encode_module, terminal_psi_identity};
use psi_terminal_fixed_fuel::{
    FixedFuelError, derive_fixed_entry_fuel, derive_fixed_segment_fuel, validate_fixed_entry_fuel,
    validate_fixed_segment_fuel,
};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle, verify_module};

#[test]
fn straight_line_entry_has_an_exact_recomputable_bound() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(1)).unwrap();

    assert_eq!(
        certificate.terminal_psi(),
        terminal_psi_identity(&module).unwrap()
    );
    assert_eq!(certificate.schedule().schedule_version(), 1);
    assert_eq!(certificate.entry(), machine_id(1));
    assert_eq!(certificate.return_edge(), edge_id(2));
    assert!(certificate.relevant_preconditions().is_empty());
    assert_eq!(certificate.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();

    let bytes = encode_module(&module).unwrap();
    drop(verified);
    drop(module);
    let decoded = decode_module(&bytes).unwrap();
    let independently_verified =
        verify_module(&decoded, &proof, &AdmissionProfile::default()).unwrap();
    validate_fixed_entry_fuel(&independently_verified, &certificate).unwrap();
}

#[test]
fn semantic_mutation_invalidates_the_old_certificate_without_changing_cost() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(1)).unwrap();

    let mut changed = module.clone();
    changed.machines[0].blocks[0].operations[0].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(8),
    };
    let changed_verified = verify_module(&changed, &proof, &AdmissionProfile::default()).unwrap();
    let changed_certificate = derive_fixed_entry_fuel(&changed_verified, machine_id(1)).unwrap();
    assert_ne!(
        changed_certificate.terminal_psi(),
        certificate.terminal_psi()
    );
    assert_eq!(
        changed_certificate.ceiling_units(),
        certificate.ceiling_units()
    );
    assert_eq!(
        validate_fixed_entry_fuel(&changed_verified, &certificate),
        Err(FixedFuelError::CertificateMismatch)
    );
}

#[test]
fn certificate_derivation_requires_canonical_semantic_identity() {
    let (mut module, proof) = fixture();
    module.machines[0].blocks.swap(0, 1);
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    assert_eq!(
        derive_fixed_entry_fuel(&verified, machine_id(1)),
        Err(FixedFuelError::SemanticIdentity(
            CodecError::NonCanonicalOrder("blocks by BlockId")
        ))
    );
}

#[test]
fn selected_segments_include_their_exact_terminal_edge() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();

    let entry_to_jump =
        derive_fixed_segment_fuel(&verified, machine_id(1), block_id(1), edge_id(1)).unwrap();
    assert_eq!(
        entry_to_jump.terminal_psi(),
        terminal_psi_identity(&module).unwrap()
    );
    assert_eq!(entry_to_jump.schedule().schedule_version(), 1);
    assert_eq!(entry_to_jump.machine(), machine_id(1));
    assert_eq!(entry_to_jump.start_block(), block_id(1));
    assert_eq!(entry_to_jump.end_edge(), edge_id(1));
    assert!(entry_to_jump.relevant_preconditions().is_empty());
    assert_eq!(entry_to_jump.ceiling_units(), 2);
    validate_fixed_segment_fuel(&verified, &entry_to_jump).unwrap();

    let jump_target_to_return =
        derive_fixed_segment_fuel(&verified, machine_id(1), block_id(2), edge_id(2)).unwrap();
    assert_eq!(jump_target_to_return.ceiling_units(), 1);
    validate_fixed_segment_fuel(&verified, &jump_target_to_return).unwrap();
}

#[test]
fn a_segment_cannot_cross_the_reached_return_to_find_an_unrelated_edge() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();

    assert_eq!(
        derive_fixed_segment_fuel(&verified, machine_id(1), block_id(2), edge_id(1)),
        Err(FixedFuelError::SegmentEndNotReached {
            requested: edge_id(1),
            reached_return: edge_id(2),
        })
    );
}

fn fixture() -> (TerminalModule, ProofBundle) {
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let literal = ScalarTerm::integer(integer, IntegerValue::Signed(7)).unwrap();
    let goal = Proposition::Equal(literal.clone(), literal);
    let obligation = obligation_id(1);
    let module = TerminalModule {
        semantic_version: SemanticVersion::CURRENT,
        entry: machine_id(1),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            parameters: Vec::new(),
            result: ValueDeclaration {
                id: value_id(3),
                scalar_type,
            },
            structural_places: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![
                Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: operation_id(1),
                        result: ValueDeclaration {
                            id: value_id(1),
                            scalar_type,
                        },
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(7),
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: edge_id(1),
                        target: block_id(2),
                        arguments: vec![value_id(1)],
                    },
                },
                Block {
                    id: block_id(2),
                    parameters: vec![ValueDeclaration {
                        id: value_id(2),
                        scalar_type,
                    }],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: edge_id(2),
                        value: value_id(2),
                    },
                },
            ],
            contract: MachineContract {
                id: contract_id(1),
                requires: vec![goal.clone()],
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal,
                }],
            },
        }],
    };
    let proof = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
        }],
    };
    (module, proof)
}

macro_rules! id_constructor {
    ($function:ident, $type:ty) => {
        fn $function(raw: u64) -> $type {
            <$type>::new(raw).expect("test identities are nonzero")
        }
    };
}

id_constructor!(value_id, ValueId);
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
id_constructor!(operation_id, OperationId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);

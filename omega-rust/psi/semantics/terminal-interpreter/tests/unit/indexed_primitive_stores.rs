//! Runtime-indexed `&write`/`&mut` element stores execute through the same
//! verified primitive-access walk as literal-indexed stores once the u64
//! operand is resolved.

use super::{
    AcceptTerminalEffects, AdmissionProfile, CertificateEnvelope, EvidenceRoute, IntegerSign,
    IntegerType, IntegerValue, ObligationEvidence, Operation, OperationKind, OperationResult,
    ProofBundle, ProofNode, ProofRule, ProofSystemMarker, Proposition, ScalarTerm, ScalarType,
    StructuralAccess, TerminalExecutionResult, TerminalExecutionStatus, TerminalFuelMeter,
    TerminalModule, ValueDeclaration, decode_module, encode_module, encode_proof_section,
    obligation_id, operation_id, place_id, structural_type_id, value_id, verify_module,
};
use terminal_interpreter::{
    TerminalExecution, TerminalStructuralByteArrayValue, TerminalStructuralInputs,
    TerminalStructuralValue,
};
use terminal_verifier::reconstruct_operation_obligations;

fn u64_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn u8_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
}

fn index_term() -> ScalarTerm {
    ScalarTerm::value(value_id(10), u64_type())
}

/// Two machines: the entry caller loans its mutable `[u8; 3]` parameter to the
/// callee's `access` parameter, which runs a u64 index constant, a u8 value
/// constant, the indexed store, then Unit return. Host byte-array backing can
/// only bind owned/mutable entry parameters, so `&write` is observed through
/// the caller's backing exactly like the literal-indexed store tests.
fn indexed_store_module(
    access: StructuralAccess,
    index: u64,
    value: u64,
) -> (TerminalModule, ProofBundle) {
    let mut module = super::write_only_primitive_call_module();
    module.structural_types = vec![
        terminal_psi::StructuralTypeDeclaration {
            id: structural_type_id(91),
            identity: "test::Octet".into(),
            shape: terminal_psi::StructuralTypeShape::PrimitiveScalar(u8_type()),
        },
        terminal_psi::StructuralTypeDeclaration {
            id: structural_type_id(93),
            identity: "test::Triple".into(),
            shape: terminal_psi::StructuralTypeShape::FixedArray {
                element: structural_type_id(91),
                length: 3,
            },
        },
    ];
    let caller = &mut module.machines[0];
    caller.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    caller.structural_parameters[0].structural_type = structural_type_id(93);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[0].kind
    else {
        panic!("writer call")
    };
    structural_arguments[0].access = access;
    let callee = &mut module.machines[1];
    callee.structural_parameters[0].access = access;
    callee.structural_parameters[0].structural_type = structural_type_id(93);
    callee.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(1),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(10),
                scalar_type: u64_type(),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(u128::from(index)),
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(2),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(11),
                scalar_type: u8_type(),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(u128::from(value)),
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(3),
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyIndexedPrimitiveStore {
                destination: place_id(92),
                path: Vec::new(),
                index: value_id(10),
                value: value_id(11),
                obligation: obligation_id(1),
            },
        },
    ];
    let bundle = certificate(&module);
    (module, bundle)
}

/// Reconstruct the store's `index < extent` obligation and prove it from the
/// closed literal relation plus the index constant's reconstructed equation.
fn certificate(module: &TerminalModule) -> ProofBundle {
    let site = reconstruct_operation_obligations(module)
        .unwrap()
        .pop()
        .expect("one canonical indexed-store obligation");
    assert!(site.canonical_certificate);
    // The index constant's reconstructed equation carries the exact literal.
    let (axiom, equality, literal) = site
        .semantic_axioms
        .iter()
        .enumerate()
        .find_map(|(index, fact)| {
            let Proposition::Equal(left, literal) = fact else {
                return None;
            };
            (*left == index_term()).then(|| (index, fact.clone(), literal.clone()))
        })
        .expect("the index constant's reconstructed equation");
    let obligation = site.obligation.proposition.clone();
    let Proposition::LessThan(_, bound) = &obligation else {
        panic!("the obligation is index < declared extent")
    };
    let proof = ProofNode {
        conclusion: obligation.clone(),
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(ProofNode {
                conclusion: Proposition::LessThan(literal.clone(), bound.clone()),
                rule: ProofRule::Primitive(
                    proof_admission::PrimitiveJudgment::ClosedIntegerRelation,
                ),
            }),
            equality: Box::new(ProofNode {
                conclusion: equality,
                rule: ProofRule::SemanticAxiom { index: axiom },
            }),
            endpoint: 0,
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: semantic_vocabulary::EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
        ..ProofBundle::default()
    };
    verify_module(module, &bundle, &AdmissionProfile::default()).unwrap();
    bundle
}

fn start(module: &TerminalModule, bundle: &ProofBundle) -> TerminalExecution {
    // Decode and independently verify the real artifact, not private state.
    let semantic = encode_module(module).unwrap();
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    let proof = encode_proof_section(module, bundle).unwrap();
    TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 700,
                structural_type: structural_type_id(93),
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            byte_arrays: &[TerminalStructuralByteArrayValue {
                argument_index: 0,
                path: Vec::new(),
                bytes: vec![11, 128, 255],
            }],
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn runtime_indexed_write_only_store_mutates_the_selected_element_only() {
    for access in [
        StructuralAccess::WriteOnlyBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        for index in [0, 1, 2] {
            let (module, bundle) = indexed_store_module(access, index, 7);
            let mut execution = start(&module, &bundle);
            assert_eq!(
                execution
                    .resume(
                        &mut TerminalFuelMeter::unbounded(),
                        &mut AcceptTerminalEffects
                    )
                    .unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            let mut expected = [11, 128, 255];
            expected[index as usize] = 7;
            assert_eq!(
                execution.structural_byte_array(700, &[]),
                Some(&expected[..]),
                "only the runtime-selected byte changes"
            );
        }
    }
}

#[test]
fn indexed_store_rejects_wrong_custody_and_operand_types() {
    let (module, _) = indexed_store_module(StructuralAccess::WriteOnlyBorrow, 1, 7);
    let mut shared = module.clone();
    shared.machines[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert!(
        verify_module(
            &shared,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
    // The index is a u64 operand: any other declared width or sign rejects.
    let mut narrow = module.clone();
    narrow.machines[1].blocks[0].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = u8_type();
    assert!(
        verify_module(
            &narrow,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
    // The stored value must be the array's exact element type.
    let mut wide = module;
    wide.machines[1].blocks[0].operations[1]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = u64_type();
    assert!(verify_module(&wide, &ProofBundle::default(), &AdmissionProfile::default()).is_err());
}

#[test]
fn out_of_extent_index_can_be_formed_but_never_verified() {
    let (mut module, _) = indexed_store_module(StructuralAccess::WriteOnlyBorrow, 1, 7);
    let OperationKind::IntegerConstant { value } =
        &mut module.machines[1].blocks[0].operations[0].kind
    else {
        panic!("index constant")
    };
    *value = IntegerValue::Unsigned(5);
    // Formation still succeeds: the runtime operand is an inert u64.
    terminal_verifier::validate_module(&module).unwrap();
    // The reconstructed obligation `5 < 3` has no honest certificate.
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
}

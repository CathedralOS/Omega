//! Unsupported native byte operations reject without erasing verified effects.

use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PsiSemanticId, ScalarTerm, ScalarType,
};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_psi::{
    Block, Operation, OperationKind, OperationResult, SuccessorEdge, Terminator, ValueDeclaration,
};
use terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, LoweringError, lower_artifact_sections,
    lower_artifact_sections_for_native_realization, lower_artifact_sections_for_optimization,
};

fn id<Identity: PsiSemanticId>(raw: u64) -> Identity {
    Identity::new(raw).unwrap()
}

#[test]
fn verified_field_length_and_indexed_store_reject_at_every_native_entrance() {
    let source = r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8; 3] in Utf8; }
        machine Record::replace(&mut self) { self.out = "XXX"; }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Record::replace").unwrap();
    let mut module = lowered.semantic_module;
    let machine = &mut module.machines[0];
    let (destination, path, field) = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::StructuralByteSequenceFieldStore {
                destination,
                path,
                field,
                ..
            } => Some((*destination, path.clone(), *field)),
            _ => None,
        })
        .expect("source retains the exact bounded field");
    machine
        .structural_places
        .retain(|place| place.id == destination);
    let count = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let byte = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let scalar = |operation, value, scalar_type, kind| Operation {
        static_reach_binding: None,
        id: id(operation),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(value),
            scalar_type,
        }),
        kind,
    };
    let successor = |edge, target| SuccessorEdge {
        edge: id(edge),
        target: id(target),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.entry = id(101);
    machine.blocks = vec![
        Block {
            id: id(101),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![
                scalar(
                    101,
                    101,
                    count,
                    OperationKind::StructuralByteSequenceFieldLength {
                        source: destination,
                        path: path.clone(),
                        field,
                    },
                ),
                scalar(
                    102,
                    102,
                    count,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(0),
                    },
                ),
                scalar(
                    103,
                    103,
                    byte,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(65),
                    },
                ),
                scalar(
                    104,
                    104,
                    ScalarType::Boolean,
                    OperationKind::IntegerLessThan {
                        left: id(102),
                        right: id(101),
                    },
                ),
            ],
            terminator: Terminator::Conditional {
                condition: id(104),
                when_true: successor(101, 102),
                when_false: successor(102, 103),
            },
        },
        Block {
            id: id(102),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: id(105),
                result: OperationResult::Unit,
                kind: OperationKind::StructuralByteSequenceFieldByteStore {
                    destination,
                    path,
                    field,
                    index: id(102),
                    value: id(103),
                    length: id(101),
                    obligation: id(101),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id(103),
                trivial_affine_discards: Vec::new(),
            },
        },
        Block {
            id: id(103),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: id(104),
                trivial_affine_discards: Vec::new(),
            },
        },
    ];
    let questions = terminal_verifier::reconstruct_terminal_obligations(&module).unwrap();
    let [question] = questions.obligations() else {
        panic!("one indexed-store question")
    };
    // The selected edge folds the constant index to zero. Rejoin the authored
    // SSA operand through its exact constant equation for the store's goal.
    let zero = ScalarTerm::integer(
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        IntegerValue::Unsigned(0),
    )
    .unwrap();
    let selected_bound = Proposition::LessThan(zero.clone(), ScalarTerm::value(id(101), count));
    let index_equation = Proposition::Equal(ScalarTerm::value(id(102), count), zero);
    let bound_axiom = question
        .semantic_axioms
        .iter()
        .position(|axiom| axiom == &selected_bound)
        .expect("selected true edge establishes the folded index bound");
    let index_axiom = question
        .semantic_axioms
        .iter()
        .position(|axiom| axiom == &index_equation)
        .expect("the authored index retains its exact constant equation");
    let proof = terminal_verifier::ProofBundle {
        evidence: vec![terminal_verifier::ObligationEvidence {
            obligation: question.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: id(101),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: question.obligation.proposition.clone(),
                    rule: ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(ProofNode {
                            conclusion: selected_bound,
                            rule: ProofRule::SemanticAxiom { index: bound_axiom },
                        }),
                        equality: Box::new(ProofNode {
                            conclusion: index_equation,
                            rule: ProofRule::SemanticAxiom { index: index_axiom },
                        }),
                        endpoint: 0,
                    },
                },
            }),
        }],
        ..Default::default()
    };
    let profile = AdmissionProfile::default();
    let mut wrong_polarity = module.clone();
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut wrong_polarity.machines[0].blocks[0].terminator
    else {
        panic!("entry retains the exact index guard")
    };
    std::mem::swap(when_true, when_false);
    assert!(
        terminal_verifier::verify_module(&wrong_polarity, &proof, &profile).is_err(),
        "the false edge cannot reuse the true edge's indexed-store proof"
    );
    for store_first in [false, true] {
        if store_first {
            // Keep canonical BlockId order while putting the store before its
            // execution predecessor in storage. Operation/proof identities stay fixed.
            let machine = &mut module.machines[0];
            let renamed = |block| {
                if block == id(101) {
                    id(102)
                } else if block == id(102) {
                    id(101)
                } else {
                    block
                }
            };
            machine.entry = renamed(machine.entry);
            for block in &mut machine.blocks {
                block.id = renamed(block.id);
                if let Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } = &mut block.terminator
                {
                    when_true.target = renamed(when_true.target);
                    when_false.target = renamed(when_false.target);
                }
            }
            machine.blocks.sort_by_key(|block| block.id);
        }
        let semantics = encode_module(&module).unwrap();
        let proof_bytes = encode_proof_bundle(&proof).unwrap();
        terminal_verifier::verify_module(
            &decode_module(&semantics).unwrap(),
            &decode_proof_bundle(&proof_bytes).unwrap(),
            &profile,
        )
        .expect("canonical byte operation is valid before native rejection");
        let expected = if store_first {
            LoweringError::UnsupportedStructuralByteSequenceFieldByteStore(id(105))
        } else {
            LoweringError::UnsupportedStructuralByteSequenceFieldLength(id(101))
        };
        for result in [
            lower_artifact_sections(&semantics, &proof_bytes, &profile).map(|_| ()),
            lower_artifact_sections_for_optimization(&semantics, &proof_bytes, &profile)
                .map(|_| ()),
            lower_artifact_sections_for_native_realization(&semantics, &proof_bytes, &profile)
                .map(|_| ()),
        ] {
            assert!(
                matches!(result, Err(ArtifactLoweringError::Lowering(error)) if error == expected)
            );
        }
    }
}

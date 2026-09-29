//! An element view bound by a block parameter measures the extent of the
//! producer whose exact descriptor every arrival transfers. The verifier
//! reconstructs that relation from dominance and the edge arguments alone:
//! a different view on one arrival, a producer that does not dominate the read,
//! or a mutable rebinding the producer never authorized supplies no equation.

use super::super::{
    CertificateEnvelope, EvidenceIdentity, EvidenceRoute, IntegerValue, ObligationEvidence,
    PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker, Proposition, ScalarTerm,
    reconstruct_interpretable_operation_obligations, validate_module_for_interpretation,
};
use super::{
    AdmissionProfile, BlockId, EdgeId, IntegerSign, IntegerType, ModuleError, ObligationId,
    Operation, OperationId, OperationKind, OperationResult, PlaceId, ProofBundle, ScalarType,
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId,
    StructuralTypeShape, SuccessorEdge, TerminalModule, Terminator, ValueDeclaration, ValueId, id,
    ranked_countdown_with_width, verify_module_for_interpretation,
};
use terminal_psi::{Block, StructuralArgument, StructuralOperationResult};

const VIEW: u64 = 1;
const ELEMENT: u64 = 9;
const ARRAY: u64 = 10;

const ARRAY_PLACE: u64 = 1;
const PRODUCED: u64 = 2;
const ARRIVED: u64 = 3;
const SIGNATURE_VIEW: u64 = 5;

const SELECTOR: u64 = 20;
const INDEX: u64 = 2;
const LENGTH: u64 = 3;

const ENTRY: u64 = 1;
const READ: u64 = 2;
const LEFT: u64 = 6;
const RIGHT: u64 = 7;

fn count() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn value(raw: u64) -> ScalarTerm {
    ScalarTerm::value(id(raw, ValueId::new), ScalarType::Integer(count()))
}

fn literal(raw: u128) -> ScalarTerm {
    ScalarTerm::integer(count(), IntegerValue::Unsigned(raw)).unwrap()
}

fn view_argument(place: u64, access: StructuralAccess) -> StructuralArgument {
    StructuralArgument {
        place: id(place, PlaceId::new),
        path: Vec::new(),
        access,
    }
}

fn view_parameter(
    place: u64,
    position: u32,
    access: StructuralAccess,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id(place, PlaceId::new),
        position,
        is_self: false,
        structural_type: id(VIEW, StructuralTypeId::new),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn establishment() -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(1, OperationId::new),
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: id(PRODUCED, PlaceId::new),
            structural_type: id(VIEW, StructuralTypeId::new),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishElementView {
            destination: id(PRODUCED, PlaceId::new),
            source: view_argument(ARRAY_PLACE, StructuralAccess::SharedBorrow),
            element: id(ELEMENT, StructuralTypeId::new),
        },
    }
}

fn scalar(operation: u64, result: u64, scalar_type: ScalarType, kind: OperationKind) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(operation, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(result, ValueId::new),
            scalar_type,
        }),
        kind,
    }
}

fn edge(raw: u64, target: u64, view: u64, access: StructuralAccess) -> SuccessorEdge {
    SuccessorEdge {
        edge: id(raw, EdgeId::new),
        target: id(target, BlockId::new),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![view_argument(view, access)],
        trivial_affine_discards: Vec::new(),
    }
}

fn jump(edge: SuccessorEdge) -> Terminator {
    Terminator::Jump {
        edge: edge.edge,
        target: edge.target,
        arguments: edge.arguments,
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: edge.structural_arguments,
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}

fn block(raw: u64, operations: Vec<Operation>, terminator: Terminator) -> Block {
    Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        id: id(raw, BlockId::new),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations,
        terminator,
    }
}

/// Entry establishes a view over a four-element array and transfers it to a
/// read block, whose `entries[0]` owes `0 < entries.len`.
fn arrival(access: StructuralAccess) -> TerminalModule {
    let mut module = ranked_countdown_with_width(64);
    let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(VIEW, StructuralTypeId::new),
            identity: "test::Elements".into(),
            shape: StructuralTypeShape::ElementView {
                element: id(ELEMENT, StructuralTypeId::new),
            },
        },
        StructuralTypeDeclaration {
            id: id(ELEMENT, StructuralTypeId::new),
            identity: "test::Element".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(byte)),
        },
        StructuralTypeDeclaration {
            id: id(ARRAY, StructuralTypeId::new),
            identity: "test::Array".into(),
            shape: StructuralTypeShape::FixedArray {
                element: id(ELEMENT, StructuralTypeId::new),
                length: 4,
            },
        },
    ];
    let machine = &mut module.machines[0];
    machine.ranked_scc = None;
    machine.entry = id(ENTRY, BlockId::new);
    machine.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: id(SELECTOR, ValueId::new),
        scalar_type: ScalarType::Boolean,
    }];
    machine.structural_parameters = vec![
        StructuralParameterDeclaration {
            structural_type: id(ARRAY, StructuralTypeId::new),
            ..view_parameter(ARRAY_PLACE, 0, StructuralAccess::SharedBorrow)
        },
        view_parameter(SIGNATURE_VIEW, 1, StructuralAccess::SharedBorrow),
    ];
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(ARRAY_PLACE, PlaceId::new),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: id(SIGNATURE_VIEW, PlaceId::new),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: id(PRODUCED, PlaceId::new),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(1, OperationId::new),
                structural_type: id(VIEW, StructuralTypeId::new),
            },
        },
        StructuralPlaceDeclaration {
            id: id(ARRIVED, PlaceId::new),
            kind: StructuralPlaceKind::BlockParameter {
                block: id(READ, BlockId::new),
                position: 0,
            },
        },
    ];
    let mut read = block(
        READ,
        vec![
            scalar(
                2,
                INDEX,
                ScalarType::Integer(count()),
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(0),
                },
            ),
            scalar(
                3,
                LENGTH,
                ScalarType::Integer(count()),
                OperationKind::ElementViewLength {
                    source: id(ARRIVED, PlaceId::new),
                },
            ),
            scalar(
                4,
                4,
                ScalarType::Integer(byte),
                OperationKind::ElementViewRead {
                    source: id(ARRIVED, PlaceId::new),
                    index: id(INDEX, ValueId::new),
                    length: id(LENGTH, ValueId::new),
                    obligation: id(1, ObligationId::new),
                    path: Vec::new(),
                },
            ),
        ],
        Terminator::ReturnUnit {
            edge: id(30, EdgeId::new),
            trivial_affine_discards: Vec::new(),
        },
    );
    read.structural_parameters = vec![view_parameter(ARRIVED, 0, access)];
    machine.blocks = vec![
        block(
            ENTRY,
            vec![establishment()],
            jump(edge(10, READ, PRODUCED, access)),
        ),
        read,
    ];
    module
}

/// Entry selects between two staging blocks that both jump to the read block.
/// `left_producer` places the establishment in the left staging block instead
/// of entry; `right_view` names what the right staging block transfers.
fn joined(left_producer: bool, right_view: u64) -> TerminalModule {
    let mut module = arrival(StructuralAccess::SharedBorrow);
    let machine = &mut module.machines[0];
    let read = machine.blocks.pop().expect("read block");
    let mut entry = machine.blocks.pop().expect("entry block");
    let mut left_operations = Vec::new();
    if left_producer {
        left_operations = std::mem::take(&mut entry.operations);
    }
    entry.terminator = Terminator::Conditional {
        condition: id(SELECTOR, ValueId::new),
        when_true: SuccessorEdge {
            structural_arguments: Vec::new(),
            ..edge(11, LEFT, PRODUCED, StructuralAccess::SharedBorrow)
        },
        when_false: SuccessorEdge {
            structural_arguments: Vec::new(),
            ..edge(12, RIGHT, PRODUCED, StructuralAccess::SharedBorrow)
        },
    };
    machine.blocks = vec![
        entry,
        block(
            LEFT,
            left_operations,
            jump(edge(13, READ, PRODUCED, StructuralAccess::SharedBorrow)),
        ),
        block(
            RIGHT,
            Vec::new(),
            jump(edge(14, READ, right_view, StructuralAccess::SharedBorrow)),
        ),
        read,
    ];
    module
}

fn read_axioms(module: &TerminalModule) -> Vec<Proposition> {
    reconstruct_interpretable_operation_obligations(
        validate_module_for_interpretation(module).expect("element view arrival validates"),
    )
    .unwrap()
    .iter()
    .find(|obligation| obligation.obligation.id == id(1, ObligationId::new))
    .expect("the read owes its bound")
    .semantic_axioms
    .clone()
}

fn arrival_extent() -> Proposition {
    Proposition::Equal(value(LENGTH), literal(4))
}

fn axiom(axioms: &[Proposition], conclusion: Proposition) -> ProofNode {
    let index = axioms
        .iter()
        .position(|candidate| *candidate == conclusion)
        .unwrap_or_else(|| panic!("missing exact axiom {conclusion:?} in {axioms:#?}"));
    ProofNode {
        conclusion,
        rule: ProofRule::SemanticAxiom { index },
    }
}

/// `0 < 4` closes by computation; the index and extent equations substitute
/// each literal endpoint for its exact value.
fn read_bound(module: &TerminalModule) -> ProofBundle {
    let axioms = read_axioms(module);
    let closed = ProofNode {
        conclusion: Proposition::LessThan(literal(0), literal(4)),
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    };
    let indexed = ProofNode {
        conclusion: Proposition::LessThan(value(INDEX), literal(4)),
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(closed),
            equality: Box::new(axiom(&axioms, Proposition::Equal(value(INDEX), literal(0)))),
            endpoint: 0,
        },
    };
    let bounded = ProofNode {
        conclusion: Proposition::LessThan(value(INDEX), value(LENGTH)),
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(indexed),
            equality: Box::new(axiom(&axioms, arrival_extent())),
            endpoint: 1,
        },
    };
    ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: id(1, ObligationId::new),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: bounded,
            }),
        }],
        ..ProofBundle::default()
    }
}

#[test]
fn arrival_view_measures_its_dominating_establishment() {
    let module = arrival(StructuralAccess::SharedBorrow);
    assert!(read_axioms(&module).contains(&arrival_extent()));
    verify_module_for_interpretation(&module, &read_bound(&module), &AdmissionProfile::default())
        .expect("the transferred extent discharges the read bound");
    let mut reordered = module.clone();
    reordered.machines[0].blocks.reverse();
    assert!(read_axioms(&reordered).contains(&arrival_extent()));
}

#[test]
fn arrival_extent_crosses_staging_arrivals_of_one_descriptor() {
    let module = joined(false, PRODUCED);
    assert!(read_axioms(&module).contains(&arrival_extent()));
    verify_module_for_interpretation(&module, &read_bound(&module), &AdmissionProfile::default())
        .expect("both arrivals transfer the dominating producer's descriptor");
}

#[test]
fn arrival_extent_rejects_a_different_view_on_one_arrival() {
    let module = joined(false, SIGNATURE_VIEW);
    assert!(!read_axioms(&module).contains(&arrival_extent()));
    assert!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
}

#[test]
fn arrival_extent_rejects_a_producer_that_does_not_dominate_the_read() {
    // The backward walk meets the producer on the left arrival and a machine
    // parameter on the right one. Only dominance rules the producer out.
    let module = joined(true, SIGNATURE_VIEW);
    assert!(!read_axioms(&module).contains(&arrival_extent()));
}

#[test]
fn arrival_extent_rejects_a_mutable_view_whose_length_a_writer_may_replace() {
    // Element-view length observations are immutable. A mutable arrival would
    // let a writer replace the extent the producer's equation describes, so its
    // length read is refused before any equation could be reconstructed. The
    // arrival rule's own freshness check guards any later mutable admission.
    let module = arrival(StructuralAccess::MutableBorrow);
    assert_eq!(
        validate_module_for_interpretation(&module).map(|_| ()),
        Err(ModuleError::InvalidElementViewLengthSource {
            operation: id(3, OperationId::new),
            source: id(ARRIVED, PlaceId::new),
        })
    );
}

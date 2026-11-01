use super::*;
use semantic_vocabulary::{
    ByteSequencePredicate, IntegerSign, IntegerType, IntegerValue, PlaceId, StructuralTypeId,
};
use std::slice::from_ref;
use terminal_psi::{StructuralTypeShape, TerminalIdentityIndex};

#[test]
fn byte_predicate_observations_require_complete_current_typed_contents() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let subject = PlaceId::new(1).unwrap();
    let declaration = |id, shape| StructuralTypeDeclaration {
        quotient_carrier: None,
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: false,
        descriptor_cleanup: true,
        shape,
        scalar_restriction: None,
    };
    let types = vec![
        declaration(
            scalar,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
        ),
        declaration(
            array,
            StructuralTypeShape::FixedArray {
                element: scalar,
                length: 2.into(),
            },
        ),
    ];
    let term = ScalarTerm::ByteSequencePredicate {
        predicate: ByteSequencePredicate::AsciiOnly,
        root: subject,
        path: Vec::new(),
    };
    let mut contract = PayloadMembershipContract {
        scalar_observations: [65u8, 66]
            .into_iter()
            .enumerate()
            .map(|(index, value)| PayloadScalarObservation {
                path: vec![DescriptorPathSegment::FixedIndex(index as u64)],
                value: Some(
                    ScalarTerm::integer(integer, IntegerValue::Unsigned(value.into())).unwrap(),
                ),
                entry: None,
            })
            .collect(),
        scalar_requirements: Vec::new().into(),
        carrier: array,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let classify = |contract: &PayloadMembershipContract| {
        observed_byte_predicate(&types, contract, subject, &term)
    };
    assert_eq!(classify(&contract), Some(true));
    assert_eq!(
        observed_byte_predicate(&types, &contract, PlaceId::new(2).unwrap(), &term),
        None
    );
    let mut missing = contract.clone();
    missing.scalar_observations.pop();
    assert_eq!(classify(&missing), None);
    let mut duplicate = contract.clone();
    duplicate
        .scalar_observations
        .push(duplicate.scalar_observations[0].clone());
    assert_eq!(classify(&duplicate), None);
    let mut wrong_type = contract.clone();
    wrong_type.scalar_observations[1].value = Some(ScalarTerm::Boolean(true));
    assert_eq!(classify(&wrong_type), None);
    contract.scalar_observations[1].value =
        Some(ScalarTerm::integer(integer, IntegerValue::Unsigned(255)).unwrap());
    assert_eq!(classify(&contract), Some(false));
}

#[test]
fn closure_gate_keeps_normalization_without_an_eligible_domain() {
    use semantic_vocabulary::StructuralDomainId;
    let first = StructuralDomainId::new(1).unwrap();
    let second = StructuralDomainId::new(2).unwrap();
    let mut contract = PayloadMembershipContract {
        scalar_observations: Vec::new(),
        scalar_requirements: Vec::new().into(),
        carrier: StructuralTypeId::new(1).unwrap(),
        qualifications: vec![second, first, first],
        projected_qualifications: Vec::new(),
    };
    assert!(closure_may_change(&[], &[], &contract));
    close(&[], &[], &mut contract);
    assert_eq!(contract.qualifications, vec![first, second]);
    assert!(!closure_may_change(&[], &[], &contract));
    let unchanged = contract.clone();
    close(&[], &[], &mut contract);
    assert_eq!(contract, unchanged);
}

#[test]
fn closure_gate_preserves_eligible_and_opaque_domain_behavior() {
    use semantic_vocabulary::{DomainSemanticId, StructuralDomainId};
    use terminal_psi::StructuralDomainPredicate;

    let carrier = StructuralTypeId::new(1).unwrap();
    let domain_id = StructuralDomainId::new(1).unwrap();
    let types = vec![StructuralTypeDeclaration {
        quotient_carrier: None,
        id: carrier,
        identity: TerminalIdentityIndex(1),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: false,
        descriptor_cleanup: true,
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        scalar_restriction: None,
    }];
    let domain = StructuralDomainDeclaration {
        required_memberships: Some(Vec::new()),
        complete_predicate_theory: true,
        predicate: Some(StructuralDomainPredicate {
            subject: PlaceId::new(1).unwrap(),
            proposition: Proposition::Truth,
        }),
        id: domain_id,
        semantic_domain: DomainSemanticId::new(1).unwrap(),
        identity: TerminalIdentityIndex(2),
        carrier,
        content_projection: None,
        byte_predicate: None,
        establishment_routes: Vec::new(),
    };
    let empty = PayloadMembershipContract {
        scalar_observations: Vec::new(),
        scalar_requirements: Vec::new().into(),
        carrier,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let mut contract = empty.clone();
    assert!(closure_may_change(&types, from_ref(&domain), &contract));
    close(&types, from_ref(&domain), &mut contract);
    assert_eq!(contract.qualifications, vec![domain_id]);
    assert!(!closure_may_change(&types, from_ref(&domain), &contract));

    let mut opaque = domain.clone();
    opaque.predicate = None;
    let mut unknown = domain.clone();
    unknown.required_memberships = None;
    for excluded in [opaque, unknown] {
        let mut contract = empty.clone();
        let domains = from_ref(&excluded);
        assert!(!closure_may_change(&types, domains, &contract));
        close(&types, domains, &mut contract);
        assert_eq!(contract, empty);
    }
    // Eligibility must not substitute for proving the predicate.
    let mut unprovable = domain;
    unprovable.predicate.as_mut().unwrap().proposition = Proposition::Falsehood;
    let domains = from_ref(&unprovable);
    let mut contract = empty.clone();
    assert!(closure_may_change(&types, domains, &contract));
    close(&types, domains, &mut contract);
    assert_eq!(contract, empty);
}

fn scalar_constant_query_machine(id: u64) -> terminal_psi::TerminalMachine {
    use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId};
    use terminal_psi::{Block, TerminalMachine, TerminalMachineResult, Terminator};
    let block = BlockId::new(1).unwrap();
    TerminalMachine {
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        id: MachineId::new(id).unwrap(),
        declaration: None,
        attachment: None,
        nominal_cleanup_subject: None,
        parameters: Vec::new(),
        parameter_order: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block,
        blocks: vec![Block::plain(
            block,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        )],
        contract: terminal_psi::MachineContract::empty(ContractId::new(1).unwrap()),
    }
}

#[test]
fn scalar_constant_queries_index_matches_original_graph_observations() {
    use semantic_vocabulary::{
        BlockId, EdgeId, OperationId, ScalarQualificationSetId, StructuralCaseId,
    };
    use terminal_psi::{
        Block, Operation, OperationKind, OperationResult, StructuralCaseSuccessorEdge, Terminator,
        ValueDeclaration,
    };
    let id = |n| ValueId::new(n).unwrap();
    let block_id = |n| BlockId::new(n).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let declaration = |n, scalar_type| ValueDeclaration {
        id: id(n),
        scalar_type,
        qualifications: ScalarQualificationSetId::ZERO,
    };
    let parameter = |n| declaration(n, ScalarType::Integer(integer));
    let jump = |target, arguments| Terminator::Jump {
        edge: EdgeId::new(target).unwrap(),
        target: block_id(target),
        arguments,
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    let mut machine = scalar_constant_query_machine(1);
    machine.blocks[0].parameters.push(parameter(1));
    machine.blocks[0].operations = vec![
        Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Scalar(parameter(100)),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(7),
            },
        ),
        Operation::unsuspended(
            OperationId::new(2).unwrap(),
            OperationResult::Scalar(parameter(102)),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(9),
            },
        ),
        Operation::unsuspended(
            OperationId::new(3).unwrap(),
            OperationResult::Scalar(declaration(101, ScalarType::Boolean)),
            OperationKind::BooleanConstant { value: true },
        ),
    ];
    machine.blocks[0].terminator = jump(2, vec![id(100)]);
    for (number, parameter_id, target, arguments) in [
        (2, 2, 3, vec![id(2)]),
        (3, 3, 4, vec![id(3)]),
        (5, 5, 4, vec![id(102)]),
        (8, 8, 7, Vec::new()),
        (9, 9, 10, vec![id(9)]),
        (10, 10, 9, vec![id(10)]),
    ] {
        let mut block = Block::plain(block_id(number), Vec::new(), jump(target, arguments));
        block.parameters.push(parameter(parameter_id));
        machine.blocks.push(block);
    }
    for (number, parameter_id) in [(4, 4), (6, 6), (7, 7), (11, 11), (13, u64::MAX)] {
        let mut block = Block::plain(
            block_id(number),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(number).unwrap(),
                cleanup_actions: Vec::new(),
            },
        );
        block.parameters.push(parameter(parameter_id));
        machine.blocks.push(block);
    }
    machine.blocks.push(Block::plain(
        block_id(12),
        Vec::new(),
        Terminator::StructuralCase {
            source: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            cases: vec![StructuralCaseSuccessorEdge {
                edge: EdgeId::new(12).unwrap(),
                target: block_id(11),
                case: StructuralCaseId::new(1).unwrap(),
                payload_fields: Vec::new(),
                structural_arguments: Vec::new(),
                cleanup_actions: Vec::new(),
            }],
        },
    ));
    for roots in [
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 100, 101, 102, u64::MAX],
        vec![u64::MAX, 102, 101, 100, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1],
    ] {
        let queries = TerminalScalarExpressions::new(&machine);
        for value in roots {
            assert_eq!(
                queries.literal(id(value)),
                terminal_scalar_literal(&machine, id(value)),
                "value {value}"
            );
        }
    }
    let queries = TerminalScalarExpressions::new(&machine);
    assert_eq!(
        queries.literal(id(2)),
        Some(ScalarTerm::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(7)
        })
    );
    for root in [1, 4, 6, 7, 9, 10, 11, u64::MAX] {
        assert_eq!(queries.literal(id(root)), None);
    }
}

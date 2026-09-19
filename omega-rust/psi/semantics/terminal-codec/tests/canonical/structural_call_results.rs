use super::{
    i32_type, multi_claim_structural_call_fixture, structural_call_fixture,
    structural_effect_fixture,
};
use crate::canonical::{
    claim_id, operation_id, structural_domain_id, structural_type_id, value_id,
};
use semantic_vocabulary::{ScalarType, StructuralPlaceKind};
use terminal_codec::{CodecError, decode_module, encode_module, semantic_fingerprint};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralDomainDeclaration, StructuralPathSegment,
    TerminalMachineResult, Terminator, ValueDeclaration, VocabularyMarker,
};

#[test]
fn structural_call_result_round_trips_with_current_format_and_vocabulary() {
    let module = structural_call_fixture();
    let bytes = encode_module(&module).expect("structural call should encode");

    assert_eq!(&bytes[8..10], 102_u16.to_le_bytes());
    assert_eq!(
        &bytes[10..12],
        VocabularyMarker::CURRENT.get().to_le_bytes()
    );
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
}

#[test]
fn structural_call_multi_claim_map_round_trips_and_rejects_inexact_maps() {
    let module = multi_claim_structural_call_fixture();
    let bytes = encode_module(&module).expect("multi-claim structural call should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    let expected = Err(CodecError::MalformedStructuralFoundation(
        "structural call returned claims disagree with its result bindings",
    ));

    let mut swapped_paths = module.clone();
    let OperationKind::CallStructural {
        returned_claim_transfers,
        ..
    } = &mut swapped_paths.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let first_caller = returned_claim_transfers[0].caller_claim;
    returned_claim_transfers[0].caller_claim = returned_claim_transfers[1].caller_claim;
    returned_claim_transfers[1].caller_claim = first_caller;
    assert_eq!(encode_module(&swapped_paths), expected);

    let mut missing_return = module.clone();
    let OperationKind::CallStructural {
        returned_claim_transfers,
        ..
    } = &mut missing_return.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    returned_claim_transfers.pop();
    assert_eq!(encode_module(&missing_return), expected);

    let mut duplicate_caller = module.clone();
    let OperationKind::CallStructural {
        returned_claim_transfers,
        ..
    } = &mut duplicate_caller.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    returned_claim_transfers[1].caller_claim = claim_id(1);
    assert_eq!(encode_module(&duplicate_caller), expected);

    let mut incomplete_callee_return = module;
    let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut incomplete_callee_return.machines[1].blocks[0].terminator
    else {
        unreachable!()
    };
    returned_claims.pop();
    assert_eq!(
        encode_module(&incomplete_callee_return),
        Err(CodecError::MalformedStructuralFoundation(
            "structural callee return does not preserve its exact entry claim map"
        ))
    );
}

#[test]
fn structural_call_rows_require_canonical_claim_and_qualification_order() {
    let baseline = structural_call_fixture();

    let mut duplicate_result_claim = baseline.clone();
    let OperationResult::Structural(result) =
        &mut duplicate_result_claim.machines[0].blocks[0].operations[0].result
    else {
        unreachable!()
    };
    result.claims.push(result.claims[0].clone());
    assert_eq!(
        encode_module(&duplicate_result_claim),
        Err(CodecError::NonCanonicalOrder(
            "structural operation result claims by ClaimId"
        ))
    );

    let mut duplicate_argument_transfer = baseline.clone();
    let OperationKind::CallStructural {
        claim_transfers, ..
    } = &mut duplicate_argument_transfer.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    claim_transfers.push(claim_transfers[0]);
    assert_eq!(
        encode_module(&duplicate_argument_transfer),
        Err(CodecError::NonCanonicalOrder(
            "call claim transfers by claim and argument index"
        ))
    );

    let mut duplicate_returned_transfer = baseline.clone();
    let OperationKind::CallStructural {
        returned_claim_transfers,
        ..
    } = &mut duplicate_returned_transfer.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    returned_claim_transfers.push(returned_claim_transfers[0]);
    assert_eq!(
        encode_module(&duplicate_returned_transfer),
        Err(CodecError::NonCanonicalOrder(
            "structural-call returned claim transfers"
        ))
    );

    let mut unordered_qualifications = baseline;
    unordered_qualifications
        .structural_domains
        .push(StructuralDomainDeclaration {
            id: structural_domain_id(2),
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(2).unwrap(),
            identity: "example::Occurrence::Retained".to_owned(),
            carrier: structural_type_id(1),
            content_projection: None,
        });
    let OperationResult::Structural(result) =
        &mut unordered_qualifications.machines[0].blocks[0].operations[0].result
    else {
        unreachable!()
    };
    result.qualifications = vec![structural_domain_id(2), structural_domain_id(1)];
    assert_eq!(
        encode_module(&unordered_qualifications),
        Err(CodecError::NonCanonicalOrder(
            "structural operation result qualifications by StructuralDomainId"
        ))
    );
}

#[test]
fn structural_call_result_place_must_name_its_exact_producer_and_type() {
    let baseline = structural_call_fixture();

    let mut wrong_producer = baseline.clone();
    let StructuralPlaceKind::OperationResult { producer, .. } =
        &mut wrong_producer.machines[0].structural_places[1].kind
    else {
        unreachable!()
    };
    *producer = operation_id(2);
    assert!(matches!(
        encode_module(&wrong_producer),
        Err(CodecError::MalformedStructuralFoundation(_))
    ));

    let mut wrong_type = baseline;
    let StructuralPlaceKind::OperationResult {
        structural_type, ..
    } = &mut wrong_type.machines[0].structural_places[1].kind
    else {
        unreachable!()
    };
    *structural_type = structural_type_id(2);
    assert_eq!(
        encode_module(&wrong_type),
        Err(CodecError::MalformedStructuralFoundation(
            "structural call result place disagrees with its producer"
        ))
    );
}

#[test]
fn structural_call_result_paths_round_trip_and_missing_call_custody_fails_closed() {
    let mut module = structural_call_fixture();
    let root = module.structural_types[0].id;
    let leaf = structural_type_id(2);
    let domain = structural_domain_id(2);
    module.structural_domains.push(StructuralDomainDeclaration {
        id: domain,
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(2).unwrap(),
        identity: "RegionPayload::Ready".into(),
        carrier: leaf,
        content_projection: None,
    });
    let row = terminal_psi::StructuralPathQualification {
        path: vec![StructuralPathSegment::Field("metadata".into())],
        domain,
    };
    for machine in &mut module.machines {
        machine.structural_parameters[0].projected_qualifications = vec![row.clone()];
        let TerminalMachineResult::Structural(result) = &mut machine.result else {
            panic!("fixture machines return structural values")
        };
        assert_eq!(result.structural_type, root);
        result.projected_qualifications = vec![row.clone()];
    }
    let OperationResult::Structural(call_result) =
        &mut module.machines[0].blocks[0].operations[0].result
    else {
        panic!("fixture call returns a structural value")
    };
    call_result.projected_qualifications = vec![row];

    let encoded = encode_module(&module).expect("projected result custody encodes");
    assert_eq!(decode_module(&encoded), Ok(module.clone()));

    let mut truncated = encoded;
    truncated.pop();
    assert!(decode_module(&truncated).is_err());

    let OperationResult::Structural(call_result) =
        &mut module.machines[0].blocks[0].operations[0].result
    else {
        unreachable!()
    };
    call_result.projected_qualifications.clear();
    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::StructuralCallTargetMismatch { .. }
        ))
    ));
}

#[test]
fn boundary_scalar_parameter_and_argument_order_round_trips_canonically() {
    let mut module = structural_effect_fixture();
    let first = value_id(1);
    let second = value_id(2);
    module.boundary_machines[0]
        .parameter_order
        .splice(0..0, [terminal_psi::BoundaryParameterKind::Scalar; 2]);
    module.boundary_machines[0].scalar_parameters = vec![ScalarType::Boolean; 2];
    let operations = &mut module.machines[1].blocks[0].operations;
    operations[0].id = operation_id(4);
    operations[1].id = operation_id(5);
    operations.splice(
        0..0,
        [
            Operation {
                static_reach_binding: None,
                id: operation_id(2),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: first,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanConstant { value: false },
            },
            Operation {
                static_reach_binding: None,
                id: operation_id(3),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: second,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanConstant { value: true },
            },
        ],
    );
    let OperationKind::BoundaryCall { arguments, .. } =
        &mut module.machines[1].blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    *arguments = vec![second, first];

    let bytes = encode_module(&module).expect("scalar boundary lanes should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    let mut reordered = module;
    let OperationKind::BoundaryCall { arguments, .. } =
        &mut reordered.machines[1].blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    arguments.swap(0, 1);
    assert_ne!(
        semantic_fingerprint(&reordered).unwrap(),
        semantic_fingerprint(&decode_module(&bytes).unwrap()).unwrap(),
        "boundary scalar argument order is semantic",
    );
}

#[test]
fn structural_foundation_rejects_wrong_boundary_scalar_arity() {
    let mut module = structural_effect_fixture();
    module.boundary_machines[0]
        .parameter_order
        .insert(0, terminal_psi::BoundaryParameterKind::Scalar);
    module.boundary_machines[0].scalar_parameters = vec![ScalarType::Integer(i32_type())];

    assert_eq!(
        encode_module(&module),
        Err(CodecError::MalformedStructuralFoundation(
            "boundary call has the wrong scalar arity"
        )),
    );
}

//! Array presentation remains a Unit-call contract even with a coherent scalar ABI.

use super::*;
use legalized_operations::{LegalizedCallUnitParameter, LegalizedStructuralContract};
use semantic_vocabulary::{
    BlockId, IntegerType, MachineId, OperationId, PlaceId, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

fn fixed_array_call(
    target: target::NativeTarget,
) -> (LegalizedScalarFunction, LegalizedScalarCall) {
    let place = PlaceId::new(1).unwrap();
    let array_type = StructuralTypeId::new(1).unwrap();
    let byte_type = StructuralTypeId::new(2).unwrap();
    let view_type = StructuralTypeId::new(3).unwrap();
    let root_shape = ValueShape::borrowed_reference(17, 1);
    let view_shape = ValueShape::borrowed_reference(16, 8);
    let policy = CallingPolicy::native_for_target(target);
    let incoming = evaluate_call_plan(
        policy,
        &CallSignature {
            parameters: vec![root_shape],
            result: None,
        },
    )
    .unwrap();
    let outgoing = evaluate_call_plan(
        policy,
        &CallSignature {
            parameters: vec![view_shape],
            result: None,
        },
    )
    .unwrap();
    let placement = incoming.parameters[0].clone();
    let source = LegalizedScalarFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        ranked: None,
        provenance: target_operations::TerminalPsiProvenance {
            operations: Vec::new(),
            edges: Vec::new(),
        },
        call_plan: incoming,
        parameters: Vec::new(),
        entry_block: BlockId::new(1).unwrap(),
        // This test supplies only the signature consumed by the argument reader;
        // it makes no claim of graph or source admission.
        blocks: Vec::new(),
        structural: Some(LegalizedStructuralContract {
            structural_types: vec![
                StructuralTypeDeclaration {
                    id: array_type,
                    identity: "array".into(),
                    shape: StructuralTypeShape::FixedArray {
                        element: byte_type,
                        length: 17,
                    },
                },
                StructuralTypeDeclaration {
                    id: byte_type,
                    identity: "byte".into(),
                    shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    )),
                },
                StructuralTypeDeclaration {
                    id: view_type,
                    identity: "view".into(),
                    shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
                },
            ],
            parameters: vec![LegalizedCallUnitParameter {
                semantic: StructuralParameterDeclaration {
                    place,
                    position: 0,
                    is_self: false,
                    structural_type: array_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::MutableBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
                target: target_operations::TargetStructuralParameter {
                    place,
                    structural_type: array_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::MutableBorrow,
                    projected_qualifications: Vec::new(),
                    shape: root_shape,
                    placement: placement.clone(),
                },
            }],
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
        }),
    };
    let call = LegalizedScalarCall {
        source: legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit,
        callee: MachineId::new(2).unwrap(),
        arguments: vec![LegalizedScalarArgument::Structural {
            semantic: StructuralArgument {
                place,
                access: StructuralAccess::MutableBorrow,
                path: Vec::new(),
            },
            target: target_operations::TargetStructuralArgument {
                place,
                access: StructuralAccess::MutableBorrow,
                path: Vec::new(),
                root_structural_type: array_type,
                structural_type: view_type,
                shape: view_shape,
                source_byte_offset: 0,
                fixed_array_length: Some(17),
                element_stride: Some(1),
                source: placement.into(),
                destination: outgoing.parameters[0].clone(),
            },
        }],
        call_plan: outgoing,
        result_placement: None,
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    (source, call)
}

#[test]
fn fixed_array_view_rejects_coherent_scalar_result_calls() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let (source, mut call) = fixed_array_call(target);
        let operation = OperationId::new(1).unwrap();
        call.validate_shape().expect("coherent Unit call");
        assert_eq!(
            validate_borrowed_argument(&source, &call, operation),
            Some(())
        );

        call.call_plan = evaluate_call_plan(
            source.call_plan.policy,
            &CallSignature {
                parameters: vec![ValueShape::borrowed_reference(16, 8)],
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap();
        call.result_placement = call.call_plan.result.clone();
        let LegalizedScalarArgument::Structural {
            target: argument, ..
        } = &mut call.arguments[0]
        else {
            panic!("structural argument");
        };
        argument.destination = call.call_plan.parameters[0].clone();
        call.validate_shape().expect("coherent scalar-result call");
        assert_eq!(validate_borrowed_argument(&source, &call, operation), None);
    }
}

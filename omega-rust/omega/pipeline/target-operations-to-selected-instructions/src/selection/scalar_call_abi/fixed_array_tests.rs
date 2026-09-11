//! Exact array argument placements and borrowed array presentation contracts.

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
            result: None,
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
            ]
            .into(),
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
        structural_result: None,
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
            validate_borrowed_argument(&source, &call, operation, 0),
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
        assert_eq!(
            validate_borrowed_argument(&source, &call, operation, 0),
            None
        );
    }
}

#[test]
fn inline_array_arguments_do_not_admit_stack_or_indirect_results() {
    use crate::selection::aggregate_result_input::{direct_fragments, inline_argument_fragments};
    for (policy, prefix) in [
        (CallingPolicy::SystemVAMD64, 6),
        (CallingPolicy::Aapcs64, 8),
        (CallingPolicy::MicrosoftX64, 4),
    ] {
        for bytes in 1..=24 {
            let shape = ValueShape::integer(bytes, 1);
            let call = evaluate_call_plan(
                policy,
                &CallSignature {
                    parameters: vec![ValueShape::integer(8, 8); prefix]
                        .into_iter()
                        .chain([shape])
                        .collect(),
                    result: None,
                },
            )
            .unwrap();
            let placement = &call.parameters[prefix];
            let inline = match policy {
                CallingPolicy::SystemVAMD64 => true,
                CallingPolicy::Aapcs64 => bytes <= 16,
                CallingPolicy::MicrosoftX64 => matches!(bytes, 1 | 2 | 4 | 8),
                _ => unreachable!(),
            };
            assert_eq!(
                inline_argument_fragments(placement),
                inline,
                "{policy:?} {bytes}"
            );
            assert!(
                !direct_fragments(placement),
                "argument stack bytes are not result registers"
            );
        }
    }
    let mut placement = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(23, 1)],
            result: None,
        },
    )
    .unwrap()
    .parameters
    .remove(0);
    assert!(inline_argument_fragments(&placement));
    for mutation in 0..6 {
        let mut changed = placement.clone();
        let ValueLocation::Stack {
            stack_byte_offset,
            value_byte_offset,
            byte_size,
            alignment,
        } = &mut changed.locations[1]
        else {
            panic!("stack fragment");
        };
        match mutation {
            0 => *stack_byte_offset += 8,
            1 => *value_byte_offset += 1,
            2 => *byte_size = 0,
            3 => *byte_size = 9,
            4 => *alignment = 0,
            5 => *alignment = 3,
            _ => unreachable!(),
        }
        assert!(!inline_argument_fragments(&changed), "mutation {mutation}");
    }
    placement.locations.pop();
    assert!(!inline_argument_fragments(&placement));
}

#[test]
fn inline_stack_arrays_do_not_consume_call_register_operands() {
    for (target, prefix, bytes, expected_registers) in [
        (target::NativeTarget::linux_x64(), 5, 16, 6),
        (target::NativeTarget::linux_arm64(), 7, 16, 7),
        (target::NativeTarget::windows_x64(), 4, 8, 4),
    ] {
        let (_, mut call) = fixed_array_call(target);
        let mut array = call.arguments[0].clone();
        let plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); prefix]
                    .into_iter()
                    .chain([ValueShape::integer(bytes, 1), ValueShape::integer(8, 8)])
                    .collect(),
                result: None,
            },
        )
        .unwrap();
        let LegalizedScalarArgument::Structural {
            semantic,
            target: argument,
        } = &mut array
        else {
            panic!("array argument");
        };
        semantic.access = StructuralAccess::Owned;
        argument.access = StructuralAccess::Owned;
        argument.destination = plan.parameters[prefix].clone();
        argument.shape = argument.destination.shape;
        // This tests physical operand projection, not semantic source admission.
        call.arguments = plan
            .parameters
            .iter()
            .enumerate()
            .map(|(position, placement)| {
                if position == prefix {
                    array.clone()
                } else {
                    LegalizedScalarArgument::Scalar {
                        source: ValueId::new(u64::try_from(position + 1).unwrap()).unwrap(),
                        placement: placement.clone(),
                    }
                }
            })
            .collect();
        call.call_plan = plan;
        assert_eq!(register_argument_count(&call), expected_registers);
        let order = register_argument_order(&call);
        assert!(!order.contains(&prefix));
        assert_eq!(
            order.contains(&(prefix + 1)),
            target == target::NativeTarget::linux_x64()
        );
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let key = unit_key(&call, &environment).expect("existing ordinary call row");
        let row = environment.constraint(key).unwrap();
        assert_eq!(row.operands.len(), expected_registers);
        let expected_views = call
            .call_plan
            .parameters
            .iter()
            .flat_map(|placement| &placement.locations)
            .filter_map(|location| match location {
                ValueLocation::Register { register, .. } => {
                    environment.fixed_register_view(*register)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            row.operands
                .iter()
                .map(|operand| operand.fixed_view.unwrap())
                .collect::<Vec<_>>(),
            expected_views
        );
    }
}

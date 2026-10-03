//! A call result and an unrelated operand cannot select a different borrow ABI.

use super::{
    AbstractFunctionResult, AbstractOperation, AbstractOperationPlan, AbstractStructuralCallResult,
    EdgeId, IntegerSign, IntegerType, IntegerValue, NativeTarget, OperationId, PlaceId, ScalarType,
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeId, TargetOperationPlan, TargetStructuralArgument, TargetUnitOperation, ValueId,
    indexed_element_borrow_plan, mutate_call_row,
};
use abstract_operations::AbstractResult;
use terminal_psi::{StructuralOperationResult, StructuralResultDeclaration};

#[test]
fn mixed_value_call_transport_replays_common_fields_and_payload() {
    let source = super::source_plan(
        r#"
        data Message { stamp: i32; case Empty; case Number(value: i32); }
        machine relay(message: Message) -> Message { message }
        machine forward(message: Message) -> Message { relay(message) }
        data Main { message: Message; }
        machine Main::run(main: &mut Main) {
            main.message = forward(Message::Number { stamp: 23, value: 47 });
        }
        "#,
    );
    for native in [
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
    ] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("mixed values use their declared storage at ordinary calls");
        crate::validate_abstract_to_target_translation(&source, native, &target)
            .expect("independent call and constructor replay");
        for mutation in 0..3 {
            let mut changed = target.clone();
            let argument = changed
                .functions
                .iter_mut()
                .flat_map(|function| &mut function.graph.blocks)
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| match operation {
                    TargetUnitOperation::Call { arguments, .. } => arguments.first_mut(),
                    _ => None,
                })
                .expect("owned call argument");
            match mutation {
                0 => argument.source_byte_offset += 1,
                1 => argument.shape.byte_size += 1,
                2 => argument.access = StructuralAccess::SharedBorrow,
                _ => unreachable!(),
            }
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "forged argument {mutation} on {native:?}"
            );
        }
    }
}

fn array_result(place: u64) -> StructuralOperationResult {
    StructuralOperationResult {
        place: PlaceId::new(place).unwrap(),
        structural_type: StructuralTypeId::new(2).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    }
}

fn with_owned_sibling() -> AbstractOperationPlan {
    let mut source = indexed_element_borrow_plan();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let local = PlaceId::new(10).unwrap();
    let AbstractOperation::CallStructural {
        structural_arguments,
        ..
    } = &mut source.functions[0].operations[0]
    else {
        panic!("projected borrow call");
    };
    structural_arguments.push(StructuralArgument {
        place: local,
        access: StructuralAccess::Owned,
        path: Vec::new(),
    });
    source.functions[0].operations.splice(
        0..0,
        [
            AbstractOperation::IntegerConstant {
                psi_operation: OperationId::new(10).unwrap(),
                result: ValueId::new(10).unwrap(),
                scalar_type: unsigned,
                value: IntegerValue::Unsigned(7),
            },
            AbstractOperation::EstablishScalarArray {
                psi_operation: OperationId::new(11).unwrap(),
                result: array_result(10),
                elements: vec![ValueId::new(10).unwrap(); 4],
            },
        ],
    );
    source.functions[1]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: PlaceId::new(20).unwrap(),
            position: 1,
            is_self: false,
            structural_type: StructuralTypeId::new(2).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    source
}

fn call_result(source: &mut AbstractOperationPlan) -> &mut AbstractStructuralCallResult {
    source.functions[0]
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            AbstractOperation::CallStructural { result, .. } => Some(result),
            _ => None,
        })
        .expect("one direct call")
}

fn arguments(target: &TargetOperationPlan) -> &[TargetStructuralArgument] {
    target
        .functions
        .iter()
        .flat_map(|function| &function.graph.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation {
            TargetUnitOperation::Call { arguments, .. } => Some(arguments.as_slice()),
            _ => None,
        })
        .expect("one retained direct call")
}

#[test]
fn projected_operand_transport_is_independent_of_result_and_owned_sibling() {
    // Four targeted compositions, not a product of every source/result shape:
    // adding one local operand and then changing only the result contract.
    let baseline = indexed_element_borrow_plan();
    let sibling = with_owned_sibling();
    let mut scalar = sibling.clone();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    scalar.functions[1].result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(22).unwrap(),
        scalar_type: unsigned,
    });
    scalar.functions[1].operations = vec![
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(21).unwrap(),
            result: ValueId::new(21).unwrap(),
            scalar_type: unsigned,
            value: IntegerValue::Unsigned(70),
        },
        AbstractOperation::Return {
            psi_edge: EdgeId::new(2).unwrap(),
            result: ValueId::new(22).unwrap(),
            value: ValueId::new(21).unwrap(),
            scalar_type: unsigned,
            cleanup_actions: Vec::new(),
        },
    ];
    *call_result(&mut scalar) = AbstractStructuralCallResult::Scalar(AbstractResult {
        value: ValueId::new(30).unwrap(),
        scalar_type: unsigned,
    });
    let mut structural = sibling.clone();
    structural.functions[1].result =
        AbstractFunctionResult::Structural(StructuralResultDeclaration {
            place: PlaceId::new(22).unwrap(),
            structural_type: StructuralTypeId::new(2).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        });
    structural.functions[1].operations = vec![AbstractOperation::ReturnStructural {
        psi_edge: EdgeId::new(2).unwrap(),
        source: PlaceId::new(20).unwrap(),
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }];
    *call_result(&mut structural) = AbstractStructuralCallResult::Structural {
        result: array_result(30),
        returned_claim_transfers: Vec::new(),
        selected_evidence: Vec::new(),
    };
    for native in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
        let baseline_target =
            crate::lower_to_target_operations(&baseline, crate::TargetLoweringRequest::new(native))
                .expect("baseline projected owned-array subloan");
        crate::validate_abstract_to_target_translation(&baseline, native, &baseline_target)
            .unwrap();
        let expected = &arguments(&baseline_target)[0];
        for (name, source) in [
            ("unit with sibling", &sibling),
            ("scalar with sibling", &scalar),
            ("structural with sibling", &structural),
        ] {
            let target = crate::lower_to_target_operations(
                source,
                crate::TargetLoweringRequest::new(native),
            )
            .unwrap_or_else(|error| panic!("{native:?} {name}: {error:?}"));
            crate::validate_abstract_to_target_translation(source, native, &target).unwrap();
            let actual = arguments(&target);
            assert_eq!(actual.len(), 2);
            let mut projection = actual[0].clone();
            // A hidden result pointer can legitimately shift outgoing ABI
            // placement; replay above checks that independently evaluated plan.
            projection.destination = expected.destination.clone();
            assert_eq!(
                &projection, expected,
                "{native:?} {name}: operand transport changed"
            );
            for change_source in [false, true] {
                let changed = mutate_call_row(
                    &target,
                    |operation| matches!(operation, TargetUnitOperation::Call { .. }),
                    |operation| {
                        let TargetUnitOperation::Call { arguments, .. } = operation else {
                            unreachable!()
                        };
                        if change_source {
                            arguments[0].place = arguments[1].place;
                            arguments[0].source = arguments[1].source.clone();
                        } else {
                            arguments[0].access = StructuralAccess::MutableBorrow;
                        }
                    },
                );
                assert!(
                    crate::validate_abstract_to_target_translation(source, native, &changed)
                        .is_err(),
                    "{native:?} {name}: substituted source={change_source}"
                );
            }
        }
    }
}

//! Raw selection/replay controls, not source admission or native execution claims.
use super::*;
use terminal_psi::{
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

fn projected_call(target: target::NativeTarget) -> LegalizedScalarFunction {
    let mut source = borrowed_calls::borrowed_call(target);
    let record = StructuralTypeId::new(2).unwrap();
    let root_shape = ValueShape::borrowed_reference(4, 2);
    let leaf_shape = ValueShape::borrowed_reference(2, 2);
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![root_shape],
            result: None,
        },
    )
    .unwrap();
    let signature = source.structural.as_mut().unwrap();
    signature.structural_types[0].shape = StructuralTypeShape::FixedArray {
        element: record,
        length: 2,
    };
    signature.structural_types.push(StructuralTypeDeclaration {
        id: record,
        identity: "Record".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                identity: "value".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
                )),
            }],
        },
    });
    let parameter = &mut signature.parameters[0];
    parameter.semantic.access = StructuralAccess::WriteOnlyBorrow;
    parameter.target.access = StructuralAccess::WriteOnlyBorrow;
    parameter.target.shape = root_shape;
    parameter.target.placement = source.call_plan.parameters[0].clone();
    let row = &mut source.blocks[0].instructions[0];
    row.result = None;
    let LegalizedScalarInstructionKind::Call(call) = &mut row.kind else {
        panic!("call fixture")
    };
    call.call_plan = evaluate_call_plan(
        source.call_plan.policy,
        &CallSignature {
            parameters: vec![leaf_shape],
            result: None,
        },
    )
    .unwrap();
    call.result_placement = None;
    let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0] else {
        panic!("structural argument")
    };
    semantic.access = StructuralAccess::WriteOnlyBorrow;
    semantic.path = vec![StructuralPathSegment::FixedIndex(1)];
    target.access = semantic.access;
    target.path = semantic.path.clone();
    target.structural_type = record;
    target.shape = leaf_shape;
    target.source_byte_offset = 2;
    target.source = parameter.target.placement.clone().into();
    target.destination = call.call_plan.parameters[0].clone();
    returned(&mut source.blocks[0]).value = LegalizedScalarReturnValue::Unit;
    source
}

#[test]
fn projected_write_only_call_replays_original_pointer_offset_and_contract() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        let source = projected_call(target);
        let construct = |source: &LegalizedScalarFunction| {
            build(
                0,
                source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        let selected = construct(&source).expect("exact projected write-only call selects");
        let validate = |source: &LegalizedScalarFunction, candidate: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                candidate,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source, &selected).unwrap();
        assert!(
            selected.memory_accesses.is_empty(),
            "pointer adjustment never reads or copies the referent"
        );
        assert!(selected.outgoing_arguments.is_empty());
        let address = selected.blocks[0]
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction.kind,
                    SelectedInstructionKind::AddressOffset { byte_offset: 2 }
                )
            })
            .expect("literal index becomes its exact byte offset");
        assert!(
            selected.blocks[0].instructions[address]
                .provenance
                .fuel
                .is_empty()
        );
        assert_eq!(selected.calls.len(), 1);

        for mutation in 0..10 {
            let mut candidate = selected.clone();
            match mutation {
                0 => {
                    candidate.blocks[0].instructions[address].kind =
                        SelectedInstructionKind::AddressOffset { byte_offset: 0 }
                }
                1 => {
                    candidate.blocks[0].instructions[address].kind =
                        SelectedInstructionKind::CopyI64
                }
                2 => candidate.blocks[0].instructions[address]
                    .operands
                    .swap(0, 1),
                3 => {
                    candidate.blocks[0].instructions[address].provenance.fuel =
                        source.blocks[0].instructions[0].fuel.clone()
                }
                4 => candidate.calls[0].effect.output += 1,
                mutation => {
                    let LegalizedScalarArgument::Structural { semantic, target } =
                        &mut candidate.calls[0].call.arguments[0]
                    else {
                        panic!("structural call")
                    };
                    match mutation {
                        5 => target.source_byte_offset = 0,
                        6 => semantic.path = vec![StructuralPathSegment::FixedIndex(0)],
                        7 => target.source = target.destination.clone().into(),
                        8 => {
                            semantic.access = StructuralAccess::MutableBorrow;
                            target.access = semantic.access;
                        }
                        _ => target.path = vec![StructuralPathSegment::FixedIndex(0)],
                    }
                }
            }
            assert!(
                validate(&source, &candidate).is_err(),
                "selected mutation {mutation}"
            );
        }

        for mutation in 0..7 {
            let mut changed = source.clone();
            let LegalizedScalarInstructionKind::Call(call) =
                &mut changed.blocks[0].instructions[0].kind
            else {
                panic!("call fixture")
            };
            let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0]
            else {
                panic!("structural call")
            };
            match mutation {
                0 => target.source_byte_offset = 0,
                1 => {
                    semantic.path = vec![StructuralPathSegment::FixedIndex(0)];
                    target.path = semantic.path.clone();
                }
                2 => target.source = target.destination.clone().into(),
                3 => {
                    semantic.access = StructuralAccess::MutableBorrow;
                    target.access = semantic.access;
                }
                4 => {
                    semantic.path = vec![StructuralPathSegment::FixedIndex(2)];
                    target.path = semantic.path.clone();
                    target.source_byte_offset = 4;
                }
                5 => target.structural_type = target.root_structural_type,
                _ => target.shape = ValueShape::integer(2, 2),
            }
            assert!(construct(&changed).is_err(), "source mutation {mutation}");
            assert!(
                validate(&changed, &selected).is_err(),
                "receiving mutation {mutation}"
            );
        }
    }
}

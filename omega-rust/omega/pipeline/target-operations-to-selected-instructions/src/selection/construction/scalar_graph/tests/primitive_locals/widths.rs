//! A primitive observation retains its exact footprint, pointer, type and fresh result.
use super::*;
use selected_instructions::SelectedMemoryAccessRole;
use semantic_vocabulary::IeeeFloatFormat;

fn typed_fixture(target: target::NativeTarget, scalar: ScalarType) -> LegalizedScalarFunction {
    let mut source = local_fixture(target, true);
    let shape = crate::selection::scalar_call_abi::scalar_shape(scalar).unwrap();
    source.structural.as_mut().unwrap().structural_types[0].shape =
        StructuralTypeShape::PrimitiveScalar(scalar);
    source.blocks[0].instructions[0]
        .result
        .as_mut()
        .unwrap()
        .scalar_type = scalar;
    source.blocks[0].instructions[0].kind = LegalizedScalarInstructionKind::Constant(
        if matches!(scalar, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed) {
            IntegerValue::Signed(-1)
        } else {
            IntegerValue::Unsigned(1)
        },
    );
    let LegalizedScalarInstructionKind::EstablishPrimitiveLocal {
        value,
        shape: local_shape,
        ..
    } = &mut source.blocks[0].instructions[1].kind
    else {
        panic!("local")
    };
    value.scalar_type = scalar;
    *local_shape = shape;
    let LegalizedScalarInstructionKind::Call(call) = &mut source.blocks[0].instructions[2].kind
    else {
        panic!("call")
    };
    let reference = ValueShape::borrowed_reference(shape.byte_size, shape.alignment);
    call.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![reference],
            result: None,
        },
    )
    .unwrap();
    let LegalizedScalarArgument::Structural {
        target: argument, ..
    } = &mut call.arguments[0]
    else {
        panic!("argument")
    };
    argument.shape = reference;
    argument.destination = call.call_plan.parameters[0].clone();
    source.blocks[0].instructions[3]
        .result
        .as_mut()
        .unwrap()
        .scalar_type = scalar;
    source
}

#[test]
fn primitive_read_width_and_definition_substitutions_reject_on_every_target() {
    let mut scalars = vec![
        ScalarType::Boolean,
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
    ];
    for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
        for width in [8, 16, 32, 64] {
            scalars.push(ScalarType::Integer(IntegerType::new(sign, width).unwrap()));
        }
    }
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
        for scalar in &scalars {
            let source = typed_fixture(target, *scalar);
            let selected = build(
                0,
                &source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let validate = |candidate: &SelectedFunction| {
                crate::selection::validation::scalar_graph::validate(
                    0,
                    &source,
                    candidate,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            validate(&selected).unwrap();
            let width = crate::selection::scalar_call_abi::scalar_shape(*scalar)
                .unwrap()
                .byte_size;
            let access = selected
                .memory_accesses
                .iter()
                .find(|access| access.role == SelectedMemoryAccessRole::ReadPlace)
                .unwrap();
            assert_eq!(access.byte_count, u32::from(width));
            for mutation in 0..6 {
                let mut changed = selected.clone();
                let instruction = changed
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.instructions)
                    .find(|row| row.id == access.instruction)
                    .unwrap();
                match mutation {
                    0 => {
                        instruction.kind = if width == 8 {
                            SelectedInstructionKind::Load32 { byte_offset: 0 }
                        } else {
                            SelectedInstructionKind::Load64 { byte_offset: 0 }
                        }
                    }
                    1 => instruction.operands.swap(0, 1),
                    2 => instruction.provenance.values = vec![ValueId::new(1).unwrap()],
                    3 => instruction.provenance.fuel.clear(),
                    4 => {
                        let result = instruction.operands[1].virtual_register;
                        changed
                            .virtual_registers
                            .iter_mut()
                            .find(|register| register.id == result)
                            .unwrap()
                            .scalar_type = if *scalar == ScalarType::Boolean {
                            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
                        } else {
                            ScalarType::Boolean
                        };
                    }
                    _ => {
                        changed
                            .memory_accesses
                            .iter_mut()
                            .find(|row| row.instruction == access.instruction)
                            .unwrap()
                            .byte_count += 1
                    }
                }
                assert!(
                    validate(&changed).is_err(),
                    "{target:?} {scalar:?} mutation {mutation}"
                );
            }
        }
    }
}

//! Independent selection replay controls; these raw fixtures do not claim source admission.
use super::*;
use semantic_vocabulary::IeeeFloatFormat;

fn fixture(target: target::NativeTarget, format: IeeeFloatFormat) -> LegalizedScalarFunction {
    let mut source = projected_borrows::projected_call(target);
    let scalar_type = ScalarType::IeeeFloat(format);
    let shape = crate::selection::scalar_call_abi::scalar_shape(scalar_type).unwrap();
    let root_shape = source.call_plan.parameters[0].shape;
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape, shape, root_shape],
            result: None,
        },
    )
    .unwrap();
    source.parameters = source.call_plan.parameters[..2]
        .iter()
        .enumerate()
        .map(|(position, placement)| LegalizedScalarParameter {
            value: ValueId::new(100 + position as u64).unwrap(),
            scalar_type,
            definition_site: ValueDefinitionSite::FunctionParameter(position as u32),
            placement: placement.clone(),
        })
        .collect();
    source.structural.as_mut().unwrap().parameters[0]
        .target
        .placement = source.call_plan.parameters[2].clone();
    let LegalizedScalarInstructionKind::Call(call) = &mut source.blocks[0].instructions[0].kind
    else {
        panic!("projected call")
    };
    let mut reference = call.arguments[0].clone();
    call.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape, shape, reference.placement().shape],
            result: None,
        },
    )
    .unwrap();
    let LegalizedScalarArgument::Structural {
        target: argument, ..
    } = &mut reference
    else {
        panic!("reference")
    };
    argument.source = source.call_plan.parameters[2].clone().into();
    argument.destination = call.call_plan.parameters[2].clone();
    // Reversed actuals make source identity observable independently of ABI position.
    call.arguments = vec![
        LegalizedScalarArgument::Scalar {
            source: source.parameters[1].value,
            placement: call.call_plan.parameters[0].clone(),
        },
        LegalizedScalarArgument::Scalar {
            source: source.parameters[0].value,
            placement: call.call_plan.parameters[1].clone(),
        },
        reference,
    ];
    source
}

#[test]
fn ieee_borrowed_calls_reject_substituted_sources_transfer_widths_and_fixed_views() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            let source = fixture(target, format);
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let constraints = SelectedSelectionConstraints {
                keys: environment.selected_keys(),
                fixed_inputs: source
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(parameter_index, parameter)| {
                        let [ValueLocation::Register { register, .. }] =
                            parameter.placement.locations.as_slice()
                        else {
                            panic!("register parameter")
                        };
                        SelectedFixedInputConstraint {
                            machine: source.machine,
                            source_value: parameter.value,
                            parameter_index,
                            register: *register,
                            fixed_view: environment.fixed_register_view(*register).unwrap(),
                        }
                    })
                    .collect(),
            };
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
            let validate = |source: &LegalizedScalarFunction, selected: &SelectedFunction| {
                crate::selection::validation::scalar_graph::validate(
                    0,
                    source,
                    selected,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            let selected = construct(&source).expect("genuine typed IEEE selection fixture");
            validate(&source, &selected).expect("unmodified selection independently replays");

            let mut substituted_source = source.clone();
            let LegalizedScalarInstructionKind::Call(call) =
                &mut substituted_source.blocks[0].instructions[0].kind
            else {
                panic!("call")
            };
            for (argument_index, argument) in call.arguments[..2].iter_mut().enumerate() {
                let LegalizedScalarArgument::Scalar { source: value, .. } = argument else {
                    panic!("scalar")
                };
                *value = source.parameters[argument_index].value;
            }
            let reordered = construct(&substituted_source)
                .expect("same-type source permutation remains a valid program");
            validate(&substituted_source, &reordered).unwrap();
            assert!(
                validate(&substituted_source, &selected).is_err(),
                "{target:?} {format:?}: original selected operands cannot witness reordered sources"
            );

            let mut wrong_contract = selected.clone();
            for (argument_index, argument) in wrong_contract.calls[0].call.arguments[..2]
                .iter_mut()
                .enumerate()
            {
                let LegalizedScalarArgument::Scalar { source: value, .. } = argument else {
                    panic!("scalar")
                };
                *value = source.parameters[argument_index].value;
            }
            assert!(
                validate(&source, &wrong_contract).is_err(),
                "retained call sources must match the executable transport"
            );

            let transfers = selected.blocks[0]
                .instructions
                .iter()
                .enumerate()
                .filter_map(|(position, row)| {
                    let (kind, key) = match row.kind {
                        SelectedInstructionKind::Float32ToBits => (
                            SelectedInstructionKind::Float64ToBits,
                            constraints.keys.float64_to_bits,
                        ),
                        SelectedInstructionKind::Float64ToBits => (
                            SelectedInstructionKind::Float32ToBits,
                            constraints.keys.float32_to_bits,
                        ),
                        SelectedInstructionKind::BitsToFloat32 => (
                            SelectedInstructionKind::BitsToFloat64,
                            constraints.keys.bits_to_float64,
                        ),
                        SelectedInstructionKind::BitsToFloat64 => (
                            SelectedInstructionKind::BitsToFloat32,
                            constraints.keys.bits_to_float32,
                        ),
                        _ => return None,
                    };
                    Some((position, kind, key.unwrap()))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                transfers.len(),
                4,
                "two incoming and two outgoing IEEE transfers"
            );
            for (position, kind, key) in transfers {
                let mut changed = selected.clone();
                changed.blocks[0].instructions[position].kind = kind;
                changed.blocks[0].instructions[position].constraint = key;
                assert!(
                    validate(&source, &changed).is_err(),
                    "{target:?} {format:?}: coherent transfer kind/key cannot change semantic width"
                );
            }
            let call_position = selected.blocks[0]
                .instructions
                .iter()
                .position(|row| matches!(row.kind, SelectedInstructionKind::CallUnit { .. }))
                .unwrap();
            let mut wrong_views = selected.clone();
            let operands = &mut wrong_views.blocks[0].instructions[call_position].operands;
            assert_eq!(operands.len(), 3);
            // GPR pointer is first; both FP arguments have the same class but distinct fixed views.
            let first = operands[1].fixed_view;
            operands[1].fixed_view = operands[2].fixed_view;
            operands[2].fixed_view = first;
            assert!(
                validate(&source, &wrong_views).is_err(),
                "same-class fixed ABI views are not interchangeable"
            );
        }
    }
}

//! Ordinary floating ABI rows retain exact semantic formats and target placements.

use super::{installed_scalar_abi_is_canonical, scalar_home_shape};
use calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, ValueLocation, ValueShape, evaluate_call_plan,
};
use semantic_vocabulary::{IeeeFloatFormat, ScalarType, ValueId};
use target::NativeTarget;
use target_operations::{ScalarAbiValue, ScalarFunctionAbi};

fn targets() -> [NativeTarget; 4] {
    [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ]
}

fn fixture(target: NativeTarget, format: IeeeFloatFormat) -> ScalarFunctionAbi {
    let shape = ValueShape::float(match format {
        IeeeFloatFormat::Binary32 => 4,
        IeeeFloatFormat::Binary64 => 8,
    });
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape, shape],
            result: Some(shape),
        },
    )
    .expect("canonical floating call plan");
    let scalar_type = ScalarType::IeeeFloat(format);
    ScalarFunctionAbi {
        parameters: call_plan
            .parameters
            .iter()
            .zip([1, 2])
            .map(|(placement, identity)| ScalarAbiValue {
                value: ValueId::new(identity).unwrap(),
                scalar_type,
                placement: placement.clone(),
            })
            .collect(),
        result: ScalarAbiValue {
            value: ValueId::new(3).unwrap(),
            scalar_type,
            placement: call_plan.result.clone().unwrap(),
        },
        call_plan,
    }
}

#[test]
fn floating_scalar_abi_accepts_exact_formats_without_widening_unit_homes() {
    for target in targets() {
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            let abi = fixture(target, format);
            assert!(installed_scalar_abi_is_canonical(&abi, target));
            assert_eq!(scalar_home_shape(ScalarType::IeeeFloat(format)), None);
        }
    }
}

#[test]
fn floating_scalar_abi_rejects_wrong_class_and_width_even_when_rows_agree() {
    for target in targets() {
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            let abi = fixture(target, format);
            let width = abi.result.placement.shape.byte_size;
            for shape in [
                ValueShape::integer(width, width),
                ValueShape::float(if width == 4 { 8 } else { 4 }),
            ] {
                let mut wrong_parameter = abi.clone();
                wrong_parameter.parameters[0].placement.shape = shape;
                wrong_parameter.call_plan.parameters[0].shape = shape;
                assert!(!installed_scalar_abi_is_canonical(&wrong_parameter, target));

                let mut wrong_result = abi.clone();
                wrong_result.result.placement.shape = shape;
                wrong_result.call_plan.result.as_mut().unwrap().shape = shape;
                assert!(!installed_scalar_abi_is_canonical(&wrong_result, target));
            }
        }
    }
}

fn replace_register(location: &mut ValueLocation) {
    let ValueLocation::Register { register, .. } = location else {
        panic!("fixture must use a floating register");
    };
    *register = match register {
        MachineRegister::X86Xmm(_) => MachineRegister::X86Xmm(7),
        MachineRegister::Aarch64V(_) => MachineRegister::Aarch64V(7),
        _ => panic!("fixture must use a floating register class"),
    };
}

#[test]
fn floating_scalar_abi_rejects_substituted_registers_even_when_rows_agree() {
    for target in targets() {
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            let abi = fixture(target, format);
            let mut wrong_parameter = abi.clone();
            replace_register(&mut wrong_parameter.parameters[0].placement.locations[0]);
            wrong_parameter.call_plan.parameters[0] =
                wrong_parameter.parameters[0].placement.clone();
            assert!(!installed_scalar_abi_is_canonical(&wrong_parameter, target));

            let mut wrong_result = abi;
            replace_register(&mut wrong_result.result.placement.locations[0]);
            wrong_result.call_plan.result = Some(wrong_result.result.placement.clone());
            assert!(!installed_scalar_abi_is_canonical(&wrong_result, target));
        }
    }
}

use crate::register_model::units;
use register_model::RegisterConstraintKey;
use register_model::RegisterUnitId;
use register_model::sorted_units;
use selected_instructions::IeeeFloatArithmeticForm;
use selected_instructions::IeeeFloatArithmeticOperation;
use selected_instructions::IeeeFloatIntegerCarrier;
use selected_instructions::IeeeFloatToIntegerDomain;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineBarrier;
use selected_instructions::MachineEffectDeclaration;
use selected_instructions::MachineEncodedControlEffect;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineEncodedTrapBehavior;
use selected_instructions::MachineMemoryEffect;
use selected_instructions::MachineSemanticKind;
use selected_instructions::MachineSizeKnowledge;
use selected_instructions::MachineTrapBehavior;
use selected_instructions::ieee_float_arithmetic_is_trapping;

pub fn implicit_uses() -> Vec<RegisterUnitId> {
    sorted_units(units("mxcsr"))
}

pub fn clobbers() -> Vec<RegisterUnitId> {
    sorted_units(units("xmm4").into_iter().chain(units("xmm5")))
}

pub fn conversion_control(form: IeeeFloatArithmeticForm) -> bool {
    matches!(
        form.operation,
        IeeeFloatArithmeticOperation::IsNan
            | IeeeFloatArithmeticOperation::IsInfinite
            | IeeeFloatArithmeticOperation::IsFinite
            | IeeeFloatArithmeticOperation::IsNormal
            | IeeeFloatArithmeticOperation::IsSubnormal
            | IeeeFloatArithmeticOperation::Classify
            | IeeeFloatArithmeticOperation::FromInteger(IeeeFloatIntegerCarrier::U64)
            | IeeeFloatArithmeticOperation::TruncateToInteger {
                target: IeeeFloatIntegerCarrier::U64,
                ..
            }
            | IeeeFloatArithmeticOperation::TruncateToInteger {
                domain: IeeeFloatToIntegerDomain::Trapping,
                ..
            }
    )
}

pub fn effects(form: IeeeFloatArithmeticForm) -> MachineEncodedEffects {
    let inputs = form.operand_count() as u16;
    let mut encoded = MachineEncodedEffects {
        implicit_unit_uses: implicit_uses(),
        implicit_unit_clobbers: clobbers(),
        ..MachineEncodedEffects::fallthrough_v1((0..inputs).collect(), vec![inputs])
    };
    if form.operation.rounding_direction().is_some() {
        let model = crate::x86_64_physical_register_model_ref();
        let stack_pointer = model.view_named("rsp").expect("canonical stack pointer").id;
        encoded.implicit_unit_uses =
            sorted_units(encoded.implicit_unit_uses.into_iter().chain(units("rsp")));
        encoded.memory =
            selected_instructions::MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
                stack_pointer,
                byte_count: 16,
            };
        encoded.stack = selected_instructions::MachineEncodedStackEffect::TemporaryBytesV1 {
            stack_pointer,
            byte_count: 16,
        };
        encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    }
    if conversion_control(form) {
        encoded.implicit_unit_clobbers =
            sorted_units(clobbers().into_iter().chain(units("condition_flags")));
    }
    if form.operand_count() == 3 {
        encoded.implicit_unit_clobbers = sorted_units(clobbers().into_iter().chain(units("xmm3")));
    }
    if ieee_float_arithmetic_is_trapping(form) {
        encoded.trap = MachineEncodedTrapBehavior::TrappingIntegerV1;
        encoded.control = MachineEncodedControlEffect::FallThroughOrTrapV1;
    }
    encoded
}

pub fn size(form: IeeeFloatArithmeticForm) -> MachineSizeKnowledge {
    crate::float_sizes::ieee_float_arithmetic_size(form)
}

pub fn declaration(
    form: IeeeFloatArithmeticForm,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration::single(
        MachineSemanticKind::NearestIeeeFloatArithmetic(form),
        constraint,
        if form.operation.rounding_direction().is_some() {
            MachineMemoryEffect::TemporaryStackStorageV1
        } else {
            MachineMemoryEffect::NoneV1
        },
        if form.operation.rounding_direction().is_some() {
            MachineTrapBehavior::MayArchitecturalFaultV1
        } else if ieee_float_arithmetic_is_trapping(form) {
            MachineTrapBehavior::TrappingIntegerV1
        } else {
            MachineTrapBehavior::NeverV1
        },
        if ieee_float_arithmetic_is_trapping(form) {
            MachineBarrier::ExternalEffect
        } else {
            MachineBarrier::None
        },
        MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
        size(form),
        effects(form),
    )
}

use register_model::RegisterConstraintKey;
use register_model::RegisterUnitId;
use register_model::sorted_units;
use selected_instructions::IeeeFloatArithmeticForm;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineBarrier;
use selected_instructions::MachineEffectDeclaration;
use selected_instructions::MachineEncodedControlEffect;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineEncodedTrapBehavior;
use selected_instructions::MachineMemoryEffect;
use selected_instructions::MachineSemanticKind;
use selected_instructions::MachineTrapBehavior;
use selected_instructions::ieee_float_arithmetic_is_trapping;

pub fn view_units(name: &str) -> Vec<RegisterUnitId> {
    crate::aarch64_physical_register_model_ref()
        .view_named(name)
        .unwrap_or_else(|| panic!("canonical AArch64 model declares {name}"))
        .units
        .clone()
}

pub fn implicit_uses() -> Vec<RegisterUnitId> {
    sorted_units(view_units("fpcr"))
}

pub fn clobbers() -> Vec<RegisterUnitId> {
    sorted_units(["q16", "q17", "q18"].into_iter().flat_map(view_units))
}

pub fn effects(form: IeeeFloatArithmeticForm) -> MachineEncodedEffects {
    let inputs = form.operand_count() as u16;
    let mut effects = MachineEncodedEffects {
        implicit_unit_uses: implicit_uses(),
        implicit_unit_clobbers: clobbers(),
        ..MachineEncodedEffects::fallthrough_v1((0..inputs).collect(), vec![inputs])
    };
    if ieee_float_arithmetic_is_trapping(form) {
        effects.trap = MachineEncodedTrapBehavior::TrappingIntegerV1;
        effects.control = MachineEncodedControlEffect::FallThroughOrTrapV1;
    }
    effects
}

pub fn declaration(
    form: IeeeFloatArithmeticForm,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration::single(
        MachineSemanticKind::NearestIeeeFloatArithmetic(form),
        constraint,
        MachineMemoryEffect::NoneV1,
        if ieee_float_arithmetic_is_trapping(form) {
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
        crate::float_sizes::ieee_float_arithmetic_size(form),
        effects(form),
    )
}

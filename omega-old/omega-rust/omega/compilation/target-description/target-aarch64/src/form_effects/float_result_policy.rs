use register_model::RegisterConstraintKey;
use register_model::RegisterUnitId;
use selected_instructions::FloatResultPolicyForm;
use selected_instructions::FloatResultPolicyKind;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineBarrier;
use selected_instructions::MachineEffectDeclaration;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineMemoryEffect;
use selected_instructions::MachineSemanticKind;
use selected_instructions::MachineTrapBehavior;

pub fn clobbers(form: FloatResultPolicyForm) -> Vec<RegisterUnitId> {
    let physical = crate::aarch64_physical_register_model_ref();
    let names: &[&str] = if form.can_trap() {
        &["q16", "nzcv"]
    } else {
        &["q16", "q17", "nzcv"]
    };
    let mut units = names
        .iter()
        .copied()
        .flat_map(|name| physical.views.iter().filter(move |view| view.name == name))
        .flat_map(|view| view.units.iter().copied())
        .collect::<Vec<_>>();
    units.sort_unstable();
    units.dedup();
    units
}

pub fn effects(form: FloatResultPolicyForm) -> MachineEncodedEffects {
    MachineEncodedEffects::float_result_policy_v1(
        form.operand_count() as u16,
        clobbers(form),
        form.can_trap(),
    )
}

pub fn declaration(
    form: FloatResultPolicyForm,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    let trapping = form.policy == FloatResultPolicyKind::Trapping;
    MachineEffectDeclaration::single(
        MachineSemanticKind::FloatResultPolicy(form),
        constraint,
        MachineMemoryEffect::NoneV1,
        if trapping {
            MachineTrapBehavior::TrappingIntegerV1
        } else {
            MachineTrapBehavior::NeverV1
        },
        if trapping {
            MachineBarrier::ExternalEffect
        } else {
            MachineBarrier::None
        },
        MachineAlternativeFamily::FloatResultPolicy(form),
        crate::float_sizes::float_result_policy_size(form),
        effects(form),
    )
}

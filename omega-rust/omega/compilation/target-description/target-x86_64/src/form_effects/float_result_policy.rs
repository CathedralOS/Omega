use crate::register_model::units_of;
use register_model::RegisterConstraintKey;
use register_model::RegisterUnitId;
use selected_instructions::FloatResultPolicyForm;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineBarrier;
use selected_instructions::MachineEffectDeclaration;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineMemoryEffect;
use selected_instructions::MachineSemanticKind;
use selected_instructions::MachineSizeKnowledge;
use selected_instructions::MachineTrapBehavior;

pub fn clobbers(form: FloatResultPolicyForm) -> Vec<RegisterUnitId> {
    units_of(if form.can_trap() {
        &["xmm3", "condition_flags"]
    } else {
        &["xmm3", "xmm4", "condition_flags"]
    })
}

pub fn effects(form: FloatResultPolicyForm) -> MachineEncodedEffects {
    MachineEncodedEffects::float_result_policy_v1(
        form.operand_count() as u16,
        clobbers(form),
        form.can_trap(),
    )
}

pub fn size(form: FloatResultPolicyForm) -> MachineSizeKnowledge {
    crate::float_sizes::float_result_policy_size(form)
}

pub fn declaration(
    form: FloatResultPolicyForm,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration::single(
        MachineSemanticKind::FloatResultPolicy(form),
        constraint,
        MachineMemoryEffect::NoneV1,
        if form.can_trap() {
            MachineTrapBehavior::TrappingIntegerV1
        } else {
            MachineTrapBehavior::NeverV1
        },
        if form.can_trap() {
            MachineBarrier::ExternalEffect
        } else {
            MachineBarrier::None
        },
        MachineAlternativeFamily::FloatResultPolicy(form),
        size(form),
        effects(form),
    )
}

use register_model::RegisterConstraintKey;
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
use selected_instructions::SaturatingCarrier;
use selected_instructions::TrappingForm;
use selected_instructions::TrappingOperation;
use semantic_vocabulary::IntegerSign;

pub fn effects(reads: Vec<u16>, writes: Vec<u16>) -> MachineEncodedEffects {
    let physical = crate::aarch64_physical_register_model_ref();
    MachineEncodedEffects {
        implicit_unit_clobbers: physical
            .view_named("nzcv")
            .expect("canonical AArch64 model declares nzcv")
            .units
            .clone(),
        trap: MachineEncodedTrapBehavior::TrappingIntegerV1,
        control: MachineEncodedControlEffect::FallThroughOrTrapV1,
        ..MachineEncodedEffects::fallthrough_v1(reads, writes)
    }
}

pub fn declaration(
    form: TrappingForm,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    let sources = form.source_count() as u16;
    MachineEffectDeclaration::single(
        MachineSemanticKind::TrappingInteger(form),
        constraint,
        MachineMemoryEffect::NoneV1,
        MachineTrapBehavior::TrappingIntegerV1,
        MachineBarrier::ExternalEffect,
        MachineAlternativeFamily::TrappingInteger(form),
        MachineSizeKnowledge::ExactBytes(byte_size(form)),
        effects(
            (0..sources).collect(),
            (sources..operand_count(form) as u16).collect(),
        ),
    )
}

pub const fn operand_count(form: TrappingForm) -> usize {
    form.source_count() + 2
}

pub const fn conversion_high_bit(source: IntegerSign, carrier: SaturatingCarrier) -> Option<u8> {
    match (source, carrier) {
        (IntegerSign::Signed, SaturatingCarrier::I64)
        | (IntegerSign::Unsigned, SaturatingCarrier::U64) => None,
        (IntegerSign::Signed, carrier) if carrier.is_signed() => None,
        (IntegerSign::Signed, SaturatingCarrier::U64) => Some(63),
        (_, carrier) if carrier.is_signed() => Some(carrier.bits() as u8 - 1),
        (_, carrier) => Some(carrier.bits() as u8),
    }
}

pub fn byte_size(form: TrappingForm) -> u16 {
    use TrappingOperation::*;
    let carrier = form.carrier;
    let narrow_check = 3;
    let words = match form.operation {
        Convert { source } => {
            1 + if conversion_high_bit(source, carrier).is_some()
                || source == IntegerSign::Signed && carrier != SaturatingCarrier::I64
            {
                narrow_check
            } else {
                0
            }
        }
        Add | Subtract => {
            if carrier.is_narrow() {
                1 + narrow_check
            } else {
                3
            }
        }
        Multiply => {
            if carrier.is_narrow() {
                1 + narrow_check
            } else {
                5
            }
        }
        Divide | Remainder => {
            let overflow = if carrier == SaturatingCarrier::I64 {
                4
            } else if carrier.is_signed() && carrier.is_narrow() {
                narrow_check
            } else {
                0
            };
            3 + overflow + u16::from(form.operation == Remainder)
        }
        ShiftRight => 4,
        ShiftLeft => 4 + if carrier.is_narrow() { narrow_check } else { 4 },
    };
    words * 4
}

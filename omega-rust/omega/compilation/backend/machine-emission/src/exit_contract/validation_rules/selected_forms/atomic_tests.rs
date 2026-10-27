//! Exit admission must retain the indivisible effect verified by the ISA
//! decoder, not erase it into a generic fall-through instruction.
use super::assert_non_return_refused;
use selected_instructions::{
    AtomicForm, AtomicWidth, MachineAlternativeFamily, MachineAlternativeKey,
    MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedTrapBehavior,
    SelectedInstructionId, SelectedInstructionKind,
};

#[test]
fn atomic_exit_admission_binds_memory_width_operands_and_control() {
    let physical = super::aarch64_physical_register_model();
    let registers =
        ["x2", "x0", "x1", "x3"].map(|name| physical.model().view_named(name).unwrap().id);
    for form in AtomicForm::ALL {
        let kind = SelectedInstructionKind::AtomicAccess { form };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::AtomicAccess(form),
            variant: 0,
        };
        let encoded = isa_aarch64::encode_aarch64_selected_form(
            &physical,
            kind,
            alternative,
            &registers[..usize::from(form.operand_count())],
        )
        .unwrap();
        let instruction = SelectedInstructionId(1);
        let (store, encoding, layout) =
            super::validated_aarch64_form_fixture(&encoded, instruction, alternative, kind);
        for mutation in 0..8 {
            let mut changed = store.clone();
            let footprint = changed.footprint_mut(encoding.footprint).unwrap();
            match mutation {
                0 => footprint.encoded.memory = MachineEncodedMemoryEffect::NoneV1,
                1 => {
                    footprint.encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: u16::from(form.width.bytes()),
                    }
                }
                2 => {
                    footprint.encoded.memory = MachineEncodedMemoryEffect::AtomicPointerV1 {
                        pointer_operand: 0,
                        form: AtomicForm {
                            width: if form.width == AtomicWidth::Bits8 {
                                AtomicWidth::Bits16
                            } else {
                                AtomicWidth::Bits8
                            },
                            ..form
                        },
                    }
                }
                3 => footprint.encoded.trap = MachineEncodedTrapBehavior::NeverV1,
                4 => footprint.encoded.external_operand_reads.clear(),
                5 => footprint
                    .encoded
                    .external_operand_writes
                    .push(form.operand_count()),
                6 => {
                    footprint.encoded.control =
                        MachineEncodedControlEffect::AtomicRetryUntilCommitV1
                }
                _ => footprint.encoded.control = MachineEncodedControlEffect::DirectRelativeCallV1,
            }
            assert_non_return_refused(
                &format!("{form:?}, mutation {mutation}"),
                instruction,
                kind,
                &encoding,
                &layout,
                &changed,
            );
        }
    }
}

//! Exit admission must retain the indivisible effect verified by the ISA
//! decoder, not erase it into a generic fall-through instruction.
use super::validate_non_return;
use machine_code::{
    ResolvedSelectedFormRow, SelectedFormDecodedFootprint, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormMachineDisposition,
};
use selected_instructions::{
    AtomicForm, AtomicWidth, MachineAlternativeFamily, MachineAlternativeKey,
    MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedTrapBehavior,
    SelectedInstructionId, SelectedInstructionKind,
};

#[test]
fn atomic_exit_admission_binds_memory_width_operands_and_control() {
    let physical = register_model::validate_physical_register_model(
        isa_aarch64::aarch64_physical_register_model(),
    )
    .unwrap();
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
        let effects = &encoded.footprint().encoded;
        let mut store = machine_code::SelectedFormEncodedRowStore::new();
        let (code, interned_footprint) = store.intern(
            encoded.bytes(),
            SelectedFormDecodedFootprint {
                register_reads: encoded.footprint().register_reads.clone(),
                register_writes: encoded.footprint().register_writes.clone(),
                implicit_defs: effects.implicit_unit_defs.clone(),
                implicit_clobbers: effects.implicit_unit_clobbers.clone(),
                encoded: effects.clone(),
            },
        );
        let encoding = SelectedFormEncodingRow {
            instruction,
            alternative,
            machine_disposition: SelectedFormMachineDisposition::RetainedV1,
            address: None,
            state: SelectedFormEncodingState::Encoded,
            code,
            footprint: interned_footprint,
        };
        let layout = ResolvedSelectedFormRow {
            instruction,
            alternative,
            offset: 0,
            bytes: encoded.bytes().to_vec(),
            branch: None,
            internal_machine_fixup: None,
            normalized_foreign_call_fixup: None,
            byte_literal_address_fixup: None,
        };
        validate_non_return(instruction, kind, &encoding, &layout, &store).unwrap();
        for mutation in 0..7 {
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
                _ => footprint.encoded.control = MachineEncodedControlEffect::DirectRelativeCallV1,
            }
            assert!(
                validate_non_return(instruction, kind, &encoding, &layout, &changed).is_err(),
                "{form:?}, mutation {mutation}"
            );
        }
    }
}

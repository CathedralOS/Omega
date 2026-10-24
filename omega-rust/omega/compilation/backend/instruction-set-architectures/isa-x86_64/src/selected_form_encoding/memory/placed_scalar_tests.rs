//! Source-free ISA readers reject pointer, width, polarity and address substitutions.
use super::replay_battery;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier, MachineEncodedMemoryEffect,
    MachineSemanticKind, PlacedScalarForm, SelectedInstructionKind,
};

#[test]
fn placed_scalar_forms_decode_one_exact_width_transfer() {
    let physical =
        register_model::validate_physical_register_model(crate::x86_64_physical_register_model())
            .unwrap();
    let constraints = crate::validate_x86_64_register_constraint_catalog(
        crate::x86_64_register_constraint_catalog(&physical),
        &physical,
    )
    .unwrap();
    let target = target::NativeTarget::linux_x64();
    let catalog = crate::validate_x86_64_machine_effect_catalog(
        target,
        &constraints,
        crate::x86_64_machine_effect_catalog(target, &constraints).unwrap(),
    )
    .unwrap();
    let base = physical.model().view_named("r12").unwrap().id;
    let data = physical.model().view_named("r11").unwrap().id;
    for form in PlacedScalarForm::ALL {
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::PlacedScalarAccess(form),
            variant: 0,
        };
        let kind = SelectedInstructionKind::PlacedScalarAccess {
            form,
            contract: [3; 32],
            byte_offset: 8,
        };
        let declaration = catalog
            .catalog()
            .declaration_by_semantic(MachineSemanticKind::PlacedScalarAccess(form))
            .unwrap();
        assert_eq!(declaration.barrier, MachineBarrier::ExternalEffect);
        let encoded = super::encode_x86_64_selected_memory_form(
            &physical,
            kind,
            alternative,
            &[base, data],
            8,
        )
        .unwrap();
        let expected = if form.write {
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
        } else {
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: u16::from(form.bytes),
            }
        };
        assert_eq!(encoded.footprint().encoded.memory, expected);
        assert_eq!(
            encoded.footprint().register_reads,
            if form.write {
                vec![base, data]
            } else {
                vec![base]
            }
        );
        assert_eq!(
            encoded.footprint().register_writes,
            if form.write { vec![] } else { vec![data] }
        );
        let verify = |kind,
                      alternative,
                      operands: &[register_model::RegisterViewId],
                      displacement,
                      bytes: &[u8]| {
            super::validate_x86_64_selected_memory_form(
                &physical,
                kind,
                alternative,
                operands,
                displacement,
                bytes,
            )
        };
        replay_battery::replay_rejects_byte_mutations(encoded.bytes(), |bytes| {
            verify(kind, alternative, &[base, data], 8, bytes)
        });
        replay_battery::replay_rejects_substituted_requests(
            encoded.bytes(),
            kind,
            alternative,
            &[base, data],
            8,
            verify,
        );
        assert!(verify(kind, alternative, &[data, base], 8, encoded.bytes()).is_err());
        assert!(verify(kind, alternative, &[data, data], 8, encoded.bytes()).is_err());
        let changed = PlacedScalarForm {
            write: !form.write,
            bytes: form.bytes,
        };
        assert!(
            verify(
                SelectedInstructionKind::PlacedScalarAccess {
                    form: changed,
                    contract: [3; 32],
                    byte_offset: 8
                },
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::PlacedScalarAccess(changed),
                    variant: 0
                },
                &[base, data],
                8,
                encoded.bytes()
            )
            .is_err()
        );
        let changed = PlacedScalarForm {
            write: form.write,
            bytes: if form.bytes == 8 { 4 } else { 8 },
        };
        assert!(
            verify(
                SelectedInstructionKind::PlacedScalarAccess {
                    form: changed,
                    contract: [3; 32],
                    byte_offset: 8
                },
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::PlacedScalarAccess(changed),
                    variant: 0
                },
                &[base, data],
                8,
                encoded.bytes()
            )
            .is_err()
        );
    }
}

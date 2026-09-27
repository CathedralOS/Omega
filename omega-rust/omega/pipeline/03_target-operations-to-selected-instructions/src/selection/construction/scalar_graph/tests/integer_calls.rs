//! Ordinary returning calls normalize their exact integer type before comparison.
use super::{
    IntegerSign, IntegerType, LegalizedScalarInstructionKind, ScalarType, SelectedFunction,
    SelectedInstructionKind, SelectedSelectionConstraints, ValueId, build, fixture_with_integer,
};

#[test]
fn total_native_widening_normalizes_the_source_and_replays_exact_operands() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        for source_sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            for destination_sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
                for source_bits in [8, 16, 32, 64] {
                    for destination_bits in [8, 16, 32, 64] {
                        let source_type = IntegerType::new(source_sign, source_bits).unwrap();
                        let destination_type =
                            IntegerType::new(destination_sign, destination_bits).unwrap();
                        let mut source = fixture_with_integer(target, 0, source_type);
                        source.blocks[0].instructions.truncate(2);
                        source.provenance.operations.truncate(2);
                        source.blocks[0].instructions[0].kind =
                            LegalizedScalarInstructionKind::Constant(match source_sign {
                                IntegerSign::Signed => source_type.minimum_value(),
                                IntegerSign::Unsigned => source_type.maximum_value(),
                            });
                        let widening = &mut source.blocks[0].instructions[1];
                        widening.result.as_mut().unwrap().scalar_type =
                            ScalarType::Integer(destination_type);
                        widening.kind = LegalizedScalarInstructionKind::IntegerWiden {
                            operand: ValueId::new(1).unwrap(),
                            source_type,
                        };
                        let selected = build(
                            0,
                            &source,
                            target,
                            &constraints,
                            environment.physical(),
                            environment.constraints(),
                        );
                        if !source_type.can_widen_to(destination_type) {
                            assert!(selected.is_err(), "{source_type:?} -> {destination_type:?}");
                            continue;
                        }
                        let selected = selected.unwrap();
                        let validate = |candidate: &SelectedFunction| {
                            crate::selection::validation::scalar_graph::validate(
                                0,
                                &source,
                                candidate,
                                target,
                                &constraints,
                                environment.physical(),
                                environment.constraints(),
                            )
                        };
                        validate(&selected).unwrap();
                        let expected = match (source_sign, source_bits) {
                            (IntegerSign::Signed, 8) => SelectedInstructionKind::SignExtendI8,
                            (IntegerSign::Signed, 16) => SelectedInstructionKind::SignExtendI16,
                            (IntegerSign::Signed, 32) => SelectedInstructionKind::SignExtendI32,
                            (IntegerSign::Unsigned, 8) => SelectedInstructionKind::ZeroExtendU8,
                            (IntegerSign::Unsigned, 16) => SelectedInstructionKind::ZeroExtendU16,
                            (IntegerSign::Unsigned, 32) => SelectedInstructionKind::ZeroExtendU32,
                            _ => panic!("a native widening source is narrower than 64 bits"),
                        };
                        assert_eq!(selected.blocks[0].instructions[1].kind, expected);
                        for replacement in [
                            SelectedInstructionKind::CopyI64,
                            SelectedInstructionKind::SignExtendI8,
                            SelectedInstructionKind::SignExtendI16,
                            SelectedInstructionKind::SignExtendI32,
                            SelectedInstructionKind::ZeroExtendU8,
                            SelectedInstructionKind::ZeroExtendU16,
                            SelectedInstructionKind::ZeroExtendU32,
                        ] {
                            if replacement == expected {
                                continue;
                            }
                            let mut changed = selected.clone();
                            changed.blocks[0].instructions[1].kind = replacement;
                            assert!(validate(&changed).is_err());
                        }
                        let mut changed = selected.clone();
                        let widening = &mut changed.blocks[0].instructions[1];
                        widening.operands[0].virtual_register =
                            widening.operands[1].virtual_register;
                        assert!(validate(&changed).is_err());
                        let mut changed = selected.clone();
                        changed.blocks[0].instructions[1].provenance.values[0] =
                            ValueId::new(2).unwrap();
                        assert!(validate(&changed).is_err());
                    }
                }
            }
        }
    }
}

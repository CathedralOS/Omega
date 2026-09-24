//! Saturating multiplication selects one carrier-named form with an
//! operand-3 scratch on every target; only x86-64 u64 sits on the fixed
//! RAX/RCX/RDX `MUL` row, which needs a distinct right register.
use super::{
    IntegerSign, IntegerType, LegalizedScalarInstructionKind, ScalarType, SelectedFunction,
    SelectedInstructionKind, SelectedSelectionConstraints, ValueId, build, fixture_with_integer,
};
use legalized_operations::SaturatingCarrier;
use semantic_vocabulary::OperationId;

#[test]
fn saturating_multiply_selects_the_carrier_form_and_rejects_drift() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let x86 = target.architecture == target::Architecture::X86_64;
        for carrier in SaturatingCarrier::ALL {
            for shared_operand in [false, true] {
                let mut source = fixture_with_integer(target, 0, carrier.integer_type());
                source.blocks[0].instructions.truncate(3);
                source.provenance.operations.truncate(3);
                source.blocks[0].instructions[2].kind =
                    LegalizedScalarInstructionKind::SaturatingMultiply {
                        carrier,
                        left: ValueId::new(1).unwrap(),
                        right: ValueId::new(if shared_operand { 1 } else { 2 }).unwrap(),
                    };
                let selected = build(
                    0,
                    &source,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .unwrap_or_else(|error| panic!("{target:?} {carrier:?}: {error:?}"));
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
                let position = selected.blocks[0]
                    .instructions
                    .iter()
                    .position(|instruction| {
                        instruction.provenance.operations == [OperationId::new(3).unwrap()]
                    })
                    .unwrap();
                let instruction = &selected.blocks[0].instructions[position];
                assert_eq!(
                    instruction.kind,
                    SelectedInstructionKind::SaturatingMultiply { carrier }
                );
                let (key, other_key) = if carrier == SaturatingCarrier::U64 {
                    (
                        constraints.keys.saturating_multiply_u64,
                        constraints.keys.saturating_multiply_clamped,
                    )
                } else {
                    (
                        constraints.keys.saturating_multiply_clamped,
                        constraints.keys.saturating_multiply_u64,
                    )
                };
                assert_eq!(instruction.constraint, key, "{target:?} {carrier:?}");
                assert_eq!(instruction.operands.len(), 4);
                assert!(instruction.provenance.obligations.is_empty());
                if x86 && carrier == SaturatingCarrier::U64 && shared_operand {
                    assert_ne!(
                        instruction.operands[0].virtual_register,
                        instruction.operands[1].virtual_register,
                        "a squared u64 operand cannot carry both RAX and RCX"
                    );
                }
                // The clamp keeps the result normalized, so the multiply
                // itself defines the source value: nothing re-normalizes it.
                assert!(
                    selected.blocks[0].instructions[position + 1..]
                        .iter()
                        .all(|later| later.provenance.values != [ValueId::new(3).unwrap()])
                );
                let sibling = SaturatingCarrier::ALL
                    [(usize::from(carrier.ordinal()) + 1) % SaturatingCarrier::ALL.len()];
                for corruption in 0..6 {
                    let mut changed = selected.clone();
                    let instruction = &mut changed.blocks[0].instructions[position];
                    match corruption {
                        // A sibling carrier would clamp to the wrong bounds.
                        0 => {
                            instruction.kind =
                                SelectedInstructionKind::SaturatingMultiply { carrier: sibling }
                        }
                        1 => instruction.kind = SelectedInstructionKind::SaturatingAdd { carrier },
                        2 => {
                            instruction.operands.pop();
                        }
                        3 => instruction.constraint = other_key,
                        4 => instruction.provenance.operations.clear(),
                        _ => {
                            let output = instruction.operands[2].virtual_register;
                            let wrong_sign = if carrier.is_signed() {
                                IntegerSign::Unsigned
                            } else {
                                IntegerSign::Signed
                            };
                            changed.virtual_registers[output.0 as usize].scalar_type =
                                ScalarType::Integer(
                                    IntegerType::new(wrong_sign, carrier.bits()).unwrap(),
                                );
                        }
                    }
                    assert!(
                        validate(&changed).is_err(),
                        "{target:?} {carrier:?} shared={shared_operand}: corruption {corruption}"
                    );
                }
            }
        }
    }
}

//! Projection-only controls: raw rows do not assert source-proof admission.

use super::*;
use legalized_operations::{LegalizedExactIntegerBinary, LegalizedImmediate};
use semantic_vocabulary::{BlockId, IntegerValue, ObligationId, OperationId};

fn sequence() -> LegalizedExactIntegerSequence {
    let value = |raw| ValueId::new(raw).unwrap();
    let operation = |raw| OperationId::new(raw).unwrap();
    let site = |node| ValueDefinitionSite::Node {
        block: BlockId::new(1).unwrap(),
        node,
    };
    let fuel = |raw| {
        vec![FuelSettlement {
            site: PsiProvenance::Operation(operation(raw)),
            units: 1,
        }]
    };
    let binary = |raw, operator, left, right| {
        LegalizedIntegerStep::ExactBinary(LegalizedExactIntegerBinary {
            operator,
            source_value: value(raw),
            obligation: ObligationId::new(raw).unwrap(),
            accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                [raw as u8; 32],
            ),
            operation: operation(raw),
            definition_site: site(u32::try_from(raw - 2).unwrap()),
            fuel: fuel(raw),
            left: value(left),
            right: value(right),
        })
    };
    LegalizedExactIntegerSequence {
        steps: vec![
            LegalizedIntegerStep::Immediate(LegalizedImmediate {
                source_value: value(2),
                value: IntegerValue::Unsigned(4),
                constant_operation: operation(2),
                definition_site: site(0),
                fuel: fuel(2),
            }),
            binary(3, LegalizedExactIntegerOperator::Subtract, 1, 2),
            binary(4, LegalizedExactIntegerOperator::Add, 3, 3),
            binary(5, LegalizedExactIntegerOperator::Subtract, 4, 2),
        ],
    }
}

#[test]
fn ordered_integer_projection_reuses_values_and_rejects_detached_rows() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let catalog = environment.constraints();
        let keys = environment.selected_keys();
        let source = sequence();
        let inputs = [(ValueId::new(1).unwrap(), VirtualRegisterId(0))];
        let mut registers = vec![VirtualRegister {
            id: VirtualRegisterId(0),
            scalar_type: ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            ),
            class: row(catalog, keys.materialize_i64).unwrap().operands[0].class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: inputs[0].0,
                parameter_index: 0,
            },
            definition_site: ValueDefinitionSite::FunctionParameter(0),
            entry_fixed_view: None,
        }];
        // A returned earlier result does not erase later source definitions.
        let result = ValueId::new(3).unwrap();
        let (instructions, returned) = build(
            0,
            &source,
            result,
            &inputs,
            13,
            &mut registers,
            &keys,
            catalog,
        )
        .unwrap();
        assert_eq!(instructions.len(), 4);
        assert_eq!(registers.len(), 5);
        assert_eq!(returned, VirtualRegisterId(2));
        assert_eq!(
            instructions[2].operands[0].virtual_register,
            VirtualRegisterId(2)
        );
        assert_eq!(
            instructions[2].operands[1].virtual_register,
            VirtualRegisterId(2)
        );
        let check = |registers: &[VirtualRegister], instructions: &[SelectedInstruction]| {
            crate::selection::validation::integer_sequence::validate(
                0,
                &source,
                result,
                &inputs,
                13,
                1,
                registers,
                instructions,
                &keys,
                catalog,
            )
        };
        assert_eq!(check(&registers, &instructions).unwrap(), returned);
        for corruption in 0..12 {
            let mut changed_registers = registers.clone();
            let mut changed_instructions = instructions.clone();
            match corruption {
                0 => changed_instructions[2].operands[1].virtual_register = VirtualRegisterId(1),
                1 => {
                    changed_instructions[1].kind = SelectedInstructionKind::ExactSubtractI64 {
                        obligation: ObligationId::new(3).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([99; 32]),
                    }
                }
                2 => {
                    changed_instructions[1].provenance.operations[0] = OperationId::new(99).unwrap()
                }
                3 => changed_instructions[1].provenance.fuel[0].units += 1,
                4 => {
                    changed_registers[2].definition_site = ValueDefinitionSite::FunctionParameter(0)
                }
                5 => changed_registers[2].origin = changed_registers[1].origin,
                6 => {
                    changed_instructions.pop();
                }
                7 => changed_instructions.swap(1, 2),
                8 => {
                    changed_registers[2].entry_fixed_view =
                        Some(environment.physical().model().views[0].id)
                }
                9 => changed_instructions[1]
                    .clobbers
                    .push(register_model::RegisterUnitId(999)),
                10 => changed_instructions[1].provenance.obligations.clear(),
                _ => changed_instructions[1].operands[0].access = RegisterOperandAccess::Def,
            }
            assert!(
                check(&changed_registers, &changed_instructions).is_err(),
                "corruption {corruption}"
            );
        }
    }
}

//! Mixed scalar and whole-view calls replay the existing CallI64 transport.
use super::*;
mod corruption;
mod fixture;

#[test]
fn mixed_borrowed_calls_replay_linux_x64() {
    replay(target::NativeTarget::linux_x64(), 6);
}

#[test]
fn mixed_borrowed_calls_replay_windows_x64() {
    replay(target::NativeTarget::windows_x64(), 4);
}

#[test]
fn mixed_borrowed_calls_replay_linux_arm64() {
    replay(target::NativeTarget::linux_arm64(), 8);
}

#[test]
fn mixed_borrowed_calls_replay_macos_arm64() {
    replay(target::NativeTarget::macos_arm64(), 8);
}

fn replay(target: target::NativeTarget, maximum: usize) {
    let environment = register_environment::baseline_target_register_environment(target).unwrap();
    for parameter_count in [0, 1, maximum - 1] {
        for scalar_count in 1..maximum {
            for conditional in [false, true] {
                let source = fixture::source(target, parameter_count, scalar_count, conditional);
                let constraints = fixture::constraints(&source, &environment);
                let construct = |source: &LegalizedScalarFunction| {
                    build(
                        0,
                        source,
                        target,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                    )
                };
                let validate = |source: &LegalizedScalarFunction, candidate: &SelectedFunction| {
                    crate::selection::validation::scalar_graph::validate(
                        0,
                        source,
                        candidate,
                        target,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                    )
                };
                let selected = construct(&source).unwrap();
                validate(&source, &selected).unwrap();
                assert!(selected.outgoing_arguments.is_empty());
                assert!(
                    selected.memory_accesses.is_empty(),
                    "calls transport descriptor pointers without reading referents"
                );
                assert_eq!(selected.calls.len(), if conditional { 3 } else { 2 });
                for contract in &selected.calls {
                    let row = selected
                        .blocks
                        .iter()
                        .flat_map(|block| &block.instructions)
                        .find(|row| row.id == contract.instruction)
                        .unwrap();
                    assert_eq!(row.constraint, constraints.keys.call_i64[scalar_count + 1]);
                    assert_eq!(row.operands.len(), scalar_count + 2);
                    assert_eq!(row.provenance.values.len(), scalar_count + 1);
                    let pointer = row.operands[scalar_count].virtual_register;
                    assert!(
                        matches!(selected.virtual_registers[pointer.0 as usize].origin,
                        VirtualRegisterOrigin::AbiTransport { place, byte_offset: 0, .. }
                        if place == PlaceId::new(1).unwrap())
                    );
                }
                // First call reads the computed value; later calls read its durable result.
                let first = selected.calls[0].instruction;
                let rows = &selected.blocks[0].instructions;
                let first_position = rows.iter().position(|row| row.id == first).unwrap();
                let durable_result = rows[first_position + 1].operands[1].virtual_register;
                for contract in &selected.calls[1..] {
                    let block = selected
                        .blocks
                        .iter()
                        .find(|block| {
                            block
                                .instructions
                                .iter()
                                .any(|row| row.id == contract.instruction)
                        })
                        .unwrap();
                    let position = block
                        .instructions
                        .iter()
                        .position(|row| row.id == contract.instruction)
                        .unwrap();
                    let scalar_copy = &block.instructions[position - scalar_count - 1];
                    assert_eq!(scalar_copy.operands[0].virtual_register, durable_result);
                }
                for mutation in 0..corruption::SELECTED_COUNT {
                    let mut changed = selected.clone();
                    corruption::selected(&mut changed, scalar_count, mutation);
                    assert!(
                        validate(&source, &changed).is_err(),
                        "{target:?}, parameters {parameter_count}, scalars {scalar_count}, conditional {conditional}, selected corruption {mutation}"
                    );
                }
                for mutation in 0..corruption::SOURCE_COUNT {
                    let mut changed = source.clone();
                    corruption::source(&mut changed, scalar_count, mutation);
                    assert!(construct(&changed).is_err(), "source corruption {mutation}");
                    assert!(
                        validate(&changed, &selected).is_err(),
                        "receiving corruption {mutation}"
                    );
                }
                let mut oversized = source.clone();
                let LegalizedScalarInstructionKind::Call(call) =
                    &mut oversized.blocks[0].instructions[2].kind
                else {
                    unreachable!();
                };
                let borrowed = call.arguments.pop().unwrap();
                while call.arguments.len() < maximum {
                    call.arguments.push(call.arguments[0].clone());
                }
                call.arguments.push(borrowed);
                assert!(
                    construct(&oversized).is_err(),
                    "unsupported CallI64 arity stays fenced"
                );
                assert!(validate(&oversized, &selected).is_err());
            }
        }
    }
}

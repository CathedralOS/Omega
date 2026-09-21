//! Replay rejects changes to the exact field write, its address and its proof.

use super::{
    AdmissionProfile, Legalized, NativeTarget, Role, Selected, Target, legalize_target_operations,
    select_instructions, selection_constraints, validate_legalized_operations,
    validate_selected_instructions,
};
#[test]
fn indexed_field_write_replay_rejects_address_extent_and_evidence_changes() {
    let terminal = super::super::indexed::indexed_replacement(false);
    let input = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &terminal_codec::encode_module(&terminal.semantic_module).unwrap(),
            proof_bytes: &terminal_codec::encode_proof_section(
                &terminal.semantic_module,
                &terminal.proof_bundle,
            )
            .unwrap(),
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .map(|artifact| {
        artifact
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .unwrap();
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap();
    let source = verified.input().plan();
    let unit = verified.unit();
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let legal = legalize_target_operations(&target, source, unit).unwrap();
        validate_legalized_operations(&target, source, unit, legal.plan().clone()).unwrap();
        let replacement_fact = legal
            .plan()
            .scalar_functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .find_map(|row| match row.kind {
                Legalized::StructuralByteSequenceFieldStore { accepted_fact, .. } => {
                    Some(accepted_fact)
                }
                _ => None,
            })
            .unwrap();
        for mutation in 0..4 {
            let mut changed = target.clone();
            let store = changed
                .functions
                .iter_mut()
                .flat_map(|function| &mut function.graph.blocks)
                .flat_map(|block| &mut block.operations)
                .find(|operation| {
                    matches!(
                        operation,
                        Target::StructuralByteSequenceFieldByteStore { .. }
                    )
                })
                .unwrap();
            let Target::StructuralByteSequenceFieldByteStore {
                destination,
                index,
                value,
                length,
                ..
            } = store
            else {
                unreachable!()
            };
            match mutation {
                0 => destination.access = terminal_psi::StructuralAccess::SharedBorrow,
                1 => *length = index.source_value(),
                2 => *index = *value,
                _ => *value = *index,
            }
            assert!(
                legalize_target_operations(&changed, source, unit).is_err(),
                "target mutation {mutation}: {native:?}"
            );
        }
        for mutation in 0..5 {
            let mut changed = legal.plan().clone();
            let store = changed
                .scalar_functions
                .iter_mut()
                .flat_map(|function| &mut function.blocks)
                .flat_map(|block| &mut block.instructions)
                .find(|row| {
                    matches!(
                        row.kind,
                        Legalized::StructuralByteSequenceFieldByteStore { .. }
                    )
                })
                .unwrap();
            let Legalized::StructuralByteSequenceFieldByteStore {
                destination,
                index,
                value,
                length,
                accepted_fact,
                ..
            } = &mut store.kind
            else {
                unreachable!()
            };
            match mutation {
                0 => destination.access = terminal_psi::StructuralAccess::SharedBorrow,
                1 => *length = *index,
                2 => std::mem::swap(index, value),
                3 => *accepted_fact = replacement_fact,
                _ => store.fuel.clear(),
            }
            assert!(
                validate_legalized_operations(&target, source, unit, changed).is_err(),
                "legalized mutation {mutation}: {native:?}"
            );
        }
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        for mutation in 0..10 {
            let mut changed = selected.plan().clone();
            let function = changed
                .functions
                .iter_mut()
                .find(|function| {
                    function
                        .memory_accesses
                        .iter()
                        .any(|access| matches!(access.role, Role::WriteByteSequence { .. }))
                })
                .unwrap();
            let access = function
                .memory_accesses
                .iter_mut()
                .find(|access| matches!(access.role, Role::WriteByteSequence { .. }))
                .unwrap();
            let block = function
                .blocks
                .iter_mut()
                .find(|block| {
                    block
                        .instructions
                        .iter()
                        .any(|instruction| instruction.id == access.instruction)
                })
                .unwrap();
            let position = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == access.instruction)
                .unwrap();
            match mutation {
                0 => access.byte_offset += 8,
                1 => access.byte_count = 8,
                2 => access.role = Role::WritePlace,
                3 => {
                    if let Role::WriteByteSequence { index, length, .. } = &mut access.role {
                        *length = *index;
                    }
                }
                4 => block.instructions[position].operands.swap(0, 1),
                5 => {
                    block.instructions[position].kind = Selected::Store {
                        byte_offset: 0,
                        byte_size: 8,
                    }
                }
                6 => block.instructions[position].provenance.fuel.clear(),
                7 => block.instructions[position - 1].operands.swap(0, 1),
                8 => {
                    block.instructions[position - 2].kind =
                        Selected::AddressOffset { byte_offset: 0 }
                }
                _ => {
                    block.instructions.remove(position);
                }
            }
            assert!(
                validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed
                )
                .is_err(),
                "selected mutation {mutation}: {native:?}"
            );
        }
    }
}

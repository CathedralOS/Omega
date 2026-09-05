//! Placement replay controls shared by the direct, structural, and frame fixtures.
use omega_machine_code::RelocationFreeTextSectionPlacement;
use omega_machine_emission::{
    StructuralFragmentPlacementInputs, TextPlacementInput, validate_fragment_text_section,
};

pub(super) fn direct(staged: &crate::StagedOptimizedRelocationFreeTextSection) {
    let source = staged.source();
    let current = source.source();
    let input = if source.fragments().structural_unit_functions.is_empty() {
        TextPlacementInput::RelocationFree(source.fragments())
    } else {
        TextPlacementInput::Structural {
            fragments: source.fragments(),
            facts: StructuralFragmentPlacementInputs {
                program: current.program(),
                structural_encoding: current.encoding().structural_unit_functions(),
                exit: current.exit_contract().contract(),
                physical: current.register_environment().physical(),
                constraints: current.register_environment().constraints(),
            },
        }
    };
    check(input, staged.text_section());
    let retained = staged.shared_text_section();
    assert!(std::ptr::eq(retained.as_ref(), staged.text_section()));
}
pub(super) fn fixed(staged: &crate::StagedOptimizedFixedFrameTextSection) {
    check(
        TextPlacementInput::InternalCalls(staged.source().fragments()),
        staged.text_section(),
    );
    let retained = staged.shared_text_section();
    assert!(std::ptr::eq(retained.as_ref(), staged.text_section()));
}

fn check(input: TextPlacementInput<'_>, original: &RelocationFreeTextSectionPlacement) {
    validate_fragment_text_section(input, original).unwrap();
    for mutation in 0..16 {
        let mut changed = original.clone();
        match mutation {
            0 => {
                changed.source_fragments =
                    omega_optimization_core::FunctionFragmentEmissionIdentity::from_canonical_bytes(
                        b"wrong placement source",
                    )
            }
            1 => changed.semantic_entry_offset += 1,
            2 => changed.section_alignment += 1,
            3 => changed.byte_count += 1,
            4 => changed.bytes[0] ^= 1,
            5 => {
                changed.functions.pop();
            }
            6 => changed.functions.push(changed.functions[0].clone()),
            7 => changed.functions[0].source_function_index += 1,
            8 => changed.functions[0].section_offset += 1,
            9 => changed.functions[0].byte_count += 1,
            10 => changed.functions[0].blocks[0].function_offset += 1,
            11 => changed.functions[0].blocks[0].section_offset += 1,
            12 => changed.functions[0].blocks[0].byte_count += 1,
            13 => changed.functions[0].blocks[0].instructions[0].function_offset += 1,
            14 => changed.functions[0].blocks[0].instructions[0].section_offset += 1,
            15 => changed.functions[0].blocks[0].instructions[0].byte_count += 1,
            _ => unreachable!(),
        }
        changed.identity = changed.recomputed_identity();
        assert!(
            validate_fragment_text_section(input, &changed).is_err(),
            "placement mutation {mutation}"
        );
    }
    if original.resolved_internal_machine_calls.is_empty() {
        return;
    }
    for mutation in 0..18 {
        let mut changed = original.clone();
        let row = &mut changed.resolved_internal_machine_calls[0];
        match mutation {
            0 => row.call_function_offset += 1,
            1 => row.call_section_offset += 1,
            2 => row.call_byte_count += 1,
            3 => row.opcode_function_offset += 1,
            4 => row.opcode_section_offset += 1,
            5 => row.field_function_offset += 1,
            6 => row.field_section_offset += 1,
            7 => row.next_instruction_function_offset += 1,
            8 => row.next_instruction_section_offset += 1,
            9 => row.callee_section_offset += 1,
            10 => row.field_byte_width += 1,
            11 => row.addend += 1,
            12 => row.displacement ^= 1,
            13 => changed.bytes[row.field_section_offset as usize] ^= 1,
            14 => {
                changed.resolved_internal_machine_calls.pop();
            }
            15 => {
                let duplicate = *row;
                changed.resolved_internal_machine_calls.push(duplicate);
            }
            16 => row.callee = psi_core::MachineId::new(999).unwrap(),
            17 => row.caller = psi_core::MachineId::new(999).unwrap(),
            _ => unreachable!(),
        }
        changed.identity = changed.recomputed_identity();
        assert!(
            validate_fragment_text_section(input, &changed).is_err(),
            "call mutation {mutation}"
        );
    }
}

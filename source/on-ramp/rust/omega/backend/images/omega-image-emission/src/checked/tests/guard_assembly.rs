//! Runtime guard and checked-assembly replay regressions.

use super::*;

#[test]
fn runtime_text_guard_replay_binds_buffer_and_storage_symbols() {
    let target = NativeTarget::linux_x64();
    let mut object = ObjectPlan::with_capacity(target, 0, 2);
    let buffer_symbol = object.layout.symbols.insert(SymbolPlan {
        name: "omega_data_text_guard_buffer".to_owned(),
        section: SymbolSection::Section(SectionKind::Data),
        offset: 0,
        size: 16,
        kind: SymbolKind::Object,
        import_library: String::new(),
    });
    let storage_symbol = object.layout.symbols.insert(SymbolPlan {
        name: omega_object_file::runtime_frame_storage_symbol_name(),
        section: SymbolSection::Section(SectionKind::Bss),
        offset: 0,
        size: 64,
        kind: SymbolKind::Object,
        import_library: String::new(),
    });
    let instruction_index = 41;
    let instruction_offset = 32;
    let mut relocations = RelocationPlan::with_target(target);
    for (relative_offset, symbol_handle) in [(2usize, buffer_symbol), (12, storage_symbol)] {
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: instruction_index,
            },
            section: SectionKind::Text,
            offset: instruction_offset + relative_offset,
            byte_width: 8,
            symbol_handle,
            addend: 0,
            kind: RelocationKind::Absolute64,
        });
    }

    validate_compiler_runtime_text_relocations(
        omega_target::Architecture::X86_64,
        &object,
        &relocations,
        instruction_index,
        instruction_offset,
        "omega_data_text_guard_buffer",
        &[(
            10,
            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        )],
    )
    .expect("runtime-text replay should accept its exact data and storage symbols");

    let diagnostic = validate_compiler_runtime_text_relocations(
        omega_target::Architecture::X86_64,
        &object,
        &relocations,
        instruction_index,
        instruction_offset,
        "omega_data_other_buffer",
        &[(
            10,
            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        )],
    )
    .expect_err("a substituted runtime-text buffer symbol must reject");
    assert!(diagnostic.message.contains("buffer/storage relocation set"));

    let mut missing_source = RelocationPlan::with_target(target);
    missing_source.push_record(
        relocations
            .records()
            .next()
            .expect("buffer relocation")
            .1
            .clone(),
    );
    let diagnostic = validate_compiler_runtime_text_relocations(
        omega_target::Architecture::X86_64,
        &object,
        &missing_source,
        instruction_index,
        instruction_offset,
        "omega_data_text_guard_buffer",
        &[(
            10,
            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        )],
    )
    .expect_err("a missing runtime-text source relocation must reject");
    assert!(diagnostic.message.contains("buffer/storage relocation set"));
}

#[test]
fn runtime_value_guard_replay_derives_recursive_operand_sites() {
    use omega_target_operations::{RuntimeStorageRegion, RuntimeValueOperand};

    let mut operands = psi_arena::Arena::new();
    let indexed = operands.insert(RuntimeValueOperand::FrameIndexed {
        descriptor_offset: 16,
        index_region: RuntimeStorageRegion::Machine,
        index_offset: 8,
        index_byte_size: 4,
        element_byte_size: 16,
        field_byte_offset: 4,
        byte_size: 4,
    });
    let left = operands.insert(RuntimeValueOperand::Convert {
        source: indexed,
        source_byte_size: 4,
        target_byte_size: 8,
        source_is_float: false,
        target_is_float: false,
        source_signed: true,
        target_signed: true,
        trapping: false,
        saturating: false,
    });
    let right = operands.insert(RuntimeValueOperand::TextEquals {
        left_region: RuntimeStorageRegion::RuntimeFrame,
        left_offset: 40,
        left_is_bounded_buffer: false,
        right_region: RuntimeStorageRegion::Machine,
        right_offset: 80,
        right_is_bounded_buffer: false,
    });

    let sites = compiler_runtime_value_compare_address_sites(
        omega_target::Architecture::X86_64,
        &operands,
        left,
        right,
    )
    .expect("recursive value operands should yield exact relocation sites");
    let right_start = omega_isa_x86_64::runtime_value_operand_width(&operands, left);
    assert_eq!(
        sites,
        vec![
            (0, RuntimeStorageRegion::RuntimeFrame),
            (
                omega_isa_x86_64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET,
                RuntimeStorageRegion::Machine,
            ),
            (right_start, RuntimeStorageRegion::RuntimeFrame),
            (
                right_start + omega_isa_x86_64::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET,
                RuntimeStorageRegion::Machine,
            ),
        ]
    );

    let diagnostic = compiler_runtime_value_compare_address_sites(
        omega_target::Architecture::X86_64,
        &operands,
        omega_target_operations::RuntimeValueOperandHandle::invalid(),
        right,
    )
    .expect_err("an invalid retained operand root must reject");
    assert!(diagnostic.message.contains("invalid operand handle"));
}

#[test]
fn checked_emission_rejects_unclassified_executable_bytes() {
    let inventory = PlacedExecutableRegionInventory {
        text_address: 0x1000,
        text_byte_count: 4,
        text_fingerprint: 1,
        inventory_fingerprint: 2,
        regions: Vec::new(),
        unclassified_gaps: vec![omega_image::PlacedExecutableGap {
            section_offset: 0,
            address: 0x1000,
            byte_count: 4,
            byte_fingerprint: 3,
        }],
    };

    let target = NativeTarget::linux_x64();
    let code = omega_machine_bytes::EncodedMachinePlan::with_capacity(target, 0, 0, 0).code;
    let object = ObjectPlan::with_capacity(target, 0, 0);
    let diagnostic = validate_executable_region_enumeration(&inventory, &code, &object, &[])
        .expect_err("checked images must classify every executable byte");
    assert!(diagnostic.message.contains("4 unclassified byte(s)"));
}

#[test]
fn validates_checked_assembly_at_retained_instruction_boundaries() {
    use omega_machine_bytes::{
        CheckedInstructionValidationKind, EncodedMachineCode, EncodedMachineInstruction,
    };
    use psi_arena::Arena;

    let mut bytes = Arena::with_capacity(5);
    let halt = bytes.insert_many([0xf4]);
    let fence = bytes.insert_many([0x0f, 0xae, 0xf0]);
    let cli = bytes.insert_many([0xfa]);
    let mut instructions = Arena::with_capacity(3);
    instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 4,
        bytes: halt,
        compiler_validation_kind: None,
        checked_validation_kind: Some(CheckedInstructionValidationKind::MachineHalt),
        checked_operand_loaders: [None, None],
    });
    instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 5,
        bytes: fence,
        compiler_validation_kind: None,
        checked_validation_kind: Some(CheckedInstructionValidationKind::FullFence),
        checked_operand_loaders: [None, None],
    });
    instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 6,
        bytes: cli,
        compiler_validation_kind: None,
        checked_validation_kind: Some(CheckedInstructionValidationKind::InterruptDisable),
        checked_operand_loaders: [None, None],
    });
    let code = EncodedMachineCode {
        functions: Arena::new(),
        instructions,
        bytes,
        runtime_value_operands: Arena::new(),
        byte_count: 5,
    };

    let relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
    let (count, fingerprint) = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &[0xf4, 0x0f, 0xae, 0xf0, 0xfa],
        &relocations,
    )
    .expect("closed checked-assembly bytes should validate");
    assert_eq!(count, 3);
    assert_ne!(fingerprint, 0);

    let diagnostic = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &[0xf4, 0x0f, 0xae, 0xe8, 0xfa],
        &relocations,
    )
    .expect_err("a changed final fence kind must reject");
    assert!(diagnostic.message.contains("changed after encoding"));
}

#[test]
fn checked_assembly_rejects_an_incomplete_operand_loader_envelope() {
    use omega_machine_bytes::{
        CheckedInstructionValidationKind, EncodedMachineCode, EncodedMachineInstruction,
    };
    use psi_arena::Arena;

    let mut bytes = Arena::with_capacity(1);
    let span = bytes.insert_many([0xee]);
    let mut instructions = Arena::with_capacity(1);
    instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 7,
        bytes: span,
        compiler_validation_kind: None,
        checked_validation_kind: Some(CheckedInstructionValidationKind::PortWriteImmediatePort {
            port: 0x3f8,
            value_operand_byte_width: 10,
        }),
        checked_operand_loaders: [None, None],
    });
    let code = EncodedMachineCode {
        functions: Arena::new(),
        instructions,
        bytes,
        runtime_value_operands: Arena::new(),
        byte_count: 1,
    };

    let diagnostic = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &[0xee],
        &RelocationPlan::with_target(NativeTarget::linux_x64()),
    )
    .expect_err("catalog validation must not omit required operand loaders");
    assert!(
        diagnostic
            .message
            .contains("complete operand-loader validation envelope")
    );
}

#[test]
fn validates_immediate_port_identity_and_privileged_io_envelopes() {
    use omega_machine_bytes::{
        CheckedInstructionValidationKind, CheckedOperandLoaderKind, CheckedOperandLoaderRegister,
        CheckedOperandLoaderValidation, EncodedMachineCode, EncodedMachineInstruction,
    };
    use psi_arena::Arena;

    let mut out_bytes = Vec::new();
    out_bytes.extend([0x49, 0xba]);
    out_bytes.extend(0x3f8u64.to_le_bytes());
    out_bytes.extend([0x44, 0x89, 0xd2]);
    out_bytes.extend([0x49, 0xbb]);
    out_bytes.extend(0x41u64.to_le_bytes());
    out_bytes.extend([0x44, 0x89, 0xd8, 0xee]);
    let mut in_bytes = Vec::new();
    in_bytes.extend([0x49, 0xba]);
    in_bytes.extend(0x3fdu64.to_le_bytes());
    in_bytes.extend([0x44, 0x89, 0xd2, 0xec, 0x49, 0xbf]);
    in_bytes.extend(0u64.to_le_bytes());
    in_bytes.extend([0x41, 0x88, 0x87]);
    in_bytes.extend(4u32.to_le_bytes());

    let mut bytes = Arena::with_capacity(out_bytes.len() + in_bytes.len());
    let out_span = bytes.insert_many(out_bytes.iter().copied());
    let in_span = bytes.insert_many(in_bytes.iter().copied());
    let mut instructions = Arena::with_capacity(2);
    instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 8,
        bytes: out_span,
        compiler_validation_kind: None,
        checked_validation_kind: Some(CheckedInstructionValidationKind::PortWriteImmediatePort {
            port: 0x3f8,
            value_operand_byte_width: 10,
        }),
        checked_operand_loaders: [
            Some(CheckedOperandLoaderValidation {
                byte_offset: 0,
                byte_width: 10,
                register: CheckedOperandLoaderRegister::R10,
                kind: CheckedOperandLoaderKind::Immediate { value: 0x3f8 },
            }),
            Some(CheckedOperandLoaderValidation {
                byte_offset: 13,
                byte_width: 10,
                register: CheckedOperandLoaderRegister::R11,
                kind: CheckedOperandLoaderKind::Immediate { value: 0x41 },
            }),
        ],
    });
    instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 9,
        bytes: in_span,
        compiler_validation_kind: None,
        checked_validation_kind: Some(CheckedInstructionValidationKind::PortReadImmediatePort {
            port: 0x3fd,
            destination_byte_offset: 4,
        }),
        checked_operand_loaders: [
            Some(CheckedOperandLoaderValidation {
                byte_offset: 0,
                byte_width: 10,
                register: CheckedOperandLoaderRegister::R10,
                kind: CheckedOperandLoaderKind::Immediate { value: 0x3fd },
            }),
            None,
        ],
    });
    let code = EncodedMachineCode {
        functions: Arena::new(),
        instructions,
        bytes,
        runtime_value_operands: Arena::new(),
        byte_count: out_bytes.len() + in_bytes.len(),
    };
    let mut final_bytes = out_bytes;
    final_bytes.extend(in_bytes);
    let destination_relocation_offset = final_bytes.len() - 31 + 16;
    final_bytes[destination_relocation_offset..destination_relocation_offset + 8]
        .copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
    let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
    relocations.push_record(RelocationRecord {
        origin: RelocationOrigin::Instruction {
            function_symbol_handle: Handle::invalid(),
            selected_instruction_index: 9,
        },
        section: SectionKind::Text,
        offset: destination_relocation_offset,
        byte_width: 8,
        symbol_handle: Handle::invalid(),
        addend: 0,
        kind: RelocationKind::Absolute64,
    });

    let (count, fingerprint) = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &final_bytes,
        &relocations,
    )
    .expect("closed port identities and opcode envelopes should validate");
    assert_eq!(count, 2);
    assert_ne!(fingerprint, 0);

    let mut wrong_port = final_bytes.clone();
    wrong_port[2] ^= 1;
    let diagnostic = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &wrong_port,
        &relocations,
    )
    .expect_err("changing a final port identity must reject");
    assert!(diagnostic.message.contains("changed its port"));

    let mut wrong_value = final_bytes.clone();
    wrong_value[15] ^= 1;
    let diagnostic = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &wrong_value,
        &relocations,
    )
    .expect_err("changing a final immediate operand value must reject");
    assert!(diagnostic.message.contains("immediate operand loader"));

    let mut wrong_opcode = final_bytes;
    wrong_opcode[out_span.len() - 1] = 0x90;
    let diagnostic = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &wrong_opcode,
        &relocations,
    )
    .expect_err("changing a final out opcode must reject");
    assert!(diagnostic.message.contains("privileged opcode envelope"));
}

#[test]
fn validates_direct_storage_operand_loader_semantics() {
    use omega_machine_bytes::{
        CheckedInstructionValidationKind, CheckedOperandLoaderKind, CheckedOperandLoaderRegister,
        CheckedOperandLoaderValidation, EncodedMachineCode, EncodedMachineInstruction,
    };
    use psi_arena::{Arena, Handle};
    use psi_language_core::inline_assembly::AsmControlRegister;

    let mut encoded = Vec::new();
    encoded.extend([0x49, 0xbf]);
    encoded.extend(0u64.to_le_bytes());
    encoded.extend([0x4d, 0x8b, 0x97]);
    encoded.extend(32u32.to_le_bytes());
    encoded.extend([0x41, 0x0f, 0x22, 0xda]);

    let mut bytes = Arena::with_capacity(encoded.len());
    let span = bytes.insert_many(encoded.iter().copied());
    let mut instructions = Arena::with_capacity(1);
    instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 11,
        bytes: span,
        compiler_validation_kind: None,
        checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
            register: AsmControlRegister::Cr3,
            source_operand_byte_width: 17,
        }),
        checked_operand_loaders: [
            Some(CheckedOperandLoaderValidation {
                byte_offset: 0,
                byte_width: 17,
                register: CheckedOperandLoaderRegister::R10,
                kind: CheckedOperandLoaderKind::Storage {
                    byte_offset: 32,
                    byte_size: 8,
                },
            }),
            None,
        ],
    });
    let code = EncodedMachineCode {
        functions: Arena::new(),
        instructions,
        bytes,
        runtime_value_operands: Arena::new(),
        byte_count: encoded.len(),
    };

    let mut final_bytes = encoded;
    final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
    let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
    relocations.push_record(RelocationRecord {
        origin: RelocationOrigin::Instruction {
            function_symbol_handle: Handle::invalid(),
            selected_instruction_index: 11,
        },
        section: SectionKind::Text,
        offset: 2,
        byte_width: 8,
        symbol_handle: Handle::invalid(),
        addend: 0,
        kind: RelocationKind::Absolute64,
    });

    validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &final_bytes,
        &relocations,
    )
    .expect("direct storage loader semantics and relocation should validate");

    let mut wrong_load = final_bytes.clone();
    wrong_load[10] ^= 1;
    let diagnostic = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &wrong_load,
        &relocations,
    )
    .expect_err("changing the retained source load must reject");
    assert!(diagnostic.message.contains("storage operand loader"));

    let missing_relocation = RelocationPlan::with_target(NativeTarget::linux_x64());
    let diagnostic = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &final_bytes,
        &missing_relocation,
    )
    .expect_err("a storage loader without its exact relocation must reject");
    assert!(diagnostic.message.contains("source-storage relocation"));
}

fn indirect_operand_fixture(
    kind: omega_machine_bytes::CheckedOperandLoaderKind,
    pointer_byte_offset: u32,
    value_byte_offset: u32,
) -> (
    omega_machine_bytes::EncodedMachineCode,
    Vec<u8>,
    RelocationPlan,
) {
    use omega_machine_bytes::{
        CheckedInstructionValidationKind, CheckedOperandLoaderRegister,
        CheckedOperandLoaderValidation, EncodedMachineCode, EncodedMachineInstruction,
    };
    use psi_arena::Arena;
    use psi_language_core::inline_assembly::AsmControlRegister;

    let mut encoded = Vec::new();
    encoded.extend([0x49, 0xbf]);
    encoded.extend(0u64.to_le_bytes());
    encoded.extend([0x49, 0x8b, 0x87]);
    encoded.extend(pointer_byte_offset.to_le_bytes());
    encoded.extend([0x4c, 0x8b, 0x90]);
    encoded.extend(value_byte_offset.to_le_bytes());
    encoded.extend([0x41, 0x0f, 0x22, 0xda]);

    let mut bytes = Arena::with_capacity(encoded.len());
    let span = bytes.insert_many(encoded.iter().copied());
    let mut instructions = Arena::with_capacity(1);
    instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 12,
        bytes: span,
        compiler_validation_kind: None,
        checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
            register: AsmControlRegister::Cr3,
            source_operand_byte_width: 24,
        }),
        checked_operand_loaders: [
            Some(CheckedOperandLoaderValidation {
                byte_offset: 0,
                byte_width: 24,
                register: CheckedOperandLoaderRegister::R10,
                kind,
            }),
            None,
        ],
    });
    let code = EncodedMachineCode {
        functions: Arena::new(),
        instructions,
        bytes,
        runtime_value_operands: Arena::new(),
        byte_count: encoded.len(),
    };

    let mut final_bytes = encoded;
    final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
    let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
    relocations.push_record(RelocationRecord {
        origin: RelocationOrigin::Instruction {
            function_symbol_handle: Handle::invalid(),
            selected_instruction_index: 12,
        },
        section: SectionKind::Text,
        offset: 2,
        byte_width: 8,
        symbol_handle: Handle::invalid(),
        addend: 0,
        kind: RelocationKind::Absolute64,
    });
    (code, final_bytes, relocations)
}

#[test]
fn validates_pointee_and_fixed_index_operand_loader_semantics() {
    use omega_machine_bytes::CheckedOperandLoaderKind;

    let (pointee_code, pointee_bytes, pointee_relocations) = indirect_operand_fixture(
        CheckedOperandLoaderKind::Pointee {
            pointer_byte_offset: 24,
            field_byte_offset: 8,
            byte_size: 8,
        },
        24,
        8,
    );
    let (_, pointee_fingerprint) = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &pointee_code,
        &pointee_bytes,
        &pointee_relocations,
    )
    .expect("pointee loader semantics and relocation should validate");

    let (fixed_code, fixed_bytes, fixed_relocations) = indirect_operand_fixture(
        CheckedOperandLoaderKind::FrameFixedIndexed {
            descriptor_byte_offset: 24,
            element_index: 2,
            element_byte_size: 4,
            field_byte_offset: 0,
            byte_size: 8,
        },
        24,
        8,
    );
    let (_, fixed_fingerprint) = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &fixed_code,
        &fixed_bytes,
        &fixed_relocations,
    )
    .expect("fixed-index loader semantics and relocation should validate");
    assert_ne!(
        pointee_fingerprint, fixed_fingerprint,
        "semantically distinct operand plans must not share a certificate fingerprint"
    );

    let mut wrong_pointer_load = pointee_bytes;
    wrong_pointer_load[10] ^= 1;
    let diagnostic = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &pointee_code,
        &wrong_pointer_load,
        &pointee_relocations,
    )
    .expect_err("changing the retained pointer load must reject");
    assert!(diagnostic.message.contains("indirect operand loader"));
}

#[test]
fn validates_frame_base_indexed_operand_loader_semantics() {
    use omega_machine_bytes::{
        CheckedInstructionValidationKind, CheckedOperandLoaderKind, CheckedOperandLoaderRegister,
        CheckedOperandLoaderValidation, EncodedMachineCode, EncodedMachineInstruction,
    };
    use psi_arena::Arena;
    use psi_language_core::inline_assembly::AsmControlRegister;

    let mut encoded = Vec::new();
    encoded.extend([0x49, 0xbf]);
    encoded.extend(0u64.to_le_bytes());
    encoded.extend([0x45, 0x8b, 0x9f]);
    encoded.extend(16u32.to_le_bytes());
    encoded.extend([0x4d, 0x69, 0xdb]);
    encoded.extend(24u32.to_le_bytes());
    encoded.extend([0x4c, 0x89, 0xf8]);
    encoded.extend([0x4c, 0x01, 0xd8]);
    encoded.extend([0x4c, 0x8b, 0x90]);
    encoded.extend(40u32.to_le_bytes());
    encoded.extend([0x41, 0x0f, 0x22, 0xda]);

    let mut bytes = Arena::with_capacity(encoded.len());
    let span = bytes.insert_many(encoded.iter().copied());
    let mut instructions = Arena::with_capacity(1);
    instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 13,
        bytes: span,
        compiler_validation_kind: None,
        checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
            register: AsmControlRegister::Cr3,
            source_operand_byte_width: 37,
        }),
        checked_operand_loaders: [
            Some(CheckedOperandLoaderValidation {
                byte_offset: 0,
                byte_width: 37,
                register: CheckedOperandLoaderRegister::R10,
                kind: CheckedOperandLoaderKind::FrameBaseIndexed {
                    base_byte_offset: 32,
                    index_byte_offset: 16,
                    index_byte_size: 4,
                    element_byte_size: 24,
                    field_byte_offset: 8,
                    byte_size: 8,
                },
            }),
            None,
        ],
    });
    let code = EncodedMachineCode {
        functions: Arena::new(),
        instructions,
        bytes,
        runtime_value_operands: Arena::new(),
        byte_count: encoded.len(),
    };

    let mut final_bytes = encoded;
    final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
    let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
    relocations.push_record(RelocationRecord {
        origin: RelocationOrigin::Instruction {
            function_symbol_handle: Handle::invalid(),
            selected_instruction_index: 13,
        },
        section: SectionKind::Text,
        offset: 2,
        byte_width: 8,
        symbol_handle: Handle::invalid(),
        addend: 0,
        kind: RelocationKind::Absolute64,
    });

    validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &final_bytes,
        &relocations,
    )
    .expect("frame-base-indexed loader semantics and relocation should validate");

    let mut wrong_scale = final_bytes;
    wrong_scale[20] ^= 1;
    let diagnostic = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &wrong_scale,
        &relocations,
    )
    .expect_err("changing the retained element scale must reject");
    assert!(
        diagnostic
            .message
            .contains("frame-base-indexed operand loader")
    );
}

#[test]
fn validates_cross_region_frame_indexed_operand_loader_semantics() {
    use omega_machine_bytes::{
        CheckedInstructionValidationKind, CheckedOperandLoaderKind, CheckedOperandLoaderRegister,
        CheckedOperandLoaderValidation, EncodedMachineCode, EncodedMachineInstruction,
    };
    use psi_arena::Arena;
    use psi_language_core::inline_assembly::AsmControlRegister;

    let mut encoded = Vec::new();
    encoded.extend([0x49, 0xbf]);
    encoded.extend(0u64.to_le_bytes());
    encoded.extend([0x49, 0x8b, 0x87]);
    encoded.extend(24u32.to_le_bytes());
    encoded.extend([0x49, 0xbf]);
    encoded.extend(0u64.to_le_bytes());
    encoded.extend([0x45, 0x0f, 0xb6, 0x9f]);
    encoded.extend(12u32.to_le_bytes());
    encoded.extend([0x4d, 0x69, 0xdb]);
    encoded.extend(32u32.to_le_bytes());
    encoded.extend([0x4c, 0x01, 0xd8]);
    encoded.extend([0x4c, 0x8b, 0x90]);
    encoded.extend(8u32.to_le_bytes());
    encoded.extend([0x41, 0x0f, 0x22, 0xda]);

    let mut bytes = Arena::with_capacity(encoded.len());
    let span = bytes.insert_many(encoded.iter().copied());
    let mut instructions = Arena::with_capacity(1);
    instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 14,
        bytes: span,
        compiler_validation_kind: None,
        checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
            register: AsmControlRegister::Cr3,
            source_operand_byte_width: 52,
        }),
        checked_operand_loaders: [
            Some(CheckedOperandLoaderValidation {
                byte_offset: 0,
                byte_width: 52,
                register: CheckedOperandLoaderRegister::R10,
                kind: CheckedOperandLoaderKind::FrameIndexed {
                    descriptor_byte_offset: 24,
                    index_from_machine: true,
                    index_byte_offset: 12,
                    index_byte_size: 1,
                    element_byte_size: 32,
                    field_byte_offset: 8,
                    byte_size: 8,
                },
            }),
            None,
        ],
    });
    let code = EncodedMachineCode {
        functions: Arena::new(),
        instructions,
        bytes,
        runtime_value_operands: Arena::new(),
        byte_count: encoded.len(),
    };

    let mut final_bytes = encoded;
    final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
    final_bytes[19..27].copy_from_slice(&0x0fed_cba9_8765_4321u64.to_le_bytes());
    let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
    for offset in [2, 19] {
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 14,
            },
            section: SectionKind::Text,
            offset,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Absolute64,
        });
    }

    validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &final_bytes,
        &relocations,
    )
    .expect("cross-region frame-indexed semantics and both relocations should validate");

    let mut missing_second = RelocationPlan::with_target(NativeTarget::linux_x64());
    missing_second.push_record(RelocationRecord {
        origin: RelocationOrigin::Instruction {
            function_symbol_handle: Handle::invalid(),
            selected_instruction_index: 14,
        },
        section: SectionKind::Text,
        offset: 2,
        byte_width: 8,
        symbol_handle: Handle::invalid(),
        addend: 0,
        kind: RelocationKind::Absolute64,
    });
    let diagnostic = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &final_bytes,
        &missing_second,
    )
    .expect_err("a cross-region operand without its index-base relocation must reject");
    assert!(diagnostic.message.contains("source-storage relocation"));
}

#[test]
fn validates_cross_region_machine_indexed_operand_loader_semantics() {
    use omega_machine_bytes::{
        CheckedInstructionValidationKind, CheckedOperandLoaderKind, CheckedOperandLoaderRegister,
        CheckedOperandLoaderValidation, EncodedMachineCode, EncodedMachineInstruction,
    };
    use psi_arena::Arena;
    use psi_language_core::inline_assembly::AsmControlRegister;

    let mut encoded = Vec::new();
    encoded.extend([0x49, 0xbf]);
    encoded.extend(0u64.to_le_bytes());
    encoded.extend([0x4c, 0x89, 0xf8]);
    encoded.extend([0x49, 0xbf]);
    encoded.extend(0u64.to_le_bytes());
    encoded.extend([0x45, 0x0f, 0xb7, 0x9f]);
    encoded.extend(20u32.to_le_bytes());
    encoded.extend([0x4d, 0x69, 0xdb]);
    encoded.extend(16u32.to_le_bytes());
    encoded.extend([0x4c, 0x01, 0xd8]);
    encoded.extend([0x4c, 0x8b, 0x90]);
    encoded.extend(72u32.to_le_bytes());
    encoded.extend([0x41, 0x0f, 0x22, 0xda]);

    let mut bytes = Arena::with_capacity(encoded.len());
    let span = bytes.insert_many(encoded.iter().copied());
    let mut instructions = Arena::with_capacity(1);
    instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 15,
        bytes: span,
        compiler_validation_kind: None,
        checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
            register: AsmControlRegister::Cr3,
            source_operand_byte_width: 48,
        }),
        checked_operand_loaders: [
            Some(CheckedOperandLoaderValidation {
                byte_offset: 0,
                byte_width: 48,
                register: CheckedOperandLoaderRegister::R10,
                kind: CheckedOperandLoaderKind::MachineIndexed {
                    base_byte_offset: 64,
                    index_from_frame: true,
                    index_byte_offset: 20,
                    index_byte_size: 2,
                    element_byte_size: 16,
                    field_byte_offset: 8,
                    byte_size: 8,
                },
            }),
            None,
        ],
    });
    let code = EncodedMachineCode {
        functions: Arena::new(),
        instructions,
        bytes,
        runtime_value_operands: Arena::new(),
        byte_count: encoded.len(),
    };

    let mut final_bytes = encoded;
    final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
    final_bytes[15..23].copy_from_slice(&0x0fed_cba9_8765_4321u64.to_le_bytes());
    let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
    for offset in [2, 15] {
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 15,
            },
            section: SectionKind::Text,
            offset,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            addend: 0,
            kind: RelocationKind::Absolute64,
        });
    }

    validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &final_bytes,
        &relocations,
    )
    .expect("cross-region machine-indexed semantics and both relocations should validate");

    let mut wrong_index_extension = final_bytes;
    wrong_index_extension[24] ^= 1;
    let diagnostic = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &wrong_index_extension,
        &relocations,
    )
    .expect_err("changing the unsigned index load must reject");
    assert!(
        diagnostic
            .message
            .contains("machine-indexed operand loader")
    );
}

#[test]
fn rejects_mutated_final_wrmsr_opcode_after_index_binding() {
    use omega_machine_bytes::{
        CheckedInstructionValidationKind, CheckedOperandLoaderKind, CheckedOperandLoaderRegister,
        CheckedOperandLoaderValidation, EncodedMachineCode, EncodedMachineInstruction,
    };
    use psi_arena::Arena;

    let mut encoded = Vec::new();
    encoded.extend([0x49, 0xba]);
    encoded.extend(0xc000_0080u64.to_le_bytes());
    encoded.extend([0x41, 0x52]);
    encoded.extend([0x49, 0xbb]);
    encoded.extend(0x1122_3344_5566_7788u64.to_le_bytes());
    encoded.extend([
        0x41, 0x5a, 0x44, 0x89, 0xd1, 0x44, 0x89, 0xd8, 0x4c, 0x89, 0xda, 0x48, 0xc1, 0xea, 0x20,
        0x0f, 0x30,
    ]);
    let mut bytes = Arena::with_capacity(encoded.len());
    let span = bytes.insert_many(encoded.iter().copied());
    let mut instructions = Arena::with_capacity(1);
    instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 10,
        bytes: span,
        compiler_validation_kind: None,
        checked_validation_kind: Some(CheckedInstructionValidationKind::MsrWriteImmediateIndex {
            index: 0xc000_0080,
            value_operand_byte_width: 10,
        }),
        checked_operand_loaders: [
            Some(CheckedOperandLoaderValidation {
                byte_offset: 0,
                byte_width: 10,
                register: CheckedOperandLoaderRegister::R10,
                kind: CheckedOperandLoaderKind::Immediate { value: 0xc000_0080 },
            }),
            Some(CheckedOperandLoaderValidation {
                byte_offset: 12,
                byte_width: 10,
                register: CheckedOperandLoaderRegister::R11,
                kind: CheckedOperandLoaderKind::Immediate {
                    value: 0x1122_3344_5566_7788,
                },
            }),
        ],
    });
    let code = EncodedMachineCode {
        functions: Arena::new(),
        instructions,
        bytes,
        runtime_value_operands: Arena::new(),
        byte_count: encoded.len(),
    };
    let relocations = RelocationPlan::with_target(NativeTarget::linux_x64());

    validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &encoded,
        &relocations,
    )
    .expect("exact WRMSR index and split-value envelope should validate");

    let last = encoded.len() - 1;
    encoded[last] = 0x31;
    let diagnostic = validate_checked_instruction_bytes(
        omega_target::Architecture::X86_64,
        &code,
        &encoded,
        &relocations,
    )
    .expect_err("a changed final WRMSR opcode must reject");
    assert!(diagnostic.message.contains("privileged opcode envelope"));
}

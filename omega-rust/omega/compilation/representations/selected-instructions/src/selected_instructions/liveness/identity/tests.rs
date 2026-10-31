//! Preserve the exact predecessor byte protocol over real raw liveness rows.
//! Malformed storage variants exercise encoding only, not analysis admission.
use super::*;
use crate::*;
use arena::{Arena, Handle, HandleSpan};
use register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId};
use semantic_vocabulary::{BlockId, EdgeId, MachineId};

fn fixture() -> FunctionLiveness {
    let mut positions = Arena::new();
    let operands = positions.insert_many(
        [
            RegisterOperandAccess::Use,
            RegisterOperandAccess::Def,
            RegisterOperandAccess::UseDef,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, access)| OperandPosition {
            position: LivenessPosition(0x10203040),
            instruction: SelectedInstructionId(0xa0b0c0d0),
            operand: u16::MAX - index as u16,
            virtual_register: VirtualRegisterId(u32::MAX - index as u32),
            access,
            class: RegisterClassId(u16::MAX),
            fixed_view: (index != 0).then_some(RegisterViewId(0x1234)),
            tied_to: (index == 2).then_some(u16::MAX),
            early_clobber: index == 2,
        }),
    );
    let vregs = vec![
        VirtualRegisterId(u32::MAX),
        VirtualRegisterId(0),
        VirtualRegisterId(u32::MAX),
    ];
    let units = vec![
        RegisterUnitId(u16::MAX),
        RegisterUnitId(0),
        RegisterUnitId(u16::MAX),
    ];
    FunctionLiveness {
        machine: MachineId::new(u64::MAX).unwrap(),
        entry_definitions: vec![
            EntryDefinition {
                virtual_register: VirtualRegisterId(7),
                class: RegisterClassId(8),
                fixed_view: None,
            },
            EntryDefinition {
                virtual_register: VirtualRegisterId(9),
                class: RegisterClassId(10),
                fixed_view: Some(RegisterViewId(u16::MAX)),
            },
        ],
        operand_positions: positions,
        blocks: vec![BlockLiveness {
            block: SelectedBlockId(u32::MAX),
            source_block: BlockId::new(u64::MAX).unwrap(),
            virtual_live_in: vregs.clone(),
            virtual_live_out: vregs.clone(),
            unit_live_in: units.clone(),
            unit_live_out: units.clone(),
            instructions: vec![InstructionLiveness {
                position: LivenessPosition(0x55667788),
                instruction: SelectedInstructionId(0x90abcdef),
                operands,
                virtual_uses: vregs.clone(),
                virtual_defs: vregs.clone(),
                virtual_live_in: vregs.clone(),
                virtual_live_out: vregs.clone(),
                unit_uses: units.clone(),
                unit_defs: units.clone(),
                unit_clobbers: units.clone(),
                unit_live_in: units.clone(),
                unit_live_out: units.clone(),
            }],
            successors: [None, Some(EdgeId::new(u64::MAX).unwrap())]
                .into_iter()
                .enumerate()
                .map(|(index, psi_edge)| SuccessorLiveness {
                    terminator: SelectedInstructionId(0x11223344),
                    polarity_ordinal: index as u8,
                    psi_edge,
                    target: SelectedBlockId(index as u32),
                    virtual_live: vregs.clone(),
                    unit_live: units.clone(),
                })
                .collect(),
        }],
    }
}
fn same(function: &FunctionLiveness) {
    assert_eq!(
        liveness_function_digest(function),
        buffered_original_digest(function)
    );
}

#[test]
fn streamed_liveness_digest_matches_original_bytes_rows_options_spans_and_boundaries() {
    let populated = fixture();
    same(&populated);
    let mut empty = populated.clone();
    empty.entry_definitions.clear();
    empty.operand_positions = Arena::new();
    empty.blocks.clear();
    same(&empty);
    assert_ne!(
        liveness_function_digest(&empty),
        liveness_function_digest(&populated)
    );
    for count in [
        0, 1, 2, 7, 8, 15, 16, 31, 32, 55, 56, 63, 64, 65, 127, 128, 129, 4097,
    ] {
        let mut function = populated.clone();
        function.blocks[0]
            .virtual_live_in
            .resize(count, VirtualRegisterId(17));
        function.blocks[0].instructions[0]
            .virtual_live_out
            .resize(count, VirtualRegisterId(18));
        function.blocks[0].successors[0]
            .unit_live
            .resize(count, RegisterUnitId(19));
        same(&function);
    }
    for byte in 0..=u8::MAX {
        let mut function = populated.clone();
        function.blocks[0].successors[0].polarity_ordinal = byte;
        let handle = function.blocks[0].instructions[0].operands.start();
        function.operand_positions.get_mut(handle).operand = u16::from(byte) << 8;
        same(&function);
    }
    // Membership is the actual span content, not just its count or address.
    let mut membership = populated.clone();
    let start = membership.blocks[0].instructions[0].operands.start();
    membership.blocks[0].instructions[0].operands = HandleSpan::from_parts(start, 1);
    same(&membership);
    assert_ne!(
        liveness_function_digest(&membership),
        liveness_function_digest(&populated)
    );
    membership.blocks[0].instructions[0].operands = HandleSpan::from_parts(
        Handle::from_parts(start.arena_index(), start.generation() + 1),
        3,
    );
    same(&membership); // The unchanged span_or_empty rejects stale generations.
    membership.blocks[0].instructions[0].operands = HandleSpan::empty();
    same(&membership);
    let mut unspanned = populated.clone();
    unspanned
        .operand_positions
        .insert(OperandPosition::default());
    same(&unspanned);
    // Keep the old active len plus COMPLETE storage_slice, even with a hole.
    let mut vacant = populated.clone();
    assert!(vacant.operand_positions.free(Handle::from_parts(
        start.arena_index() + 1,
        start.generation()
    )));
    assert_eq!(vacant.operand_positions.len(), 2);
    assert_eq!(vacant.operand_positions.storage_slice().len(), 3);
    same(&vacant);
}

fn buffered_original_digest(function: &FunctionLiveness) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(
        256 + function.operand_positions.len() * 24 + function.blocks.len() * 1024,
    );
    bytes.extend_from_slice(b"omega.terminal-register-liveness-function.v2\0");
    buffered_function(&mut bytes, function);
    crate::digest_encoder::content_identity(&bytes).into()
}

fn buffered_function(bytes: &mut Vec<u8>, function: &FunctionLiveness) {
    crate::codec_bytes::push_encoded(bytes, function.machine.get().to_le_bytes());
    crate::codec_bytes::encode_len(bytes, function.entry_definitions.len());
    for definition in &function.entry_definitions {
        crate::codec_bytes::push_encoded(bytes, definition.virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, definition.class.0.to_le_bytes());
        buffered_option_u16(bytes, definition.fixed_view.map(|view| view.0));
    }
    crate::codec_bytes::encode_len(bytes, function.operand_positions.len());
    for operand in function.operand_positions.storage_slice() {
        buffered_operand(bytes, operand);
    }
    crate::codec_bytes::encode_len(bytes, function.blocks.len());
    for block in &function.blocks {
        crate::codec_bytes::push_encoded(bytes, block.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, block.source_block.get().to_le_bytes());
        buffered_vregs(bytes, &block.virtual_live_in);
        buffered_vregs(bytes, &block.virtual_live_out);
        buffered_units(bytes, &block.unit_live_in);
        buffered_units(bytes, &block.unit_live_out);
        crate::codec_bytes::encode_len(bytes, block.instructions.len());
        for instruction in &block.instructions {
            crate::codec_bytes::push_encoded(bytes, instruction.position.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, instruction.instruction.0.to_le_bytes());
            // Encode membership by the actual operand rows, not arena addresses.
            let operands = function
                .operand_positions
                .span_or_empty(instruction.operands);
            crate::codec_bytes::encode_len(bytes, operands.len());
            for operand in operands {
                buffered_operand(bytes, operand);
            }
            buffered_vregs(bytes, &instruction.virtual_uses);
            buffered_vregs(bytes, &instruction.virtual_defs);
            buffered_vregs(bytes, &instruction.virtual_live_in);
            buffered_vregs(bytes, &instruction.virtual_live_out);
            buffered_units(bytes, &instruction.unit_uses);
            buffered_units(bytes, &instruction.unit_defs);
            buffered_units(bytes, &instruction.unit_clobbers);
            buffered_units(bytes, &instruction.unit_live_in);
            buffered_units(bytes, &instruction.unit_live_out);
        }
        crate::codec_bytes::encode_len(bytes, block.successors.len());
        for successor in &block.successors {
            crate::codec_bytes::push_encoded(bytes, successor.terminator.0.to_le_bytes());
            bytes.push(successor.polarity_ordinal);
            crate::codec_bytes::push_encoded(
                bytes,
                successor
                    .psi_edge
                    .map_or(0, |edge| edge.get())
                    .to_le_bytes(),
            );
            crate::codec_bytes::push_encoded(bytes, successor.target.0.to_le_bytes());
            buffered_vregs(bytes, &successor.virtual_live);
            buffered_units(bytes, &successor.unit_live);
        }
    }
}

fn buffered_option_u16(bytes: &mut Vec<u8>, value: Option<u16>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
        }
    }
}

fn buffered_vregs(bytes: &mut Vec<u8>, values: &[crate::VirtualRegisterId]) {
    crate::codec_bytes::encode_le_list(bytes, values, |value| value.0.to_le_bytes());
}

fn buffered_units(bytes: &mut Vec<u8>, values: &[register_model::RegisterUnitId]) {
    crate::codec_bytes::encode_le_list(bytes, values, |value| value.0.to_le_bytes());
}

fn buffered_operand(bytes: &mut Vec<u8>, operand: &crate::OperandPosition) {
    crate::codec_bytes::push_encoded(bytes, operand.position.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, operand.instruction.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, operand.operand.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, operand.virtual_register.0.to_le_bytes());
    bytes.push(operand.access.access_tag());
    crate::codec_bytes::push_encoded(bytes, operand.class.0.to_le_bytes());
    buffered_option_u16(bytes, operand.fixed_view.map(|view| view.0));
    buffered_option_u16(bytes, operand.tied_to);
    bytes.push(u8::from(operand.early_clobber));
}

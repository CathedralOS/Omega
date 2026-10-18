use super::tests::{physical, protocol, source_plan};
use crate::frame_protocol::{FrameProtocolByteSpan, TargetFrameProtocolEncodingPlan};
use isa_aarch64::{
    AARCH64_CONDITIONAL_BRANCH_BYTE_REACH, Aarch64SelectedFormEncodingError,
    ValidatedAarch64SelectedFormEncoding,
};
use machine_code::{
    FunctionFragmentBlockSpan, FunctionFragmentBranchEvidence,
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentConditionalBranchPredicate,
    FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan,
    FunctionFragmentFrameApplication, FunctionFragmentInstructionSpan,
    FunctionFragmentInternalMachineFixup, FunctionFragmentInternalMachineFixupKind,
    FunctionFragmentInternalMachineFixupState,
};
use optimization_core::FunctionFragmentEmissionManifestIdentity;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionProvenance,
};
use semantic_vocabulary::{EdgeId, MachineId};
use target::NativeTarget;

#[test]
fn frame_epilogue_growth_widens_out_of_range_rel8_and_preserves_in_range_rel8() {
    for (padding, expected_width) in [(126, 6), (125, 2)] {
        let mut source = source_plan();
        let physical = physical();
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::ConditionalBranchNonZero,
            variant: 0,
        };
        let encoded = isa_x86_64::encode_x86_64_selected_short_nonzero_branch_form(
            &physical,
            alternative,
            padding + 1,
        )
        .unwrap();
        let function = &mut source.functions[0];
        let mut call = function.blocks[0].instructions[0].clone();
        call.instruction = SelectedInstructionId(5);
        call.offset = 2;
        let fixup = call.internal_machine_fixup.as_mut().unwrap();
        fixup.opcode_function_offset += 2;
        fixup.patch_function_offset += 2;
        fixup.reference_function_offset += 2;
        let mut branch = function.blocks[0].instructions[0].clone();
        branch.alternative = alternative;
        branch.bytes = encoded.bytes().to_vec();
        branch.internal_machine_fixup = None;
        branch.control = FunctionFragmentControlProvenance::None;
        branch.branch = Some(Box::new(FunctionFragmentBranchEvidence::Conditional(
            FunctionFragmentConditionalBranchEvidence {
                predicate: FunctionFragmentConditionalBranchPredicate::NonZeroV1,
                source_block: SelectedBlockId(1),
                when_taken_edge: EdgeId::new(2).unwrap(),
                when_taken_block: SelectedBlockId(3),
                when_taken_offset: (padding + 3) as u64,
                when_fallthrough_edge: EdgeId::new(3).unwrap(),
                when_fallthrough_block: SelectedBlockId(2),
                when_fallthrough_offset: 2,
                byte_displacement: padding + 1,
                decoded_register_reads: encoded.footprint().register_reads.clone(),
                decoded_effects: encoded.footprint().encoded.clone(),
            },
        )));
        let mut filler = branch.clone();
        filler.instruction = SelectedInstructionId(3);
        filler.offset = 7;
        filler.bytes = vec![0x90; padding as usize - 5];
        filler.branch = None;
        let mut first_return = function.blocks[0].instructions[1].clone();
        first_return.offset = (padding + 2) as u64;
        let mut last_return = first_return.clone();
        last_return.instruction = SelectedInstructionId(4);
        last_return.offset += 1;
        function.blocks = vec![
            FunctionFragmentBlockSpan {
                block: SelectedBlockId(1),
                offset: 0,
                byte_count: 2,
                instructions: vec![branch],
            },
            FunctionFragmentBlockSpan {
                block: SelectedBlockId(2),
                offset: 2,
                byte_count: (padding + 1) as u64,
                instructions: vec![call, filler, first_return],
            },
            FunctionFragmentBlockSpan {
                block: SelectedBlockId(3),
                offset: (padding + 3) as u64,
                byte_count: 1,
                instructions: vec![last_return],
            },
        ];
        function.bytes = function
            .instructions()
            .flat_map(|row| row.bytes.clone())
            .collect();
        function.byte_count = function.bytes.len() as u64;
        source.identity = source.recomputed_identity();
        let manifest = FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest");
        let protocol = protocol(source.entry);
        let application = super::super::apply_frame_protocol_to_fragments(
            &source, manifest, &protocol, &physical,
        )
        .unwrap();
        let branch = &application.fragments.functions[0].blocks[0].instructions[0];
        assert_eq!(branch.bytes.len(), expected_width);
        let growth = expected_width as u64 - 2;
        let function = &application.fragments.functions[0];
        let call = &function.blocks[1].instructions[0];
        let fixup = call.internal_machine_fixup.unwrap();
        assert_eq!(call.offset, 3 + growth);
        assert_eq!(fixup.opcode_function_offset, 3 + growth);
        assert_eq!(fixup.patch_function_offset, 4 + growth);
        assert_eq!(fixup.reference_function_offset, 8 + growth);
        assert_eq!(
            application.functions[0].epilogues[0].function_offset,
            padding as u64 + 3 + growth
        );
        assert_eq!(
            application.functions[0].epilogues[1].function_offset,
            padding as u64 + 5 + growth
        );
        assert_eq!(
            branch
                .branch
                .as_deref()
                .unwrap()
                .as_conditional()
                .unwrap()
                .byte_displacement,
            padding + 2
        );
        super::super::validate_frame_protocol_application(
            &source,
            manifest,
            &protocol,
            &physical,
            &application,
        )
        .unwrap();
        let mut corrupted = application.clone();
        corrupted.fragments.functions[0].blocks[0].instructions[0].bytes[1] ^= 1;
        corrupted.fragments.functions[0].bytes[2] ^= 1;
        corrupted.fragments.identity = corrupted.fragments.recomputed_identity();
        corrupted.identity = corrupted.recomputed_identity();
        assert!(
            super::super::validate_frame_protocol_application(
                &source, manifest, &protocol, &physical, &corrupted,
            )
            .is_err()
        );
    }
}

type Aarch64BranchEncoder =
    fn(
        &ValidatedPhysicalRegisterModel,
        MachineAlternativeKey,
        i64,
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError>;

/// One AArch64 conditional branch family: the condition its one-word
/// `B.cond` tests, the inverse its widened pair's skip word tests, and both
/// encoders.
struct Aarch64Conditional {
    predicate: FunctionFragmentConditionalBranchPredicate,
    family: MachineAlternativeFamily,
    condition: u32,
    inverse: u32,
    one_word: Aarch64BranchEncoder,
    widened: Aarch64BranchEncoder,
}

const AARCH64_CONDITIONALS: [Aarch64Conditional; 3] = [
    Aarch64Conditional {
        predicate: FunctionFragmentConditionalBranchPredicate::NonZeroV1,
        family: MachineAlternativeFamily::ConditionalBranchNonZero,
        condition: 0x1,
        inverse: 0x0,
        one_word: isa_aarch64::encode_aarch64_selected_nonzero_branch_form,
        widened: isa_aarch64::encode_aarch64_selected_nonzero_widened_branch_form,
    },
    Aarch64Conditional {
        predicate: FunctionFragmentConditionalBranchPredicate::U64LessThanV1,
        family: MachineAlternativeFamily::ConditionalBranchU64LessThan,
        condition: 0x3,
        inverse: 0x2,
        one_word: isa_aarch64::encode_aarch64_selected_u64_less_than_branch_form,
        widened: isa_aarch64::encode_aarch64_selected_u64_less_than_widened_branch_form,
    },
    Aarch64Conditional {
        predicate: FunctionFragmentConditionalBranchPredicate::I64LessThanV1,
        family: MachineAlternativeFamily::ConditionalBranchI64LessThan,
        condition: 0xb,
        inverse: 0xa,
        one_word: isa_aarch64::encode_aarch64_selected_i64_less_than_branch_form,
        widened: isa_aarch64::encode_aarch64_selected_i64_less_than_widened_branch_form,
    },
];

const MEBIBYTE: u64 = 1 << 20;
const AARCH64_RET: u32 = 0xd65f_03c0;
const AARCH64_NOP: u32 = 0xd503_201f;

fn aarch64_row(
    instruction: u32,
    family: MachineAlternativeFamily,
    bytes: Vec<u8>,
    control: FunctionFragmentControlProvenance,
) -> FunctionFragmentInstructionSpan {
    FunctionFragmentInstructionSpan {
        instruction: SelectedInstructionId(instruction),
        alternative: MachineAlternativeKey { family, variant: 0 },
        offset: 0,
        bytes,
        branch: None,
        internal_machine_fixup: None,
        normalized_foreign_call_fixup: None,
        image_address_fixup: None,
        provenance: SelectedInstructionProvenance::default(),
        control,
    }
}

/// One AArch64 function holding a single conditional branch of
/// `branch_width` bytes, `filler` bytes of straight-line code and two returns.
///
/// Forward: `[branch] [call, filler, return] [return]`; the branch takes the
/// last block and falls through to the second. Backward: `[filler, return]
/// [branch] [call, return]`; the branch takes the first block and falls
/// through to the last. The call carries an internal-call fixup whose
/// coordinates must move with any widening before it.
fn aarch64_source(
    conditional: &Aarch64Conditional,
    branch_width: usize,
    filler: u64,
    backward: bool,
) -> FunctionFragmentEmissionPlan {
    let callee = MachineId::new(2).unwrap();
    let mut call = aarch64_row(
        10,
        MachineAlternativeFamily::CallScalar,
        0x9400_0000_u32.to_le_bytes().to_vec(),
        FunctionFragmentControlProvenance::InternalCall {
            target: selected_instructions::SelectedCallTarget::Machine(callee),
            owner: target_operations::CallSiteOwner::Operation(
                semantic_vocabulary::OperationId::new(1).unwrap(),
            ),
        },
    );
    call.internal_machine_fixup = Some(FunctionFragmentInternalMachineFixup {
        kind: FunctionFragmentInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
        state: FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1,
        callee,
        opcode_function_offset: 0,
        patch_function_offset: 0,
        reference_function_offset: 0,
        patch_byte_width: 4,
        addend: 0,
    });
    let branch = aarch64_row(
        1,
        conditional.family,
        vec![0; branch_width],
        FunctionFragmentControlProvenance::None,
    );
    let straight_line = aarch64_row(
        2,
        MachineAlternativeFamily::CopyI64,
        AARCH64_NOP.to_le_bytes().repeat(filler as usize / 4),
        FunctionFragmentControlProvenance::None,
    );
    let returns = [(3, 1), (4, 4)].map(|(instruction, edge)| {
        aarch64_row(
            instruction,
            MachineAlternativeFamily::ReturnScalar,
            AARCH64_RET.to_le_bytes().to_vec(),
            FunctionFragmentControlProvenance::Return {
                psi_return_edge: EdgeId::new(edge).unwrap(),
            },
        )
    });
    let [first_return, last_return] = returns;
    let (rosters, branch_block, taken, fallthrough) = if backward {
        (
            [
                vec![straight_line, first_return],
                vec![branch],
                vec![call, last_return],
            ],
            2,
            1,
            3,
        )
    } else {
        (
            [
                vec![branch],
                vec![call, straight_line, first_return],
                vec![last_return],
            ],
            1,
            3,
            2,
        )
    };

    let mut source = source_plan();
    source.target = NativeTarget::macos_arm64();
    let function = &mut source.functions[0];
    let mut offset = 0_u64;
    function.blocks = rosters
        .into_iter()
        .zip(1..)
        .map(|(mut rows, block)| {
            let start = offset;
            for row in &mut rows {
                row.offset = offset;
                if let Some(fixup) = &mut row.internal_machine_fixup {
                    fixup.opcode_function_offset = offset;
                    fixup.patch_function_offset = offset;
                    fixup.reference_function_offset = offset;
                }
                offset += row.bytes.len() as u64;
            }
            FunctionFragmentBlockSpan {
                block: SelectedBlockId(block),
                offset: start,
                byte_count: offset - start,
                instructions: rows,
            }
        })
        .collect();
    let block_offset = |blocks: &[FunctionFragmentBlockSpan], block: u32| {
        blocks
            .iter()
            .find(|candidate| candidate.block == SelectedBlockId(block))
            .unwrap()
            .offset
    };
    let taken_offset = block_offset(&function.blocks, taken);
    let fallthrough_offset = block_offset(&function.blocks, fallthrough);
    let row = &mut function.blocks[branch_block as usize - 1].instructions[0];
    let displacement = taken_offset as i64 - row.offset as i64;
    let encoder = if branch_width == 4 {
        conditional.one_word
    } else {
        conditional.widened
    };
    let alternative = MachineAlternativeKey {
        family: conditional.family,
        variant: 0,
    };
    let encoded = encoder(
        isa_aarch64::validated_aarch64_physical_register_model(),
        alternative,
        displacement,
    )
    .unwrap();
    row.bytes = encoded.bytes().to_vec();
    row.branch = Some(Box::new(FunctionFragmentBranchEvidence::Conditional(
        FunctionFragmentConditionalBranchEvidence {
            predicate: conditional.predicate,
            source_block: SelectedBlockId(branch_block),
            when_taken_edge: EdgeId::new(2).unwrap(),
            when_taken_block: SelectedBlockId(taken),
            when_taken_offset: taken_offset,
            when_fallthrough_edge: EdgeId::new(3).unwrap(),
            when_fallthrough_block: SelectedBlockId(fallthrough),
            when_fallthrough_offset: fallthrough_offset,
            byte_displacement: displacement,
            decoded_register_reads: encoded.footprint().register_reads.clone(),
            decoded_effects: encoded.footprint().encoded.clone(),
        },
    )));
    function.bytes = function
        .instructions()
        .flat_map(|row| row.bytes.clone())
        .collect();
    function.byte_count = function.bytes.len() as u64;
    source.identity = source.recomputed_identity();
    source
}

/// A one-word prologue (`stp x29, x30, [sp, #-16]!`) and a one-word
/// epilogue (`ldp x29, x30, [sp], #16`): each moves later code by four bytes.
fn aarch64_protocol(machine: MachineId) -> TargetFrameProtocolEncodingPlan {
    let mut protocol = protocol(machine);
    protocol.target = NativeTarget::macos_arm64();
    protocol.functions[0].prologue = FrameProtocolByteSpan {
        offset: 0,
        length: 4,
    };
    protocol.functions[0].epilogue = FrameProtocolByteSpan {
        offset: 4,
        length: 4,
    };
    protocol.bytes = [0xa9bf_7bfd_u32, 0xa8c1_7bfd]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    protocol
}

fn word_at(bytes: &[u8], offset: u64) -> u32 {
    let start = offset as usize;
    u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap())
}

/// `B.cond`: the condition and the byte displacement from the word itself.
fn decode_conditional(word: u32) -> (u32, i64) {
    assert_eq!(
        word & 0xff00_0010,
        0x5400_0000,
        "{word:#010x} is not B.cond"
    );
    let words = ((((word >> 5) & 0x7ffff) << 13) as i32) >> 13;
    (word & 0xf, i64::from(words) * 4)
}

/// `B`: the byte displacement from the word itself.
fn decode_unconditional(word: u32) -> i64 {
    assert_eq!(word & 0xfc00_0000, 0x1400_0000, "{word:#010x} is not B");
    i64::from((((word & 0x03ff_ffff) << 6) as i32) >> 6) * 4
}

/// Apply the AArch64 frame to `source`, check that the branch row reaches its
/// taken and fallthrough blocks through the decoded words of the flat image and
/// that the independent replay derives the same widths, and return the applied
/// function's branch width.
fn apply_and_decode_aarch64_branch(
    source: &FunctionFragmentEmissionPlan,
    conditional: &Aarch64Conditional,
    backward: bool,
) -> (usize, FunctionFragmentFrameApplication) {
    let physical = isa_aarch64::validated_aarch64_physical_register_model();
    let manifest = FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest");
    let protocol = aarch64_protocol(source.entry);
    let application =
        super::super::apply_frame_protocol_to_fragments(source, manifest, &protocol, physical)
            .unwrap();
    let function = &application.fragments.functions[0];
    let (branch_block, taken_block, fallthrough_block) =
        if backward { (1, 0, 2) } else { (0, 2, 1) };
    let branch = &function.blocks[branch_block].instructions[0];
    let taken = function.blocks[taken_block].offset;
    let fallthrough = function.blocks[fallthrough_block].offset;
    let evidence = branch.branch.as_deref().unwrap().as_conditional().unwrap();
    assert_eq!(evidence.when_taken_offset, taken);
    assert_eq!(evidence.when_fallthrough_offset, fallthrough);
    assert_eq!(
        evidence.byte_displacement,
        taken as i64 - branch.offset as i64
    );
    assert_eq!(branch.offset + branch.bytes.len() as u64, fallthrough);
    assert_eq!(
        &function.bytes[branch.offset as usize..fallthrough as usize],
        &branch.bytes[..]
    );
    let first = word_at(&function.bytes, branch.offset);
    match branch.bytes.len() {
        4 => {
            assert!(AARCH64_CONDITIONAL_BRANCH_BYTE_REACH.contains(&evidence.byte_displacement));
            assert_eq!(
                decode_conditional(first),
                (conditional.condition, taken as i64 - branch.offset as i64)
            );
        }
        8 => {
            assert!(!AARCH64_CONDITIONAL_BRANCH_BYTE_REACH.contains(&evidence.byte_displacement));
            // The inverse condition skips the far `B` to the fallthrough block.
            assert_eq!(decode_conditional(first), (conditional.inverse, 8));
            let far = word_at(&function.bytes, branch.offset + 4);
            assert_eq!(
                branch.offset as i64 + 4 + decode_unconditional(far),
                taken as i64
            );
        }
        width => panic!("unexpected AArch64 conditional width {width}"),
    }
    super::super::validate_frame_protocol_application(
        source,
        manifest,
        &protocol,
        physical,
        &application,
    )
    .unwrap();
    let width = application.fragments.functions[0].blocks[branch_block].instructions[0]
        .bytes
        .len();
    (width, application)
}

#[test]
fn aarch64_frame_growth_widens_conditionals_leaving_imm19_and_keeps_reachable_ones() {
    // The one-word prologue and the epilogue between the branch and its target
    // move the taken edge by one word. Each pair straddles an imm19 boundary:
    // forward 2^20 - 4 becomes 2^20 (out) or stays at 2^20 - 4; backward
    // -2^20 becomes -(2^20 + 4) (out) or stays at -2^20.
    let cases = [
        (false, MEBIBYTE - 16, 8),
        (false, MEBIBYTE - 20, 4),
        (true, MEBIBYTE - 4, 8),
        (true, MEBIBYTE - 8, 4),
    ];
    for conditional in &AARCH64_CONDITIONALS {
        for (backward, filler, expected_width) in cases {
            let source = aarch64_source(conditional, 4, filler, backward);
            let (width, application) =
                apply_and_decode_aarch64_branch(&source, conditional, backward);
            let predicate = conditional.predicate;
            assert_eq!(width, expected_width, "{predicate:?} backward={backward}");
            // Everything after the branch moved by the widening growth.
            let growth = width as u64 - 4;
            let function = &application.fragments.functions[0];
            let epilogues = &application.functions[0].epilogues;
            let (call, expected_call, expected_epilogues) = if backward {
                (
                    &function.blocks[2].instructions[0],
                    16 + filler + growth,
                    [4 + filler, 20 + filler + growth],
                )
            } else {
                (
                    &function.blocks[1].instructions[0],
                    4 + 4 + growth,
                    [8 + filler + 4 + growth, 16 + filler + 4 + growth],
                )
            };
            assert_eq!(call.offset, expected_call);
            let fixup = call.internal_machine_fixup.unwrap();
            assert_eq!(fixup.opcode_function_offset, expected_call);
            assert_eq!(fixup.patch_function_offset, expected_call);
            assert_eq!(fixup.reference_function_offset, expected_call);
            assert_eq!(
                epilogues
                    .iter()
                    .map(|epilogue| epilogue.function_offset)
                    .collect::<Vec<_>>(),
                expected_epilogues
            );
            assert_eq!(function.byte_count, 28 + filler + growth);
            assert_eq!(function.bytes.len() as u64, function.byte_count);
        }
    }
}

#[test]
fn aarch64_layout_widened_conditional_reencodes_its_far_displacement_through_frame_application() {
    // Layout realized these branches as `B.<inverse> +8; B` because the taken
    // edge was already past imm19; frame application re-encodes the pair at
    // the moved displacement instead of the one-word form.
    for conditional in &AARCH64_CONDITIONALS {
        for backward in [false, true] {
            let source = aarch64_source(conditional, 8, MEBIBYTE, backward);
            let (width, _) = apply_and_decode_aarch64_branch(&source, conditional, backward);
            let predicate = conditional.predicate;
            assert_eq!(width, 8, "{predicate:?} backward={backward}");
        }
    }
}

use super::tests::{physical, protocol, source_plan};
use machine_code::{
    FunctionFragmentBlockSpan, FunctionFragmentBranchEvidence,
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentConditionalBranchPredicate,
    FunctionFragmentControlProvenance,
};
use optimization_core::FunctionFragmentEmissionManifestIdentity;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
};
use semantic_vocabulary::EdgeId;

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
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter().flat_map(|row| row.bytes.clone()))
            .collect();
        function.byte_count = function.bytes.len() as u64;
        source.identity = source.recomputed_identity();
        let manifest = FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest");
        let protocol = protocol(source.entry);
        let application =
            crate::apply_frame_protocol_to_fragments(&source, manifest, &protocol, &physical)
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
        crate::validate_frame_protocol_application(
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
            crate::validate_frame_protocol_application(
                &source, manifest, &protocol, &physical, &corrupted,
            )
            .is_err()
        );
    }
}

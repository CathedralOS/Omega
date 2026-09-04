//! Active-resident rematerialization through relocation-free emission on both targets.

use crate::tests::*;

#[test]
fn active_resident_rematerialization_emits_relocation_free_fragments_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let realization = staged_active_resident_allocation_recovery_realization(target);
        let StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(
            rematerialization,
        ) = realization.source()
        else {
            unreachable!("fixture selects active-resident recovery")
        };
        let action = rematerialization.rematerialization().plan().functions[0]
            .action
            .as_ref()
            .expect("the admitted source must retain its rematerialization action");
        let fresh = action.fresh_materialize;
        let transformed_selected = rematerialization
            .rematerialization()
            .receipt()
            .transformed_selected();
        let transformed_homes = rematerialization.homes().receipt();
        let register_environment = rematerialization
            .source()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment()
            .identity();
        let optimized_source = rematerialization
            .source()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .optimized_target()
            .optimized();
        let pre_physical = optimized_source.pre_physical_manifest().record().identity;
        let verified_input = optimized_source.verified_input().clone();
        let source_manifest = realization.manifest().record().clone();
        let mut emitted = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::AllocationRecovery(Box::new(
                realization,
            )),
        )
        .unwrap();

        assert_eq!(
            validate_optimized_function_fragment_emission(&emitted).unwrap(),
            emitted.custody()
        );
        assert_eq!(
            emitted.source().selected_plan().psi,
            emitted.fragments().psi
        );
        assert_eq!(
            emitted.source().register_homes().receipt(),
            transformed_homes
        );
        assert_eq!(
            emitted.source().register_environment().identity(),
            register_environment
        );
        assert_eq!(
            emitted.source().pre_physical_manifest().record().identity,
            pre_physical
        );
        assert_eq!(emitted.source().verified_input(), &verified_input);
        assert_eq!(emitted.fragments().selected, transformed_selected);
        assert_eq!(emitted.manifest().record().selected, transformed_selected);
        assert_eq!(
            emitted.manifest().record().source_realization,
            source_manifest.identity
        );
        assert_eq!(
            source_manifest.allocation_recovery_selections,
            source_manifest.selections
        );
        assert_eq!(
            emitted.manifest().record().source_kind,
            FunctionFragmentEmissionSourceKind::AllocationRecoveryV1
        );

        let fresh_span = emitted.fragments().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.instruction == fresh)
            .expect("the fresh materialization must have an emitted instruction span");
        assert_eq!(
            fresh_span.alternative.family,
            omega_selected_instructions::MachineAlternativeFamily::MaterializeI64
        );
        assert!(!fresh_span.bytes.is_empty());

        let branch = emitted.fragments().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.branch.is_some())
            .expect("the conditional source must retain one resolved branch");
        match target.architecture {
            omega_target::Architecture::X86_64 => {
                assert_eq!(&branch.bytes[..2], [0x0f, 0x85]);
                assert_eq!(branch.bytes.len(), 6);
            }
            omega_target::Architecture::Aarch64 => {
                let instruction = u32::from_le_bytes(branch.bytes.as_slice().try_into().unwrap());
                assert_eq!(instruction & 0xff00_001f, 0x5400_0001);
                assert_eq!(branch.bytes.len(), 4);
            }
        }

        let record = emitted.manifest().record();
        let encoded = record.encode();
        assert_eq!(&encoded[8..12], &10_u32.to_le_bytes());
        assert_eq!(encoded[45], 3);
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&encoded),
            Ok(record.clone())
        );
        let mut unknown_source = encoded;
        unknown_source[45] = 8;
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&unknown_source),
            Err(FunctionFragmentEmissionManifestDecodeError::UnknownSourceKind(8))
        );

        let original_fresh_byte = emitted.fragments().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.instruction == fresh)
            .unwrap()
            .bytes[0];
        emitted.fragments_mut().functions[0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| row.instruction == fresh)
            .unwrap()
            .bytes[0] ^= 1;
        emitted.fragments_mut().identity = emitted.fragments().recomputed_identity();
        assert_eq!(
            validate_optimized_function_fragment_emission(&emitted),
            Err(FunctionFragmentEmissionError::ArtifactMismatch)
        );
        emitted.fragments_mut().functions[0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| row.instruction == fresh)
            .unwrap()
            .bytes[0] = original_fresh_byte;
        emitted.fragments_mut().identity = emitted.fragments().recomputed_identity();
        assert_eq!(
            validate_optimized_function_fragment_emission(&emitted).unwrap(),
            emitted.custody()
        );

        let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
        assert_eq!(
            validate_optimized_relocation_free_text_section(&placed).unwrap(),
            placed.custody()
        );
        assert_eq!(
            placed.text_section().relocation_requirements,
            omega_object_file::TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1
        );
        assert_eq!(
            placed
                .manifest()
                .record()
                .statistics
                .relocation_requirements,
            0
        );
        assert_eq!(
            placed.manifest().record().source_kind,
            FunctionFragmentEmissionSourceKind::AllocationRecoveryV1
        );
        let text_encoded = placed.manifest().record().encode();
        assert_eq!(&text_encoded[8..12], &11_u32.to_le_bytes());
        assert_eq!(text_encoded[45], 1);
        assert_eq!(text_encoded[46], 3);
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&text_encoded),
            Ok(placed.manifest().record().clone())
        );
    }
}

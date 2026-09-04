pub(super) fn assert_exact_effect(
    effect: &omega_regalloc::AbstractSpillMemoryEffect,
    pseudo: &omega_regalloc::HomedSpillPseudoInstruction,
    owner: &omega_regalloc::FunctionHomedSpillPseudoInstructions,
) {
    let storage_id = match pseudo {
        omega_regalloc::HomedSpillPseudoInstruction::Store { storage, .. }
        | omega_regalloc::HomedSpillPseudoInstruction::Reload { storage, .. } => *storage,
    };
    let storage = owner
        .storage
        .iter()
        .find(|row| row.id == storage_id)
        .unwrap();
    match (effect, pseudo) {
        (
            omega_regalloc::AbstractSpillMemoryEffect::Write {
                pseudo,
                action,
                block,
                point,
                before_instruction,
                before_reload,
                source,
                source_view,
                storage: effect_storage,
                storage_class,
                spill_area_offset,
                size_bytes,
                alignment_bytes,
            },
            omega_regalloc::HomedSpillPseudoInstruction::Store {
                id,
                action: expected_action,
                block: expected_block,
                point: expected_point,
                before_instruction: expected_instruction,
                before_reload: expected_reload,
                source: expected_source,
                source_view: expected_view,
                storage: expected_storage,
            },
        ) => {
            assert_eq!(
                (*pseudo, *action, *block, *point, *before_instruction),
                (
                    *id,
                    *expected_action,
                    *expected_block,
                    *expected_point,
                    *expected_instruction
                ),
            );
            assert_eq!(
                (*before_reload, *source, *source_view, *effect_storage),
                (
                    *expected_reload,
                    *expected_source,
                    *expected_view,
                    *expected_storage
                ),
            );
            assert_geometry(
                (
                    *storage_class,
                    *spill_area_offset,
                    *size_bytes,
                    *alignment_bytes,
                ),
                storage,
            );
        }
        (
            omega_regalloc::AbstractSpillMemoryEffect::Read {
                pseudo,
                action,
                block,
                point,
                before_instruction,
                storage: effect_storage,
                storage_class,
                spill_area_offset,
                size_bytes,
                alignment_bytes,
                result,
                destination_class,
                destination_view,
            },
            omega_regalloc::HomedSpillPseudoInstruction::Reload {
                id,
                action: expected_action,
                block: expected_block,
                point: expected_point,
                before_instruction: expected_instruction,
                storage: expected_storage,
                result: expected_result,
                destination_class: expected_class,
                destination_view: expected_view,
            },
        ) => {
            assert_eq!(
                (*pseudo, *action, *block, *point, *before_instruction),
                (
                    *id,
                    *expected_action,
                    *expected_block,
                    *expected_point,
                    *expected_instruction
                ),
            );
            assert_eq!(
                (
                    *effect_storage,
                    *result,
                    *destination_class,
                    *destination_view
                ),
                (
                    *expected_storage,
                    *expected_result,
                    *expected_class,
                    *expected_view
                ),
            );
            assert_geometry(
                (
                    *storage_class,
                    *spill_area_offset,
                    *size_bytes,
                    *alignment_bytes,
                ),
                storage,
            );
        }
        _ => panic!("abstract access kind must match its homed pseudo"),
    }
}

fn assert_geometry(
    actual: (omega_regalloc::LogicalSpillStorageClass, u64, u64, u64),
    storage: &omega_regalloc::SpillPseudoStorage,
) {
    assert_eq!(
        actual,
        (
            storage.class,
            storage.spill_area_offset,
            storage.size_bytes,
            storage.alignment_bytes,
        ),
    );
}

use omega_object_file::{
    ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan, RelocationRecord,
    SectionKind,
};
use psi_diagnostics::Diagnostic;
use psi_layout_plans::{
    ByteOrder, MaterializationAction, RelocationTarget, SymbolicMaterializationPlan,
};

/// Appends only the loader-native relocation actions from a symbolic
/// materialization plan. Resolved writes and post-handoff writer actions have
/// different consumers and remain in the materialization plan.
pub fn append_native_materialization_relocations(
    materialization: &SymbolicMaterializationPlan,
    section: SectionKind,
    section_base_offset: usize,
    owner_symbol_handle: ObjectSymbolHandle,
    relocations: &mut RelocationPlan,
    mut target_symbol: impl FnMut(RelocationTarget) -> Option<ObjectSymbolHandle>,
) -> Result<usize, Diagnostic> {
    if materialization.byte_order != ByteOrder::LittleEndian {
        return Err(Diagnostic::error(
            "native absolute relocations currently require a little-endian materialization plan",
        ));
    }
    if section == SectionKind::Bss {
        return Err(Diagnostic::error(
            "symbolic materialization relocations require an initialized text or data section",
        ));
    }
    if !owner_symbol_handle.is_valid() {
        return Err(Diagnostic::error(
            "symbolic materialization relocation requires a valid owner object symbol",
        ));
    }

    let mut appended = 0;
    for action in &materialization.actions {
        let MaterializationAction::NativePointerRelocation {
            target,
            destination_byte_offset,
            width_bits,
            ..
        } = action
        else {
            continue;
        };
        if *width_bits != 64 {
            return Err(Diagnostic::error(format!(
                "object relocation lowering does not support a {width_bits}-bit native pointer relocation"
            )));
        }
        let local_offset = usize::try_from(*destination_byte_offset).map_err(|_| {
            Diagnostic::error("materialization relocation offset does not fit the compiler host")
        })?;
        let local_end = local_offset
            .checked_add(8)
            .ok_or_else(|| Diagnostic::error("materialization relocation byte range overflows"))?;
        if local_end > materialization.byte_len {
            return Err(Diagnostic::error(format!(
                "materialization relocation at {local_offset}..{local_end} exceeds the {}-byte materialization",
                materialization.byte_len
            )));
        }
        let offset = section_base_offset
            .checked_add(local_offset)
            .ok_or_else(|| Diagnostic::error("materialization section offset overflows"))?;
        let symbol_handle = target_symbol(*target)
            .filter(|handle| handle.is_valid())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "symbolic relocation target {target:?} has no object symbol"
                ))
            })?;
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Materialization {
                object_symbol_handle: owner_symbol_handle,
            },
            section,
            offset,
            byte_width: 8,
            symbol_handle,
            addend: 0,
            kind: RelocationKind::Absolute64,
        });
        appended += 1;
    }

    Ok(appended)
}

#[cfg(test)]
mod tests {
    use super::append_native_materialization_relocations;
    use omega_object_file::{RelocationKind, RelocationOrigin, RelocationPlan, SectionKind};
    use omega_target::NativeTarget;
    use psi_arena::Handle;
    use psi_layout_plans::{
        ByteOrder, EntryStubId, MaterializationAction, PlacementConstraints, PlacementPhase,
        RelocationTarget, SymbolicMaterializationPlan,
    };

    #[test]
    fn native_pointer_action_becomes_a_data_relocation_with_materialization_origin() {
        let target = RelocationTarget::Entry(
            EntryStubId::from_normalized_identity(7).expect("normalized entry"),
        );
        let materialization = SymbolicMaterializationPlan {
            byte_len: 16,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
            actions: vec![MaterializationAction::NativePointerRelocation {
                field: "entry".into(),
                target,
                destination_byte_offset: 8,
                width_bits: 64,
            }],
        };
        let owner = Handle::from_arena_index(2);
        let entry_symbol = Handle::from_arena_index(3);
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());

        let appended = append_native_materialization_relocations(
            &materialization,
            SectionKind::Data,
            32,
            owner,
            &mut relocations,
            |candidate| (candidate == target).then_some(entry_symbol),
        )
        .expect("native action should lower");

        assert_eq!(appended, 1);
        let record = relocations.records().next().expect("relocation record").1;
        assert_eq!(record.section, SectionKind::Data);
        assert_eq!(record.offset, 40);
        assert_eq!(record.kind, RelocationKind::Absolute64);
        assert_eq!(
            record.origin,
            RelocationOrigin::Materialization {
                object_symbol_handle: owner
            }
        );
    }

    #[test]
    fn native_pointer_action_rejects_bss_destination() {
        let materialization = SymbolicMaterializationPlan {
            byte_len: 8,
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
            actions: Vec::new(),
        };
        let error = append_native_materialization_relocations(
            &materialization,
            SectionKind::Bss,
            0,
            Handle::from_arena_index(1),
            &mut RelocationPlan::with_target(NativeTarget::linux_x64()),
            |_| None,
        )
        .expect_err("BSS has no bytes to relocate");
        assert!(error.message.contains("initialized"));
    }
}

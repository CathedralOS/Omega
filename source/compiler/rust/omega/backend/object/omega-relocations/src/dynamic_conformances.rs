use omega_control_flow::MachineFunctionIdentity;
use omega_object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan,
    RelocationRecord, SectionKind, SymbolKind, SymbolSection, object_function_symbol,
};
use omega_target::NativeTarget;
use omega_target_operations::{TargetDataObjectKind, TargetDataPlan};
use psi_diagnostics::Diagnostic;

pub(super) fn collect_dynamic_conformance_table_relocations(
    target: NativeTarget,
    data: &TargetDataPlan,
    object: &ObjectPlan,
    relocations: &mut RelocationPlan,
) -> Result<(), Diagnostic> {
    let pointer_size = target.pointer_size;
    if pointer_size != 8 {
        return Err(Diagnostic::error(format!(
            "dynamic conformance table relocation requires 64-bit pointers, found {pointer_size} bytes"
        )));
    }
    for (_, table) in data.dynamic_conformance_tables.iter() {
        if !data.objects.is_valid(table.object) {
            return Err(Diagnostic::error(
                "dynamic conformance table names an invalid target-data object",
            ));
        }
        let data_object = data.objects.get(table.object);
        let bytes = data.bytes.span(data_object.bytes).ok_or_else(|| {
            Diagnostic::error("dynamic conformance table has an invalid target-data byte span")
        })?;
        let expected_size = table
            .rows
            .len()
            .checked_mul(pointer_size)
            .ok_or_else(|| Diagnostic::error("dynamic conformance table size overflow"))?;
        if table.rows.is_empty()
            || data_object.kind != TargetDataObjectKind::DynamicConformanceTable
            || data_object.alignment < pointer_size
            || data_object.offset % pointer_size != 0
            || bytes.len() != expected_size
            || bytes.iter().any(|byte| *byte != 0)
        {
            return Err(Diagnostic::error(
                "dynamic conformance table target-data object has invalid slot bytes or alignment",
            ));
        }
        let owner_symbol = exact_table_object_symbol(object, data_object)?;
        let mut prior_requirement: Option<&str> = None;
        for (index, row) in table.rows.iter().enumerate() {
            if row.requirement_identity.is_empty()
                || row.realization_identity.is_empty()
                || !row.realization.is_valid()
                || prior_requirement.is_some_and(|prior| prior >= row.requirement_identity.as_ref())
            {
                return Err(Diagnostic::error(
                    "dynamic conformance table rows lost strict normalized requirement order or exact realization identity",
                ));
            }
            prior_requirement = Some(row.requirement_identity.as_ref());
            let target_symbol = object_function_symbol(
                object,
                MachineFunctionIdentity::source(row.realization),
            )
            .map(|(handle, _)| handle)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "dynamic conformance table realization `{}` has no exact private function symbol",
                    row.realization_identity
                ))
            })?;
            let slot_offset = index
                .checked_mul(pointer_size)
                .and_then(|local| data_object.offset.checked_add(local))
                .ok_or_else(|| {
                    Diagnostic::error("dynamic conformance table slot offset overflow")
                })?;
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: owner_symbol,
                },
                section: SectionKind::Data,
                offset: slot_offset,
                byte_width: pointer_size,
                symbol_handle: target_symbol,
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }
    }
    Ok(())
}

fn exact_table_object_symbol(
    object: &ObjectPlan,
    data_object: &omega_target_operations::TargetDataObject,
) -> Result<ObjectSymbolHandle, Diagnostic> {
    let mut matches = object
        .layout
        .symbols
        .iter()
        .filter(|(_, symbol)| symbol.name == data_object.symbol.as_ref());
    let (handle, symbol) = matches.next().ok_or_else(|| {
        Diagnostic::error("dynamic conformance table has no private data object symbol")
    })?;
    if matches.next().is_some()
        || symbol.kind != SymbolKind::Object
        || symbol.section != SymbolSection::Section(SectionKind::Data)
        || symbol.offset != data_object.offset
        || symbol.size != data_object.bytes.count() as usize
    {
        return Err(Diagnostic::error(
            "dynamic conformance table private data object symbol is duplicated or shape-incoherent",
        ));
    }
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_control_flow::StateKey;
    use omega_object_file::{FunctionSymbolPlan, ObjectPlan, SymbolPlan};
    use omega_target_operations::{
        DynamicConformanceTable, DynamicConformanceTableRow, TargetDataObject,
    };
    use psi_symbols::SymbolHandle;
    use std::sync::Arc;

    fn state(index: u32) -> StateKey {
        StateKey {
            machine: SymbolHandle::from_arena_index(index),
            state: SymbolHandle::from_arena_index(index + 10),
            segment_index: 0,
        }
    }

    fn fixture() -> (TargetDataPlan, ObjectPlan, Vec<ObjectSymbolHandle>) {
        let target = NativeTarget::linux_x64();
        let mut data = TargetDataPlan::with_capacity(1, 16);
        let bytes = data.bytes.insert_many([0; 16]);
        let object_handle = data.objects.insert(TargetDataObject {
            symbol: Arc::from("omega_dynamic_conformance_fixture"),
            kind: TargetDataObjectKind::DynamicConformanceTable,
            offset: 0,
            bytes,
            alignment: 8,
            ..TargetDataObject::default()
        });
        data.dynamic_conformance_tables
            .insert(DynamicConformanceTable {
                object: object_handle,
                trait_identity: Arc::from("Shape"),
                conformance_identity: Arc::from("Item::Primary"),
                rows: vec![
                    DynamicConformanceTableRow {
                        requirement_identity: Arc::from("requirement-a"),
                        realization_identity: Arc::from("realization-a"),
                        realization: state(1),
                    },
                    DynamicConformanceTableRow {
                        requirement_identity: Arc::from("requirement-b"),
                        realization_identity: Arc::from("realization-b"),
                        realization: state(2),
                    },
                ],
            });

        let mut object = ObjectPlan::with_capacities(target, 0, 3, 2);
        object.layout.symbols.insert(SymbolPlan {
            name: "omega_dynamic_conformance_fixture".to_owned(),
            section: SymbolSection::Section(SectionKind::Data),
            offset: 0,
            size: 16,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let mut targets = Vec::new();
        for (index, key) in [state(1), state(2)].into_iter().enumerate() {
            let symbol = object.layout.symbols.insert(SymbolPlan {
                name: format!("function-{index}"),
                section: SymbolSection::Section(SectionKind::Text),
                offset: index * 8,
                size: 8,
                kind: SymbolKind::Function,
                import_library: String::new(),
            });
            object.layout.function_symbols.insert(FunctionSymbolPlan {
                identity: MachineFunctionIdentity::source(key),
                symbol,
            });
            targets.push(symbol);
        }
        (data, object, targets)
    }

    #[test]
    fn exact_table_rows_become_private_data_absolute_relocations() {
        let (data, object, targets) = fixture();
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        collect_dynamic_conformance_table_relocations(
            NativeTarget::linux_x64(),
            &data,
            &object,
            &mut relocations,
        )
        .expect("table relocations");
        let records = relocations
            .records()
            .map(|(_, row)| row)
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 2);
        assert_eq!((records[0].offset, records[1].offset), (0, 8));
        assert_eq!(
            (records[0].symbol_handle, records[1].symbol_handle),
            (targets[0], targets[1])
        );
        assert!(records.iter().all(|record| {
            record.section == SectionKind::Data
                && record.byte_width == 8
                && record.kind == RelocationKind::Absolute64
                && record.addend == 0
                && matches!(record.origin, RelocationOrigin::Materialization { .. })
        }));
    }

    #[test]
    fn table_relocation_rejects_byte_and_exact_function_identity_drift() {
        let (mut data, object, _) = fixture();
        *data
            .bytes
            .get_mut(data.objects.storage_slice()[0].bytes.start()) = 1;
        let error = collect_dynamic_conformance_table_relocations(
            NativeTarget::linux_x64(),
            &data,
            &object,
            &mut RelocationPlan::with_target(NativeTarget::linux_x64()),
        )
        .expect_err("nonzero unrelocated slot must reject");
        assert!(error.message.contains("slot bytes"));

        let (data, mut object, targets) = fixture();
        object.layout.function_symbols.insert(FunctionSymbolPlan {
            identity: MachineFunctionIdentity::source(state(1)),
            symbol: targets[0],
        });
        let error = collect_dynamic_conformance_table_relocations(
            NativeTarget::linux_x64(),
            &data,
            &object,
            &mut RelocationPlan::with_target(NativeTarget::linux_x64()),
        )
        .expect_err("duplicate exact function target must reject");
        assert!(error.message.contains("no exact private function symbol"));
    }
}

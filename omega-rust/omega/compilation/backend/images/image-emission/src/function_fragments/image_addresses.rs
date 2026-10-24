//! One relocation mechanism for image addresses; placement owners supply only
//! the referent symbol and addend, not another ISA-specific patching path. The
//! same owner selects the rows an observation owns in the published plan, so a
//! pool's replay compares rows rather than keeping a parallel count.

use super::{Error, host};
use machine_code::ImageAddressFields;
use object_file::{
    ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan, RelocationRecord,
    SectionKind,
};

pub(super) fn records(
    fields: ImageAddressFields,
    origin: RelocationOrigin,
    destination: ObjectSymbolHandle,
    addend: i64,
) -> Result<Vec<RelocationRecord>, Error> {
    let record = |offset, kind| -> Result<RelocationRecord, Error> {
        Ok(RelocationRecord {
            origin,
            section: SectionKind::Text,
            offset: host(offset)?,
            byte_width: 4,
            symbol_handle: destination,
            addend,
            kind,
        })
    };
    match fields {
        ImageAddressFields::X86_64Relative32 { patch_offset } => Ok(vec![record(
            patch_offset,
            RelocationKind::X86_64Relative32,
        )?]),
        ImageAddressFields::Aarch64PageAddress {
            page_patch_offset,
            page_offset_patch_offset,
        } => Ok(vec![
            record(page_patch_offset, RelocationKind::Aarch64Page21)?,
            record(
                page_offset_patch_offset,
                RelocationKind::Aarch64PageOffset12,
            )?,
        ]),
    }
}

/// The plan's rows one image object identity owns: those bound to one of
/// `symbols`, plus rows whose materialization origin names one. A placement
/// owner publishes exactly these rows, and its replay compares the plan to the
/// roster it derives from the retained observations.
pub(super) fn rows_for_symbols(
    plan: &RelocationPlan,
    symbols: impl IntoIterator<Item = ObjectSymbolHandle>,
) -> Vec<RelocationRecord> {
    let symbols = symbols.into_iter().collect::<Vec<_>>();
    plan.records()
        .filter(|(_, record)| {
            symbols.iter().any(|symbol| {
                record.symbol_handle == *symbol
                    || record.origin
                        == RelocationOrigin::Materialization {
                            object_symbol_handle: *symbol,
                        }
            })
        })
        .map(|(_, record)| record.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::rows_for_symbols;
    use object_file::{
        ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan,
        RelocationRecord, SectionKind, SymbolKind, SymbolPlan, SymbolSection,
    };

    fn symbol(object: &mut ObjectPlan, name: &str) -> ObjectSymbolHandle {
        object.layout.symbols.insert(SymbolPlan {
            name: name.into(),
            section: SymbolSection::Section(SectionKind::Text),
            offset: 0,
            size: 0,
            kind: SymbolKind::Object,
            import_library: String::new(),
        })
    }

    fn record(
        origin: RelocationOrigin,
        symbol_handle: ObjectSymbolHandle,
        offset: usize,
    ) -> RelocationRecord {
        RelocationRecord {
            origin,
            section: SectionKind::Text,
            offset,
            byte_width: 4,
            symbol_handle,
            addend: 0,
            kind: RelocationKind::X86_64Relative32,
        }
    }

    #[test]
    fn owned_rows_join_binding_symbols_and_materialization_origins_in_plan_order() {
        let mut object = ObjectPlan::with_capacity(target::NativeTarget::macos_arm64(), 1, 2);
        let first = symbol(&mut object, "first");
        let second = symbol(&mut object, "second");
        let mut plan = RelocationPlan::with_target(object.target);
        plan.push_record(record(
            RelocationOrigin::Materialization {
                object_symbol_handle: second,
            },
            first,
            0,
        ));
        plan.push_record(record(
            RelocationOrigin::Materialization {
                object_symbol_handle: first,
            },
            first,
            8,
        ));
        plan.push_record(record(
            RelocationOrigin::Materialization {
                object_symbol_handle: second,
            },
            second,
            16,
        ));
        // One row binds `first` directly, one materialization row names it as
        // the owner, and the row bound to `second` is not this identity's.
        let rows = rows_for_symbols(&plan, [first]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].offset, 0);
        assert_eq!(rows[1].offset, 8);
        // Both symbol bindings and materialization origins select, in plan order.
        let both = rows_for_symbols(&plan, [second, first]);
        assert_eq!(
            both.iter().map(|row| row.offset).collect::<Vec<_>>(),
            [0, 8, 16]
        );
        assert!(rows_for_symbols(&plan, []).is_empty());
    }
}

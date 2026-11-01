//! Exercise native layout identity from retained and decoded artifacts only.

use semantic_vocabulary::StructuralTypeId;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{NativeLayoutCatalog, NativeLayoutRealization, TerminalModule};
use terminal_psi_to_abstract_operations::RetainedTerminalArtifact;

pub(super) fn inspect(retained: &RetainedTerminalArtifact) -> Result<(), String> {
    let decoded = CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
        .map_err(|error| format!("native layout transport: {error:?}"))?;
    let module = retained.artifact().semantic_module();
    if module.native_layouts != decoded.semantic_module().native_layouts {
        return Err("native layout catalog changed during transport".into());
    }
    let records = module
        .native_layouts
        .records()
        .filter_map(|record| {
            module
                .native_layouts
                .realizations(record)
                .ok()
                .filter(|rows| {
                    rows.iter()
                        .any(|row| !row.report.private_callback_demands.is_empty())
                })
                .map(|_| record.record)
        })
        .collect::<Vec<_>>();
    if records.len() != 2 {
        return Err(
            "native layout witness must retain two private-slot record applications".into(),
        );
    }
    let targets = [
        "macos_arm64",
        "linux_x86_64",
        "linux_arm64",
        "windows_x86_64",
    ];
    let mut varying = None;
    for record in &records {
        let mut sizes = Vec::new();
        let mut identities = Vec::new();
        for target in targets {
            let profile = target::TargetProfile::from_omega_target_name(Some(target))
                .map_err(|error| error.to_string())?;
            let original = layout::terminal_layout_identity(module, *record, profile)?
                .ok_or("private-slot record lost its authored report")?;
            let loaded =
                layout::terminal_layout_identity(decoded.semantic_module(), *record, profile)?
                    .ok_or("decoded record lost its authored report")?;
            if original.data_layout != loaded.data_layout
                || original.slot_layout != loaded.slot_layout
                || original.realization != loaded.realization
                || original.data_layout == original.slot_layout
            {
                return Err("retained/decoded native layout coordinates differ".into());
            }
            let [slot] = original
                .realization
                .report
                .private_callback_demands
                .as_slice()
            else {
                return Err("native layout lost its exact private slot".into());
            };
            slot.slot_application.validate_identity_encoding()?;
            if slot.offset != 8 || slot.layout_subject_identity != original.policy_identity {
                return Err(
                    "native layout changed its selected private slot owner or offset".into(),
                );
            }
            sizes.push(
                original
                    .realization
                    .report
                    .layout
                    .size
                    .ok_or("native layout has no extent")?,
            );
            identities.push(original.data_layout);
        }
        match sizes.as_slice() {
            [16, 16, 16, 16] => {
                if identities.windows(2).any(|pair| pair[0] != pair[1]) {
                    return Err(
                        "invariant equal-geometry layout acquired target-specific identity".into(),
                    );
                }
            }
            [16, 16, 16, 24] => {
                if identities[0] == identities[3] {
                    return Err("distinct target layouts collapsed to one native identity".into());
                }
                varying = Some(*record);
            }
            _ => {
                return Err(format!(
                    "native layout witness has unexpected target extents {sizes:?}"
                ));
            }
        }
    }
    let record = varying.ok_or("native layout witness lost its target-varying application")?;
    let mut enclosing = module.structural_types.iter().filter(|declaration| {
        let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
            return false;
        };
        records.iter().all(|record| {
            fields
                .iter()
                .any(|field| field.field_type.as_structural() == Some(*record))
        })
    });
    let enclosing = enclosing
        .next()
        .map(|row| row.id)
        .filter(|_| enclosing.next().is_none())
        .ok_or("native layout witness needs one exact enclosing record")?;
    for target in targets {
        let profile = target::TargetProfile::from_omega_target_name(Some(target))
            .map_err(|error| error.to_string())?;
        let original = callback_paths(module, enclosing, profile)?;
        let loaded = callback_paths(decoded.semantic_module(), enclosing, profile)?;
        if original != loaded || original.len() != 2 {
            return Err("retained/decoded callback paths differ or omit a private slot".into());
        }
        let mut offsets = original
            .iter()
            .map(|row| row.field_offset(profile.native_target()))
            .collect::<Result<Vec<_>, _>>()?;
        offsets.sort_unstable();
        if offsets != [8, 24]
            || original.iter().any(|row| {
                !row.fields().is_ok_and(|fields| fields.len() == 1)
                    || row.terminal.offset != 8
                    || row.root.size != if target == "windows_x86_64" { 48 } else { 40 }
            })
        {
            return Err(
                "canonical callback closure lost the enclosing path or selected extent".into(),
            );
        }
    }
    // Ingress checks nominal provenance; native closure additionally owns
    // semantic non-overlap and pointer-sized containment.
    for (mutation, reason) in [
        (Mutation::SlotOverlap, "overlaps semantic field storage"),
        (
            Mutation::SlotOverflow,
            "invalid target alignment or containing extent",
        ),
    ] {
        let mut changed = module.clone();
        changed.native_layouts = mutate(module, record, mutation)?;
        let profile = target::TargetProfile::from_omega_target_name(Some("windows_x86_64"))
            .map_err(|error| error.to_string())?;
        match callback_paths(&changed, enclosing, profile) {
            Err(observed) if observed.contains(reason) => {}
            Err(observed) => {
                return Err(format!(
                    "callback {mutation:?} rejected for unrelated reason: {observed}"
                ));
            }
            Ok(_) => return Err(format!("canonical callback closure accepted {mutation:?}")),
        }
    }
    for mutation in [
        Mutation::DropRecord,
        Mutation::Extent,
        Mutation::Preimage,
        Mutation::Owner,
        Mutation::Semantic,
        Mutation::Target,
        Mutation::Fields,
    ] {
        let mut changed = module.clone();
        changed.native_layouts = mutate(module, record, mutation)?;
        let encoded = terminal_codec::encode_module(&changed)
            .map_err(|error| format!("cannot encode {mutation:?}: {error:?}"))?;
        match terminal_codec::decode_module(&encoded) {
            Err(terminal_codec::CodecError::NativeLayout(_)) => {}
            Err(error) => {
                return Err(format!(
                    "native layout {mutation:?} rejected for an unrelated reason: {error:?}"
                ));
            }
            Ok(_) => return Err(format!("native layout ingress accepted {mutation:?}")),
        }
    }
    Ok(())
}

fn callback_paths(
    module: &TerminalModule,
    record: StructuralTypeId,
    target: target::TargetProfile,
) -> Result<Vec<calling_conventions::BoundaryCallbackLayout>, String> {
    let mut layouts = layout::TerminalCallbackLayouts::new(module, target, |application| {
        Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
            application,
        ))
    });
    layouts.for_parameter(layout::CallbackStorageRoot {
        record,
        parameter: calling_conventions::NativeParameterId::new(1)
            .ok_or("callback control has an absent parameter identity")?,
        formal_ordinal: 0,
        native_ordinal: 0,
    })
}

#[derive(Debug, Clone, Copy)]
enum Mutation {
    DropRecord,
    Extent,
    Preimage,
    Owner,
    Semantic,
    Target,
    Fields,
    SlotOverlap,
    SlotOverflow,
}

fn mutate(
    module: &TerminalModule,
    selected: StructuralTypeId,
    mutation: Mutation,
) -> Result<NativeLayoutCatalog, String> {
    let mut changed = NativeLayoutCatalog::default();
    for record in module.native_layouts.records() {
        if record.record == selected && matches!(mutation, Mutation::DropRecord) {
            continue;
        }
        let mut fields = module.native_layouts.fields(record)?.to_vec();
        let mut rows: Vec<NativeLayoutRealization> =
            module.native_layouts.realizations(record)?.to_vec();
        if record.record == selected {
            let row = rows.first_mut().ok_or("mutation needs a target layout")?;
            match mutation {
                Mutation::Extent => {
                    row.report.layout.size = row.report.layout.size.map(|size| size + 8)
                }
                Mutation::Preimage => {
                    let slot = row
                        .report
                        .private_callback_demands
                        .first_mut()
                        .ok_or("mutation needs a private slot")?;
                    std::sync::Arc::make_mut(&mut slot.slot_application.application_encoding)[0] ^=
                        1;
                }
                Mutation::Owner => {
                    row.report.private_callback_demands[0]
                        .slot_application
                        .layout_identity = "foreign::Policy".into()
                }
                Mutation::Target => {
                    row.target
                        .as_mut()
                        .ok_or("mutation needs target facts")?
                        .pointer_size = 4
                }
                Mutation::Fields => fields.clear(),
                Mutation::Semantic => {}
                Mutation::SlotOverlap | Mutation::SlotOverflow => {
                    for row in &mut rows {
                        row.report.private_callback_demands[0].offset = match mutation {
                            Mutation::SlotOverlap => 0,
                            _ => row
                                .report
                                .layout
                                .size
                                .ok_or("mutation needs a fixed extent")?,
                        };
                    }
                }
                Mutation::DropRecord => {}
            }
        }
        changed.insert(
            record.record,
            record.data_identity,
            record.policy_identity,
            if record.record == selected && matches!(mutation, Mutation::Semantic) {
                record.policy_identity
            } else {
                record.semantic_identity
            },
            fields,
            rows,
        );
    }
    Ok(changed)
}

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
    for mutation in [
        Mutation::DropRecord,
        Mutation::Extent,
        Mutation::Preimage,
        Mutation::Owner,
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

#[derive(Debug, Clone, Copy)]
enum Mutation {
    DropRecord,
    Extent,
    Preimage,
    Owner,
    Target,
    Fields,
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
                Mutation::DropRecord => {}
            }
        }
        changed.insert(
            record.record,
            record.data_identity,
            record.policy_identity,
            fields,
            rows,
        );
    }
    Ok(changed)
}

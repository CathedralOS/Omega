//! Invalid exchange controls for actual Source-produced dynamic image views.
//! The corpus fixture owns the source and runtime oracle. These mutations test
//! canonical admission, never create a source proof or execute a forged module.

use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{StructuralAccess, TerminalDynamicReferenceOrigin};
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorStoredOrigin, terminal_graph,
};

pub(super) fn inspect(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    let module = artifact.semantic_module();
    let graph = terminal_graph(module);
    let mut analysis = DescriptorAnalysis::new(&module.structural_types, &graph);
    let mut literal_rows = Vec::new();
    for table in &module.dynamic_dispatch.tables {
        for row in table.rows.iter().flatten() {
            let slots = analysis
                .result_slots(*row)
                .ok_or("image control row has unknown descriptor results")?;
            if !slots.is_empty()
                && slots.iter().all(|slot| {
                    !slot.origins.is_empty()
                        && slot
                            .origins
                            .iter()
                            .all(|origin| matches!(origin, DescriptorStoredOrigin::Literal { .. }))
                })
            {
                literal_rows.push(*row);
            }
        }
    }
    literal_rows.sort();
    literal_rows.dedup();
    let [literal_row] = literal_rows.as_slice() else {
        return Err("image controls require one wholly literal-returning dynamic row".into());
    };

    // Retain the exact image body and its table identity. Absent rows are legal
    // dynamic catalog entries; removing the input-returning sibling prevents
    // its earlier input-coverage refusal from masking the image-origin guard.
    let mut image = module.clone();
    for table in &mut image.dynamic_dispatch.tables {
        for row in &mut table.rows {
            if row.as_ref() != Some(literal_row) {
                *row = None;
            }
        }
    }
    terminal_codec::encode_module(&image)
        .and_then(|bytes| terminal_codec::decode_module(&bytes))
        .map_err(|error| format!("isolated image row is not admissible: {error:?}"))?;

    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut invalid = image.clone();
        let mut changed = false;
        for interface in &mut invalid.dynamic_dispatch.interfaces {
            for slot in &mut interface.slots {
                for region in &mut slot.result_reference_regions {
                    if region.access != StructuralAccess::SharedBorrow {
                        return Err("image control requires shared public view regions".into());
                    }
                    region.access = access;
                    changed = true;
                }
            }
        }
        if !changed {
            return Err("image control has no public reference region".into());
        }
        // The unchanged result leaf is shared. This checks the earlier public
        // access boundary, not the later literal-origin branch in isolation.
        super::reject_module_at_ingress(
            &invalid,
            "image view with mutable or write-only public region",
            "dynamic reference region disagrees with result access",
        )?;
    }

    let mut invalid = image;
    let mut changed = false;
    for interface in &mut invalid.dynamic_dispatch.interfaces {
        for slot in &mut interface.slots {
            for region in &mut slot.result_reference_regions {
                let TerminalDynamicReferenceOrigin::Parameter { argument_index } = region.origin
                else {
                    return Err("image control requires an input-relative public region".into());
                };
                let parameter = usize::try_from(argument_index)
                    .ok()
                    .and_then(|position| slot.structural_parameters.get(position))
                    .ok_or("image control region has no exact parameter")?;
                region.origin = TerminalDynamicReferenceOrigin::Invocation {
                    carrier_type: parameter.structural_type,
                };
                changed = true;
            }
        }
    }
    if !changed {
        return Err("image invocation control has no public reference region".into());
    }
    // Keep each complete typed field path and its shared access. The body still
    // returns the original immutable literals, not provider invocation storage.
    super::reject_module_at_ingress(
        &invalid,
        "literal view advertised as invocation storage",
        "dynamic literal view has no shared input-region contract",
    )
}

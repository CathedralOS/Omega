//! Closing private callback demands and their two-hop paths.

use crate::layout::builder::semantic_ranges::{plan_laid_semantic_ranges, ranges_overlap};
use crate::layout::{
    DataLayout, DataShape, FieldLayout, TargetClosedPlanLaidDataLayoutIdentity,
    TargetClosedPrivateCallbackDemand, TargetClosedTwoHopPrivateCallbackPath, TypeLayoutDescriptor,
};
use abstract_operations_to_target_operations::calling_conventions::{
    callback_layout_field_slot_id, callback_layout_slot_id, callback_requirement_id,
};
use arena::Arena;
use diagnostics::Diagnostic;
use target::NativeTarget;
use typed_trees_to_checked_trees::checked_trees::CheckedTrees;

pub(crate) fn close_two_hop_private_callback_paths(
    program: &CheckedTrees,
    data_layouts: &Arena<DataLayout>,
    fields: &Arena<FieldLayout>,
    layout_identities: &[TargetClosedPlanLaidDataLayoutIdentity],
    private_demands: &[TargetClosedPrivateCallbackDemand],
) -> Result<Vec<TargetClosedTwoHopPrivateCallbackPath>, Diagnostic> {
    let mut paths = Vec::new();
    for (root_layout_index, root_identity) in layout_identities.iter().enumerate() {
        let matching_roots = data_layouts
            .iter()
            .filter(|(_, layout)| layout.symbol == root_identity.data_symbol)
            .collect::<Vec<_>>();
        let [(_, root)] = matching_roots.as_slice() else {
            return Err(Diagnostic::error(format!(
                "target-closed plan-laid root resolves to {} exact data layouts",
                matching_roots.len()
            )));
        };
        if root.layout != root_identity.physical {
            return Err(Diagnostic::error(
                "target-closed plan-laid root changed its physical layout",
            ));
        }
        let DataShape::Record {
            fields: root_fields,
        } = root.shape
        else {
            continue;
        };
        let root_field_layouts = fields.span(root_fields).ok_or_else(|| {
            Diagnostic::error("target-closed plan-laid root retained an invalid field span")
        })?;
        for (field_ordinal, field_layout) in root_field_layouts.iter().enumerate() {
            let TypeLayoutDescriptor::Named {
                symbol: child_symbol,
                ..
            } = field_layout.type_descriptor
            else {
                continue;
            };
            if root_field_layouts
                .iter()
                .filter(|candidate| candidate.symbol == field_layout.symbol)
                .count()
                != 1
            {
                return Err(Diagnostic::error(
                    "target-closed callback path field symbol is not unique in its root record",
                ));
            }
            if field_layout.type_symbol.is_valid() && field_layout.type_symbol != child_symbol {
                return Err(Diagnostic::error(format!(
                    "target-closed callback path field changed its exact named child symbol from {:?} to {:?}",
                    field_layout.type_symbol, child_symbol
                )));
            }
            let matching_children = layout_identities
                .iter()
                .enumerate()
                .filter(|(_, identity)| identity.data_symbol == child_symbol)
                .collect::<Vec<_>>();
            let [(child_layout_index, child_identity)] = matching_children.as_slice() else {
                continue;
            };
            if root_identity.data_symbol == child_identity.data_symbol {
                return Err(Diagnostic::error(
                    "target-closed callback path cannot recursively contain its root layout",
                ));
            }
            let matching_child_data = data_layouts
                .iter()
                .filter(|(_, layout)| layout.symbol == child_symbol)
                .collect::<Vec<_>>();
            let [(_, child_data)] = matching_child_data.as_slice() else {
                return Err(Diagnostic::error(format!(
                    "target-closed callback path child resolves to {} exact data layouts",
                    matching_child_data.len()
                )));
            };
            if !matches!(child_data.shape, DataShape::Record { .. })
                || child_data.layout != child_identity.physical
                || field_layout.layout != child_identity.physical
                || field_layout.layout.alignment == 0
                || !field_layout
                    .offset
                    .is_multiple_of(field_layout.layout.alignment)
                || field_layout
                    .offset
                    .checked_add(field_layout.layout.size)
                    .is_none_or(|end| end > root_identity.physical.size)
            {
                return Err(Diagnostic::error(
                    "target-closed callback path changed its exact inline child geometry",
                ));
            }
            let field_identity = program
                .normalized_hermetic_symbol_identity(field_layout.symbol)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "target-closed callback path cannot rederive its exact field identity: {reason}"
                    ))
                })?;
            let field_slot = callback_layout_field_slot_id(root_identity.layout, &field_identity);
            let field_index = root_fields
                .start()
                .arena_index()
                .checked_add(u32::try_from(field_ordinal).map_err(|_| {
                    Diagnostic::error("target-closed callback path field ordinal overflowed")
                })?)
                .ok_or_else(|| {
                    Diagnostic::error("target-closed callback path field handle overflowed")
                })?;
            let field = arena::Handle::from_parts(field_index, root_fields.start().generation());
            for (terminal_demand_index, terminal_demand) in private_demands
                .iter()
                .enumerate()
                .filter(|(_, demand)| demand.data_symbol == child_symbol)
            {
                if terminal_demand.alignment == 0
                    || !terminal_demand
                        .offset
                        .is_multiple_of(terminal_demand.alignment)
                    || terminal_demand
                        .offset
                        .checked_add(terminal_demand.byte_size)
                        .is_none_or(|end| end > child_identity.physical.size)
                {
                    return Err(Diagnostic::error(
                        "target-closed callback path changed its terminal child geometry",
                    ));
                }
                let composed_offset = field_layout
                    .offset
                    .checked_add(terminal_demand.offset)
                    .ok_or_else(|| {
                        Diagnostic::error("target-closed callback path composed offset overflowed")
                    })?;
                if !composed_offset.is_multiple_of(terminal_demand.alignment)
                    || composed_offset
                        .checked_add(terminal_demand.byte_size)
                        .is_none_or(|end| end > root_identity.physical.size)
                {
                    return Err(Diagnostic::error(
                        "target-closed callback path composed range is outside or misaligned for its root layout",
                    ));
                }
                paths.push(TargetClosedTwoHopPrivateCallbackPath {
                    root_layout_index,
                    root_layout: root_identity.clone(),
                    field_symbol: field_layout.symbol,
                    field,
                    field_layout: field_layout.clone(),
                    field_identity: field_identity.clone().into(),
                    field_slot,
                    field_relative_offset: field_layout.offset,
                    field_extent: field_layout.layout.size,
                    field_alignment: field_layout.layout.alignment,
                    child_layout_index: *child_layout_index,
                    child_layout: (*child_identity).clone(),
                    terminal_demand_index,
                    terminal_demand: terminal_demand.clone(),
                    composed_offset,
                });
            }
        }
    }
    paths.sort_unstable_by_key(|path| {
        (
            path.root_layout.layout,
            path.field_slot,
            path.terminal_demand.slot,
        )
    });
    for (index, path) in paths.iter().enumerate() {
        if paths[..index].iter().any(|prior| {
            prior.root_layout.layout == path.root_layout.layout
                && prior.field_slot == path.field_slot
                && (prior.field != path.field
                    || prior.field_symbol != path.field_symbol
                    || prior.field_layout != path.field_layout
                    || prior.child_layout != path.child_layout)
        }) {
            return Err(Diagnostic::error(
                "target-closed callback path field slot collides across distinct root-to-child edges",
            ));
        }
    }
    if paths.windows(2).any(|pair| {
        pair[0].root_layout.layout == pair[1].root_layout.layout
            && pair[0].field_slot == pair[1].field_slot
            && pair[0].terminal_demand.slot == pair[1].terminal_demand.slot
    }) {
        return Err(Diagnostic::error(
            "target-closed callback path catalog repeats one exact two-hop path",
        ));
    }
    Ok(paths)
}

pub(crate) fn close_private_callback_demands(
    plan: &symbol_resolved_trees_to_typed_trees::typed_trees::typed_trees::PlanLaidLayout,
    fields: &[FieldLayout],
    target: NativeTarget,
    canonical_layout_subject: &str,
    layout: abstract_operations_to_target_operations::calling_conventions::LayoutPlanId,
) -> Result<Vec<TargetClosedPrivateCallbackDemand>, Diagnostic> {
    if plan.private_callback_demands.is_empty() {
        return Ok(Vec::new());
    }
    if plan.validated_layout.size != u64::try_from(plan.size).ok()
        || plan.validated_layout.align != plan.align as u64
    {
        return Err(Diagnostic::error(format!(
            "plan-laid data `{}` changed its validated size/alignment before private callback closure",
            plan.data_name
        )));
    }
    if target.pointer_size == 0
        || target.pointer_alignment == 0
        || !target.pointer_alignment.is_power_of_two()
    {
        return Err(Diagnostic::error(format!(
            "plan-laid data `{}` cannot close private callback slots for invalid target pointer geometry {}/{}",
            plan.data_name, target.pointer_size, target.pointer_alignment
        )));
    }
    if plan.align < target.pointer_alignment || !plan.align.is_multiple_of(target.pointer_alignment)
    {
        return Err(Diagnostic::error(format!(
            "plan-laid data `{}` has alignment {}, which cannot align a {}-byte private callback slot",
            plan.data_name, plan.align, target.pointer_alignment
        )));
    }

    let semantic_ranges = plan_laid_semantic_ranges(plan, fields)?;
    let mut occupied_private = Vec::<(usize, usize, &str)>::new();
    let mut closed = Vec::with_capacity(plan.private_callback_demands.len());
    for demand in &plan.private_callback_demands {
        if demand.layout_subject_identity != canonical_layout_subject {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` private callback slot `{}` changed layout subject from `{canonical_layout_subject}` to `{}`",
                plan.data_name, demand.slot_identity, demand.layout_subject_identity
            )));
        }
        if closed
            .iter()
            .any(|prior: &TargetClosedPrivateCallbackDemand| {
                prior.slot_identity.as_ref() == demand.slot_identity
            })
        {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` repeats private callback slot `{}` during target closure",
                plan.data_name, demand.slot_identity
            )));
        }
        let offset = usize::try_from(demand.offset).map_err(|_| {
            Diagnostic::error(format!(
                "plan-laid data `{}` private callback slot `{}` offset {} cannot be represented for the selected target",
                plan.data_name, demand.slot_identity, demand.offset
            ))
        })?;
        if !offset.is_multiple_of(target.pointer_alignment) {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` private callback slot `{}` offset {} is not aligned to the selected target's {}-byte function-pointer alignment",
                plan.data_name, demand.slot_identity, offset, target.pointer_alignment
            )));
        }
        let end = offset.checked_add(target.pointer_size).ok_or_else(|| {
            Diagnostic::error(format!(
                "plan-laid data `{}` private callback slot `{}` extent overflows",
                plan.data_name, demand.slot_identity
            ))
        })?;
        if end > plan.size {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` private callback slot `{}` range {}..{} lies outside its {}-byte layout",
                plan.data_name, demand.slot_identity, offset, end, plan.size
            )));
        }
        if semantic_ranges
            .iter()
            .any(|&(start, semantic_end)| ranges_overlap(offset, end, start, semantic_end))
        {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` private callback slot `{}` range {}..{} overlaps semantic field storage",
                plan.data_name, demand.slot_identity, offset, end
            )));
        }
        if let Some((_, _, prior)) = occupied_private
            .iter()
            .find(|&&(start, prior_end, _)| ranges_overlap(offset, end, start, prior_end))
        {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` private callback slots `{prior}` and `{}` overlap",
                plan.data_name, demand.slot_identity
            )));
        }

        let slot = callback_layout_slot_id(layout, &demand.slot_identity);
        let requirement = callback_requirement_id(&demand.callback_requirement_identity);
        if let Some(prior) = closed
            .iter()
            .find(|prior: &&TargetClosedPrivateCallbackDemand| {
                prior.slot == slot && prior.slot_identity.as_ref() != demand.slot_identity
            })
        {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` private callback slots `{}` and `{}` collide on one nominal slot identity",
                plan.data_name, prior.slot_identity, demand.slot_identity
            )));
        }
        if let Some(prior) = closed
            .iter()
            .find(|prior: &&TargetClosedPrivateCallbackDemand| {
                prior.requirement == requirement
                    && prior.callback_requirement_identity.as_ref()
                        != demand.callback_requirement_identity
            })
        {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` callback requirements `{}` and `{}` collide on one nominal requirement identity",
                plan.data_name,
                prior.callback_requirement_identity,
                demand.callback_requirement_identity
            )));
        }

        occupied_private.push((offset, end, demand.slot_identity.as_str()));
        closed.push(TargetClosedPrivateCallbackDemand {
            data_symbol: plan.data_symbol,
            slot_application: demand.slot_application.clone(),
            slot_identity: demand.slot_identity.clone().into(),
            layout_subject_identity: demand.layout_subject_identity.clone().into(),
            callback_requirement_identity: demand.callback_requirement_identity.clone().into(),
            layout,
            slot,
            requirement,
            offset,
            byte_size: target.pointer_size,
            alignment: target.pointer_alignment,
        });
    }
    closed.sort_unstable_by(|left, right| left.slot_identity.cmp(&right.slot_identity));
    Ok(closed)
}

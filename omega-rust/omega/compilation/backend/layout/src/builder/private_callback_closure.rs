//! Target-closed private callback slots and complete inline record paths.
use crate::builder::semantic_ranges::{plan_laid_semantic_ranges, ranges_overlap};
use crate::{
    DataLayout, DataShape, FieldLayout, PrivateCallbackInlineField, PrivateCallbackLayout,
    TargetClosedDataLayoutIdentity, TargetClosedPrivateCallbackDemand,
};
use arena::Arena;
use calling_conventions::{callback_layout_slot_id, callback_requirement_id};
use diagnostics::Diagnostic;
use target::NativeTarget;
use typed_trees::TypedTrees;

pub(crate) fn close_private_callback_layouts(
    program: &TypedTrees,
    data: &Arena<DataLayout>,
    fields: &Arena<FieldLayout>,
    selected: &[TargetClosedDataLayoutIdentity],
    demands: &[TargetClosedPrivateCallbackDemand],
    target: NativeTarget,
) -> Result<
    (
        Vec<TargetClosedDataLayoutIdentity>,
        Vec<PrivateCallbackLayout>,
    ),
    Diagnostic,
> {
    if demands.is_empty() {
        return Ok((selected.to_vec(), Vec::new()));
    }
    let mut identities = selected.to_vec();
    for (_, root) in data.iter() {
        if identities
            .iter()
            .any(|identity| identity.data_symbol == root.symbol)
            || !reaches_slot(root.symbol, data, fields, demands, &mut Vec::new())
        {
            continue;
        }
        let canonical = program
            .normalized_hermetic_symbol_identity(root.symbol)
            .or_else(|reason| {
                program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == root.symbol)
                    .and_then(|definition| definition.generic_instance)
                    .map(|reference| {
                        format!(
                            "closed-data-instance::{}",
                            program.package_qualified_type_identity_text(reference)
                        )
                    })
                    .ok_or(reason)
            })
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "callback record has no exact data identity: {reason}"
                ))
            })?;
        let DataShape::Record {
            fields: root_fields,
        } = root.shape
        else {
            continue;
        };
        let root_fields = fields
            .span(root_fields)
            .ok_or_else(|| Diagnostic::error("callback root field span is stale"))?;
        let mut geometry = Vec::new();
        for quantity in [
            root.layout.size,
            root.layout.alignment,
            target.pointer_size,
            target.pointer_alignment,
        ] {
            geometry.extend_from_slice(&(quantity as u64).to_le_bytes());
        }
        for field in root_fields {
            let identity = program
                .normalized_hermetic_symbol_identity(field.symbol)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "callback field has no exact declaration identity: {reason}"
                    ))
                })?;
            geometry.extend_from_slice(&(identity.len() as u64).to_le_bytes());
            geometry.extend_from_slice(identity.as_bytes());
            for quantity in [field.offset, field.layout.size, field.layout.alignment] {
                geometry.extend_from_slice(&(quantity as u64).to_le_bytes());
            }
        }
        let layout = calling_conventions::LayoutPlanId::new(
            calling_conventions::callback_nominal_report_fingerprint(
                b"omega.callback-natural-data-layout.v1",
                &[canonical.as_bytes(), &geometry],
            ),
        )
        .expect("callback report identity is nonzero");
        let identity = TargetClosedDataLayoutIdentity {
            data_symbol: root.symbol,
            data_identity: canonical.into(),
            layout_subject_identity: None,
            layout,
            physical: root.layout,
        };
        if identities
            .iter()
            .any(|prior| prior.layout == identity.layout)
        {
            return Err(Diagnostic::error(
                "callback natural layout identity collides with another root",
            ));
        }
        identities.push(identity);
    }
    let mut layouts = Vec::new();
    for root in &identities {
        if reaches_slot(root.data_symbol, data, fields, demands, &mut Vec::new()) {
            close_path(
                program,
                data,
                fields,
                &identities,
                demands,
                root,
                root,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut layouts,
            )?;
        }
    }
    layouts.sort_by_key(|layout| {
        (
            layout.root_layout().layout,
            layout
                .native_demand(
                    calling_conventions::NativeParameterId::new(1)
                        .expect("nonzero report parameter"),
                )
                .expect("newly built path is valid")
                .destination,
        )
    });
    for pair in layouts.windows(2) {
        let parameter =
            calling_conventions::NativeParameterId::new(1).expect("nonzero report parameter");
        if pair[0].native_demand(parameter) == pair[1].native_demand(parameter) {
            return Err(Diagnostic::error(
                "callback layout catalog repeats one exact inline path",
            ));
        }
    }
    Ok((identities, layouts))
}
fn reaches_slot(
    symbol: symbols::SymbolHandle,
    data: &Arena<DataLayout>,
    fields: &Arena<FieldLayout>,
    demands: &[TargetClosedPrivateCallbackDemand],
    seen: &mut Vec<symbols::SymbolHandle>,
) -> bool {
    if demands.iter().any(|demand| demand.data_symbol == symbol) {
        return true;
    }
    if seen.contains(&symbol) {
        return false;
    }
    seen.push(symbol);
    let found = data
        .iter()
        .find(|(_, layout)| layout.symbol == symbol)
        .is_some_and(|(_, layout)| {
            let DataShape::Record { fields: children } = layout.shape else {
                return false;
            };
            fields.span(children).is_some_and(|children| {
                children
                    .iter()
                    .filter_map(|field| field.type_descriptor.inline_data_symbol())
                    .any(|child| reaches_slot(child, data, fields, demands, seen))
            })
        });
    seen.pop();
    found
}
#[allow(clippy::too_many_arguments)]
fn close_path(
    program: &TypedTrees,
    data: &Arena<DataLayout>,
    fields: &Arena<FieldLayout>,
    identities: &[TargetClosedDataLayoutIdentity],
    demands: &[TargetClosedPrivateCallbackDemand],
    root: &TargetClosedDataLayoutIdentity,
    current: &TargetClosedDataLayoutIdentity,
    path: &mut Vec<PrivateCallbackInlineField>,
    seen: &mut Vec<symbols::SymbolHandle>,
    output: &mut Vec<PrivateCallbackLayout>,
) -> Result<(), Diagnostic> {
    if seen.contains(&current.data_symbol) {
        return Err(Diagnostic::error(
            "callback inline path recursively contains its own carrier",
        ));
    }
    seen.push(current.data_symbol);
    for terminal in demands
        .iter()
        .filter(|demand| demand.data_symbol == current.data_symbol)
    {
        if current.layout_subject_identity.as_deref()
            != Some(terminal.layout_subject_identity.as_ref())
        {
            return Err(Diagnostic::error(
                "callback terminal lost its exact selected layout policy",
            ));
        }
        output.push(
            PrivateCallbackLayout::from_fields(root.clone(), path, terminal.clone())
                .map_err(Diagnostic::error)?,
        );
    }
    let matching = data
        .iter()
        .filter(|(_, layout)| layout.symbol == current.data_symbol)
        .collect::<Vec<_>>();
    let [(_, layout)] = matching.as_slice() else {
        return Err(Diagnostic::error(
            "callback path has no unique typed data layout",
        ));
    };
    if layout.layout != current.physical {
        return Err(Diagnostic::error(
            "callback path changed its complete carrier geometry",
        ));
    }
    if let DataShape::Record { fields: children } = layout.shape {
        for field in fields
            .span(children)
            .ok_or_else(|| Diagnostic::error("callback path field span is stale"))?
        {
            let Some(child_symbol) = field.type_descriptor.inline_data_symbol() else {
                continue;
            };
            if !reaches_slot(child_symbol, data, fields, demands, &mut Vec::new()) {
                continue;
            }
            let matching = identities
                .iter()
                .filter(|identity| identity.data_symbol == child_symbol)
                .collect::<Vec<_>>();
            let [child] = matching.as_slice() else {
                return Err(Diagnostic::error(
                    "callback inline field has no unique closed child layout",
                ));
            };
            if field.layout != child.physical
                || field.layout.alignment == 0
                || !field.offset.is_multiple_of(field.layout.alignment)
                || field
                    .offset
                    .checked_add(field.layout.size)
                    .is_none_or(|end| end > current.physical.size)
                || fields.span(children).is_none_or(|children| {
                    children
                        .iter()
                        .filter(|candidate| candidate.symbol == field.symbol)
                        .count()
                        != 1
                })
                || (field.type_symbol.is_valid() && field.type_symbol != child.data_symbol)
            {
                return Err(Diagnostic::error(
                    "callback path changed its exact inline field geometry",
                ));
            }
            let identity = program
                .normalized_hermetic_symbol_identity(field.symbol)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "callback path field identity cannot close: {reason}"
                    ))
                })?;
            path.push(PrivateCallbackInlineField {
                symbol: field.symbol,
                field_layout: field.clone(),
                identity: identity.into(),
                offset: field.offset,
                extent: field.layout.size,
                alignment: field.layout.alignment,
                child_layout: (*child).clone(),
            });
            close_path(
                program, data, fields, identities, demands, root, child, path, seen, output,
            )?;
            path.pop();
        }
    }
    seen.pop();
    Ok(())
}

pub(crate) fn close_private_callback_demands(
    plan: &typed_trees::typed_trees::PlanLaidLayout,
    geometry: &typed_trees::PlanLaidGeometry,
    fields: &[FieldLayout],
    target: NativeTarget,
    canonical_layout_subject: &str,
    layout: calling_conventions::LayoutPlanId,
) -> Result<Vec<TargetClosedPrivateCallbackDemand>, Diagnostic> {
    if geometry.private_callback_demands.is_empty() {
        return Ok(Vec::new());
    }
    if geometry.validated_layout.size != u64::try_from(geometry.size).ok()
        || geometry.validated_layout.align != geometry.align as u64
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
    if geometry.align < target.pointer_alignment
        || !geometry.align.is_multiple_of(target.pointer_alignment)
    {
        return Err(Diagnostic::error(format!(
            "plan-laid data `{}` has alignment {}, which cannot align a {}-byte private callback slot",
            plan.data_name, geometry.align, target.pointer_alignment
        )));
    }

    let semantic_ranges = plan_laid_semantic_ranges(plan, geometry, fields)?;
    let mut occupied_private = Vec::<(usize, usize, &str)>::new();
    let mut closed = Vec::with_capacity(geometry.private_callback_demands.len());
    for demand in &geometry.private_callback_demands {
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
        if end > geometry.size {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` private callback slot `{}` range {}..{} lies outside its {}-byte layout",
                plan.data_name, demand.slot_identity, offset, end, geometry.size
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

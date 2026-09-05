use super::{
    PackagePolicyCallingPlan, PackagePolicyNativeParameterOrigin, strictly_sorted, validate_nominal,
};
use crate::record::{
    PackagePolicyCallbackDestination as Destination, PackageReviewBoundaryShapeClass,
    PackageReviewTypeParameterKind,
};

pub(super) fn validate(policy: &PackagePolicyCallingPlan) -> Result<(), &'static str> {
    let callbacks = &policy.callbacks;
    // The checked source callback vocabulary has 32 entries. Bound the
    // correspondence scans before examining adversarial recovered catalogs.
    if callbacks.binders.len() > 32
        || callbacks.demands.len() > 32
        || callbacks.materializations.len() > 32
        || callbacks.layouts.len() > 32
    {
        return Err("calling callback catalog exceeds normalized capacity");
    }
    if !strictly_sorted(&callbacks.demands)
        || !strictly_sorted(&callbacks.materializations)
        || !strictly_sorted(&callbacks.layouts)
        || callbacks
            .binders
            .windows(2)
            .any(|pair| pair[0].static_machine_ordinal >= pair[1].static_machine_ordinal)
    {
        return Err("calling callback catalogs are not canonical");
    }
    for binder in &callbacks.binders {
        validate_nominal(&binder.parameter)?;
        validate_nominal(&binder.requirement)?;
        let ordinal = binder.static_parameter_ordinal as usize;
        if !matches!(
            policy
                .static_parameters
                .get(ordinal)
                .map(|parameter| &parameter.kind),
            Some(PackageReviewTypeParameterKind::Machine(_))
        ) || policy.static_parameters[..ordinal]
            .iter()
            .filter(|parameter| {
                matches!(parameter.kind, PackageReviewTypeParameterKind::Machine(_))
            })
            .count()
            != binder.static_machine_ordinal as usize
        {
            return Err("calling callback binder changed its static telescope coordinate");
        }
    }
    for (index, demand) in callbacks.demands.iter().enumerate() {
        validate_nominal(&demand.requirement)?;
        if callbacks.demands[..index]
            .iter()
            .any(|prior| prior.destination == demand.destination)
        {
            return Err("calling callback demand destination is repeated");
        }
        let rows = callbacks
            .materializations
            .iter()
            .filter(|row| row.destination == demand.destination);
        let mut count = 0;
        for row in rows {
            let binder = callbacks
                .binders
                .get(row.binder_index as usize)
                .ok_or("calling callback materialization binder is out of bounds")?;
            if binder.requirement != demand.requirement {
                return Err("calling callback materialization supplies a different requirement");
            }
            match demand.destination {
                Destination::Parameter { native_ordinal } => {
                    let native = policy
                        .native_parameters
                        .get(native_ordinal as usize)
                        .ok_or("calling callback native destination is out of bounds")?;
                    if !matches!(native.origin, PackagePolicyNativeParameterOrigin::PrivateCallback { binder_index, .. } if binder_index == row.binder_index)
                    {
                        return Err("calling direct callback changed its exact native binder");
                    }
                }
                Destination::Field {
                    native_ordinal,
                    layout_index,
                } => {
                    let layout = callbacks
                        .layouts
                        .get(layout_index as usize)
                        .ok_or("calling callback layout index is out of bounds")?;
                    if layout.native_ordinal != native_ordinal {
                        return Err("calling callback field changed its native ordinal");
                    }
                }
            }
            count += 1;
        }
        if count != 1 {
            return Err("calling callback demand has no unique materialization");
        }
    }
    if callbacks.materializations.len() != callbacks.demands.len() {
        return Err("calling callback materializations contain extra destinations");
    }
    for (index, native) in policy.native_parameters.iter().enumerate() {
        if matches!(native.origin, PackagePolicyNativeParameterOrigin::PrivateCallback { .. })
            && callbacks.demands.iter().filter(|demand| matches!(demand.destination, Destination::Parameter { native_ordinal } if native_ordinal as usize == index)).count() != 1
        { return Err("calling private native parameter has no unique callback demand"); }
    }
    for (index, layout) in callbacks.layouts.iter().enumerate() {
        super::validate_application_lifetimes(policy, &layout.terminal_slot)?;
        validate_nominal(&layout.root_layout.policy)?;
        validate_nominal(&layout.terminal_slot.declaration)?;
        validate_nominal(&layout.terminal_slot.trait_identity)?;
        let native = policy
            .native_parameters
            .get(layout.native_ordinal as usize)
            .ok_or("calling layout native parameter is out of bounds")?;
        if !matches!(native.origin, PackagePolicyNativeParameterOrigin::SemanticFormal { formal_ordinal, .. } if formal_ordinal == layout.formal_ordinal)
            || callbacks.demands.iter().filter(|demand| matches!(demand.destination, Destination::Field { layout_index, .. } if layout_index as usize == index)).count() != 1
        { return Err("calling callback layout has no unique semantic destination"); }
        let placement = &policy.physical.parameters[layout.native_ordinal as usize];
        let formal = &policy.semantic_parameters[layout.formal_ordinal as usize];
        let by_reference = matches!(
            policy.shape_graph.shapes[usize::from(formal.shape_root)].class,
            PackageReviewBoundaryShapeClass::Reference
        );
        if (!by_reference
            && (layout.root_layout.byte_size != u64::from(placement.shape.byte_size)
                || layout.root_layout.alignment != u64::from(placement.shape.alignment)))
            || !layout.root_layout.alignment.is_power_of_two()
            || layout.terminal_byte_size != u64::from(policy.target.pointer_size)
            || layout.terminal_alignment != u64::from(policy.target.pointer_alignment)
        {
            return Err("calling callback layout differs from its target-closed shape");
        }
        let (base, terminal_owner) = if let Some(field) = &layout.inline_field {
            validate_nominal(&field.field)?;
            validate_nominal(&field.child_layout.policy)?;
            if field.extent != field.child_layout.byte_size
                || field.alignment != field.child_layout.alignment
                || !contained(
                    field.offset,
                    field.extent,
                    field.alignment,
                    layout.root_layout.byte_size,
                )
            {
                return Err("calling inline callback field has inconsistent geometry");
            }
            (field.offset, &field.child_layout)
        } else {
            (0, &layout.root_layout)
        };
        if !contained(
            layout.terminal_offset,
            layout.terminal_byte_size,
            layout.terminal_alignment,
            terminal_owner.byte_size,
        ) || base.checked_add(layout.terminal_offset) != Some(layout.composed_offset)
            || !contained(
                layout.composed_offset,
                layout.terminal_byte_size,
                layout.terminal_alignment,
                layout.root_layout.byte_size,
            )
        {
            return Err("calling terminal callback field has inconsistent composed geometry");
        }
    }
    validate_disjoint_layouts(&callbacks.layouts)?;
    Ok(())
}

pub(super) fn validate_disjoint_layouts(
    layouts: &[crate::record::PackagePolicyCallbackLayout],
) -> Result<(), &'static str> {
    for (index, layout) in layouts.iter().enumerate() {
        let end = layout
            .composed_offset
            .checked_add(layout.terminal_byte_size)
            .ok_or("calling callback interval overflows")?;
        for prior in &layouts[..index] {
            let prior_end = prior
                .composed_offset
                .checked_add(prior.terminal_byte_size)
                .ok_or("calling callback interval overflows")?;
            if prior.native_ordinal == layout.native_ordinal
                && prior.composed_offset < end
                && layout.composed_offset < prior_end
            {
                return Err("calling callback destinations overlap within one native parameter");
            }
        }
    }
    Ok(())
}

fn contained(offset: u64, extent: u64, alignment: u64, parent: u64) -> bool {
    alignment.is_power_of_two()
        && offset.is_multiple_of(alignment)
        && offset.checked_add(extent).is_some_and(|end| end <= parent)
}

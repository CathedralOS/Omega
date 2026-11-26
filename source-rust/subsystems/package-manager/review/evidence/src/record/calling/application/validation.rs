// Structural custody checks for `PackagePolicyCallingPlan` (inert record
// consistency, not compiler certification).

mod shapes;
mod target;

use super::{PackagePolicyCallingPlan, PackagePolicyNativeParameterOrigin};
use crate::record::public_policy::validation::scope;
use crate::record::validation::{ordered, ordered_by};
use crate::record::{
    PackagePolicyCallbackDestination as Destination, PackagePolicyServiceMethod,
    PackagePolicyTypeParameterKind, PackageReviewBoundaryShapeClass,
};
use application_identity::callback_declarations::BOUNDARY_CALLBACK_CAPACITY;
use application_identity::type_identity::encoding::{CoordinateTag, atom, compound};

impl PackagePolicyCallingPlan {
    pub(crate) fn lifetime_scope(&self) -> Result<(u32, u32), &'static str> {
        let application_scope = self
            .boundary_lifetime_parameter_count
            .checked_add(self.application_lifetime_parameter_count)
            .ok_or("calling lifetime telescope overflows")?;
        let lifetime_count = application_scope
            .checked_add(self.requirement_lifetime_parameter_count)
            .ok_or("calling lifetime telescope overflows")?;
        Ok((application_scope, lifetime_count))
    }

    pub(crate) fn validate_canonical_structure(&self) -> Result<(), &'static str> {
        if self.abi.requirement_identity != self.requirement.path {
            return Err("calling ABI key changes its source-qualified requirement");
        }
        self.validate_structure()
    }

    pub(crate) fn validate_for_service(
        &self,
        method: &PackagePolicyServiceMethod,
    ) -> Result<(), &'static str> {
        // Earlier policy records have only the declaration path in the key.
        if self.abi.requirement_identity == self.requirement.path {
            return self.validate_canonical_structure();
        }
        self.validate_structure()?;
        // Closed uses keep their values in the ABI tuple and their original
        // binder kinds in this declaration telescope. Neither replaces the other.
        if self.static_parameters.len() != self.abi.operands.len()
            || self
                .static_parameters
                .iter()
                .zip(&self.abi.operands)
                .any(|(parameter, operand)| {
                    use crate::record::PackagePolicyTypeParameterKind as Parameter;
                    use semantic_vocabulary::CallingOperandKind as Operand;
                    let kind = match parameter.kind {
                        Parameter::Type => Operand::Type,
                        Parameter::Implementation => Operand::Implementation,
                        Parameter::Const(_) => Operand::Const,
                        Parameter::Value(_) => Operand::Value,
                        Parameter::Machine(_) => Operand::Machine,
                        Parameter::Proposition(_) => Operand::Proposition,
                    };
                    kind != operand.kind
                })
            || !self.boundary_arguments.is_empty()
            || !self.requirement_arguments.is_empty()
            || !self.requirement_lifetime_arguments.is_empty()
            || self.boundary_lifetime_parameter_count != 0
            || self.application_lifetime_parameter_count != 0
            || self.requirement_lifetime_parameter_count
                != method.signature.requirement_lifetime_parameter_count
            || !self
                .boundary_trait
                .matches_unapplied_trait_declaration(&self.abi.declaring_application)
        {
            return Err("calling ABI key changes its closed declaring application");
        }
        // Dispatch uses the ordinary type identities in the method census.
        // The exact-owner semantic signature is checked separately.
        let parameters = method
            .signature
            .parameters
            .iter()
            .zip(&method.parameter_type_identities)
            .map(|(parameter, type_identity)| {
                compound(
                    CoordinateTag::Parameter,
                    [
                        atom(
                            CoordinateTag::Mutable,
                            if parameter.is_mutable { "yes" } else { "no" },
                        ),
                        atom(
                            CoordinateTag::Const,
                            if parameter.is_const { "yes" } else { "no" },
                        ),
                        type_identity.clone(),
                    ],
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        if !self
            .requirement
            .matches_named_callable(&self.abi.requirement_identity, &parameters, "")
        {
            return Err("calling ABI key changes its source-qualified requirement");
        }
        Ok(())
    }

    fn validate_structure(&self) -> Result<(), &'static str> {
        self.abi.validate()?;
        target::validate(self.target)?;
        let (application_scope, lifetime_count) = self.lifetime_scope()?;
        let scope = scope(&self.static_parameters, lifetime_count as usize);
        crate::record::public_policy::validation::signatures::parameters(&scope, 0)?;
        self.boundary_trait.validate_resolved()?;
        self.requirement.validate_resolved()?;
        self.requirement_trait.validate_resolved()?;
        if self.native_parameters.len() > 256 || self.semantic_parameters.len() > 256 {
            return Err("calling parameter catalog exceeds normalized capacity");
        }
        if self.boundary_lifetime_arguments.len() != self.boundary_lifetime_parameter_count as usize
            || self
                .boundary_lifetime_arguments
                .iter()
                .any(|ordinal| *ordinal >= application_scope)
            || self.requirement.owner != self.requirement_trait.owner
            || self
                .requirement_lifetime_arguments
                .iter()
                .any(|ordinal| *ordinal >= application_scope)
        {
            return Err("calling requirement owner or lifetime application is inconsistent");
        }
        if self.native_parameters.len() != self.physical.parameters.len()
            || self.semantic_parameters.len() != self.shape_graph.parameters.len()
            || self.semantic_result.is_some() != self.shape_graph.result.is_some()
            || self.semantic_result.is_some() != self.physical.result.is_some()
        {
            return Err("calling semantic, native, and physical telescopes disagree");
        }
        shapes::validate(self)?;
        for (ordinal, parameter) in self.native_parameters.iter().enumerate() {
            if parameter.name.is_empty()
                || self.native_parameters[..ordinal]
                    .iter()
                    .any(|prior| prior.name == parameter.name)
            {
                return Err("calling native names are empty or repeated");
            }
            let placement = &self.physical.parameters[ordinal];
            let (byte_size, alignment) = match parameter.origin {
                PackagePolicyNativeParameterOrigin::SemanticFormal {
                    formal_ordinal,
                    shape_root,
                } => {
                    let formal = self
                        .semantic_parameters
                        .get(formal_ordinal as usize)
                        .ok_or("calling native formal is out of bounds")?;
                    if formal.shape_root != shape_root || formal.name != parameter.name {
                        return Err("calling native formal changed its semantic origin");
                    }
                    let shape = self
                        .shape_graph
                        .shapes
                        .get(usize::from(shape_root))
                        .ok_or("calling native shape is out of bounds")?;
                    (shape.byte_size, shape.alignment)
                }
                PackagePolicyNativeParameterOrigin::PrivateCallback {
                    binder_index,
                    byte_size,
                    alignment,
                } => {
                    if self.callbacks.binders.get(binder_index as usize).is_none()
                        || byte_size != self.target.pointer_size
                        || alignment != self.target.pointer_alignment
                    {
                        return Err(
                            "calling private callback has no matching binder or pointer shape",
                        );
                    }
                    (byte_size, alignment)
                }
            };
            if placement.shape.byte_size != u32::from(byte_size)
                || placement.shape.alignment != alignment
            {
                return Err("calling native origin differs from its physical placement");
            }
        }
        if self
            .dispatch_only_formal
            .is_some_and(|ordinal| ordinal as usize >= self.semantic_parameters.len())
        {
            return Err("calling table dispatch names an absent semantic formal");
        }
        for (ordinal, formal) in self.semantic_parameters.iter().enumerate() {
            if self.shape_graph.parameters[ordinal] != formal.shape_root
                || self.native_parameters.iter().filter(|native| matches!(native.origin,
                    PackagePolicyNativeParameterOrigin::SemanticFormal { formal_ordinal, .. } if formal_ordinal as usize == ordinal)).count() != usize::from(self.dispatch_only_formal != Some(ordinal as u32))
            {
                return Err("calling semantic formal has no unique native placement");
            }
        }
        if let (Some(root), Some(placement)) = (self.shape_graph.result, &self.physical.result) {
            let shape = &self.shape_graph.shapes[usize::from(root)];
            if u32::from(shape.byte_size) != placement.shape.byte_size
                || shape.alignment != placement.shape.alignment
            {
                return Err("calling result shape differs from its physical placement");
            }
        }
        validate(self)?;
        shapes::validate_opaque(self)
    }
}

fn validate_application_lifetimes(
    policy: &PackagePolicyCallingPlan,
    application: &crate::record::PackagePolicyClosedConformanceApplication,
) -> Result<(), &'static str> {
    let (_, count) = policy.lifetime_scope()?;
    if application
        .lifetime_arguments
        .iter()
        .chain(&application.trait_lifetime_arguments)
        .any(|ordinal| *ordinal >= count)
    {
        return Err("calling conformance lifetime is outside its containing telescope");
    }
    Ok(())
}

// Validates callback binders, demands, materializations, and disjoint layout
// geometry for `PackagePolicyCallingPlan`.
//
// Recovered catalogs are bounded by `BOUNDARY_CALLBACK_CAPACITY` before
// checking callback correspondences.

fn validate(policy: &PackagePolicyCallingPlan) -> Result<(), &'static str> {
    let callbacks = &policy.callbacks;
    if callbacks.binders.len() > BOUNDARY_CALLBACK_CAPACITY
        || callbacks.demands.len() > BOUNDARY_CALLBACK_CAPACITY
        || callbacks.materializations.len() > BOUNDARY_CALLBACK_CAPACITY
        || callbacks.layouts.len() > BOUNDARY_CALLBACK_CAPACITY
    {
        return Err("calling callback catalog exceeds normalized capacity");
    }
    ordered(&callbacks.demands)?;
    ordered(&callbacks.materializations)?;
    ordered(&callbacks.layouts)?;
    ordered_by(&callbacks.binders, |a, b| {
        a.static_machine_ordinal.cmp(&b.static_machine_ordinal)
    })?;
    for binder in &callbacks.binders {
        binder.parameter.validate_resolved()?;
        binder.requirement.validate_resolved()?;
        let ordinal = binder.static_parameter_ordinal as usize;
        if !matches!(
            policy
                .static_parameters
                .get(ordinal)
                .map(|parameter| &parameter.kind),
            Some(PackagePolicyTypeParameterKind::Machine(_))
        ) || policy.static_parameters[..ordinal]
            .iter()
            .filter(|parameter| {
                matches!(parameter.kind, PackagePolicyTypeParameterKind::Machine(_))
            })
            .count()
            != binder.static_machine_ordinal as usize
        {
            return Err("calling callback binder changed its static telescope coordinate");
        }
    }
    for (index, demand) in callbacks.demands.iter().enumerate() {
        demand.requirement.validate_resolved()?;
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
        self::validate_application_lifetimes(policy, &layout.terminal_slot)?;
        if let Some(policy) = &layout.root_layout.policy {
            policy.validate_resolved()?;
        }
        layout.terminal_slot.declaration.validate_resolved()?;
        layout.terminal_slot.trait_identity.validate_resolved()?;
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
        let mut base = 0u64;
        let mut terminal_owner = &layout.root_layout;
        for field in &layout.inline_fields {
            field.field.validate_resolved()?;
            if let Some(policy) = &field.child_layout.policy {
                policy.validate_resolved()?;
            }
            if field.extent != field.child_layout.byte_size
                || field.alignment != field.child_layout.alignment
                || !contained(
                    field.offset,
                    field.extent,
                    field.alignment,
                    terminal_owner.byte_size,
                )
            {
                return Err("calling inline callback field has inconsistent geometry");
            }
            base = base
                .checked_add(field.offset)
                .ok_or("calling inline callback path overflows")?;
            terminal_owner = &field.child_layout;
        }
        if terminal_owner.policy.is_none() {
            return Err("calling terminal callback slot has no selected layout policy");
        }
        layout_plans::validate_private_callback_placements(
            &layout.terminal_placements,
            layout.terminal_byte_size,
            layout.terminal_alignment,
            terminal_owner.byte_size,
            terminal_owner.alignment,
        )
        .map_err(|_| "calling terminal callback slot has invalid geometry")?;
        let composed =
            layout_plans::compose_private_callback_placements(&layout.terminal_placements, base)
                .map_err(|_| "calling terminal callback path overflows")?;
        if composed != layout.composed_placements {
            return Err("calling terminal callback field has inconsistent composed geometry");
        }
        callback_ranges(layout)?;
    }
    validate_disjoint_layouts(&callbacks.layouts)?;
    Ok(())
}

fn validate_disjoint_layouts(
    layouts: &[crate::record::PackagePolicyCallbackLayout],
) -> Result<(), &'static str> {
    for (index, layout) in layouts.iter().enumerate() {
        let ranges = callback_ranges(layout)?;
        for prior in &layouts[..index] {
            if prior.native_ordinal != layout.native_ordinal {
                continue;
            }
            let prior_ranges = callback_ranges(prior)?;
            if ranges.iter().any(|(start, end)| {
                prior_ranges
                    .iter()
                    .any(|(prior_start, prior_end)| prior_start < end && start < prior_end)
            }) {
                return Err("calling callback destinations overlap within one native parameter");
            }
        }
    }
    Ok(())
}

pub(in crate::record::calling::application::validation) fn contained(
    offset: u64,
    extent: u64,
    alignment: u64,
    parent: u64,
) -> bool {
    alignment.is_power_of_two()
        && offset.is_multiple_of(alignment)
        && offset.checked_add(extent).is_some_and(|end| end <= parent)
}

pub(in crate::record::calling::application::validation) fn callback_ranges(
    layout: &crate::record::PackagePolicyCallbackLayout,
) -> Result<Vec<(u64, u64)>, &'static str> {
    layout_plans::validate_private_callback_placements(
        &layout.composed_placements,
        layout.terminal_byte_size,
        layout.terminal_alignment,
        layout.root_layout.byte_size,
        layout.root_layout.alignment,
    )
    .map_err(|_| "calling callback destination has invalid composed geometry")
}

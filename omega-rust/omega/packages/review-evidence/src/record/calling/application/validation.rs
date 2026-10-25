//! Structural custody checks for inert records, not compiler certification.

mod callbacks;
mod shapes;
mod target;

use super::{PackagePolicyCallingPlan, PackagePolicyNativeParameterOrigin};

impl PackagePolicyCallingPlan {
    pub(crate) fn validate_canonical_structure(&self) -> Result<(), &'static str> {
        target::validate(self.target)?;
        let application_scope = self
            .boundary_lifetime_parameter_count
            .checked_add(self.application_lifetime_parameter_count)
            .ok_or("calling lifetime telescope overflows")?;
        let lifetime_count = application_scope
            .checked_add(self.requirement_lifetime_parameter_count)
            .ok_or("calling lifetime telescope overflows")?;
        let scope = crate::record::public_policy::validation::scope(
            &self.static_parameters,
            lifetime_count as usize,
        );
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
        for (ordinal, formal) in self.semantic_parameters.iter().enumerate() {
            if self.shape_graph.parameters[ordinal] != formal.shape_root
                || self.native_parameters.iter().filter(|native| matches!(native.origin,
                    PackagePolicyNativeParameterOrigin::SemanticFormal { formal_ordinal, .. } if formal_ordinal as usize == ordinal)).count() != 1
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
        callbacks::validate(self)?;
        shapes::validate_opaque(self)
    }
}

fn strictly_sorted<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn validate_application_lifetimes(
    policy: &PackagePolicyCallingPlan,
    application: &crate::record::PackagePolicyClosedConformanceApplication,
) -> Result<(), &'static str> {
    let count = policy
        .boundary_lifetime_parameter_count
        .checked_add(policy.application_lifetime_parameter_count)
        .and_then(|count| count.checked_add(policy.requirement_lifetime_parameter_count))
        .ok_or("calling lifetime telescope overflows")?;
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

//! Region parameters have declaration identity, not a runtime carrier.
use crate::{LoweringError, MachineRegion};
use omega_resolved::DeclarationId;
use omega_syntax::NodeKind;
use omega_typed::{GenericParameters, TypedSignature, ValueType};

/// The region belongs to this declaration, not merely a binder at the same
/// position. Only direct place references have a slot mapping here.
pub(crate) fn direct<'a>(ty: &'a ValueType, machine: &DeclarationId) -> Option<(usize, &'a ValueType)> {
    let ValueType::AtLifetime(reference, region) = ty else { return None };
    let ValueType::Parameter { owner, index } = region.as_ref() else { return None };
    (owner.as_ref() == machine && crate::lowering::types::place_reference(reference).is_some()).then_some((*index, reference))
}

pub(crate) fn slot_carrier<'a>(ty: &'a ValueType, machine: &DeclarationId) -> &'a ValueType {
    direct(ty, machine).map_or(ty, |(_, reference)| reference)
}

/// Whether one executable body serves every application of a lifetime-only
/// machine. That holds only when `signature` can publish each binder as the
/// region of direct place-reference slots. A binder inside another type, such
/// as `schema: TypeSchema<'t>`, is an aggregate lifetime frontier with no Psi
/// region. That declaration stays an open template, and the template planner
/// judges it with its generic signature intact. It must not become a checked
/// body whose carriers name an unapplied binder.
pub(crate) fn single_body(sig: &TypedSignature, telescope: Option<&GenericParameters>) -> bool {
    telescope.is_some_and(|t| !t.parameters.is_empty() && t.parameters.iter().all(|p| p.kind == NodeKind::LifetimeParam))
        && signature(sig, telescope).is_ok()
}

pub(crate) fn signature(sig: &TypedSignature, telescope: Option<&GenericParameters>) -> Result<Vec<MachineRegion>, LoweringError> {
    let mut regions = Vec::new();
    if let Some(params) = telescope.filter(|t| t.parameters.iter().any(|p| p.kind == NodeKind::LifetimeParam)) {
        if params.witness_count != 0 || !params.equations.is_empty()
            || !params.parameters.iter().all(|p| p.kind == NodeKind::LifetimeParam && p.carrier.is_none() && p.multiplicity.is_none())
        {
            return Err(LoweringError::new(&sig.site, "machine needs a region-only telescope"));
        }
        regions.resize_with(params.parameters.len(), MachineRegion::default);
    }
    let count = regions.len();
    let binder = |ty: &ValueType| -> Result<Option<usize>, LoweringError> {
        if let Some((index, _)) = direct(ty, &sig.machine) {
            if index < count {
                return Ok(Some(index));
            }
            return Err(LoweringError::new(&sig.site, "reference requires its machine region binder"));
        }
        if count != 0 && ty.components().any(|part| matches!(part, ValueType::Parameter { owner, .. } if owner.as_ref() == &sig.machine)) {
            return Err(LoweringError::new(&sig.site, "machine region needs a direct reference slot"));
        }
        Ok(None)
    };
    if let Some(index) = binder(&sig.result)? {
        regions[index].reference_result = true;
    }
    for (position, (_, ty)) in sig.parameters.iter().enumerate() {
        if let Some(index) = binder(ty)? {
            regions[index].reference_parameters.push(position);
        }
    }
    Ok(regions)
}

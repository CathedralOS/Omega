//! Structural operand origins from the current control graph inventory.
//!
//! A borrowed arrival carries an address or descriptor, never an owned home.
//! Call families share this origin join, then apply their own projection and
//! ABI presentation. The backing extent is independent of an arrival's pointer
//! placement: a four-byte record passed by address still has four backing bytes.

use crate::LoweringError;
use crate::lowering::structural_layout::structural_shape;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::AbstractFunction;
use calling_conventions::ValueShape;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetStructuralArgumentSource, TargetStructuralHomeOrigin, TargetStructuralHomeRequirement,
    TargetStructuralParameter,
};
use terminal_psi::StructuralParameterDeclaration;

pub(in crate::lowering) struct StructuralArgumentRoot {
    pub declaration: StructuralParameterDeclaration,
    pub backing_shape: ValueShape,
    pub source: TargetStructuralArgumentSource,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn resolve<'parameter>(
    place: PlaceId,
    function: &AbstractFunction,
    parameters: impl IntoIterator<Item = &'parameter TargetStructuralParameter>,
    homes: &BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
    address_joins: &BTreeSet<PlaceId>,
    block_views: &BTreeSet<PlaceId>,
    types: &StructuralTypeLookup<'_>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<StructuralArgumentRoot, LoweringError> {
    let unknown = || LoweringError::UnknownStructuralArgumentPlace {
        machine: function.machine,
        place,
    };
    if let Some(parameter) = parameters
        .into_iter()
        .find(|parameter| parameter.place == place)
    {
        let declaration = function.structural_parameter(place).ok_or_else(unknown)?;
        return Ok(StructuralArgumentRoot {
            declaration: declaration.clone(),
            backing_shape: structural_shape(
                declaration.structural_type,
                types,
                shape_cache,
                active,
            )?,
            source: parameter.placement.clone().into(),
        });
    }
    if let Some(home) = homes.get(&place) {
        let declaration = match &home.origin {
            TargetStructuralHomeOrigin::OperationResult { result, .. } => {
                StructuralParameterDeclaration {
                    place: result.place,
                    position: 0,
                    structural_type: result.structural_type,
                    access: terminal_psi::StructuralAccess::Owned,
                    multiplicity: result.multiplicity,
                    qualifications: result.qualifications.clone(),
                    projected_qualifications: result.projected_qualifications.clone(),
                }
            }
            TargetStructuralHomeOrigin::BlockParameter { declaration, .. } => declaration.clone(),
        };
        // Existing storage supplies the argument source, including a view's
        // descriptor payload; the home origin retains its exact producer.
        return Ok(StructuralArgumentRoot {
            declaration,
            backing_shape: home.layout.shape(),
            source: home.argument_source(),
        });
    }
    if address_joins.contains(&place) || block_views.contains(&place) {
        let (block, declaration) = function
            .block_entries
            .iter()
            .find_map(|entry| {
                entry
                    .structural_parameter(place)
                    .map(|parameter| (entry.block, parameter))
            })
            .ok_or_else(unknown)?;
        if address_joins.contains(&place)
            && !abstract_operations::control_flow::address_joins::is_address_join(
                declaration,
                |identity| types.get(&identity).copied(),
            )
        {
            return Err(unknown());
        }
        return Ok(StructuralArgumentRoot {
            declaration: declaration.clone(),
            backing_shape: structural_shape(
                declaration.structural_type,
                types,
                shape_cache,
                active,
            )?,
            source: TargetStructuralArgumentSource::BlockParameter { block, place },
        });
    }
    Err(unknown())
}

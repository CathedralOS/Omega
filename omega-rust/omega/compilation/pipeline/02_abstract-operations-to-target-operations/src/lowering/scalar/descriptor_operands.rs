//! Full descriptor operands share declaration and selector projection.
//! This owner supplies geometry, never liveness or permission certificates.
use super::element_views::ElementViewBackings;
use crate::LoweringError;
use crate::lowering::structural_layout::runtime_descriptor_projection;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::AbstractFunction;
use std::collections::BTreeMap;
use terminal_psi::{StructuralAccess, StructuralArgument};

/// A projected descriptor borrows its exact current carrier. Runtime selectors
/// stay in operand custody on the side of each stored-reference crossing.
pub(in crate::lowering) fn descriptor_projection(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    backings: ElementViewBackings<'_>,
    source: &StructuralArgument,
) -> Result<
    (
        semantic_vocabulary::StructuralTypeId,
        Vec<target_operations::TargetStructuralRuntimeIndex>,
    ),
    LoweringError,
> {
    let invalid = || LoweringError::UnsupportedOperationInScalarFunction(function.machine);
    if !matches!(
        source.access,
        StructuralAccess::Owned | StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) {
        return Err(invalid());
    }
    let (root, access) = function
        .arrival_parameter(source.place)
        .map(|parameter| (parameter.structural_type, parameter.access))
        .or_else(|| {
            backings
                .homes
                .get(&source.place)
                .map(|home| (home.structural_type(), home.access()))
        })
        .or_else(|| {
            function.operations.iter().find_map(|operation| {
                let abstract_operations::AbstractOperation::EstablishByteSequenceLiteral {
                    place,
                    structural_type,
                    ..
                } = operation
                else {
                    return None;
                };
                (place.id == source.place
                    && structural_type.shape.is_borrowed_byte_view()
                    && structural_types.get(&structural_type.id).copied() == Some(structural_type)
                    && matches!(
                        place.kind,
                        semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                            structural_type: identity,
                            ..
                        } if identity == structural_type.id
                    ))
                .then_some((structural_type.id, StructuralAccess::SharedBorrow))
            })
        })
        .ok_or_else(invalid)?;
    if !(access == source.access
        || access == StructuralAccess::Owned
        || access == StructuralAccess::MutableBorrow
            && source.access == StructuralAccess::SharedBorrow)
    {
        return Err(invalid());
    }
    let regions = crate::lowering::structural_layout::reference_projection_regions(
        root,
        &source.path,
        structural_types,
    )
    .ok_or_else(invalid)?;
    let (selected, _, _, leaf_indices) =
        runtime_descriptor_projection(regions.root, regions.path, structural_types)?;
    let indices = regions
        .pointer_slots
        .into_iter()
        .flat_map(|slot| slot.indices)
        .chain(leaf_indices)
        .collect();
    let indices = crate::lowering::control_flow::scalar_sources::runtime_indices(
        indices,
        function,
        &backings.selectors,
    )?;
    Ok((selected, indices))
}

/// Count snapshots remain attached to their exact defining observation. A
/// transported SSA home can retain that definition; its place alone cannot
/// identify a projected descriptor or donate a sibling's extent.
pub(in crate::lowering) fn observes_count(
    function: &AbstractFunction,
    values: &BTreeMap<semantic_vocabulary::ValueId, super::KnownScalar>,
    length: semantic_vocabulary::ValueId,
    source: &StructuralArgument,
    bytes: bool,
) -> bool {
    let Some(super::KnownScalar::Integer { scalar_type, .. }) = values.get(&length) else {
        return false;
    };
    if scalar_type.sign() != semantic_vocabulary::IntegerSign::Unsigned || scalar_type.bits() != 64
    {
        return false;
    }
    function.operations.iter().any(|operation| match operation {
        abstract_operations::AbstractOperation::ByteSequenceLength {
            result,
            source: observed,
            ..
        } if bytes => {
            result.value == length
                && terminal_semantics::same_descriptor_coordinate(observed, source)
        }
        abstract_operations::AbstractOperation::ElementViewLength {
            result,
            source: observed,
            ..
        } if !bytes => {
            result.value == length
                && terminal_semantics::same_descriptor_coordinate(observed, source)
        }
        _ => false,
    })
}

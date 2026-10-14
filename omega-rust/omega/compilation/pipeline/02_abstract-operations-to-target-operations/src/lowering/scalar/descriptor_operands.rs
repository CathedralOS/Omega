//! Full descriptor operands share declaration and selector projection.
//! This owner supplies geometry, never liveness or permission certificates.
use super::element_views::ElementViewBackings;
use crate::LoweringError;
use crate::lowering::structural_layout::runtime_descriptor_projection;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::AbstractFunction;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeShape,
};

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
    let invalid = || refusal(function, &(source, backings.homes.get(&source.place)));
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
            // Completed descriptors can retain their words in SSA instead of
            // allocating a home. This joins the live producer's declared
            // storage carrier; it grants no access to the borrowed backing.
            backings
                .view_result(function, source.place)
                .map(|result| (result.structural_type, StructuralAccess::Owned))
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

/// Whether `collection` storage can back the parameter's declared view
/// descriptor when an edge binds the storage itself as the argument. A
/// borrowed view parameter can also arrive over a loaned local's storage: the
/// destination receives a descriptor formed over that exact backing, so the
/// argument's resolved type is the referent's, not the descriptor's. Element
/// identity, extent and window shape stay under the same establishment rules
/// as an ordinary view formation; this grants no access, liveness or custody.
pub(in crate::lowering) fn storage_backs_view_parameter(
    collection: StructuralTypeId,
    path: &[StructuralPathSegment],
    parameter: &StructuralParameterDeclaration,
    structural_types: &StructuralTypeLookup<'_>,
) -> bool {
    let Some(declaration) = structural_types.get(&parameter.structural_type).copied() else {
        return false;
    };
    let view_element = match &declaration.shape {
        StructuralTypeShape::ElementView { element } => Some(*element),
        shape if shape.is_borrowed_byte_carrier() => None,
        _ => return false,
    };
    let element = view_element.or_else(|| {
        structural_types.values().find_map(|declaration| {
            (declaration.scalar_restriction.is_none()
                && matches!(declaration.shape,
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                    if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 8)))
            .then_some(declaration.id)
            .filter(|element| {
                terminal_semantics::collection_backing(
                    structural_types.values().copied(),
                    collection,
                    *element,
                )
                .is_some()
            })
        })
    });
    let Some(element) = element else {
        return false;
    };
    let Some(backing) = terminal_semantics::collection_backing(
        structural_types.values().copied(),
        collection,
        element,
    ) else {
        return false;
    };
    terminal_semantics::collection_view_descriptor(
        &declaration.shape,
        Some(backing.extent),
        terminal_psi::ElementViewRegion::WholeCollection,
        path,
        structural_types.get(&element).copied(),
    )
    .is_some()
}

/// Optional developer provenance for a descriptor refusal; semantic admission
/// and the published error remain unchanged.
#[track_caller]
pub(in crate::lowering) fn refusal(
    function: &AbstractFunction,
    context: &impl std::fmt::Debug,
) -> LoweringError {
    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
        let site = std::panic::Location::caller();
        eprintln!(
            "descriptor lowering refusal {:?} at {}:{}: {context:?}",
            function.machine,
            site.file(),
            site.line()
        );
    }
    LoweringError::UnsupportedOperationInScalarFunction(function.machine)
}

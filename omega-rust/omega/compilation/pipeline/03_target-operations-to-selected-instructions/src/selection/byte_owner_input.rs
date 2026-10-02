//! Rejoin bounded-owner storage presented as a borrowed byte view.
//!
//! Whole and inline-field owners share one physical layout: a live length word
//! followed by inline backing. This module authenticates the exact entry,
//! block, or local-home origin before selection synthesizes a descriptor; it
//! does not infer authority from carrier shape or from a producer family.
//! Whole-owner presentation is shared-only; established mutable storage keeps
//! its existing exclusive field projection.

use legalized_operations::LegalizedScalarFunction;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

/// Geometry for a borrowed view over authenticated bounded-owner storage.
pub(super) fn view_argument(
    function: &LegalizedScalarFunction,
    operation: semantic_vocabulary::OperationId,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
) -> Option<(u32, u64)> {
    let signature = function.structural.as_ref()?;
    if !matches!(
        semantic.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || target.place != semantic.place
        || target.access != semantic.access
        || target.path != semantic.path
        || target.fixed_array_length.is_some()
        || target.element_stride.is_some()
        || !target.indices.is_empty()
        || target.shape != calling_conventions::ValueShape::borrowed_reference(16, 8)
    {
        return None;
    }
    let source = source_declaration(function, operation, semantic, target)?;
    if target.root_structural_type != source.structural_type {
        return None;
    }
    let geometry = crate::structural_inputs::structural_reference_input::bounded_byte_view(
        &source,
        semantic,
        target.structural_type,
        &signature.structural_types,
    )?;
    (target.source_byte_offset == geometry.0).then_some(geometry)
}

/// Rejoin the exact owner storage named by the retained target source. This is
/// identity and liveness-independent layout replay: legalization has already
/// checked current ownership and dominance, while selection still rejects a
/// substituted entry placement, block coordinate, or local producer.
fn source_declaration(
    function: &LegalizedScalarFunction,
    operation: semantic_vocabulary::OperationId,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
) -> Option<terminal_psi::StructuralParameterDeclaration> {
    let signature = function.structural.as_ref()?;
    match &target.source {
        target_operations::TargetStructuralArgumentSource::Placement(placement) => {
            let mut matching = signature
                .parameters
                .iter()
                .filter(|parameter| parameter.semantic.place == semantic.place);
            let parameter = matching.next()?;
            if matching.next().is_some()
                || parameter.target.placement != *placement
                || parameter.semantic.multiplicity != StructuralMultiplicity::Unrestricted
                || !matches!(
                    parameter.semantic.access,
                    StructuralAccess::Owned
                        | StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                )
            {
                return None;
            }
            Some(parameter.semantic.clone())
        }
        target_operations::TargetStructuralArgumentSource::BlockParameter { block, place } => {
            if *place != semantic.place || *block == function.entry_block {
                return None;
            }
            let mut matching = function.blocks.iter().flat_map(|candidate| {
                candidate
                    .structural_parameters
                    .iter()
                    .filter(|parameter| parameter.place == semantic.place)
                    .map(move |parameter| (candidate.id, parameter))
            });
            let (owner, parameter) = matching.next()?;
            if matching.next().is_some()
                || owner != *block
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !matches!(
                    parameter.access,
                    StructuralAccess::Owned
                        | StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                )
                || !signature.structural_places.iter().any(|declaration| {
                    declaration.id == *place
                        && declaration.kind
                            == (StructuralPlaceKind::BlockParameter {
                                block: *block,
                                position: parameter.position,
                            })
                })
                || (parameter.access == StructuralAccess::Owned
                    && super::aggregate_result_input::block_home(function, *place)
                        != Some((*block, parameter)))
                || (parameter.access != StructuralAccess::Owned
                    && super::address_join_input::join(function, *place)
                        != Some((*block, parameter)))
            {
                return None;
            }
            Some(parameter.clone())
        }
        target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
            let (producer, result) = super::aggregate_result_input::home(function, semantic.place)?;
            if producer != *psi_operation
                || producer == operation
                || result.multiplicity != StructuralMultiplicity::Unrestricted
            {
                return None;
            }
            Some(terminal_psi::StructuralParameterDeclaration {
                place: result.place,
                position: 0,
                is_self: false,
                structural_type: result.structural_type,
                multiplicity: result.multiplicity,
                access: StructuralAccess::Owned,
                qualifications: result.qualifications.clone(),
                projected_qualifications: result.projected_qualifications.clone(),
            })
        }
        target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal { .. }
        | target_operations::TargetStructuralArgumentSource::EstablishedByteView { .. }
        | target_operations::TargetStructuralArgumentSource::EstablishedElementView { .. } => None,
    }
}

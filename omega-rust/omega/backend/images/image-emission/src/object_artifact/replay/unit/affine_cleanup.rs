//! Exact Unit-affine cleanup evidence replay.
//!
//! This module validates root, residual, and nominal cleanup actions against
//! retained places, structural paths, provenance, code attribution, and cleanup targets.
//! It does not choose cleanup actions, infer layouts, or emit instructions.

mod cleanup_roots;
mod discard_actions;
mod nominal_actions;

use machine_code::{
    BoundarySettlementRecord, InternalUnitCallRecord, MachineCodeFunction, SemanticCodeAttribution,
    SemanticCodeSite, UnitAffineCleanupRecord, UnitParameterHomeRecord,
};
use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
use target_operations::TerminalPsiProvenance;

use crate::ObjectError;

pub(crate) fn exact_construction_prefix(cleanup: &UnitAffineCleanupRecord) -> bool {
    let construction_locals = cleanup
        .locals
        .iter()
        .filter_map(|(_, place, element_type)| match place.kind {
            semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
                construction: Some(construction),
            } => Some((
                declaration_ordinal,
                structural_type,
                construction,
                element_type.id,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    if construction_locals.is_empty() {
        return true;
    }
    let expected_root_length = match construction_locals.len() {
        2 => 3,
        3 => 4,
        4 => 5,
        5 => 6,
        6 => 7,
        7 => 8,
        8 => 9,
        9 => 10,
        10 => 11,
        11 => 12,
        12 => 13,
        13 => 14,
        14 => 15,
        15 => 16,
        16 => 17,
        17 => 18,
        18 => 19,
        19 => 20,
        20 => 21,
        21 => 22,
        22 => 23,
        23 => 24,
        24 => 25,
        25 => 26,
        _ => return false,
    };
    construction_locals.len() == cleanup.locals.len()
        && construction_locals.iter().enumerate().all(
            |(index, (ordinal, structural_type, construction, declared_type))| {
                usize::try_from(*ordinal) == Ok(index)
                    && usize::try_from(construction.index) == Ok(index)
                    && structural_type == declared_type
            },
        )
        && construction_locals
            .first()
            .is_some_and(|(_, element_type, first, _)| {
                construction_locals
                    .iter()
                    .all(|(_, candidate_type, candidate, _)| {
                        candidate_type == element_type
                            && candidate.root_structural_type == first.root_structural_type
                    })
                    && cleanup.structural_types.iter().any(|root| {
                        root.id == first.root_structural_type
                            && matches!(
                                root.shape,
                                terminal_psi::StructuralTypeShape::FixedArray { element, length }
                                    if element == *element_type
                                        && length == expected_root_length
                            )
                    })
            })
}

/// Everything the affine cleanup check reads: the function's provenance
/// and attribution, its parameter homes, calls and settlements, the
/// attachments and functions nominal cleanups may invoke, the cleanup
/// record itself, and what the function's continuations already settled.
#[derive(Clone, Copy)]
struct CleanupInputs<'a> {
    provenance: &'a TerminalPsiProvenance,
    attribution: &'a [SemanticCodeAttribution],
    parameter_homes: &'a [UnitParameterHomeRecord],
    internal_unit_calls: &'a [InternalUnitCallRecord],
    boundary_settlements: &'a [BoundarySettlementRecord],
    attachments: &'a std::collections::BTreeMap<MachineId, Option<StructuralTypeId>>,
    functions: &'a std::collections::BTreeMap<MachineId, &'a MachineCodeFunction>,
    cleanup: &'a UnitAffineCleanupRecord,
    allow_mixed_nominal_roots: bool,
    fully_consumed_affine_parameter: bool,
    partially_consumed_affine_parameter: bool,
    continuation_discards: &'a [PlaceId],
}

/// Validates one Unit function's affine cleanup: its construction prefix
/// and span, the roots it must discard, the shape of its actions (whole
/// root discards, residual discards of one parameter, or nominal cleanups),
/// its locals, and the attribution of its edge.
pub(crate) fn validate_unit_affine_cleanup(
    machine: MachineId,
    provenance: &TerminalPsiProvenance,
    bytes: &[u8],
    attribution: &[SemanticCodeAttribution],
    parameter_homes: &[UnitParameterHomeRecord],
    internal_unit_calls: &[InternalUnitCallRecord],
    boundary_settlements: &[BoundarySettlementRecord],
    attachments: &std::collections::BTreeMap<MachineId, Option<StructuralTypeId>>,
    functions: &std::collections::BTreeMap<MachineId, &MachineCodeFunction>,
    cleanup: &UnitAffineCleanupRecord,
    allow_mixed_nominal_roots: bool,
    fully_consumed_affine_parameter: bool,
    partially_consumed_affine_parameter: bool,
    continuation_discards: &[PlaceId],
) -> Result<(), ObjectError> {
    let inputs = CleanupInputs {
        provenance,
        attribution,
        parameter_homes,
        internal_unit_calls,
        boundary_settlements,
        attachments,
        functions,
        cleanup,
        allow_mixed_nominal_roots,
        fully_consumed_affine_parameter,
        partially_consumed_affine_parameter,
        continuation_discards,
    };
    let invalid = || ObjectError::InvalidUnitAffineCleanupEvidence(machine);
    if !exact_construction_prefix(cleanup) {
        return Err(invalid());
    }
    let end = cleanup
        .code_offset
        .checked_add(cleanup.byte_count)
        .ok_or_else(invalid)?;
    let roots = cleanup_roots::cleanup_roots(&inputs);
    let projected_result = crate::object_artifact::replay::structural::affine_projected_calls::exact_projected_affine_result(
        parameter_homes,
        internal_unit_calls,
        Some(cleanup),
    );
    let action_shape_invalid = if projected_result.is_some() {
        false
    } else if cleanup.actions == roots.expected_root_actions {
        discard_actions::root_discards_are_duplicated(cleanup)
    } else if matches!(
        cleanup.actions.get(roots.expected_local_actions.len()),
        Some(terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
            _
        ))
    ) {
        discard_actions::residual_discards_are_malformed(&inputs, &roots)
    } else {
        nominal_actions::nominal_cleanups_are_malformed(&inputs, end)
    };
    if cleanup.byte_count == 0
        || end != bytes.len()
        || !provenance.edges.contains(&cleanup.psi_edge)
        || locals_are_malformed(&inputs, &roots.local_operations)
        || action_shape_invalid
        || edge_attribution_is_not_unique(attribution, cleanup)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Whether the cleanup's locals are malformed: each must be established by
/// a distinct provenance operation attributed to zero bytes, occupy a
/// trivial affine local place of its ordinal and type, and have an empty
/// record type.
fn locals_are_malformed(
    inputs: &CleanupInputs<'_>,
    local_operations: &std::collections::BTreeSet<semantic_vocabulary::OperationId>,
) -> bool {
    let CleanupInputs {
        provenance,
        attribution,
        cleanup,
        ..
    } = *inputs;
    local_operations.len() != cleanup.locals.len()
        || cleanup.locals.iter().enumerate().any(
            |(ordinal, (operation, place, structural_type))| {
                !provenance.operations.contains(operation)
                    || !matches!(
                        place.kind,
                        semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                            declaration_ordinal,
                            structural_type: local_type,
                            ..
                        } if usize::try_from(declaration_ordinal) == Ok(ordinal)
                            && local_type == structural_type.id
                    )
                    || !matches!(
                        structural_type.shape,
                        terminal_psi::StructuralTypeShape::Record { ref fields }
                            if fields.is_empty()
                    )
                    || attribution
                        .iter()
                        .filter(|attribution| {
                            attribution.site == SemanticCodeSite::Operation(*operation)
                                && attribution.byte_count == 0
                        })
                        .count()
                        != 1
            },
        )
}

/// Whether the cleanup's edge is not attributed exactly once to its bytes.
fn edge_attribution_is_not_unique(
    attribution: &[SemanticCodeAttribution],
    cleanup: &UnitAffineCleanupRecord,
) -> bool {
    attribution
        .iter()
        .filter(|attribution| {
            attribution.site == SemanticCodeSite::Edge(cleanup.psi_edge)
                && attribution.code_offset == cleanup.code_offset
                && attribution.byte_count == cleanup.byte_count
        })
        .count()
        != 1
}

fn is_partial_cleanup_path(path: &[terminal_psi::StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path.iter().all(|segment| match segment {
            terminal_psi::StructuralPathSegment::Referent
            | terminal_psi::StructuralPathSegment::FixedByteRange { .. } => false,
            terminal_psi::StructuralPathSegment::Field(identity) => !identity.is_empty(),
            terminal_psi::StructuralPathSegment::FixedIndex(_) => true,
        })
}

fn bounded_nominal_receiver_shape(shape: calling_conventions::ValueShape) -> bool {
    shape == calling_conventions::ValueShape::integer(0, 1)
        || shape.class == calling_conventions::ValueClass::Integer
            && shape.byte_size != 0
            && matches!(shape.alignment, 1 | 2 | 4 | 8)
            && shape.byte_size.is_multiple_of(shape.alignment)
}

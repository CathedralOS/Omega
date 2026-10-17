//! A return consumes a projected binding at its actual exit, not at the
//! binding's establishment. Earlier sibling uses and moves must finish first.
//! The checked exit already owns the residual cleanup of the current frontier;
//! reconstructing the initial owner's complement here would revive moved holes.

use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::lowering_error::LoweringError;
use crate::terminal_identities::{allocate_dense, block_id, edge_id, place_id};
use crate::unit::attached_unit::argument_evaluation::Evaluation;
use crate::unit::unit_cleanup::CleanupRegion;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{
    Block, StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, Terminator,
};

/// Materialize the exact returned subtree into one whole arrival place. The
/// existing edge transport transfers custody and independently checks cleanup;
/// no reference establishment, origin selection or copy permission is invented.
pub(super) fn materialize(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    source: &StructuralArgument,
    structural_type: semantic_vocabulary::StructuralTypeId,
    multiplicity: terminal_psi::StructuralMultiplicity,
    completion: Terminator,
    regions: Vec<CleanupRegion>,
    next_place: &mut u64,
    temporary_places: &mut Vec<StructuralPlaceDeclaration>,
    evaluation: &mut Evaluation,
    values: &mut Vec<terminal_psi::ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(Terminator, Option<Block>), LoweringError> {
    if source.path.is_empty() {
        let mut completion = completion;
        let Terminator::ReturnStructural {
            cleanup_actions, ..
        } = &mut completion
        else {
            return Err(LoweringError::Unsupported(
                "structural return lost its completion",
            ));
        };
        *cleanup_actions = evaluation.emit_cleanup_regions(
            identities, checked, regions, values, next_value, next_block, next_edge, operations,
        )?;
        return Ok((completion, None));
    }
    let Terminator::ReturnStructural {
        source: returned_source,
        returned_claims,
        ..
    } = completion
    else {
        return Err(LoweringError::Unsupported(
            "projected return lost its completion",
        ));
    };
    if returned_source != source.place {
        return Err(LoweringError::Unsupported(
            "projected return lost its current root",
        ));
    }
    let block = block_id(allocate_dense(next_block)?);
    let place = place_id(allocate_dense(next_place)?);
    temporary_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::BlockParameter { block, position: 0 },
    });
    let mut arrival_block = block;
    let mut arrival_parameters = Vec::new();
    let mut structural_parameters = vec![StructuralParameterDeclaration {
        place,
        position: 0,
        structural_type,
        multiplicity,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    let selected = regions
        .iter()
        .any(|region| !region.conditions.is_empty() || region.selected_region.is_some());
    let capture_operation_end = operations.len();
    let mut operation_start = capture_operation_end;
    let cleanup_actions = if selected {
        crate::unit::attached_unit::argument_evaluation::cleanup_control::CleanupEmission {
            checked,
            current: &mut arrival_block,
            parameters: &mut arrival_parameters,
            structural_parameters: &mut structural_parameters,
            block_erased_scalar_formals: &mut Vec::new(),
            erased_proof_formals: &evaluation.erased_proof_formals,
            operation_start: &mut operation_start,
            blocks: &mut evaluation.blocks,
        }
        .emit(
            identities,
            &regions,
            &mut Vec::new(),
            next_value,
            next_block,
            next_edge,
            operations,
        )?;
        Vec::new()
    } else {
        regions.into_iter().map(|region| region.action).collect()
    };
    let arrival = Block {
        id: arrival_block,
        parameters: arrival_parameters,
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: crate::scalar_lowering::contracts::erased_proof_formal_declarations(
            identities,
            checked,
            &evaluation.erased_proof_formals,
        ),
        structural_parameters,
        operations: operations[operation_start..].to_vec(),
        terminator: Terminator::ReturnStructural {
            edge: edge_id(allocate_dense(next_edge)?),
            source: place,
            returned_claims,
            cleanup_actions: Vec::new(),
        },
    };
    // Arrival cleanup blocks now own these tag observations. They are not
    // operations of the source block that captures the projected result.
    // Keep allocated identities monotonic while restoring that source prefix.
    operations.truncate(capture_operation_end);
    Ok((
        Terminator::Jump {
            edge: edge_id(allocate_dense(next_edge)?),
            target: block,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: (0..evaluation.erased_proof_formals.len())
                .map(|position| {
                    Ok(semantic_vocabulary::ProofTerm::Formal {
                        position: u32::try_from(position).map_err(|_| {
                            LoweringError::Unsupported(
                                "projected return erased-proof roster exceeds u32",
                            )
                        })?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
            structural_arguments: vec![source.clone()],
            cleanup_actions,
        },
        Some(arrival),
    ))
}

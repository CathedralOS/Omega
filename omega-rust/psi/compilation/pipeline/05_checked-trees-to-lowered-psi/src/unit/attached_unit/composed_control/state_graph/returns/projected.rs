//! A return consumes a projected binding at its actual exit, not at the
//! binding's establishment. Earlier sibling uses and moves must finish first.
//! The checked exit already owns the residual cleanup of the current frontier;
//! reconstructing the initial owner's complement here would revive moved holes.

use super::super::super::catalogs::ComposedCatalogs;
use crate::emission::operation_emission::buffer::StructuralBinding;
use crate::lowering_error::LoweringError;
use crate::terminal_identities::{allocate_dense, block_id, edge_id, place_id};
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{
    Block, StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, Terminator,
};

/// Materialize the exact returned subtree into one whole arrival place. The
/// existing edge transport transfers custody and independently checks cleanup;
/// no reference establishment, origin selection or copy permission is invented.
pub(in crate::unit::attached_unit::composed_control::state_graph) fn materialize(
    binding: &StructuralBinding,
    source: semantic_vocabulary::PlaceId,
    completion: Terminator,
    catalogs: &mut ComposedCatalogs<'_>,
    next_block: &mut u64,
    next_edge: &mut u64,
) -> Result<(Terminator, Option<Block>), LoweringError> {
    if binding.path().is_empty() {
        return Ok((completion, None));
    }
    let Terminator::ReturnStructural {
        source: returned_source,
        returned_claims,
        cleanup_actions,
        ..
    } = completion
    else {
        return Err(LoweringError::Unsupported(
            "projected return lost its completion",
        ));
    };
    if returned_source != source {
        return Err(LoweringError::Unsupported(
            "projected return lost its current root",
        ));
    }
    let block = block_id(allocate_dense(next_block)?);
    let place = place_id(allocate_dense(&mut catalogs.next_place)?);
    catalogs.temporary_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::BlockParameter { block, position: 0 },
    });
    let arrival = Block {
        id: block,
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place,
            position: 0,
            structural_type: binding.structural_type(),
            multiplicity: binding.multiplicity(),
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        operations: Vec::new(),
        terminator: Terminator::ReturnStructural {
            edge: edge_id(allocate_dense(next_edge)?),
            source: place,
            returned_claims,
            cleanup_actions: Vec::new(),
        },
    };
    Ok((
        Terminator::Jump {
            edge: edge_id(allocate_dense(next_edge)?),
            target: block,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: source,
                path: binding.path().to_vec(),
                access: StructuralAccess::Owned,
            }],
            cleanup_actions,
        },
        Some(arrival),
    ))
}

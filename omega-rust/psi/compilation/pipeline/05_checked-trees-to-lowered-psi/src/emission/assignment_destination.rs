//! Capture an evaluated assignment address before evaluating its replacement.
//!
//! A path is not an address: a later RHS call may replace a reference
//! descriptor traversed by that path. A borrowed continuation parameter binds
//! the actual endpoint on the incoming edge. This presents storage without
//! observing its old value, moving its owner or widening its existing access.

use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::lowering_error::LoweringError;
use crate::terminal_identities::{allocate_dense, block_id, edge_id, place_id, value_id};
use crate::unit::attached_unit::argument_evaluation::Evaluation;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    Block, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration, Terminator,
    ValueDeclaration,
};

pub(super) fn capture(
    source: PlaceId,
    path: Vec<StructuralPathSegment>,
    structural_type: StructuralTypeId,
    access: StructuralAccess,
    qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
    evaluation: &mut Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    next_place: &mut u64,
    places: &mut Vec<StructuralPlaceDeclaration>,
    operations: &mut OperationBuffer,
) -> Result<PlaceId, LoweringError> {
    let continuation = block_id(allocate_dense(next_block)?);
    let place = place_id(allocate_dense(next_place)?);
    let parameters = values
        .iter()
        .map(|value| {
            Ok(ValueDeclaration {
                id: value_id(allocate_dense(next_value)?),
                scalar_type: value.scalar_type,
                qualifications: value.qualifications.clone(),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::BlockParameter {
            block: continuation,
            position: 0,
        },
    });
    evaluation.blocks.push(Block {
        id: evaluation.current,
        parameters: std::mem::take(&mut evaluation.parameters),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: std::mem::take(&mut evaluation.block_structural_parameters),
        operations: operations[evaluation.operation_start..].to_vec(),
        terminator: Terminator::Jump {
            edge: edge_id(allocate_dense(next_edge)?),
            target: continuation,
            arguments: values.iter().map(|value| value.id).collect(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: source,
                path,
                access,
            }],
            cleanup_actions: Vec::new(),
        },
    });
    evaluation.current = continuation;
    evaluation.operation_start = operations.len();
    operations.length_observations.clear();
    *values = parameters.clone();
    evaluation.parameters = parameters;
    evaluation.block_structural_parameters = vec![StructuralParameterDeclaration {
        place,
        position: 0,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications,
        projected_qualifications,
    }];
    Ok(place)
}

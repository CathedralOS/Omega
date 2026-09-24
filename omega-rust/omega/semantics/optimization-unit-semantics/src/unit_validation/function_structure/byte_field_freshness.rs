//! Current-CFG custody of a bounded field's live-length observation.
//! Original Terminal provenance cannot authorize a stale observation after a rewrite.
use crate::OptimizationUnitValidationError;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

#[cfg(test)]
mod tests;

pub(super) fn validate(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    for (block_position, block) in function.blocks.iter().enumerate() {
        for (node_position, node) in block.nodes.iter().enumerate() {
            let O::StructuralByteSequenceFieldByteStore {
                destination,
                path,
                field,
                length,
                ..
            } = &node.operation
            else {
                continue;
            };
            let invalid = OptimizationUnitValidationError::InvalidByteSequenceWrite {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_position).map_err(|_| {
                    OptimizationUnitValidationError::StructuralCatalogMismatch {
                        machine: Some(function.machine),
                    }
                })?,
            };
            let exact_path = field_path(function, types, *destination, path, *field)
                .ok_or_else(|| invalid.clone())?;
            let producer = function.blocks.iter().enumerate().find_map(|(owner, block)| {
                block.nodes.iter().position(|candidate| matches!(&candidate.operation,
                    O::StructuralByteSequenceFieldLength { source, path: measured_path, field: measured_field, result, .. }
                    if source == destination && measured_path == path && measured_field == field && result.value == *length
                )).map(|position| (owner, position))
            }).ok_or_else(|| invalid.clone())?;
            if !unchanged_since(
                function,
                predecessors,
                producer,
                (block_position, node_position),
                |operation| changes_length(function, types, *destination, &exact_path, operation),
            ) {
                return Err(invalid);
            }
        }
    }
    Ok(())
}

fn field_path(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root: PlaceId,
    path: &[StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<Vec<StructuralPathSegment>> {
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == root)?;
    let carrier = super::super::structural_catalog::resolve_structural_path(
        types,
        parameter.structural_type,
        path,
    )?;
    let StructuralTypeShape::Record { fields } = &types.get(&carrier)?.shape else {
        return None;
    };
    let selected = fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?;
    let mut exact = path.to_vec();
    exact.push(StructuralPathSegment::Field(selected.identity.clone()));
    Some(exact)
}

fn overlaps(left: &[StructuralPathSegment], right: &[StructuralPathSegment]) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn changes_length(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root: PlaceId,
    path: &[StructuralPathSegment],
    operation: &O,
) -> bool {
    match operation {
        O::StructuralByteSequenceFieldStore {
            destination,
            path: written,
            field,
            ..
        } => {
            *destination == root
                && field_path(function, types, *destination, written, *field)
                    .is_none_or(|written| overlaps(path, &written))
        }
        O::WriteOnlyPrimitiveStore { destination, .. } => destination.place == root,
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments.iter().any(|argument| {
            argument.place == root
                && overlaps(path, &argument.path)
                && matches!(
                    argument.access,
                    StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                        | StructuralAccess::Owned
                )
        }),
        // These operations cannot alter this bounded field's live length.
        // Scalar stores target a separately validated primitive leaf, and byte
        // stores preserve length even when they replace the measured field's contents.
        O::StructuralByteSequenceFieldByteStore { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::StructuralScalarFieldStore { .. }
        | O::ByteSequenceWrite { .. }
        | O::ByteSequenceRead { .. }
        | O::ByteSequenceLength { .. }
        | O::ByteSequenceSubslice { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishPrimitiveLocal { .. }
        | O::PrimitiveLocalStore { .. }
        | O::PrimitiveScalarRead { .. }
        | O::EstablishScalarArray { .. }
        | O::EstablishScalarCase { .. }
        | O::EstablishRecord { .. }
        | O::StructuralCaseMembership { .. }
        | O::IntegerStructuralField { .. }
        | O::IndexedPrimitiveRead { .. }
        | O::BooleanStructuralField { .. }
        | O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
        | O::IeeeFloatCompare { .. }
        | O::BooleanConstant { .. }
        | O::BooleanNot { .. }
        | O::BooleanEqual { .. }
        | O::IntegerEqual { .. }
        | O::IntegerLessThan { .. }
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerWiden { .. }
        | O::IntegerExactCast { .. }
        | O::IntegerBitwiseAnd { .. }
        | O::IntegerBitwiseOr { .. }
        | O::IntegerBitwiseXor { .. }
        | O::WrappingIntegerShiftLeft { .. }
        | O::WrappingIntegerShiftRight { .. }
        | O::ExactIntegerShiftLeft { .. }
        | O::ExactIntegerShiftRight { .. }
        | O::WrappingIntegerAdd { .. }
        | O::ExactIntegerAdd { .. }
        | O::SaturatingIntegerAdd { .. }
        | O::WrappingIntegerSubtract { .. }
        | O::ExactIntegerSubtract { .. }
        | O::SaturatingIntegerSubtract { .. }
        | O::WrappingIntegerMultiply { .. }
        | O::ExactIntegerMultiply { .. }
        | O::SaturatingIntegerMultiply { .. }
        | O::TrappingInteger { .. }
        | O::ExactIntegerDivide { .. }
        | O::ExactIntegerRemainder { .. }
        | O::WrappingIntegerDivide { .. }
        | O::WrappingIntegerRemainder { .. }
        | O::SaturatingIntegerDivide { .. }
        | O::SaturatingIntegerRemainder { .. }
        | O::Call { .. }
        | O::Jump { .. }
        | O::Conditional { .. }
        | O::StructuralCase { .. }
        | O::Return { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => false,
        // Unresolved dynamic receivers, escaped descriptors, and other effects
        // have no independently established disjoint write footprint here.
        _ => true,
    }
}

/// Every finite path arriving at the use must reach the observation first,
/// without a length-changing operation. Revisiting a point closes only that
/// graph search branch; other predecessors and mutations on backedges still run.
fn unchanged_since(
    function: &PsiOptimizationFunction,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    producer: (usize, usize),
    use_site: (usize, usize),
    invalidates: impl Fn(&O) -> bool,
) -> bool {
    let mut pending = vec![use_site];
    let mut visited = BTreeSet::new();
    let mut reached_producer = false;
    while let Some((block_position, position)) = pending.pop() {
        if !visited.insert((block_position, position)) {
            continue;
        }
        let block = &function.blocks[block_position];
        if position != 0 {
            let previous = (block_position, position - 1);
            if previous == producer {
                reached_producer = true;
            } else {
                if invalidates(&block.nodes[position - 1].operation) {
                    return false;
                }
                pending.push(previous);
            }
        } else {
            if block.id == function.entry {
                return false;
            }
            let Some(arrivals) = predecessors
                .get(&block.id)
                .filter(|arrivals| !arrivals.is_empty())
            else {
                return false;
            };
            for arrival in arrivals {
                let Some((owner, predecessor)) = function
                    .blocks
                    .iter()
                    .enumerate()
                    .find(|(_, candidate)| candidate.id == *arrival)
                else {
                    return false;
                };
                let mut found_edge = false;
                for (position, node) in predecessor.nodes.iter().enumerate() {
                    if node.successors.iter().any(|edge| edge.target == block.id) {
                        found_edge = true;
                        pending.push((owner, position + 1));
                    }
                }
                if !found_edge {
                    return false;
                }
            }
        }
    }
    reached_producer
}

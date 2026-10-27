//! Independently replay an owned byte snapshot's exact sources and live extents.
//! Capacity evidence is checked against the current declaration by the unit catalog.
use crate::OptimizationUnitValidationError;
use crate::unit_validation::catalog_mismatch;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, StructuralDomainId, StructuralPlaceKind, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::control_flow::ByteSequenceOperand;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralDomainDeclaration, StructuralMultiplicity,
    StructuralTypeDeclaration, StructuralTypeShape,
};

pub(super) fn validate(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    let invalid = || catalog_mismatch(function);
    for (block_position, block) in function.blocks.iter().enumerate() {
        for (node_position, node) in block.nodes.iter().enumerate() {
            let O::EstablishByteSequenceConcatenation {
                psi_operation,
                result,
                left,
                right,
                ..
            } = &node.operation
            else {
                continue;
            };
            if result.multiplicity != StructuralMultiplicity::Unrestricted
                || !result.projected_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
                || !result.claims.is_empty()
                || !types
                    .get(&result.structural_type)
                    .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence())
                || !function.structural_places.iter().any(|place| {
                    place.id == result.place
                        && place.kind
                            == (StructuralPlaceKind::OperationResult {
                                producer: *psi_operation,
                                structural_type: result.structural_type,
                            })
                })
            {
                return Err(invalid());
            }
            for operand in [left, right] {
                if operand.source() == result.place
                    || function.entry_claim_declared(operand.source())
                    || function.content_entry_claim_declared(operand.source())
                {
                    return Err(invalid());
                }
                match operand {
                    ByteSequenceOperand::Whole { source, length } => {
                        if !super::descriptor_sources::valid_whole(
                            function, types, domains, source, *length,
                        ) {
                            return Err(invalid());
                        }
                    }
                    ByteSequenceOperand::Field {
                        source,
                        field,
                        length,
                    } => {
                        let (structural_type, access) =
                            super::byte_field_freshness::byte_field_root(function, source.place)
                                .ok_or_else(invalid)?;
                        if source.access != StructuralAccess::SharedBorrow
                            || access == StructuralAccess::WriteOnlyBorrow
                            || !terminal_psi::is_static_structural_scalar_store_path(&source.path)
                        {
                            return Err(invalid());
                        }
                        let parent = super::super::structural_catalog::resolve_structural_path(
                            types,
                            structural_type,
                            &source.path,
                        )
                        .ok_or_else(invalid)?;
                        let StructuralTypeShape::Record { fields, .. } =
                            &types.get(&parent).ok_or_else(invalid)?.shape
                        else {
                            return Err(invalid());
                        };
                        if !fields.iter().any(|declaration| {
                            declaration.id == *field
                                && !declaration.relevance.is_erased()
                                && matches!(
                                    declaration.field_type.byte_sequence_carrier(),
                                    Some(ByteSequenceCarrier::BoundedOwned { .. })
                                )
                        }) {
                            return Err(invalid());
                        }
                        let exact = super::byte_field_freshness::field_path(
                            function,
                            types,
                            source.place,
                            &source.path,
                            *field,
                        )
                        .ok_or_else(invalid)?;
                        let names = crate::unit_validation::arrival_storage::storage_names(
                            function,
                            block.id,
                            source.place,
                        );
                        let (producer, producer_root) = function.blocks.iter().enumerate().find_map(|(owner, block)| names.in_block(block.id).find_map(|name| block.nodes.iter().position(|candidate| matches!(&candidate.operation,
                            O::StructuralByteSequenceFieldLength { source: measured, path, field: measured_field, result, .. }
                            if *measured == name && *path == source.path && measured_field == field && result.value == *length)).map(|position| ((owner, position), name)))).ok_or_else(invalid)?;
                        if !super::structural_freshness::unchanged_since(
                            function,
                            predecessors,
                            producer,
                            producer_root,
                            (block_position, node_position),
                            source.place,
                            |candidate, root| {
                                super::structural_freshness::structural_metadata_may_change(
                                    function, types, root, &exact, candidate,
                                ) || matches!(candidate, O::StructuralByteSequenceFieldByteStore { destination, path, field, .. }
                                    if *destination == root && super::byte_field_freshness::field_path(function, types, *destination, path, *field).is_none_or(|written| exact.starts_with(&written) || written.starts_with(&exact)))
                            },
                        ) {
                            return Err(invalid());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

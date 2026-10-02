//! Independently replay an owned byte snapshot's exact sources and live extents.
//! Capacity evidence is checked against the current declaration by the unit catalog.
use crate::OptimizationUnitValidationError;
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
    let invalid = || OptimizationUnitValidationError::StructuralCatalogMismatch {
        machine: Some(function.machine),
    };
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
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            StructuralTypeShape::ByteSequence(
                                ByteSequenceCarrier::BoundedOwned { .. }
                            )
                        )
                    })
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
                    || function
                        .entry_claim_declarations
                        .iter()
                        .any(|claim| claim.input == operand.source())
                    || function
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == operand.source())
                {
                    return Err(invalid());
                }
                match operand {
                    ByteSequenceOperand::Whole { source, length } => {
                        let kind = function
                            .structural_places
                            .iter()
                            .find(|place| place.id == *source)
                            .map(|place| &place.kind);
                        super::byte_views::validate_source(
                            function,
                            block.id,
                            u32::try_from(node_position).map_err(|_| invalid())?,
                            &node.operation,
                            kind,
                            types,
                            domains,
                            *source,
                            Some(*length),
                        )?;
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
                            || !terminal_psi::is_bounded_structural_scalar_store_path(&source.path)
                        {
                            return Err(invalid());
                        }
                        let parent = super::super::structural_catalog::resolve_structural_path(
                            types,
                            structural_type,
                            &source.path,
                        )
                        .ok_or_else(invalid)?;
                        let StructuralTypeShape::Record { fields } =
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::indexed_byte_fields::indexed_field_unit;
    use crate::tests::support::id;
    use semantic_vocabulary::{ObligationId, OperationId, PlaceId};

    #[test]
    fn concatenation_replays_content_freshness_and_read_authority() {
        let mut unit = indexed_field_unit();
        let structural_type = id(90_001, StructuralTypeId::new);
        unit.structural_types
            .make_mut()
            .push(StructuralTypeDeclaration {
                id: structural_type,
                identity: "concat-owned-six".into(),
                shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity: 6,
                }),
            });
        let types = unit
            .structural_types
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect::<BTreeMap<_, _>>();
        let mut function = unit
            .functions
            .iter()
            .find(|function| function.machine == unit.entry)
            .unwrap()
            .clone();
        let (block, position) = function
            .blocks
            .iter()
            .enumerate()
            .find_map(|(block, owner)| {
                owner
                    .nodes
                    .iter()
                    .position(|node| {
                        matches!(
                            node.operation,
                            O::StructuralByteSequenceFieldByteStore { .. }
                        )
                    })
                    .map(|position| (block, position))
            })
            .unwrap();
        let byte_write = function.blocks[block].nodes[position].clone();
        let O::StructuralByteSequenceFieldByteStore {
            destination,
            ref path,
            field,
            length,
            ..
        } = byte_write.operation
        else {
            unreachable!()
        };
        let operand = ByteSequenceOperand::Field {
            source: terminal_psi::StructuralArgument {
                place: destination,
                access: StructuralAccess::SharedBorrow,
                path: path.clone(),
            },
            field,
            length,
        };
        let operation = id(90_002, OperationId::new);
        let place = id(90_003, PlaceId::new);
        let mut concat = byte_write.clone();
        concat.operation = O::EstablishByteSequenceConcatenation {
            psi_operation: operation,
            result: terminal_psi::StructuralOperationResult {
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
            left: operand.clone(),
            right: operand,
            capacity_obligation: id(90_004, ObligationId::new),
        };
        function
            .structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type,
                },
            });
        function.blocks[block].nodes.insert(position, concat);
        let domains = unit
            .structural_domains
            .iter()
            .map(|domain| (domain.id, domain))
            .collect::<BTreeMap<_, _>>();
        validate(&function, &types, &domains, &BTreeMap::new()).unwrap();
        crate::unit_validation::structural_catalog::validate_function_structural_catalog(
            &function, &types, &domains,
        )
        .expect("the generated owned place names its exact concatenation producer");

        let mut stale = function.clone();
        stale.blocks[block].nodes.insert(position, byte_write);
        assert!(
            validate(&stale, &types, &domains, &BTreeMap::new()).is_err(),
            "a length-preserving byte write changes the captured content"
        );

        let mut unreadable = function.clone();
        unreadable
            .structural_parameters
            .iter_mut()
            .find(|parameter| parameter.place == destination)
            .unwrap()
            .access = StructuralAccess::WriteOnlyBorrow;
        assert!(
            validate(&unreadable, &types, &domains, &BTreeMap::new()).is_err(),
            "metadata observation cannot grant content read authority"
        );

        let mut substituted = function;
        let O::EstablishByteSequenceConcatenation { left, .. } =
            &mut substituted.blocks[block].nodes[position].operation
        else {
            unreachable!()
        };
        let ByteSequenceOperand::Field { field, .. } = left else {
            unreachable!()
        };
        *field = id(90_005, semantic_vocabulary::StructuralFieldId::new);
        assert!(
            validate(&substituted, &types, &domains, &BTreeMap::new()).is_err(),
            "length must observe the exact selected field"
        );
    }
}

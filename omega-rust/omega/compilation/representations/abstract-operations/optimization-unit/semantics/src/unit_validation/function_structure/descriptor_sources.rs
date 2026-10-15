//! Descriptor observations and subranges use the same complete source owner.
//! Type projection supplies a carrier; live source/loan availability and each
//! operation's selector and range obligations remain independent judgments.
use crate::unit_validation::structural_catalog::{
    StructuralStorageProjection, structural_borrowed_storage_projection,
};
use abstract_operations::AbstractOperation as Operation;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeMap;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(super) fn valid(
    function: &PsiOptimizationFunction,
    operation: &Operation,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    domains: &BTreeMap<
        semantic_vocabulary::StructuralDomainId,
        &terminal_psi::StructuralDomainDeclaration,
    >,
) -> bool {
    let (source, bytes, cut) = match operation {
        Operation::ByteSequenceLength { source, .. } => (source, true, None),
        Operation::ElementViewLength { source, .. } => (source, false, None),
        Operation::ByteSequenceSubslice {
            source,
            result,
            length,
            ..
        } => (source, true, Some((result, *length))),
        Operation::ElementViewSubslice {
            source,
            result,
            length,
            ..
        } => (source, false, Some((result, *length))),
        _ => return false,
    };
    let requested = match cut {
        Some((result, _)) if result.multiplicity == StructuralMultiplicity::Affine => {
            StructuralAccess::MutableBorrow
        }
        Some((result, _)) if result.multiplicity != StructuralMultiplicity::Unrestricted => {
            return false;
        }
        _ => StructuralAccess::SharedBorrow,
    };
    let Some(shape) = source_shape(function, types, domains, source, requested) else {
        return false;
    };
    if !matches!(
        (&shape, bytes),
        (StructuralTypeShape::ByteSequence(_), true)
            | (StructuralTypeShape::ElementView { .. }, false)
    ) {
        return false;
    }
    if let Some((result, length)) = cut {
        if result.place == source.place
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
        {
            return false;
        }
        let result_shape = types
            .get(&result.structural_type)
            .map(|declaration| &declaration.shape);
        let carrier_matches = match (&shape, result_shape) {
            (
                StructuralTypeShape::ElementView { element, .. },
                Some(StructuralTypeShape::ElementView {
                    element: result_element,
                    ..
                }),
            ) => element == result_element,
            (
                StructuralTypeShape::ByteSequence(_),
                Some(StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    ..
                })),
            ) => true,
            _ => false,
        };
        if !carrier_matches {
            return false;
        }
        function.block_nodes().any(|node| match &node.operation {
            Operation::ByteSequenceLength {
                result: measured,
                source: observed,
                ..
            } if bytes => {
                measured.value == length
                    && terminal_semantics::same_descriptor_coordinate(observed, source)
            }
            Operation::ElementViewLength {
                result: measured,
                source: observed,
                ..
            } if !bytes => {
                measured.value == length
                    && terminal_semantics::same_descriptor_coordinate(observed, source)
            }
            _ => false,
        })
    } else {
        true
    }
}
fn covers(actual: StructuralAccess, requested: StructuralAccess) -> bool {
    actual == requested
        || actual == StructuralAccess::Owned
        || actual == StructuralAccess::MutableBorrow && requested == StructuralAccess::SharedBorrow
}

/// Read the complete selected descriptor and pair it with its own observation.
/// Scalar dominance and current carrier custody are checked independently.
pub(super) fn valid_whole(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    domains: &BTreeMap<
        semantic_vocabulary::StructuralDomainId,
        &terminal_psi::StructuralDomainDeclaration,
    >,
    source: &terminal_psi::StructuralArgument,
    length: semantic_vocabulary::ValueId,
) -> bool {
    source.access == StructuralAccess::SharedBorrow
        && matches!(source_shape(function, types, domains, source, StructuralAccess::SharedBorrow),
        Some(StructuralTypeShape::ByteSequence(_)))
        && function.block_nodes().any(|node| matches!(&node.operation,
            Operation::ByteSequenceLength { source: measured, result, .. }
            if result.value == length && terminal_semantics::same_descriptor_coordinate(measured, source)))
}

fn source_shape(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    domains: &BTreeMap<
        semantic_vocabulary::StructuralDomainId,
        &terminal_psi::StructuralDomainDeclaration,
    >,
    source: &terminal_psi::StructuralArgument,
    requested: StructuralAccess,
) -> Option<StructuralTypeShape> {
    if !covers(source.access, requested) {
        return None;
    }
    let mut borrow = source.clone();
    // A readable owned arrival may observe its carrier without transferring
    // ownership. Attenuating for projection does not change the retained operand.
    borrow.access = requested;
    let signature = crate::unit_validation::operation_contracts::structural_source_contract(
        function,
        source.place,
    )?;
    if !terminal_semantics::descriptor_source_projection_accessible(
        types.values().copied(),
        signature.structural_type,
        signature.multiplicity,
        signature.backing_access(types.values().copied()),
        &source.path,
        requested,
    ) {
        return None;
    }
    let byte_qualified = types
        .get(&signature.structural_type)
        .is_some_and(|declaration| {
            matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
        })
        && signature.declared_byte_predicates(domains).is_some();
    if signature.multiplicity == StructuralMultiplicity::Linear
        || signature.has_result_evidence()
        || !(signature.is_unqualified() || byte_qualified)
    {
        return None;
    }
    let (root, prefix) = crate::unit_validation::references::project_referent_tail(
        types,
        signature.structural_type,
        signature.backing_access(types.values().copied()),
        &borrow,
    )?;
    let selected = structural_borrowed_storage_projection(
        types.values().copied(),
        root,
        &source.path[prefix..],
        requested,
    )?;
    match selected {
        StructuralStorageProjection::Endpoint(identity) => types
            .get(&identity)
            .map(|declaration| declaration.shape.clone()),
        StructuralStorageProjection::ByteField { carrier, .. } => {
            if matches!(carrier, ByteSequenceCarrier::BorrowedView { access: Some(access) } | ByteSequenceCarrier::BoundedView { access: Some(access), .. } if !covers(access, requested))
            {
                return None;
            }
            Some(StructuralTypeShape::ByteSequence(carrier))
        }
        StructuralStorageProjection::ByteWindow { .. } => None,
    }
}

/// A count observes the current selected descriptor, not an immutable snapshot
/// of its old pointer words. Reuse the CFG metadata frontier for every complete
/// byte operand, including projected descriptors reached through references.
pub(super) fn validate_whole_freshness(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    predecessors: &BTreeMap<
        semantic_vocabulary::BlockId,
        std::collections::BTreeSet<semantic_vocabulary::BlockId>,
    >,
) -> Result<(), crate::OptimizationUnitValidationError> {
    for (block_position, block) in function.blocks.iter().enumerate() {
        for (node_position, node) in block.nodes.iter().enumerate() {
            let operands = match &node.operation {
                Operation::EstablishByteSequenceConcatenation { left, right, .. }
                | Operation::ByteSequenceEqual { left, right, .. } => [Some(left), Some(right)],
                Operation::ByteSequencePredicate { operand, .. } => [Some(operand), None],
                _ => continue,
            };
            for operand in operands.into_iter().flatten() {
                let terminal_psi::control_flow::ByteSequenceOperand::Whole { source, length } =
                    operand
                else {
                    continue;
                };
                let invalid = || crate::unit_validation::catalog_mismatch(function);
                let names = crate::unit_validation::arrival_storage::storage_names(
                    function,
                    block.id,
                    source.place,
                );
                let (producer, producer_root) = function.blocks.iter().enumerate().find_map(|(owner, block)| {
                    names.in_block(block.id).find_map(|name| {
                        block.nodes.iter().position(|candidate| matches!(&candidate.operation,
                            Operation::ByteSequenceLength { source: observed, result, .. }
                            if observed.place == name
                                && terminal_semantics::structural_paths_select_same_location(&observed.path, &source.path)
                                && result.value == *length
                        )).map(|position| ((owner, position), name))
                    })
                }).ok_or_else(invalid)?;
                if !super::structural_freshness::unchanged_since(
                    function,
                    predecessors,
                    producer,
                    producer_root,
                    (block_position, node_position),
                    source.place,
                    |operation, root| {
                        // A different carrier name cannot establish disjointness
                        // from storage reached through an opaque reference.
                        source
                            .path
                            .contains(&terminal_psi::StructuralPathSegment::Referent)
                            && matches!(
                                operation,
                                Operation::ByteSequenceStore { .. }
                                    | Operation::StructuralByteSequenceFieldStore { .. }
                            )
                            || super::structural_freshness::structural_metadata_may_change(
                                function,
                                types,
                                root,
                                &source.path,
                                operation,
                            )
                    },
                ) {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(())
}

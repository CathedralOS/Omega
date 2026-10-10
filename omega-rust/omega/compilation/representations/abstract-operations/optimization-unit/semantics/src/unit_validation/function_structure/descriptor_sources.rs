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
    if !covers(source.access, requested) {
        return false;
    }
    let mut borrow = source.clone();
    // A readable owned arrival may observe its carrier without transferring
    // ownership. Attenuating for projection does not change the retained operand.
    borrow.access = requested;
    let Some((root, prefix)) =
        crate::unit_validation::references::access_referent_tail(function, types, &borrow)
    else {
        return false;
    };
    let Some(selected) = structural_borrowed_storage_projection(
        types.values().copied(),
        root,
        &source.path[prefix..],
        requested,
    ) else {
        return false;
    };
    let shape = match selected {
        StructuralStorageProjection::Endpoint(identity) => types
            .get(&identity)
            .map(|declaration| declaration.shape.clone()),
        StructuralStorageProjection::ByteField { carrier, .. } => {
            if matches!(carrier, ByteSequenceCarrier::BorrowedView { access: Some(access) } | ByteSequenceCarrier::BoundedView { access: Some(access), .. } if !covers(access, requested))
            {
                return false;
            }
            Some(StructuralTypeShape::ByteSequence(carrier))
        }
        StructuralStorageProjection::ByteWindow { .. } => None,
    };
    let Some(shape) = shape else {
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
                StructuralTypeShape::ElementView { element },
                Some(StructuralTypeShape::ElementView {
                    element: result_element,
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

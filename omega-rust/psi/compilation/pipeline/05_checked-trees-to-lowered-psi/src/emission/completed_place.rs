//! One place's authored evaluation frontiers, shared by call schedules.
//!
//! A collection is observed before its index. Descriptor crossings therefore
//! complete before the next selector can replace their supplying slot. Inline
//! offsets need no saved address. An owned copy reads its selected contents only
//! after its own selectors, and before the next actual argument begins.

use crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment;
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

#[derive(Clone)]
pub(crate) enum Frontier {
    Address {
        prefix: usize,
        structural_type: StructuralTypeId,
    },
    Selector {
        position: usize,
    },
    Value {
        structural_type: StructuralTypeId,
    },
}

pub(crate) fn plan(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    path: &[PreparedStructuralPathSegment],
    copy_value: bool,
    access: terminal_psi::StructuralAccess,
) -> Result<Vec<Frontier>, LoweringError> {
    let mut frontiers = Vec::new();
    let capture_address = access != terminal_psi::StructuralAccess::Owned || copy_value;
    let mut type_path = Vec::new();
    let root_declaration = types
        .iter()
        .find(|declaration| declaration.id == root)
        .ok_or(LoweringError::Unsupported(
            "completed place has no root declaration",
        ))?;
    let mut pending = matches!(
        root_declaration.shape,
        StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
            )
    )
    .then_some((0, root));
    for (position, segment) in path.iter().enumerate() {
        let index = matches!(
            segment,
            PreparedStructuralPathSegment::ComputedIndex(_)
                | PreparedStructuralPathSegment::EvaluatedFixedIndex { .. }
                | PreparedStructuralPathSegment::RuntimeIndex(_)
                | PreparedStructuralPathSegment::CapturedIndex(_)
                | PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedIndex(_))
        );
        if index {
            let tip =
                terminal_semantics::runtime_structural_path_tip(types.iter(), root, &type_path)
                    .ok_or(LoweringError::Unsupported(
                        "completed place lost its indexed carrier",
                    ))?;
            if matches!(
                tip.shape,
                StructuralTypeShape::ElementView { .. }
                    | StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                            | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                    )
            ) {
                // A view's extent belongs to that collection, so capture before
                // its marker as well as before its index expression.
                let prefix = if position > 0
                    && matches!(
                        path[position - 1],
                        PreparedStructuralPathSegment::ViewExtent { .. }
                    ) {
                    position - 1
                } else {
                    position
                };
                pending = Some((prefix, tip.id));
            }
        }
        if matches!(
            segment,
            PreparedStructuralPathSegment::ComputedIndex(_)
                | PreparedStructuralPathSegment::EvaluatedFixedIndex { .. }
                | PreparedStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::StorageRead { .. }
                        | checked_trees::CheckedRuntimeIndex::StructuralField(_)
                )
        ) {
            if let Some((prefix, structural_type)) = pending.take().filter(|_| capture_address) {
                frontiers.push(Frontier::Address {
                    prefix,
                    structural_type,
                });
            }
            frontiers.push(Frontier::Selector { position });
        }
        match segment {
            PreparedStructuralPathSegment::Static(segment) => {
                type_path.push(segment.clone());
                if *segment == StructuralPathSegment::Referent {
                    let tip = terminal_semantics::runtime_structural_path_tip(
                        types.iter(),
                        root,
                        &type_path,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "completed place lost its dereferenced carrier",
                    ))?;
                    pending = Some((position + 1, tip.id));
                }
            }
            PreparedStructuralPathSegment::EvaluatedFixedIndex { index, .. } => {
                type_path.push(StructuralPathSegment::FixedIndex(*index));
            }
            PreparedStructuralPathSegment::ViewExtent { .. } => {}
            _ => {
                // Coordinates select no storage in this type-only walk. Their
                // actual values and bounds remain with the emitting operation.
                type_path.push(StructuralPathSegment::FixedIndex(0));
            }
        }
    }
    if let Some((prefix, structural_type)) = pending.filter(|_| capture_address) {
        frontiers.push(Frontier::Address {
            prefix,
            structural_type,
        });
    }
    if copy_value {
        let tip = terminal_semantics::runtime_structural_path_tip(types.iter(), root, &type_path)
            .ok_or(LoweringError::Unsupported(
            "completed copy lost its selected carrier",
        ))?;
        frontiers.push(Frontier::Value {
            structural_type: tip.id,
        });
    }
    if frontiers.windows(2).any(|pair| {
        matches!((&pair[0], &pair[1]),
            (Frontier::Address { prefix: first, .. }, Frontier::Address { prefix: second, .. })
                if first > second)
    }) {
        return unsupported("completed place frontiers reverse their authored path");
    }
    Ok(frontiers)
}

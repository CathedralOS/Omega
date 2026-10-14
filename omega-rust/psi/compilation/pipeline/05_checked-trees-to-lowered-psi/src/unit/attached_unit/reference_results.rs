//! Reference carriers retain ingress provenance and the exact source loan end.
//! A consumer borrows the referent; it does not acquire its linear claims or
//! cleanup. A computed backing is established by the ordinary value schedule
//! before this operation; reference-bearing backing keeps its contained loans
//! and normal disposition rather than borrowing a detached owned snapshot.
use super::super::StructuralArgument;
use super::{
    LoweringError, Operation, OperationKind, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, allocate_dense, lookup_type_id, place_id,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, structural_result_declaration};
use checked_trees::CheckedUnitStructuralResultBindingPlan;

/// The checked expression the reference operation actually evaluates.
/// Source coordinates stay in this producer companion, never in Terminal.
pub(crate) struct Formation<'checked> {
    pub(crate) checked: &'checked checked_trees::CheckedTrees,
    pub(crate) state: symbols::SymbolHandle,
    pub(crate) statement_index: usize,
    pub(crate) expression: checked_trees::expression::ExpressionHandle,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PublishedReferenceStorage {
    Declared,
    RepresentationView(terminal_psi::ReferenceStorageView),
    Placed,
}

/// Publish the source-selected storage interpretation once at formation.
/// A projected backing already captures its byte endpoint, so a view's
/// relative offset is zero. Callers that retain a scalar binding namespace
/// keep the returned declaration there; the operation always owns its SSA row.
pub(crate) fn publish_storage(
    storage: checked_trees::CheckedReferenceStorage,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<
    (
        PublishedReferenceStorage,
        Option<terminal_psi::ValueDeclaration>,
    ),
    LoweringError,
> {
    let checked_trees::CheckedReferenceStorage::View { stored_scalar } = storage else {
        return Ok((
            match storage {
                checked_trees::CheckedReferenceStorage::Placed => PublishedReferenceStorage::Placed,
                _ => PublishedReferenceStorage::Declared,
            },
            None,
        ));
    };
    let byte_offset = crate::terminal_identities::value_id(allocate_dense(next_value)?);
    let value = terminal_psi::ValueDeclaration {
        id: byte_offset,
        scalar_type: crate::unit::terminal_scalar_type(crate::unit::PrimitiveType::U64)?,
        qualifications: Default::default(),
    };
    let id = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id,
        result: terminal_psi::OperationResult::Scalar(value.clone()),
        kind: OperationKind::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Unsigned(0),
        },
    });
    Ok((
        PublishedReferenceStorage::RepresentationView(terminal_psi::ReferenceStorageView {
            byte_offset,
            stored_scalar,
        }),
        Some(value),
    ))
}

pub(super) fn emit(
    formation: Formation<'_>,
    result: &CheckedUnitStructuralResultBindingPlan,
    source: StructuralArgument,
    storage: PublishedReferenceStorage,
    types: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let structural_type = lookup_type_id(types, &result.type_identity)?;
    emit_carrier(
        formation,
        structural_type,
        source,
        storage,
        declarations,
        place_id(allocate_dense(next_place)?),
        operations,
    )
}

pub(crate) fn emit_carrier(
    formation: Formation<'_>,
    structural_type: StructuralTypeId,
    source: StructuralArgument,
    storage: PublishedReferenceStorage,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    place: semantic_vocabulary::PlaceId,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let id = operations.allocate();
    // A captured loan keeps the carrier its type declares. Ordinary
    // references own affine lifetime resources; collection views use their
    // descriptor operation and its shared/exclusive multiplicity.
    // Both routes retain the exact evaluated source and loan provenance below.
    let declaration = terminal_psi::find_structural_type(declarations, structural_type)
        .ok_or(LoweringError::Unsupported("loan carrier type is missing"))?;
    let (kind, multiplicity) = match declaration.shape {
        terminal_psi::StructuralTypeShape::Reference { .. } => (
            match storage {
                PublishedReferenceStorage::Placed => OperationKind::EstablishPlacedReference {
                    source: source.clone(),
                },
                PublishedReferenceStorage::Declared => OperationKind::EstablishReference {
                    source: source.clone(),
                    storage: None,
                },
                PublishedReferenceStorage::RepresentationView(view) => {
                    OperationKind::EstablishReference {
                        source: source.clone(),
                        storage: Some(view),
                    }
                }
            },
            StructuralMultiplicity::Affine,
        ),
        terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::byte_sequence_view_carrier_pattern!(),
        )
        | terminal_psi::StructuralTypeShape::ElementView { .. }
            if matches!(storage, PublishedReferenceStorage::Declared) =>
        {
            let element = match declaration.shape {
                terminal_psi::StructuralTypeShape::ElementView { element } => element,
                _ => {
                    let scalar = crate::emission::scalar_types::terminal_scalar_type(
                        checked_trees::types::PrimitiveType::U8,
                    )?;
                    declarations
                        .iter()
                        .find(|declaration| {
                            declaration.scalar_restriction.is_none()
                                && declaration.shape
                                    == terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar)
                        })
                        .ok_or(LoweringError::Unsupported(
                            "byte loan carrier has no u8 declaration",
                        ))?
                        .id
                }
            };
            let region = terminal_psi::ElementViewRegion::WholeCollection;
            match source.access {
                terminal_psi::StructuralAccess::SharedBorrow => (
                    OperationKind::EstablishElementView {
                        destination: place,
                        source: source.clone(),
                        element,
                        region,
                    },
                    StructuralMultiplicity::Unrestricted,
                ),
                terminal_psi::StructuralAccess::MutableBorrow => (
                    OperationKind::EstablishMutableElementView {
                        destination: place,
                        source: source.clone(),
                        element,
                        region,
                    },
                    StructuralMultiplicity::Affine,
                ),
                _ => {
                    return Err(LoweringError::Unsupported(
                        "byte loan carrier has no readable access",
                    ));
                }
            }
        }
        _ => {
            return Err(LoweringError::Unsupported(
                "loan carrier has no formation operation",
            ));
        }
    };
    // The ordinary and constructed-reference routes share this capture. The
    // lowered source is already evaluated, including every selector snapshot.
    // These rows associate checked occurrences with emitted coordinates; they
    // neither replay the borrow judgment nor grant preservation permission.
    let borrow = &formation.checked.facts.borrow;
    if let Some(state) = borrow
        .states
        .iter()
        .find_map(|(_, state)| (state.state_symbol == formation.state).then_some(state))
    {
        for loan in state.loans.handles().filter(|loan| {
            let row = borrow.loans.get(*loan);
            row.statement_index == formation.statement_index
                && row.formation_expression == formation.expression
        }) {
            operations
                .loan_captures
                .push(lowered_psi::LoweredSourceLoanCapture {
                    loan,
                    formation: id,
                    carrier: place,
                    source: source.clone(),
                });
        }
    }
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id,
        result: structural_result_declaration(place, structural_type, multiplicity),
        kind,
    });
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id,
            structural_type,
        },
    })
}

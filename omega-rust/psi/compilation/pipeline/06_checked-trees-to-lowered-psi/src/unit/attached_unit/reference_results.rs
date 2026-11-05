//! Reference carriers retain ingress provenance and the exact source loan end.
//! A consumer borrows the referent; it does not acquire its linear claims or
//! cleanup. A computed backing is established by the ordinary value schedule
//! before this operation; reference-bearing backing keeps its contained loans
//! and normal disposition rather than borrowing a detached owned snapshot.
use super::super::StructuralArgument;
use super::{
    LoweringError, Operation, OperationKind, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralTypeId, allocate_dense, lookup_type_id, place_id,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, structural_result_declaration};
use crate::emission::operation_emission::place_declarations::operation_result_place;
use crate::emission::scalar_types::terminal_scalar_type;
use crate::retention::access_lifecycles::storage_fields;
use crate::shared::terminal_identities::lookup_domain_id_for_carrier;
use crate::shared::terminal_identities::{
    stored_structural_identity, structural_declaration_by_identity,
};
use checked_trees::CheckedUnitStructuralResultBindingPlan;

/// The checked expression the reference operation actually evaluates.
/// Source coordinates stay in this producer companion, never in Terminal.
pub(crate) struct Formation<'checked> {
    pub(crate) checked: &'checked checked_trees::CheckedTrees,
    pub(crate) statement_index: usize,
    pub(crate) expression: checked_trees::expression::ExpressionHandle,
    /// The supplying endpoint, never the receiving reference annotation.
    pub(crate) backing_type: typed_trees::types::TypeReferenceHandle,
    pub(crate) source_contract: &'checked checked_trees::CheckedReferenceSourceContract,
    pub(crate) qualification_decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
    pub(crate) domain_ids:
        &'checked [crate::shared::terminal_identities::StructuralDomainBinding],
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PublishedReferenceStorage {
    Declared,
    RepresentationView(terminal_psi::ReferenceStorageView),
    Access,
    ResidentAccess {
        extent: symbols::SymbolHandle,
        granted: language_semantics::SemanticDomainId,
        resident: language_semantics::SemanticDomainId,
    },
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
                checked_trees::CheckedReferenceStorage::Access => PublishedReferenceStorage::Access,
                checked_trees::CheckedReferenceStorage::ResidentAccess {
                    extent,
                    granted,
                    resident,
                } => PublishedReferenceStorage::ResidentAccess {
                    extent,
                    granted,
                    resident,
                },
                _ => PublishedReferenceStorage::Declared,
            },
            None,
        ));
    };
    let byte_offset = crate::shared::terminal_identities::value_id(allocate_dense(next_value)?);
    let value = terminal_psi::ValueDeclaration {
        id: byte_offset,
        scalar_type: crate::unit::terminal_scalar_type(crate::unit::PrimitiveType::U64)?,
        qualifications: Default::default(),
    };
    let id = operations.allocate();
    operations.push(Operation::unsuspended(
        id,
        terminal_psi::OperationResult::Scalar(value),
        OperationKind::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Unsigned(0),
        },
    ));
    Ok((
        PublishedReferenceStorage::RepresentationView(terminal_psi::ReferenceStorageView {
            byte_offset,
            stored_scalar,
        }),
        Some(value),
    ))
}

pub(super) fn emit(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    formation: Formation<'_>,
    result: &CheckedUnitStructuralResultBindingPlan,
    source: StructuralArgument,
    storage: PublishedReferenceStorage,
    types: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    evaluation: &super::argument_evaluation::Evaluation,
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let structural_type = lookup_type_id(types, &result.type_identity)?;
    emit_carrier(
        identities,
        formation,
        structural_type,
        source,
        storage,
        declarations,
        evaluation,
        place_id(allocate_dense(next_place)?),
        operations,
    )
}

pub(crate) fn emit_carrier(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    formation: Formation<'_>,
    structural_type: StructuralTypeId,
    source: StructuralArgument,
    storage: PublishedReferenceStorage,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    evaluation: &super::argument_evaluation::Evaluation,
    place: semantic_vocabulary::PlaceId,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let id = operations.allocate();
    let parameters = evaluation.structural_root_parameters().collect::<Vec<_>>();
    let resident_source = if let PublishedReferenceStorage::ResidentAccess {
        extent,
        granted,
        resident,
    } = storage
    {
        let (root, _, _, _) = super::structural_values::member_memberships::source_memberships_in(
            source.place,
            parameters.iter().copied(),
            operations.iter().chain(
                evaluation
                    .blocks
                    .iter()
                    .flat_map(|block| block.operations.iter()),
            ),
        )
        .ok_or(LoweringError::Unsupported(
            "resident reference lost its supplying declaration",
        ))?;
        let carrier = terminal_semantics::runtime_structural_path_tip(
            declarations.iter(),
            root,
            &source.path,
        )
        .ok_or(LoweringError::Unsupported(
            "resident reference lost its supplying owner",
        ))?;
        let (base, length) = storage_fields(identities, formation.checked, extent, carrier)?;
        let carrier = carrier.id;
        Some(terminal_psi::ResidentReferenceSource {
            base,
            length,
            granted: lookup_domain_id_for_carrier(formation.domain_ids, granted, carrier)?,
            resident: lookup_domain_id_for_carrier(formation.domain_ids, resident, carrier)?,
        })
    } else {
        None
    };
    // A captured loan keeps the carrier its type declares. Ordinary
    // references own affine lifetime resources; collection views use their
    // descriptor operation and its shared/exclusive multiplicity.
    // Both routes retain the exact evaluated source and loan provenance below.
    let declaration = terminal_psi::find_structural_type(declarations, structural_type)
        .ok_or(LoweringError::Unsupported("loan carrier type is missing"))?;
    let (kind, multiplicity) = match declaration.shape {
        terminal_psi::StructuralTypeShape::Reference { .. } => (
            match storage {
                PublishedReferenceStorage::Access
                | PublishedReferenceStorage::ResidentAccess { .. } => {
                    OperationKind::EstablishAccessReference {
                        source: source.clone(),
                        resident: resident_source,
                    }
                }
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
                terminal_psi::StructuralTypeShape::ElementView { element, .. } => element,
                _ => {
                    let scalar = terminal_scalar_type(checked_trees::types::PrimitiveType::U8)?;
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
    let mut result = structural_result_declaration(place, structural_type, multiplicity);
    if resident_source.is_none()
        && matches!(
            declaration.shape,
            terminal_psi::StructuralTypeShape::Reference { .. }
        )
    {
        // Completed evaluation can move a source producer into an earlier
        // private block. Its emitted declaration remains the membership owner.
        // A descriptor carries the actual source's memberships below its
        // borrowed boundary, never qualifications on the descriptor itself.
        // Constructor transport then prefixes these same rows with its field
        // or element path. A destination annotation cannot supply missing rows.
        let terminal_psi::OperationResult::Structural(result) = &mut result else {
            return Err(LoweringError::Unsupported(
                "reference result is not structural",
            ));
        };
        if let Some((root, whole, projected, additional)) =
            super::structural_values::member_memberships::source_memberships_in(
                source.place,
                parameters.iter().copied(),
                operations.iter().chain(
                    evaluation
                        .blocks
                        .iter()
                        .flat_map(|block| block.operations.iter()),
                ),
            )
        {
            let path = terminal_semantics::reference_source_storage_path(
                declarations.iter(),
                root,
                &source.path,
            );
            let (captured, additional_input) =
                terminal_semantics::reference_carrier_membership_contract(
                    declarations.iter(),
                    root,
                    whole,
                    projected,
                    additional,
                    path,
                    source.access,
                    structural_type,
                    match &storage {
                        PublishedReferenceStorage::RepresentationView(view) => Some(view),
                        _ => None,
                    },
                )
                .map_err(LoweringError::Unsupported)?;
            result.projected_qualifications.extend(captured);
            result.additional_input_qualifications = additional_input;
            if let Some(field) =
                terminal_semantics::runtime_structural_field(declarations.iter(), root, path)
                && field.field_type.byte_sequence_carrier().is_some()
            {
                if !formation.backing_type.is_valid() {
                    return Err(LoweringError::Unsupported(
                        "inline field loan lost its supplying type",
                    ));
                }
                let terminal_psi::StructuralTypeShape::Reference { referent, .. } =
                    declaration.shape
                else {
                    return Err(LoweringError::Unsupported(
                        "field loan lost its reference carrier",
                    ));
                };
                // These are independent judgments on this checked occurrence,
                // separate from the source type's standing obligations above.
                let decisions = formation
                    .checked
                    .facts
                    .qualifications
                    .value_formations
                    .span(formation.qualification_decisions)
                    .ok_or(LoweringError::Unsupported(
                        "field loan membership decisions are stale",
                    ))?;
                for domain in decisions.iter().map(|decision| decision.semantic_domain) {
                    result.projected_qualifications.push(
                        terminal_psi::StructuralPathQualification {
                            path: vec![terminal_psi::StructuralPathSegment::Referent],
                            domain: lookup_domain_id_for_carrier(
                                formation.domain_ids,
                                domain,
                                referent,
                            )?,
                        },
                    );
                }
            }
        }
        if matches!(
            storage,
            PublishedReferenceStorage::Declared | PublishedReferenceStorage::RepresentationView(_)
        ) {
            let required = source_contract_memberships(
                identities,
                &formation,
                declaration,
                source.access,
                storage,
                declarations,
            )?;
            // A fact can also be required by the original storage. Transporting
            // its input origin cannot waive that independently recorded duty.
            result.additional_input_qualifications.retain(|row| {
                !terminal_psi::StructuralPathQualification::select(&[], &required, &row.path)
                    .0
                    .contains(&row.domain)
            });
            result.projected_qualifications.extend(required);
        }
        result.projected_qualifications.sort();
        result.projected_qualifications.dedup();
        result.additional_input_qualifications.sort();
        result.additional_input_qualifications.dedup();
    }
    // The ordinary and constructed-reference routes share this capture. The
    // lowered source is already evaluated, including every selector snapshot.
    // These rows associate checked occurrences with emitted coordinates; they
    // neither replay the borrow judgment nor grant preservation permission.
    for loan in operations
        .reference_formations
        .loans_for(formation.statement_index, formation.expression)
    {
        operations
            .loan_captures
            .push(lowered_psi::LoweredSourceLoanCapture {
                loan,
                formation: id,
                carrier: place,
                source: source.clone(),
            });
    }
    operations.reference_backings.record(place, source);
    operations.push(Operation::unsuspended(id, result, kind));
    Ok(operation_result_place(place, id, structural_type))
}

/// A source contract belongs to the checked value, including when its storage
/// home has no membership rows. Lower it independently of those live rows.
fn source_contract_memberships(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    formation: &Formation<'_>,
    declaration: &terminal_psi::StructuralTypeDeclaration,
    access: terminal_psi::StructuralAccess,
    storage: PublishedReferenceStorage,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<terminal_psi::StructuralPathQualification>, LoweringError> {
    let refuse = |reason: &str| LoweringError::InvalidReferenceContract {
        supplying_type: formation.backing_type,
        reason: reason.to_owned(),
    };
    let contract = match formation.source_contract {
        checked_trees::CheckedReferenceSourceContract::Standing { contract, .. } => contract,
        checked_trees::CheckedReferenceSourceContract::Unavailable(reason) => {
            return Err(refuse(reason));
        }
        checked_trees::CheckedReferenceSourceContract::SeparateStorage => {
            return Err(refuse("pointer formation has no supplying contract"));
        }
    };
    // A proved empty requirement list adds no membership and needs no
    // definition for an otherwise identity-only abstract referent.
    if contract.qualifications.is_empty() && contract.projected_qualifications.is_empty() {
        return Ok(Vec::new());
    }
    if !formation.backing_type.is_valid() {
        return Err(refuse("reference formation lost its supplying type"));
    }
    let supplying = structural_declaration_by_identity(
        declarations,
        identities,
        formation.checked,
        stored_structural_identity(formation.checked, formation.backing_type),
        "reference formation lost its supplying carrier",
    )?;
    let terminal_psi::StructuralTypeShape::Reference { referent, .. } = declaration.shape else {
        return Err(refuse("reference formation lost its reference carrier"));
    };
    if matches!(storage, PublishedReferenceStorage::Declared) && supplying.id != referent {
        return Err(refuse(
            "reference formation source differs from its exact referent",
        ));
    }
    let projected = super::parameters::lower_projected_qualifications(
        identities,
        &contract.projected_qualifications,
        formation.domain_ids,
        supplying.id,
        declarations,
    )?;
    let whole = contract
        .qualifications
        .iter()
        .map(|domain| lookup_domain_id_for_carrier(formation.domain_ids, *domain, supplying.id))
        .collect::<Result<Vec<_>, _>>()?;
    terminal_semantics::reference_carrier_memberships(
        declarations.iter(),
        supplying.id,
        &whole,
        &projected,
        &[],
        access,
        declaration.id,
        match &storage {
            PublishedReferenceStorage::RepresentationView(view) => Some(view),
            _ => None,
        },
    )
    .map_err(refuse)
}

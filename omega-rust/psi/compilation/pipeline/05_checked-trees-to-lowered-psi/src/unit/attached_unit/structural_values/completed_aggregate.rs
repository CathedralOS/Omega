//! Complete and publish one aggregate establishment. Record, case and array
//! literals complete their members through one operation: every structural
//! member transports the memberships of its own completed source under that
//! member's declared path, and one operation commits the result. Member
//! geometry — declaration-ordered fields, optionally under a selected case
//! payload, or fixed-index elements — is the whole difference, so a receiving
//! annotation never supplies a predicate.
//! `retention::copied_memberships::construction_memberships` reconstructs
//! these same rows from the committed operation.

use super::super::super::StructuralFieldType;
use super::super::{
    CheckedScalarExpressionRole, LoweringError, OperationKind, OperationResult, PlaceId,
    StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId,
    unsupported,
};
use super::member_memberships;
use crate::emission::operation_emission::buffer::{OperationBuffer, structural_result_declaration};
use terminal_psi::{
    RecordFieldInitializer, RecordFieldValue, StructuralArgument, StructuralFieldDeclaration,
    StructuralParameterDeclaration, StructuralPathQualification, StructuralPathSegment,
};

/// One completed aggregate's members, before its establishing operation
/// commits them: declaration-ordered fields over their dense roster, or
/// index-ordered elements.
pub(in crate::unit::attached_unit) enum CompletedAggregate<'a> {
    Record {
        fields: Vec<RecordFieldInitializer>,
        declarations: &'a [StructuralFieldDeclaration],
    },
    Case {
        result_case: semantic_vocabulary::StructuralCaseId,
        fields: Vec<RecordFieldInitializer>,
        declarations: &'a [StructuralFieldDeclaration],
        /// The first payload member: common members precede it.
        common_fields: usize,
    },
    Array {
        elements: Vec<StructuralArgument>,
    },
}

/// Publish the completed aggregate's member memberships and its establishing
/// operation. Only a member roster that disagrees with the establishment's own
/// declarations is refused: the emitting frame produced these completed
/// operands, and no receiver annotation replaces a missing predicate.
pub(in crate::unit::attached_unit) fn emit_completed<'parameter>(
    place: PlaceId,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    aggregate: CompletedAggregate<'_>,
    parameters: impl Iterator<Item = &'parameter StructuralParameterDeclaration> + Clone,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let mut rows = Vec::new();
    let kind = match aggregate {
        CompletedAggregate::Record {
            fields,
            declarations,
        } => {
            append_fields(
                &fields,
                declarations,
                None,
                parameters,
                operations,
                &mut rows,
            )?;
            OperationKind::EstablishRecord { fields }
        }
        CompletedAggregate::Case {
            result_case,
            fields,
            declarations,
            common_fields,
        } => {
            append_fields(
                &fields,
                declarations,
                Some((result_case, common_fields)),
                parameters,
                operations,
                &mut rows,
            )?;
            OperationKind::EstablishStructuralCase {
                result_case,
                fields,
            }
        }
        CompletedAggregate::Array { elements } => {
            append_elements(&elements, parameters, operations, &mut rows)?;
            OperationKind::EstablishStructuralArray { elements }
        }
    };
    rows.sort();
    rows.dedup();
    declare_establishment(place, structural_type, multiplicity, rows, kind, operations)
}

/// Declare and push one establishment's result: the single operation owning
/// the fresh place, with the memberships its completed members transported.
fn declare_establishment(
    place: PlaceId,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    projected_qualifications: Vec<StructuralPathQualification>,
    kind: OperationKind,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let mut result = structural_result_declaration(place, structural_type, multiplicity);
    if let OperationResult::Structural(result) = &mut result {
        result.projected_qualifications = projected_qualifications;
    }
    let operation = operations.push_leaf(result, kind);
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation,
            structural_type,
        },
    })
}

/// Complete every field member's memberships under its declared field path,
/// beneath the selected case's label from `payload` on.
fn append_fields<'parameter>(
    fields: &[RecordFieldInitializer],
    declarations: &[StructuralFieldDeclaration],
    payload: Option<(semantic_vocabulary::StructuralCaseId, usize)>,
    parameters: impl Iterator<Item = &'parameter StructuralParameterDeclaration> + Clone,
    operations: &OperationBuffer,
    destination: &mut Vec<StructuralPathQualification>,
) -> Result<(), LoweringError> {
    let declarations = declarations
        .iter()
        .filter(|declaration| !declaration.relevance.is_erased())
        .collect::<Vec<_>>();
    if declarations.len() != fields.len() {
        return unsupported("completed aggregate member has no declaration");
    }
    for (position, (field, declaration)) in fields.iter().zip(&declarations).enumerate() {
        // A member whose initializer names a different declaration has no
        // declaration at this roster position.
        if field.field != declaration.id {
            return unsupported("completed aggregate member has no declaration");
        }
        let RecordFieldValue::Structural(argument) = &field.value else {
            continue;
        };
        // Inline byte fields carry their predicates in the field contract;
        // projected qualifications describe nominal structural children.
        if !matches!(declaration.field_type, StructuralFieldType::Structural(_)) {
            continue;
        }
        let mut prefix = Vec::new();
        if let Some((selected_case, common_fields)) = payload
            && position >= common_fields
        {
            prefix.push(StructuralPathSegment::Case(selected_case));
        }
        prefix.push(StructuralPathSegment::Field(declaration.identity.clone()));
        member_memberships::append(
            argument,
            &prefix,
            parameters.clone(),
            operations,
            destination,
        );
    }
    Ok(())
}

/// Complete every element member's memberships under its fixed index.
fn append_elements<'parameter>(
    elements: &[StructuralArgument],
    parameters: impl Iterator<Item = &'parameter StructuralParameterDeclaration> + Clone,
    operations: &OperationBuffer,
    destination: &mut Vec<StructuralPathQualification>,
) -> Result<(), LoweringError> {
    for (ordinal, argument) in elements.iter().enumerate() {
        let index = u64::try_from(ordinal).map_err(|_| {
            LoweringError::Unsupported("aggregate element ordinal exceeds its carrier")
        })?;
        let prefix = [StructuralPathSegment::FixedIndex(index)];
        member_memberships::append(
            argument,
            &prefix,
            parameters.clone(),
            operations,
            destination,
        );
    }
    Ok(())
}

impl super::emission::Emission<'_, '_, '_> {
    /// Complete one aggregate member's initializer from its checked operand.
    /// Scalar leaves replay their retained computation, zeroed scalars take
    /// the declared carrier's zero, and structural operands complete as child
    /// establishments under the member's own path — beneath the selected
    /// case's label for payload members — with owned member claims rebased
    /// under that same path.
    pub(super) fn completed_member_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        field: &checked_trees::CheckedStructuralRecordField,
        target: &StructuralFieldDeclaration,
        role: CheckedScalarExpressionRole,
        selected_case: Option<semantic_vocabulary::StructuralCaseId>,
        scalar_positions: &mut Vec<(usize, usize)>,
        field_index: usize,
        source_count: usize,
        claims: &mut Vec<terminal_psi::StructuralResultClaimBinding>,
    ) -> Result<RecordFieldValue, LoweringError> {
        use checked_trees::CheckedStructuralRecordFieldValue;
        Ok(match field.value {
            CheckedStructuralRecordFieldValue::Scalar(value) => {
                let evaluated = self.scalar(identities, role, value, source_count)?;
                self.scalar_field_value(
                    identities,
                    target,
                    evaluated,
                    scalar_positions,
                    field_index,
                )?
            }
            CheckedStructuralRecordFieldValue::Structural(value) => {
                if let Some(scalar_type) = target.field_type.scalar_type() {
                    self.zeroed_scalar_field(
                        identities,
                        value,
                        scalar_type,
                        target,
                        scalar_positions,
                        field_index,
                    )?
                } else {
                    let mut prefix = Vec::new();
                    if let Some(selected_case) = selected_case {
                        prefix.push(StructuralPathSegment::Case(selected_case));
                    }
                    prefix.push(StructuralPathSegment::Field(target.identity.clone()));
                    let child_type = super::field_carrier::resolve(
                        identities,
                        self.checked,
                        self.structural_types,
                        &target.field_type,
                        field.type_reference,
                    )?;
                    let completed = self.child_constructor_operand(
                        identities,
                        child_type,
                        value,
                        field.type_reference,
                        &prefix,
                    )?;
                    if self.multiplicity == StructuralMultiplicity::Affine {
                        for claim in completed.claims {
                            let mut path = prefix.clone();
                            path.extend(claim.path);
                            claims.push(terminal_psi::StructuralResultClaimBinding {
                                claim: claim.claim,
                                path,
                            });
                        }
                    }
                    RecordFieldValue::Structural(completed.argument)
                }
            }
        })
    }
}

//! Checked structural type plans as Terminal structural type declarations,
//! shared by every plan family that declares structural storage.

use std::collections::BTreeSet;

use checked_trees::CheckedUnitStructuralFieldType;
use checked_trees::types::PrimitiveType;
use semantic_vocabulary::{IeeeFloatFormat, StructuralCaseId, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralCaseDeclaration, StructuralFieldDeclaration,
    StructuralFieldType, StructuralTypeDeclaration,
};

use crate::emission::scalar_types::terminal_scalar_type;
use crate::expression_preparation::bindings::structural_paths::terminal_access;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{allocate_dense, lookup_type_id, structural_field_id};

mod scalar_refinements;

pub(crate) fn terminal_structural_field_type(
    primitive: PrimitiveType,
) -> Result<StructuralFieldType, LoweringError> {
    Ok(match primitive {
        PrimitiveType::F32 => StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32),
        PrimitiveType::F64 => StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64),
        primitive => StructuralFieldType::Scalar(terminal_scalar_type(primitive)?),
    })
}

pub(crate) fn terminal_byte_sequence_carrier(
    carrier: checked_trees::CheckedByteSequenceCarrier,
) -> ByteSequenceCarrier {
    match carrier {
        // The access crosses with the carrier. Collapsing it here would hand
        // Terminal a shared view for every authored `&mut [u8]`, which is what
        // left the verifier unable to check a byte store's authority.
        checked_trees::CheckedByteSequenceCarrier::BorrowedView { access } => {
            ByteSequenceCarrier::BorrowedView {
                access: access.map(terminal_access),
            }
        }
        checked_trees::CheckedByteSequenceCarrier::BoundedView { access, capacity } => {
            ByteSequenceCarrier::BoundedView {
                access: access.map(terminal_access),
                capacity,
            }
        }
        checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity } => {
            ByteSequenceCarrier::BoundedOwned { capacity }
        }
    }
}

/// Preserve one field contract across records, case payloads and retained types.
/// Atomic access authority belongs to the declaration, never to a caller shape.
pub(crate) fn lower_field_type(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    field_type: &CheckedUnitStructuralFieldType,
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
) -> Result<StructuralFieldType, LoweringError> {
    Ok(match field_type {
        CheckedUnitStructuralFieldType::BoundedInteger(integer) => {
            StructuralFieldType::BoundedInteger(*integer)
        }
        CheckedUnitStructuralFieldType::RefinedScalar {
            primitive,
            predicate,
        } => StructuralFieldType::RefinedScalar(scalar_refinements::lower(
            identities, *primitive, predicate,
        )?),
        CheckedUnitStructuralFieldType::AtomicBoundedInteger(integer) => {
            StructuralFieldType::AtomicBoundedInteger(*integer)
        }
        CheckedUnitStructuralFieldType::Atomic(primitive) => {
            StructuralFieldType::Atomic(terminal_scalar_type(*primitive)?)
        }
        CheckedUnitStructuralFieldType::Scalar(primitive) => {
            terminal_structural_field_type(*primitive)?
        }
        CheckedUnitStructuralFieldType::PredicateByteSequence {
            carrier,
            predicates,
        } => StructuralFieldType::PredicateByteSequence {
            carrier: terminal_byte_sequence_carrier(*carrier),
            predicates: *predicates,
        },
        CheckedUnitStructuralFieldType::ByteSequence(carrier) => {
            StructuralFieldType::ByteSequence(terminal_byte_sequence_carrier(*carrier))
        }
        CheckedUnitStructuralFieldType::Structural { type_identity } => {
            StructuralFieldType::Structural(lookup_type_id(type_ids, type_identity)?)
        }
        CheckedUnitStructuralFieldType::ProviderBacked { .. }
        | CheckedUnitStructuralFieldType::BindingBacked { .. } => {
            return unsupported("provider-backed fields require attached provider erasure");
        }
        CheckedUnitStructuralFieldType::Erased { type_identity } => StructuralFieldType::Erased {
            type_identity: identities.intern(
                &checked
                    .type_identity_text(*type_identity)
                    .unwrap_or_default(),
            ),
        },
    })
}

pub(crate) fn lower_mixed_fields(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    fields: &[checked_trees::CheckedUnitStructuralFieldPlan],
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    next_field: &mut u64,
) -> Result<Vec<StructuralFieldDeclaration>, LoweringError> {
    let mut field_identities = BTreeSet::new();
    fields
        .iter()
        .map(|field| {
            if !field.identity.is_valid() || !field_identities.insert(field.identity.clone()) {
                return unsupported("mixed structural type has duplicate field identities");
            }
            let field_type = lower_field_type(identities, checked, &field.field_type, type_ids)?;
            Ok(StructuralFieldDeclaration {
                storage: field.storage.clone(),
                id: structural_field_id(allocate_dense(next_field)?),
                identity: identities.intern(&field.identity.spelling()),
                relevance: field.relevance,
                field_type,
            })
        })
        .collect()
}

pub(crate) fn lower_mixed_cases(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    cases: &[checked_trees::CheckedUnitStructuralCasePlan],
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    next_field: &mut u64,
    next_case: &mut u64,
) -> Result<Vec<StructuralCaseDeclaration>, LoweringError> {
    let mut case_identities = BTreeSet::new();
    cases
        .iter()
        .map(|case| {
            if !case.identity.is_valid() || !case_identities.insert(case.identity.clone()) {
                return unsupported("mixed structural type has duplicate case identities");
            }
            Ok(StructuralCaseDeclaration {
                id: StructuralCaseId::new(allocate_dense(next_case)?)
                    .expect("allocated structural case identity is nonzero"),
                identity: identities.intern(&case.identity.spelling()),
                fields: lower_mixed_fields(
                    identities,
                    checked,
                    &case.fields,
                    type_ids,
                    next_field,
                )?,
            })
        })
        .collect()
}

pub(crate) fn unique_declaration<'a>(
    types: &'a [StructuralTypeDeclaration],
    identity: StructuralTypeId,
    absent: &'static str,
    ambiguous: &'static str,
) -> Result<&'a StructuralTypeDeclaration, LoweringError> {
    let mut matching = types
        .iter()
        .filter(|declaration| declaration.id == identity);
    let declaration = matching.next().ok_or(LoweringError::Unsupported(absent))?;
    if matching.next().is_some() {
        return unsupported(ambiguous);
    }
    Ok(declaration)
}

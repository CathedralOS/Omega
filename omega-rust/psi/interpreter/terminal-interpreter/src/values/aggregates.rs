//! Typed leaf paths for ordinary scalar record payloads. The declaration owns
//! every containing record, including erased and empty children; bytes or a
//! flattened offset cannot substitute for that semantic graph. This traversal
//! grants no qualification, reference, claim or storage identity.

use super::TerminalScalarAggregateValue;
use crate::errors::{TerminalInterpretError, malformed};
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, StructuralTypeId};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralRecordStorage, StructuralTypeDeclaration,
    StructuralTypeShape,
};

enum Visit<'declaration> {
    Type(&'declaration StructuralTypeDeclaration),
    Field(&'declaration StructuralFieldDeclaration),
    LeaveType,
    LeaveField,
}

/// Visit the complete ordered runtime scalar roster of a finite record tree.
/// Arrays, references and case-bearing descendants keep their separate codecs.
pub(crate) fn record_scalar_fields<'declaration>(
    declaration: &'declaration StructuralTypeDeclaration,
    lookup: impl Fn(StructuralTypeId) -> Option<&'declaration StructuralTypeDeclaration>,
    mut scalar: impl FnMut(
        &[Path],
        &terminal_psi::StructuralFieldType,
    ) -> Result<(), TerminalInterpretError>,
) -> Result<(), TerminalInterpretError> {
    let mut pending = vec![Visit::Type(declaration)];
    let mut active = Vec::new();
    let mut path = Vec::new();
    while let Some(visit) = pending.pop() {
        match visit {
            Visit::Type(declaration) => {
                if declaration.quotient_carrier.is_some() || active.contains(&declaration.id) {
                    return Err(malformed());
                }
                let StructuralTypeShape::Record { fields, storage } = &declaration.shape else {
                    return Err(malformed());
                };
                if matches!(
                    storage,
                    StructuralRecordStorage::PlacedView(_)
                        | StructuralRecordStorage::PlacedAccessor(_)
                ) {
                    return Err(malformed());
                }
                active.push(declaration.id);
                pending.push(Visit::LeaveType);
                pending.extend(
                    fields
                        .iter()
                        .rev()
                        .filter(|field| !field.relevance.is_erased())
                        .map(Visit::Field),
                );
            }
            Visit::Field(field) => {
                path.push(Path::Field(field.id));
                pending.push(Visit::LeaveField);
                if field.field_type.scalar_type().is_some() {
                    scalar(&path, &field.field_type)?;
                } else if let terminal_psi::StructuralFieldType::Structural(child) =
                    field.field_type
                {
                    let declaration = lookup(child)
                        .filter(|declaration| declaration.id == child)
                        .ok_or_else(malformed)?;
                    pending.push(Visit::Type(declaration));
                } else {
                    return Err(malformed());
                }
            }
            Visit::LeaveType => {
                active.pop().ok_or_else(malformed)?;
            }
            Visit::LeaveField => {
                path.pop().ok_or_else(malformed)?;
            }
        }
    }
    Ok(())
}

impl TerminalScalarAggregateValue {
    pub(crate) fn validate_against<'declaration>(
        &self,
        declaration: &'declaration StructuralTypeDeclaration,
        lookup: impl Fn(StructuralTypeId) -> Option<&'declaration StructuralTypeDeclaration>,
    ) -> Result<(), TerminalInterpretError> {
        if declaration.id != self.structural_type || declaration.quotient_carrier.is_some() {
            return Err(malformed());
        }
        let mut actual = self.fields.iter();
        let mut accept = |path: &[Path], field_type: &terminal_psi::StructuralFieldType| {
            let (found, value) = actual.next().ok_or_else(malformed)?;
            if found != path
                || field_type.scalar_type() != Some(value.scalar_type())
                || !value.satisfies_field(field_type)
            {
                return Err(malformed());
            }
            Ok(())
        };
        match (&declaration.shape, self.result_case) {
            (StructuralTypeShape::Record { .. }, None) => {
                record_scalar_fields(declaration, lookup, &mut accept)?
            }
            (StructuralTypeShape::Sum { cases }, Some(selected)) => {
                let case =
                    terminal_psi::find_structural_case(cases, selected).ok_or_else(malformed)?;
                for field in &case.fields {
                    if field.relevance.is_erased() || field.field_type.scalar_type().is_none() {
                        return Err(malformed());
                    }
                    accept(&[Path::Field(field.id)], &field.field_type)?;
                }
            }
            _ => return Err(malformed()),
        }
        if actual.next().is_some() {
            return Err(malformed());
        }
        Ok(())
    }
}

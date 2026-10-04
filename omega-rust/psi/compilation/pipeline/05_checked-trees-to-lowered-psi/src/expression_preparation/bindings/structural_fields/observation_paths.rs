//! Make implicit source dereferences explicit at the observation boundary.
//!
//! The source path identifies fields and selectors. Declared reference carriers
//! supply each stored-pointer crossing; live loan custody remains an independent
//! receiving check. Stores retain their own explicit, exclusive projection walk.

use super::{
    CheckedUnitStructuralPathSegment, LoweringError, StructuralAccess, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, unsupported,
};

pub(super) fn readable_carrier(
    mut selected: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Result<(&StructuralTypeDeclaration, usize), LoweringError> {
    let mut references = Vec::new();
    loop {
        let mut matching = declarations.iter().filter(|row| row.id == selected);
        let (Some(declaration), None) = (matching.next(), matching.next()) else {
            return unsupported("field observation requires one carrier declaration");
        };
        let StructuralTypeShape::Reference { referent, access } = &declaration.shape else {
            return Ok((declaration, references.len()));
        };
        if !matches!(
            access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) || references.contains(&selected)
        {
            return unsupported("field observation crosses an unreadable or cyclic reference");
        }
        references.push(selected);
        selected = *referent;
    }
}

pub(super) fn explicit_path<'a>(
    structural_type: StructuralTypeId,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    declarations: &'a [StructuralTypeDeclaration],
) -> Result<
    (
        Vec<CheckedUnitStructuralPathSegment>,
        Option<&'a StructuralTypeDeclaration>,
    ),
    LoweringError,
> {
    use checked_trees::CheckedStructuralPredicatePathSegment as Source;
    let mut selected = Some(structural_type);
    let mut result = Vec::with_capacity(path.len());
    for segment in path {
        let (declaration, crossings) = readable_carrier(
            selected.ok_or(LoweringError::Unsupported(
                "field observation descends below a scalar",
            ))?,
            declarations,
        )?;
        result.extend(std::iter::repeat_n(
            CheckedUnitStructuralPathSegment::Referent,
            crossings,
        ));
        selected = match (segment, &declaration.shape) {
            (
                Source::Field(identity),
                StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. },
            ) => {
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let (Some(field), None) = (matching.next(), matching.next()) else {
                    return unsupported("field observation requires one declared field");
                };
                if field.relevance.is_erased() {
                    return unsupported("field observation cannot read an erased field");
                }
                result.push(CheckedUnitStructuralPathSegment::Field(identity.clone()));
                match &field.field_type {
                    StructuralFieldType::Structural(child) => Some(*child),
                    _ => None,
                }
            }
            (Source::FixedIndex(index), StructuralTypeShape::FixedArray { element, length })
                if index < length =>
            {
                result.push(CheckedUnitStructuralPathSegment::FixedIndex(*index));
                Some(*element)
            }
            (Source::RuntimeIndex(index), StructuralTypeShape::FixedArray { element, .. }) => {
                result.push(CheckedUnitStructuralPathSegment::RuntimeIndex(
                    index.clone(),
                ));
                Some(*element)
            }
            _ => return unsupported("field observation has an incompatible declared projection"),
        };
    }
    let declaration = match selected {
        Some(selected) => {
            let (declaration, crossings) = readable_carrier(selected, declarations)?;
            result.extend(std::iter::repeat_n(
                CheckedUnitStructuralPathSegment::Referent,
                crossings,
            ));
            Some(declaration)
        }
        None => None,
    };
    Ok((result, declaration))
}

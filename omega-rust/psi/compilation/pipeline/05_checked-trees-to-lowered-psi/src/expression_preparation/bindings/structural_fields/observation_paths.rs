//! Make implicit source dereferences explicit at the observation boundary.
//!
//! The source path identifies fields and selectors. Declared reference carriers
//! supply each stored-pointer crossing; live loan custody remains an independent
//! receiving check. Stores retain their own explicit, exclusive projection walk.

use super::{
    CheckedUnitStructuralPathSegment, LoweringError, ScalarType, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, unsupported,
};
use semantic_vocabulary::CanonicalStructuralPathSegment;

/// One declared projection supplies physical geometry and its scalar endpoint.
/// Canonical field identity exists only for inline storage: pointer loads and
/// descriptor indexing instead require executable reads with their live bounds.
pub(super) struct ObservationPath<'declaration> {
    pub(super) segments: Vec<CheckedUnitStructuralPathSegment>,
    pub(super) declaration: Option<&'declaration StructuralTypeDeclaration>,
    pub(super) scalar_field: Option<&'declaration terminal_psi::StructuralFieldDeclaration>,
    pub(super) canonical: Option<Vec<CanonicalStructuralPathSegment>>,
}

impl ObservationPath<'_> {
    pub(super) fn scalar_type(&self) -> Option<ScalarType> {
        if let Some(field) = self.scalar_field {
            return field.field_type.scalar_type();
        }
        match &self.declaration?.shape {
            StructuralTypeShape::PrimitiveScalar(scalar_type) => Some(*scalar_type),
            _ => None,
        }
    }
}

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
        if !matches!(access, terminal_psi::reading_borrow_pair_pattern!())
            || references.contains(&selected)
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
) -> Result<ObservationPath<'a>, LoweringError> {
    use checked_trees::CheckedStructuralPredicatePathSegment as Source;
    let mut selected = Some(structural_type);
    let mut selected_case_fields: Option<&[terminal_psi::StructuralFieldDeclaration]> = None;
    let mut result = Vec::with_capacity(path.len());
    let mut canonical = Some(Vec::with_capacity(path.len()));
    let mut scalar_field = None;
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
        if crossings != 0 {
            canonical = None;
        }
        selected = match (segment, &declaration.shape) {
            (
                Source::Case(identity),
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. },
            ) if selected_case_fields.is_none() => {
                let mut matching = cases.iter().filter(|case| case.identity == *identity);
                let (Some(case), None) = (matching.next(), matching.next()) else {
                    return unsupported("field observation requires one declared case");
                };
                selected_case_fields = Some(&case.fields);
                result.push(CheckedUnitStructuralPathSegment::Case(identity.clone()));
                if let Some(canonical) = &mut canonical {
                    canonical.push(CanonicalStructuralPathSegment::Case(case.id));
                }
                Some(declaration.id)
            }
            (Source::Field(identity), _) => {
                let fields = selected_case_fields
                    .take()
                    .or_else(|| declaration.shape.direct_fields())
                    .ok_or(LoweringError::Unsupported(
                        "field observation requires a field carrier",
                    ))?;
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let (Some(field), None) = (matching.next(), matching.next()) else {
                    return unsupported("field observation requires one declared field");
                };
                if field.relevance.is_erased() {
                    return unsupported("field observation cannot read an erased field");
                }
                result.push(CheckedUnitStructuralPathSegment::Field(identity.clone()));
                if let Some(canonical) = &mut canonical {
                    canonical.push(CanonicalStructuralPathSegment::Field(field.id));
                }
                match &field.field_type {
                    StructuralFieldType::Structural(child) => Some(*child),
                    _ => {
                        scalar_field = Some(field);
                        None
                    }
                }
            }
            (Source::FixedIndex(index), StructuralTypeShape::FixedArray { element, length })
                if index < length =>
            {
                result.push(CheckedUnitStructuralPathSegment::FixedIndex(*index));
                if let Some(canonical) = &mut canonical {
                    canonical.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                }
                Some(*element)
            }
            (Source::FixedIndex(index), StructuralTypeShape::ElementView { element, .. }) => {
                // The declaration selects the element type, not a live bound.
                // Preparation retains its descriptor count and the read owes
                // a dominating index obligation at the concrete operation.
                result.push(CheckedUnitStructuralPathSegment::FixedIndex(*index));
                canonical = None;
                Some(*element)
            }
            (
                Source::RuntimeIndex(index),
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element, .. },
            ) => {
                result.push(CheckedUnitStructuralPathSegment::RuntimeIndex(
                    index.clone(),
                ));
                canonical = None;
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
            if crossings != 0 {
                canonical = None;
            }
            Some(declaration)
        }
        None => None,
    };
    Ok(ObservationPath {
        segments: result,
        declaration,
        scalar_field,
        canonical,
    })
}

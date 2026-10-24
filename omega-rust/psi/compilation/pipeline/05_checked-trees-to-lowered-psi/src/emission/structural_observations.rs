//! Make implicit source dereferences explicit at the observation boundary.
//!
//! The source path identifies fields and selectors. Declared reference carriers
//! supply each stored-pointer crossing; live loan custody remains an independent
//! receiving check. Stores retain their own explicit, exclusive projection walk.

use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedUnitStructuralPathSegment;
use semantic_vocabulary::CanonicalStructuralPathSegment;
use semantic_vocabulary::{ScalarReadProjection, ScalarType, StructuralTypeId};
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};

/// One declared projection supplies physical geometry and its scalar endpoint.
/// Canonical field identity exists only for inline storage: pointer loads and
/// descriptor indexing instead require executable reads with their live bounds.
pub(crate) struct ObservationPath<'declaration> {
    pub(crate) segments: Vec<CheckedUnitStructuralPathSegment>,
    pub(crate) declaration: Option<&'declaration StructuralTypeDeclaration>,
    pub(crate) scalar_field: Option<&'declaration terminal_psi::StructuralFieldDeclaration>,
    pub(crate) canonical: Option<Vec<CanonicalStructuralPathSegment>>,
    /// Static selectors plus explicit pointer crossings. Runtime selectors
    /// need their separately evaluated scalar namespace before forming a term.
    pub(crate) observation: Option<Vec<ScalarReadProjection>>,
}

impl ObservationPath<'_> {
    pub(crate) fn scalar_type(&self) -> Option<ScalarType> {
        if let Some(field) = self.scalar_field {
            return field.field_type.scalar_type();
        }
        match &self.declaration?.shape {
            StructuralTypeShape::PrimitiveScalar(scalar_type) => Some(*scalar_type),
            _ => None,
        }
    }
}

pub(crate) fn readable_carrier(
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

pub(crate) fn explicit_path<'a>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    structural_type: StructuralTypeId,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    declarations: &'a [StructuralTypeDeclaration],
) -> Result<ObservationPath<'a>, LoweringError> {
    use checked_trees::CheckedStructuralPredicatePathSegment as Source;
    let mut selected = Some(structural_type);
    let mut selected_case_fields: Option<&[terminal_psi::StructuralFieldDeclaration]> = None;
    let mut result = Vec::with_capacity(path.len());
    let mut canonical = Some(Vec::with_capacity(path.len()));
    let mut observation = Some(Vec::with_capacity(path.len()));
    let mut scalar_field: Option<&terminal_psi::StructuralFieldDeclaration> = None;
    for segment in path {
        let (declaration, crossings) = match selected {
            Some(selected) => {
                let (declaration, crossings) = readable_carrier(selected, declarations)?;
                (Some(declaration), crossings)
            }
            None => (None, 0),
        };
        result.extend(std::iter::repeat_n(
            CheckedUnitStructuralPathSegment::Referent,
            crossings,
        ));
        if crossings != 0 {
            canonical = None;
            if let Some(observation) = &mut observation {
                observation.extend(std::iter::repeat_n(
                    ScalarReadProjection::Referent,
                    crossings,
                ));
            }
        }
        // Inline byte fields and declared byte descriptors have the same
        // element geometry. Selecting a byte still owes its own live extent
        // observation and index obligation when the prepared path is emitted.
        let byte_carrier = declaration
            .and_then(|declaration| match declaration.shape {
                StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                _ => None,
            })
            .or_else(|| scalar_field.and_then(|field| field.field_type.byte_sequence_carrier()));
        if let Some(carrier) = byte_carrier {
            match segment {
                Source::FixedIndex(index) => {
                    if matches!(
                        carrier,
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }
                            | terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. }
                            if *index >= capacity
                    ) {
                        return unsupported("byte observation index exceeds its storage capacity");
                    }
                    result.push(CheckedUnitStructuralPathSegment::FixedIndex(*index));
                    if let Some(observation) = &mut observation {
                        observation.push(ScalarReadProjection::Static(
                            CanonicalStructuralPathSegment::FixedIndex(*index),
                        ));
                    }
                }
                Source::RuntimeIndex(index) => {
                    observation = None;
                    result.push(CheckedUnitStructuralPathSegment::RuntimeIndex(
                        index.clone(),
                    ));
                }
                _ => return unsupported("byte observation requires an element selector"),
            }
            let scalar_type = crate::emission::scalar_types::terminal_scalar_type(
                typed_trees::types::PrimitiveType::U8,
            )?;
            let element = StructuralFieldType::Scalar(scalar_type)
                .leaf_declaration(declarations.iter())
                .ok_or(LoweringError::Unsupported(
                    "byte observation has no unique retained element declaration",
                ))?;
            selected = Some(element.id);
            scalar_field = None;
            canonical = None;
            continue;
        }
        let declaration = declaration.ok_or(LoweringError::Unsupported(
            "field observation descends below a scalar",
        ))?;
        selected = match (segment, &declaration.shape) {
            (
                Source::Case(identity),
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. },
            ) if selected_case_fields.is_none() => {
                let mut matching = cases
                    .iter()
                    .filter(|case| case.identity == identities.intern(&identity.spelling()));
                let (Some(case), None) = (matching.next(), matching.next()) else {
                    return unsupported("field observation requires one declared case");
                };
                selected_case_fields = Some(&case.fields);
                result.push(CheckedUnitStructuralPathSegment::Case(identity.clone()));
                if let Some(observation) = &mut observation {
                    observation.push(ScalarReadProjection::Static(
                        CanonicalStructuralPathSegment::Case(case.id),
                    ));
                }
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
                let mut matching = fields
                    .iter()
                    .filter(|field| field.identity == identities.intern(&identity.spelling()));
                let (Some(field), None) = (matching.next(), matching.next()) else {
                    return unsupported("field observation requires one declared field");
                };
                if field.relevance.is_erased() {
                    return unsupported("field observation cannot read an erased field");
                }
                result.push(CheckedUnitStructuralPathSegment::Field(identity.clone()));
                if let Some(observation) = &mut observation {
                    observation.push(ScalarReadProjection::Static(
                        CanonicalStructuralPathSegment::Field(field.id),
                    ));
                }
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
                if let Some(observation) = &mut observation {
                    observation.push(ScalarReadProjection::Static(
                        CanonicalStructuralPathSegment::FixedIndex(*index),
                    ));
                }
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
                if let Some(observation) = &mut observation {
                    observation.push(ScalarReadProjection::Static(
                        CanonicalStructuralPathSegment::FixedIndex(*index),
                    ));
                }
                canonical = None;
                Some(*element)
            }
            (
                Source::RuntimeIndex(index),
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element, .. },
            ) => {
                observation = None;
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
                if let Some(observation) = &mut observation {
                    observation.extend(std::iter::repeat_n(
                        ScalarReadProjection::Referent,
                        crossings,
                    ));
                }
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
        observation,
    })
}

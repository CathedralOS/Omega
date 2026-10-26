//! Make implicit source dereferences explicit at the observation boundary.
//!
//! The source path identifies fields and selectors. Declared reference carriers
//! supply each stored-pointer crossing. Executable observations require readable
//! edges; declared predicate coordinates require only the exact typed path and
//! grant no observation or custody. All purposes use one geometry walk. Stores
//! retain their own explicit, exclusive projection walk.

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
    /// Static observation identity keeps pointer crossings explicit rather than
    /// pretending they are inline offsets. Runtime selectors require their own
    /// occurrence-bound terms and cannot become static canonical segments.
    pub(crate) fn static_observation_path(&self) -> Option<Vec<CanonicalStructuralPathSegment>> {
        self.observation
            .as_ref()?
            .iter()
            .map(|projection| match projection {
                ScalarReadProjection::Referent => Some(CanonicalStructuralPathSegment::Referent),
                ScalarReadProjection::Static(segment) => Some(*segment),
                ScalarReadProjection::Index(_) | ScalarReadProjection::Extent => None,
            })
            .collect()
    }

    pub(crate) fn field_type(&self) -> Result<StructuralFieldType, LoweringError> {
        if let Some(field) = self.scalar_field {
            return Ok(field.field_type.clone());
        }
        let declaration = self.declaration.ok_or(LoweringError::Unsupported(
            "structural observation has no declared endpoint",
        ))?;
        Ok(match declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => StructuralFieldType::Scalar(scalar),
            _ => StructuralFieldType::Structural(declaration.id),
        })
    }

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

fn carrier_declaration(
    selected: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Result<&StructuralTypeDeclaration, LoweringError> {
    let mut matching = declarations.iter().filter(|row| row.id == selected);
    let (Some(declaration), None) = (matching.next(), matching.next()) else {
        return unsupported("field observation requires one carrier declaration");
    };
    Ok(declaration)
}

pub(crate) fn readable_carrier(
    mut selected: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Result<(&StructuralTypeDeclaration, usize), LoweringError> {
    let mut references = Vec::new();
    loop {
        let declaration = carrier_declaration(selected, declarations)?;
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

/// A declared predicate coordinate is metadata, not a readable projection.
/// It deliberately exposes no executable segments or observation expression.
pub(crate) struct DeclaredPredicatePath {
    pub(crate) path: Vec<CanonicalStructuralPathSegment>,
    pub(crate) field_type: StructuralFieldType,
}

#[derive(Clone, Copy)]
enum PathPurpose {
    ExecutableObservation,
    StoredValue,
    DeclaredPredicate,
}

/// Actual observation preparation retains strict readable reference edges.
pub(crate) fn explicit_path<'a>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    structural_type: StructuralTypeId,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    declarations: &'a [StructuralTypeDeclaration],
) -> Result<ObservationPath<'a>, LoweringError> {
    resolve_path(
        identities,
        structural_type,
        path,
        declarations,
        PathPurpose::ExecutableObservation,
    )
}

/// Copy the selected stored value, including a reference descriptor endpoint.
/// Intermediate references are actual readable loads, just as for an ordinary
/// observation. The final descriptor is the value being copied, not another
/// implicit crossing. Static coordinates cannot manufacture selector evidence.
pub(crate) fn stored_value_path<'a>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    declarations: &'a [StructuralTypeDeclaration],
) -> Result<ObservationPath<'a>, LoweringError> {
    use checked_trees::CheckedStructuralPredicatePathSegment as Source;
    let path = path
        .iter()
        .map(|segment| match segment {
            CheckedUnitStructuralPathSegment::Referent => Ok(Source::Referent),
            CheckedUnitStructuralPathSegment::Case(identity) => Ok(Source::Case(identity.clone())),
            CheckedUnitStructuralPathSegment::Field(identity) => {
                Ok(Source::Field(identity.clone()))
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => Ok(Source::FixedIndex(*index)),
            _ => unsupported("stored value path requires static declared coordinates"),
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    resolve_path(
        identities,
        structural_type,
        &path,
        declarations,
        PathPurpose::StoredValue,
    )
}

/// Retain explicit declaration coordinates without observing their contents.
/// Any scalar/read consumer still owns its separate access and custody checks.
pub(crate) fn declared_predicate_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    structural_type: StructuralTypeId,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Result<DeclaredPredicatePath, LoweringError> {
    let selected = resolve_path(
        identities,
        structural_type,
        path,
        declarations,
        PathPurpose::DeclaredPredicate,
    )?;
    let path = selected
        .static_observation_path()
        .ok_or(LoweringError::Unsupported(
            "declared predicate path requires occurrence-bound selector evidence",
        ))?;
    Ok(DeclaredPredicatePath {
        path,
        field_type: selected.field_type()?,
    })
}

fn resolve_path<'a>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    structural_type: StructuralTypeId,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    declarations: &'a [StructuralTypeDeclaration],
    purpose: PathPurpose,
) -> Result<ObservationPath<'a>, LoweringError> {
    use checked_trees::CheckedStructuralPredicatePathSegment as Source;
    let mut selected = Some(structural_type);
    let mut selected_case_fields: Option<&[terminal_psi::StructuralFieldDeclaration]> = None;
    let mut result = Vec::with_capacity(path.len());
    let mut canonical = Some(Vec::with_capacity(path.len()));
    let mut observation = Some(Vec::with_capacity(path.len()));
    let mut scalar_field: Option<&terminal_psi::StructuralFieldDeclaration> = None;
    for segment in path {
        // An explicit checked step consumes one edge before any implicit
        // source-member traversal. Its suffix must not be crossed twice.
        if matches!(segment, Source::Referent) {
            if selected_case_fields.is_some() {
                return unsupported("reference observation has an unfinished case path");
            }
            let receiver = selected.ok_or(LoweringError::Unsupported(
                "reference observation has no structural receiver",
            ))?;
            let declaration = carrier_declaration(receiver, declarations)?;
            let StructuralTypeShape::Reference { referent, access } = declaration.shape else {
                return unsupported("reference observation requires a reference carrier");
            };
            if !matches!(purpose, PathPurpose::DeclaredPredicate)
                && !matches!(access, terminal_psi::reading_borrow_pair_pattern!())
            {
                return unsupported("reference observation requires readable access");
            }
            result.push(CheckedUnitStructuralPathSegment::Referent);
            canonical = None;
            if let Some(observation) = &mut observation {
                observation.push(ScalarReadProjection::Referent);
            }
            selected = Some(referent);
            scalar_field = None;
            continue;
        }
        let (declaration, crossings) = match selected {
            Some(selected) => {
                let (declaration, crossings) = match purpose {
                    PathPurpose::ExecutableObservation | PathPurpose::StoredValue => {
                        readable_carrier(selected, declarations)?
                    }
                    PathPurpose::DeclaredPredicate => {
                        (carrier_declaration(selected, declarations)?, 0)
                    }
                };
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
    if selected_case_fields.is_some() {
        return unsupported("field observation case selection has no payload field");
    }
    let declaration = match selected {
        Some(selected) => {
            let (declaration, crossings) = if !matches!(purpose, PathPurpose::ExecutableObservation)
                || matches!(path.last(), Some(Source::Referent))
            {
                (carrier_declaration(selected, declarations)?, 0)
            } else {
                readable_carrier(selected, declarations)?
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

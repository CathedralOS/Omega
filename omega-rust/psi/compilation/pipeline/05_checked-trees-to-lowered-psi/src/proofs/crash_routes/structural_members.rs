//! Structural member paths, terms, float and byte-sequence fields and sum
//! subjects.

use crate::proofs::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatFormat,
    IeeeFloatStructuralField, LoweringError, PlaceId, ScalarTerm, ScalarType,
    StructuralCaseSubject, StructuralFieldType, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, unsupported,
};

/// Resolve type meaning, not access or bounds evidence. Fixed arrays own a
/// static extent; views keep their independently admitted live-length
/// obligation while continuing through the same element/member namespace.
fn indexed_element_type(
    shape: &StructuralTypeShape,
    element_index: u64,
) -> Result<StructuralTypeId, LoweringError> {
    match shape {
        StructuralTypeShape::FixedArray { element, length } => {
            if element_index >= *length {
                return unsupported("structural contract fixed index is out of bounds");
            }
            Ok(*element)
        }
        StructuralTypeShape::ElementView { element, .. } => Ok(*element),
        _ => unsupported("structural contract index receiver is not a collection"),
    }
}

pub(crate) fn lower_structural_member_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    parameter_position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<
    (
        PlaceId,
        Vec<CanonicalStructuralPathSegment>,
        StructuralFieldType,
    ),
    LoweringError,
> {
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == parameter_position)
        .ok_or(LoweringError::Unsupported(
            "structural scalar contract names a non-structural parameter",
        ))?;
    lower_structural_place_path(
        identities,
        parameter.place,
        parameter.structural_type,
        parameter.access,
        path,
        structural_types,
    )
}

/// Resolve a declaration-owned observation independently of whether its root
/// is an invocation formal or a normal result. Result storage is not a formal:
/// it keeps its own type and place without borrowed-parameter interpretation.
fn lower_structural_place_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    root: PlaceId,
    root_type: StructuralTypeId,
    access: terminal_psi::StructuralAccess,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<
    (
        PlaceId,
        Vec<CanonicalStructuralPathSegment>,
        StructuralFieldType,
    ),
    LoweringError,
> {
    // Preserve the existing outer scalar-root policy. Resolving a stored
    // predicate coordinate below it does not itself observe an inner referent.
    if access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return unsupported("structural scalar contract requires readable access");
    }
    let selected = crate::emission::structural_observations::declared_predicate_path(
        identities,
        root_type,
        path,
        structural_types,
    )?;
    Ok((root, selected.path, selected.field_type))
}

fn structural_observation_path<'declaration>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    root_type: StructuralTypeId,
    access: terminal_psi::StructuralAccess,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    structural_types: &'declaration [StructuralTypeDeclaration],
) -> Result<crate::emission::structural_observations::ObservationPath<'declaration>, LoweringError>
{
    if access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return unsupported("structural scalar contract requires readable access");
    }
    crate::emission::structural_observations::explicit_path(
        identities,
        root_type,
        path,
        structural_types,
    )
}

/// A descriptor extent retains the full structural path. Carrier checking
/// distinguishes element counts from byte storage; neither field names nor
/// an equally numbered scalar parameter supply descriptor authority.
pub(crate) fn lower_sequence_extent_term(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    parameter_position: u32,
    path: &[checked_trees::CheckedScalarReadProjection],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    lower_selector: &mut impl FnMut(
        &mut terminal_psi::TerminalIdentityTableBuilder,
        &checked_trees::CheckedScalarExpression,
    ) -> Result<ScalarTerm, LoweringError>,
) -> Result<ScalarTerm, LoweringError> {
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == parameter_position)
        .ok_or(LoweringError::Unsupported(
            "closed sequence extent names a non-structural parameter",
        ))?;
    if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return unsupported("closed sequence extent requires readable access");
    }
    // Inline coordinates and executable crossings come from one declaration
    // walk. An extent behind a reference is an observation at that exact
    // descriptor, not an inline field or an inferred capacity.
    let (carrier, mut observation) = lower_checked_observation_path(
        identities,
        parameter,
        path,
        structural_types,
        lower_selector,
    )?;
    let carrier = match carrier {
        terminal_psi::StructuralFieldType::Structural(identity) => {
            let declaration = terminal_psi::find_structural_type(structural_types, identity)
                .ok_or(LoweringError::Unsupported(
                    "closed sequence extent type is absent",
                ))?;
            match &declaration.shape {
                terminal_psi::StructuralTypeShape::ElementView { .. } => None,
                terminal_psi::StructuralTypeShape::ByteSequence(carrier) => Some(*carrier),
                _ => return unsupported("closed sequence extent leaf is not a descriptor"),
            }
        }
        leaf => Some(
            leaf.byte_sequence_carrier()
                .ok_or(LoweringError::Unsupported(
                    "closed sequence extent leaf is not a descriptor",
                ))?,
        ),
    };
    if matches!(
        carrier,
        Some(
            terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::WriteOnlyBorrow),
            } | terminal_psi::ByteSequenceCarrier::BoundedView {
                access: Some(terminal_psi::StructuralAccess::WriteOnlyBorrow),
                ..
            }
        )
    ) {
        return unsupported("closed sequence extent requires readable access");
    }
    let integer = crate::emission::scalar_types::terminal_scalar_type(
        checked_trees::types::PrimitiveType::U64,
    )?
    .as_integer()
    .ok_or(LoweringError::Unsupported(
        "sequence extent is not an integer",
    ))?;
    let canonical = observation
        .iter()
        .map(|projection| match projection {
            semantic_vocabulary::ScalarReadProjection::Static(segment) => Some(segment.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>();
    if let Some(path) = canonical {
        return Ok(ScalarTerm::view_extent(parameter.place, path, integer));
    }
    observation.push(semantic_vocabulary::ScalarReadProjection::Extent);
    ScalarTerm::structural_read(parameter.place, observation, ScalarType::Integer(integer))
        .map_err(LoweringError::InvalidCrashPredicate)
}

/// Indexed observations retain each selector in the contract's scalar
/// namespace. Static runs use the same declared-field resolver as ordinary
/// member contracts; a dynamic step changes only collection selection.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_structural_indexed_term(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    expression: &checked_trees::CheckedScalarExpression,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    lower_selector: &mut impl FnMut(
        &mut terminal_psi::TerminalIdentityTableBuilder,
        &checked_trees::CheckedScalarExpression,
    ) -> Result<ScalarTerm, LoweringError>,
) -> Result<ScalarTerm, LoweringError> {
    use checked_trees::CheckedScalarReadProjection;
    use semantic_vocabulary::ScalarReadProjection;
    let checked_trees::CheckedScalarExpression::StructuralParameterIndexedRead {
        root,
        path,
        index,
        element_path,
        primitive_type,
        ..
    } = expression
    else {
        return unsupported("indexed contract observation has no selected read");
    };
    let position = root.parameter().ok_or(LoweringError::Unsupported(
        "indexed contract observation requires a structural formal",
    ))?;
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == position)
        .ok_or(LoweringError::Unsupported(
            "indexed contract formal is absent",
        ))?;
    if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return unsupported("indexed contract observation requires readable access");
    }
    let mut projections = path
        .iter()
        .cloned()
        .map(CheckedScalarReadProjection::Static)
        .collect::<Vec<_>>();
    projections.push(CheckedScalarReadProjection::RuntimeIndex((**index).clone()));
    projections.extend_from_slice(element_path);
    let (_, terminal_path) = lower_checked_observation_path(
        identities,
        parameter,
        &projections,
        structural_types,
        lower_selector,
    )?;
    let actual = terminal_semantics::structural_read_scalar_type(
        structural_types.iter(),
        parameter.structural_type,
        &terminal_path,
    );
    let expected = crate::emission::scalar_types::terminal_scalar_type(*primitive_type)?;
    if actual != Some(expected) {
        return unsupported("indexed contract path does not select its retained scalar type");
    }
    // A closed selection is the same declared observation as a member path,
    // not a dynamic-read-shaped term that makes its pure domain opaque.
    let fixed = terminal_path
        .iter()
        .map(|step| match step {
            ScalarReadProjection::Static(step) => Some(step.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>();
    let term = match (fixed, expected) {
        (Some(path), ScalarType::Integer(integer)) => {
            ScalarTerm::integer_field_path(parameter.place, path, integer)
        }
        (Some(path), ScalarType::Boolean) => ScalarTerm::boolean_field_path(parameter.place, path),
        (Some(path), ScalarType::IeeeFloat(format)) => {
            ScalarTerm::ieee_float_field_path(parameter.place, path, format)
        }
        (None, _) => ScalarTerm::StructuralRead {
            root: parameter.place,
            path: terminal_path,
            scalar_type: expected,
        },
    };
    term.validate()
        .map_err(LoweringError::InvalidCrashPredicate)?;
    Ok(term)
}

/// Checked selectors keep their exact scalar namespace alongside the declared
/// observation walk. Scalar leaves and descriptor metadata share this owner;
/// neither may replace a selected descriptor by its unindexed container.
fn lower_checked_observation_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    parameter: &StructuralParameterDeclaration,
    projections: &[checked_trees::CheckedScalarReadProjection],
    structural_types: &[StructuralTypeDeclaration],
    lower_selector: &mut impl FnMut(
        &mut terminal_psi::TerminalIdentityTableBuilder,
        &checked_trees::CheckedScalarExpression,
    ) -> Result<ScalarTerm, LoweringError>,
) -> Result<
    (
        StructuralFieldType,
        Vec<semantic_vocabulary::ScalarReadProjection>,
    ),
    LoweringError,
> {
    use checked_trees::CheckedScalarReadProjection;
    use semantic_vocabulary::ScalarReadProjection;
    if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return unsupported("contract observation requires readable access");
    }
    let mut terminal_path = Vec::new();
    let mut selected = StructuralFieldType::Structural(parameter.structural_type);
    if projections.is_empty() {
        let observation = structural_observation_path(
            identities,
            parameter.structural_type,
            parameter.access,
            &[],
            structural_types,
        )?;
        selected = observation.field_type()?;
        terminal_path.extend(observation.observation.ok_or(LoweringError::Unsupported(
            "contract observation has no retained empty path",
        ))?);
    }
    let mut cursor = 0;
    while cursor < projections.len() {
        if matches!(&projections[cursor], CheckedScalarReadProjection::Static(_)) {
            let mut static_path = Vec::new();
            while let Some(CheckedScalarReadProjection::Static(segment)) = projections.get(cursor) {
                static_path.push(segment.clone());
                cursor += 1;
            }
            let StructuralFieldType::Structural(structural_type) = selected else {
                return unsupported("indexed contract member follows a scalar leaf");
            };
            let observation = structural_observation_path(
                identities,
                structural_type,
                parameter.access,
                &static_path,
                structural_types,
            )?;
            selected = observation.field_type()?;
            terminal_path.extend(observation.observation.ok_or(LoweringError::Unsupported(
                "indexed contract member has no retained observation path",
            ))?);
            continue;
        }
        let CheckedScalarReadProjection::RuntimeIndex(selector) = &projections[cursor] else {
            unreachable!();
        };
        if let StructuralFieldType::Structural(identity) = selected {
            let observation = structural_observation_path(
                identities,
                identity,
                parameter.access,
                &[],
                structural_types,
            )?;
            selected = observation.field_type()?;
            terminal_path.extend(observation.observation.ok_or(LoweringError::Unsupported(
                "contract collection has no retained observation path",
            ))?);
        }
        let selector = lower_selector(identities, &selector.expression)?;
        if !matches!(selector.scalar_type(), ScalarType::Integer(integer) if !integer.is_address())
        {
            return unsupported("indexed contract selector is not a fixed integer");
        }
        let fixed = match &selector {
            ScalarTerm::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                ..
            } => Some(u64::try_from(*value).map_err(|_| {
                LoweringError::Unsupported("indexed contract selector is outside the count range")
            })?),
            ScalarTerm::Integer {
                value: semantic_vocabulary::IntegerValue::Signed(value),
                ..
            } => Some(u64::try_from(*value).map_err(|_| {
                LoweringError::Unsupported("indexed contract selector is outside the count range")
            })?),
            _ => None,
        };
        let byte_carrier = match &selected {
            StructuralFieldType::Structural(identity) => {
                let declaration = terminal_psi::find_structural_type(structural_types, *identity)
                    .ok_or(LoweringError::Unsupported(
                    "indexed contract collection type is absent",
                ))?;
                match &declaration.shape {
                    StructuralTypeShape::FixedArray { element, .. } => {
                        let element = match fixed {
                            Some(index) => indexed_element_type(&declaration.shape, index)?,
                            None => *element,
                        };
                        selected = StructuralFieldType::Structural(element);
                        None
                    }
                    StructuralTypeShape::ElementView { element, access } => {
                        if *access == Some(terminal_psi::StructuralAccess::WriteOnlyBorrow) {
                            return unsupported(
                                "indexed contract view observation requires readable access",
                            );
                        }
                        selected = StructuralFieldType::Structural(*element);
                        None
                    }
                    StructuralTypeShape::ByteSequence(carrier) => Some(*carrier),
                    _ => return unsupported("indexed contract receiver is not a collection"),
                }
            }
            field => Some(
                field
                    .byte_sequence_carrier()
                    .ok_or(LoweringError::Unsupported(
                        "indexed contract receiver is not a collection",
                    ))?,
            ),
        };
        if let Some(carrier) = byte_carrier {
            if matches!((carrier, fixed),
                (terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }
                | terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. }, Some(index))
                    if index >= capacity
            ) {
                return unsupported("indexed contract byte index exceeds its storage capacity");
            }
            if matches!(
                carrier,
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(terminal_psi::StructuralAccess::WriteOnlyBorrow),
                } | terminal_psi::ByteSequenceCarrier::BoundedView {
                    access: Some(terminal_psi::StructuralAccess::WriteOnlyBorrow),
                    ..
                }
            ) {
                return unsupported("indexed contract byte observation requires readable access");
            }
            selected =
                StructuralFieldType::Scalar(crate::emission::scalar_types::terminal_scalar_type(
                    checked_trees::types::PrimitiveType::U8,
                )?);
        }
        terminal_path.push(match fixed {
            Some(index) => {
                ScalarReadProjection::Static(CanonicalStructuralPathSegment::FixedIndex(index))
            }
            None => ScalarReadProjection::Index(Box::new(selector)),
        });
        cursor += 1;
    }
    Ok((selected, terminal_path))
}

pub(crate) fn lower_structural_member_term(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    parameter_position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    expected: ScalarType,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == parameter_position)
        .ok_or(LoweringError::Unsupported(
            "structural contract has no formal",
        ))?;
    lower_observation_term(
        identities,
        parameter.place,
        parameter.structural_type,
        parameter.access,
        path,
        expected,
        structural_types,
    )
}

/// Observe a scalar beneath the actual normal result declaration. It shares
/// path/type checking with formals but cannot acquire a formal's loan access.
pub(crate) fn lower_structural_result_member_term(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    result: &terminal_psi::StructuralResultDeclaration,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    expected: ScalarType,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    lower_observation_term(
        identities,
        result.place,
        result.structural_type,
        terminal_psi::StructuralAccess::Owned,
        path,
        expected,
        structural_types,
    )
}

fn lower_observation_term(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    root: PlaceId,
    root_type: StructuralTypeId,
    access: terminal_psi::StructuralAccess,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    expected: ScalarType,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    let selected =
        structural_observation_path(identities, root_type, access, path, structural_types)?;
    let actual = selected.field_type()?;
    if let Some(canonical) = selected.canonical {
        return structural_member_term(root, canonical, actual, expected);
    }
    if actual.scalar_type() != Some(expected) {
        return unsupported("structural observation contract has a different scalar carrier");
    }
    let path = selected.observation.ok_or(LoweringError::Unsupported(
        "structural observation contract requires evaluated selector terms",
    ))?;
    ScalarTerm::structural_read(root, path, expected).map_err(LoweringError::InvalidCrashPredicate)
}

fn structural_member_term(
    root: PlaceId,
    terminal_path: Vec<CanonicalStructuralPathSegment>,
    actual: StructuralFieldType,
    expected: ScalarType,
) -> Result<ScalarTerm, LoweringError> {
    if actual.scalar_type() != Some(expected) {
        return unsupported(
            "structural scalar contract path does not end at the retained scalar type",
        );
    }
    Ok(match expected {
        ScalarType::Boolean => ScalarTerm::boolean_field_path(root, terminal_path),
        ScalarType::Integer(integer_type) => {
            ScalarTerm::integer_field_path(root, terminal_path, integer_type)
        }
        ScalarType::IeeeFloat(format) => {
            ScalarTerm::ieee_float_field_path(root, terminal_path, format)
        }
    })
}

pub(crate) fn lower_ieee_float_field(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    field: &checked_trees::CheckedStructuralParameterField,
    format: IeeeFloatFormat,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<IeeeFloatStructuralField, LoweringError> {
    let (root, path, actual) = lower_structural_member_path(
        identities,
        field.parameter_position,
        &field.path,
        parameters,
        structural_types,
    )?;
    if actual != StructuralFieldType::IeeeFloat(format) {
        return unsupported("structural IEEE predicate leaf has the wrong retained format");
    }
    IeeeFloatStructuralField::new(root, path).map_err(LoweringError::InvalidCrashPredicate)
}

/// Declaration and call contracts observe the same typed byte storage.
pub(crate) fn lower_byte_predicate_term(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    predicate: semantic_vocabulary::ByteSequencePredicate,
    root: checked_trees::CheckedStorageRoot,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    let position = root.parameter().ok_or(LoweringError::Unsupported(
        "closed byte predicate requires a structural formal",
    ))?;
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == position)
        .ok_or(LoweringError::Unsupported(
            "byte predicate names a non-structural formal",
        ))?;
    let fixed_bytes = |identity| {
        terminal_psi::find_structural_type(structural_types, identity).is_some_and(|row| {
            matches!(
                row.shape,
                terminal_psi::StructuralTypeShape::FixedArray { .. }
            ) && terminal_semantics::fixed_byte_extent(structural_types.iter(), identity).is_some()
        })
    };
    let (root, path, carrier, inline_fixed) = if path.is_empty() {
        let carrier =
            terminal_psi::find_structural_type(structural_types, parameter.structural_type)
                .and_then(|declaration| match declaration.shape {
                    terminal_psi::StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                    _ => None,
                });
        (
            parameter.place,
            Vec::new(),
            carrier,
            fixed_bytes(parameter.structural_type),
        )
    } else {
        let (root, path, field) = crate::proofs::crash_routes::lower_structural_member_path(
            identities,
            position,
            path,
            parameters,
            structural_types,
        )?;
        (
            root,
            path,
            field.byte_sequence_carrier(),
            field.as_structural().is_some_and(fixed_bytes),
        )
    };
    let readable = inline_fixed
        || match carrier {
            Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }) => true,
            Some(
                terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
            ) => {
                matches!(
                    access.unwrap_or(parameter.access),
                    terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::MutableBorrow
                )
            }
            _ => false,
        };
    if !readable || parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return unsupported("byte predicate formal is not a readable byte carrier");
    }
    Ok(ScalarTerm::ByteSequencePredicate {
        predicate,
        root,
        path,
    })
}

pub(crate) fn lower_byte_sequence_field(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    field: &checked_trees::CheckedStructuralParameterField,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ByteSequenceStructuralField, LoweringError> {
    let (root, path, actual) = lower_structural_member_path(
        identities,
        field.parameter_position,
        &field.path,
        parameters,
        structural_types,
    )?;
    // Content predicates constrain the bytes, not their equality carrier.
    if actual.byte_sequence_carrier().is_none() {
        return unsupported("structural byte-sequence predicate leaf has the wrong retained type");
    }
    ByteSequenceStructuralField::new(root, path).map_err(LoweringError::InvalidCrashPredicate)
}

fn lower_sum_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    root: PlaceId,
    structural_type: StructuralTypeId,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<(StructuralCaseSubject, StructuralTypeId), LoweringError> {
    let selected = crate::emission::structural_observations::declared_predicate_path(
        identities,
        structural_type,
        path,
        structural_types,
    )?;
    let StructuralFieldType::Structural(structural_type) = selected.field_type else {
        return unsupported("structural sum predicate path does not reach a structural value");
    };
    if !matches!(
        terminal_psi::find_structural_type(structural_types, structural_type)
            .map(|declaration| &declaration.shape),
        Some(terminal_psi::sum_or_mixed_shape_pattern!())
    ) {
        return unsupported("structural sum predicate subject is not a sum");
    }
    Ok((
        StructuralCaseSubject::new(root, selected.path),
        structural_type,
    ))
}

/// Preserve an exact selected case without asserting any payload equation.
/// Entry requirements, normal guarantees and crash predicates share one path
/// resolver. Only guarantees supply a declared result; the result is not an
/// entry parameter and cannot lend a coincidentally equal parameter ordinal.
pub(crate) fn lower_structural_case_membership(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    subject: &checked_trees::CheckedStructuralCaseSubject,
    case_identity: &checked_trees::CheckedMemberIdentity,
    parameters: &[StructuralParameterDeclaration],
    result: Option<&terminal_psi::StructuralResultDeclaration>,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<semantic_vocabulary::Proposition, LoweringError> {
    let (root, structural_type) = match subject.root {
        checked_trees::CheckedStructuralCaseRoot::Parameter { position } => {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.position == position)
                .ok_or(LoweringError::Unsupported(
                    "case membership parameter is absent",
                ))?;
            (parameter.place, parameter.structural_type)
        }
        checked_trees::CheckedStructuralCaseRoot::Result => {
            let result = result.ok_or(LoweringError::Unsupported(
                "case membership result is outside the contract namespace",
            ))?;
            (result.place, result.structural_type)
        }
    };
    let (subject, structural_type) = lower_sum_path(
        identities,
        root,
        structural_type,
        &subject.path,
        structural_types,
    )?;
    let declaration = terminal_psi::find_structural_type(structural_types, structural_type).ok_or(
        LoweringError::Unsupported("case membership declaration is absent"),
    )?;
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &declaration.shape
    else {
        return unsupported("case membership declaration is not a sum");
    };
    let mut matching = cases
        .iter()
        .filter(|case| case.identity == identities.intern(&case_identity.spelling()));
    let case = matching
        .next()
        .ok_or(LoweringError::Unsupported("case membership was redirected"))?;
    if matching.next().is_some() {
        return unsupported("case membership identity is ambiguous");
    }
    Ok(semantic_vocabulary::Proposition::StructuralCaseMembership {
        subject,
        case: case.id,
    })
}

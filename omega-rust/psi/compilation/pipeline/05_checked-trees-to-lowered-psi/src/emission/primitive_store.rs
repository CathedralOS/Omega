//! Primitive-reference store emission shared by Unit and scalar-result bodies.
//! A runtime element's bound travels from range checking through the shared
//! destination path (`runtime_elements.rs`), as for every other store route.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, CheckedUnitStructuralPathSegment, LoweringError,
    OperationKind, PlaceId, PrimitiveType, ScalarType, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeId,
    StructuralTypeShape, ValueDeclaration, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::runtime_elements::ProjectionStep;
use crate::structural_types::unique_declaration;
use terminal_psi::live_field;

/// One receiving judgment in its declaration's original coordinates.
pub(crate) struct ReceivingPlace {
    pub(crate) root_type: StructuralTypeId,
    pub(crate) qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    pub(crate) projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
    pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
}

/// The resolved physical endpoint and, after descriptor capture, the original
/// logical receiving judgment. Both consume the same completed authored RHS.
pub(crate) struct Destination {
    pub(crate) logical_receiving: Option<ReceivingPlace>,
    pub(crate) root_type: StructuralTypeId,
    pub(crate) qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    pub(crate) projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<ProjectionStep>,
    pub(crate) scalar_type: ScalarType,
}

pub(crate) fn parameter_destination(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    parameter: &StructuralParameterDeclaration,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<Destination, LoweringError> {
    let writable = matches!(
        parameter.access,
        terminal_psi::exclusive_borrow_pair_pattern!()
    ) || (!path.is_empty() && parameter.access == super::StructuralAccess::Owned);
    if !writable
        || parameter.multiplicity == StructuralMultiplicity::Linear
        || (path.is_empty() && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
    {
        return unsupported("primitive store parameter lost its exclusive custody");
    }
    let (path, scalar_type) =
        lower_destination_path(identities, parameter.structural_type, path, types)?;
    Ok(Destination {
        logical_receiving: None,
        root_type: parameter.structural_type,
        qualifications: parameter.qualifications.clone(),
        projected_qualifications: parameter.projected_qualifications.clone(),
        place: parameter.place,
        path,
        scalar_type,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_assignment(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    destination: Destination,
    structural_types: &[StructuralTypeDeclaration],
    preserves_byte_memberships: bool,
    receiving_range: Option<&checked_trees::CheckedPrimitiveReceivingRange>,
    role: CheckedScalarExpressionRole,
    value: &checked_trees::CheckedScalarStoreValue,
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    source_value_count: usize,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
) -> Result<OperationKind, LoweringError> {
    // Capture the exact call-result slot before selector evaluation creates
    // private staging slots. Selector continuations preserve this scalar
    // prefix; read its rebound declaration after the destination is captured.
    let call_result =
        if let checked_trees::CheckedScalarStoreValue::ScalarResult { position } = value {
            let position = usize::try_from(*position).map_err(|_| {
                LoweringError::Unsupported("primitive store call-result position exceeds usize")
            })?;
            if position.checked_add(1) != Some(values.len())
                || values[position].scalar_type != destination.scalar_type
            {
                return unsupported("primitive store lost its exact established call result");
            }
            Some(position)
        } else {
            None
        };
    // The runtime elements' selectors are evaluated before the stored value.
    let indexes = super::runtime_elements::evaluate_selectors(
        identities,
        &destination.path,
        checked,
        machine,
        state,
        statement_index,
        evaluation,
        source_value_count,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
        calls,
    )?;
    let value = if let Some(position) = call_result {
        let value = *values.get(position).ok_or(LoweringError::Unsupported(
            "primitive store lost its call-result slot after selector evaluation",
        ))?;
        if value.scalar_type != destination.scalar_type {
            return unsupported(
                "primitive store call-result carrier changed after selector evaluation",
            );
        }
        value
    } else {
        let value = match value {
            checked_trees::CheckedScalarStoreValue::Pure(expression) => {
                checked_trees::CheckedCallScalarArgument::Pure(expression.clone())
            }
            checked_trees::CheckedScalarStoreValue::Computation(root) => {
                checked_trees::CheckedCallScalarArgument::Computation(*root)
            }
            checked_trees::CheckedScalarStoreValue::ScalarResult { .. } => {
                unreachable!("captured call result")
            }
        };
        evaluation.source_value(
            identities,
            checked,
            machine,
            state,
            statement_index,
            role,
            &value,
            source_value_count,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )?
    };
    let path =
        super::runtime_elements::complete_path(destination.path, indexes, calls, operations)?;
    let physical = ReceivingPlace {
        root_type: destination.root_type,
        qualifications: destination.qualifications,
        projected_qualifications: destination.projected_qualifications,
        path: path.clone(),
    };
    let logical = destination.logical_receiving.as_ref().unwrap_or(&physical);
    let logical_scalar = terminal_semantics::primitive_projection_restriction(
        calls.structural_types.iter(),
        logical.root_type,
        &logical.path,
    )
    .cloned();
    let logical_restriction = receiving_restriction(logical, calls)?;
    if logical_restriction != logical_scalar && !preserves_byte_memberships {
        return unsupported("primitive write lost its exact logical byte preservation judgment");
    }
    if let Some(StructuralFieldType::BoundedInteger(integer)) = &logical_scalar {
        if receiving_range.is_none_or(|range| range.integer != *integer) {
            return unsupported("primitive store lost its logical receiving-range judgment");
        }
    }
    // The source store plan retains the completed RHS and occurrence. Its
    // integer decision was authenticated by OperationFrame before emission;
    // byte preservation is the exact checked target's independent judgment.
    // Physical goals use only the physical declaration and full root roster.
    let restriction = receiving_restriction(&physical, calls)?;
    if terminal_semantics::primitive_projection_type(
        calls.structural_types.iter(),
        logical.root_type,
        &logical.path,
    ) != Some(destination.scalar_type)
        || terminal_semantics::primitive_projection_type(
            calls.structural_types.iter(),
            physical.root_type,
            &physical.path,
        ) != Some(destination.scalar_type)
    {
        return unsupported("captured primitive destination changed its scalar endpoint");
    }
    let logical_proven = receiving_range
        .map(|range| StructuralFieldType::BoundedInteger(range.integer))
        .or_else(|| {
            preserves_byte_memberships
                .then(|| logical_restriction.clone())
                .flatten()
        });
    let range_obligation = if let Some(restriction) = &restriction {
        let obligation =
            super::obligation_id(super::allocate_dense(&mut calls.next_obligation_identity)?);
        // A logical proof can discharge a physical interval only by actual
        // numeric containment. A different physical goal remains owed to the
        // normal complete source-check cut; a boolean cannot invent its proof.
        if logical_proven
            .as_ref()
            .is_some_and(|proven| receiving_entails(proven, restriction))
        {
            operations.source_checked_obligations.push(obligation);
        }
        Some(obligation)
    } else {
        None
    };

    super::runtime_elements::observe_element_extent(
        destination.place,
        destination.root_type,
        &path,
        structural_types,
        next_value,
        operations,
    )?;
    Ok(OperationKind::WriteOnlyPrimitiveStore {
        range_obligation,
        destination: destination.place,
        path,
        value: value.id,
    })
}

fn receiving_restriction(
    place: &ReceivingPlace,
    calls: &CallEmissionContext<'_>,
) -> Result<Option<StructuralFieldType>, LoweringError> {
    terminal_semantics::primitive_store_restriction_with_predicates(
        calls.structural_types.iter(),
        place.root_type,
        &place.qualifications,
        &place.projected_qualifications,
        &place.path,
        |carrier, domain| {
            let binding = calls
                .domain_ids
                .iter()
                .find(|row| row.carrier == carrier && row.domain == domain)?;
            let mut predicates = terminal_psi::ByteSequencePredicateSet::EMPTY;
            predicates.insert(binding.byte_predicate?);
            Some(predicates)
        },
    )
    .ok_or(LoweringError::Unsupported(
        "primitive write does not preserve enclosing qualifications",
    ))
}

fn receiving_entails(proven: &StructuralFieldType, required: &StructuralFieldType) -> bool {
    match (proven, required) {
        (
            StructuralFieldType::BoundedInteger(proven),
            StructuralFieldType::BoundedInteger(required),
        ) => {
            proven.integer_type() == required.integer_type()
                && required.minimum() <= proven.minimum()
                && proven.maximum() <= required.maximum()
        }
        _ => false,
    }
}

/// Resolve a destination's complete storage projection against the emitted
/// declarations. Unlike a scalar-field operation this endpoint is the
/// primitive itself, so an array element needs no synthetic field identity.
/// Fields, literal elements, and the assignment's runtime elements compose in
/// one walk.
pub(crate) fn lower_destination_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<(Vec<ProjectionStep>, ScalarType), LoweringError> {
    let (steps, walked) = walk_path(identities, structural_type, path, types)?;
    let scalar_type = match walked {
        Walked::Declared(structural_type) => {
            let StructuralTypeShape::PrimitiveScalar(scalar_type) = unique_declaration(
                types,
                structural_type,
                "primitive projection type is absent",
                "primitive projection type is ambiguous",
            )?
            .shape
            else {
                return unsupported("primitive projection does not end at a primitive scalar");
            };
            scalar_type
        }
        Walked::ByteElement => terminal_scalar_type(PrimitiveType::U8)?,
        Walked::ScalarField(scalar) => scalar,
    };
    Ok((steps, scalar_type))
}

/// A static projection to a primitive leaf, for observations that name no
/// runtime element.
pub(crate) fn lower_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<(Vec<terminal_psi::StructuralPathSegment>, ScalarType), LoweringError> {
    if let Some((CheckedUnitStructuralPathSegment::Field(identity), carrier)) = path.split_last() {
        // Reads may observe declaration-constrained scalar fields through a
        // pointer. Such a field is not raw primitive store storage. Reuse the
        // carrier walk (including its bounds), then query read-only leaf typing.
        let (steps, _) = walk_path(identities, structural_type, carrier, types)?;
        let mut path = super::runtime_elements::static_segments(steps)?;
        path.push(terminal_psi::StructuralPathSegment::Field(
            identities.intern(&identity.spelling()),
        ));
        let scalar_type = terminal_semantics::primitive_read_projection_type(
            types.iter(),
            structural_type,
            &path,
        )
        .ok_or(LoweringError::Unsupported(
            "primitive observation has no readable scalar endpoint",
        ))?;
        return Ok((path, scalar_type));
    }
    let (steps, scalar_type) = lower_destination_path(identities, structural_type, path, types)?;
    Ok((
        super::runtime_elements::static_segments(steps)?,
        scalar_type,
    ))
}

/// Resolve one scalar endpoint after any ordered element projections. A field
/// retains its declared scalar carrier and invariant; array elements retain
/// their declared primitive type. Runtime selectors change neither.
pub(crate) fn indexed_element_scalar_type(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    element: StructuralTypeId,
    element_path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<ScalarType, LoweringError> {
    if let Some((CheckedUnitStructuralPathSegment::Field(identity), carrier)) =
        element_path.split_last()
    {
        let (_, walked) = walk_path(identities, element, carrier, types)?;
        let Walked::Declared(record) = walked else {
            return unsupported("indexed field carrier is not a record");
        };
        let declaration = unique_declaration(
            types,
            record,
            "primitive projection type is absent",
            "primitive projection type is ambiguous",
        )?;
        let fields = declaration
            .shape
            .direct_fields()
            .ok_or(LoweringError::Unsupported(
                "indexed field carrier has no directly addressed fields",
            ))?;
        let mut matching = fields
            .iter()
            .filter(|field| field.identity == identities.intern(&identity.spelling()));
        let (Some(field), None) = (matching.next(), matching.next()) else {
            return unsupported("indexed field is absent or ambiguous");
        };
        if field.relevance.is_erased() {
            return unsupported("indexed field is erased");
        }
        let scalar_type = match &field.field_type {
            declared if terminal_semantics::scalar_field_requires_evidence(declared) => {
                declared.scalar_type().ok_or(LoweringError::Unsupported(
                    "indexed field invariant has no scalar carrier",
                ))?
            }
            StructuralFieldType::Scalar(ScalarType::Boolean) => ScalarType::Boolean,
            StructuralFieldType::Scalar(ScalarType::Integer(integer)) if !integer.is_address() => {
                ScalarType::Integer(*integer)
            }
            StructuralFieldType::Scalar(scalar @ ScalarType::IeeeFloat(_)) => *scalar,
            StructuralFieldType::IeeeFloat(format) => ScalarType::IeeeFloat(*format),
            _ => {
                return unsupported(
                    "indexed element field is not an observable scalar declaration",
                );
            }
        };
        return Ok(scalar_type);
    }
    let (_, walked) = walk_path(identities, element, element_path, types)?;
    let scalar_type = match walked {
        Walked::ScalarField(scalar_type) => scalar_type,
        Walked::Declared(identity) => match unique_declaration(
            types,
            identity,
            "primitive projection type is absent",
            "primitive projection type is ambiguous",
        )?
        .shape
        {
            StructuralTypeShape::PrimitiveScalar(scalar_type) => scalar_type,
            _ => return unsupported("indexed element path does not end at a scalar"),
        },
        Walked::ByteElement => {
            return unsupported("indexed element path requires its byte carrier");
        }
    };
    match scalar_type {
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => Ok(scalar_type),
        ScalarType::Integer(integer) if !integer.is_address() => Ok(scalar_type),
        _ => unsupported("indexed element leaf is not a readable scalar"),
    }
}

/// Walk the checked segments from the destination root, resolving each field
/// against its declared record, and return the steps and the structural
/// type they select. Each runtime element names a distinct selector of the
/// assignment's target.
/// Storage uses retained declarations, including byte elements. IEEE field
/// observations may additionally carry their scalar presentation directly.
type Walked = terminal_semantics::Position;

fn walk_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<(Vec<ProjectionStep>, Walked), LoweringError> {
    use terminal_psi::StructuralPathSegment as Segment;
    let mut walked = Walked::Declared(structural_type);
    let mut result = Vec::with_capacity(path.len());
    for segment in path {
        let Walked::Declared(structural_type) = walked else {
            return unsupported("primitive projection descends below a byte element");
        };
        let declaration = unique_declaration(
            types,
            structural_type,
            "primitive projection type is absent",
            "primitive projection type is ambiguous",
        )?;
        walked = match (segment, &declaration.shape) {
            (
                CheckedUnitStructuralPathSegment::Referent,
                StructuralTypeShape::Reference { referent, .. },
            ) => {
                result.push(ProjectionStep::Static(Segment::Referent));
                Walked::Declared(*referent)
            }
            (
                CheckedUnitStructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields, .. },
            ) => {
                let Some(field) = live_field(fields, identities.intern(&identity.spelling()))
                else {
                    return unsupported("primitive projection field is erased or ambiguous");
                };
                result.push(ProjectionStep::Static(Segment::Field(field.identity)));
                match field.field_type {
                    StructuralFieldType::Structural(child) => Walked::Declared(child),
                    StructuralFieldType::Scalar(scalar @ ScalarType::IeeeFloat(_)) => {
                        Walked::ScalarField(scalar)
                    }
                    StructuralFieldType::IeeeFloat(format) => {
                        Walked::ScalarField(ScalarType::IeeeFloat(format))
                    }
                    _ => Walked::Declared(
                        terminal_semantics::runtime_structural_path_tip(
                            types.iter(),
                            structural_type,
                            &[Segment::Field(field.identity)],
                        )
                        .ok_or(LoweringError::Unsupported(
                            "primitive projection field has no exact retained declaration",
                        ))?
                        .id,
                    ),
                }
            }
            (
                segment @ (CheckedUnitStructuralPathSegment::FixedIndex(_)
                | CheckedUnitStructuralPathSegment::RuntimeIndex(_)),
                StructuralTypeShape::FixedArray { .. }
                | StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(_),
            ) => {
                let (step, element) =
                    super::structural_scalar_store::element_selection(declaration, segment, types)?;
                result.push(step);
                Walked::Declared(element)
            }
            _ => {
                return unsupported(
                    "primitive projection has an incompatible or out-of-bounds path",
                );
            }
        };
    }
    Ok((result, walked))
}

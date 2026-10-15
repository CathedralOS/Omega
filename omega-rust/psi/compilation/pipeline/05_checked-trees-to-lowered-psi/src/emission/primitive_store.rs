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

/// The resolved primitive endpoint is shared by ordinary and composed bodies.
/// Their namespaces choose a parameter or live local; this owner validates the
/// exclusive parameter projection and evaluates exactly one authored RHS.
pub(crate) struct Destination {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<ProjectionStep>,
    pub(crate) scalar_restriction: Option<StructuralFieldType>,
    pub(crate) scalar_type: ScalarType,
}

pub(crate) fn parameter_destination(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    parameter: &StructuralParameterDeclaration,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<Destination, LoweringError> {
    if !matches!(
        parameter.access,
        terminal_psi::exclusive_borrow_pair_pattern!()
    ) || parameter.multiplicity == StructuralMultiplicity::Linear
        || (path.is_empty() && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
        || !parameter.qualifications.is_empty()
    {
        return unsupported("primitive store parameter lost its exclusive custody");
    }
    let scalar_restriction =
        destination_restriction(identities, parameter.structural_type, path, types)?;
    let (path, scalar_type) =
        lower_destination_path(identities, parameter.structural_type, path, types)?;
    Ok(Destination {
        place: parameter.place,
        path,
        scalar_restriction,
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
    let range_obligation = if let Some(restriction) = &destination.scalar_restriction {
        let decided = match restriction {
            StructuralFieldType::BoundedInteger(_) => {
                if receiving_range.is_none() {
                    return unsupported("primitive store lost its receiving-range judgment");
                }
                true
            }
            StructuralFieldType::RefinedScalar(_) if receiving_range.is_none() => false,
            _ => return unsupported("primitive store has an incompatible receiving judgment"),
        };
        let obligation =
            super::obligation_id(super::allocate_dense(&mut calls.next_obligation_identity)?);
        // An interval judgment retains its numeric shortcut. General predicates
        // retain their complete receiving goal; original operations obtain source
        // authority only at the completed immutable source-check cut.
        if decided {
            operations.source_checked_obligations.push(obligation);
        }
        Some(obligation)
    } else {
        if receiving_range.is_some() {
            return unsupported("primitive store erased its receiving restriction");
        }
        None
    };
    Ok(OperationKind::WriteOnlyPrimitiveStore {
        range_obligation,
        destination: destination.place,
        path: super::runtime_elements::complete_path(destination.path, indexes, calls, operations)?,
        value: value.id,
    })
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

pub(crate) fn destination_restriction(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    structural_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<Option<StructuralFieldType>, LoweringError> {
    let (_, walked) = walk_path(identities, structural_type, path, types)?;
    match walked {
        Walked::Declared(identity) => Ok(unique_declaration(
            types,
            identity,
            "primitive endpoint is absent",
            "primitive endpoint is ambiguous",
        )?
        .scalar_restriction
        .clone()),
        Walked::ByteElement | Walked::ScalarField(_) => Ok(None),
    }
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
/// The walk's current position: a declared structural type, or a byte
/// carrier's element — the u8 scalar that rides no declared identity.
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
                    _ => {
                        return unsupported(
                            "primitive projection requires a plain IEEE or structural field",
                        );
                    }
                }
            }
            (
                CheckedUnitStructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => {
                result.push(ProjectionStep::Static(Segment::FixedIndex(*index)));
                Walked::Declared(*element)
            }
            (
                CheckedUnitStructuralPathSegment::RuntimeIndex(runtime),
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element, .. },
            ) => {
                // Every checked selector vocabulary walks a fixed array the
                // same way: assignment indexes, parameter and local
                // positions, and stored-field snapshots each name a distinct
                // runtime element of the projection.
                result.push(super::runtime_elements::selector_step(runtime));
                Walked::Declared(*element)
            }
            (
                CheckedUnitStructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::ElementView { element, .. },
            ) => {
                // The descriptor declares the element type, never its live
                // bound. Even a literal selector needs an extent obligation.
                result.push(ProjectionStep::RuntimeIndexLiteral { index: *index });
                Walked::Declared(*element)
            }
            (
                segment @ (CheckedUnitStructuralPathSegment::FixedIndex(_)
                | CheckedUnitStructuralPathSegment::RuntimeIndex(_)),
                StructuralTypeShape::ByteSequence(carrier),
            ) => {
                match segment {
                    CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                        if let terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. } =
                            carrier
                            && *index >= *capacity
                        {
                            return unsupported(
                                "primitive projection has an incompatible or out-of-bounds path",
                            );
                        }
                        result.push(ProjectionStep::Static(Segment::FixedIndex(*index)));
                    }
                    CheckedUnitStructuralPathSegment::RuntimeIndex(runtime) => {
                        // A repeated operand still selects a new element of
                        // this carrier and owns its own positional bound.
                        result.push(super::runtime_elements::selector_step(runtime));
                    }
                    _ => {}
                }
                Walked::ByteElement
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

//! Shared checked custody replay for scalar stores through structural fields.

use super::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedUnitStructuralPathSegment,
    LoweringError, OperationKind, PlaceId, PrimitiveType, ScalarType, StructuralAccess,
    StructuralFieldId, StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
    ValueId, allocate_dense, obligation_id, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::runtime_elements::ProjectionStep;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoreAccessPolicy {
    MutableOnly,
    Exclusive,
}

pub(crate) struct LoweredStructuralScalarStore {
    /// Carrier steps from the destination root; a runtime element's selector
    /// is evaluated by the emitter before the stored value.
    pub steps: Vec<ProjectionStep>,
    pub field: StructuralFieldId,
    pub scalar_type: ScalarType,
    pub requires_range_obligation: bool,
}

impl LoweredStructuralScalarStore {
    /// The store into `field` at the end of `steps`, once the caller has
    /// established the root's custody. The field must hold `scalar_type`.
    fn at_field(
        scalar_type: ScalarType,
        steps: Vec<ProjectionStep>,
        field: &terminal_psi::StructuralFieldDeclaration,
    ) -> Result<Self, LoweringError> {
        if field.field_type.scalar_type() != Some(scalar_type) {
            return unsupported("structural scalar store field has a different type");
        }
        Ok(Self {
            steps,
            field: field.id,
            scalar_type,
            requires_range_obligation: terminal_semantics::scalar_field_requires_evidence(
                &field.field_type,
            ),
        })
    }

    /// The static carrier path, for routes that evaluate no selector.
    pub(crate) fn static_path(&self) -> Result<Vec<StructuralPathSegment>, LoweringError> {
        crate::emission::runtime_elements::static_segments(self.steps.clone())
    }

    /// Each replacement requests its own declaration-derived range proof against
    /// the completed RHS. Neither an earlier read nor the root's initial validity
    /// can stand in for proving the value about to be stored. `indexes` are the
    /// carrier's evaluated runtime elements, in path order.
    pub(crate) fn into_operation(
        self,
        destination: PlaceId,
        indexes: Vec<ValueId>,
        value: ValueId,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<OperationKind, LoweringError> {
        let range_obligation = if self.requires_range_obligation {
            Some(obligation_id(allocate_dense(
                &mut calls.next_obligation_identity,
            )?))
        } else {
            None
        };
        Ok(OperationKind::StructuralScalarFieldStore {
            destination,
            path: crate::emission::runtime_elements::complete_path(self.steps, indexes, calls)?,
            field: self.field,
            value,
            range_obligation,
        })
    }
}

pub(crate) fn lower_structural_scalar_store_destination(
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    expected_statement_index: u32,
    parameter: &StructuralParameterDeclaration,
    structural_types: &[StructuralTypeDeclaration],
    scalar_parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
    available_scalar_types: &[ScalarType],
    access_policy: StoreAccessPolicy,
) -> Result<LoweredStructuralScalarStore, LoweringError> {
    if store.destination.parameter_position() != Some(parameter.position)
        || !checked_store_source_matches(
            store.value.as_pure().ok_or(LoweringError::Unsupported(
                "this structural scalar store route requires a pure RHS",
            ))?,
            store.primitive_type,
            scalar_parameters,
            available_scalar_types,
        )
    {
        return unsupported("structural scalar store lost exact exclusive custody");
    }
    lower_structural_scalar_store_place(
        store,
        expected_statement_index,
        parameter,
        structural_types,
        access_policy,
    )
}

/// Resolve the destination independently of the ordered source expression.
/// Callers must validate and emit that expression in its current scalar namespace.
/// They also rejoin the authored destination through their source-position map
/// before supplying `parameter`. Its Terminal position is dense: scalar formals
/// and omitted receivers can make it differ from the authored position.
pub(crate) fn lower_structural_scalar_store_place(
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    expected_statement_index: u32,
    parameter: &StructuralParameterDeclaration,
    structural_types: &[StructuralTypeDeclaration],
    access_policy: StoreAccessPolicy,
) -> Result<LoweredStructuralScalarStore, LoweringError> {
    let scalar_type = terminal_scalar_type(store.primitive_type)?;
    let (steps, field) = lower_structural_field_place_steps(
        store.statement_index,
        expected_statement_index,
        store
            .destination
            .parameter_position()
            .map(|_| parameter.position)
            .ok_or(LoweringError::Unsupported(
                "parameter store route cannot substitute a local destination",
            ))?,
        &store.carrier_path,
        &store.field_identity,
        parameter,
        structural_types,
        access_policy,
    )?;
    LoweredStructuralScalarStore::at_field(scalar_type, steps, field)
}

/// The owned-local counterpart of `lower_structural_scalar_store_place`: the
/// root's custody was already established by its producing operation — the
/// caller checks the produced result's multiplicity, qualifications, and
/// claims — so only the carrier geometry and field type compose here.
pub(crate) fn lower_structural_scalar_store_local(
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    root_type: StructuralTypeId,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<LoweredStructuralScalarStore, LoweringError> {
    let scalar_type = terminal_scalar_type(store.primitive_type)?;
    let (steps, field) = lower_structural_field_steps(
        root_type,
        &store.carrier_path,
        &store.field_identity,
        structural_types,
    )?;
    LoweredStructuralScalarStore::at_field(scalar_type, steps, field)
}

/// Reconstruct the exact carrier and relevant field independently of payload
/// type, for routes whose carrier names no runtime element.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_structural_field_place<'a>(
    statement_index: u32,
    expected_statement_index: u32,
    destination_parameter_position: u32,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &str,
    parameter: &StructuralParameterDeclaration,
    structural_types: &'a [StructuralTypeDeclaration],
    access_policy: StoreAccessPolicy,
) -> Result<
    (
        Vec<StructuralPathSegment>,
        &'a terminal_psi::StructuralFieldDeclaration,
    ),
    LoweringError,
> {
    let (steps, field) = lower_structural_field_place_steps(
        statement_index,
        expected_statement_index,
        destination_parameter_position,
        carrier_path,
        field_identity,
        parameter,
        structural_types,
        access_policy,
    )?;
    Ok((
        crate::emission::runtime_elements::static_segments(steps)?,
        field,
    ))
}

/// Reconstruct the exact carrier and relevant field independently of payload type.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_structural_field_place_steps<'a>(
    statement_index: u32,
    expected_statement_index: u32,
    destination_parameter_position: u32,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &str,
    parameter: &StructuralParameterDeclaration,
    structural_types: &'a [StructuralTypeDeclaration],
    access_policy: StoreAccessPolicy,
) -> Result<
    (
        Vec<ProjectionStep>,
        &'a terminal_psi::StructuralFieldDeclaration,
    ),
    LoweringError,
> {
    validate_root_access(parameter, access_policy)?;
    if statement_index != expected_statement_index
        || destination_parameter_position != parameter.position
    {
        return unsupported("structural scalar store lost exact exclusive custody");
    }
    lower_structural_field_steps(
        parameter.structural_type,
        carrier_path,
        field_identity,
        structural_types,
    )
}

/// Typed field projections and representation-byte projections require the
/// same live root authority. Geometry cannot turn a shared place exclusive.
pub(crate) fn validate_root_access(
    parameter: &StructuralParameterDeclaration,
    access_policy: StoreAccessPolicy,
) -> Result<(), LoweringError> {
    let access_matches = match access_policy {
        StoreAccessPolicy::MutableOnly => parameter.access == StructuralAccess::MutableBorrow,
        StoreAccessPolicy::Exclusive => matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        ),
    };
    if !access_matches
        || !matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
    {
        return unsupported("structural scalar store lost exact exclusive custody");
    }
    Ok(())
}

/// Shared geometry follows exact declarations; callers separately establish the
/// root's current ownership or exclusive-borrow authority. The carrier names
/// no runtime element.
pub(crate) fn lower_structural_field_path<'a>(
    root_type: StructuralTypeId,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &str,
    structural_types: &'a [StructuralTypeDeclaration],
) -> Result<
    (
        Vec<StructuralPathSegment>,
        &'a terminal_psi::StructuralFieldDeclaration,
    ),
    LoweringError,
> {
    let (steps, field) =
        lower_structural_field_steps(root_type, carrier_path, field_identity, structural_types)?;
    Ok((
        crate::emission::runtime_elements::static_segments(steps)?,
        field,
    ))
}

/// Normalize a storage root to its field-bearing declaration and retain any
/// pointer crossing explicitly. All field-store kinds use this geometry;
/// it cannot establish permission or select an address from possible origins.
fn structural_field_root(
    root_type: StructuralTypeId,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<(&StructuralTypeDeclaration, Vec<ProjectionStep>), LoweringError> {
    let declaration = structural_types
        .iter()
        .find(|declaration| declaration.id == root_type)
        .ok_or(LoweringError::Unsupported(
            "structural scalar store root type is absent",
        ))?;
    // A reference-rooted walk joins its referent: `cell.value = 7` through a
    // `&mut Cell` local walks `Cell`'s members — the `ref-mut` shell is the
    // loan custody, never the record that owns the field.
    let field_owner = match &declaration.shape {
        StructuralTypeShape::Reference { referent, .. } => structural_types
            .iter()
            .find(|candidate| candidate.id == *referent)
            .ok_or(LoweringError::Unsupported(
                "structural scalar store referent type is absent",
            ))?,
        _ => declaration,
    };
    let mut steps = Vec::new();
    // A record reference carries a pointer and its loan, not inline fields.
    // Make that crossing explicit so downstream geometry cannot mistake the
    // carrier's own home for the caller's record storage.
    if matches!(declaration.shape, StructuralTypeShape::Reference { .. })
        && matches!(field_owner.shape, StructuralTypeShape::Record { .. })
    {
        steps.push(ProjectionStep::Static(StructuralPathSegment::Referent));
    }
    Ok((field_owner, steps))
}

/// Resolve field geometry from the same storage root used by restoration
/// windows. The explicit reference crossing describes pointer geometry only;
/// operation admission and independent replay establish mutation authority.
pub(crate) fn lower_structural_field_steps<'a>(
    root_type: StructuralTypeId,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &str,
    structural_types: &'a [StructuralTypeDeclaration],
) -> Result<
    (
        Vec<ProjectionStep>,
        &'a terminal_psi::StructuralFieldDeclaration,
    ),
    LoweringError,
> {
    let (mut field_owner, mut steps) = structural_field_root(root_type, structural_types)?;
    for segment in carrier_path {
        let nested = match segment {
            CheckedUnitStructuralPathSegment::Field(identity) if !identity.is_empty() => {
                let StructuralTypeShape::Record { fields } = &field_owner.shape else {
                    return unsupported("structural scalar store carrier is not a record");
                };
                let carriers = fields
                    .iter()
                    .filter(|field| {
                        field.identity == *identity
                            && !field.relevance.is_erased()
                            && matches!(field.field_type, StructuralFieldType::Structural(_))
                    })
                    .collect::<Vec<_>>();
                let [carrier] = carriers.as_slice() else {
                    return unsupported("structural scalar store carrier is absent or ambiguous");
                };
                let StructuralFieldType::Structural(nested) = carrier.field_type else {
                    unreachable!("carrier shape was checked above")
                };
                steps.push(ProjectionStep::Static(StructuralPathSegment::Field(
                    identity.clone(),
                )));
                nested
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                let element = match &field_owner.shape {
                    StructuralTypeShape::FixedArray { element, length } => {
                        if *index >= *length {
                            return unsupported(
                                "structural scalar store fixed index is out of bounds",
                            );
                        }
                        *element
                    }
                    // A view's extent is its own stored runtime length, not a
                    // literal in the type: the element's bound re-proves
                    // against that extent when the store executes.
                    StructuralTypeShape::ElementView { element } => {
                        steps.push(ProjectionStep::RuntimeIndexLiteral { index: *index });
                        let selected = *element;
                        field_owner = structural_types
                            .iter()
                            .find(|row| row.id == selected)
                            .ok_or(LoweringError::Unsupported(
                                "view element declaration is absent",
                            ))?;
                        continue;
                    }
                    _ => {
                        return unsupported("structural scalar store carrier is not a fixed array");
                    }
                };
                steps.push(ProjectionStep::Static(StructuralPathSegment::FixedIndex(
                    *index,
                )));
                element
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex(runtime) => {
                let element = match &field_owner.shape {
                    StructuralTypeShape::FixedArray { element, .. } => *element,
                    // A view's extent is its own stored runtime length: the
                    // runtime element's bound re-proves against that extent
                    // when the store executes, exactly as a fixed index's
                    // does.
                    StructuralTypeShape::ElementView { element } => *element,
                    _ => {
                        return unsupported("structural scalar store carrier is not a fixed array");
                    }
                };
                match runtime {
                    checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth }
                        if !steps.contains(&ProjectionStep::AssignmentIndex { depth: *depth }) =>
                    {
                        steps.push(ProjectionStep::AssignmentIndex { depth: *depth });
                    }
                    // A borrow-mirrored index names the calling state's own
                    // scalar parameter — `&mut cells[at]` injects the store
                    // coordinate `at` — which the scalar source frame reads
                    // directly rather than a bound selector row.
                    checked_trees::CheckedRuntimeIndex::Parameter { position } => {
                        steps.push(ProjectionStep::RuntimeIndexParameter {
                            position: *position,
                        });
                    }
                    _ => {
                        return unsupported("structural scalar store carrier path is unsupported");
                    }
                }
                element
            }
            _ => return unsupported("structural scalar store carrier path is unsupported"),
        };
        field_owner = structural_types
            .iter()
            .find(|candidate| candidate.id == nested)
            .ok_or(LoweringError::Unsupported(
                "structural scalar store nested carrier type is absent",
            ))?;
    }
    // A mixed shape's common fields sit at one offset in every case, so the
    // leaf writes one without a `Case` step, as a read observes it.
    let (StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }) =
        &field_owner.shape
    else {
        return unsupported("structural scalar store field owner is not a record");
    };
    let matching = fields
        .iter()
        .filter(|field| field.identity == field_identity && !field.relevance.is_erased())
        .collect::<Vec<_>>();
    let [field] = matching.as_slice() else {
        return unsupported("structural scalar store field is absent or ambiguous");
    };
    Ok((steps, field))
}

/// A window path's leaf: the last segment is a record member (`Field`) or an
/// element selection of a fixed array (`Element`). An element's path is
/// already complete; it does not borrow an enclosing record's field ID.
pub(crate) enum WindowLeaf<'a> {
    /// The route's last segment names a record member; the hole's declared
    /// type is that field's structural type.
    Field(&'a terminal_psi::StructuralFieldDeclaration),
    /// The route ends in an element selection of the declared type.
    Element { element_type: StructuralTypeId },
}

/// Walk a window operation's spelled path — record fields and fixed-array
/// elements in any order — to its leaf. The leaf's kind is decided by the
/// last segment: a field leaf keeps the field-hole spelling Terminal already
/// admits, and an element leaf selects the array's element type.
pub(crate) fn lower_window_path<'a>(
    root_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    structural_types: &'a [StructuralTypeDeclaration],
) -> Result<(Vec<ProjectionStep>, WindowLeaf<'a>), LoweringError> {
    let (mut field_owner, mut steps) = structural_field_root(root_type, structural_types)?;
    let mut cursor = path;
    while let Some((segment, rest)) = cursor.split_first() {
        cursor = rest;
        match segment {
            CheckedUnitStructuralPathSegment::Field(identity) if !identity.is_empty() => {
                let (StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. }) = &field_owner.shape
                else {
                    return unsupported("structural window carrier is not a record");
                };
                if rest.is_empty() {
                    // The leaf resolves by identity alone: a scalar leaf is a
                    // window the caller rejects on the leaf's declared type.
                    let mut leafs = fields.iter().filter(|field| {
                        field.identity == *identity && !field.relevance.is_erased()
                    });
                    let leaf = leafs.next();
                    if leaf.is_none() || leafs.next().is_some() {
                        return unsupported("structural window leaf is absent or ambiguous");
                    }
                    return Ok((steps, WindowLeaf::Field(leaf.expect("leaf checked above"))));
                }
                let mut carriers = fields.iter().filter(|field| {
                    field.identity == *identity
                        && !field.relevance.is_erased()
                        && matches!(field.field_type, StructuralFieldType::Structural(_))
                });
                let carrier = carriers.next();
                if carrier.is_none() || carriers.next().is_some() {
                    return unsupported("structural window carrier is absent or ambiguous");
                }
                let carrier = carrier.expect("carrier presence was checked above");
                steps.push(ProjectionStep::Static(StructuralPathSegment::Field(
                    identity.clone(),
                )));
                let StructuralFieldType::Structural(nested) = carrier.field_type else {
                    unreachable!("carrier shape was checked above")
                };
                field_owner = structural_types
                    .iter()
                    .find(|candidate| candidate.id == nested)
                    .ok_or(LoweringError::Unsupported(
                        "structural window nested carrier type is absent",
                    ))?;
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                let StructuralTypeShape::FixedArray { element, length } = &field_owner.shape else {
                    return unsupported("structural window carrier is not a fixed array");
                };
                if *index >= *length {
                    return unsupported("structural window fixed index is out of bounds");
                }
                steps.push(ProjectionStep::Static(StructuralPathSegment::FixedIndex(
                    *index,
                )));
                field_owner = structural_types
                    .iter()
                    .find(|candidate| candidate.id == *element)
                    .ok_or(LoweringError::Unsupported(
                        "structural window element type is absent",
                    ))?;
                if rest.is_empty() {
                    return Ok((
                        steps,
                        WindowLeaf::Element {
                            element_type: *element,
                        },
                    ));
                }
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
            ) => {
                let StructuralTypeShape::FixedArray { element, .. } = &field_owner.shape else {
                    return unsupported("structural window carrier is not a fixed array");
                };
                if !steps.contains(&ProjectionStep::AssignmentIndex { depth: *depth }) {
                    steps.push(ProjectionStep::AssignmentIndex { depth: *depth });
                }
                field_owner = structural_types
                    .iter()
                    .find(|candidate| candidate.id == *element)
                    .ok_or(LoweringError::Unsupported(
                        "structural window element type is absent",
                    ))?;
                if rest.is_empty() {
                    return Ok((
                        steps,
                        WindowLeaf::Element {
                            element_type: *element,
                        },
                    ));
                }
            }
            _ => return unsupported("structural window carrier path is unsupported"),
        }
    }
    unsupported("structural window path has no leaf segment")
}

fn checked_store_source_matches(
    value: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
    scalar_parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
    available_scalar_types: &[ScalarType],
) -> bool {
    if checked_store_literal_matches(value, primitive_type) {
        return true;
    }
    if let CheckedScalarExpression::Local {
        position,
        primitive_type: source_type,
    } = value
    {
        return scalar_parameters.is_empty()
            && *position == 0
            && *source_type == primitive_type
            && terminal_scalar_type(*source_type).ok()
                == available_scalar_types.get(*position).copied()
            && available_scalar_types.len() == 1
            && matches!(
                primitive_type,
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
            );
    }
    if scalar_parameters.is_empty() {
        return checked_store_literal_matches(value, primitive_type);
    }
    let (position, source_type) = match value {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => (*position, *primitive_type),
        CheckedScalarExpression::Boolean(boolean) => {
            let CheckedBooleanExpression::Parameter { position } = boolean.as_ref() else {
                return false;
            };
            (*position, PrimitiveType::Bool)
        }
        _ => return false,
    };
    scalar_parameters.get(position).is_some_and(|parameter| {
        Some(parameter.source_position) == authored_scalar_position(position)
            && parameter.primitive_type == primitive_type
            && source_type == primitive_type
    }) && scalar_parameters
        .iter()
        .enumerate()
        .all(|(index, parameter)| {
            Some(parameter.source_position) == authored_scalar_position(index)
        })
}

fn authored_scalar_position(dense_position: usize) -> Option<u32> {
    u32::try_from(dense_position).ok()?.checked_add(1)
}

pub(crate) fn checked_store_literal_matches(
    value: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> bool {
    match (value, primitive_type) {
        (CheckedScalarExpression::IeeeFloatLiteral { value }, primitive_type) => {
            terminal_scalar_type(primitive_type).ok() == Some(ScalarType::IeeeFloat(value.format()))
        }
        (CheckedScalarExpression::IntegerLiteral { .. }, primitive_type) => {
            primitive_type.accepts_integer_literal()
        }
        (CheckedScalarExpression::Boolean(boolean), PrimitiveType::Bool) => {
            matches!(boolean.as_ref(), CheckedBooleanExpression::Constant(_))
        }
        _ => false,
    }
}

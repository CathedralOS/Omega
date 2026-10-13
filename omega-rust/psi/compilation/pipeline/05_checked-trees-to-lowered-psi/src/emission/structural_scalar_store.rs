//! Shared checked custody replay for scalar stores through structural fields.
//!
//! Direct fields and element-view leaves consume the same occurrence-bound
//! receiving-range judgment. Its exact emitted interval must match: an index
//! proof, a final state invariant, or a different field's bounds cannot stand
//! in for checking this value immediately before this store.

use super::{
    CheckedUnitStructuralPathSegment, LoweringError, OperationKind, PlaceId, ScalarType,
    StructuralFieldId, StructuralFieldType, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, ValueId, allocate_dense,
    obligation_id, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::runtime_elements::ProjectionStep;

pub(crate) struct LoweredStructuralScalarStore {
    /// Carrier steps from the destination root; a runtime element's selector
    /// is evaluated by the emitter before the stored value.
    pub steps: Vec<ProjectionStep>,
    pub field: StructuralFieldId,
    pub requires_range_obligation: bool,
    bounded_integer: Option<semantic_vocabulary::BoundedIntegerType>,
    pub range_decided: bool,
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
            bounded_integer: match &field.field_type {
                StructuralFieldType::BoundedInteger(integer) => Some(*integer),
                _ => None,
            },
            range_decided: false,
            steps,
            field: field.id,
            requires_range_obligation: terminal_semantics::scalar_field_requires_evidence(
                &field.field_type,
            ),
        })
    }

    /// Bind the already completed source judgment to this emitted field's full
    /// range obligation. A different qualification or transformed geometry is
    /// not authorized by an integer interval, even if its carrier is identical.
    pub(crate) fn bind_range_decision(
        &mut self,
        checked: &checked_trees::CheckedTrees,
        primitive_type: checked_trees::types::PrimitiveType,
        range_decision: arena::Handle<checked_trees::CheckedIntegerAssignmentDecision>,
    ) -> Result<(), LoweringError> {
        self.range_decided = source_range_decided(
            checked,
            primitive_type,
            range_decision,
            self.bounded_integer,
        )?;
        Ok(())
    }

    /// Each replacement requests its own declaration-derived range proof against
    /// the completed RHS. Neither an earlier read nor the root's initial validity
    /// can stand in for proving the value about to be stored. The completed path
    /// retains the carrier's captured selectors and their bound obligations.
    pub(crate) fn into_operation(
        self,
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        value: ValueId,
        calls: &mut CallEmissionContext<'_>,
        operations: &mut OperationBuffer,
    ) -> Result<OperationKind, LoweringError> {
        let range_obligation = if self.requires_range_obligation {
            Some(obligation_id(allocate_dense(
                &mut calls.next_obligation_identity,
            )?))
        } else {
            None
        };
        if self.range_decided
            && let Some(obligation) = range_obligation
        {
            operations.source_checked_obligations.push(obligation);
        }
        Ok(OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field: self.field,
            value,
            range_obligation,
        })
    }
}

/// Match a retained source judgment to the complete emitted integer range.
pub(crate) fn source_range_decided(
    checked: &checked_trees::CheckedTrees,
    primitive_type: checked_trees::types::PrimitiveType,
    range_decision: arena::Handle<checked_trees::CheckedIntegerAssignmentDecision>,
    bounded_integer: Option<semantic_vocabulary::BoundedIntegerType>,
) -> Result<bool, LoweringError> {
    if !range_decision.is_valid() {
        return Ok(false);
    }
    let decision = checked.facts.proof.integer_assignments.get(range_decision);
    let Some(integer) = bounded_integer else {
        return Ok(false);
    };
    let mathematical = |value| match value {
        semantic_vocabulary::IntegerValue::Signed(value) => {
            numerics::bignum::BigInt::from_i128(value)
        }
        semantic_vocabulary::IntegerValue::Unsigned(value) => {
            numerics::bignum::BigInt::from_u128(value)
        }
    };
    let decided = terminal_scalar_type(primitive_type)?
        == ScalarType::Integer(integer.integer_type())
        && decision.minimum == mathematical(integer.minimum())
        && decision.maximum == mathematical(integer.maximum());
    Ok(decided)
}

/// Resolve a scalar field store's destination below an exclusive borrowed
/// parameter, independently of the ordered source expression. Callers
/// validate and emit that expression in its current scalar namespace.
pub(crate) fn lower_structural_scalar_store_place(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    primitive_type: checked_trees::types::PrimitiveType,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &checked_trees::CheckedMemberIdentity,
    parameter: &StructuralParameterDeclaration,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<LoweredStructuralScalarStore, LoweringError> {
    let scalar_type = terminal_scalar_type(primitive_type)?;
    let (steps, field) = lower_structural_field_place_steps(
        identities,
        carrier_path,
        field_identity,
        parameter,
        structural_types,
    )?;
    LoweredStructuralScalarStore::at_field(scalar_type, steps, field)
}

/// The owned-local counterpart of `lower_structural_scalar_store_place`: the
/// root's custody was already established by its producing operation — the
/// caller checks the produced result's multiplicity, qualifications, and
/// claims — so only the carrier geometry and field type compose here.
pub(crate) fn lower_structural_scalar_store_local(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    primitive_type: checked_trees::types::PrimitiveType,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &checked_trees::CheckedMemberIdentity,
    root_type: StructuralTypeId,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<LoweredStructuralScalarStore, LoweringError> {
    let scalar_type = terminal_scalar_type(primitive_type)?;
    let (steps, field) = lower_structural_field_steps(
        identities,
        root_type,
        carrier_path,
        field_identity,
        structural_types,
    )?;
    LoweredStructuralScalarStore::at_field(scalar_type, steps, field)
}

/// Reconstruct the exact carrier and relevant field below an exclusive
/// borrowed parameter independently of payload type, for routes whose
/// carrier names no runtime element.
pub(crate) fn lower_structural_field_place<'a>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &checked_trees::CheckedMemberIdentity,
    parameter: &StructuralParameterDeclaration,
    structural_types: &'a [StructuralTypeDeclaration],
) -> Result<
    (
        Vec<StructuralPathSegment>,
        &'a terminal_psi::StructuralFieldDeclaration,
    ),
    LoweringError,
> {
    let (steps, field) = lower_structural_field_place_steps(
        identities,
        carrier_path,
        field_identity,
        parameter,
        structural_types,
    )?;
    Ok((
        crate::emission::runtime_elements::static_segments(steps)?,
        field,
    ))
}

/// Reconstruct the exact carrier and relevant field below an exclusive
/// borrowed parameter independently of payload type.
pub(crate) fn lower_structural_field_place_steps<'a>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &checked_trees::CheckedMemberIdentity,
    parameter: &StructuralParameterDeclaration,
    structural_types: &'a [StructuralTypeDeclaration],
) -> Result<
    (
        Vec<ProjectionStep>,
        &'a terminal_psi::StructuralFieldDeclaration,
    ),
    LoweringError,
> {
    validate_root_access(parameter)?;
    lower_structural_field_steps(
        identities,
        parameter.structural_type,
        carrier_path,
        field_identity,
        structural_types,
    )
}

/// Typed field projections and representation-byte projections require the
/// same live write authority. A nonlinear owned entry or continuation
/// home supplies that authority directly, just as a completed local home does.
/// Borrowed roots still need exclusive access. Geometry never changes access
/// or discards root and projected qualification evidence.
pub(crate) fn validate_root_access(
    parameter: &StructuralParameterDeclaration,
) -> Result<(), LoweringError> {
    let access_matches = match parameter.access {
        terminal_psi::ownership::StructuralAccess::Owned => true,
        terminal_psi::exclusive_borrow_pair_pattern!() => true,
        _ => false,
    };
    if !access_matches
        || !matches!(
            parameter.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    root_type: StructuralTypeId,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &checked_trees::CheckedMemberIdentity,
    structural_types: &'a [StructuralTypeDeclaration],
) -> Result<
    (
        Vec<StructuralPathSegment>,
        &'a terminal_psi::StructuralFieldDeclaration,
    ),
    LoweringError,
> {
    let (steps, field) = lower_structural_field_steps(
        identities,
        root_type,
        carrier_path,
        field_identity,
        structural_types,
    )?;
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
    let declaration = terminal_psi::find_structural_type(structural_types, root_type).ok_or(
        LoweringError::Unsupported("structural scalar store root type is absent"),
    )?;
    // A reference carries its pointer and loan, not inline members. Retain
    // every crossing so downstream geometry observes the referent, never the
    // carrier's home. This applies at the root and at every selected child.
    let mut field_owner = declaration;
    let mut steps = Vec::new();
    while let StructuralTypeShape::Reference { referent, .. } = &field_owner.shape {
        if steps.len() >= structural_types.len() {
            return unsupported("field geometry contains a cyclic reference path");
        }
        field_owner = terminal_psi::find_structural_type(structural_types, *referent).ok_or(
            LoweringError::Unsupported("structural scalar store referent type is absent"),
        )?;
        steps.push(ProjectionStep::Static(StructuralPathSegment::Referent));
    }
    Ok((field_owner, steps))
}

/// Resolve field geometry from the same storage root used by restoration
/// windows. The explicit reference crossing describes pointer geometry only;
/// operation admission and independent replay establish mutation authority.
pub(crate) fn lower_structural_field_steps<'a>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    root_type: StructuralTypeId,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &checked_trees::CheckedMemberIdentity,
    structural_types: &'a [StructuralTypeDeclaration],
) -> Result<
    (
        Vec<ProjectionStep>,
        &'a terminal_psi::StructuralFieldDeclaration,
    ),
    LoweringError,
> {
    let mut field_owner = terminal_psi::find_structural_type(structural_types, root_type).ok_or(
        LoweringError::Unsupported("structural scalar store root type is absent"),
    )?;
    let mut steps = Vec::new();
    let mut payload_fields = None;
    for segment in carrier_path {
        if matches!(segment, CheckedUnitStructuralPathSegment::Referent) {
            if payload_fields.is_some() {
                return unsupported("field path case must select a payload field");
            }
            let StructuralTypeShape::Reference { referent, .. } = &field_owner.shape else {
                return unsupported("field path reference crossing has no reference carrier");
            };
            field_owner = terminal_psi::find_structural_type(structural_types, *referent).ok_or(
                LoweringError::Unsupported("field path referent type is absent"),
            )?;
            steps.push(ProjectionStep::Static(StructuralPathSegment::Referent));
            continue;
        }
        // A declared member selects through its owner's reference shell.
        // Delay this normalization until member selection so an explicit
        // checked Referent step consumes the pointer exactly once.
        let (owner, crossings) = structural_field_root(field_owner.id, structural_types)?;
        field_owner = owner;
        steps.extend(crossings);
        if let CheckedUnitStructuralPathSegment::Case(identity) = segment {
            if payload_fields.is_some() {
                return unsupported("field path repeats an unconsumed case namespace");
            }
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &field_owner.shape
            else {
                return unsupported("field path case owner is not a sum");
            };
            let mut matching = cases
                .iter()
                .filter(|case| case.identity == identities.intern(&identity.spelling()));
            let selected = matching
                .next()
                .ok_or(LoweringError::Unsupported("field path case is absent"))?;
            if matching.next().is_some() {
                return unsupported("field path case is ambiguous");
            }
            payload_fields = Some(selected.fields.as_slice());
            steps.push(ProjectionStep::Static(StructuralPathSegment::Case(
                selected.id,
            )));
            continue;
        }
        if payload_fields.is_some()
            && !matches!(segment, CheckedUnitStructuralPathSegment::Field(_))
        {
            return unsupported("field path case must select a payload field");
        }
        let nested = match segment {
            CheckedUnitStructuralPathSegment::Field(identity) if identity.is_valid() => {
                let fields = match payload_fields.take() {
                    Some(fields) => fields,
                    None => field_owner
                        .shape
                        .direct_fields()
                        .ok_or(LoweringError::Unsupported(
                            "structural scalar store carrier has no direct fields",
                        ))?,
                };
                let carriers = fields
                    .iter()
                    .filter(|field| {
                        field.identity == identities.intern(&identity.spelling())
                            && !field.relevance.is_erased()
                            && matches!(field.field_type, StructuralFieldType::Structural(_))
                    })
                    .collect::<Vec<_>>();
                let [carrier] = carriers.as_slice() else {
                    return unsupported("structural scalar store carrier is absent or ambiguous");
                };
                let Some(nested) = carrier.field_type.as_structural() else {
                    unreachable!("carrier shape was checked above")
                };
                steps.push(ProjectionStep::Static(StructuralPathSegment::Field(
                    identities.intern(&identity.spelling()),
                )));
                nested
            }
            CheckedUnitStructuralPathSegment::FixedIndex(_)
            | CheckedUnitStructuralPathSegment::RuntimeIndex(_) => {
                let (step, element) = element_selection(field_owner, segment)?;
                if matches!(step, ProjectionStep::AssignmentIndex { .. }) && steps.contains(&step) {
                    return unsupported("structural scalar store carrier path is unsupported");
                }
                steps.push(step);
                element
            }
            _ => return unsupported("structural scalar store carrier path is unsupported"),
        };
        field_owner = terminal_psi::find_structural_type(structural_types, nested).ok_or(
            LoweringError::Unsupported("structural scalar store carrier type is absent"),
        )?;
    }
    let (owner, crossings) = structural_field_root(field_owner.id, structural_types)?;
    field_owner = owner;
    steps.extend(crossings);
    // A mixed shape's common fields sit at one offset in every case, so the
    // leaf writes one without a `Case` step, as a read observes it.
    let Some(fields) = payload_fields.or_else(|| field_owner.shape.direct_fields()) else {
        return unsupported("structural scalar store field owner is not a record");
    };
    let matching = fields
        .iter()
        .filter(|field| {
            field.identity == identities.intern(&field_identity.spelling())
                && !field.relevance.is_erased()
        })
        .collect::<Vec<_>>();
    let [field] = matching.as_slice() else {
        return unsupported("structural scalar store field is absent or ambiguous");
    };
    Ok((steps, field))
}

/// The element one index segment selects below `owner`, and the step that
/// selects it. A fixed array's literal index is static geometry bounded by its
/// declared extent. A view's extent is its own stored runtime length, not a
/// literal in the type, so even a literal selector is a runtime element whose
/// bound re-proves against that extent when the operation executes. Scalar
/// stores and restoration windows share this one decision: Terminal has no
/// static crossing into an element view.
fn element_selection(
    owner: &StructuralTypeDeclaration,
    segment: &CheckedUnitStructuralPathSegment,
) -> Result<(ProjectionStep, StructuralTypeId), LoweringError> {
    let (element, view) = match &owner.shape {
        StructuralTypeShape::FixedArray { element, length } => {
            if matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(index) if index >= length)
            {
                return unsupported("structural element index is out of bounds");
            }
            (*element, false)
        }
        StructuralTypeShape::ElementView { element } => (*element, true),
        _ => return unsupported("structural element carrier is not a fixed array or view"),
    };
    let step = match segment {
        CheckedUnitStructuralPathSegment::FixedIndex(index) if view => {
            ProjectionStep::RuntimeIndexLiteral { index: *index }
        }
        CheckedUnitStructuralPathSegment::FixedIndex(index) => {
            ProjectionStep::Static(StructuralPathSegment::FixedIndex(*index))
        }
        CheckedUnitStructuralPathSegment::RuntimeIndex(selector) => {
            crate::emission::runtime_elements::selector_step(selector)
        }
        _ => return unsupported("structural element selector is unsupported"),
    };
    Ok((step, element))
}

/// A window path's leaf: the last segment is a record member (`Field`) or an
/// element selection of a fixed array or view (`Subtree`). An element's path
/// is already complete; it does not borrow an enclosing record's field ID.
pub(crate) enum WindowLeaf<'a> {
    /// The route's last segment names a record member; the hole's declared
    /// type is that field's structural type.
    Field(&'a terminal_psi::StructuralFieldDeclaration),
    /// The route selects a complete subtree without an additional field.
    /// This includes an element or the referent selected by a root crossing.
    Subtree { structural_type: StructuralTypeId },
}

/// Walk a window operation's spelled path — record fields and fixed-array or
/// view elements in any order — to its leaf. The leaf's kind is decided by
/// the last segment: a field leaf keeps the field-hole spelling Terminal
/// already admits, and an element leaf selects the element type.
pub(crate) fn lower_window_path<'a>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    root_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    structural_types: &'a [StructuralTypeDeclaration],
) -> Result<(Vec<ProjectionStep>, WindowLeaf<'a>, bool), LoweringError> {
    let (mut field_owner, mut steps) = structural_field_root(root_type, structural_types)?;
    // Owning a descriptor does not own its pointee. Remember crossings while
    // resolving the path, including element views nested in owned records.
    let mut borrowed_backing = !steps.is_empty();
    let mut cursor = path;
    while let Some((segment, rest)) = cursor.split_first() {
        cursor = rest;
        match segment {
            CheckedUnitStructuralPathSegment::Field(identity) if identity.is_valid() => {
                let (terminal_psi::record_or_mixed_shape_pattern!(fields)) = &field_owner.shape
                else {
                    return unsupported("structural window carrier is not a record");
                };
                if rest.is_empty() {
                    // The leaf resolves by identity alone: a scalar leaf is a
                    // window the caller rejects on the leaf's declared type.
                    let mut leafs = fields.iter().filter(|field| {
                        field.identity == identities.intern(&identity.spelling())
                            && !field.relevance.is_erased()
                    });
                    let leaf = leafs.next();
                    if leaf.is_none() || leafs.next().is_some() {
                        return unsupported("structural window leaf is absent or ambiguous");
                    }
                    return Ok((
                        steps,
                        WindowLeaf::Field(leaf.expect("leaf checked above")),
                        borrowed_backing,
                    ));
                }
                let mut carriers = fields.iter().filter(|field| {
                    field.identity == identities.intern(&identity.spelling())
                        && !field.relevance.is_erased()
                        && matches!(field.field_type, StructuralFieldType::Structural(_))
                });
                let carrier = carriers.next();
                if carrier.is_none() || carriers.next().is_some() {
                    return unsupported("structural window carrier is absent or ambiguous");
                }
                let carrier = carrier.expect("carrier presence was checked above");
                steps.push(ProjectionStep::Static(StructuralPathSegment::Field(
                    identities.intern(&identity.spelling()),
                )));
                let Some(nested) = carrier.field_type.as_structural() else {
                    unreachable!("carrier shape was checked above")
                };
                field_owner = terminal_psi::find_structural_type(structural_types, nested).ok_or(
                    LoweringError::Unsupported("structural window nested carrier type is absent"),
                )?;
            }
            CheckedUnitStructuralPathSegment::FixedIndex(_)
            | CheckedUnitStructuralPathSegment::RuntimeIndex(_) => {
                borrowed_backing |=
                    matches!(field_owner.shape, StructuralTypeShape::ElementView { .. });
                let (step, element) = element_selection(field_owner, segment)?;
                if !(matches!(step, ProjectionStep::AssignmentIndex { .. })
                    && steps.contains(&step))
                {
                    steps.push(step);
                }
                field_owner = terminal_psi::find_structural_type(structural_types, element).ok_or(
                    LoweringError::Unsupported("structural window element type is absent"),
                )?;
                if rest.is_empty() {
                    return Ok((
                        steps,
                        WindowLeaf::Subtree {
                            structural_type: element,
                        },
                        borrowed_backing,
                    ));
                }
            }
            CheckedUnitStructuralPathSegment::Referent => {
                borrowed_backing = true;
                let StructuralTypeShape::Reference { referent, .. } = field_owner.shape else {
                    return unsupported("structural window referent crossing names no reference");
                };
                field_owner = terminal_psi::find_structural_type(structural_types, referent)
                    .ok_or(LoweringError::Unsupported(
                        "structural window referent type is absent",
                    ))?;
                steps.push(ProjectionStep::Static(StructuralPathSegment::Referent));
            }
            _ => return unsupported("structural window carrier path is unsupported"),
        }
    }
    Ok((
        steps,
        WindowLeaf::Subtree {
            structural_type: field_owner.id,
        },
        borrowed_backing,
    ))
}

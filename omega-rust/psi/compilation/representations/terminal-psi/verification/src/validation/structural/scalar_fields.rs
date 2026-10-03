//! Scalar-field mutation and observation under exact structural authority.
use crate::validation::{
    ModuleError, OperationId, OperationKind, PlaceId, ScalarType, StructuralAccess,
    StructuralFieldId, StructuralFieldType, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeId, StructuralTypeShape, TerminalMachine, TerminalModule,
};

/// A relevant scalar field stored directly in `structural_type`: a record
/// field, or a mixed shape's common field. A common field occupies the same
/// offset in every case, so writing it needs no case knowledge and leaves the
/// tag and the active payload untouched; payload fields stay unreachable here.
fn direct_relevant_scalar_field(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    permit_bounded_read: bool,
) -> Option<ScalarType> {
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)?;
    let (StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }) =
        &declaration.shape
    else {
        return None;
    };
    fields.iter().find_map(|candidate| {
        (candidate.id == field && !candidate.relevance.is_erased())
            .then(|| scalar_field_type(&candidate.field_type, permit_bounded_read))
            .flatten()
    })
}

fn scalar_field_type(
    field_type: &StructuralFieldType,
    permit_bounded_read: bool,
) -> Option<ScalarType> {
    match field_type {
        StructuralFieldType::Scalar(scalar_type) | StructuralFieldType::Atomic(scalar_type) => {
            Some(*scalar_type)
        }
        StructuralFieldType::BoundedInteger(bounded) if permit_bounded_read => {
            Some(ScalarType::Integer(bounded.integer_type()))
        }
        StructuralFieldType::BoundedInteger(_) => None,
        StructuralFieldType::IeeeFloat(format) => Some(ScalarType::IeeeFloat(*format)),
        StructuralFieldType::ByteSequence(_)
        | StructuralFieldType::PredicateByteSequence { .. }
        | StructuralFieldType::Structural(_)
        | StructuralFieldType::Erased { .. } => None,
    }
}

/// The scalar type a field observation yields below `root_type`: a relevant
/// field of the record `path` reaches, or a relevant payload field of the case
/// a final `Case` step selects. This checks type and backing access. A `Case` step
/// names an overlaid payload that is live only while that case is active, so
/// operation reconstruction separately requires the case's membership fact
/// at the read, exactly as it does for `StructuralCaseLeafCopy`.
fn observed_scalar_field(
    module: &TerminalModule,
    root_type: StructuralTypeId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    field: StructuralFieldId,
    access: StructuralAccess,
) -> Option<ScalarType> {
    let declaration = terminal_semantics::observed_field_declaration(
        module.structural_types.iter(),
        root_type,
        path,
        field,
    )?;
    // A non-atomic snapshot requires exclusive or owned storage. A shared
    // cell must use AtomicAccess, retaining its ordering and event identity.
    if matches!(declaration.field_type, StructuralFieldType::Atomic(_)) {
        let access = terminal_semantics::observed_field_backing_access(
            module.structural_types.iter(),
            root_type,
            path,
            access,
        )?;
        if !matches!(
            access,
            StructuralAccess::Owned | StructuralAccess::MutableBorrow
        ) {
            return None;
        }
    }
    scalar_field_type(&declaration.field_type, true)
}

pub(in crate::validation) fn has_empty_structural_custody(
    machine: &TerminalMachine,
    place: PlaceId,
) -> bool {
    terminal_semantics::arrival_parameter(machine, place).is_some_and(|parameter| {
        parameter.qualifications.is_empty() && parameter.projected_qualifications.is_empty()
    }) && machine
        .entry_claims
        .iter()
        .all(|claim| claim.input != place)
        && machine
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != place)
}

/// Recover the invariant of the exact field observed by a validated read.
/// Construction proves it, entry checks it, and raw bounded-field stores reject.
pub(crate) fn integer_structural_field_read_range(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Option<semantic_vocabulary::BoundedIntegerType> {
    let OperationKind::IntegerStructuralField {
        source,
        ref path,
        field,
    } = operation.kind
    else {
        return None;
    };
    let signature =
        crate::validation::structural::result_contracts::source_signature(machine, source)?;
    // A bounded payload field keeps its declared range: construction proves
    // it for the active case exactly as for a record field.
    let declaration = terminal_semantics::observed_field_declaration(
        module.structural_types.iter(),
        signature.structural_type,
        path,
        field,
    )?;
    let StructuralFieldType::BoundedInteger(bounds) = declaration.field_type else {
        return None;
    };
    (operation.result.scalar_ref()?.scalar_type == ScalarType::Integer(bounds.integer_type()))
        .then_some(bounds)
}

fn has_readable_structural_access(access: StructuralAccess) -> bool {
    matches!(
        access,
        StructuralAccess::Owned | StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    )
}

pub(in crate::validation) fn structural_scalar_field_store_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: OperationId,
    destination: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
) -> Result<ScalarType, ModuleError> {
    let invalid = || ModuleError::InvalidStructuralScalarFieldStore {
        operation,
        destination,
        path: path.to_vec(),
        field,
    };
    let parent_type =
        super::field_stores::parent_type(module, machine, destination, path).ok_or_else(invalid)?;
    direct_relevant_scalar_field(module, parent_type, field, true).ok_or_else(invalid)
}

/// Resolve the declaration's invariant independently of the stored SSA value.
pub(crate) fn structural_scalar_field_store_range(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Option<semantic_vocabulary::BoundedIntegerType> {
    let OperationKind::StructuralScalarFieldStore {
        destination,
        ref path,
        field,
        ..
    } = operation.kind
    else {
        return None;
    };
    let signature =
        crate::validation::structural::result_contracts::source_signature(machine, destination)?;
    let (root_type, path) = crate::validation::references::mutable_record_field_destination(
        module,
        machine,
        destination,
        path,
    )
    .unwrap_or((signature.structural_type, path));
    let parent =
        crate::validation::foundation::resolve_runtime_projection(module, root_type, path)?;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parent)?;
    // The same direct fields `direct_relevant_scalar_field` admits.
    let (StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }) =
        &declaration.shape
    else {
        return None;
    };
    let declaration = fields
        .iter()
        .find(|declaration| declaration.id == field && !declaration.relevance.is_erased())?;
    match declaration.field_type {
        StructuralFieldType::BoundedInteger(bounds) => Some(bounds),
        _ => None,
    }
}

pub(in crate::validation) fn validate_integer_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: OperationId,
    source: PlaceId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    field: StructuralFieldId,
    result_type: ScalarType,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidIntegerStructuralField {
        operation,
        source,
        field,
    };
    // Member reads observe, they do not transfer: a locally constructed
    // record carrying borrowed-view members still answers scalar reads.
    if let Some(result) = crate::validation::record::constructed_source(module, machine, source) {
        if matches!(result_type, ScalarType::Integer(_))
            && observed_scalar_field(
                module,
                result.structural_type,
                path,
                field,
                StructuralAccess::Owned,
            ) == Some(result_type)
        {
            return Ok(());
        }
        return Err(invalid());
    }
    if let Some(parameter) = terminal_semantics::arrival_parameter(machine, source) {
        if !matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) || !has_readable_structural_access(parameter.access)
            || !has_empty_structural_custody(machine, source)
        {
            return Err(invalid());
        }
        let ScalarType::Integer(_) = result_type else {
            return Err(ModuleError::IntegerStructuralFieldRequiresIntegerResult(
                operation,
            ));
        };
        if observed_scalar_field(
            module,
            parameter.structural_type,
            path,
            field,
            parameter.access,
        ) != Some(result_type)
        {
            return Err(invalid());
        }
        return Ok(());
    }
    // A reference-established local is a readable place the same way a
    // borrowed parameter is: `let cell: &Cell = &cells[i]` mints an
    // `EstablishReference` result whose loan's access the argument spells,
    // and field observation through it reads the referent's declared record.
    let (producer, borrowed_result) = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            operation
                .result
                .structural()
                .and_then(|result| (result.place == source).then_some((operation, result)))
        })
        .ok_or_else(invalid)?;
    let access = match &producer.kind {
        OperationKind::EstablishReference { source }
        | OperationKind::EstablishElementView { source, .. }
        | OperationKind::EstablishMutableElementView { source, .. } => Some(source.access),
        // A call result spells the returned loan's access on its `Reference`
        // type shell: `let seen: &Node = self.first()` binds the `&Node`
        // referent through the call's result, and reads through it observe
        // the referent's declared record the same way a borrow-carrier
        // establishment does.
        OperationKind::Call { .. }
        | OperationKind::CallStructural { .. }
        | OperationKind::CallStructuralWithScalarArguments { .. }
        | OperationKind::BoundaryCall { .. } => module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == borrowed_result.structural_type)
            .and_then(|declaration| match &declaration.shape {
                StructuralTypeShape::Reference { access, .. } => Some(*access),
                _ => None,
            }),
        _ => None,
    };
    let empty_result_custody = borrowed_result.qualifications.is_empty()
        && borrowed_result.projected_qualifications.is_empty()
        && machine
            .entry_claims
            .iter()
            .all(|claim| claim.input != source)
        && machine
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != source);
    if !access.is_some_and(has_readable_structural_access)
        || !matches!(
            borrowed_result.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || !empty_result_custody
    {
        return Err(invalid());
    }
    let ScalarType::Integer(_) = result_type else {
        return Err(ModuleError::IntegerStructuralFieldRequiresIntegerResult(
            operation,
        ));
    };
    // A call result's `Reference` shell keeps the loan's access on the type;
    // the observed record is its referent. Borrow-carrier establishments
    // already stamp the peeled referent identity.
    let observed_root = match module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == borrowed_result.structural_type)
        .map(|declaration| &declaration.shape)
    {
        Some(StructuralTypeShape::Reference { referent, .. }) => *referent,
        _ => borrowed_result.structural_type,
    };
    let access = access.ok_or_else(invalid)?;
    if observed_scalar_field(module, observed_root, path, field, access) != Some(result_type) {
        return Err(invalid());
    }
    Ok(())
}

pub(in crate::validation) fn validate_boolean_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: OperationId,
    source: PlaceId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidBooleanStructuralField {
        operation,
        source,
        field,
    };
    if let Some(result) = crate::validation::record::completed_source(module, machine, source) {
        return if observed_scalar_field(
            module,
            result.structural_type,
            path,
            field,
            StructuralAccess::Owned,
        ) == Some(ScalarType::Boolean)
        {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    let parameter = terminal_semantics::arrival_parameter(machine, source).ok_or_else(invalid)?;
    if parameter.access == StructuralAccess::WriteOnlyBorrow {
        return Err(ModuleError::StructuralObservationRequiresReadableAccess { operation, source });
    }
    if !matches!(
        parameter.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !has_readable_structural_access(parameter.access)
        || !has_empty_structural_custody(machine, source)
        || observed_scalar_field(
            module,
            parameter.structural_type,
            path,
            field,
            parameter.access,
        ) != Some(ScalarType::Boolean)
    {
        return Err(invalid());
    }
    Ok(())
}

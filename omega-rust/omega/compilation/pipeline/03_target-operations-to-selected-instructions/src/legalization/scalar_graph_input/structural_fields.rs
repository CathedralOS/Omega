use super::{AbstractOperation, PsiOptimizationFunction, ScalarType};
use optimization_unit_semantics::byte_field_root;
use terminal_psi::find_structural_type;
use terminal_psi::live_field_by_id;

pub(in crate::legalization) fn scalar_store_destination(
    function: &PsiOptimizationFunction,
    destination: &abstract_operations::StructuralScalarStoreDestination,
    path: &[terminal_psi::StructuralPathSegment],
    types: &[terminal_psi::StructuralTypeDeclaration],
    custody: &super::reference_custody::Custody,
) -> Option<(
    abstract_operations::StructuralScalarStoreDestination,
    Vec<terminal_psi::StructuralPathSegment>,
)> {
    if !path.contains(&terminal_psi::StructuralPathSegment::Referent) {
        return Some((destination.clone(), path.to_vec()));
    }
    let resolved = super::reference_custody::referent_destination(
        function,
        custody,
        types,
        destination.place(),
        path,
        terminal_psi::StructuralAccess::WriteOnlyBorrow,
    )?;
    (resolved.carrier_type == destination.structural_type())
        .then(|| (destination.clone(), path.to_vec()))
}

pub(in crate::legalization) fn replacement(
    function: &PsiOptimizationFunction,
    operation: &AbstractOperation,
    types: &[terminal_psi::StructuralTypeDeclaration],
    custody: &super::reference_custody::Custody,
) -> Option<terminal_psi::StructuralArgument> {
    if let AbstractOperation::ByteSequenceStore {
        destination,
        source,
        ..
    } = operation
    {
        let parameter = function.arrival_parameter(*destination)?;
        if source == destination
            || !matches!(
                parameter.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            )
            || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !parameter.projected_qualifications.is_empty()
            || !types.iter().any(|declaration| {
                declaration.id == parameter.structural_type
                    && declaration.shape.is_bounded_owned_byte_sequence()
            })
            || !super::byte_views::contains_readable_bytes(function, *source, types)
            || function
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == *destination || claim.input == *source)
            || function
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == *destination || claim.input.root == *source)
        {
            return None;
        }
        return Some(terminal_psi::StructuralArgument {
            place: *destination,
            access: parameter.access,
            path: Vec::new(),
        });
    }
    let (destination, path, field, source) = match operation {
        AbstractOperation::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } => (destination, path, field, Some(*source)),
        AbstractOperation::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        } => (destination, path, field, None),
        _ => return None,
    };
    let semantic_path = path.clone();
    // Whole-field replacement may address the exact completed owned home.
    // Indexed writes retain their existing borrowed-storage contract.
    let owned_result = source
        .filter(|_| !path.contains(&terminal_psi::StructuralPathSegment::Referent))
        .and_then(|_| super::structural_case::source_result(function, *destination).ok());
    // An owned record, an arrival loan and a returned-reference carrier retain
    // distinct custody. Only the reference route crosses into its referent.
    let (root, root_type, access, path) = if let Some((producer, result)) = owned_result {
        // Replacing an unrestricted field does not copy or dispose the root.
        // Its predicates and affine/linear custody remain on the same owner;
        // reconstruct the exact produced home instead of requiring plain data.
        if !function.structural_places.iter().any(|place| {
            place.id == *destination
                && place.kind
                    == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type: result.structural_type,
                    }
        }) {
            return None;
        }
        (
            *destination,
            result.structural_type,
            terminal_psi::StructuralAccess::Owned,
            path.clone(),
        )
    } else if let Some(parameter) = function.arrival_parameter(*destination) {
        if !(matches!(
            parameter.access,
            terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) || source.is_some() && parameter.access == terminal_psi::StructuralAccess::Owned)
            || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return None;
        }
        (
            parameter.place,
            parameter.structural_type,
            parameter.access,
            path.clone(),
        )
    } else {
        let resolved = super::reference_custody::referent_destination(
            function,
            custody,
            types,
            *destination,
            path,
            terminal_psi::StructuralAccess::MutableBorrow,
        )?;
        (
            *destination,
            resolved.referent,
            terminal_psi::StructuralAccess::MutableBorrow,
            resolved.path,
        )
    };
    let (declared_type, multiplicity, declared_access) =
        if let Some(parameter) = function.arrival_parameter(*destination) {
            (
                parameter.structural_type,
                parameter.multiplicity,
                parameter.access,
            )
        } else {
            let (_, result) = super::structural_case::source_result(function, *destination).ok()?;
            (
                result.structural_type,
                result.multiplicity,
                terminal_psi::StructuralAccess::Owned,
            )
        };
    if !terminal_semantics::reference_source_projection_accessible(
        types.iter(),
        declared_type,
        multiplicity,
        declared_access,
        &semantic_path,
        terminal_psi::StructuralAccess::WriteOnlyBorrow,
    ) || !terminal_psi::is_static_structural_scalar_store_path(&path)
        || source == Some(*destination)
        || function
            .entry_claim_declarations
            .iter()
            .any(|claim| claim.input == root || Some(claim.input) == source)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == root || Some(claim.input.root) == source)
        || source.is_some_and(|source| {
            !super::byte_views::contains_readable_bytes(function, source, types)
        })
    {
        return None;
    }
    crate::structural_inputs::structural_reference_input::byte_field_storage(
        root_type, &path, *field, types,
    )?;
    Some(terminal_psi::StructuralArgument {
        place: root,
        access,
        path: semantic_path,
    })
}

/// The exclusive root a shared view field's descriptor replacement writes,
/// rejoined like a bounded field replacement. The descriptor geometry must
/// resolve, and the source must be an established two-word descriptor. Current
/// graph ownership replay separately proves its backing survives the stored view.
pub(in crate::legalization) fn view_replacement(
    function: &PsiOptimizationFunction,
    operation: &AbstractOperation,
    types: &[terminal_psi::StructuralTypeDeclaration],
    custody: &super::reference_custody::Custody,
) -> Option<terminal_psi::StructuralArgument> {
    let AbstractOperation::StructuralByteViewFieldStore {
        destination,
        path,
        field,
        source,
        ..
    } = operation
    else {
        return None;
    };
    // A returned reference owns its carrier while lending mutable authority
    // to this path, so custody must resolve the crossing before declaration.
    let (root_type, access, projected_path) = if field.is_none() {
        // The whole slot is a declared descriptor home, whose own
        // establishment is its write authority.
        let home = optimization_unit_semantics::declared_descriptor_home(function, *destination)?;
        (
            home.structural_type,
            terminal_psi::StructuralAccess::Owned,
            path.clone(),
        )
    } else if let Some(resolved) = super::reference_custody::referent_destination(
        function,
        custody,
        types,
        *destination,
        path,
        terminal_psi::StructuralAccess::MutableBorrow,
    ) {
        (
            resolved.referent,
            terminal_psi::StructuralAccess::MutableBorrow,
            resolved.path,
        )
    } else {
        let (root, access) = byte_field_root(function, *destination)?;
        if !matches!(
            access,
            terminal_psi::StructuralAccess::Owned
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) {
            return None;
        }
        (root, access, path.clone())
    };
    if source == destination
        || !super::byte_views::view_type(function, *source, types).is_some_and(|identity| {
            types
                .iter()
                .any(|row| row.id == identity && row.shape.is_borrowed_byte_view())
        })
        || function
            .entry_claim_declarations
            .iter()
            .any(|claim| claim.input == *destination)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == *destination)
    {
        return None;
    }
    crate::structural_inputs::structural_reference_input::projected_view_slot(
        root_type,
        &projected_path,
        *field,
        types,
    )?;
    Some(terminal_psi::StructuralArgument {
        place: *destination,
        access,
        path: path.clone(),
    })
}

pub(in crate::legalization) fn byte_index_operands(
    function: &PsiOptimizationFunction,
    unit: &optimization_unit::PsiOptimizationUnit,
    operation: semantic_vocabulary::OperationId,
    source: &terminal_psi::StructuralArgument,
    field: semantic_vocabulary::StructuralFieldId,
) -> Result<Vec<legalized_operations::LegalizedRuntimeIndexOperand>, crate::LegalizationError> {
    let geometry = byte_geometry(function, source, field, &unit.structural_types)
        .ok_or(crate::LegalizationError::custody())?;
    crate::legalization::runtime_indices::operands(function, unit, operation, &geometry.elements)
}

/// The selector operands of one descriptor slot store, rejoined against the
/// slot geometry its destination argument resolves to.
pub(in crate::legalization) fn view_slot_index_operands(
    function: &PsiOptimizationFunction,
    unit: &optimization_unit::PsiOptimizationUnit,
    operation: semantic_vocabulary::OperationId,
    destination: &terminal_psi::StructuralArgument,
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Result<Vec<legalized_operations::LegalizedRuntimeIndexOperand>, crate::LegalizationError> {
    let geometry = view_slot_geometry(function, destination, field, &unit.structural_types)
        .ok_or(crate::LegalizationError::custody())?;
    crate::legalization::runtime_indices::operands(function, unit, operation, &geometry.elements)
}

/// The descriptor slot below one store destination's current root.
pub(in crate::legalization) fn view_slot_geometry(
    function: &PsiOptimizationFunction,
    destination: &terminal_psi::StructuralArgument,
    field: Option<semantic_vocabulary::StructuralFieldId>,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<crate::structural_inputs::structural_reference_input::ProjectedByteField> {
    let Some(field) = field else {
        let home =
            optimization_unit_semantics::declared_descriptor_home(function, destination.place)?;
        return (destination.access == terminal_psi::StructuralAccess::Owned)
            .then(|| {
                crate::structural_inputs::structural_reference_input::projected_view_slot(
                    home.structural_type,
                    &destination.path,
                    None,
                    types,
                )
            })
            .flatten();
    };
    byte_geometry(function, destination, field, types)
}

pub(in crate::legalization) fn byte_geometry(
    function: &PsiOptimizationFunction,
    source: &terminal_psi::StructuralArgument,
    field: semantic_vocabulary::StructuralFieldId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<crate::structural_inputs::structural_reference_input::ProjectedByteField> {
    let (root, access) = byte_field_root(function, source.place)?;
    if access != source.access {
        return None;
    }
    crate::structural_inputs::structural_reference_input::projected_byte_field(
        root,
        &source.path,
        field,
        types,
    )
}

/// The bounded byte field one indexed byte read observes, as the structural
/// argument selection reconstructs its storage from.
pub(in crate::legalization) fn byte_read(
    function: &PsiOptimizationFunction,
    operation: &AbstractOperation,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<terminal_psi::StructuralArgument> {
    let AbstractOperation::StructuralByteSequenceFieldRead {
        result,
        source,
        path,
        field,
        ..
    } = operation
    else {
        return None;
    };
    let (structural_type, access) = byte_field_root(function, *source)?;
    // A write-only borrow may observe the live length but not the bytes.
    if access == terminal_psi::StructuralAccess::WriteOnlyBorrow
        || result.scalar_type
            != ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    8,
                )
                .ok()?,
            )
    {
        return None;
    }
    crate::structural_inputs::structural_reference_input::projected_byte_field(
        structural_type,
        path,
        *field,
        types,
    )?;
    Some(terminal_psi::StructuralArgument {
        place: *source,
        access,
        path: path.clone(),
    })
}

/// One scalar field observation's legal shape: the defining operation and
/// result, the runtime carrier, the declared case whose payload holds the
/// field (absent for a record field), and the observed field.
pub(in crate::legalization) type FieldObservation = (
    semantic_vocabulary::OperationId,
    abstract_operations::AbstractResult,
    terminal_psi::StructuralArgument,
    Option<semantic_vocabulary::StructuralCaseId>,
    semantic_vocabulary::StructuralFieldId,
);

pub(in crate::legalization) fn read(
    function: &PsiOptimizationFunction,
    operation: &AbstractOperation,
    types: &[terminal_psi::StructuralTypeDeclaration],
    custody: &super::reference_custody::Custody,
) -> Option<FieldObservation> {
    if let AbstractOperation::StructuralByteSequenceFieldLength {
        psi_operation,
        result,
        source,
        path,
        field,
    } = operation
    {
        let (structural_type, access) = byte_field_root(function, *source)?;
        if result.scalar_type
            != ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .ok()?,
            )
        {
            return None;
        }
        crate::structural_inputs::structural_reference_input::projected_byte_field(
            structural_type,
            path,
            *field,
            types,
        )?;
        return Some((
            *psi_operation,
            *result,
            terminal_psi::StructuralArgument {
                place: *source,
                access,
                path: path.clone(),
            },
            None,
            *field,
        ));
    }
    let AbstractOperation::StructuralScalarFieldRead {
        psi_operation: operation,
        result,
        source: place,
        path,
        field,
    } = operation
    else {
        return None;
    };
    let (operation, result, place, field) = (*operation, *result, *place, *field);
    // A scalar snapshot of a content-claimed root neither consumes nor
    // discharges the claim; lowering already admitted the read under the
    // owned-observation rule, and this replay only rejoins its geometry.
    // Read the selected address in a reference home, never an arbitrary
    // candidate from the independently reconstructed origin set.
    let mut through_reference = false;
    // Access belongs to the exact declaration. Owned input observations retain
    // the value ABI; selection must supply captured value storage, not reinterpret
    // an incoming value fragment as the address used by a borrowed parameter.
    let (structural_type, access) = if let Some(parameter) = function
        .structural_parameters
        .iter()
        .chain(function.block_structural_parameters())
        .find(|parameter| parameter.place == place)
    {
        if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
            return None;
        }
        (parameter.structural_type, parameter.access)
    } else {
        let (_, result) = super::structural_case::source_result(function, place).ok()?;
        // A view or reference lends storage through its descriptor; an owned
        // aggregate instead observes its own value storage.
        let (structural_type, access) = match find_structural_type(types, result.structural_type)
            .map(|declaration| &declaration.shape)
        {
            Some(terminal_psi::StructuralTypeShape::ElementView { .. }) => (
                result.structural_type,
                terminal_psi::StructuralAccess::SharedBorrow,
            ),
            Some(terminal_psi::StructuralTypeShape::Reference {
                referent,
                access:
                    access @ (terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow),
            }) => {
                let leaf = custody.readable_leaf(place, &[])?;
                if !leaf.matches_operation_result(result) || leaf.referent != *referent {
                    return None;
                }
                through_reference = true;
                (*referent, *access)
            }
            _ => (
                result.structural_type,
                terminal_psi::StructuralAccess::Owned,
            ),
        };
        (structural_type, access)
    };
    let access = terminal_semantics::observed_field_backing_access(
        types.iter(),
        structural_type,
        path,
        access,
    )?;
    let declaration =
        terminal_semantics::observed_field_declaration(types.iter(), structural_type, path, field)?;
    if matches!(
        declaration.field_type,
        terminal_psi::StructuralFieldType::Atomic(_)
    ) && !matches!(
        access,
        terminal_psi::StructuralAccess::Owned | terminal_psi::StructuralAccess::MutableBorrow
    ) {
        return None;
    }
    // A final `Case` step selects which payload overlay holds the field. It
    // names no storage of its own: the payload lives at the sum's place, so
    // the runtime carrier stops at the sum and the case travels beside it.
    let (carrier_path, case) = match path.split_last() {
        Some((semantic_vocabulary::CanonicalStructuralPathSegment::Case(case), prefix)) => {
            (prefix, Some(*case))
        }
        _ => (path.as_slice(), None),
    };
    let mut runtime_path = runtime_path(structural_type, carrier_path, types)?;
    match case {
        None => crate::structural_inputs::structural_reference_input::field_read(
            structural_type,
            &runtime_path,
            field,
            result.scalar_type,
            types,
        )?,
        Some(case) => crate::structural_inputs::structural_reference_input::case_field_read(
            structural_type,
            &runtime_path,
            case,
            field,
            result.scalar_type,
            types,
        )?,
    };
    if through_reference {
        runtime_path.insert(0, terminal_psi::StructuralPathSegment::Referent);
    }
    let source = terminal_psi::StructuralArgument {
        place,
        access,
        path: runtime_path,
    };
    Some((operation, result, source, case, field))
}

/// The runtime spelling of a canonical projection beneath `root`: each field
/// by its declared identity, and fixed-array indices bounded at every hop.
/// Element-view indexing retains its terminal-segment admission.
fn runtime_path(
    root: semantic_vocabulary::StructuralTypeId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<Vec<terminal_psi::StructuralPathSegment>> {
    let mut carrier = root;
    let mut runtime_path = Vec::with_capacity(path.len());
    for (position, segment) in path.iter().enumerate() {
        match segment {
            semantic_vocabulary::CanonicalStructuralPathSegment::Field(field) => {
                let fields = find_structural_type(types, carrier)?
                    .shape
                    .direct_fields()?;
                let selected = live_field_by_id(fields, *field)?;
                let child = match &selected.field_type {
                    terminal_psi::StructuralFieldType::Structural(child) => *child,
                    leaf => {
                        leaf.canonical_leaf_type()?
                            .leaf_declaration(types.iter())?
                            .id
                    }
                };
                runtime_path.push(terminal_psi::StructuralPathSegment::Field(
                    selected.identity.clone(),
                ));
                carrier = child;
            }
            // Replay each fixed extent even when a later field continues
            // inside the element; the producer's offset is not authority.
            semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index) => {
                let element = match &find_structural_type(types, carrier)?.shape {
                    terminal_psi::StructuralTypeShape::FixedArray { element, length } => {
                        if index >= length {
                            return None;
                        }
                        element
                    }
                    // An element view's live extent is the bound proven by
                    // its producer; the index names one member.
                    terminal_psi::StructuralTypeShape::ElementView { element, .. }
                        if position + 1 == path.len() =>
                    {
                        element
                    }
                    _ => return None,
                };
                runtime_path.push(terminal_psi::StructuralPathSegment::FixedIndex(*index));
                carrier = *element;
            }
            _ => return None,
        }
    }
    Some(runtime_path)
}

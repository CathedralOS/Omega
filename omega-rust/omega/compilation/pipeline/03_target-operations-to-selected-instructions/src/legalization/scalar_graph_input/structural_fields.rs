use super::{AbstractOperation, PsiOptimizationFunction, ScalarType};

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
    let abstract_operations::StructuralScalarStoreDestination::OwnedResult { producer, result } =
        destination
    else {
        return Some((destination.clone(), path.to_vec()));
    };
    if !matches!(
        path.first(),
        Some(terminal_psi::StructuralPathSegment::Referent)
    ) {
        return Some((destination.clone(), path.to_vec()));
    }
    let resolved = super::reference_custody::writable_referent_destination(
        custody,
        function,
        types,
        result.place,
        path,
    )?;
    (resolved.producer == *producer && resolved.result == *result)
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
        let parameter = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == *destination)?;
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
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
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
    // Whole-field replacement may address the exact completed owned record.
    // Indexed writes retain their existing borrowed-storage contract.
    let owned_result = source.and_then(|_| {
        super::structural_case::source_result(function, *destination)
            .ok()
            .filter(|(_, result)| {
                types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                        && matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::Record { .. }
                        )
                })
            })
    });
    let (root, root_type, access, path) = if let Some((producer, result)) = owned_result {
        if result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
            || !function.structural_places.iter().any(|place| {
                place.id == *destination
                    && place.kind
                        == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type: result.structural_type,
                        }
            })
        {
            return None;
        }
        (
            *destination,
            result.structural_type,
            terminal_psi::StructuralAccess::Owned,
            path.clone(),
        )
    } else if let Some(parameter) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == *destination)
    {
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
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
        let resolved = super::reference_custody::writable_referent_destination(
            custody,
            function,
            types,
            *destination,
            path,
        )?;
        (
            *destination,
            resolved.referent,
            terminal_psi::StructuralAccess::MutableBorrow,
            resolved.path,
        )
    };
    if !terminal_psi::is_bounded_structural_scalar_store_path(&path)
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
/// resolve, and the source must be an established literal: the verifier
/// admitted no other view as outliving the destination root.
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
    let (root_type, access, projected_path) = if let Some(parameter) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == *destination)
    {
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return None;
        }
        (parameter.structural_type, parameter.access, path.clone())
    } else {
        let resolved = super::reference_custody::writable_referent_destination(
            custody,
            function,
            types,
            *destination,
            path,
        )?;
        (
            resolved.referent,
            terminal_psi::StructuralAccess::MutableBorrow,
            resolved.path,
        )
    };
    if source == destination
        || super::literals::declaration_producer(function, *source).is_none()
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
    crate::structural_inputs::structural_reference_input::byte_view_field_descriptor(
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

/// Rejoin byte-field observations to their original parameter or exact local
/// producer. Whole-unit custody separately checks availability; neither the
/// observation nor its field geometry creates storage or borrowed authority.
fn byte_field_root(
    function: &PsiOptimizationFunction,
    source: semantic_vocabulary::PlaceId,
) -> Option<(
    semantic_vocabulary::StructuralTypeId,
    terminal_psi::StructuralAccess,
)> {
    if function
        .entry_claim_declarations
        .iter()
        .any(|claim| claim.input == source)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == source)
    {
        return None;
    }
    if let Some(parameter) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)
    {
        return (matches!(
            parameter.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) && parameter.multiplicity != terminal_psi::StructuralMultiplicity::Linear
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty())
        .then_some((parameter.structural_type, parameter.access));
    }
    let (producer, result) = super::structural_case::source_result(function, source).ok()?;
    if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !function.structural_places.iter().any(|place| {
            place.id == source
                && place.kind
                    == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type: result.structural_type,
                    }
        })
    {
        return None;
    }
    Some((
        result.structural_type,
        terminal_psi::StructuralAccess::Owned,
    ))
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
    // A bounded owner holds its bytes inline; a shared view field holds a
    // descriptor whose pointer the read follows.
    crate::structural_inputs::structural_reference_input::byte_field_storage(
        structural_type,
        path,
        *field,
        types,
    )
    .map(|_| ())
    .or_else(|| {
        crate::structural_inputs::structural_reference_input::byte_view_field_descriptor(
            structural_type,
            path,
            *field,
            types,
        )
        .map(|_| ())
    })?;
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
        crate::structural_inputs::structural_reference_input::byte_field_length(
            structural_type,
            path,
            *field,
            result.scalar_type,
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
    if function
        .entry_claim_declarations
        .iter()
        .any(|claim| claim.input == place)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == place)
    {
        return None;
    }
    // A field read spelled on a live `&` carrier rebases onto the referent
    // root the custody names — the carrier never materializes its own
    // pointer — resolving `(place, access, path prefix)` for the argument.
    let mut rebased: Option<(
        semantic_vocabulary::PlaceId,
        terminal_psi::StructuralAccess,
        Vec<terminal_psi::StructuralPathSegment>,
        semantic_vocabulary::StructuralTypeId,
    )> = None;
    // Access belongs to the exact declaration. Owned input observations retain
    // the value ABI; selection must supply captured value storage, not reinterpret
    // an incoming value fragment as the address used by a borrowed parameter.
    let (structural_type, access) = if let Some(parameter) = function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == place)
    {
        if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return None;
        }
        (parameter.structural_type, parameter.access)
    } else {
        let (_, result) = super::structural_case::source_result(function, place).ok()?;
        if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return None;
        }
        // An element view's descriptor is the read's carrier, not the
        // observed storage: a view element read borrows through the live
        // extent, an owned result reads its own value storage. A live `&`
        // loan carrier rebases onto the referent root its custody names; the
        // referent declaration resolves the field geometry either way.
        let (structural_type, access) = match types
            .iter()
            .find(|declaration| declaration.id == result.structural_type)
            .map(|declaration| &declaration.shape)
        {
            Some(terminal_psi::StructuralTypeShape::ElementView { .. }) => (
                result.structural_type,
                terminal_psi::StructuralAccess::SharedBorrow,
            ),
            Some(terminal_psi::StructuralTypeShape::Reference {
                referent,
                access:
                    terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow,
            }) => {
                let leaf = custody.leaf(place, &[])?;
                let (root_place, root_path) = match &leaf.root {
                    super::reference_custody::Root::Place(place) => (*place, Vec::new()),
                    super::reference_custody::Root::Ingress(place, path) => (*place, path.clone()),
                    super::reference_custody::Root::Alternatives(..) => return None,
                };
                let root_contract = function
                    .structural_parameters
                    .iter()
                    .chain(
                        function
                            .blocks
                            .iter()
                            .flat_map(|block| &block.structural_parameters),
                    )
                    .find(|parameter| parameter.place == root_place)
                    .map(|parameter| (parameter.structural_type, parameter.access))
                    .or_else(|| {
                        super::structural_case::source_result(function, root_place)
                            .ok()
                            .map(|(_, result)| {
                                (
                                    result.structural_type,
                                    terminal_psi::StructuralAccess::Owned,
                                )
                            })
                    })?;
                rebased = Some((root_place, root_contract.1, root_path, root_contract.0));
                (*referent, root_contract.1)
            }
            _ => (
                result.structural_type,
                terminal_psi::StructuralAccess::Owned,
            ),
        };
        (structural_type, access)
    };
    // A final `Case` step selects which payload overlay holds the field. It
    // names no storage of its own: the payload lives at the sum's place, so
    // the runtime carrier stops at the sum and the case travels beside it.
    let (carrier_path, case) = match path.split_last() {
        Some((semantic_vocabulary::CanonicalStructuralPathSegment::Case(case), prefix)) => {
            (prefix, Some(*case))
        }
        _ => (path.as_slice(), None),
    };
    let runtime_path = runtime_path(structural_type, carrier_path, types)?;
    let (source, observed) = match rebased {
        Some((root_place, root_access, mut root_path, root_type)) => {
            root_path.extend(runtime_path.iter().cloned());
            (
                terminal_psi::StructuralArgument {
                    place: root_place,
                    access: root_access,
                    path: root_path,
                },
                root_type,
            )
        }
        None => (
            terminal_psi::StructuralArgument {
                place,
                access,
                path: runtime_path,
            },
            structural_type,
        ),
    };
    match case {
        None => crate::structural_inputs::structural_reference_input::field_read(
            observed,
            &source.path,
            field,
            result.scalar_type,
            types,
        )?,
        Some(case) => crate::structural_inputs::structural_reference_input::case_field_read(
            observed,
            &source.path,
            case,
            field,
            result.scalar_type,
            types,
        )?,
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
                let terminal_psi::StructuralTypeShape::Record { fields } = &types
                    .iter()
                    .find(|declaration| declaration.id == carrier)?
                    .shape
                else {
                    return None;
                };
                let selected = fields
                    .iter()
                    .find(|candidate| candidate.id == *field && !candidate.relevance.is_erased())?;
                let child = match &selected.field_type {
                    terminal_psi::StructuralFieldType::Structural(child) => *child,
                    leaf => {
                        let shape = leaf.canonical_leaf_shape()?;
                        types
                            .iter()
                            .find(|declaration| declaration.shape == shape)
                            .map(|declaration| declaration.id)?
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
                let element = match &types
                    .iter()
                    .find(|declaration| declaration.id == carrier)?
                    .shape
                {
                    terminal_psi::StructuralTypeShape::FixedArray { element, length } => {
                        if index >= length {
                            return None;
                        }
                        element
                    }
                    // An element view's live extent is the bound proven by
                    // its producer; the index names one member.
                    terminal_psi::StructuralTypeShape::ElementView { element }
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

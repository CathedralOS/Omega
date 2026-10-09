use super::*;

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_structural_case_membership(
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::StructuralCaseMembership { .. } = &operation.kind else {
        unreachable!("dispatched validate_structural_case_membership")
    };
    if operation.result.scalar().is_none_or(|result| {
        result.scalar_type != ScalarType::Boolean || !result.qualifications.is_empty()
    }) {
        return malformed("case membership requires an unqualified Boolean result");
    }
    // Full module validation independently checks the source's nominal
    // owner, readable access, establishment order and live custody.
    Ok(())
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_structural_leaf_copy(
    module: &TerminalModule,
    operation: &Operation,
) -> Result<(), CodecError> {
    let (OperationKind::StructuralLeafCopy { .. } | OperationKind::StructuralCaseLeafCopy { .. }) =
        &operation.kind
    else {
        unreachable!("dispatched validate_structural_leaf_copy")
    };
    if operation.result.structural().is_none_or(|result| {
        (result.multiplicity != StructuralMultiplicity::Unrestricted
            // Descriptor disposal is affine even though shared access is
            // copyable. The copy retains backing and parent obligations;
            // ordinary affine payloads still require an owned transfer.
            && !(result.multiplicity == StructuralMultiplicity::Affine
                && module.structural_types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                        && matches!(declaration.shape, StructuralTypeShape::Reference {
                            access: terminal_psi::StructuralAccess::SharedBorrow, ..
                        })
                })))
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
    }) {
        return malformed("leaf copy has invalid multiplicity or qualification custody");
    }
    // Full module validation independently checks the source's readable
    // access, the resolved leaf type, custody and liveness.
    Ok(())
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_write_only_primitive_store(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::WriteOnlyPrimitiveStore {
        destination,
        value,
        path,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_write_only_primitive_store")
    };
    if operation.result != OperationResult::Unit {
        return malformed("write-only primitive store declares a non-Unit result");
    }
    // Entry and block parameters carry the same storage authority. A path
    // selects inside an aggregate; it does not select a different parameter
    // lookup or permit an undeclared destination.
    let projected = !path.is_empty();
    let destination_type = if let Some(parameter) = machine.structural_parameter(*destination) {
        let declaration_matches = machine.structural_places.iter().any(|place| {
            if place.id != *destination {
                return false;
            }
            match place.kind {
                StructuralPlaceKind::Parameter { position } => {
                    position == parameter.position
                        && machine.structural_parameters.contains(parameter)
                }
                StructuralPlaceKind::BlockParameter { block, position } => {
                    position == parameter.position
                        && machine
                            .block(&block)
                            .and_then(|block| block.structural_parameters.get(position as usize))
                            == Some(parameter)
                }
                _ => false,
            }
        });
        let writable = matches!(
            parameter.access,
            terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) || (projected
            && parameter.access == terminal_psi::StructuralAccess::Owned);
        let multiplicity = parameter.multiplicity == StructuralMultiplicity::Unrestricted
            || (projected && parameter.multiplicity == StructuralMultiplicity::Affine);
        if !writable
            || !multiplicity
            || !declaration_matches
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == *destination)
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == *destination)
        {
            return malformed("write-only primitive store has invalid destination custody");
        }
        parameter.structural_type
    } else {
        let Some(result) = machine
            .operations()
            .filter(|producer| {
                projected || matches!(producer.kind, OperationKind::EstablishPrimitiveLocal { .. })
            })
            .filter_map(|producer| producer.result.structural())
            .find(|result| {
                result.place == *destination
                    && result.claims.is_empty()
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && (result.multiplicity == StructuralMultiplicity::Unrestricted
                        || (projected && result.multiplicity == StructuralMultiplicity::Affine))
            })
        else {
            return malformed(
                "primitive store destination is neither a parameter nor an initialized local",
            );
        };
        // Each producer validates its exact operation-result declaration;
        // independent verification checks dominance and reference custody.
        if machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == *destination)
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == *destination)
        {
            return malformed("primitive local store cannot carry entry claims");
        }
        result.structural_type
    };
    let Some(expected) = terminal_semantics::primitive_projection_type(
        module.structural_types.iter(),
        destination_type,
        path,
    ) else {
        return malformed("write-only primitive store requires a primitive-scalar root");
    };
    let actual = declared_scalar_type(machine, *value);
    if actual != Some(expected) {
        return malformed("write-only primitive store value type does not match referent");
    }
    Ok(())
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_structural_scalar_field_store(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::StructuralScalarFieldStore {
        destination,
        path,
        field,
        value,
        range_obligation,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_structural_scalar_field_store")
    };
    if operation.result != OperationResult::Unit {
        return malformed("structural scalar field store declares a non-Unit result");
    }
    let parent_type = field_store_parent(module, machine, *destination, path)?;
    let Some(expected) =
        module
            .structural_type(parent_type)
            .and_then(|declaration| match &declaration.shape {
                // A mixed shape's common fields are direct fields at one offset
                // in every case; payload fields stay out of reach of this store.
                terminal_psi::record_or_mixed_shape_pattern!(fields) => {
                    fields.iter().find_map(|candidate| {
                        (candidate.id == *field && !candidate.relevance.is_erased())
                            .then_some(&candidate.field_type)
                            .and_then(|field_type| match field_type {
                                StructuralFieldType::Scalar(scalar_type)
                                    if range_obligation.is_none() =>
                                {
                                    Some(*scalar_type)
                                }
                                field_type
                                    if terminal_semantics::scalar_field_requires_evidence(
                                        field_type,
                                    ) && range_obligation.is_some() =>
                                {
                                    field_type.scalar_type()
                                }
                                StructuralFieldType::IeeeFloat(format)
                                    if range_obligation.is_none() =>
                                {
                                    Some(semantic_vocabulary::ScalarType::IeeeFloat(*format))
                                }
                                _ => None,
                            })
                    })
                }
                _ => None,
            })
    else {
        return malformed("structural scalar field store does not select a relevant scalar field");
    };
    let actual = declared_scalar_type(machine, *value);
    if actual != Some(expected) {
        return malformed("structural scalar field store value type does not match field");
    }
    Ok(())
}

/// Borrowed-storage restoration window moves: `MoveStructuralField` opens the
/// window on one declared structural field beneath a mutable-borrowed root and
/// `StoreStructuralField` reseats exactly that field. Foundation validation
/// proves the field's declared type and the result/stored value's declared type
/// agree and that the root carries mutable-borrow authority; the window's
/// opening, exact repair, and non-crash-exit closure belong to ordered
/// verification, and the moved subtree's custody transfer belongs to the
/// interpreter.
pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_move_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::MoveStructuralField {
        source,
        path,
        field,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_move_structural_field")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("structural field move has no structural result");
    };
    if !result.qualifications.is_empty() || !result.projected_qualifications.is_empty() {
        return malformed("structural field move result carries unattested qualifications");
    }
    // A moved value retains its declared linear frontier. Result claim-path
    // structure is checked by the common result foundation; current custody
    // and its transfer are not a second storage-shape classification here.
    let Some(root_type) = window_root_type(module, machine, *source, path, *field) else {
        return malformed("structural field move requires a mutable-borrowed parameter");
    };
    let field_type = window_field_type(module, root_type, path, *field)?;
    if result.structural_type != field_type {
        return malformed("structural field move result type does not match field");
    }
    Ok(())
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_store_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::StoreStructuralField {
        destination,
        path,
        field,
        value,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_store_structural_field")
    };
    if operation.result != OperationResult::Unit {
        return malformed("structural field store declares a non-Unit result");
    }
    if value.access != terminal_psi::StructuralAccess::Owned || !value.path.is_empty() {
        return malformed("structural field store value must be a whole owned place");
    }
    let Some(root_type) = window_root_type(module, machine, *destination, path, *field) else {
        return malformed("structural field store requires a mutable-borrowed parameter");
    };
    let field_type = window_field_type(module, root_type, path, *field)?;
    if structural_place_type(machine, value.place) != Some(field_type) {
        return malformed("structural field store value type does not match field");
    }
    Ok(())
}

/// The declared root type a window operation may name. A field window may
/// open beneath a mutable-borrowed parameter or a completed owned home; a
/// whole-root window needs the original exclusive parameter loan. A reference
/// crossing without a following child selector still names a whole referent.
/// The declared scalar type of a value anywhere it may be declared on the
/// machine — machine parameters, the result, block parameters and operation
/// results.
fn declared_scalar_type(
    machine: &TerminalMachine,
    value: semantic_vocabulary::ValueId,
) -> Option<ScalarType> {
    machine
        .parameters
        .iter()
        .chain(machine.result.scalar_ref())
        .chain(machine.block_parameters())
        .chain(machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|candidate| candidate.result.scalar_ref())
        }))
        .find(|declaration| declaration.id == value)
        .map(|declaration| declaration.scalar_type)
}

fn window_root_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    if !terminal_semantics::structural_window_selects_child(path, field) {
        if path.is_empty() {
            return mutable_borrowed_parameter_type(machine, place);
        }
        // Serialization proves the carrier and referent geometry. Ordered
        // custody must additionally authenticate the reference's backing as
        // exclusive parameter storage, with no live descendants.
        if path.first() != Some(&terminal_psi::StructuralPathSegment::Referent) {
            return None;
        }
        let root_type = borrowed_field_store_root_type(machine, place)?;
        module
            .structural_types
            .iter()
            .any(|declaration| {
                declaration.id == root_type
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::Reference {
                            access: terminal_psi::StructuralAccess::MutableBorrow,
                            ..
                        }
                    )
            })
            .then_some(root_type)
    } else {
        borrowed_field_store_root_type(machine, place)
    }
}

/// The declared type of the root storage a window operation may name: either
/// a declared mutable-borrowed machine/block parameter or a completed owned
/// home. Block bindings retain the same declared storage shape; reconstructing
/// their incoming authority, alias interference, and exact restoration debt
/// belongs to ordered verification, not this serialization foundation check.
fn borrowed_field_store_root_type(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .chain(machine.block_structural_parameters())
        .find(|parameter| parameter.place == place)
        && matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned | terminal_psi::StructuralAccess::MutableBorrow
        )
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && (parameter.access == terminal_psi::StructuralAccess::Owned
            || parameter.is_unqualified_root())
    {
        return Some(parameter.structural_type);
    }
    machine
        .operations()
        .filter_map(|producer| producer.result.structural())
        .find(|result| {
            result.place == place
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
        })
        .map(|result| result.structural_type)
}

/// A declared mutable-borrowed machine or block parameter with transferable
/// multiplicity and no qualifications or entry claims.
fn mutable_borrowed_parameter_type(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    machine
        .structural_parameters
        .iter()
        .chain(machine.block_structural_parameters())
        .find(|parameter| {
            parameter.place == place
                && parameter.access == terminal_psi::StructuralAccess::MutableBorrow
                && matches!(
                    parameter.multiplicity,
                    StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                )
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && !machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == place)
                && !machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == place)
        })
        .map(|parameter| parameter.structural_type)
}

/// Resolve the same exact storage projection the operation encodes. Bounds
/// evidence and restoration custody remain independent verifier obligations.
fn window_field_type(
    module: &TerminalModule,
    root_type: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Result<semantic_vocabulary::StructuralTypeId, CodecError> {
    let Some(projection) = terminal_semantics::structural_window_projection(
        module.structural_types.iter(),
        root_type,
        path,
        field,
    ) else {
        return malformed("structural field move does not select a structural field");
    };
    Ok(projection.structural_type)
}

/// Observing owned aggregate storage retains its full declared custody;
/// reference/view carriers keep their separate loan-access contract.
fn owns_observed_storage(
    module: &TerminalModule,
    structural_type: semantic_vocabulary::StructuralTypeId,
    access: terminal_psi::StructuralAccess,
) -> bool {
    access == terminal_psi::StructuralAccess::Owned
        && module.structural_types.iter().any(|declaration| {
            declaration.id == structural_type && (declaration.shape).is_composite_shape()
        })
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_scalar_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let (OperationKind::ScalarStructuralField {
        source,
        path,
        field,
    }
    | OperationKind::BooleanStructuralField {
        source,
        path,
        field,
    }) = &operation.kind
    else {
        unreachable!("dispatched validate_scalar_structural_field")
    };
    let Some(result) = operation.result.scalar_ref() else {
        return malformed("scalar structural field has no scalar result");
    };
    if !matches!(
        (&operation.kind, result.scalar_type),
        (
            OperationKind::ScalarStructuralField { .. },
            ScalarType::Integer(_) | ScalarType::IeeeFloat(_)
        ) | (
            OperationKind::BooleanStructuralField { .. },
            ScalarType::Boolean
        )
    ) {
        return malformed("scalar structural field has an invalid result type");
    }
    // A constructed/call-result record remains a local result, not a
    // synthetic parameter. The verifier separately checks its producer,
    // dominance, live ownership and loans at this observation.
    if let Some((producer, local)) = machine
        .operations()
        .filter(|producer| {
            matches!(
                producer.kind,
                OperationKind::EstablishStructuralArray { .. }
                    | OperationKind::EstablishRecord { .. }
                    | OperationKind::EstablishZeroedStructural { .. }
                    | OperationKind::EstablishStructuralLocal { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
                    // A boundary call's result is the same class of readable
                    // observation source: the call's declared result custody
                    // is the local's own, as it is for an in-module call.
                    | OperationKind::BoundaryCall { .. }
                    | OperationKind::StructuralLeafCopy { .. }
                    | OperationKind::StructuralCaseLeafCopy { .. }
                    // A loan carrier is equally a readable observation source:
                    // `&cells[i]` mints its referent storage through the
                    // reference establishment, and a view local reads its
                    // element fields through the view descriptor the same way
                    // the store arm admits it as a write destination.
                    | OperationKind::EstablishReference { .. }
                    | OperationKind::EstablishElementView { .. }
                    | OperationKind::EstablishMutableElementView { .. }
            )
        })
        .filter_map(|producer| producer.result.structural().map(|local| (producer, local)))
        .find(|(_, local)| local.place == *source)
    {
        // The observed field belongs to a record carrier or to the payload of
        // the case a final `Case` step selects; which case is active is the
        // verifier's reconstruction judgment, not a decode-time shape.
        // A call result's `Reference` shell keeps the loan's access on the
        // type; the observed record is its referent, matching the peel the
        // validating verifier applies for the same read.
        let (observed_root, access) = match module
            .structural_type(local.structural_type)
            .map(|declaration| &declaration.shape)
        {
            Some(StructuralTypeShape::Reference { referent, access }) => (*referent, *access),
            _ => (
                local.structural_type,
                match &producer.kind {
                    OperationKind::EstablishReference { source, .. }
                    | OperationKind::EstablishElementView { source, .. }
                    | OperationKind::EstablishMutableElementView { source, .. } => source.access,
                    _ => terminal_psi::StructuralAccess::Owned,
                },
            ),
        };
        let matching = terminal_semantics::observed_field_declaration(
            module.structural_types.iter(),
            observed_root,
            path,
            *field,
        )
        .is_some_and(|candidate| {
            candidate.field_type.scalar_type() == Some(result.scalar_type)
                && scalar_observation_authorized(
                    module,
                    observed_root,
                    path,
                    &candidate.field_type,
                    access,
                )
        });
        if !matching
            || (!owns_observed_storage(module, local.structural_type, access)
                && (local.multiplicity == StructuralMultiplicity::Linear
                    || !local.claims.is_empty()))
            || !local.qualifications.is_empty()
            || !local.projected_qualifications.is_empty()
        {
            return malformed("scalar record field has invalid local result custody");
        }
        return Ok(());
    }
    let Some(parameter) = machine.signature_structural_parameter(*source).or_else(|| {
        let declaration = machine.structural_place(*source)?;
        let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
            return None;
        };
        machine
            .block(&block)?
            .structural_parameters
            .get(position as usize)
            .filter(|parameter| parameter.place == *source && parameter.position == position)
    }) else {
        return malformed("scalar structural field source is not a parameter");
    };
    let matching = terminal_semantics::observed_field_declaration(
        module.structural_types.iter(),
        parameter.structural_type,
        path,
        *field,
    )
    .filter(|candidate| {
        candidate.field_type.scalar_type() == Some(result.scalar_type)
            && scalar_observation_authorized(
                module,
                parameter.structural_type,
                path,
                &candidate.field_type,
                parameter.access,
            )
    });
    let owned_observation =
        owns_observed_storage(module, parameter.structural_type, parameter.access);
    if (!owned_observation && parameter.multiplicity == StructuralMultiplicity::Linear)
        || !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned
                | terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        )
        || (!owned_observation
            && machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == *source))
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == *source)
        || matching.is_none()
    {
        return malformed("scalar structural field has invalid source custody");
    }
    Ok(())
}

/// Scalar representation alone grants no permission to snapshot a shared cell.
fn scalar_observation_authorized(
    module: &TerminalModule,
    root: semantic_vocabulary::StructuralTypeId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    field: &StructuralFieldType,
    access: terminal_psi::StructuralAccess,
) -> bool {
    !matches!(field, StructuralFieldType::Atomic(_))
        || matches!(
            terminal_semantics::observed_field_backing_access(
                module.structural_types.iter(),
                root,
                path,
                access,
            ),
            Some(
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::MutableBorrow
            )
        )
}

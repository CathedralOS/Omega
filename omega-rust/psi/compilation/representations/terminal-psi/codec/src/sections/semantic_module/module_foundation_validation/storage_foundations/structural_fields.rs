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
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let (terminal_psi::structural_leaf_copy_pair_pattern!()) = &operation.kind else {
        unreachable!("dispatched validate_structural_leaf_copy")
    };
    if operation.result.structural().is_none_or(|result| {
        (result.multiplicity != StructuralMultiplicity::Unrestricted
            // Descriptor disposal is affine even though shared access is
            // copyable. The copy retains backing and parent obligations;
            // ordinary affine payloads still require an owned transfer.
            && !(result.multiplicity == StructuralMultiplicity::Affine
                && terminal_semantics::is_shared_reference_carrier(
                    module.structural_types.iter(), result.structural_type,
                )))
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
    }) {
        return malformed("leaf copy has invalid multiplicity or qualification custody");
    }
    let result = operation
        .result
        .structural()
        .expect("checked structural copy result");
    if !result.qualifications.is_empty() || !result.projected_qualifications.is_empty() {
        let OperationKind::StructuralLeafCopy { source, path } = &operation.kind else {
            return malformed("case leaf copy does not authenticate memberships");
        };
        let descriptors = terminal_semantics::descriptor_results::terminal_graph(module);
        let mut analysis = terminal_semantics::descriptor_results::DescriptorAnalysis::new(
            &module.structural_types,
            &descriptors,
        );
        let mut history_failures = Vec::new();
        let memberships = (|| {
            let root = terminal_semantics::descriptor_results::terminal_payload_contract(
                machine, *source,
            )?;
            let query = terminal_semantics::descriptor_results::descriptor_contents_query(
                &module.structural_types,
                *source,
                root.carrier,
                path,
                None,
            )?;
            let position = machine.blocks.iter().find_map(|block| {
                block
                    .operations
                    .iter()
                    .position(|candidate| candidate.id == operation.id)
                    .and_then(|ordinal| {
                        u32::try_from(ordinal)
                            .ok()
                            .map(|ordinal| (block.id, ordinal))
                    })
            })?;
            let targets = analysis.payload_targets(machine.id, position, &query)?;
            terminal_semantics::descriptor_results::payload_memberships(
                &module.structural_types,
                &module.structural_domains,
                targets,
                |place| {
                    terminal_semantics::descriptor_results::current_payload_contract(
                        &module.structural_types,
                        &module.structural_domains,
                        &descriptors,
                        machine.id,
                        position,
                        place,
                        &|source| {
                            terminal_semantics::descriptor_results::terminal_payload_contract(
                                machine, source,
                            )
                        },
                    )
                    .map_err(|reason| history_failures.push((place, reason)))
                    .ok()
                },
            )
        })()
        .and_then(|contract| {
            contract.presented(
                &module.structural_types,
                &module.structural_domains,
                result.structural_type,
            )
        });
        if !memberships.as_ref().is_some_and(|contract| {
            contract.carrier == result.structural_type
                && result
                    .qualifications
                    .iter()
                    .all(|domain| contract.qualifications.contains(domain))
                && result
                    .projected_qualifications
                    .iter()
                    .all(|row| contract.projected_qualifications.contains(row))
        }) {
            return Err(CodecError::MalformedStructuralOperation {
                machine: machine
                    .declaration
                    .as_ref()
                    .and_then(|declaration| module.identities.text(declaration.callable))
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("{:?}", machine.id)),
                operation: operation.id,
                reason: format!(
                    "leaf copy memberships do not follow from every current payload source: source={source:?}, path={path:?}, requested={:?}/{:?}/{:?}, current={memberships:?}, history={history_failures:?}",
                    result.structural_type, result.qualifications, result.projected_qualifications,
                ),
            });
        }
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
        range_obligation,
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
    let (destination_type, qualifications, projected_qualifications) =
        if let Some(parameter) = machine.structural_parameter(*destination) {
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
                            && machine.block(&block).and_then(|block| {
                                block.structural_parameters.get(position as usize)
                            }) == Some(parameter)
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
            (
                parameter.structural_type,
                parameter.qualifications.as_slice(),
                parameter.projected_qualifications.as_slice(),
            )
        } else {
            let Some(result) = machine
                .operations()
                .filter(|producer| {
                    projected
                        || matches!(producer.kind, OperationKind::EstablishPrimitiveLocal { .. })
                })
                .filter_map(|producer| producer.result.structural())
                .find(|result| {
                    result.place == *destination
                        && result.claims.is_empty()
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
            (
                result.structural_type,
                result.qualifications.as_slice(),
                result.projected_qualifications.as_slice(),
            )
        };
    let Some(expected) = terminal_semantics::primitive_projection_type(
        module.structural_types.iter(),
        destination_type,
        path,
    ) else {
        return malformed("write-only primitive store requires a primitive-scalar root");
    };
    let Some(restriction) = terminal_semantics::primitive_store_restriction(
        module.structural_types.iter(),
        module.structural_domains.iter(),
        destination_type,
        qualifications,
        projected_qualifications,
        path,
    ) else {
        return malformed("primitive store does not preserve enclosing qualification");
    };
    if restriction.is_some() != range_obligation.is_some() {
        return malformed("primitive store receiving obligation does not match its declaration");
    }
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
    let (root_type, carrier_path) =
        field_store_carrier(module, machine, *destination, path, *field)?;
    let Some(declaration) = terminal_semantics::projected_field_declaration(
        module.structural_types.iter(),
        root_type,
        carrier_path,
        *field,
    ) else {
        return malformed("structural scalar field store does not select a relevant scalar field");
    };
    let expected = match &declaration.field_type {
        StructuralFieldType::Scalar(scalar_type) if range_obligation.is_none() => {
            Some(*scalar_type)
        }
        field_type
            if terminal_semantics::scalar_field_requires_evidence(field_type)
                && range_obligation.is_some() =>
        {
            field_type.scalar_type()
        }
        StructuralFieldType::IeeeFloat(format) if range_obligation.is_none() => {
            Some(semantic_vocabulary::ScalarType::IeeeFloat(*format))
        }
        _ => None,
    };
    let Some(expected) = expected else {
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
    // A moved value retains its declared linear frontier. Result claim-path
    // structure is checked by the common result foundation; current custody
    // and its transfer are not a second storage-shape classification here.
    let Some(root_type) = window_root_type(
        module,
        machine,
        *source,
        path,
        *field,
        terminal_psi::StructuralAccess::MutableBorrow,
    ) else {
        return malformed("structural field move requires a mutable-borrowed parameter");
    };
    let field_type = window_field_type(module, root_type, path, *field)?;
    if result.structural_type != field_type {
        return malformed("structural field move result type does not match field");
    }
    let (qualifications, projected) =
        window_memberships(module, machine, *source, root_type, path, *field)?;
    if result.qualifications != qualifications
        || result.projected_qualifications != projected
        || !result.qualification_establishments.is_empty()
    {
        return malformed("structural field move does not retain the selected source memberships");
    }
    Ok(())
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_store_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::StoreStructuralField {
        disposition,
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
    if value.access != terminal_psi::StructuralAccess::Owned
        || !value.path.is_empty()
        || !terminal_semantics::structural_store_value_admitted(
            machine,
            module.structural_types.iter(),
            *disposition,
            value.place,
        )
    {
        return malformed("structural field store value must be a whole owned place");
    }
    let Some(root_type) = window_root_type(
        module,
        machine,
        *destination,
        path,
        *field,
        disposition.required_access(),
    ) else {
        return malformed("structural field store requires a mutable-borrowed parameter");
    };
    let field_type = window_field_type(module, root_type, path, *field)?;
    if structural_place_type(machine, value.place) != Some(field_type) {
        return malformed("structural field store value type does not match field");
    }
    let (required, required_projected) =
        window_memberships(module, machine, *destination, root_type, path, *field)?;
    let Some((offered, offered_projected)) = declared_memberships(machine, value.place) else {
        return malformed("structural field store value has no membership declaration");
    };
    if !required.iter().all(|domain| offered.contains(domain))
        || !required_projected
            .iter()
            .all(|row| offered_projected.contains(row))
    {
        return malformed("structural field store does not restore the selected memberships");
    }
    Ok(())
}

/// The declared root type a window operation may name. A field window may
/// open beneath a mutable-borrowed parameter or a completed owned home; a
/// whole-root window needs actual mutable backing custody. A reference
/// crossing without a following child selector still names a whole referent.

fn window_root_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
    required_access: terminal_psi::StructuralAccess,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    if !terminal_semantics::structural_window_selects_child(path, field) {
        if path.is_empty() {
            return mutable_borrowed_parameter_type(machine, place, required_access);
        }
        // Serialization proves the carrier and referent geometry. Ordered
        // custody must additionally authenticate the reference's backing as
        // live owned or exclusive ingress storage, with no live descendants.
        let (terminal_psi::StructuralPathSegment::Referent, carrier_path) = path.split_last()?
        else {
            return None;
        };
        let root_type = borrowed_field_store_root_type(machine, place, required_access)?;
        let carrier = terminal_semantics::structural_window_projection(
            module.structural_types.iter(),
            root_type,
            carrier_path,
            None,
        )?;
        module
            .structural_types
            .iter()
            .any(|declaration| {
                declaration.id == carrier.structural_type
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::Reference {
                            access,
                            ..
                        } if required_access.admits_referent(access)
                    )
            })
            .then_some(root_type)
    } else {
        borrowed_field_store_root_type(machine, place, required_access)
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
    required_access: terminal_psi::StructuralAccess,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .chain(machine.block_structural_parameters())
        .find(|parameter| parameter.place == place)
        && required_access.admits(parameter.access)
        && (parameter.access == terminal_psi::StructuralAccess::Owned
            || matches!(
                parameter.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            ))
    {
        return Some(parameter.structural_type);
    }
    machine
        .operations()
        .filter_map(|producer| producer.result.structural())
        .find(|result| result.place == place)
        .map(|result| result.structural_type)
}

/// A declared mutable-borrowed machine or block parameter with transferable
/// multiplicity and no entry claims. Memberships are retained separately.
fn mutable_borrowed_parameter_type(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
    required_access: terminal_psi::StructuralAccess,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    machine
        .structural_parameters
        .iter()
        .chain(machine.block_structural_parameters())
        .find(|parameter| {
            parameter.place == place
                && required_access.admits_referent(parameter.access)
                && matches!(
                    parameter.multiplicity,
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
                )
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

/// Qualification paths describe exact storage subtrees, not a restriction on
/// their representation. Moves retain the selected declaration's memberships;
/// replacements supply them from their value. This checks encoded roster
/// consistency, not ordered liveness or a new establishment proof.
fn window_memberships(
    module: &TerminalModule,
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
    root_type: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Result<
    (
        Vec<semantic_vocabulary::StructuralDomainId>,
        Vec<terminal_psi::StructuralPathQualification>,
    ),
    CodecError,
> {
    let Some(projection) = terminal_semantics::structural_window_projection(
        module.structural_types.iter(),
        root_type,
        path,
        field,
    ) else {
        return malformed("qualified structural window has no declared projection");
    };
    let Some((whole, projected)) = declared_memberships(machine, place) else {
        return malformed("structural window has no membership declaration");
    };
    terminal_psi::StructuralPathQualification::select_restorable(whole, projected, &projection.path)
        .ok_or_else(|| {
            CodecError::MalformedStructuralFoundation(
                "structural window cannot restore an ancestor's qualification",
            )
        })
}

pub(super) fn declared_memberships(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<(
    &[semantic_vocabulary::StructuralDomainId],
    &[terminal_psi::StructuralPathQualification],
)> {
    machine
        .structural_parameter(place)
        .map(|parameter| {
            (
                parameter.qualifications.as_slice(),
                parameter.projected_qualifications.as_slice(),
            )
        })
        .or_else(|| {
            machine
                .operations()
                .filter_map(|operation| operation.result.structural())
                .find(|result| result.place == place)
                .map(|result| {
                    (
                        result.qualifications.as_slice(),
                        result.projected_qualifications.as_slice(),
                    )
                })
        })
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
                    | OperationKind::EstablishPlacedReference { source }
                    | terminal_psi::element_view_establish_pair_pattern!(source, ..) => {
                        source.access
                    }
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
        {
            return malformed("scalar record field has invalid local result custody");
        }
        // A scalar snapshot neither consumes the source nor establishes any
        // of its memberships. Readable access and exact declared geometry are
        // the same for qualified records and every structural producer.
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
        // A scalar snapshot neither consumes the source nor establishes or
        // discharges any claim on it. Owned composite storage may be read
        // under an ordinary or content-bearing entry claim alike: a partition
        // provider reads the magnitude it conserves. Borrowed observation of
        // claimed storage keeps the narrower rule.
        || (!owned_observation
            && (machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == *source)
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == *source)))
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
    let Some(access) = terminal_semantics::observed_field_backing_access(
        module.structural_types.iter(),
        root,
        path,
        access,
    ) else {
        return false;
    };
    access != terminal_psi::StructuralAccess::WriteOnlyBorrow
        && (!matches!(field, StructuralFieldType::Atomic(_))
            || matches!(access, terminal_psi::mutating_access_pair_pattern!()))
}

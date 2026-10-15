//! Structural-root uniqueness, availability, observation, and return contracts.
use super::super::catalog_mismatch;
use crate::OptimizationUnitValidationError;
use crate::unit_validation::structural_catalog::structural_root_key;
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{
    PlaceId, ScalarType, StructuralDomainId, StructuralPlaceKind, StructuralTypeId,
};
use std::collections::{BTreeMap, BTreeSet};

mod availability;
mod primitive_locals;

pub(crate) use availability::validate_structural_place_availability;
pub(crate) use availability::{operation_place_accesses, operation_place_inputs};

pub(crate) fn validate_structural_root_uniqueness(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let mut roots = BTreeSet::new();
    for place in &function.structural_places {
        if !roots.insert(structural_root_key(place.kind)) {
            return Err(
                OptimizationUnitValidationError::DuplicateStructuralPlaceRoot {
                    machine: function.machine,
                    kind: place.kind,
                },
            );
        }
    }
    Ok(())
}

/// Validate the closed root roles of structural observations and structural
/// returns. This is deliberately independent of the later full ownership walk:
/// it establishes which catalog roots may participate and replays every
/// observation invariant still representable after Terminal-to-Omega lowering.
pub(crate) fn validate_structural_root_operations(
    unit: &optimization_unit::PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    structural_domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let shared_descriptor_result = function.result.structural().is_some_and(|result| {
        result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && terminal_semantics::descriptor_results::is_shared_descriptor(
                structural_types.values().copied(),
                result.structural_type,
            )
    });
    // A descriptor owns its payload, not its backing. Reconstruct every
    // returned origin even for functions with no callers in this unit.
    let aggregate_descriptor_slots = function.result.structural().map(|result| {
        terminal_semantics::descriptor_results::aggregate_descriptor_slots(
            &unit.structural_types,
            result.place,
            result.structural_type,
        )
    });
    let aggregate_origins_valid = match aggregate_descriptor_slots {
        None => true,
        Some(None) => false,
        Some(Some(slots)) => {
            slots.is_empty()
                || crate::descriptor_results::owned_aggregate_descriptor_returns(
                    function,
                    unit.functions.iter(),
                    &unit.boundary_machines,
                    &unit.structural_types,
                )
        }
    };
    let descriptor_origins_valid = aggregate_origins_valid
        && (!shared_descriptor_result
            || crate::descriptor_results::optimized_graph(unit).is_some_and(|graph| {
                terminal_semantics::descriptor_results::reconstruct_result_origins(
                    &unit.structural_types,
                    &graph,
                    function.machine,
                )
                .is_some()
            }));
    let place_kinds = function
        .structural_places
        .iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    // Stores through an entry loan and a state loan must retain the exact
    // declaration of their own storage root. Availability and exclusive name
    // transfer are independently replayed by the control-flow ownership walk.
    let exact_store_destination = |destination: &terminal_psi::StructuralParameterDeclaration| {
        function.arrival_parameter(destination.place) == Some(destination)
            && (!matches!(
                place_kinds.get(&destination.place),
                Some(StructuralPlaceKind::BlockParameter { .. })
            ) || matches!(
                destination.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ))
    };
    // Both scalar store forms reconstruct the same root authority; local
    // results retain their exact producing operation instead of an ABI slot.
    let exact_scalar_store_destination =
        |destination: &abstract_operations::StructuralScalarStoreDestination| match destination {
            abstract_operations::StructuralScalarStoreDestination::Parameter(parameter) => {
                exact_store_destination(parameter)
                    && matches!(
                        parameter.access,
                        terminal_psi::StructuralAccess::Owned
                            | terminal_psi::StructuralAccess::MutableBorrow
                            | terminal_psi::StructuralAccess::WriteOnlyBorrow
                    )
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            }
            abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                producer,
                result,
            } => {
                place_kinds.get(&result.place)
                    == Some(&StructuralPlaceKind::OperationResult {
                        producer: *producer,
                        structural_type: result.structural_type,
                    })
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && function.block_nodes().any(|node| {
                        node.operation
                            .structural_result()
                            .is_some_and(|(operation, actual)| {
                                operation == *producer && actual == result
                            })
                    })
            }
            // An owned arrival's home is established by its own block entry:
            // the exact non-entry block declaration at that place, owned and
            // unqualified. Whether the arrival is still live and whole at the
            // store is the ownership walk's frontier check, not this catalog's.
            abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                block,
                parameter,
            } => {
                place_kinds.get(&parameter.place)
                    == Some(&StructuralPlaceKind::BlockParameter {
                        block: *block,
                        position: parameter.position,
                    })
                    && *block != function.entry
                    && parameter.access == terminal_psi::StructuralAccess::Owned
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
                    && function.block(*block).is_some_and(|candidate| {
                        candidate
                            .structural_parameters
                            .get(parameter.position as usize)
                            == Some(parameter)
                    })
            }
        };
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            primitive_locals::validate(
                function,
                block.id,
                node_index,
                &node.operation,
                structural_types,
            )?;
            match &node.operation {
                O::StructuralByteSequenceFieldByteStore { .. } => {
                    if super::byte_views::byte_field_store_capacity(
                        function,
                        &node.operation,
                        structural_types,
                    )
                    .is_none()
                    {
                        return Err(catalog_mismatch(function));
                    }
                }
                abstract_operations::byte_sequence_store_pair_pattern!(source, ..) => {
                    super::byte_views::validate_byte_view_source(
                        function,
                        block.id,
                        node_index,
                        &node.operation,
                        place_kinds.get(source),
                        structural_types,
                        structural_domains,
                    )?;
                    if super::byte_views::byte_field_store_capacity(
                        function,
                        &node.operation,
                        structural_types,
                    )
                    .is_none()
                    {
                        return Err(catalog_mismatch(function));
                    }
                }
                O::StructuralByteViewFieldStore { source, .. } => {
                    if !super::byte_views::byte_view_field_store_is_valid(
                        function,
                        &node.operation,
                        place_kinds.get(source),
                        structural_types,
                    ) {
                        return Err(catalog_mismatch(function));
                    }
                }
                abstract_operations::byte_len_or_field_read_pattern!(source, path, field, ..) => {
                    let geometry = if matches!(
                        path.first(),
                        Some(terminal_psi::StructuralPathSegment::Referent)
                    ) {
                        super::super::references::record_field_destination(
                            function,
                            structural_types,
                            *source,
                            path,
                            terminal_psi::StructuralAccess::MutableBorrow,
                        )
                        .filter(|(_, storage_path)| {
                            terminal_psi::is_static_structural_scalar_store_path(storage_path)
                        })
                    } else {
                        super::byte_field_freshness::byte_field_root(function, *source)
                            .map(|(root, _)| (root, path.as_slice()))
                    };
                    let valid = super::byte_field_freshness::byte_field_root(function, *source)
                        .is_some_and(|(_, access)| {
                            let Some((structural_type, storage_path)) = geometry else { return false; };
                            (access != terminal_psi::StructuralAccess::WriteOnlyBorrow
                                || matches!(node.operation, O::StructuralByteSequenceFieldLength { .. }))
                            // Every retained runtime segment independently names
                            // its operation-owned accepted bound in the fact inventory.
                            && terminal_psi::is_structural_scalar_store_path(storage_path)
                            && super::byte_field_freshness::field_carrier(
                                structural_types, structural_type, storage_path,
                            ).and_then(|parent| structural_types.get(&parent))
                            .is_some_and(|declaration| match &declaration.shape {
                                terminal_psi::StructuralTypeShape::Record { fields, .. } => fields.iter().any(|candidate|
                                    candidate.id == *field && !candidate.relevance.is_erased()
                                    && terminal_semantics::sequence_field_allows_observation(
                                        structural_types.values().copied(),
                                        &candidate.field_type,
                                        if matches!(node.operation, O::StructuralByteSequenceFieldLength { .. }) {
                                            terminal_semantics::SequenceFieldObservation::Length
                                        } else {
                                            terminal_semantics::SequenceFieldObservation::Contents
                                        },
                                    )),
                                _ => false,
                            })
                        });
                    if !valid {
                        return Err(catalog_mismatch(function));
                    }
                }
                O::StructuralCaseMembership {
                    source,
                    path,
                    case,
                    result,
                    ..
                } => {
                    let signature =
                        crate::unit_validation::operation_contracts::structural_source_contract(
                            function, *source,
                        );
                    let valid = result.scalar_type == ScalarType::Boolean
                        && signature.is_some_and(|signature| {
                            signature.access != terminal_psi::StructuralAccess::WriteOnlyBorrow
                                && super::super::structural_catalog::resolve_structural_path(
                                    structural_types,
                                    signature.structural_type,
                                    path,
                                )
                                .and_then(|endpoint| structural_types.get(&endpoint))
                                .is_some_and(|declaration| match &declaration.shape {
                                    terminal_psi::StructuralTypeShape::Sum { cases }
                                    | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => {
                                        cases.iter().any(|candidate| candidate.id == *case)
                                    }
                                    _ => false,
                                })
                        });
                    if !valid {
                        return Err(
                            OptimizationUnitValidationError::InvalidStructuralCaseDispatch {
                                machine: function.machine,
                                source: *source,
                            },
                        );
                    }
                }
                O::ByteSequenceSubslice { .. }
                | O::ByteSequenceLength { .. }
                | O::ElementViewSubslice { .. }
                | O::ElementViewLength { .. } => {
                    if !super::descriptor_sources::valid(
                        function,
                        &node.operation,
                        structural_types,
                        structural_domains,
                    ) {
                        return Err(catalog_mismatch(function));
                    }
                }
                O::ByteSequenceEqual { left, right, .. } => {
                    for operand in [left, right] {
                        if let terminal_psi::control_flow::ByteSequenceOperand::Whole {
                            source,
                            length,
                        } = operand
                            && !super::descriptor_sources::valid_whole(
                                function,
                                structural_types,
                                structural_domains,
                                source,
                                *length,
                            )
                        {
                            return Err(catalog_mismatch(function));
                        }
                    }
                }
                O::ByteSequencePredicate { operand, .. } => {
                    if let terminal_psi::control_flow::ByteSequenceOperand::Whole { source, length } =
                        operand
                        && !super::descriptor_sources::valid_whole(
                            function,
                            structural_types,
                            structural_domains,
                            source,
                            *length,
                        )
                    {
                        return Err(catalog_mismatch(function));
                    }
                }
                O::ByteSequenceRead { source, .. } => {
                    super::byte_views::validate_byte_view_source(
                        function,
                        block.id,
                        node_index,
                        &node.operation,
                        place_kinds.get(source),
                        structural_types,
                        structural_domains,
                    )?;
                }
                O::ByteSequenceWrite { destination, .. } => {
                    super::byte_views::validate_byte_view_source(
                        function,
                        block.id,
                        node_index,
                        &node.operation,
                        place_kinds.get(destination),
                        structural_types,
                        structural_domains,
                    )?;
                }
                O::WriteOnlyPrimitiveStore {
                    destination,
                    path,
                    value,
                    ..
                } => {
                    // A primitive leaf store into an owned block home is not
                    // realized natively yet; only the field store admits it.
                    let valid = exact_scalar_store_destination(destination)
                        && !matches!(
                            destination,
                            abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter { .. }
                        )
                        && (destination.multiplicity()
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                            || (!path.is_empty()
                                && destination.multiplicity()
                                    == terminal_psi::StructuralMultiplicity::Affine))
                        && function
                            .entry_claim_declarations
                            .iter()
                            .all(|claim| claim.input != destination.place())
                        && function
                            .content_entry_claims
                            .iter()
                            .all(|claim| claim.input.root != destination.place())
                        && terminal_semantics::primitive_projection_type(
                            structural_types.values().copied(),
                            destination.structural_type(),
                            path,
                        ) == Some(value.scalar_type);
                    if !valid {
                        return Err(
                            OptimizationUnitValidationError::InvalidWriteOnlyPrimitiveStore {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    }
                }
                O::StructuralScalarFieldStore {
                    destination,
                    path,
                    field,
                    value,
                    range_obligation,
                    ..
                } => {
                    let geometry = if path.contains(&terminal_psi::StructuralPathSegment::Referent)
                    {
                        super::super::references::record_field_destination(
                            function,
                            structural_types,
                            destination.place(),
                            path,
                            terminal_psi::StructuralAccess::WriteOnlyBorrow,
                        )
                    } else {
                        Some((destination.structural_type(), path.as_slice()))
                    };
                    let field_declaration = geometry.and_then(|(root, storage_path)| {
                        terminal_psi::is_structural_scalar_store_path(storage_path).then_some(())?;
                        terminal_semantics::projected_field_declaration(
                            structural_types.values().copied(),
                            root,
                            storage_path,
                            *field,
                        )
                    });
                    // The retained store destination must be the complete
                    // declaration of its actual root. A mutable block arrival
                    // supplies its incoming permission, never an entry ABI slot.
                    // Availability above and exclusive-transfer replay below
                    // independently establish dominance and live custody.
                    let exact_destination = exact_scalar_store_destination(destination);
                    let valid = exact_destination
                        && function
                            .content_entry_claims
                            .iter()
                            .all(|claim| claim.input.root != destination.place())
                        && field_declaration.is_some_and(|declaration| {
                            declaration.field_type.scalar_type() == Some(value.scalar_type)
                                && terminal_semantics::scalar_field_requires_evidence(
                                    &declaration.field_type,
                                ) == range_obligation.is_some()
                        });
                    if !valid {
                        return Err(
                            OptimizationUnitValidationError::InvalidStructuralScalarFieldStore {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    }
                }
                O::StructuralScalarFieldRead {
                    result,
                    source,
                    path,
                    field,
                    ..
                } => {
                    // Root catalogs and dominance are checked before this pass;
                    // current ownership separately checks live, whole owned inputs.
                    // The read observes a Boolean or integer field of exactly
                    // its result's type; float fields have no read here.
                    let valid = matches!(
                        result.scalar_type,
                        ScalarType::Boolean | ScalarType::Integer(_)
                    ) && readable_field_type(function, structural_types, *source)
                        .is_some_and(|(root, access)| {
                            // A call result's `Reference` shell keeps the loan's
                            // access on the type; the observed record is its
                            // referent, the peel every read-side validator shares.
                            let (observed_root, access) = match structural_types
                                .iter()
                                .find(|(id, _)| **id == root)
                                .map(|(_, declaration)| &declaration.shape)
                            {
                                Some(terminal_psi::StructuralTypeShape::Reference {
                                    referent,
                                    access,
                                }) => (*referent, *access),
                                Some(terminal_psi::StructuralTypeShape::ElementView { .. }) => {
                                    (root, terminal_psi::StructuralAccess::SharedBorrow)
                                }
                                _ => (root, access),
                            };
                            observed_scalar_field(
                                structural_types,
                                observed_root,
                                path,
                                *field,
                                access,
                            ) == Some(result.scalar_type)
                        });
                    if !valid {
                        return Err(
                            OptimizationUnitValidationError::InvalidStructuralScalarFieldRead {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    }
                }
                O::EstablishPlacedReference {
                    psi_operation,
                    result,
                    source,
                    correspondence,
                } => {
                    let actual = crate::unit_validation::references::reference_source_type(
                        function,
                        structural_types,
                        source,
                    );
                    let mut plans = unit
                        .structural_types
                        .placed_views()
                        .iter()
                        .filter(|plan| plan.carrier == correspondence.carrier);
                    let (Some(plan), None) = (plans.next(), plans.next()) else {
                        return Err(catalog_mismatch(function));
                    };
                    let exact = terminal_semantics::placed_ram_plan_correspondence(
                        unit.structural_types.as_slice(),
                        unit.structural_types.placed_views(),
                        plan,
                        correspondence.schema,
                    )
                    .is_ok_and(|expected| expected == *correspondence);
                    if !exact
                        || !result.is_affine_plain()
                        || !matches!(place_kinds.get(&result.place),
                            Some(StructuralPlaceKind::OperationResult { producer, structural_type })
                                if *producer == *psi_operation && *structural_type == result.structural_type)
                        || !matches!(
                            source.access,
                            terminal_psi::StructuralAccess::SharedBorrow
                                | terminal_psi::StructuralAccess::MutableBorrow
                        )
                        || !actual.is_some_and(|actual| {
                            crate::unit_validation::references::placed_reference_source_compatible(
                                structural_types,
                                actual,
                                result.structural_type,
                                correspondence,
                            )
                        })
                        || !crate::unit_validation::references::leaf_access_within(
                            structural_types,
                            result.structural_type,
                            &[],
                            source.access,
                        )
                    {
                        return Err(catalog_mismatch(function));
                    }
                }
                O::EstablishReference {
                    psi_operation,
                    result,
                    source,
                    storage,
                } => {
                    let field_compatible = storage.is_none()
                        && crate::unit_validation::operation_contracts::structural_source_contract(
                            function,
                            source.place,
                        )
                        .is_some_and(|contract| {
                            let path = terminal_semantics::reference_source_storage_path(
                                structural_types.values().copied(),
                                contract.structural_type,
                                &source.path,
                            );
                            terminal_semantics::reference_byte_field_compatible(
                                structural_types.values().copied(),
                                structural_domains.values().copied(),
                                contract.structural_type,
                                path,
                                result.structural_type,
                                &result.projected_qualifications,
                            )
                        });
                    // Whole-carrier establishment: the result is an affine
                    // reference over a primitive referent and the source must
                    // retain that referent or a total shared primitive bit view,
                    // under authority covering the reference's own loan.
                    let expected = crate::unit_validation::references::referent(
                        structural_types,
                        result.structural_type,
                    )
                    // A record referent is equally the whole established
                    // carrier: `&Node` binds the source naming its referent,
                    // primitive or not.
                    .or_else(|| {
                        match structural_types
                            .get(&result.structural_type)
                            .map(|declaration| &declaration.shape)
                        {
                            Some(terminal_psi::StructuralTypeShape::Reference {
                                referent, ..
                            }) => Some(*referent),
                            _ => None,
                        }
                    });
                    let valid = expected.is_some()
                        && crate::unit_validation::references::reference_source_accessible(function, structural_types, source)
                        && result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        && result.qualifications.is_empty()
                        && (field_compatible || crate::unit_validation::operation_contracts::structural_source_contract(
                            function,
                            source.place,
                        )
                        .is_some_and(|contract| {
                            let path = terminal_semantics::reference_source_storage_path(
                                structural_types.values().copied(),
                                contract.structural_type,
                                &source.path,
                            );
                            let (whole, projected) = contract.selected_qualifications(path);
                            let mut captured = Vec::new();
                            terminal_psi::StructuralPathQualification::append_under(
                                &whole,
                                &projected,
                                &[terminal_psi::StructuralPathSegment::Referent],
                                &mut captured,
                            );
                            result
                                .projected_qualifications
                                .iter()
                                .all(|row| captured.contains(row))
                                && (source.access == terminal_psi::StructuralAccess::SharedBorrow
                                    || captured
                                        .iter()
                                        .all(|row| result.projected_qualifications.contains(row)))
                        }))
                        && result.claims.is_empty()
                        && matches!(
                            place_kinds.get(&result.place),
                            Some(StructuralPlaceKind::OperationResult {
                                producer,
                                structural_type,
                            }) if *producer == *psi_operation
                                && *structural_type == result.structural_type
                        )
                        && matches!(
                            source.access,
                            terminal_psi::StructuralAccess::MutableBorrow
                                | terminal_psi::StructuralAccess::SharedBorrow
                        )
                        && (field_compatible || crate::unit_validation::references::reference_source_type(
                            function,
                            structural_types,
                            source,
                        )
                        .is_some_and(|actual| match storage {
                            Some(storage) => {
                                terminal_semantics::reference_storage_source_compatible(
                                    structural_types.values().copied(),
                                    actual,
                                    result.structural_type,
                                    storage.stored_scalar,
                                )
                            }
                            None => terminal_semantics::reference_source_compatible(
                                structural_types.values().copied(),
                                actual,
                                result.structural_type,
                            ),
                        }))
                        && crate::unit_validation::references::leaf_access_within(
                            structural_types,
                            result.structural_type,
                            &[],
                            source.access,
                        );
                    if !valid {
                        return Err(catalog_mismatch(function));
                    }
                }
                O::ReleaseReference { source, path, .. } => {
                    // A projection consumes one owned descriptor slot, never
                    // a referent or a borrowed carrier's sibling authority.
                    // Current ownership separately authenticates availability
                    // and the exact live loan at this declared path.
                    let valid =
                        crate::unit_validation::operation_contracts::structural_source_contract(
                            function, *source,
                        )
                        .is_some_and(|signature| {
                            (path.is_empty()
                                || (signature.access == terminal_psi::StructuralAccess::Owned
                                    && signature.multiplicity
                                        == terminal_psi::StructuralMultiplicity::Affine
                                    && signature.is_unqualified()
                                    && !signature.has_result_evidence()))
                                && path.iter().all(|segment| {
                                    matches!(
                                        segment,
                                        terminal_psi::StructuralPathSegment::Field(_)
                                            | terminal_psi::StructuralPathSegment::FixedIndex(_)
                                    )
                                })
                                && crate::unit_validation::references::leaf_type(
                                    structural_types,
                                    signature.structural_type,
                                    path,
                                )
                                .and_then(|selected| structural_types.get(&selected))
                                .is_some_and(|declaration| {
                                    matches!(
                                        declaration.shape,
                                        terminal_psi::StructuralTypeShape::Reference { .. }
                                    )
                                })
                        });
                    if !valid {
                        return Err(catalog_mismatch(function));
                    }
                }
                O::MoveStructuralField {
                    psi_operation,
                    result,
                    source,
                    path,
                    field,
                } => {
                    // Replay the verified static shape: the exact borrowed
                    // parameter row, a declared structural field beneath the
                    // spelled path, and a fresh operation-result place of
                    // exactly that type under clean exact-once custody.
                    let valid = borrowed_window_root(
                        function,
                        source,
                        &place_kinds,
                        terminal_psi::StructuralAccess::MutableBorrow,
                    ) && window_field_type(
                        function,
                        structural_types,
                        source,
                        path,
                        *field,
                        terminal_psi::StructuralAccess::MutableBorrow,
                    ) == Some(result.structural_type)
                        && root_window_loan(source, path, *field)
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && matches!(
                            place_kinds.get(&result.place),
                            Some(StructuralPlaceKind::OperationResult {
                                producer,
                                structural_type,
                            }) if *producer == *psi_operation
                                && *structural_type == result.structural_type
                        );
                    if !valid {
                        return Err(catalog_mismatch(function));
                    }
                }
                O::StoreStructuralField {
                    disposition,
                    destination,
                    path,
                    field,
                    value,
                    ..
                } => {
                    // The repair targets the same borrowed root authority and
                    // consumes one owned whole place of the declared field
                    // type. Whether a window is open there is debt evidence
                    // the Terminal verifier retained; the unit keeps the
                    // static contract honest.
                    let valid = borrowed_window_root(
                        function,
                        destination,
                        &place_kinds,
                        disposition.required_access(),
                    ) && root_window_loan(destination, path, *field)
                        && value.access == terminal_psi::StructuralAccess::Owned
                        && value.path.is_empty()
                        && window_field_type(
                            function,
                            structural_types,
                            destination,
                            path,
                            *field,
                            disposition.required_access(),
                        )
                        .is_some_and(|field_type| {
                            crate::unit_validation::operation_contracts::structural_source_contract(
                                function,
                                value.place,
                            )
                            .is_some_and(|signature| {
                                signature.structural_type == field_type
                                    && (*disposition
                                        == terminal_psi::StructuralStoreDisposition::Vacant
                                        || terminal_semantics::discardable_store_value(
                                            structural_types.values().copied(),
                                            field_type,
                                            signature.multiplicity,
                                            function
                                                .entry_claim_declarations
                                                .iter()
                                                .chain(&function.block_entry_claims)
                                                .any(|claim| claim.input == value.place)
                                                || function
                                                    .content_entry_claims
                                                    .iter()
                                                    .any(|claim| claim.input.root == value.place)
                                                || signature.operation_result().is_some_and(
                                                    |result| !result.claims.is_empty(),
                                                ),
                                        ))
                                    && signature.is_unqualified()
                            })
                        });
                    if !valid {
                        return Err(catalog_mismatch(function));
                    }
                }
                O::StructuralLeafCopy {
                    psi_operation,
                    result,
                    source,
                    path,
                } => {
                    // Replay the verified static shape: a readable source
                    // root, the spelled path resolving to exactly the result
                    // type, and a fresh operation-result place under clean
                    // custody. The case path also carries the verifier's
                    // owned-copy shape: an `Affine` non-reference leaf minted
                    // out of a proven-active case payload transfers custody
                    // to the successor — no stored value moves. An `Affine`
                    // `SharedBorrow` result is an independently retired
                    // descriptor copy: its exact reference type stays intact
                    // and the original occurrence remains live. Reference
                    // custody, not referent bytes, is duplicated.
                    let owned_case_copy =
                        matches!(path, abstract_operations::StructuralCopyPath::Case(_))
                            && result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                            && structural_types.get(&result.structural_type).is_some_and(
                                |declaration| {
                                    !matches!(
                                        declaration.shape,
                                        terminal_psi::StructuralTypeShape::Reference { .. }
                                    )
                                },
                            );
                    let shared_descriptor_copy = result.multiplicity
                        == terminal_psi::StructuralMultiplicity::Affine
                        && terminal_semantics::is_shared_reference_carrier(
                            structural_types.values().copied(),
                            result.structural_type,
                        );
                    let valid =
                        crate::unit_validation::operation_contracts::structural_source_contract(
                            function, *source,
)
                        .is_some_and(|signature| {
                            signature.access != terminal_psi::StructuralAccess::WriteOnlyBorrow
                            // A whole same-type copy transports its exact value
                            // contract regardless of carrier layout. Projection
                            // may retain only memberships of the selected path;
                            // a receiving result cannot establish new domains.
                            && result.qualification_establishments.is_empty()
                            && match path {
                                abstract_operations::StructuralCopyPath::Ordinary(path)
                                    if path.is_empty() && signature.structural_type == result.structural_type =>
                                {
                                    signature.multiplicity == result.multiplicity
                                        && signature.has_exact_qualifications(&result.qualifications)
                                        && !signature.has_result_evidence()
                                }
                                abstract_operations::StructuralCopyPath::Ordinary(path) => {
                                    result.qualifications.iter().all(|domain| {
                                        signature.carries_qualification(path, *domain)
                                    })
                                }
                                abstract_operations::StructuralCopyPath::Case(_) => {
                                    result.qualifications.is_empty()
                                }
                            }
                            && match path {
                                abstract_operations::StructuralCopyPath::Ordinary(path) => {
                                    // Copy the selected descriptor value, not
                                    // a fresh reference over its source home.
                                    // Backing and immediate parents are retained
                                    // by the independent custody replay.
                                    // A byte-carrier copy mints the overlaid
                                    // record out of a `u8` element window
                                    // instead of the spelled leaf type.
                                    let expected = result.structural_type;
                                    let selected = crate::unit_validation::structural_catalog::resolve_leaf_copy_path(structural_types, signature.structural_type, path);
                                    selected == Some(expected)
                                        || selected.is_some_and(|selected| terminal_semantics::shared_descriptor_copy_matches(
                                            structural_types.values().copied(), selected,
                                            path.is_empty().then_some(signature.access), expected,
                                        ))
                                        || (path.is_empty() && abstract_operations::quotients::licensed_copy(
                                            &unit.structural_types, function.machine, *psi_operation, *source,
                                            signature.structural_type, result,
                                            function.block_nodes().map(|node| &node.operation)))
                                        || crate::unit_validation::structural_catalog::byte_carrier_leaf_window(structural_types, signature.structural_type, path, expected)
                                },
                                abstract_operations::StructuralCopyPath::Case(path) => {
                                    // Case evidence selects the value, not a
                                    // fresh borrow over its descriptor home.
                                    let expected = result.structural_type;
                                    terminal_semantics::canonical_case_leaf_type(structural_types.values().copied(), signature.structural_type, path) == Some(expected)
                                }
                            }
                        }) && (result.multiplicity
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                            || owned_case_copy
                            || shared_descriptor_copy)
                            && result.projected_qualifications.is_empty()
                            && result.claims.is_empty()
                            && matches!(
                                place_kinds.get(&result.place),
                                Some(StructuralPlaceKind::OperationResult {
                                    producer,
                                    structural_type,
                                }) if *producer == *psi_operation
                                    && *structural_type == result.structural_type
                            );
                    if !valid {
                        return Err(catalog_mismatch(function));
                    }
                }
                O::ReturnStructural { source, .. } => {
                    let Some(signature) = function.result.structural() else {
                        return Err(
                            OptimizationUnitValidationError::StructuralReturnSourceContractMismatch {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    };
                    let source_contract =
                        function
                            .structural_parameters
                            .iter()
                            .find(|parameter| {
                                parameter.place == *source
                                    && matches!(
                                        place_kinds.get(source),
                                        Some(StructuralPlaceKind::Parameter { position })
                                            if *position == parameter.position
                                    )
                            })
                            .or_else(|| {
                                let Some(StructuralPlaceKind::BlockParameter { block, position }) =
                                    place_kinds.get(source).copied()
                                else {
                                    return None;
                                };
                                function.block(block)
                                    .and_then(|block| {
                                        block.structural_parameters.get(position as usize)
                                    })
                                    .filter(|parameter| {
                                        parameter.place == *source
                                            && parameter.position == position
                                            && (parameter.access
                                                == terminal_psi::StructuralAccess::Owned
                                                || shared_descriptor_result)
                                    })
                            })
                            .map(|parameter| {
                                (
                                    parameter.structural_type,
                                    parameter.multiplicity,
                                    parameter.qualifications.as_slice(),
                                    parameter.projected_qualifications.as_slice(),
                                    BTreeSet::new(),
                                )
                            })
                            .or_else(|| {
                                let Some(StructuralPlaceKind::ByteSequenceLiteral {
                                    structural_type,
                                    ..
                                }) = place_kinds.get(source).copied()
                                else {
                                    return None;
                                };
                                Some((
                                    structural_type,
                                    terminal_psi::StructuralMultiplicity::Unrestricted,
                                    &[][..],
                                    &[][..],
                                    BTreeSet::new(),
                                ))
                            })
                            .or_else(|| {
                                let Some(StructuralPlaceKind::OperationResult {
                                    producer,
                                    structural_type,
                                }) = place_kinds.get(source).copied()
                                else {
                                    return None;
                                };
                                function.block_nodes()
                                .find_map(|node| {
                                    let (actual_producer, result) = node.operation.structural_result()?;
                                    if actual_producer != producer
                                        || result.place != *source
                                        || result.structural_type != structural_type
                                    {
                                        return None;
                                    }
                                    // Ordinary payloads must own their storage.
                                    // Shared descriptors instead retain backing
                                    // through the independently replayed summary.
                                    if !shared_descriptor_result
                                        && crate::unit_validation::operation_contracts::structural_source_contract(
                                            function, *source,
).is_none_or(|source| source.access != terminal_psi::StructuralAccess::Owned)
                                    {
                                        return None;
                                    }
                                    Some((
                                        result.structural_type,
                                        result.multiplicity,
                                        result.qualifications.as_slice(),
                                        result.projected_qualifications.as_slice(),
                                        result.qualification_establishments.iter()
                                            .map(|binding| binding.domain).collect::<BTreeSet<_>>(),
                                    ))
                                })
                            });
                    let exact_bounded_owned_bytes = structural_types
                        .get(&signature.structural_type)
                        .is_some_and(|declaration| {
                            declaration.shape.is_bounded_owned_byte_sequence()
                        });
                    if !descriptor_origins_valid
                        || source_contract.is_none_or(
                            |(structural_type, multiplicity, qualifications, projected, minted)| {
                                // Mirror the Terminal return-edge rule: domains
                                // minted onto the source by its producer's
                                // authorized qualification establishments shed at
                                // the contract edge, so the remaining source
                                // qualifications must live inside the declared
                                // result roster rather than equal it.
                                structural_type != signature.structural_type
                                    || multiplicity != signature.multiplicity
                                    || if exact_bounded_owned_bytes {
                                        !minted.is_empty()
                                            || qualifications != signature.qualifications
                                            || projected != signature.projected_qualifications
                                    } else {
                                        !qualifications
                                            .iter()
                                            .filter(|domain| !minted.contains(domain))
                                            .all(|domain| signature.qualifications.contains(domain))
                                            || projected
                                                .iter()
                                                .filter(|projection| {
                                                    !minted.contains(&projection.domain)
                                                })
                                                .cloned()
                                                .collect::<Vec<_>>()
                                                != signature.projected_qualifications
                                    }
                            },
                        )
                    {
                        return Err(
                            OptimizationUnitValidationError::StructuralReturnSourceContractMismatch {
                                machine: function.machine,
                                block: block.id,
                                node: node_index,
                            },
                        );
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// The scalar one field observation reads beneath `root`: a relevant field of
/// the record `path` reaches, or a relevant payload field of the declared case
/// a final `Case` step names. The payload overlays the sum's own place; the
/// verified module proved that case active, and this replay rebuilds only the
/// declared carrier and leaf, so a substituted case or field fails here.

fn observed_scalar_field(
    structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
    access: terminal_psi::StructuralAccess,
) -> Option<ScalarType> {
    let access = terminal_semantics::observed_field_backing_access(
        structural_types.values().copied(),
        root,
        path,
        access,
    )?;
    let declaration = terminal_semantics::observed_field_declaration(
        structural_types.values().copied(),
        root,
        path,
        field,
    )?;
    if matches!(
        declaration.field_type,
        terminal_psi::StructuralFieldType::Atomic(_)
    ) && !matches!(
        access,
        terminal_psi::StructuralAccess::Owned | terminal_psi::StructuralAccess::MutableBorrow
    ) {
        return None;
    }
    relevant_scalar_field(std::slice::from_ref(declaration), field, true)
}

fn relevant_scalar_field(
    fields: &[terminal_psi::StructuralFieldDeclaration],
    field: semantic_vocabulary::StructuralFieldId,
    allow_refined_scalar: bool,
) -> Option<ScalarType> {
    fields.iter().find_map(|candidate| {
        (candidate.id == field && !candidate.relevance.is_erased())
            .then_some(&candidate.field_type)
            .and_then(|field_type| match field_type {
                terminal_psi::StructuralFieldType::Scalar(scalar_type) => Some(*scalar_type),
                terminal_psi::StructuralFieldType::Atomic(_) => field_type.atomic_scalar_type(),
                terminal_psi::StructuralFieldType::IeeeFloat(format) => {
                    Some(ScalarType::IeeeFloat(*format))
                }
                // Construction obligations remain checked separately. A scalar
                // observation can use this carrier, but an unproved store cannot.
                terminal_psi::StructuralFieldType::BoundedInteger(bounds)
                    if allow_refined_scalar =>
                {
                    Some(ScalarType::Integer(bounds.integer_type()))
                }
                terminal_psi::StructuralFieldType::RefinedScalar(refinement)
                    if allow_refined_scalar =>
                {
                    Some(refinement.carrier())
                }
                _ => None,
            })
    })
}

fn readable_field_type(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    place: PlaceId,
) -> Option<(StructuralTypeId, terminal_psi::StructuralAccess)> {
    let signature =
        crate::unit_validation::operation_contracts::structural_source_contract(function, place)?;
    if signature.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
        // A nominal aggregate's memberships strengthen its value contract;
        // they neither forbid reading a field nor grant readable access.
        // Byte, view and reference carriers keep their specialized evidence
        // rules. Current ownership separately proves the occurrence is live.
        || (!signature.is_unqualified()
            && !structural_types.get(&signature.structural_type)
                .is_some_and(|declaration| declaration.shape.is_composite_shape()))
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == place)
    {
        return None;
    }
    // Reading an owned field retains its complete claim frontier. Exact live
    // ownership and loan interference are replayed separately at this node;
    // borrowed carriers keep the existing claim-free access contract.
    if signature.access == terminal_psi::StructuralAccess::Owned
        && structural_types
            .get(&signature.structural_type)
            .is_some_and(|declaration| (declaration.shape).is_composite_shape())
    {
        return Some((signature.structural_type, signature.access));
    }
    if signature.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || function
            .entry_claim_declarations
            .iter()
            .any(|claim| claim.input == place)
        || function.block_nodes().any(|node| {
            let result = match &node.operation {
                O::EstablishRecord { result, .. }
                | O::EstablishZeroedStructural { result, .. }
                | O::EstablishScalarArray { result, .. }
                | abstract_operations::structural_array_or_case_pattern!(result, ..)
                | O::EstablishReference { result, .. }
                | O::MoveStructuralField { result, .. }
                | O::CallStructural {
                    result: AbstractStructuralCallResult::Structural { result, .. },
                    ..
                }
                | O::BoundaryCall {
                    result: abstract_operations::AbstractBoundaryResult::Structural(result),
                    ..
                } => result,
                _ => return false,
            };
            result.place == place && !result.claims.is_empty()
        })
    {
        return None;
    }
    Some((signature.structural_type, signature.access))
}

/// The borrowed entry or block parameter a restoration-window operation may name,
/// replayed against the unit's retained rows: the exact declaration,
/// mutable-borrow authority, a transferable multiplicity, and no
/// qualifications or entry claims — the same static contract the Terminal
/// verifier's `borrowed_window_root` admitted.
fn borrowed_window_root(
    function: &PsiOptimizationFunction,
    root: &abstract_operations::StructuralWindowRoot,
    place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    required_access: terminal_psi::StructuralAccess,
) -> bool {
    match root {
        abstract_operations::StructuralWindowRoot::OwnedBlockParameter { block, parameter } => {
            parameter.access == terminal_psi::StructuralAccess::Owned
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && place_kinds.get(&parameter.place)
                    == Some(&StructuralPlaceKind::BlockParameter {
                        block: *block,
                        position: parameter.position,
                    })
                && function.blocks.iter().any(|candidate| {
                    candidate.id == *block && candidate.structural_parameters.contains(parameter)
                })
        }
        abstract_operations::StructuralWindowRoot::Parameter(parameter) => {
            function
                .structural_parameters
                .iter()
                .chain(function.block_structural_parameters())
                .find(|candidate| candidate.place == parameter.place)
                == Some(parameter)
                && required_access.admits_referent(parameter.access)
                && matches!(
                    parameter.multiplicity,
                    terminal_psi::StructuralMultiplicity::Unrestricted
                        | terminal_psi::StructuralMultiplicity::Affine
                )
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && function
                    .entry_claim_declarations
                    .iter()
                    .all(|claim| claim.input != parameter.place)
                && function
                    .content_entry_claims
                    .iter()
                    .all(|claim| claim.input.root != parameter.place)
                && match place_kinds.get(&parameter.place) {
                    Some(StructuralPlaceKind::Parameter { position }) => {
                        *position == parameter.position
                            && function.structural_parameters.contains(parameter)
                    }
                    Some(StructuralPlaceKind::BlockParameter { block, position }) => {
                        *position == parameter.position
                            && function.blocks.iter().any(|candidate| {
                                candidate.id == *block
                                    && candidate.structural_parameters.contains(parameter)
                            })
                    }
                    _ => false,
                }
        }
        abstract_operations::StructuralWindowRoot::OwnedResult { producer, result } => {
            // Rejoin the exact producer through the shared storage contract.
            // A structural result may expose a loan instead of owning its
            // storage, so result identity alone does not establish authority.
            let producer_admits =
                function.block_nodes()
                    .any(|node| {
                        node.operation.psi_operation() == Some(*producer)
                            && crate::unit_validation::operation_contracts::structural_operation_result_contract(
                                function, &node.operation,
                            )
                            .is_some_and(|source| {
                                source.access == terminal_psi::StructuralAccess::Owned
                                    && source.operation_result() == Some(result)
                            })
                    });
            producer_admits
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && matches!(
                    place_kinds.get(&result.place),
                    Some(StructuralPlaceKind::OperationResult {
                        producer: recorded,
                        structural_type,
                    }) if *recorded == *producer && *structural_type == result.structural_type
                )
        }
    }
}

/// A whole-root window needs actual mutable backing custody. Crossing
/// a reference alone does not select a child or transfer that authority.
fn root_window_loan(
    root: &abstract_operations::StructuralWindowRoot,
    path: &[terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> bool {
    terminal_semantics::structural_window_selects_child(path, field)
        // Declaration geometry is separate from the live ancestry check in
        // current_ownership::references::windows.
        || path.contains(&terminal_psi::StructuralPathSegment::Referent)
        || (path.is_empty() && root.parameter().is_some())
}

/// The declared `Structural` type of the field a window operation names,
/// resolved beneath the borrowed root through the spelled path; a root
/// window names the whole referent with no path. `None` means the place does
/// not host a structural field there.
fn window_field_type(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: &abstract_operations::StructuralWindowRoot,
    path: &[terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
    required_access: terminal_psi::StructuralAccess,
) -> Option<StructuralTypeId> {
    let (root, path) = if path.contains(&terminal_psi::StructuralPathSegment::Referent) {
        crate::unit_validation::references::record_field_destination(
            function,
            structural_types,
            root.place(),
            path,
            required_access,
        )?
    } else {
        (root.structural_type(), path)
    };
    terminal_semantics::structural_window_projection(
        structural_types.values().copied(),
        root,
        path,
        field,
    )
    .map(|projection| projection.structural_type)
}

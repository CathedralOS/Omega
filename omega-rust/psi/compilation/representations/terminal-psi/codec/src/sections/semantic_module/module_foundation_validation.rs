//! Closed representation-foundation validation at external module ingress.
//!
//! Checks identities, exact carrier relationships, signatures, provider
//! attachments, and operation/result shape so the independent semantic
//! verifier can rely on a well-formed module. It does not prove
//! qualifications, reach closure, or claim dataflow.
//!
//! `validate_structural_foundation` is the entry: type shapes are checked in
//! `structural_type_foundations`, domains, services, boundary machines and
//! provider candidates in `declaration_foundations`, and each machine in
//! `machine_foundations`, whose `validate_operation_foundation` dispatches
//! each operation kind to its `validate_*` function in `value_foundations`,
//! `storage_foundations` or `call_foundations`.

mod call_foundations;
mod declaration_foundations;
mod machine_foundations;
mod reference_outputs;
mod scalar_observations;
mod storage_foundations;
mod structural_type_foundations;
mod target_layouts;
mod value_foundations;

use crate::codec_error::{CodecError, malformed};
use terminal_psi::TerminalIdentityIndex;

use semantic_vocabulary::{ClaimId, ScalarType, ServiceId, StructuralPlaceKind, StructuralTypeId};
use std::collections::BTreeSet;
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralArgument, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeShape, TerminalMachine, TerminalModule, Terminator, find_structural_case,
};

/// The declared scalar type of a value anywhere it may be declared on the
/// machine — machine parameters, the result, block parameters and operation
/// results.
pub(super) fn declared_scalar_type(
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

/// Validate the closed representation foundation needed before the independent
/// semantic verifier learns these operations. This checks identities, exact
/// carrier relationships, signatures, and operation/result shape; it does not
/// prove qualifications, reach closure, or claim dataflow.
pub(crate) fn validate_structural_foundation(module: &TerminalModule) -> Result<(), CodecError> {
    terminal_semantics::quotients::validate_quotient_catalog(module)
        .map_err(CodecError::MalformedStructuralFoundation)?;
    terminal_semantics::closed_conformances::validate_closed_conformances(module)
        .map_err(|reason| CodecError::MalformedStructuralFoundation(reason))?;
    let mut placed_coordinates = BTreeSet::new();
    for row in &module.placed_view_inputs {
        if !placed_coordinates.insert((
            row.machine,
            row.source_state_identity,
            row.runtime_position,
        )) {
            return malformed("placed-view inputs repeat one structural formal");
        }
        terminal_semantics::placed_view_input_parameter(module, row)
            .map_err(CodecError::PlacedViewInput)?;
    }
    terminal_semantics::validate_placed_view_plans(module).map_err(CodecError::PlacedPlan)?;
    validate_target_families(module)?;
    validate_boundary_application_operations(module)?;
    validate_callback_registrars(module)?;
    require_unique_nonempty_identities(
        module
            .structural_types
            .iter()
            .map(|declaration| declaration.identity)
            .chain(
                module
                    .structural_referents
                    .iter()
                    .map(|referent| referent.identity),
            ),
        "structural type identity",
    )?;
    let mut type_ids = BTreeSet::new();
    for id in module
        .structural_types
        .iter()
        .map(|declaration| declaration.id)
        .chain(
            module
                .structural_referents
                .iter()
                .map(|referent| referent.id),
        )
    {
        if !type_ids.insert(id) {
            return malformed("structural identity and storage catalogs overlap");
        }
    }
    for referent in &module.structural_referents {
        if !module.structural_types.iter().any(|declaration| {
            matches!(
                declaration.shape, StructuralTypeShape::Reference { referent: target, .. }
                    if target == referent.id
            )
        }) {
            return malformed("identity-only structural type is not a reference target");
        }
    }
    require_unique_nonempty_identities(
        module
            .structural_domains
            .iter()
            .map(|declaration| declaration.identity),
        "structural domain identity",
    )?;
    require_unique_nonempty_identities(
        module
            .services
            .iter()
            .map(|declaration| declaration.identity),
        "service identity",
    )?;
    require_unique_nonempty_identities(
        module
            .boundary_machines
            .iter()
            .map(|declaration| declaration.identity),
        "boundary machine identity",
    )?;

    let types = module
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    let attachment_storage = structural_type_foundations::attachment_storage_types(
        &types,
        module
            .machines
            .iter()
            .filter_map(|machine| machine.attachment),
    );
    target_layouts::validate(module, &attachment_storage)?;
    for declaration in &module.structural_types {
        structural_type_foundations::validate_type_shape(module, declaration, &attachment_storage)?;
    }
    validate_structural_type_graph(module)?;
    for interface in &module.dynamic_dispatch.interfaces {
        for slot in &interface.slots {
            terminal_semantics::validate_dynamic_reference_regions(
                module.structural_types.iter(),
                slot,
            )
            .map_err(CodecError::MalformedStructuralFoundation)?;
        }
    }
    declaration_foundations::validate_domains(module)?;
    declaration_foundations::validate_services(module)?;
    for boundary in &module.boundary_machines {
        declaration_foundations::validate_boundary_machine(module, boundary)?;
    }
    for candidate in &module.provider_candidates {
        declaration_foundations::validate_provider_candidate(module, candidate)?;
    }
    for machine in &module.machines {
        machine_foundations::validate_machine(module, machine)?;
    }
    Ok(())
}

/// Demand rows name current operations, never provider-selected replacements.
fn validate_boundary_application_operations(module: &TerminalModule) -> Result<(), CodecError> {
    let demands = &module.boundary_application_demands;
    if demands.is_empty() {
        return Ok(());
    }
    // The canonical demand roster is already sorted. Mark only its requested
    // operations while streaming the program; do not build another program index.
    let mut found = vec![false; demands.len()];
    for operation in module.operations() {
        if let Ok(ordinal) =
            demands.binary_search_by_key(&operation.id, |demand| demand.terminal_operation())
        {
            if found[ordinal] {
                return malformed("boundary application names a repeated operation");
            }
            found[ordinal] = true;
        }
    }
    if found.iter().any(|found| !found) {
        return malformed("boundary application names an absent operation");
    }
    Ok(())
}

fn validate_callback_registrars(module: &TerminalModule) -> Result<(), CodecError> {
    let mut registrars = module
        .callback_applications
        .iter()
        .map(|application| application.registrar)
        .collect::<BTreeSet<_>>();
    if registrars.is_empty() {
        return Ok(());
    }
    // Several callback binders may belong to the same boundary call. Check
    // only the requested set while streaming the program's operations.
    for operation in module.operations() {
        if matches!(operation.kind, OperationKind::BoundaryCall { .. }) {
            registrars.remove(&operation.id);
        }
    }
    if !registrars.is_empty() {
        return malformed("callback registrar is not a boundary call");
    }
    Ok(())
}

fn validate_target_families(module: &TerminalModule) -> Result<(), CodecError> {
    use terminal_psi::{TargetMachineCallable, TargetMachineSupply};

    match module.entry {
        TargetMachineCallable::Machine(machine) if module.machine(machine).is_some() => {}
        head @ TargetMachineCallable::Boundary(boundary)
            if module.target_machine_families.iter().any(|family| {
                family.canonical == head
                    && !family.members.iter().any(|member| member.callable == head)
            }) && module
                .boundary_machine(boundary)
                .is_some_and(|declaration| {
                    declaration.kind == semantic_vocabulary::BoundaryRequirementKind::Machine
                        && declaration.role == terminal_psi::BoundaryDeclarationRole::Callable
                }) => {}
        _ => return malformed("portable entry has no retained machine or target-family head"),
    }
    // A family-root declaration is meaningful only with its complete checked
    // catalog commitment below. An arbitrary bodyless boundary cannot become
    // an executable entrance by receiving a target or an entry receipt.
    let mut owners = BTreeSet::new();
    for family in &module.target_machine_families {
        if family.head_contract == [0; 32] {
            return malformed("target-family head has no checked contract commitment");
        }
        if family.members.is_empty() {
            return malformed("target family has no members");
        }
        if !family
            .members
            .iter()
            .any(|member| member.callable == family.canonical)
        {
            // An independent head describes the portable machine contract.
            // A retained declaration grants neither supply nor selection.
            let TargetMachineCallable::Boundary(head) = family.canonical else {
                return malformed("independent target-family head is not a machine contract");
            };
            if module.boundary_machine(head).is_none_or(|declaration| {
                declaration.kind != semantic_vocabulary::BoundaryRequirementKind::Machine
                    || declaration.role != terminal_psi::BoundaryDeclarationRole::Callable
            }) {
                return malformed("independent target-family head has no machine declaration");
            }
        }
        for member in &family.members {
            if member.target.is_empty() || !owners.insert(member.callable) {
                return malformed("target-family member has an empty tag or repeated callable");
            }
            // Canonical order already owns unique heads and ordered target
            // tags. A head may be its own legacy member, never another
            // family's member, even when it names a valid declaration.
            if member.callable != family.canonical
                && module
                    .target_machine_families
                    .binary_search_by_key(&member.callable, |family| family.canonical)
                    .is_ok()
            {
                return malformed("target-family head is a member of another family");
            }
            match member.callable {
                TargetMachineCallable::Machine(machine) => {
                    if !matches!(
                        member.supply,
                        TargetMachineSupply::CheckedBody | TargetMachineSupply::Boundary
                    ) || !module.machine(machine).is_some()
                    {
                        return malformed(
                            "target-family checked member has no matching body supply",
                        );
                    }
                }
                TargetMachineCallable::Boundary(boundary) => {
                    if member.supply == TargetMachineSupply::CheckedBody
                        || module.boundary_machine(boundary).is_none_or(|declaration| {
                            declaration.role != terminal_psi::BoundaryDeclarationRole::Callable
                        })
                    {
                        return malformed(
                            "target-family opaque member has no matching declaration supply",
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_provider_attachment_foundation(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), CodecError> {
    let provider_roots = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary,
            } => Some((attachment, field, boundary)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let provider_fields = machine
        .attachment
        .and_then(|attachment| module.structural_type(attachment))
        .and_then(|attachment| match &attachment.shape {
            StructuralTypeShape::Record { fields, .. } => Some(
                fields
                    .iter()
                    .filter(|field| {
                        terminal_semantics::fused_service_field_carrier(
                            &module.structural_types,
                            field,
                        )
                        .is_some()
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    if provider_roots.is_empty() && provider_fields.is_empty() {
        return Ok(());
    }
    // One attachment may hold several provider fields; every root names one.
    if provider_fields.is_empty() {
        return malformed("provider-backed attachment specialization is incomplete");
    }
    let Some(attachment) = machine.attachment else {
        return malformed("provider-backed attachment specialization is incomplete");
    };
    let mut boundaries = BTreeSet::new();
    if provider_roots
        .iter()
        .any(|(root_attachment, field, boundary)| {
            *root_attachment != attachment
                || !provider_fields
                    .iter()
                    .any(|provider_field| provider_field.id == *field)
                || !boundaries.insert(*boundary)
                || module
                    .boundary_machine(*boundary)
                    .is_none_or(|declaration| {
                        declaration.kind != terminal_psi::BoundaryRequirementKind::Trait
                    })
        })
    {
        return malformed("provider-backed attachment specialization is incomplete");
    }
    // The retained declaration kind, not an absent nominal attachment,
    // determines whether the call consumes a provider field.
    let called = machine
        .operations()
        .filter_map(|operation| match operation.kind {
            OperationKind::BoundaryCall { boundary, .. } => Some(boundary),
            _ => None,
        })
        .filter(|boundary| {
            module
                .boundary_machine(*boundary)
                .is_some_and(|declaration| {
                    declaration.kind == terminal_psi::BoundaryRequirementKind::Trait
                })
        })
        .collect::<BTreeSet<_>>();
    if called != boundaries {
        return malformed("provider-backed attachment specialization is incomplete");
    }
    Ok(())
}

pub(crate) fn validate_structural_path(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<StructuralTypeId, CodecError> {
    if path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::RepresentationView { .. }))
    {
        return malformed("returned-origin interpretation is not an operand or owned path");
    }
    validate_reference_origin_path(module, structural_type, path)
}

/// An origin recipe describes admitted backing; it is not executable access.
/// Only result-source rows use this entrance. Operands, claims, qualifications
/// and cleanup use the ordinary path entrance, which rejects interpretations.
pub(crate) fn validate_reference_origin_family(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    path: &[terminal_psi::ReferenceOriginProjection],
) -> Result<StructuralTypeId, CodecError> {
    terminal_semantics::reference_origin_type(module.structural_types.iter(), structural_type, path)
        .ok_or(CodecError::MalformedStructuralFoundation(
            "reference origin has an invalid typed projection",
        ))
}

fn validate_reference_origin_path(
    module: &TerminalModule,
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<StructuralTypeId, CodecError> {
    // Runtime type coordinates include inline descriptors, whose original
    // root/path retain custody. They do not create an owned canonical place.
    if let Some(endpoint) = terminal_semantics::runtime_structural_path_tip(
        module.structural_types.iter(),
        structural_type,
        path,
    ) {
        return Ok(endpoint.id);
    }
    // Primitive projected storage uses the shared endpoint judgment, including
    // byte elements whose carrier is inline rather than a declared child.
    // Runtime selectors retain their owning operation's independent bounds.
    if let Some(endpoint) = terminal_semantics::primitive_projection_declaration(
        module.structural_types.iter(),
        structural_type,
        path,
    ) {
        return Ok(endpoint);
    }
    let mut payload_fields = None;
    for segment in path {
        let Some(declaration) = module.structural_type(structural_type) else {
            return malformed("structural path has an unknown structural type");
        };
        if let StructuralPathSegment::Case(case) = segment {
            if payload_fields.is_some() {
                return malformed("structural case path requires its payload field");
            }
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &declaration.shape
            else {
                return malformed("structural case path requires a sum");
            };
            let Some(selected) = find_structural_case(cases, *case) else {
                return malformed("structural path has an unknown case");
            };
            payload_fields = Some(selected.fields.as_slice());
            continue;
        }
        if payload_fields.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
            return malformed("structural case path requires its payload field");
        }
        structural_type = match (segment, &declaration.shape) {
            (StructuralPathSegment::AllElements, _) => {
                return malformed("universal qualification is not an executable storage path");
            }
            (
                StructuralPathSegment::RepresentationView {
                    structural_type: view,
                    stored_scalar,
                },
                shape,
            ) => {
                if matches!(shape, StructuralTypeShape::Reference { .. }) {
                    return malformed(
                        "storage interpretation cannot replace a descriptor crossing",
                    );
                }
                let Some(view) = terminal_semantics::representation_view_declaration(
                    module.structural_types.iter(),
                    *view,
                    *stored_scalar,
                ) else {
                    return malformed(
                        "storage interpretation has an unknown or incompatible declaration",
                    );
                };
                view.id
            }
            (StructuralPathSegment::Case(_), _) => {
                return malformed("structural case path requires its payload field");
            }
            (StructuralPathSegment::FixedByteRange { .. }, _) => {
                return malformed("fixed byte ranges require call-scoped byte-view custody");
            }
            (StructuralPathSegment::Referent, StructuralTypeShape::Reference { referent, .. }) => {
                *referent
            }
            (StructuralPathSegment::Referent, _) => {
                // A borrow formed at a non-reference leaf carries that leaf's
                // own storage: the projection's type is the leaf itself.
                structural_type
            }
            (_, StructuralTypeShape::Reference { .. }) => {
                return malformed("referent projection requires a reference carrier");
            }
            (StructuralPathSegment::Field(identity), shape) => {
                let fields = match payload_fields.take() {
                    Some(fields) => fields,
                    None => match shape {
                        terminal_psi::record_or_mixed_shape_pattern!(fields) => fields.as_slice(),
                        _ => {
                            return malformed(
                                "structural field path requires a record or case payload",
                            );
                        }
                    },
                };
                if identity.is_absent() {
                    return malformed("structural path field identity cannot be empty");
                }
                let Some(field) = terminal_psi::field_by_identity(fields, *identity) else {
                    return malformed("structural path has an unknown structural field");
                };
                if field.relevance.is_erased() {
                    return malformed("structural path cannot select an erased structural field");
                }
                match &field.field_type {
                    StructuralFieldType::Structural(next) => *next,
                    leaf => {
                        let Some(declaration) = leaf
                            .canonical_leaf_type()
                            .and_then(|leaf| leaf.leaf_declaration(module.structural_types.iter()))
                        else {
                            return malformed("structural path must retain structural custody");
                        };
                        declaration.id
                    }
                }
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) => {
                if index >= length {
                    return malformed("structural path fixed index is out of bounds");
                }
                *element
            }
            // A runtime index is structurally an element step; its bound is
            // an obligation the verifier reconstructs at the owning operation.
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::FixedArray { element, .. },
            ) => *element,
            (StructuralPathSegment::FixedIndex(_), StructuralTypeShape::Record { .. }) => {
                return malformed("structural path fixed index requires a fixed-array type");
            }
            (StructuralPathSegment::FixedIndex(_), StructuralTypeShape::Mixed { .. }) => {
                return malformed("structural path fixed index requires a fixed-array type");
            }
            (StructuralPathSegment::RuntimeIndex { .. }, StructuralTypeShape::Record { .. }) => {
                return malformed("structural path runtime index requires a fixed-array type");
            }
            (StructuralPathSegment::RuntimeIndex { .. }, StructuralTypeShape::Mixed { .. }) => {
                return malformed("structural path runtime index requires a fixed-array type");
            }
            (_, StructuralTypeShape::Sum { .. }) => {
                return malformed("structural path cannot traverse a payload-less sum");
            }
            (_, StructuralTypeShape::PrimitiveScalar(_)) => {
                return malformed("primitive-scalar structural type has no projected children");
            }
            (_, StructuralTypeShape::ByteSequence(_)) => {
                return malformed("byte-sequence structural type has no projected children");
            }
            (_, StructuralTypeShape::BindingCarrier) => {
                return malformed("fused service binding has no projected children");
            }
            // A view's extent is its stored runtime length: an index step
            // selects an element whose bound the verifier reconstructs at
            // the owning operation, as for the fixed-array siblings.
            (
                StructuralPathSegment::FixedIndex(_),
                StructuralTypeShape::ElementView { element, .. },
            ) => *element,
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::ElementView { element, .. },
            ) => *element,
        };
    }
    if payload_fields.is_some() {
        return malformed("structural case path requires its payload field");
    }
    Ok(structural_type)
}

fn validate_structural_parameters(
    module: &TerminalModule,
    parameters: &[StructuralParameterDeclaration],
) -> Result<(), CodecError> {
    let mut places = BTreeSet::new();
    for parameter in parameters {
        if !places.insert(parameter.place) {
            return malformed("structural parameters reuse a place identity");
        }
        if !has_structural_type(module, parameter.structural_type) {
            return malformed("structural parameter references an unknown type");
        }
        if parameter.access == terminal_psi::StructuralAccess::Owned
            && terminal_semantics::contains_placed_carrier(module, parameter.structural_type)
        {
            return malformed("ordinary owned parameters cannot receive placed custody");
        }
        validate_structural_qualifications(
            module,
            parameter.structural_type,
            &parameter.qualifications,
            &parameter.projected_qualifications,
        )?;
    }
    Ok(())
}

fn validate_operation_foundation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    // Interpretation steps belong to a returned-origin recipe, never an
    // executable operand. Only an actual EstablishReference operation can
    // introduce a view; a path cannot mint its representation authority.
    if operation
        .kind
        .structural_projections()
        .iter()
        .any(|projection| {
            projection
                .path
                .iter()
                .any(|segment| matches!(segment, StructuralPathSegment::RepresentationView { .. }))
        })
    {
        return malformed("returned-origin interpretation is not an executable projection");
    }
    if let Some(result) = operation.result.structural() {
        if terminal_semantics::contains_placed_carrier(module, result.structural_type) {
            return malformed("ordinary value production cannot establish placed custody");
        }
        validate_structural_qualifications(
            module,
            result.structural_type,
            &result.qualifications,
            &result.projected_qualifications,
        )?;
    }
    match &operation.kind {
        OperationKind::EstablishPlacedReference { .. } => {
            storage_foundations::validate_establish_placed_reference(module, machine, operation)?
        }
        OperationKind::EstablishReference { .. } => {
            storage_foundations::validate_establish_reference(module, machine, operation)?
        }
        OperationKind::ReleaseReference { .. } => {
            storage_foundations::validate_release_reference(module, machine, operation)?
        }
        OperationKind::ByteSequenceStore { .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("whole byte store requires Unit");
            }
            // Independent module validation replays exact borrowed owner,
            // immutable source, live extent, predicates and capacity evidence.
        }
        OperationKind::StructuralByteSequenceFieldStore { .. } => {
            storage_foundations::validate_structural_byte_sequence_field_store(
                module, machine, operation,
            )?
        }
        OperationKind::StructuralByteViewFieldStore { .. } => {
            storage_foundations::validate_structural_byte_view_field_store(
                module, machine, operation,
            )?
        }
        OperationKind::StructuralByteSequenceFieldLength { .. } => {
            storage_foundations::validate_structural_byte_sequence_field_length(operation)?
        }
        OperationKind::StructuralByteSequenceFieldByteStore { .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("byte field byte store requires Unit");
            }
            // Independent module validation reconstructs exact current length
            // provenance, scalar operand types, custody, and index bounds.
        }
        OperationKind::StructuralByteSequenceFieldRead { .. } => {
            storage_foundations::validate_structural_byte_sequence_field_read(operation)?
        }
        OperationKind::ByteSequenceSubslice { .. } => {
            storage_foundations::validate_byte_sequence_subslice(module, machine, operation)?
        }
        OperationKind::EstablishElementView { .. } => {
            storage_foundations::validate_establish_element_view(module, machine, operation)?
        }
        OperationKind::EstablishMutableElementView { .. } => {
            storage_foundations::validate_establish_mutable_element_view(
                module, machine, operation,
            )?
        }
        OperationKind::ElementViewLength { .. } => {
            storage_foundations::validate_element_view_length(module, machine, operation)?
        }
        OperationKind::ElementViewRead { .. } => {
            storage_foundations::validate_element_view_read(module, machine, operation)?
        }
        OperationKind::ElementViewSubslice { .. } => {
            storage_foundations::validate_element_view_subslice(module, machine, operation)?
        }
        OperationKind::ByteSequenceWrite { .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("byte-sequence write requires a Unit result");
            }
        }
        OperationKind::ElementViewWrite {
            destination,
            path,
            index,
            obligation,
            ..
        } => {
            if operation.result != OperationResult::Unit {
                return malformed("element-view write requires a Unit result");
            }
            let root = terminal_semantics::descriptor_results::terminal_payload_contract(
                machine,
                destination.place,
            )
            .ok_or(CodecError::MalformedStructuralFoundation(
                "element-view write root is absent",
            ))?;
            // Preserve the original root roster and every stored-descriptor
            // crossing before selecting the written element and its field.
            let mut written = destination.path.clone();
            written.push(terminal_psi::StructuralPathSegment::RuntimeIndex {
                index: *index,
                obligation: *obligation,
            });
            written.extend(path.iter().cloned());
            if !terminal_semantics::structural_memberships_disjoint_from_write(
                &root.qualifications,
                &root.projected_qualifications,
                &written,
            ) {
                return malformed(
                    "element-view scalar write overlaps qualifications it does not restore",
                );
            }
        }
        OperationKind::StructuralCaseMembership { .. } => {
            storage_foundations::validate_structural_case_membership(operation)?
        }
        terminal_psi::structural_leaf_copy_pair_pattern!() => {
            storage_foundations::validate_structural_leaf_copy(module, machine, operation)?
        }
        OperationKind::ByteSequenceRead { .. } => {
            storage_foundations::validate_byte_sequence_read(operation)?
        }
        OperationKind::ByteSequenceLength { .. } => {
            storage_foundations::validate_byte_sequence_length(module, machine, operation)?
        }
        OperationKind::WriteOnlyPrimitiveStore { .. } => {
            storage_foundations::validate_write_only_primitive_store(module, machine, operation)?
        }
        OperationKind::StructuralScalarFieldStore { .. } => {
            storage_foundations::validate_structural_scalar_field_store(module, machine, operation)?
        }
        OperationKind::MoveStructuralField { .. } => {
            storage_foundations::validate_move_structural_field(module, machine, operation)?
        }
        OperationKind::StoreStructuralField { .. } => {
            storage_foundations::validate_store_structural_field(module, machine, operation)?
        }
        OperationKind::ScalarStructuralField { .. }
        | OperationKind::BooleanStructuralField { .. } => {
            storage_foundations::validate_scalar_structural_field(module, machine, operation)?
        }
        OperationKind::EstablishScalarArray { .. } => {
            value_foundations::validate_establish_scalar_array(module, machine, operation)?
        }
        OperationKind::EstablishScalarCase { .. } => {
            value_foundations::validate_establish_scalar_case(module, machine, operation)?
        }
        OperationKind::EstablishByteSequenceConcatenation { .. } => {
            value_foundations::validate_establish_byte_sequence_concatenation(
                module, machine, operation,
            )?
        }
        OperationKind::EstablishByteSequenceLiteral { .. } => {
            value_foundations::validate_establish_byte_sequence_literal(module, machine, operation)?
        }
        OperationKind::CallUnit { .. } => {
            call_foundations::validate_call_unit(module, machine, operation)?
        }
        OperationKind::CallDynamic { .. } => {
            call_foundations::validate_call_dynamic(module, machine, operation)?
        }
        OperationKind::CallStructuralScalar { .. } => {
            call_foundations::validate_call_structural_scalar(module, machine, operation)?
        }
        terminal_psi::call_structural_pair_pattern!(..) => {
            call_foundations::validate_call_structural(module, machine, operation)?
        }
        OperationKind::BoundaryCall { .. } => {
            call_foundations::validate_boundary_call(module, machine, operation)?
        }
        OperationKind::PortWrite { .. } => {
            call_foundations::validate_port_write(module, operation)?
        }
        OperationKind::RegisterAccess { .. } => {
            call_foundations::validate_register_access(module, machine, operation)?
        }
        OperationKind::EstablishZeroedStructural { .. } => {
            value_foundations::validate_establish_zeroed_structural(module, machine, operation)?
        }
        OperationKind::EstablishStructuralLocal { .. } => {
            value_foundations::validate_establish_structural_local(module, machine, operation)?
        }
        OperationKind::EstablishTrivialAffineLocal { .. } => {
            value_foundations::validate_establish_trivial_affine_local(module, machine, operation)?
        }
        OperationKind::EstablishStructuralArray { .. } => {
            value_foundations::validate_establish_structural_array(module, machine, operation)?
        }
        OperationKind::EstablishRecord { .. } => {
            value_foundations::validate_establish_record(module, machine, operation)?
        }
        OperationKind::EstablishStructuralCase { .. } => {
            value_foundations::validate_establish_structural_case(module, machine, operation)?
        }
        OperationKind::EstablishPrimitiveLocal { .. } => {
            value_foundations::validate_establish_primitive_local(module, machine, operation)?
        }
        OperationKind::Call { .. } => {
            if !matches!(operation.result, OperationResult::Scalar(_)) {
                return malformed("scalar call declares a Unit result");
            }
        }
        // The result shape follows the event: an observing event defines the
        // observed prior and a store defines nothing. The independent verifier
        // reconstructs location, authority, orderings, and types.
        OperationKind::AtomicFence { .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("atomic fence must define no value");
            }
        }
        OperationKind::AtomicAccess { event, .. } => {
            let shape_matches = match &operation.result {
                OperationResult::Scalar(_) => event.observes_resident(),
                OperationResult::Unit => !event.observes_resident(),
                OperationResult::Structural(_) => false,
            };
            if !shape_matches {
                return malformed("atomic access result disagrees with its event");
            }
        }
        _ => {
            if !matches!(operation.result, OperationResult::Scalar(_)) {
                return malformed("scalar operation declares a Unit result");
            }
        }
    }
    Ok(())
}

/// Membership rows are semantic metadata, not access permissions. Codec
/// admission checks their exact catalog, carrier and occurrence-relative path;
/// source checking owns their proof rather than a second compiler replay.
fn validate_structural_qualifications(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    whole: &[semantic_vocabulary::StructuralDomainId],
    projected: &[terminal_psi::StructuralPathQualification],
) -> Result<(), CodecError> {
    if whole.windows(2).any(|pair| pair[0] >= pair[1])
        || projected.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return malformed("structural qualifications are noncanonical");
    }
    for (domain, carrier) in whole
        .iter()
        .map(|domain| (*domain, Ok(structural_type)))
        .chain(projected.iter().map(|row| {
            (
                row.domain,
                if row.path.is_empty() {
                    malformed("projected qualification requires a child path")
                } else {
                    terminal_semantics::qualification_path_carrier(
                        module.structural_types.iter(),
                        structural_type,
                        &row.path,
                    )
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "qualification pattern has no exact carrier",
                    ))
                },
            )
        }))
    {
        let carrier = carrier?;
        let Some(declaration) = module.structural_domain(domain) else {
            return malformed("structural qualification references an unknown domain");
        };
        if declaration.carrier != carrier {
            return malformed("structural qualification has the wrong carrier");
        }
    }
    Ok(())
}

// Claim-free results share the ordinary exact call signature. A reference
// carrier still owes loan custody: the independent verifier reconstructs its
// source mapping and lifetime instead of treating an empty claim roster as
// permission to discard or duplicate the referent.
// Both codec entrances require that validation before accepting a module. The
// wire format has no separate payload-shape distinction for claim-free calls;
// a second scalar/flat-record whitelist here would reject verified nested and
// reference-bearing records without adding an encoding or custody check.
fn callee_exact_payloadless_return(callee: &TerminalMachine) -> bool {
    let Some(result) = callee.result.structural() else {
        return false;
    };
    let mut has_return = false;
    for block in &callee.blocks {
        let Terminator::ReturnStructural {
            source,
            returned_claims,
            ..
        } = &block.terminator
        else {
            continue;
        };
        has_return = true;
        if !returned_claims.is_empty() {
            return false;
        }
        let Some(producer) = callee.structural_places.iter().find_map(|place| {
            (place.id == *source)
                .then_some(place.kind)
                .and_then(|kind| match kind {
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } if structural_type == result.structural_type => Some(producer),
                    _ => None,
                })
        }) else {
            return false;
        };
        let Some(operation) = callee.operation(producer) else {
            return false;
        };
        if !matches!(&operation.kind, OperationKind::EstablishScalarCase { fields, .. } if fields.is_empty())
            || !operation
                .result
                .structural()
                .is_some_and(|operation_result| {
                    operation_result.place == *source
                        && operation_result.structural_type == result.structural_type
                        && operation_result.multiplicity
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                        && operation_result.qualifications.is_empty()
                        && operation_result.claims.is_empty()
                })
        {
            return false;
        }
    }
    has_return
        && callee.operations().all(|operation| {
            !matches!(
                operation.kind,
                OperationKind::Call { .. }
                    | OperationKind::CallUnit { .. }
                    | OperationKind::CallStructuralScalar { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::BoundaryCall { .. }
            )
        })
}

#[derive(Clone, Copy)]
enum StructuralArgumentPresentation {
    Ordinary,
    Boundary,
}

fn validate_structural_arguments(
    module: &TerminalModule,
    machine: &TerminalMachine,
    arguments: &[StructuralArgument],
    expected: &[StructuralParameterDeclaration],
    presentation: StructuralArgumentPresentation,
) -> Result<(), CodecError> {
    for (argument, expected) in arguments.iter().zip(expected) {
        if expected.access == terminal_psi::StructuralAccess::Owned
            && terminal_semantics::contains_placed_carrier(module, expected.structural_type)
        {
            return malformed("ordinary owned call transport cannot carry placed custody");
        }
        let Some(actual_type) = structural_place_type(machine, argument.place) else {
            return malformed("structural argument references an unknown structural place");
        };
        let parameter = machine.signature_structural_parameter(argument.place);
        let result = machine
            .operations()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == argument.place);
        let owned_array_payload = terminal_semantics::scalar_array_leaf_shape(
            module.structural_types.iter(),
            actual_type,
        )
        .is_some()
            && (parameter.is_some_and(|parameter| {
                parameter.access == terminal_psi::StructuralAccess::Owned
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
            }) || result.is_some());
        if owned_array_payload {
            let payload_loan = argument.access == expected.access
                && argument.access != terminal_psi::StructuralAccess::Owned;
            // Predicate-only array memberships travel in the ordinary source
            // contract. This classifies the retained theory; independent call
            // validation still authenticates its actual source and handback.
            let supported = |carrier, whole: &[_], projected: &[_]| {
                projected.is_empty()
                    || payload_loan
                        && terminal_semantics::pure_byte_qualifications(
                            module.structural_types.iter(),
                            module.structural_domains.iter(),
                            carrier,
                            whole,
                            projected,
                        )
            };
            let plain_source = parameter.is_some_and(|parameter| {
                parameter.access == terminal_psi::StructuralAccess::Owned
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && supported(
                        actual_type,
                        &parameter.qualifications,
                        &parameter.projected_qualifications,
                    )
            }) || result.is_some_and(|result| {
                result.multiplicity == StructuralMultiplicity::Unrestricted
                    && supported(
                        actual_type,
                        &result.qualifications,
                        &result.projected_qualifications,
                    )
                    && result.claims.is_empty()
            });
            // A parameter or completed producer lends its established
            // backing at either call kind. Root predicate qualification does not
            // change array geometry; module validation retains membership
            // obligations. Preserve the root and projection;
            // the shared path resolver below checks the exact lent type.
            // Independent verification checks availability, runtime bounds
            // and exclusive overlap; canonicalization establishes none of them.
            let admitted_access = matches!(
                (argument.access, expected.access),
                (
                    terminal_psi::StructuralAccess::Owned,
                    terminal_psi::StructuralAccess::Owned
                )
            ) || payload_loan;
            if !plain_source
                || !admitted_access
                || (!payload_loan
                    && argument
                        .path
                        .iter()
                        .any(|segment| !matches!(segment, StructuralPathSegment::FixedIndex(_))))
                || expected.multiplicity != StructuralMultiplicity::Unrestricted
                || !supported(
                    expected.structural_type,
                    &expected.qualifications,
                    &expected.projected_qualifications,
                )
                || machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == argument.place)
                || machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == argument.place)
            {
                return malformed(
                    "primitive-array argument requires owned element selection or loan custody",
                );
            }
        }
        // Geometry uses the exact current home, whether an arrival or an
        // operation result. Owned backing remains owned while supplying a
        // call-scoped loan; the shared contract validates that presentation.
        let result_backing = result.map(|result| StructuralParameterDeclaration {
            place: argument.place,
            position: expected.position,
            structural_type: actual_type,
            multiplicity: result.multiplicity,
            access: terminal_psi::StructuralAccess::Owned,
            qualifications: result.qualifications.clone(),
            projected_qualifications: result.projected_qualifications.clone(),
        });
        let backing = parameter
            .or_else(|| block_parameter(machine, argument.place))
            .or(result_backing.as_ref());
        if backing.is_some_and(|actual| {
            (terminal_semantics::fixed_byte_array_loan(
                module.structural_types.iter(),
                module.structural_domains.iter(),
                actual,
                argument,
                expected,
            )
            .is_some()
                || terminal_semantics::fixed_element_array_extent(
                    module.structural_types.iter(),
                    actual,
                    argument,
                    expected,
                )
                .is_some())
                && !result.is_some_and(|result| {
                    !result.claims.is_empty() || !result.qualification_establishments.is_empty()
                })
                && !machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == argument.place)
                && !machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == argument.place)
        }) {
            continue;
        }
        // Inline bounded owners have no standalone field type to resolve.
        // Preserve the original root/path and replay the exact capacity and
        // predicate contract instead of manufacturing a structural leaf type.
        if matches!(presentation, StructuralArgumentPresentation::Ordinary)
            && backing.is_some_and(|actual| {
                terminal_semantics::bounded_byte_owner_capacity(module, actual, argument, expected)
                    .is_some()
            })
            && !result.is_some_and(|result| {
                !result.claims.is_empty() || !result.qualification_establishments.is_empty()
            })
            && !machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == argument.place)
            && !machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place)
        {
            continue;
        }
        let inline_byte_view = match presentation {
            StructuralArgumentPresentation::Boundary => true,
            StructuralArgumentPresentation::Ordinary => {
                expected.multiplicity == StructuralMultiplicity::Unrestricted
                    && !argument.path.is_empty()
                    && argument
                        .path
                        .iter()
                        .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
                    && machine.structural_parameters.iter().any(|parameter| {
                        parameter.place == argument.place
                            && parameter.access == terminal_psi::StructuralAccess::MutableBorrow
                            && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    })
            }
        };
        if inline_byte_view
            && terminal_semantics::boundary_buffer_capacity(
                module.structural_types.iter(),
                actual_type,
                argument,
                expected,
            )
            .is_some()
        {
            continue;
        }
        // A shared view observes the live bytes of either whole or projected
        // bounded storage. Call kind and entry-versus-block placement do not
        // change that presentation; verification separately checks source
        // permission, multiplicity, availability and overlapping loans.
        let shared_byte_field_loan = terminal_semantics::shared_boundary_buffer_capacity(
            module.structural_types.iter(),
            actual_type,
            argument,
            expected,
        )
        .is_some()
            && expected.multiplicity == StructuralMultiplicity::Unrestricted;
        if shared_byte_field_loan {
            continue;
        }
        // A stored shared view field lends its own descriptor: the leaf has
        // no standalone structural type, so the field's carrier and the
        // formal's shared view shape are the whole canonical check. Predicate
        // implication and custody stay with module validation.
        if terminal_semantics::stored_view_field_presentation(
            module.structural_types.iter(),
            actual_type,
            argument,
            expected,
        )
        .is_some()
        {
            continue;
        }
        // The descriptor is a call presentation, not a replacement storage
        // declaration. Both wire entrances independently validate the module
        // after this shape check, including live borrow and result custody.
        if owned_byte_view_presentation(module, machine, argument, expected) {
            continue;
        }
        // Primitive loans present the original storage, never an owned scalar.
        // Psi owns source predicate preservation at writable handback; this
        // representation boundary checks carrier, access and exact coordinates.
        let borrowed_endpoint = (argument.access == expected.access
            && argument.access != terminal_psi::StructuralAccess::Owned)
            .then(|| {
                terminal_semantics::borrowed_primitive_projection_declaration(
                    module.structural_types.iter(),
                    actual_type,
                    &argument.path,
                    Some(expected.structural_type),
                )
            })
            .flatten();
        let actual_type = match borrowed_endpoint {
            Some(endpoint) => endpoint,
            None => validate_structural_path(module, actual_type, &argument.path)?,
        };
        if actual_type != expected.structural_type
            && !(argument.access == expected.access
                && argument.access != terminal_psi::StructuralAccess::Owned
                && terminal_semantics::borrowed_referent_matches(
                    module.structural_types.iter(),
                    actual_type,
                    expected.structural_type,
                ))
        {
            return malformed("structural argument has the wrong concrete type");
        }
    }
    Ok(())
}

fn owned_byte_view_presentation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> bool {
    if !argument.path.is_empty()
        || argument.access != terminal_psi::StructuralAccess::SharedBorrow
        || expected.access != terminal_psi::StructuralAccess::SharedBorrow
        || expected.multiplicity != StructuralMultiplicity::Unrestricted
        || !expected.projected_qualifications.is_empty()
        || machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == argument.place)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == argument.place)
        || terminal_semantics::byte_owned_result_predicates(module, machine, argument.place)
            .is_none()
        || !module.structural_types.iter().any(|declaration| {
            declaration.id == expected.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                        }
                    )
                )
        })
    {
        return false;
    }
    let Some(required) = terminal_semantics::byte_parameter_predicates(module, expected) else {
        return false;
    };
    terminal_semantics::byte_sequence_source_predicates(module, machine, argument.place)
        .is_some_and(|available| available.implies(required))
}

fn structural_place_type(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<StructuralTypeId> {
    machine
        .structural_parameters
        .iter()
        .find_map(|parameter| (parameter.place == place).then_some(parameter.structural_type))
        .or_else(|| {
            machine.structural_places.iter().find_map(|declaration| {
                if declaration.id != place {
                    return None;
                }
                match declaration.kind {
                    StructuralPlaceKind::BlockParameter { .. } => {
                        block_parameter(machine, place).map(|parameter| parameter.structural_type)
                    }
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::TrivialAffineLocal {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    } => Some(structural_type),
                    StructuralPlaceKind::Parameter { .. }
                    | StructuralPlaceKind::ProviderAttachment { .. }
                    | StructuralPlaceKind::Result => None,
                }
            })
        })
}

/// The declaration a block parameter place binds at its block's entry.
fn block_parameter(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<&StructuralParameterDeclaration> {
    machine.structural_places.iter().find_map(|declaration| {
        let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
            return None;
        };
        (declaration.id == place).then_some(())?;
        machine
            .block(&block)?
            .structural_parameters
            .get(position as usize)
            .filter(|parameter| parameter.place == place)
    })
}

/// Cleanup custody comes from the exact operation result declaration, not the
/// operation's construction mechanism. A matching type alone is insufficient.
/// Qualifications remain attached to that value; their presence is not a
/// cleanup prohibition. Current-ownership checking independently enforces
/// residual partitions, live claims, loans and nominal cleanup obligations.
fn is_owned_cleanup_operation_result(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> bool {
    let mut places = machine
        .structural_places
        .iter()
        .filter(|candidate| candidate.id == place);
    let Some(declaration) = places.next() else {
        return false;
    };
    if places.next().is_some() {
        return false;
    }
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return false;
    };
    let mut operations = machine
        .operations()
        .filter(|operation| operation.id == producer);
    let Some(operation) = operations.next() else {
        return false;
    };
    if operations.next().is_some() {
        return false;
    }
    operation.result.as_structural().is_some_and(|result| {
        result.place == place
            && result.structural_type == structural_type
            && matches!(
                result.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Linear
            )
    })
}

fn validate_claim_actions(
    machine: &TerminalMachine,
    arguments: &[StructuralArgument],
    claims: impl Iterator<Item = (ClaimId, u32)>,
) -> Result<(), CodecError> {
    let mut transferred = BTreeSet::new();
    for (claim, argument_index) in claims {
        if arguments.get(argument_index as usize).is_none() {
            return malformed("claim action has an unknown structural argument index");
        }
        if !transferred.insert(claim) {
            return malformed("claim action repeats a claim identity");
        }
        // Declarations authenticate machine-local identity, not the claim's
        // current coordinates. A store can move a result-root claim beneath
        // an aggregate field; a later projection names that new home without
        // changing the original declaration. Actual/formal typed paths are
        // checked by the call foundation. Source checking, ownership replay
        // and consuming custody check the live place and relative transfer
        // path; comparing the original path here would reject valid moves.
        let declared = machine
            .entry_claims
            .iter()
            .chain(&machine.block_entry_claims)
            .any(|entry| entry.claim == claim)
            || machine.loan_completions.iter().any(|completion| {
                completion
                    .claims
                    .iter()
                    .any(|binding| binding.claim == claim)
            })
            || machine.operations().any(|operation| {
                let Some(result) = operation.result.structural() else {
                    return false;
                };
                // Multiplicity does not describe a result's claim set: an affine
                // owner carries its linear members' claims at their paths.
                machine.structural_places.iter().any(|declaration| {
                    declaration.id == result.place
                        && matches!(declaration.kind,
                        StructuralPlaceKind::OperationResult { producer, structural_type }
                        if producer == operation.id && structural_type == result.structural_type)
                }) && result.claims.iter().any(|binding| binding.claim == claim)
            });
        if !declared {
            return malformed("claim action has an undeclared structural claim identity");
        }
    }
    Ok(())
}

fn has_structural_type(module: &TerminalModule, id: StructuralTypeId) -> bool {
    module.structural_type(id).is_some()
}

fn validate_structural_type_graph(module: &TerminalModule) -> Result<(), CodecError> {
    fn visit(
        module: &TerminalModule,
        id: StructuralTypeId,
        active: &mut BTreeSet<StructuralTypeId>,
        complete: &mut BTreeSet<StructuralTypeId>,
    ) -> Result<(), CodecError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return malformed("structural type graph contains a by-value cycle");
        }
        let declaration = module
            .structural_type(id)
            .expect("structural field targets were validated before graph traversal");
        match &declaration.shape {
            // A reference does not inline its referent storage. The target's
            // existence was checked above; following it here invents a by-value cycle.
            StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::BindingCarrier
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::ElementView { .. } => {}
            StructuralTypeShape::Record { fields, .. } => {
                for field in fields {
                    if let Some(target) = field.field_type.as_structural() {
                        visit(module, target, active, complete)?;
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                visit(module, *element, active, complete)?;
            }
            StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let Some(target) = field.field_type.as_structural() {
                        visit(module, target, active, complete)?;
                    }
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    if let Some(target) = field.field_type.as_structural() {
                        visit(module, target, active, complete)?;
                    }
                }
            }
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for declaration in &module.structural_types {
        visit(module, declaration.id, &mut active, &mut complete)?;
    }
    Ok(())
}

fn validate_service_parent_graph(module: &TerminalModule) -> Result<(), CodecError> {
    fn visit(
        module: &TerminalModule,
        id: ServiceId,
        active: &mut BTreeSet<ServiceId>,
        complete: &mut BTreeSet<ServiceId>,
    ) -> Result<(), CodecError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return malformed("service parent graph contains a cycle");
        }
        let declaration = module
            .service(id)
            .expect("service parent targets were validated before graph traversal");
        for parent in &declaration.parents {
            visit(module, *parent, active, complete)?;
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for declaration in &module.services {
        visit(module, declaration.id, &mut active, &mut complete)?;
    }
    for declaration in &module.services {
        for parent in &declaration.parents {
            let parent = module
                .service(*parent)
                .expect("service parent targets were validated before closure validation");
            if parent
                .parents
                .iter()
                .any(|ancestor| !declaration.parents.contains(ancestor))
            {
                return malformed("service parent closure is incomplete");
            }
        }
    }
    Ok(())
}

fn has_service(module: &TerminalModule, id: ServiceId) -> bool {
    module
        .services
        .iter()
        .any(|declaration| declaration.id == id)
}

fn require_known_services(
    module: &TerminalModule,
    services: &[ServiceId],
) -> Result<(), CodecError> {
    if services
        .iter()
        .any(|service| !has_service(module, *service))
    {
        return malformed("published service ceiling references an unknown service");
    }
    Ok(())
}

fn require_unique_nonempty_identities(
    identities: impl Iterator<Item = TerminalIdentityIndex>,
    label: &'static str,
) -> Result<(), CodecError> {
    let mut seen = BTreeSet::new();
    for identity in identities {
        if identity.is_absent() || !seen.insert(identity) {
            return Err(CodecError::MalformedStructuralFoundation(label));
        }
    }
    Ok(())
}

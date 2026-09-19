//! Closed representation-foundation validation run before encoding.
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
mod storage_foundations;
mod structural_type_foundations;
mod value_foundations;

use crate::codec_error::{CodecError, malformed};

use semantic_vocabulary::{ClaimId, ServiceId, StructuralPlaceKind, StructuralTypeId};
use std::collections::BTreeSet;
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralArgument, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeShape, TerminalMachine, TerminalModule, Terminator,
    is_bounded_structural_scalar_store_path,
};

/// Validate the closed representation foundation needed before the independent
/// semantic verifier learns these operations. This checks identities, exact
/// carrier relationships, signatures, and operation/result shape; it does not
/// prove qualifications, reach closure, or claim dataflow.
pub(crate) fn validate_structural_foundation(module: &TerminalModule) -> Result<(), CodecError> {
    require_unique_nonempty_identities(
        module
            .structural_types
            .iter()
            .map(|declaration| declaration.identity.as_str()),
        "structural type identity",
    )?;
    require_unique_nonempty_identities(
        module
            .structural_domains
            .iter()
            .map(|declaration| declaration.identity.as_str()),
        "structural domain identity",
    )?;
    require_unique_nonempty_identities(
        module
            .services
            .iter()
            .map(|declaration| declaration.identity.as_str()),
        "service identity",
    )?;
    require_unique_nonempty_identities(
        module
            .boundary_machines
            .iter()
            .map(|declaration| declaration.identity.as_str()),
        "boundary machine identity",
    )?;

    for declaration in &module.structural_types {
        structural_type_foundations::validate_type_shape(module, declaration)?;
    }
    validate_structural_type_graph(module)?;
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
        .and_then(|attachment| {
            module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == attachment)
        })
        .and_then(|attachment| match &attachment.shape {
            StructuralTypeShape::Record { fields } => Some(
                fields
                    .iter()
                    .filter(|field| {
                        !field.relevance.is_erased()
                            && matches!(field.field_type, StructuralFieldType::Erased { .. })
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    if provider_roots.is_empty() && provider_fields.is_empty() {
        return Ok(());
    }
    let [provider_field] = provider_fields.as_slice() else {
        return malformed("provider-backed attachment specialization is incomplete");
    };
    let Some(attachment) = machine.attachment else {
        return malformed("provider-backed attachment specialization is incomplete");
    };
    let self_parameters = machine
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .collect::<Vec<_>>();
    let invalid_self = match self_parameters.as_slice() {
        [] => false,
        [parameter] => parameter.position != 0 || parameter.structural_type != attachment,
        _ => true,
    };
    let mut boundaries = BTreeSet::new();
    if invalid_self
        || provider_roots
            .iter()
            .any(|(root_attachment, field, boundary)| {
                *root_attachment != attachment
                    || *field != provider_field.id
                    || !boundaries.insert(*boundary)
                    || module
                        .boundary_machines
                        .iter()
                        .find(|declaration| declaration.id == *boundary)
                        .is_none_or(|declaration| declaration.attachment.is_some())
            })
    {
        return malformed("provider-backed attachment specialization is incomplete");
    }
    let called = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::BoundaryCall { boundary, .. } => Some(boundary),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if called != boundaries {
        return malformed("provider-backed attachment specialization is incomplete");
    }
    Ok(())
}

pub(crate) fn validate_structural_path(
    module: &TerminalModule,
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<StructuralTypeId, CodecError> {
    for segment in path {
        let Some(declaration) = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
        else {
            return malformed("structural path has an unknown structural type");
        };
        structural_type = match (segment, &declaration.shape) {
            (StructuralPathSegment::Referent, StructuralTypeShape::Reference { referent, .. }) => {
                *referent
            }
            (StructuralPathSegment::Referent, _) | (_, StructuralTypeShape::Reference { .. }) => {
                return malformed("referent projection requires a reference carrier");
            }
            (
                StructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. },
            ) => {
                if identity.is_empty() {
                    return malformed("structural path field identity cannot be empty");
                }
                let Some(field) = fields.iter().find(|field| field.identity == *identity) else {
                    return malformed("structural path has an unknown structural field");
                };
                if field.relevance.is_erased() {
                    return malformed("structural path cannot select an erased structural field");
                }
                match &field.field_type {
                    StructuralFieldType::Structural(next) => *next,
                    leaf => {
                        let Some(shape) = leaf.canonical_leaf_shape() else {
                            return malformed("structural path must retain structural custody");
                        };
                        let Some(id) = module
                            .structural_types
                            .iter()
                            .find(|declaration| declaration.shape == shape)
                            .map(|declaration| declaration.id)
                        else {
                            return malformed("structural path must retain structural custody");
                        };
                        id
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
            (StructuralPathSegment::Field(_), StructuralTypeShape::FixedArray { .. }) => {
                return malformed("structural path field requires a record type");
            }
            (StructuralPathSegment::FixedIndex(_), StructuralTypeShape::Record { .. }) => {
                return malformed("structural path fixed index requires a fixed-array type");
            }
            (StructuralPathSegment::FixedIndex(_), StructuralTypeShape::Mixed { .. }) => {
                return malformed("structural path fixed index requires a fixed-array type");
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
        };
    }
    Ok(structural_type)
}

fn validate_structural_parameters(
    module: &TerminalModule,
    parameters: &[StructuralParameterDeclaration],
) -> Result<(), CodecError> {
    let mut places = BTreeSet::new();
    let mut self_count = 0_u32;
    for parameter in parameters {
        if !places.insert(parameter.place) {
            return malformed("structural parameters reuse a place identity");
        }
        self_count += u32::from(parameter.is_self);
        if self_count > 1 {
            return malformed("structural signature declares more than one self parameter");
        }
        if !has_structural_type(module, parameter.structural_type) {
            return malformed("structural parameter references an unknown type");
        }
        for qualification in &parameter.qualifications {
            let Some(domain) = module
                .structural_domains
                .iter()
                .find(|domain| domain.id == *qualification)
            else {
                return malformed("structural parameter references an unknown qualification");
            };
            if domain.carrier != parameter.structural_type {
                return malformed("structural parameter qualification has the wrong carrier");
            }
        }
    }
    Ok(())
}

fn validate_operation_foundation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    match &operation.kind {
        OperationKind::EstablishReference { .. } => {
            storage_foundations::validate_establish_reference(module, machine, operation)?
        }
        OperationKind::ReleaseReference { .. } => {
            storage_foundations::validate_release_reference(module, machine, operation)?
        }
        OperationKind::StructuralByteSequenceFieldStore { .. } => {
            storage_foundations::validate_structural_byte_sequence_field_store(
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
        OperationKind::ByteSequenceSubslice { .. } => {
            storage_foundations::validate_byte_sequence_subslice(module, machine, operation)?
        }
        OperationKind::ByteSequenceWrite { .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("byte-sequence write requires a Unit result");
            }
        }
        OperationKind::StructuralCaseMembership { .. } => {
            storage_foundations::validate_structural_case_membership(operation)?
        }
        OperationKind::ByteSequenceRead { .. } => {
            storage_foundations::validate_byte_sequence_read(operation)?
        }
        OperationKind::ByteSequenceLength { .. } => {
            storage_foundations::validate_byte_sequence_length(operation)?
        }
        OperationKind::WriteOnlyPrimitiveStore { .. } => {
            storage_foundations::validate_write_only_primitive_store(module, machine, operation)?
        }
        OperationKind::WriteOnlyIndexedPrimitiveStore { .. } => {
            storage_foundations::validate_write_only_indexed_primitive_store(
                module, machine, operation,
            )?
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
        OperationKind::IntegerStructuralField { .. }
        | OperationKind::BooleanStructuralField { .. } => {
            storage_foundations::validate_integer_structural_field(module, machine, operation)?
        }
        OperationKind::EstablishScalarArray { .. } => {
            value_foundations::validate_establish_scalar_array(module, machine, operation)?
        }
        OperationKind::EstablishScalarCase { .. } => {
            value_foundations::validate_establish_scalar_case(module, machine, operation)?
        }
        OperationKind::EstablishByteSequenceLiteral { .. } => {
            value_foundations::validate_establish_byte_sequence_literal(module, machine, operation)?
        }
        OperationKind::CallUnit { .. } => {
            call_foundations::validate_call_unit(module, machine, operation)?
        }
        OperationKind::CallStructuralScalar { .. } => {
            call_foundations::validate_call_structural_scalar(module, machine, operation)?
        }
        OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments,
            erased_arguments: _,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        } if operation
            .result
            .structural()
            .is_none_or(|result| result.multiplicity != StructuralMultiplicity::Linear) =>
        {
            let Some(callee) = module
                .machines
                .iter()
                .find(|candidate| candidate.id == *callee)
            else {
                return malformed("mixed structural-result call references an unknown callee");
            };
            let Some(expected_result) = callee.result.structural() else {
                return malformed(
                    "mixed structural-result call references a non-structural-result callee",
                );
            };
            let Some(actual_result) = operation.result.structural() else {
                return malformed("mixed structural-result call has no structural result");
            };
            if arguments.len() != callee.parameters.len()
                || structural_arguments.len() != callee.structural_parameters.len()
                || actual_result.structural_type != expected_result.structural_type
                || actual_result.multiplicity != expected_result.multiplicity
                || actual_result.qualifications != expected_result.qualifications
                || !actual_result.projected_qualifications.is_empty()
                || !actual_result.claims.is_empty()
                || !claim_transfers.is_empty()
                || !returned_claim_transfers.is_empty()
                || if is_claim_free_structural_call(actual_result, callee) {
                    requirement_obligations.len() != callee.contract.requires.len()
                } else {
                    !requirement_obligations.is_empty() || !crash_continuations.is_empty()
                }
            {
                return malformed("mixed structural-result call exceeds its bounded signature");
            }
            let Some(StructuralPlaceDeclaration {
                kind:
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    },
                ..
            }) = machine
                .structural_places
                .iter()
                .find(|place| place.id == actual_result.place)
            else {
                return malformed(
                    "mixed structural-result call result has no operation-result declaration",
                );
            };
            if *producer != operation.id || *structural_type != actual_result.structural_type {
                return malformed(
                    "mixed structural-result call result disagrees with its producer",
                );
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                StructuralArgumentPresentation::Ordinary,
            )?;
        }
        OperationKind::CallStructural { .. }
        | OperationKind::CallStructuralWithScalarArguments { .. } => {
            call_foundations::validate_call_structural(module, machine, operation)?
        }
        OperationKind::BoundaryCall { .. } => {
            call_foundations::validate_boundary_call(module, machine, operation)?
        }
        OperationKind::PortWrite { .. } => {
            call_foundations::validate_port_write(module, operation)?
        }
        OperationKind::EstablishTrivialAffineLocal { .. } => {
            value_foundations::validate_establish_trivial_affine_local(module, machine, operation)?
        }
        OperationKind::EstablishRecord { .. } => {
            value_foundations::validate_establish_record(module, machine, operation)?
        }
        OperationKind::EstablishPrimitiveLocal { .. } => {
            value_foundations::validate_establish_primitive_local(module, machine, operation)?
        }
        OperationKind::StoreDynamicDescriptor { .. } => {
            storage_foundations::validate_store_dynamic_descriptor(module, machine, operation)?
        }
        OperationKind::Call { .. } => {
            if !matches!(operation.result, OperationResult::Scalar(_)) {
                return malformed("scalar call declares a Unit result");
            }
        }
        OperationKind::CallDynamicUnit { .. } | OperationKind::CallDynamicParameterUnit { .. } => {
            if operation.result != OperationResult::Unit {
                return malformed("dynamic Unit call declares a result value");
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

// Claim-free results share the ordinary exact call signature. A reference
// carrier still owes loan custody: the independent verifier reconstructs its
// source mapping and lifetime instead of treating an empty claim roster as
// permission to discard or duplicate the referent.
// Both codec entrances require that validation before accepting a module. The
// wire format has no separate payload-shape distinction for claim-free calls;
// a second scalar/flat-record whitelist here would reject verified nested and
// reference-bearing records without adding an encoding or custody check.
fn is_claim_free_structural_call(
    result: &terminal_psi::StructuralOperationResult,
    callee: &TerminalMachine,
) -> bool {
    matches!(
        result.multiplicity,
        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
    ) && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && callee.entry_claims.is_empty()
        && callee.content_entry_claims.is_empty()
        && callee.contract.outcome_specific_ensures.is_empty()
}

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
        let Some(operation) = callee
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == producer)
        else {
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
        && callee
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .all(|operation| {
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
        let Some(actual_type) = structural_place_type(machine, argument.place) else {
            return malformed("structural argument references an unknown structural place");
        };
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place);
        let result = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
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
            let plain_source = parameter.is_some_and(|parameter| {
                parameter.access == terminal_psi::StructuralAccess::Owned
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            }) || result.is_some_and(|result| {
                result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
            });
            if matches!(presentation, StructuralArgumentPresentation::Boundary)
                || !plain_source
                || argument.access != terminal_psi::StructuralAccess::Owned
                || !argument.path.is_empty()
                || expected.access != terminal_psi::StructuralAccess::Owned
                || expected.multiplicity != StructuralMultiplicity::Unrestricted
                || !expected.qualifications.is_empty()
                || !expected.projected_qualifications.is_empty()
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
                    "primitive-array argument requires whole plain owned internal-call custody",
                );
            }
        }
        // The canonical form retains the real array type at both ordinary and
        // boundary calls; the shared extent check recognizes the exact loan.
        if machine.structural_parameters.iter().any(|actual| {
            terminal_semantics::mutable_fixed_byte_array_extent(
                module.structural_types.iter(),
                actual,
                argument,
                expected,
            )
            .is_some()
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
        // The shared counterpart: a borrowed view reads the field's live bytes.
        // Boundary actuals admit it directly; ordinary borrowed calls admit the
        // same unrestricted subloan the verifier's shared-field rule describes.
        let shared_byte_field_loan = terminal_semantics::shared_boundary_buffer_capacity(
            module.structural_types.iter(),
            actual_type,
            argument,
            expected,
        )
        .is_some()
            && match presentation {
                StructuralArgumentPresentation::Boundary => true,
                StructuralArgumentPresentation::Ordinary => {
                    expected.multiplicity == StructuralMultiplicity::Unrestricted
                        && is_bounded_structural_scalar_store_path(&argument.path)
                        && machine.structural_parameters.iter().any(|parameter| {
                            parameter.place == argument.place
                                && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                        })
                }
            };
        if shared_byte_field_loan {
            continue;
        }
        let actual_type = validate_structural_path(module, actual_type, &argument.path)?;
        if actual_type != expected.structural_type {
            return malformed("structural argument has the wrong concrete type");
        }
    }
    Ok(())
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
                    StructuralPlaceKind::BlockParameter { block, position } => machine
                        .blocks
                        .iter()
                        .find(|candidate| candidate.id == block)?
                        .structural_parameters
                        .get(position as usize)
                        .filter(|parameter| parameter.place == place)
                        .map(|parameter| parameter.structural_type),
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

/// Result cleanup may reuse an existing owned operation result place, but a
/// merely matching type declaration does not establish that custody.
fn is_plain_affine_call_result(
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
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == producer);
    let Some(operation) = operations.next() else {
        return false;
    };
    // An atomic record establishment is the same claim-free whole owner: a
    // selected projected child leaves its sibling residual to die on the edge.
    if operations.next().is_some()
        || !matches!(
            operation.kind,
            OperationKind::CallStructuralWithScalarArguments { .. }
                | OperationKind::BoundaryCall { .. }
                | OperationKind::EstablishRecord { .. }
        )
    {
        return false;
    }
    matches!(&operation.result, OperationResult::Structural(result)
        if result.place == place
            && result.structural_type == structural_type
            && result.multiplicity == StructuralMultiplicity::Affine
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty())
}

fn validate_claim_indices(
    machine: &TerminalMachine,
    arguments: &[StructuralArgument],
    claims: impl Iterator<Item = (ClaimId, u32)>,
) -> Result<(), CodecError> {
    for (claim, argument_index) in claims {
        let Some(argument) = arguments.get(argument_index as usize) else {
            return malformed("claim action has an unknown structural argument index");
        };
        let Some(entry_claim) = machine
            .entry_claims
            .iter()
            .find(|entry_claim| entry_claim.claim == claim)
        else {
            return malformed("claim action references an unknown entry claim");
        };
        // A returned claim keeps its identity but moves to the successful
        // operation's result place. The codec checks that exact occurrence;
        // the verifier, not this structural check, establishes its liveness.
        let path = if entry_claim.input == argument.place {
            Some(entry_claim.path.as_slice())
        } else {
            machine.structural_places.iter().find_map(|declaration| {
                let StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } = declaration.kind
                else {
                    return None;
                };
                if declaration.id != argument.place {
                    return None;
                }
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find_map(|operation| {
                        if operation.id != producer
                            || !matches!(
                                operation.kind,
                                OperationKind::CallStructural { .. }
                                    | OperationKind::CallStructuralWithScalarArguments { .. }
                                    | OperationKind::BoundaryCall { .. }
                            )
                        {
                            return None;
                        }
                        let result = operation.result.structural()?;
                        if result.place != argument.place
                            || result.structural_type != structural_type
                            || result.multiplicity != StructuralMultiplicity::Linear
                        {
                            return None;
                        }
                        result
                            .claims
                            .iter()
                            .find(|binding| binding.claim == claim)
                            .map(|binding| binding.path.as_slice())
                    })
            })
        };
        if !path.is_some_and(|path| argument.path.is_empty() || path == argument.path) {
            return malformed("claim action does not match its structural argument path");
        }
    }
    Ok(())
}

fn has_structural_type(module: &TerminalModule, id: StructuralTypeId) -> bool {
    module
        .structural_types
        .iter()
        .any(|declaration| declaration.id == id)
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
            .structural_types
            .iter()
            .find(|declaration| declaration.id == id)
            .expect("structural field targets were validated before graph traversal");
        match &declaration.shape {
            // A reference does not inline its referent storage. The target's
            // existence was checked above; following it here invents a by-value cycle.
            StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_) => {}
            StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let StructuralFieldType::Structural(target) = &field.field_type {
                        visit(module, *target, active, complete)?;
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                visit(module, *element, active, complete)?;
            }
            StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let StructuralFieldType::Structural(target) = &field.field_type {
                        visit(module, *target, active, complete)?;
                    }
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    if let StructuralFieldType::Structural(target) = &field.field_type {
                        visit(module, *target, active, complete)?;
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
            .services
            .iter()
            .find(|declaration| declaration.id == id)
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
                .services
                .iter()
                .find(|candidate| candidate.id == *parent)
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

fn require_unique_nonempty_identities<'a>(
    identities: impl Iterator<Item = &'a str>,
    label: &'static str,
) -> Result<(), CodecError> {
    let mut seen = BTreeSet::new();
    for identity in identities {
        if identity.is_empty() || !seen.insert(identity) {
            return Err(CodecError::MalformedStructuralFoundation(label));
        }
    }
    Ok(())
}

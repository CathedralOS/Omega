use super::super::error::OptimizedProgramStorageSemanticWrapperObjectError;
use crate::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError,
    StagedOptimizedProgramStorageSemanticWrapperEncoding, ValidatedNativeProgramEntrySettlement,
    validate_native_program_entry_settlement,
};
use object_file::StagedValidatedOptimizedObjectArtifact;
use program_entry_plan::{
    OptimizedProgramStorageSemanticCallingApplication,
    OptimizedProgramStorageSemanticEntryContract, OptimizedProgramStorageSemanticReceiverLayout,
    ProgramEntrySourceReceiverSignature, bind_optimized_program_storage_semantic_entry_contract,
    plan_optimized_program_storage_semantic_wrapper,
};
use semantic_vocabulary::{IntegerSign, ScalarType, StructuralPlaceKind};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, StructuralTypeShape, TerminalMachineResult,
};

pub(crate) fn replay_settlement(
    settlement: &ValidatedNativeProgramEntrySettlement,
    source: &StagedValidatedOptimizedObjectArtifact,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    let calling_plans = match (
        settlement.semantic_calling_application(),
        settlement.physical_calling_application(),
        settlement.storage_entry(),
    ) {
        (Some(semantic), Some(physical), Some(storage)) => Some((semantic, physical, storage)),
        (None, None, None) => None,
        _ => {
            return Err(
                OptimizedProgramStorageSemanticWrapperObjectError::MissingPairedCallingPlans,
            );
        }
    };
    let replayed = validate_native_program_entry_settlement(
        source.terminal(),
        settlement.checked_entry(),
        NativeProgramEntrySettlement::new(
            settlement.source(),
            calling_plans,
            settlement.fused_service_establishments(),
        ),
        settlement.target(),
    )
    .map_err(OptimizedProgramStorageSemanticWrapperObjectError::Settlement)?;
    if &replayed != settlement {
        return Err(
            OptimizedProgramStorageSemanticWrapperObjectError::Settlement(
                NativeProgramEntrySettlementError::CallingPlanPairingDrift,
            ),
        );
    }
    Ok(())
}

/// Re-bind the semantic entry contract from the settlement's retained calling
/// application. The schema commits to the application identity (requirement,
/// target, source shape graph, ABI plan), never to the raw ABI plan, so the
/// bind replays the retained application's validated plan and re-checks the
/// recorded fingerprint, commitment, and boundary entry plan before the
/// contract is derived.
pub(crate) fn bind_semantic_contract(
    settlement: &ValidatedNativeProgramEntrySettlement,
) -> Result<
    OptimizedProgramStorageSemanticEntryContract,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    let semantic = settlement
        .semantic_calling_application()
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::MissingPairedCallingPlans)?;
    let storage = settlement
        .storage_entry()
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::MissingPairedCallingPlans)?;
    let (validated_plan, report_fingerprint, commitment) = semantic
        .replayed_validated_application()
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::SemanticContract)?;
    if semantic.report_fingerprint != report_fingerprint
        || semantic.commitment != commitment
        || semantic.exact_boundary_entry_plan() != validated_plan.plan()
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::SemanticContract);
    }
    let application = OptimizedProgramStorageSemanticCallingApplication::new(
        &validated_plan,
        report_fingerprint,
        commitment,
    );
    bind_optimized_program_storage_semantic_entry_contract(
        settlement.target(),
        storage,
        settlement.source(),
        &application,
    )
    .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::SemanticContract)
}

pub(crate) fn replay_semantic_contract(
    settlement: &ValidatedNativeProgramEntrySettlement,
    encoding: &StagedOptimizedProgramStorageSemanticWrapperEncoding,
    source: &StagedValidatedOptimizedObjectArtifact,
) -> Result<
    OptimizedProgramStorageSemanticEntryContract,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    let contract = bind_semantic_contract(settlement)?;
    let expected = plan_optimized_program_storage_semantic_wrapper(
        contract.clone(),
        receiver_layout(source, settlement, &contract)?,
    )
    .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::SemanticContract)?;
    if &expected != encoding.source() {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::SemanticWrapperPlanMismatch);
    }
    Ok(contract)
}

/// The wrapper provisions `self` itself, so its checked referent layout is a
/// fact of the emitted child — the selected plan's structural contract — not
/// of the boundary inputs. The native `self` parameter carries a borrowed
/// reference whose `ValueShape` retains the referent's byte extent and
/// alignment. Every field of the attached record must be zero-valid so the
/// zero-fill inside the wrapper's own frame establishes the receiver: erased
/// provider-backed fields cannot be installed by zero-fill and reject here.
pub(crate) fn receiver_layout(
    source: &StagedValidatedOptimizedObjectArtifact,
    settlement: &ValidatedNativeProgramEntrySettlement,
    contract: &OptimizedProgramStorageSemanticEntryContract,
) -> Result<
    Option<OptimizedProgramStorageSemanticReceiverLayout>,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    if !matches!(
        contract.source_signature().receiver(),
        ProgramEntrySourceReceiverSignature::ProvisionedMutable { .. }
    ) {
        return Ok(None);
    }
    let entry_machine = settlement.checked_entry().terminal_entry();
    let function = source
        .selected_plan()
        .functions
        .iter()
        .find(|function| function.machine == entry_machine)
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch)?;
    let structural = function
        .structural
        .as_ref()
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch)?;
    let Some(receiver) = structural
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.is_self)
    else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    let shape = receiver.target.shape;
    if shape.class != calling_conventions::ValueClass::BorrowedReference
        || !shape.alignment.is_power_of_two()
        || shape.byte_size == 0
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    let mut declarations = structural
        .structural_types
        .as_slice()
        .iter()
        .filter(|declaration| declaration.id == receiver.semantic.structural_type);
    let Some(declaration) = declarations.next() else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    if declarations.next().is_some() {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    let mut erased = 0;
    if !validate_receiver_fields(
        structural.structural_types.as_slice(),
        fields,
        settlement.fused_service_establishments(),
        &mut Vec::new(),
        &mut vec![receiver.semantic.structural_type],
        &mut erased,
    ) || erased != settlement.fused_service_establishments().len()
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    Ok(Some(OptimizedProgramStorageSemanticReceiverLayout::new(
        u32::from(shape.byte_size),
        u32::from(shape.alignment),
    )))
}

/// Mirrors the hosted-receiver bridge's discipline exactly: every erased
/// field in the receiver's record-field tree must rejoin one Fused
/// establishment row by its complete field route — its erased bytes are
/// installed by the selected provider, not by zero-fill — and every other
/// leaf stays zero-valid. `Structural` record children extend the route;
/// array elements and sum cases cannot name one and keep their nested-erased
/// rejection.
fn validate_receiver_fields(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    fields: &[terminal_psi::StructuralFieldDeclaration],
    rows: &[program_entry_plan::ProgramEntryFusedServiceEstablishment],
    field_path: &mut Vec<String>,
    visiting: &mut Vec<semantic_vocabulary::StructuralTypeId>,
    erased: &mut usize,
) -> bool {
    fields.iter().all(|field| {
        if field.relevance.is_erased()
            || matches!(field.field_type, StructuralFieldType::Erased { .. })
        {
            let StructuralFieldType::Erased { type_identity } = &field.field_type else {
                return false;
            };
            field_path.push(field.identity.clone());
            let joined = rows
                .iter()
                .filter(|row| {
                    row.field_path() == field_path.as_slice()
                        && row.carrier_type_identity() == type_identity.as_str()
                })
                .count()
                == 1;
            field_path.pop();
            if joined {
                *erased += 1;
            }
            joined
        } else {
            match field.field_type {
                StructuralFieldType::Scalar(
                    ScalarType::Boolean | ScalarType::Integer(_) | ScalarType::IeeeFloat(_),
                )
                | StructuralFieldType::IeeeFloat(_) => true,
                StructuralFieldType::BoundedInteger(integer) => {
                    integer.contains(semantic_vocabulary::IntegerValue::Signed(0))
                        || integer.contains(semantic_vocabulary::IntegerValue::Unsigned(0))
                }
                StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. }) => true,
                StructuralFieldType::Structural(child) => {
                    if visiting.contains(&child) {
                        return false;
                    }
                    let nested = declarations
                        .iter()
                        .filter(|declaration| declaration.id == child)
                        .collect::<Vec<_>>();
                    let [nested_declaration] = nested.as_slice() else {
                        return false;
                    };
                    match &nested_declaration.shape {
                        StructuralTypeShape::Record {
                            fields: nested_fields,
                        }
                        | StructuralTypeShape::Mixed {
                            fields: nested_fields,
                            ..
                        } => {
                            visiting.push(child);
                            field_path.push(field.identity.clone());
                            let valid = validate_receiver_fields(
                                declarations,
                                nested_fields,
                                rows,
                                field_path,
                                visiting,
                                erased,
                            );
                            field_path.pop();
                            visiting.pop();
                            valid
                        }
                        _ => zero_valid_record_storage(declarations, child, visiting),
                    }
                }
                _ => false,
            }
        }
    })
}

/// Whether zero-filled storage is an established value of one nested
/// declaration. Mirrors the hosted-receiver bridge's discipline exactly:
/// scalar leaves, bounded integers containing zero, owned byte carriers,
/// records, fixed arrays, and the first declared case of a closed sum are
/// established; references, erased fields, unknown or duplicated declarations,
/// and cycles reject.
fn zero_valid_record_storage(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    structural_type: semantic_vocabulary::StructuralTypeId,
    visiting: &mut Vec<semantic_vocabulary::StructuralTypeId>,
) -> bool {
    if visiting.contains(&structural_type) {
        return false;
    }
    let mut matches = declarations
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let Some(declaration) = matches.next() else {
        return false;
    };
    if matches.next().is_some() {
        return false;
    }
    visiting.push(structural_type);
    let valid = match &declaration.shape {
        StructuralTypeShape::Record { fields } => fields
            .iter()
            .all(|field| zero_valid_field(declarations, field, visiting)),
        StructuralTypeShape::FixedArray { element, .. } => {
            zero_valid_record_storage(declarations, *element, visiting)
        }
        StructuralTypeShape::Sum { cases } => cases.first().is_some_and(|case| {
            case.fields
                .iter()
                .all(|field| zero_valid_field(declarations, field, visiting))
        }),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields
                .iter()
                .all(|field| zero_valid_field(declarations, field, visiting))
                && cases.first().is_some_and(|case| {
                    case.fields
                        .iter()
                        .all(|field| zero_valid_field(declarations, field, visiting))
                })
        }
        StructuralTypeShape::PrimitiveScalar(
            ScalarType::Boolean | ScalarType::Integer(_) | ScalarType::IeeeFloat(_),
        )
        | StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. }) => true,
        _ => false,
    };
    visiting.pop();
    valid
}

fn zero_valid_field(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    field: &terminal_psi::StructuralFieldDeclaration,
    visiting: &mut Vec<semantic_vocabulary::StructuralTypeId>,
) -> bool {
    !field.relevance.is_erased()
        && match field.field_type {
            StructuralFieldType::Scalar(
                ScalarType::Boolean | ScalarType::Integer(_) | ScalarType::IeeeFloat(_),
            )
            | StructuralFieldType::IeeeFloat(_) => true,
            StructuralFieldType::BoundedInteger(integer) => {
                integer.contains(semantic_vocabulary::IntegerValue::Signed(0))
                    || integer.contains(semantic_vocabulary::IntegerValue::Unsigned(0))
            }
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. }) => true,
            StructuralFieldType::Structural(child) => {
                zero_valid_record_storage(declarations, child, visiting)
            }
            _ => false,
        }
}

pub(crate) fn validate_entry_shape(
    source: &StagedValidatedOptimizedObjectArtifact,
    settlement: &ValidatedNativeProgramEntrySettlement,
    contract: &OptimizedProgramStorageSemanticEntryContract,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    let module =
        terminal_codec::decode_module(source.terminal().semantic_bytes()).map_err(|_| {
            OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch
        })?;
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == settlement.checked_entry().terminal_entry())
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch)?;
    // The selected source shape is the authority: a provisioned-mutable
    // receiver adds one `self` parameter at position 0 ahead of the two
    // visible Extent roots; a free source carries the roots alone.
    let (receiver, image, storage) = match entry.structural_parameters.as_slice() {
        [image, storage] => (None, image, storage),
        [receiver, image, storage] => (Some(receiver), image, storage),
        _ => {
            return Err(
                OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch,
            );
        }
    };
    let receiver_shift = u32::from(receiver.is_some());
    if receiver.is_some()
        != matches!(
            contract.source_signature().receiver(),
            ProgramEntrySourceReceiverSignature::ProvisionedMutable { .. }
        )
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    if let Some(receiver) = receiver
        && (!receiver.is_self
            || receiver.position != 0
            || receiver.multiplicity != StructuralMultiplicity::Unrestricted
            || receiver.access != StructuralAccess::MutableBorrow
            || !receiver.qualifications.is_empty()
            || !receiver.projected_qualifications.is_empty()
            || entry.attachment != Some(receiver.structural_type)
            || !matches!(entry.structural_places.as_slice(), [receiver_place, ..]
                if receiver_place.id == receiver.place
                    && receiver_place.kind == StructuralPlaceKind::Parameter { position: 0, is_self: true }))
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    let [image_root, storage_root] = contract.roots();
    if !entry.parameters.is_empty()
        || entry.result != TerminalMachineResult::Unit
        || image.position != receiver_shift
        || storage.position != receiver_shift + 1
        || image.is_self
        || storage.is_self
        || image.place == storage.place
        || image.structural_type != storage.structural_type
        || image.multiplicity != StructuralMultiplicity::Linear
        || storage.multiplicity != StructuralMultiplicity::Linear
        || image.access != StructuralAccess::Owned
        || storage.access != StructuralAccess::Owned
        || image_root.parameter_index() != 0
        || storage_root.parameter_index() != 1
        || image_root.carrier_identity() != "named(name(Extent))"
        || storage_root.carrier_identity() != "named(name(Extent))"
        || image_root.domain() != "Extent::Granted"
        || storage_root.domain() != "Extent::Granted"
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    let ([image_domain], [storage_domain]) = (
        image.qualifications.as_slice(),
        storage.qualifications.as_slice(),
    ) else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    if image_domain != storage_domain {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    let Some(domain) = module
        .structural_domains
        .iter()
        .find(|row| row.id == *image_domain)
    else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    let Some(carrier) = module
        .structural_types
        .iter()
        .find(|row| row.id == image.structural_type)
    else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    let StructuralTypeShape::Record { fields } = &carrier.shape else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    if domain.identity != image_root.domain()
        || domain.carrier != carrier.id
        || carrier.identity != image_root.carrier_identity()
        || !matches!(fields.as_slice(), [base, length]
            if base.identity == "base"
                && base.relevance == BindingRelevance::Relevant
                && matches!(base.field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.is_address())
                && length.identity == "length"
                && length.relevance == BindingRelevance::Relevant
                && matches!(length.field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64))
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    let visible_places = &entry.structural_places[receiver_shift as usize..];
    if !matches!(visible_places, [image_place, storage_place]
    if image_place.id == image.place
        && image_place.kind == StructuralPlaceKind::Parameter {
            position: image.position, is_self: false,
        }
        && storage_place.id == storage.place
        && storage_place.kind == StructuralPlaceKind::Parameter {
            position: storage.position, is_self: false,
        })
        || !matches!(entry.entry_claims.as_slice(), [image_claim, storage_claim]
            if image_claim.input == image.place
                && image_claim.path.is_empty()
                && storage_claim.input == storage.place
                && storage_claim.path.is_empty())
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    Ok(())
}

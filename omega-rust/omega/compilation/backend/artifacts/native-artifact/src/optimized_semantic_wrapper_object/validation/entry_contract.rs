use super::super::error::OptimizedProgramStorageSemanticWrapperObjectError;
use object_file::StagedValidatedOptimizedObjectArtifact;
use program_entry_plan::{
    OptimizedProgramStorageSemanticCallingApplication,
    OptimizedProgramStorageSemanticEntryContract, OptimizedProgramStorageSemanticReceiverLayout,
    ProgramEntryReceiverSignature, bind_optimized_program_storage_semantic_entry_contract,
};
use semantic_vocabulary::{IntegerSign, ScalarType, StructuralPlaceKind};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, TerminalMachineResult,
};
use terminal_psi_to_abstract_operations::ValidatedNativeProgramEntrySettlement;

pub(crate) use terminal_semantics::zero_valid_record_storage;

/// Derive the semantic entry contract from the settlement's admitted plan and
/// exact calling application. Settlement owns their identity join; the wrapper
/// consumes that result without running calling-plan admission again.
pub(crate) fn bind_semantic_contract(
    settlement: &ValidatedNativeProgramEntrySettlement,
    identities: &terminal_psi::TerminalIdentityTable,
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
    let validated_plan = settlement
        .semantic_calling_plan()
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::MissingPairedCallingPlans)?;
    let application = OptimizedProgramStorageSemanticCallingApplication::new(
        validated_plan,
        settlement
            .physical_calling_plan()
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::MissingPairedCallingPlans)?,
        semantic.report_fingerprint,
        effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(semantic.commitment),
    );
    bind_optimized_program_storage_semantic_entry_contract(
        settlement.target(),
        storage,
        settlement.source(),
        &application,
        identities,
    )
    .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::SemanticContract)
}

/// The wrapper provisions the entry context, so its checked referent layout is a
/// fact of the emitted child — the selected plan's structural contract — not
/// of the boundary inputs. The native context parameter carries a borrowed
/// reference whose `ValueShape` retains the referent's byte extent and
/// alignment. Every field of the attached record must be zero-valid so the
/// zero-fill inside the wrapper's own frame establishes the receiver: erased
/// provider-backed fields cannot be installed by zero-fill and reject here.
pub fn receiver_layout(
    source: &StagedValidatedOptimizedObjectArtifact,
    settlement: &ValidatedNativeProgramEntrySettlement,
    contract: &OptimizedProgramStorageSemanticEntryContract,
) -> Result<
    Option<OptimizedProgramStorageSemanticReceiverLayout>,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    if !matches!(
        contract.source_signature().receiver(),
        ProgramEntryReceiverSignature::ProvisionedMutable { .. }
    ) {
        return Ok(None);
    }
    let entry_machine = settlement.checked_entry().terminal_entry();
    let function = source
        .selected_plan()
        .function(entry_machine)
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch)?;
    let structural = function
        .structural
        .as_ref()
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch)?;
    let eligibility = settlement
        .checked_entry()
        .receiver_eligibility()
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch)?;
    let terminal_psi::CheckedProgramEntryReceiverProjection::Retained {
        terminal_self: context_place,
        source_position,
    } = eligibility.projection()
    else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    let Some(receiver) = structural.parameters.iter().find(|parameter| {
        parameter.semantic.place == context_place && parameter.semantic.position == source_position
    }) else {
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
    let Some(fields) = declaration.shape.record_fields() else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    let mut erased = 0;
    if !validate_receiver_fields(
        &source.terminal().semantic_module().identities,
        contract.source_signature().identities(),
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
        shape.byte_size,
        u32::from(shape.alignment),
    )))
}

/// Mirrors the hosted-receiver bridge's discipline exactly: every service or erased
/// field in the receiver's record-field tree must rejoin one Fused
/// establishment row by its complete field route — its erased bytes are
/// installed by the selected provider, not by zero-fill — and every other
/// leaf stays zero-valid. `Structural` record children extend the route;
/// array elements and sum cases cannot name one and keep their nested-erased
/// rejection.
fn validate_receiver_fields(
    identities: &terminal_psi::TerminalIdentityTable,
    plan_identities: &terminal_psi::TerminalIdentityTable,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    fields: &[terminal_psi::StructuralFieldDeclaration],
    rows: &[program_entry_plan::ProgramEntryFusedServiceEstablishment],
    field_path: &mut Vec<String>,
    visiting: &mut Vec<semantic_vocabulary::StructuralTypeId>,
    erased: &mut usize,
) -> bool {
    fields.iter().all(|field| {
        let service_carrier = terminal_semantics::fused_service_field_carrier(declarations, field);
        if field.relevance.is_erased()
            || matches!(field.field_type, StructuralFieldType::Erased { .. })
            || service_carrier.is_some()
        {
            // A retained binding is not ordinary zero-valid record storage.
            let Some(type_identity) = service_carrier else {
                return false;
            };
            field_path.push(
                identities
                    .text(field.identity)
                    .unwrap_or_default()
                    .to_string(),
            );
            // Establishment rows index the signature's plan-local table; the
            // walked field path resolves against the module's table.
            let joined = rows
                .iter()
                .filter(|row| {
                    row.field_path()
                        .iter()
                        .map(|segment| plan_identities.text(*segment).unwrap_or_default())
                        .eq(field_path.iter().map(String::as_str))
                        && plan_identities.text(row.carrier_type_identity())
                            == Some(identities.text(type_identity).unwrap_or_default())
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
                | StructuralFieldType::Atomic(_)
                | StructuralFieldType::IeeeFloat(_) => true,
                StructuralFieldType::BoundedInteger(_) | StructuralFieldType::RefinedScalar(_) => {
                    terminal_semantics::scalar_field_accepts_zero(&field.field_type)
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
                        terminal_psi::record_or_mixed_shape_pattern!(nested_fields) => {
                            visiting.push(child);
                            field_path.push(
                                identities
                                    .text(field.identity)
                                    .unwrap_or_default()
                                    .to_string(),
                            );
                            let valid = validate_receiver_fields(
                                identities,
                                plan_identities,
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
                        _ => zero_valid_record_storage(
                            declarations,
                            child,
                            visiting,
                            zero_valid_field,
                        ),
                    }
                }
                _ => false,
            }
        }
    })
}

#[cfg(test)]
mod service_tests {
    use super::{validate_receiver_fields, zero_valid_field, zero_valid_record_storage};
    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    #[test]
    fn retained_service_leaf_requires_an_establishment_before_zero_valid_recursion() {
        let service = StructuralTypeId::new(1).unwrap();
        let declaration = StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: service,
            identity: terminal_psi::TerminalIdentityIndex(1),
            shape: StructuralTypeShape::BindingCarrier,
        };
        let field = StructuralFieldDeclaration {
            storage: None,
            id: StructuralFieldId::new(1).unwrap(),
            identity: terminal_psi::TerminalIdentityIndex(2),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(service),
        };
        assert!(!zero_valid_record_storage(
            std::slice::from_ref(&declaration),
            service,
            &mut Vec::new(),
            zero_valid_field
        ));
        let mut used = 0;
        assert!(!validate_receiver_fields(
            &Default::default(),
            &Default::default(),
            &[declaration],
            &[field],
            &[],
            &mut Vec::new(),
            &mut Vec::new(),
            &mut used
        ));
        assert_eq!(used, 0);
    }
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
            | StructuralFieldType::Atomic(_)
            | StructuralFieldType::IeeeFloat(_) => true,
            StructuralFieldType::BoundedInteger(_) | StructuralFieldType::RefinedScalar(_) => {
                terminal_semantics::scalar_field_accepts_zero(&field.field_type)
            }
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. }) => true,
            StructuralFieldType::Structural(child) => {
                zero_valid_record_storage(declarations, child, visiting, zero_valid_field)
            }
            _ => false,
        }
}

pub(crate) fn validate_entry_shape(
    source: &StagedValidatedOptimizedObjectArtifact,
    settlement: &ValidatedNativeProgramEntrySettlement,
    contract: &OptimizedProgramStorageSemanticEntryContract,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    let module = source.terminal().semantic_module();
    let entry = module
        .machine(settlement.checked_entry().terminal_entry())
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch)?;
    // The selected source shape is the authority: a provisioned-mutable
    // context occupies its explicit parameter at position 0 ahead of the two
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
            ProgramEntryReceiverSignature::ProvisionedMutable { .. }
        )
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    if let Some(receiver) = receiver
        && (receiver.position != 0
            || receiver.multiplicity != StructuralMultiplicity::Unrestricted
            || receiver.access != StructuralAccess::MutableBorrow
            || !receiver.qualifications.is_empty()
            || !receiver.projected_qualifications.is_empty()
            || !settlement
                .checked_entry()
                .receiver_eligibility()
                .is_some_and(|eligibility| {
                    matches!(eligibility.projection(),
                    terminal_psi::CheckedProgramEntryReceiverProjection::Retained {
                        terminal_self,
                        source_position: 0,
                    } if terminal_self == receiver.place)
                })
            || !matches!(entry.structural_places.as_slice(), [receiver_place, ..]
                if receiver_place.id == receiver.place
                    && receiver_place.kind == StructuralPlaceKind::Parameter { position: 0 }))
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    let [image_root, storage_root] = contract.roots();
    if !entry.parameters.is_empty()
        || entry.result != TerminalMachineResult::Unit
        || image.position != receiver_shift
        || storage.position != receiver_shift + 1
        || image.place == storage.place
        || image.structural_type != storage.structural_type
        || image.multiplicity != StructuralMultiplicity::Linear
        || storage.multiplicity != StructuralMultiplicity::Linear
        || image.access != StructuralAccess::Owned
        || storage.access != StructuralAccess::Owned
        || image_root.parameter_index() != 0
        || storage_root.parameter_index() != 1
        || module.identities.text(image_root.carrier_identity()) != Some("named(name(Extent))")
        || module.identities.text(storage_root.carrier_identity()) != Some("named(name(Extent))")
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
    let Some(domain) = module.structural_domain(*image_domain) else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    let Some(carrier) = module.structural_type(image.structural_type) else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    let Some(fields) = carrier.shape.record_fields() else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    // The module names a domain row by its family on its exact carrier; the
    // bound contract names that family and carrier separately.
    if module.identities.text(domain.identity)
        != Some(
            terminal_psi::StructuralDomainDeclaration::carrier_qualified_identity(
                image_root.domain(),
                module
                    .identities
                    .text(image_root.carrier_identity())
                    .unwrap_or_default(),
            )
            .as_str(),
        )
        || domain.carrier != carrier.id
        || module.identities.text(carrier.identity)
            != module.identities.text(image_root.carrier_identity())
        || !matches!(fields, [base, length]
            if module.identities.text(base.identity) == Some("base")
                && base.relevance == BindingRelevance::Relevant
                && matches!(base.field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.is_address())
                && module.identities.text(length.identity) == Some("length")
                && length.relevance == BindingRelevance::Relevant
                && matches!(length.field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64))
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    }
    let visible_places = &entry.structural_places[receiver_shift as usize..];
    if !matches!(visible_places, [image_place, storage_place]
    if image_place.id == image.place
        && image_place.kind == StructuralPlaceKind::Parameter {
            position: image.position,
        }
        && storage_place.id == storage.place
        && storage_place.kind == StructuralPlaceKind::Parameter {
            position: storage.position,
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

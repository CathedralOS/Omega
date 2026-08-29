use super::super::error::OptimizedProgramStorageSemanticWrapperObjectError;
use super::super::shared::*;

pub(crate) fn replay_settlement(
    settlement: &ValidatedNativeProgramEntrySettlement,
    source: &StagedValidatedOptimizedObjectArtifact,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    let calling_plans = match (
        settlement.semantic_boundary_entry_plan(),
        settlement.storage_entry(),
    ) {
        (Some(semantic), Some(storage)) => Some((semantic, storage)),
        (None, None) => None,
        _ => {
            return Err(
                OptimizedProgramStorageSemanticWrapperObjectError::MissingPairedCallingPlans,
            );
        }
    };
    let replayed = validate_native_program_entry_settlement(
        source.terminal(),
        settlement.checked_entry(),
        NativeProgramEntrySettlement::new(settlement.source(), calling_plans),
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

pub(crate) fn replay_semantic_contract(
    settlement: &ValidatedNativeProgramEntrySettlement,
    encoding: &StagedOptimizedProgramStorageSemanticWrapperEncoding,
) -> Result<
    OptimizedProgramStorageSemanticEntryContract,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    let semantic = settlement
        .semantic_boundary_entry_plan()
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::MissingPairedCallingPlans)?;
    let storage = settlement
        .storage_entry()
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::MissingPairedCallingPlans)?;
    let contract = bind_optimized_program_storage_semantic_entry_contract(
        settlement.target(),
        storage,
        settlement.source(),
        semantic,
    )
    .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::SemanticContract)?;
    let expected = plan_optimized_program_storage_semantic_wrapper(contract.clone())
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::SemanticContract)?;
    if &expected != encoding.source() {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::SemanticWrapperPlanMismatch);
    }
    Ok(contract)
}

pub(crate) fn validate_entry_shape(
    source: &StagedValidatedOptimizedObjectArtifact,
    settlement: &ValidatedNativeProgramEntrySettlement,
    contract: &OptimizedProgramStorageSemanticEntryContract,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    let module =
        psi_terminal_codec::decode_module(source.terminal().semantic_bytes()).map_err(|_| {
            OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch
        })?;
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == settlement.checked_entry().terminal_entry())
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch)?;
    let [image, storage] = entry.structural_parameters.as_slice() else {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::TerminalEntryShapeMismatch);
    };
    let [image_root, storage_root] = contract.roots();
    // A statically attached namespace does not imply a runtime receiver. The
    // checked source signature and `is_self` flags below remain the authority
    // for the receiver-free ProgramStorage contract.
    if !entry.parameters.is_empty()
        || entry.result != TerminalMachineResult::Unit
        || image.position != 0
        || storage.position != 1
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
        || !matches!(entry.structural_places.as_slice(), [image_place, storage_place]
            if image_place.id == image.place
                && image_place.kind == StructuralPlaceKind::Parameter { position: 0, is_self: false }
                && storage_place.id == storage.place
                && storage_place.kind == StructuralPlaceKind::Parameter { position: 1, is_self: false })
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

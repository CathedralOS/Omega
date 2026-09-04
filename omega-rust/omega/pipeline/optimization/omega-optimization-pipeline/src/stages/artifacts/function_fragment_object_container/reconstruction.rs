//! Relocation-free object construction and replay.

use super::*;

pub(super) fn construct_object(
    source: &StagedOptimizedRelocationFreeTextSection,
) -> Result<RelocationFreeObjectPlan, RelocationFreeObjectContainerError> {
    let text = source.text_section();
    let text_manifest = source.manifest().record();
    let mut symbols = Vec::with_capacity(text.functions.len());
    let mut semantic_entry_symbol = None;
    for (index, function) in text.functions.iter().enumerate() {
        let symbol = ObjectLocalSymbolId::new(
            u64::try_from(index)
                .map_err(|_| RelocationFreeObjectContainerError::LengthOverflow)?
                .checked_add(1)
                .ok_or(RelocationFreeObjectContainerError::LengthOverflow)?,
        )
        .ok_or(RelocationFreeObjectContainerError::LengthOverflow)?;
        let role = if function.machine == text.semantic_entry {
            semantic_entry_symbol = Some(symbol);
            RelocationFreeObjectSymbolRole::SemanticEntryV1
        } else {
            RelocationFreeObjectSymbolRole::PrivateFunctionV1
        };
        symbols.push(RelocationFreeFunctionSymbol {
            symbol,
            source_function_index: function.source_function_index,
            machine: function.machine,
            name: canonical_private_machine_symbol_name(function.machine),
            section_offset: function.section_offset,
            byte_count: function.byte_count,
            linkage: RelocationFreeObjectSymbolLinkage::ObjectLocalV1,
            role,
        });
    }
    assemble_object(
        source,
        symbols,
        semantic_entry_symbol.ok_or(RelocationFreeObjectContainerError::MissingSemanticEntry)?,
        section_name(text.target, SectionKind::Text),
        text_manifest.selections,
    )
}

pub(super) fn replay_object(
    source: &StagedOptimizedRelocationFreeTextSection,
) -> Result<RelocationFreeObjectPlan, RelocationFreeObjectContainerError> {
    let text = source.text_section();
    let mut entry = None;
    let mut symbols = Vec::with_capacity(text.functions.len());
    for ordinal in 1..=text.functions.len() {
        let function = &text.functions[ordinal - 1];
        let symbol = ObjectLocalSymbolId::new(
            u64::try_from(ordinal)
                .map_err(|_| RelocationFreeObjectContainerError::LengthOverflow)?,
        )
        .ok_or(RelocationFreeObjectContainerError::LengthOverflow)?;
        let role = if function.machine.get() == text.semantic_entry.get() {
            entry = Some(symbol);
            RelocationFreeObjectSymbolRole::SemanticEntryV1
        } else {
            RelocationFreeObjectSymbolRole::PrivateFunctionV1
        };
        symbols.push(RelocationFreeFunctionSymbol {
            symbol,
            source_function_index: u64::try_from(ordinal - 1)
                .map_err(|_| RelocationFreeObjectContainerError::LengthOverflow)?,
            machine: function.machine,
            name: format!("__omega_terminal_machine_{}", function.machine.get()),
            section_offset: function.section_offset,
            byte_count: function.byte_count,
            linkage: RelocationFreeObjectSymbolLinkage::ObjectLocalV1,
            role,
        });
    }
    let text_name = match text.target.object_format {
        ObjectFormat::MachO => "__TEXT,__text".to_owned(),
        ObjectFormat::Elf | ObjectFormat::Coff => ".text".to_owned(),
    };
    assemble_object(
        source,
        symbols,
        entry.ok_or(RelocationFreeObjectContainerError::MissingSemanticEntry)?,
        text_name,
        source.manifest().record().selections,
    )
}

pub(super) fn assemble_object(
    source: &StagedOptimizedRelocationFreeTextSection,
    symbols: Vec<RelocationFreeFunctionSymbol>,
    semantic_entry_symbol: ObjectLocalSymbolId,
    text_name: String,
    selections: OptimizationSelectionIdentity,
) -> Result<RelocationFreeObjectPlan, RelocationFreeObjectContainerError> {
    let text = source.text_section();
    let mut object = RelocationFreeObjectPlan {
        identity: RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"pending"),
        source_text_section: text.identity,
        psi: text.psi,
        fuel_schedule: text.fuel_schedule,
        selected: text.selected,
        selections,
        target: text.target,
        text_section: RelocationFreeObjectTextSection {
            name: text_name,
            alignment: text.section_alignment,
            byte_count: text.byte_count,
            bytes: text.bytes.clone(),
        },
        symbol_policy: RelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
        symbols,
        semantic_entry: text.semantic_entry,
        semantic_entry_symbol,
        relocation_record_count: 0,
        relocation_requirements:
            RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
    };
    object.identity = object
        .recomputed_identity()
        .map_err(RelocationFreeObjectContainerError::InvalidObject)?;
    validate_relocation_free_object(&object)
        .map_err(RelocationFreeObjectContainerError::InvalidObject)?;
    Ok(object)
}

pub(super) fn construct_manifest(
    source: &StagedOptimizedRelocationFreeTextSection,
    object: &RelocationFreeObjectPlan,
    container: &RelocationFreeObjectContainer,
) -> Result<ValidatedFunctionFragmentObjectContainerManifest, RelocationFreeObjectContainerError> {
    let unavailable = FunctionFragmentObjectContainerUnavailableData::Unavailable;
    let symbol_count = u64::try_from(object.symbols.len())
        .map_err(|_| RelocationFreeObjectContainerError::LengthOverflow)?;
    let container_bytes = u64::try_from(container.bytes.len())
        .map_err(|_| RelocationFreeObjectContainerError::LengthOverflow)?;
    let mut record = FunctionFragmentObjectContainerManifest {
        identity: FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(b"pending"),
        stage: FunctionFragmentObjectContainerStage::ValidatedRelocationFreeObjectContainerV1,
        source_text_section_manifest: source.manifest().record().identity,
        text_section: source.text_section().identity,
        psi: object.psi,
        fuel_schedule: object.fuel_schedule,
        selections: object.selections,
        selected: object.selected,
        target: object.target,
        semantic_entry: object.semantic_entry,
        semantic_entry_symbol: object.semantic_entry_symbol,
        symbol_policy: object.symbol_policy,
        object: object.identity,
        object_container: container.identity,
        relocation_requirements: object.relocation_requirements,
        statistics: FunctionFragmentObjectContainerStatistics {
            sections: 1,
            function_symbols: symbol_count,
            object_local_symbols: symbol_count,
            external_symbols: 0,
            text_bytes: object.text_section.byte_count,
            container_bytes,
            relocation_records: object.relocation_record_count,
        },
        external_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok(ValidatedFunctionFragmentObjectContainerManifest { record })
}

pub(super) fn receipt(
    manifest: &ValidatedFunctionFragmentObjectContainerManifest,
    object: &RelocationFreeObjectPlan,
    container: &RelocationFreeObjectContainer,
) -> StagedRelocationFreeObjectContainerCustodyReceipt {
    StagedRelocationFreeObjectContainerCustodyReceipt {
        source_text_section_manifest: manifest.record.source_text_section_manifest,
        text_section: object.source_text_section,
        object: object.identity,
        object_container: container.identity,
        manifest: manifest.record.identity,
    }
}

//! Canonical object-local symbols and section assembly from current placed text.
use super::*;

pub fn construct_relocation_free_object_from_text(
    text: &machine_code::RelocationFreeTextSectionPlacement,
    selections: OptimizationSelectionIdentity,
) -> Result<RelocationFreeObjectPlan, RelocationFreeObjectFromTextError> {
    let mut symbols = Vec::with_capacity(text.functions.len());
    let mut semantic_entry_symbol = None;
    for (index, function) in text.functions.iter().enumerate() {
        let symbol = ObjectLocalSymbolId::new(
            u64::try_from(index)
                .map_err(|_| RelocationFreeObjectFromTextError::LengthOverflow)?
                .checked_add(1)
                .ok_or(RelocationFreeObjectFromTextError::LengthOverflow)?,
        )
        .ok_or(RelocationFreeObjectFromTextError::LengthOverflow)?;
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
        text,
        symbols,
        semantic_entry_symbol.ok_or(RelocationFreeObjectFromTextError::MissingSemanticEntry)?,
        section_name(text.target, SectionKind::Text),
        selections,
    )
}

fn assemble_object(
    text: &machine_code::RelocationFreeTextSectionPlacement,
    symbols: Vec<RelocationFreeFunctionSymbol>,
    semantic_entry_symbol: ObjectLocalSymbolId,
    text_name: String,
    selections: OptimizationSelectionIdentity,
) -> Result<RelocationFreeObjectPlan, RelocationFreeObjectFromTextError> {
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
        .map_err(RelocationFreeObjectFromTextError::InvalidObject)?;
    validate_relocation_free_object(&object)
        .map_err(RelocationFreeObjectFromTextError::InvalidObject)?;
    Ok(object)
}

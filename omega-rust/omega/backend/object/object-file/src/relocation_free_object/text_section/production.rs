//! Canonical object-local symbols and section assembly from current placed text.

use crate::{
    ObjectLocalSymbolId, RelocationFreeFunctionSymbol, RelocationFreeObjectError,
    RelocationFreeObjectFromTextError, RelocationFreeObjectPlan,
    RelocationFreeObjectRelocationRequirements, RelocationFreeObjectSymbolLinkage,
    RelocationFreeObjectSymbolPolicy, RelocationFreeObjectSymbolRole,
    RelocationFreeObjectTextSection, canonical_private_machine_symbol_name, object_target_policy,
    validate_relocation_free_object,
};
use optimization_core::{OptimizationSelectionIdentity, RelocationFreeObjectPlanIdentity};
pub fn construct_relocation_free_object_from_text(
    text: &machine_code::RelocationFreeTextSectionPlacement,
    selections: OptimizationSelectionIdentity,
) -> Result<RelocationFreeObjectPlan, RelocationFreeObjectFromTextError> {
    // The text section name comes from the declared target matrix; an
    // undeclared (architecture, object-format) pair fails closed here rather
    // than building a plan the validator would reject afterward.
    let policy = object_target_policy(text.target).ok_or(
        RelocationFreeObjectFromTextError::InvalidObject(
            RelocationFreeObjectError::NonCanonicalTarget,
        ),
    )?;
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
        policy.text_section_name.to_owned(),
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

#[cfg(test)]
mod tests {
    use super::construct_relocation_free_object_from_text;
    use crate::{
        PlacedFunctionFragment, RelocationFreeObjectError, RelocationFreeObjectFromTextError,
        RelocationFreeTextSectionPlacement, TextSectionPlacementPolicy,
        TextSectionRelocationRequirements,
    };
    use optimization_core::{
        FunctionFragmentEmissionIdentity, OptimizationSelectionIdentity,
        TerminalRelocationFreeTextSectionIdentity,
    };
    use selected_instructions::SelectedInstructionPlanIdentity;
    use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
    use target::{Architecture, NativeTarget, ObjectFormat};
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    fn text_with_target(target: NativeTarget) -> RelocationFreeTextSectionPlacement {
        let machine = MachineId::new(7).unwrap();
        RelocationFreeTextSectionPlacement {
            identity: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"text"),
            source_fragments: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"fragments"),
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([4; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            selected: SelectedInstructionPlanIdentity::from_bytes([5; 32]),
            target,
            semantic_entry: machine,
            semantic_entry_offset: 0,
            policy: TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
            section_alignment: crate::object_target_policy(target)
                .map_or(4, |policy| policy.text_section_alignment),
            byte_count: 4,
            bytes: vec![0x20, 0, 0, 0xb5],
            functions: vec![PlacedFunctionFragment {
                source_function_index: 0,
                machine,
                section_offset: 0,
                byte_count: 4,
                blocks: vec![],
            }],
            resolved_internal_machine_calls: vec![],
            relocation_requirements:
                TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
            unresolved_normalized_foreign_calls: vec![],
        }
    }

    #[test]
    fn construction_fails_closed_on_undeclared_target_pair() {
        // The declared matrix, not the final validation pass, owns this
        // rejection: an undeclared (architecture, object-format) pair must not
        // borrow a section spelling before the plan is assembled.
        for target in [
            NativeTarget {
                architecture: Architecture::Aarch64,
                object_format: ObjectFormat::Coff,
                pointer_size: 8,
                pointer_alignment: 8,
            },
            NativeTarget {
                architecture: Architecture::X86_64,
                object_format: ObjectFormat::MachO,
                pointer_size: 8,
                pointer_alignment: 8,
            },
        ] {
            assert_eq!(
                construct_relocation_free_object_from_text(
                    &text_with_target(target),
                    OptimizationSelectionIdentity::from_bytes([6; 32]),
                ),
                Err(RelocationFreeObjectFromTextError::InvalidObject(
                    RelocationFreeObjectError::NonCanonicalTarget
                ))
            );
        }
    }

    #[test]
    fn construction_names_the_text_section_from_the_declared_pair_row() {
        for (target, name) in [
            (NativeTarget::linux_arm64(), ".text"),
            (NativeTarget::linux_x64(), ".text"),
            (NativeTarget::windows_x64(), ".text"),
            (NativeTarget::macos_arm64(), "__TEXT,__text"),
        ] {
            let object = construct_relocation_free_object_from_text(
                &text_with_target(target),
                OptimizationSelectionIdentity::from_bytes([6; 32]),
            )
            .expect("declared pair constructs a canonical object");
            assert_eq!(object.text_section.name, name);
        }
    }
}

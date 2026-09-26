//! Canonical object-local symbols and section assembly from current placed text.

use std::collections::BTreeMap;

use crate::{
    ObjectLocalSymbolId, RelocationFreeFunctionSymbol, RelocationFreeObjectError,
    RelocationFreeObjectFromTextError, RelocationFreeObjectNormalizedImport,
    RelocationFreeObjectPlan, RelocationFreeObjectRelocationRequirements,
    RelocationFreeObjectSymbolLinkage, RelocationFreeObjectSymbolPolicy,
    RelocationFreeObjectSymbolRole, RelocationFreeObjectTextSection,
    RelocationFreeObjectUnresolvedForeignCall, canonical_normalized_foreign_import_symbol_name,
    canonical_private_machine_symbol_name, object_target_policy, validate_relocation_free_object,
};
use machine_code::TextSectionRelocationRequirements;
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
    // Object construction owns the import plan: the distinct roster
    // coordinates the unresolved calls name become declared import symbols in
    // canonical sorted order, continuing the function symbol id sequence, and
    // every source row is bound to its declared symbol. The raw foreign
    // locator never enters the plan.
    let mut normalized_imports = Vec::new();
    let mut import_symbols = BTreeMap::new();
    for (boundary, ordinal) in text
        .unresolved_normalized_foreign_calls
        .iter()
        .map(|call| (call.boundary, call.ordinal))
        .collect::<std::collections::BTreeSet<_>>()
    {
        let symbol = ObjectLocalSymbolId::new(
            u64::try_from(symbols.len())
                .map_err(|_| RelocationFreeObjectFromTextError::LengthOverflow)?
                .checked_add(
                    u64::try_from(normalized_imports.len())
                        .map_err(|_| RelocationFreeObjectFromTextError::LengthOverflow)?,
                )
                .and_then(|base| base.checked_add(1))
                .ok_or(RelocationFreeObjectFromTextError::LengthOverflow)?,
        )
        .ok_or(RelocationFreeObjectFromTextError::LengthOverflow)?;
        import_symbols.insert((boundary, ordinal), symbol);
        normalized_imports.push(RelocationFreeObjectNormalizedImport {
            symbol,
            boundary,
            ordinal,
            name: canonical_normalized_foreign_import_symbol_name(boundary, ordinal),
        });
    }
    let unresolved_normalized_foreign_calls = text
        .unresolved_normalized_foreign_calls
        .iter()
        .map(|resolution| RelocationFreeObjectUnresolvedForeignCall {
            symbol: import_symbols[&(resolution.boundary, resolution.ordinal)],
            resolution: *resolution,
        })
        .collect::<Vec<_>>();
    let relocation_requirements = match text.relocation_requirements {
        TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1 => {
            RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1
        }
        TextSectionRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1 => {
            RelocationFreeObjectRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1
        }
    };
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
        normalized_imports,
        relocation_record_count: u64::try_from(unresolved_normalized_foreign_calls.len())
            .map_err(|_| RelocationFreeObjectFromTextError::LengthOverflow)?,
        unresolved_normalized_foreign_calls,
        relocation_requirements,
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
        ObjectLocalSymbolId, PlacedFunctionFragment, RelocationFreeObjectError,
        RelocationFreeObjectFromTextError, RelocationFreeTextSectionPlacement,
        TextSectionPlacementPolicy, TextSectionRelocationRequirements,
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
    fn construction_declares_sorted_imports_and_binds_unresolved_fields() {
        let machine = MachineId::new(7).unwrap();
        let boundary = semantic_vocabulary::BoundaryMachineId::new(3).unwrap();
        let mut text = text_with_target(NativeTarget::linux_x64());
        text.byte_count = 10;
        text.bytes = vec![0xe8, 0, 0, 0, 0, 0xe8, 0, 0, 0, 0];
        text.functions[0].byte_count = 10;
        text.unresolved_normalized_foreign_calls = vec![
            machine_code::PlacedNormalizedForeignCallResolution {
                kind: machine_code::NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1,
                state: machine_code::NormalizedForeignCallResolutionState::UnresolvedImportFieldV1,
                caller: machine,
                block: selected_instructions::SelectedBlockId(0),
                instruction: selected_instructions::SelectedInstructionId(0),
                operation: semantic_vocabulary::OperationId::new(11).unwrap(),
                boundary,
                ordinal: 2,
                call_function_offset: 0,
                call_section_offset: 0,
                call_byte_count: 5,
                opcode_function_offset: 0,
                opcode_section_offset: 0,
                field_function_offset: 1,
                field_section_offset: 1,
                next_instruction_function_offset: 5,
                next_instruction_section_offset: 5,
                field_byte_width: 4,
                addend: 0,
            },
            machine_code::PlacedNormalizedForeignCallResolution {
                kind: machine_code::NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1,
                state: machine_code::NormalizedForeignCallResolutionState::UnresolvedImportFieldV1,
                caller: machine,
                block: selected_instructions::SelectedBlockId(0),
                instruction: selected_instructions::SelectedInstructionId(1),
                operation: semantic_vocabulary::OperationId::new(12).unwrap(),
                boundary,
                ordinal: 1,
                call_function_offset: 5,
                call_section_offset: 5,
                call_byte_count: 5,
                opcode_function_offset: 5,
                opcode_section_offset: 5,
                field_function_offset: 6,
                field_section_offset: 6,
                next_instruction_function_offset: 10,
                next_instruction_section_offset: 10,
                field_byte_width: 4,
                addend: 0,
            },
        ];
        text.relocation_requirements =
            TextSectionRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1;

        let object = construct_relocation_free_object_from_text(
            &text,
            OptimizationSelectionIdentity::from_bytes([6; 32]),
        )
        .expect("unresolved import fields construct an import plan");
        assert_eq!(
            object.relocation_requirements,
            crate::RelocationFreeObjectRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1
        );
        assert_eq!(object.relocation_record_count, 2);
        // The import table deduplicates coordinates into canonical sorted
        // order and continues the function symbol id sequence.
        assert_eq!(
            object
                .normalized_imports
                .iter()
                .map(|import| (import.symbol.get(), import.boundary, import.ordinal))
                .collect::<Vec<_>>(),
            vec![
                (ObjectLocalSymbolId::new(2).unwrap().get(), boundary, 1),
                (ObjectLocalSymbolId::new(3).unwrap().get(), boundary, 2),
            ]
        );
        assert_eq!(
            object
                .unresolved_normalized_foreign_calls
                .iter()
                .map(|field| (field.symbol.get(), field.resolution.ordinal))
                .collect::<Vec<_>>(),
            vec![(3, 2), (2, 1)]
        );
        assert_eq!(
            crate::validate_relocation_free_object_from_text(
                &text,
                OptimizationSelectionIdentity::from_bytes([6; 32]),
                &object,
            ),
            Ok(())
        );
        let container = crate::encode_relocation_free_object(&object).unwrap();
        assert_eq!(
            crate::decode_relocation_free_object(&container.bytes),
            Ok(object)
        );
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

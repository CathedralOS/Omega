//! Publish owned byte-input results from their selected occurrence and real frame home.
use super::{Error, attribution, fragment, host, selected, source};
use crate::ObjectBoundarySettlement;
use machine_code::{
    BoundaryExecutionRecord, BoundaryResultRecord, BoundarySettlementRecord,
    BoundaryStructuralResultRecord, SemanticCodeSite,
};
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use selected_instructions::{
    LocalStorageSlotId, SelectedBoundarySettlement, SelectedBoundarySettlementPayload,
    SelectedInstructionKind,
};
use semantic_vocabulary::MachineId;
use target::{Architecture, NativeTarget};
use target_operations::{BoundaryRealization, CompilerBuiltinExecution};

fn decoded_home(target: NativeTarget, bytes: &[u8]) -> Option<u32> {
    if !target_operations::HostedReadByteRealization::supports_target(target) {
        return None;
    }
    match target.architecture {
        Architecture::X86_64 => isa_x86_64::decode_x86_64_selected_hosted_read_byte(bytes),
        Architecture::Aarch64 => {
            isa_aarch64::decode_aarch64_selected_hosted_read_byte(target, bytes)
        }
    }
}

/// Check supported no-code actions only. The retained, independently replayed
/// graph owns each return's live frontier, action order, and edge fuel.
pub(in crate::function_fragments) fn cleanup_actions_match(
    function: &abstract_operations::AbstractFunction,
    selected: &selected_instructions::SelectedFunction,
    actions: &[terminal_psi::TerminalAffineCleanupAction],
) -> bool {
    let Some(contract) = &selected.structural else {
        return false;
    };
    if !matches!(
        function.result,
        abstract_operations::AbstractFunctionResult::Unit
    ) {
        return false;
    }
    let results = function
        .operations
        .iter()
        .filter_map(|operation| match operation {
            abstract_operations::AbstractOperation::BoundaryCall {
                psi_operation,
                boundary,
                result: abstract_operations::AbstractBoundaryResult::Structural(result),
                ..
            } => Some((*psi_operation, *boundary, result)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut discarded = std::collections::BTreeSet::new();
    !actions.is_empty()
        && actions.iter().all(|action| {
            let terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) = action else {
                return false;
            };
            if !discarded.insert(*place) { return false; }
            let mut matching = results.iter().filter(|(_, _, result)| result.place == *place);
            let Some(&(operation, boundary, result)) = matching.next() else { return false; };
            if matching.next().is_some() { return false; }
            if result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
                || !result.claims.is_empty()
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || *action != terminal_psi::TerminalAffineCleanupAction::DiscardRoot(result.place)
                || selected.boundary_settlements.iter().filter(|row| matches!(&row.settlement,
                    SelectedBoundarySettlementPayload::HostedReadByte {
                        operation: actual_operation, boundary: actual_boundary, result: actual_result, ..
                    } if *actual_operation == operation && *actual_boundary == boundary && actual_result == result
                )).count() != 1
                || contract.structural_places.iter().filter(|declaration| {
                    declaration.id == result.place && declaration.kind == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer: operation, structural_type: result.structural_type,
                    }
                }).count() != 1
            { return false; }
            let mut types = contract.structural_types.iter().filter(|declaration| declaration.id == result.structural_type);
            let Some(declaration) = types.next() else { return false; };
            if types.next().is_some() { return false; }
            crate::boundary_results::hosted_read_byte_declaration_is_valid(declaration)
        })
}

pub(super) fn settlement(
    container: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
    located: &SelectedBoundarySettlement,
) -> Result<ObjectBoundarySettlement, Error> {
    let invalid = || Error::Mismatch("selected byte input result custody");
    let SelectedBoundarySettlementPayload::HostedReadByte {
        operation,
        boundary,
        result,
        layout,
    } = &located.settlement
    else {
        return Err(invalid());
    };
    let function = selected(container, machine)?;
    let fragment = fragment(container, machine)?;
    let instruction = function
        .blocks
        .iter()
        .find(|block| block.id == located.block)
        .and_then(|block| block.instructions.get(located.instruction_index as usize))
        .ok_or_else(invalid)?;
    let slot = LocalStorageSlotId::Structural {
        operation: *operation,
        place: result.place,
    };
    if instruction.kind != (SelectedInstructionKind::HostedReadByte { slot })
        || !instruction.operands.is_empty()
        || instruction.provenance.operations != [*operation]
        || !instruction.provenance.values.is_empty()
    {
        return Err(invalid());
    }
    let span = fragment
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|span| span.instruction == instruction.id)
        .ok_or_else(invalid)?;
    let offset =
        decoded_home(container.source().text_section().target, &span.bytes).ok_or_else(invalid)?;
    let frame = source::frame(container, machine)?.ok_or_else(invalid)?;
    let home = frame
        .local_storage_slots
        .iter()
        .find(|candidate| candidate.id == slot)
        .ok_or_else(invalid)?;
    if home.frame_offset_bytes != u64::from(offset)
        || home.size_bytes != u32::from(layout.shape.byte_size)
        || home.alignment_bytes != layout.shape.alignment
        || home
            .frame_offset_bytes
            .checked_add(u64::from(home.size_bytes))
            .is_none_or(|end| end > frame.frame_size_bytes)
    {
        return Err(invalid());
    }
    let code_offset = host(span.offset)?;
    let (abstracted, _) = source::function(container, machine)?;
    let record = BoundarySettlementRecord {
        psi_operation: *operation,
        boundary: *boundary,
        execution: BoundaryExecutionRecord::CompilerBuiltin(
            CompilerBuiltinExecution::HostedReadByte,
        ),
        realization: BoundaryRealization::HostedReadByte(Default::default()),
        scalar_arguments: Vec::new(),
        runtime_scalar_arguments: Vec::new(),
        arguments: Vec::new(),
        byte_sequence_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
        completion_provider_custody: Vec::new(),
        native_result: BoundaryResultRecord::Structural(BoundaryStructuralResultRecord {
            defining_operation: *operation,
            result: result.clone(),
            declaration: result_declaration(function, result.structural_type)?.clone(),
            layout: layout.clone(),
            home_byte_offset: offset,
        }),
        operation_ordinal: attribution::ordinal(
            abstracted,
            SemanticCodeSite::Operation(*operation),
        )?,
        code_offset,
        byte_count: span.bytes.len(),
    };
    let placed = container
        .source()
        .text_section()
        .functions
        .iter()
        .find(|placed| placed.machine == machine)
        .ok_or_else(invalid)?;
    Ok(ObjectBoundarySettlement {
        machine,
        text_offset: host(placed.section_offset)?
            .checked_add(code_offset)
            .ok_or(Error::Overflow)?,
        settlement: record,
    })
}

pub(super) fn validate(
    container: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
    located: &SelectedBoundarySettlement,
    proposed: &ObjectBoundarySettlement,
) -> Result<(), Error> {
    let invalid = || Error::Mismatch("selected byte input publication custody");
    let SelectedBoundarySettlementPayload::HostedReadByte {
        operation,
        boundary,
        result,
        layout,
    } = &located.settlement
    else {
        return Err(invalid());
    };
    let function = selected(container, machine)?;
    let fragment = fragment(container, machine)?;
    let row = function
        .blocks
        .iter()
        .find(|block| block.id == located.block)
        .and_then(|block| block.instructions.get(located.instruction_index as usize))
        .ok_or_else(invalid)?;
    let slot = LocalStorageSlotId::Structural {
        operation: *operation,
        place: result.place,
    };
    if row.kind != (SelectedInstructionKind::HostedReadByte { slot })
        || !row.operands.is_empty()
        || row.provenance.operations != [*operation]
        || !row.provenance.values.is_empty()
    {
        return Err(invalid());
    }
    let span = fragment
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|span| span.instruction == row.id)
        .ok_or_else(invalid)?;
    let frame = source::frame(container, machine)?.ok_or_else(invalid)?;
    let home = frame
        .local_storage_slots
        .iter()
        .find(|candidate| candidate.id == slot)
        .ok_or_else(invalid)?;
    let placed = container
        .source()
        .text_section()
        .functions
        .iter()
        .find(|placed| placed.machine == machine)
        .ok_or_else(invalid)?;
    let record = &proposed.settlement;
    let BoundaryResultRecord::Structural(produced) = &record.native_result else {
        return Err(invalid());
    };
    let (abstracted, _) = source::function(container, machine)?;
    let code_offset = host(span.offset)?;
    if proposed.machine != machine
        || &produced.declaration != result_declaration(function, result.structural_type)?
        || proposed.text_offset
            != host(placed.section_offset)?
                .checked_add(code_offset)
                .ok_or(Error::Overflow)?
        || record.psi_operation != *operation
        || record.boundary != *boundary
        || record.operation_ordinal
            != attribution::ordinal(abstracted, SemanticCodeSite::Operation(*operation))?
        || record.execution
            != BoundaryExecutionRecord::CompilerBuiltin(CompilerBuiltinExecution::HostedReadByte)
        || record.realization != BoundaryRealization::HostedReadByte(Default::default())
        || produced.defining_operation != *operation
        || produced.result != *result
        || produced.layout != *layout
        || u64::from(produced.home_byte_offset) != home.frame_offset_bytes
        || home.size_bytes != u32::from(layout.shape.byte_size)
        || home.alignment_bytes != layout.shape.alignment
        || home
            .frame_offset_bytes
            .checked_add(u64::from(home.size_bytes))
            .is_none_or(|end| end > frame.frame_size_bytes)
        || record.code_offset != code_offset
        || record.byte_count != span.bytes.len()
        || decoded_home(container.source().text_section().target, &span.bytes)
            != Some(produced.home_byte_offset)
        || fragment.bytes.get(
            code_offset
                ..code_offset
                    .checked_add(record.byte_count)
                    .ok_or(Error::Overflow)?,
        ) != Some(span.bytes.as_slice())
        || !record.scalar_arguments.is_empty()
        || !record.runtime_scalar_arguments.is_empty()
        || !record.arguments.is_empty()
        || !record.byte_sequence_arguments.is_empty()
        || !record.completion_claim_sources.is_empty()
        || !record.completion_receipts.is_empty()
        || !record.completion_provider_custody.is_empty()
    {
        return Err(invalid());
    }
    Ok(())
}

fn result_declaration(
    function: &selected_instructions::SelectedFunction,
    structural_type: semantic_vocabulary::StructuralTypeId,
) -> Result<&terminal_psi::StructuralTypeDeclaration, Error> {
    let invalid = || Error::Mismatch("selected byte input declaration custody");
    let contract = function.structural.as_ref().ok_or_else(invalid)?;
    let mut matching = contract
        .structural_types
        .iter()
        .filter(|row| row.id == structural_type);
    let declaration = matching.next().ok_or_else(invalid)?;
    if matching.next().is_some()
        || !crate::boundary_results::hosted_read_byte_declaration_is_valid(declaration)
    {
        return Err(invalid());
    }
    Ok(declaration)
}

#[cfg(test)]
mod tests {
    use super::cleanup_actions_match;
    use abstract_operations::{
        AbstractBoundaryResult, AbstractFunction, AbstractFunctionResult, AbstractOperation,
        AbstractResult,
    };
    use selected_instructions::{
        SelectedBlockId, SelectedBoundarySettlement, SelectedBoundarySettlementPayload,
        SelectedFunction,
    };
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId,
        ScalarType, StructuralCaseId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
        ValueId,
    };
    use terminal_psi::{
        StructuralMultiplicity, StructuralOperationResult, StructuralTypeShape,
        TerminalAffineCleanupAction,
    };

    fn fixture() -> (
        AbstractFunction,
        SelectedFunction,
        Vec<TerminalAffineCleanupAction>,
    ) {
        let machine = MachineId::new(1).unwrap();
        let structural_type = StructuralTypeId::new(1).unwrap();
        let boundary = BoundaryMachineId::new(1).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        let mut function = AbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(1).unwrap(),
            parameters: vec![],
            structural_parameters: vec![],
            result: AbstractFunctionResult::Unit,
            entry_claims: vec![],
            published_service_ceiling: vec![],
            block_entries: vec![],
            operations: vec![],
        };
        let mut selected = SelectedFunction {
            machine,
            attachment: None,
            provenance: target_operations::TerminalPsiProvenance {
                operations: vec![],
                edges: vec![],
            },
            ranked: None,
            structural: Some(legalized_operations::LegalizedStructuralContract {
                structural_types: vec![terminal_psi::StructuralTypeDeclaration {
                    id: structural_type,
                    identity: "ByteRead".into(),
                    shape: StructuralTypeShape::Sum {
                        cases: vec![
                            terminal_psi::StructuralCaseDeclaration {
                                id: StructuralCaseId::new(1).unwrap(),
                                identity: "Eof".into(),
                                fields: vec![],
                            },
                            terminal_psi::StructuralCaseDeclaration {
                                id: StructuralCaseId::new(2).unwrap(),
                                identity: "Byte".into(),
                                fields: vec![terminal_psi::StructuralFieldDeclaration {
                                    id: StructuralFieldId::new(1).unwrap(),
                                    identity: "payload".into(),
                                    relevance: terminal_psi::BindingRelevance::Relevant,
                                    field_type: terminal_psi::StructuralFieldType::Scalar(
                                        scalar_type,
                                    ),
                                }],
                            },
                        ],
                    },
                }],
                parameters: vec![],
                structural_places: vec![],
                entry_claims: vec![],
                published_service_ceiling: vec![],
            }),
            local_storage_slots: vec![],
            outgoing_arguments: vec![],
            calls: vec![],
            memory_accesses: vec![],
            boundary_settlements: vec![],
            entry_block: SelectedBlockId(0),
            virtual_registers: vec![],
            blocks: vec![],
        };
        // The frontier's canonical schedule is reverse producer identity, not vector order.
        for number in [2, 1] {
            let operation = OperationId::new(number).unwrap();
            let place = PlaceId::new(number).unwrap();
            let result = StructuralOperationResult {
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: vec![],
                projected_qualifications: vec![],
                claims: vec![],
            };
            function.operations.push(AbstractOperation::BoundaryCall {
                psi_operation: operation,
                boundary,
                result: AbstractBoundaryResult::Structural(result.clone()),
                arguments: vec![],
                structural_arguments: vec![],
                completion_claim_sources: vec![],
                completion_receipts: vec![],
            });
            selected
                .structural
                .as_mut()
                .unwrap()
                .structural_places
                .push(terminal_psi::StructuralPlaceDeclaration {
                    id: place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer: operation,
                        structural_type,
                    },
                });
            selected
                .boundary_settlements
                .push(SelectedBoundarySettlement {
                    block: SelectedBlockId(0),
                    instruction_index: u32::try_from(number - 1).unwrap(),
                    settlement: SelectedBoundarySettlementPayload::HostedReadByte {
                        operation,
                        boundary,
                        result,
                        layout: calling_conventions::evaluate_conventional_sum_layout(
                            &[],
                            &[vec![], vec![calling_conventions::ValueShape::integer(4, 4)]],
                        )
                        .unwrap(),
                    },
                });
        }
        (
            function,
            selected,
            vec![
                TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(2).unwrap()),
                TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(1).unwrap()),
            ],
        )
    }

    #[test]
    fn read_result_cleanup_requires_exact_read_payload_type_function_and_actions() {
        let (source, selected, actions) = fixture();
        assert!(cleanup_actions_match(&source, &selected, &actions));
        // Subsets and their authored order are a graph obligation, not a
        // function-wide cleanup roster reconstructed by this capability check.
        assert!(cleanup_actions_match(&source, &selected, &actions[..1]));
        assert!(cleanup_actions_match(&source, &selected, &actions[1..]));
        let mut reordered = actions.clone();
        reordered.reverse();
        assert!(cleanup_actions_match(&source, &selected, &reordered));
        for mutation in 0..7 {
            let mut source = source.clone();
            let mut selected = selected.clone();
            let mut actions = actions.clone();
            match mutation {
                0 => {
                    selected.structural.as_mut().unwrap().structural_types[0].shape =
                        StructuralTypeShape::Record { fields: vec![] }
                }
                1 => {
                    selected.boundary_settlements.pop();
                }
                2 => {
                    source.result = AbstractFunctionResult::Scalar(AbstractResult {
                        value: ValueId::new(1).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                        ),
                    })
                }
                3 => actions.push(actions[0].clone()),
                4 => {
                    actions[0] =
                        TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(99).unwrap());
                }
                5 => {
                    selected.boundary_settlements[0].settlement =
                        SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                            operation: OperationId::new(2).unwrap(),
                            boundary: BoundaryMachineId::new(1).unwrap(),
                            source: ValueId::new(1).unwrap(),
                        }
                }
                _ => {
                    selected.structural.as_mut().unwrap().structural_places[0].kind =
                        StructuralPlaceKind::OperationResult {
                            producer: OperationId::new(3).unwrap(),
                            structural_type: StructuralTypeId::new(1).unwrap(),
                        }
                }
            }
            assert!(
                !cleanup_actions_match(&source, &selected, &actions),
                "mutation {mutation}"
            );
        }
    }
}

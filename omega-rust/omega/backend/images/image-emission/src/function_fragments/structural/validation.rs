//! Independent object-record comparison against admitted call and storage contracts.
use super::*;
pub(in crate::function_fragments) fn validate_function(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    function: &ObjectFunction,
    rows: &[SemanticCodeAttribution],
) -> Result<(), Error> {
    // No-code return cleanup is carried by the mandatory retained graph.
    // A legacy singular record would be a second, incomplete custody channel.
    if function.unit_affine_cleanup.is_some() {
        return Err(Error::Mismatch(
            "shared fragments cannot carry legacy singular cleanup",
        ));
    }
    let invalid = || Error::Mismatch("structural object differs from current ABI or call evidence");
    let selected = selected(source, function.machine)?;
    let fragment = fragment(source, function.machine)?;
    // Ranked referents are retained semantic ownership, not materialized homes.
    // Complete source admission rejects executable accesses to those referents.
    let parameters = if selected.ranked.is_some() {
        &[][..]
    } else {
        selected
            .structural
            .as_ref()
            .map_or(&[][..], |contract| contract.parameters.as_slice())
    };
    let (records, homes, unused_records, unused_homes) =
        if function.mixed_structural_scalar_abi.is_some() {
            (
                &function.scalar_structural_parameters,
                &function.scalar_structural_parameter_homes,
                &function.unit_parameters,
                &function.unit_parameter_homes,
            )
        } else {
            (
                &function.unit_parameters,
                &function.unit_parameter_homes,
                &function.scalar_structural_parameters,
                &function.scalar_structural_parameter_homes,
            )
        };
    if !unused_records.is_empty()
        || !unused_homes.is_empty()
        || records.len() != parameters.len()
        || homes.len() != parameters.len()
        || function.internal_unit_calls.len()
            != selected
                .calls
                .iter()
                .filter(|row| row.call.result_placement.is_none())
                .count()
    {
        return Err(invalid());
    }
    for ((parameter, home), expected) in records.iter().zip(homes).zip(parameters) {
        let expected = &expected.target;
        if parameter.place != expected.place
            || parameter.structural_type != expected.structural_type
            || parameter.multiplicity != expected.multiplicity
            || parameter.access != expected.access
            || parameter.shape != expected.shape
            || home.place != expected.place
            || home.structural_type != expected.structural_type
            || home.multiplicity != expected.multiplicity
            || home.access != expected.access
            || home.shape != expected.shape
            || home.source != expected.placement
            || !home.indirect
        {
            return Err(invalid());
        }
        validate_incoming_location(expected.access, &expected.placement, home.location)?;
    }
    let frame_bytes = u32::try_from(
        source::frame(source, function.machine)?.map_or(0, |frame| frame.frame_size_bytes),
    )
    .map_err(|_| Error::Overflow)?;
    for (actual, contract) in function.internal_unit_calls.iter().zip(
        selected
            .calls
            .iter()
            .filter(|row| row.call.result_placement.is_none()),
    ) {
        let expected = &contract.call;
        let span = rows
            .iter()
            .find(|row| row.site == SemanticCodeSite::Operation(contract.operation))
            .ok_or_else(invalid)?;
        let call_span = fragment
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.instruction == contract.instruction)
            .ok_or_else(invalid)?;
        if actual.owner != CallSiteOwner::Operation(contract.operation)
            || actual.target != expected.callee
            || actual.result.is_some()
            || actual.semantic_result.is_some()
            || actual.structural_result.is_some()
            || actual.scalar_arguments.len()
                != expected
                    .arguments
                    .iter()
                    .filter(|argument| matches!(argument, LegalizedScalarArgument::Scalar { .. }))
                    .count()
            || actual.claim_transfers != expected.claim_transfers
            || actual.arguments.len() + actual.scalar_arguments.len() != expected.arguments.len()
            || actual.operation_ordinal != span.operation_ordinal
            || actual.code_offset != host(call_span.offset)?
            || actual.byte_count != call_span.bytes.len()
            || call_span
                .internal_machine_fixup
                .as_ref()
                .is_none_or(|fixup| fixup.callee != expected.callee)
        {
            return Err(invalid());
        }
        match (&actual.source, &expected.source) {
            (InternalUnitCallSource::Authored, LegalizedCallUnitSource::AuthoredCallUnit) => {}
            (
                InternalUnitCallSource::InstalledProvider {
                    boundary,
                    provider,
                    completion_claim_sources,
                    completion_receipts,
                },
                LegalizedCallUnitSource::InstalledProvider {
                    boundary: wanted_boundary,
                    provider: wanted_provider,
                    completion_claim_sources: wanted_sources,
                    completion_receipts: wanted_receipts,
                },
            ) if boundary == wanted_boundary
                && provider.as_ref() == wanted_provider
                && completion_claim_sources == wanted_sources
                && completion_receipts == wanted_receipts => {}
            _ => return Err(invalid()),
        }
        for (actual_argument, (parameter_index, expected_argument)) in
            actual.scalar_arguments.iter().zip(
                expected
                    .arguments
                    .iter()
                    .enumerate()
                    .filter(|(_, argument)| {
                        matches!(argument, LegalizedScalarArgument::Scalar { .. })
                    }),
            )
        {
            let LegalizedScalarArgument::Scalar {
                source: value,
                placement,
            } = expected_argument
            else {
                return Err(invalid());
            };
            if usize::try_from(actual_argument.parameter_index) != Ok(parameter_index)
                || actual_argument.source
                    != (InternalUnitScalarArgumentSourceRecord::SelectedCall {
                        source_value: *value,
                        scalar_type: scalar_type(selected, *value)?,
                        instruction: contract.instruction,
                    })
                || actual_argument.destination != *placement
                || actual_argument.code_offset != actual.code_offset
                || actual_argument.byte_count != actual.byte_count
            {
                return Err(invalid());
            }
        }
        for (actual, expected) in actual.arguments.iter().zip(
            expected
                .arguments
                .iter()
                .filter(|argument| matches!(argument, LegalizedScalarArgument::Structural { .. })),
        ) {
            let LegalizedScalarArgument::Structural { target, .. } = expected else {
                return Err(invalid());
            };
            if target.access == terminal_psi::StructuralAccess::Owned {
                validate_copy(selected, fragment, contract.operation, target.place, actual)?;
            } else if actual.code_offset != host(call_span.offset)?
                || actual.byte_count != call_span.bytes.len()
                || actual.bytes != call_span.bytes
            {
                // Borrowed arguments identify the exact call, not a referent-copy
                // prefix. Retained physical replay owns the pointer preparation.
                return Err(invalid());
            }
            match (&target.source, &actual.source) {
                (
                    target_operations::TargetStructuralArgumentSource::Placement(placement),
                    InternalUnitStructuralArgumentSourceRecord::Placement(actual_placement),
                ) if placement == actual_placement => {
                    let mut roots = parameters
                        .iter()
                        .filter(|parameter| parameter.target.place == target.place);
                    let root = roots.next().ok_or_else(invalid)?;
                    if roots.next().is_some() {
                        return Err(invalid());
                    }
                    validate_incoming_location(
                        root.target.access,
                        &root.target.placement,
                        actual.source_location,
                    )?;
                }
                (
                    target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                        psi_operation,
                    },
                    InternalUnitStructuralArgumentSourceRecord::EstablishedByteView {
                        psi_operation: actual_operation,
                    },
                ) if psi_operation == actual_operation => {
                    if actual.source_location
                        != established_views::location(source, selected, target, *psi_operation)?
                    {
                        return Err(invalid());
                    }
                }
                (
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block,
                        place,
                    },
                    InternalUnitStructuralArgumentSourceRecord::BlockParameter {
                        block: actual_block,
                        place: actual_place,
                    },
                ) if block == actual_block && place == actual_place => {
                    if actual.source_location
                        != established_views::block_location(
                            source, selected, target, *block, *place,
                        )?
                    {
                        return Err(invalid());
                    }
                }
                _ => return Err(invalid()),
            }
            if actual.place != target.place
                || actual.access != target.access
                || actual.path != target.path
                || actual.root_structural_type != target.root_structural_type
                || actual.structural_type != target.structural_type
                || actual.shape != target.shape
                || actual.source_byte_offset != target.source_byte_offset
                || actual.call_stack_bytes != frame_bytes
                || actual.fixed_array_length != target.fixed_array_length
                || actual.element_stride != target.element_stride
                || actual.destination != target.destination
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// Consume the recorded incoming location against the exact declared placement.
/// Borrowed pointer bits never become an owned referent home, including on stack.
fn validate_incoming_location(
    access: terminal_psi::StructuralAccess,
    placement: &ValuePlacement,
    location: StructuralSourceLocation,
) -> Result<(), Error> {
    let invalid =
        || Error::Mismatch("structural incoming location differs from declared access or ABI");
    if access == terminal_psi::StructuralAccess::Owned {
        return if location
            == (StructuralSourceLocation::IncomingIndirectPointer {
                register: pointer(placement)?,
            }) {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    if placement.shape.class != calling_conventions::ValueClass::BorrowedReference {
        return Err(invalid());
    }
    let StructuralSourceLocation::IncomingBorrowedPointer { location } = location else {
        return Err(invalid());
    };
    match (placement.locations.as_slice(), location) {
        (
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ],
            IndirectPointerLocation::Register(actual),
        ) if *register == actual => Ok(()),
        (
            [
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                },
            ],
            IndirectPointerLocation::Stack {
                stack_byte_offset: actual,
                alignment: 8,
            },
        ) if *stack_byte_offset == actual => Ok(()),
        (
            [
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset: None,
                    byte_size,
                    alignment,
                },
            ],
            actual,
        ) if *pointer == actual
            && *byte_size == placement.shape.byte_size
            && *alignment == placement.shape.alignment
            && matches!(
                pointer,
                IndirectPointerLocation::Register(_)
                    | IndirectPointerLocation::Stack { alignment: 8, .. }
            ) =>
        {
            Ok(())
        }
        _ => Err(invalid()),
    }
}

#[cfg(test)]
mod tests;
pub(in crate::function_fragments) fn validate_settlements(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    rows: &[ObjectBoundarySettlement],
) -> Result<(), Error> {
    let mut cursor = 0;
    for placed in &source.source().text_section().functions {
        let function = selected(source, placed.machine)?;
        let fragment = fragment(source, placed.machine)?;
        let (abstracted, _) = source::function(source, placed.machine)?;
        for located in &function.boundary_settlements {
            let actual = rows
                .get(cursor)
                .ok_or(Error::Mismatch("missing structural settlement"))?;
            cursor += 1;
            let selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(expected) =
                &located.settlement
            else {
                match located.settlement {
                    selected_instructions::SelectedBoundarySettlementPayload::HostedReadByte { .. } => byte_input::validate(source, placed.machine, located, actual)?,
                    selected_instructions::SelectedBoundarySettlementPayload::HostedExitProcessI32 { .. } => process_exit::validate(source, placed.machine, located, actual)?,
                    _ => byte_output::validate(source, placed.machine, located, actual)?,
                }
                continue;
            };
            let row = &actual.settlement;
            validate_settlement_position(function, fragment, located, row.code_offset)?;
            if actual.machine != placed.machine
                || actual.text_offset
                    != host(placed.section_offset)?
                        .checked_add(row.code_offset)
                        .ok_or(Error::Overflow)?
                || row.psi_operation != expected.operation
                || row.boundary != expected.boundary
                || row.execution
                    != machine_code::BoundaryExecutionRecord::AdmittedProvider(
                        expected.provider_execution.into(),
                    )
                || row.realization
                    != target_operations::BoundaryRealization::ClaimCompletionOnly(
                        expected.realization,
                    )
                || row.arguments != expected.arguments
                || row.completion_claim_sources != expected.completion_claim_sources
                || row.completion_receipts != expected.completion_receipts
                || !row.scalar_arguments.is_empty()
                || !row.runtime_scalar_arguments.is_empty()
                || !row.byte_sequence_arguments.is_empty()
                || !row.native_result.is_unit()
                || row.operation_ordinal
                    != attribution::ordinal(
                        abstracted,
                        SemanticCodeSite::Operation(expected.operation),
                    )?
                || row.byte_count != 0
                || row.completion_provider_custody.len() != expected.completion_receipts.len()
            {
                return Err(Error::Mismatch("structural settlement custody"));
            }
            for (custody, receipt) in row
                .completion_provider_custody
                .iter()
                .zip(&expected.completion_receipts)
            {
                let expected_source = expected
                    .completion_claim_sources
                    .iter()
                    .find(|source| source.claim() == receipt.claim)
                    .ok_or(Error::Mismatch("missing completion source"))?;
                if custody.source != *expected_source
                    || custody.receipt != *receipt
                    || custody.provider_execution != expected.provider_execution.into()
                {
                    return Err(Error::Mismatch("completion provider custody"));
                }
            }
            crate::completion_receipts::validate_completion_custody(row)
                .map_err(|_| Error::Mismatch("completion receipts"))?;
        }
    }
    if cursor != rows.len() {
        return Err(Error::Mismatch("foreign structural settlement"));
    }
    Ok(())
}

/// Identify settlement rows by the admitted operation roster and check each
/// proposed location. This does not reconstruct the producer's attribution list.
pub(in crate::function_fragments) fn validate_settlement_attributions(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
    rows: &[SemanticCodeAttribution],
) -> Result<Vec<SemanticCodeAttribution>, Error> {
    let function = selected(source, machine)?;
    let fragment = fragment(source, machine)?;
    let (abstracted, _) = source::function(source, machine)?;
    let invalid = || Error::Mismatch("settlement attribution differs from admitted operation");
    let mut sites = Vec::new();
    for located in &function.boundary_settlements {
        let site = SemanticCodeSite::Operation(located.settlement.operation());
        if sites.contains(&site) {
            return Err(invalid());
        }
        sites.push(site);
        let mut matching = rows.iter().filter(|row| row.site == site);
        let row = matching.next().ok_or_else(invalid)?;
        let expected_byte_count = if matches!(
            located.settlement,
            selected_instructions::SelectedBoundarySettlementPayload::HostedWriteByteI32 { .. }
                | selected_instructions::SelectedBoundarySettlementPayload::HostedReadByte { .. }
                | selected_instructions::SelectedBoundarySettlementPayload::HostedExitProcessI32 { .. }
        ) {
            let block = function
                .blocks
                .iter()
                .find(|block| block.id == located.block)
                .ok_or_else(invalid)?;
            let instruction = block
                .instructions
                .get(located.instruction_index as usize)
                .or(match &block.terminator {
                    selected_instructions::SelectedTerminator::HostedExitProcess {
                        instruction,
                        ..
                    } if located.instruction_index as usize == block.instructions.len() => {
                        Some(instruction)
                    }
                    _ => None,
                })
                .ok_or_else(invalid)?;
            fragment
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .find(|span| span.instruction == instruction.id)
                .ok_or_else(invalid)?
                .bytes
                .len()
        } else {
            0
        };
        if matching.next().is_some()
            || row.byte_count != expected_byte_count
            || row.operation_ordinal != attribution::ordinal(abstracted, site)?
        {
            return Err(invalid());
        }
        validate_settlement_position(function, fragment, located, row.code_offset)?;
    }
    Ok(rows
        .iter()
        .filter(|row| {
            !function.boundary_settlements.iter().any(|located| {
                matches!(
                    located.settlement,
                    selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(_)
                ) && row.site == SemanticCodeSite::Operation(located.settlement.operation())
            })
        })
        .copied()
        .collect())
}

/// Independently join a selected metadata position to its physical anchor in
/// the same block; no producer offset calculation is used during replay.
fn validate_settlement_position(
    function: &SelectedFunction,
    fragment: &machine_code::FunctionFragment,
    located: &SelectedBoundarySettlement,
    proposed_offset: usize,
) -> Result<(), Error> {
    let invalid = || Error::Mismatch("settlement position has no exact physical anchor");
    let mut blocks = function
        .blocks
        .iter()
        .filter(|block| block.id == located.block);
    let block = blocks.next().ok_or_else(invalid)?;
    if blocks.next().is_some() {
        return Err(invalid());
    }
    let position = usize::try_from(located.instruction_index).map_err(|_| Error::Overflow)?;
    let anchor = match block.instructions.get(position) {
        Some(instruction) => instruction.id,
        None if position == block.instructions.len() => match &block.terminator {
            selected_instructions::SelectedTerminator::Return { instruction, .. }
            | selected_instructions::SelectedTerminator::HostedExitProcess {
                instruction, ..
            }
            | selected_instructions::SelectedTerminator::Jump { instruction, .. }
            | selected_instructions::SelectedTerminator::ConditionalBranch {
                instruction, ..
            }
            | selected_instructions::SelectedTerminator::ConditionalBranchU64LessThan {
                instruction,
                ..
            }
            | selected_instructions::SelectedTerminator::ConditionalBranchI64LessThan {
                instruction,
                ..
            } => instruction.id,
        },
        None => return Err(invalid()),
    };
    let mut spans = fragment
        .blocks
        .iter()
        .filter(|block| block.block == located.block)
        .flat_map(|block| &block.instructions)
        .filter(|span| span.instruction == anchor);
    let span = spans.next().ok_or_else(invalid)?;
    if spans.next().is_some() || host(span.offset)? != proposed_offset {
        return Err(invalid());
    }
    Ok(())
}

/// Replay the copy's exact selected membership in physical order. Spill and
/// preservation instructions may occur between its loads and outgoing stores.
fn validate_copy(
    selected: &SelectedFunction,
    fragment: &machine_code::FunctionFragment,
    operation: OperationId,
    place: PlaceId,
    actual: &InternalUnitCallArgumentRecord,
) -> Result<(), Error> {
    let invalid = || Error::Mismatch("owned argument copy differs from selected memory operations");
    let memory = selected
        .memory_accesses
        .iter()
        .filter(|row| {
            row.origin == selected_instructions::SelectedMemoryAccessOrigin::Operation(operation)
                && row.place == place
                && matches!(
                    row.role,
                    SelectedMemoryAccessRole::ReadPlace
                        | SelectedMemoryAccessRole::WriteOutgoing { .. }
                )
        })
        .collect::<Vec<_>>();
    if memory.is_empty() {
        return Err(invalid());
    }
    let mut seen = Vec::new();
    let mut first = None;
    let mut last_end = 0;
    for span in fragment.blocks.iter().flat_map(|block| &block.instructions) {
        let matches = memory
            .iter()
            .filter(|row| row.instruction == span.instruction)
            .count();
        if matches == 0 {
            continue;
        }
        if matches != 1 || seen.contains(&span.instruction) || span.bytes.is_empty() {
            return Err(invalid());
        }
        let start = host(span.offset)?;
        let end = start.checked_add(span.bytes.len()).ok_or(Error::Overflow)?;
        if first.is_some() && start < last_end {
            return Err(invalid());
        }
        first.get_or_insert(start);
        last_end = end;
        seen.push(span.instruction);
    }
    let start = first.ok_or_else(invalid)?;
    if seen.len() != memory.len()
        || actual.code_offset != start
        || actual.byte_count != last_end.checked_sub(start).ok_or(Error::Overflow)?
        || fragment.bytes.get(start..last_end) != Some(actual.bytes.as_slice())
    {
        return Err(invalid());
    }
    Ok(())
}

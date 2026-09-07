//! Structural signature and call records projected from ordinary function data.
mod validation;
use super::{Error, attribution, host, source};
use crate::{ObjectBoundarySettlement, ObjectFunction};
use calling_conventions::{IndirectPointerLocation, ValueLocation, ValuePlacement};
use legalized_operations::{LegalizedCallUnitSource, LegalizedScalarArgument};
use machine_code::{
    InternalUnitCallArgumentRecord, InternalUnitCallRecord, InternalUnitCallSource,
    SemanticCodeAttribution, SemanticCodeSite, StructuralSourceLocation, UnitParameterHomeRecord,
    UnitParameterRecord,
};
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use selected_instructions::{
    SelectedBoundarySettlement, SelectedFunction, SelectedMemoryAccessRole,
};
use semantic_vocabulary::{MachineId, OperationId, PlaceId};
use target_operations::CallSiteOwner;
pub(super) use validation::{
    validate_function, validate_settlement_attributions, validate_settlements,
};

fn selected(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
) -> Result<&SelectedFunction, Error> {
    source
        .source()
        .source()
        .source()
        .selected_plan()
        .functions
        .iter()
        .find(|row| row.machine == machine)
        .ok_or(Error::Mismatch("function has no selected contract"))
}
fn fragment(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
) -> Result<&machine_code::FunctionFragment, Error> {
    source::fragments(source)
        .functions
        .iter()
        .find(|row| row.machine == machine)
        .ok_or(Error::Mismatch("missing ordinary function fragment"))
}
fn pointer(placement: &ValuePlacement) -> Result<calling_conventions::MachineRegister, Error> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                ..
            },
        ] => Ok(*register),
        _ => Err(Error::Unsupported(
            "structural publication requires an incoming indirect register",
        )),
    }
}
fn source_location(
    function: &SelectedFunction,
    place: PlaceId,
) -> Result<StructuralSourceLocation, Error> {
    let parameter = function
        .structural
        .as_ref()
        .and_then(|contract| {
            contract
                .parameters
                .iter()
                .find(|parameter| parameter.target.place == place)
        })
        .ok_or(Error::Mismatch("structural argument has no incoming root"))?;
    Ok(StructuralSourceLocation::IncomingIndirectPointer {
        register: pointer(&parameter.target.placement)?,
    })
}

/// Exact selected memory membership, not a target-specific byte template.
fn copy_extent(
    function: &SelectedFunction,
    fragment: &machine_code::FunctionFragment,
    operation: OperationId,
    place: PlaceId,
) -> Result<(usize, usize), Error> {
    let mut spans = Vec::new();
    for access in function.memory_accesses.iter().filter(|access| {
        access.operation == operation
            && access.place == place
            && !matches!(
                access.role,
                SelectedMemoryAccessRole::AddressOutgoing { .. }
            )
    }) {
        let span = fragment
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|span| span.instruction == access.instruction)
            .ok_or(Error::Mismatch("memory operation has no physical span"))?;
        spans.push((host(span.offset)?, span.bytes.len()));
    }
    spans.sort_unstable();
    let start = spans
        .first()
        .ok_or(Error::Mismatch("owned argument has no copy"))?
        .0;
    let mut end = start;
    for (offset, length) in spans {
        if offset < end {
            return Err(Error::Mismatch("owned copy has overlapping spans"));
        }
        end = offset.checked_add(length).ok_or(Error::Overflow)?;
    }
    Ok((start, end - start))
}
fn settlement_offset(
    function: &SelectedFunction,
    fragment: &machine_code::FunctionFragment,
    row: &SelectedBoundarySettlement,
) -> Result<usize, Error> {
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == row.block)
        .ok_or(Error::Mismatch("settlement has no block"))?;
    let index = usize::try_from(row.instruction_index).map_err(|_| Error::Overflow)?;
    let instruction = if let Some(instruction) = block.instructions.get(index) {
        instruction
    } else if index == block.instructions.len() {
        match &block.terminator {
            selected_instructions::SelectedTerminator::Return { instruction, .. }
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
            } => instruction,
        }
    } else {
        return Err(Error::Mismatch("settlement position is outside block"));
    };
    fragment
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|span| span.instruction == instruction.id)
        .ok_or(Error::Mismatch("settlement has no physical anchor"))
        .and_then(|span| host(span.offset))
}

pub(super) fn populate(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    function: &mut ObjectFunction,
    rows: &[SemanticCodeAttribution],
) -> Result<(), Error> {
    let selected = selected(source, function.machine)?;
    let fragment = fragment(source, function.machine)?;
    if let Some(contract) = &selected.structural {
        for parameter in &contract.parameters {
            let target = &parameter.target;
            function.unit_parameters.push(UnitParameterRecord {
                place: target.place,
                structural_type: target.structural_type,
                multiplicity: target.multiplicity,
                access: target.access,
                shape: target.shape,
            });
            function.unit_parameter_homes.push(UnitParameterHomeRecord {
                place: target.place,
                structural_type: target.structural_type,
                multiplicity: target.multiplicity,
                access: target.access,
                shape: target.shape,
                source: target.placement.clone(),
                location: StructuralSourceLocation::IncomingIndirectPointer {
                    register: pointer(&target.placement)?,
                },
                indirect: true,
            });
        }
    }
    for contract in selected
        .calls
        .iter()
        .filter(|contract| contract.call.result_placement.is_none())
    {
        let call = &contract.call;
        let span = rows
            .iter()
            .find(|row| row.site == SemanticCodeSite::Operation(contract.operation))
            .ok_or(Error::Mismatch("Unit call has no attributed span"))?;
        let call_span = fragment
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.instruction == contract.instruction)
            .ok_or(Error::Mismatch("Unit call has no instruction span"))?;
        let frame_bytes = u32::try_from(
            source::frame(source, function.machine)?.map_or(0, |frame| frame.frame_size_bytes),
        )
        .map_err(|_| Error::Overflow)?;
        let mut arguments = Vec::new();
        for argument in &call.arguments {
            let LegalizedScalarArgument::Structural { target, .. } = argument else {
                return Err(Error::Unsupported("Unit call scalar publication"));
            };
            let (code_offset, byte_count) =
                copy_extent(selected, fragment, contract.operation, target.place)?;
            arguments.push(InternalUnitCallArgumentRecord {
                place: target.place,
                access: target.access,
                path: target.path.clone(),
                root_structural_type: target.root_structural_type,
                structural_type: target.structural_type,
                shape: target.shape,
                source_byte_offset: target.source_byte_offset,
                source_location: source_location(selected, target.place)?,
                call_stack_bytes: frame_bytes,
                fixed_array_length: target.fixed_array_length,
                element_stride: target.element_stride,
                source: target.source.clone(),
                destination: target.destination.clone(),
                code_offset,
                byte_count,
                bytes: fragment
                    .bytes
                    .get(code_offset..code_offset.checked_add(byte_count).ok_or(Error::Overflow)?)
                    .ok_or(Error::Mismatch("copy extent exceeds function"))?
                    .to_vec(),
            });
        }
        let call_source = match &call.source {
            LegalizedCallUnitSource::AuthoredCallUnit => InternalUnitCallSource::Authored,
            LegalizedCallUnitSource::InstalledProvider {
                boundary,
                provider,
                completion_claim_sources,
                completion_receipts,
            } => InternalUnitCallSource::InstalledProvider {
                boundary: *boundary,
                provider: Box::new(provider.clone()),
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
            },
        };
        function.internal_unit_calls.push(InternalUnitCallRecord {
            source: call_source,
            owner: CallSiteOwner::Operation(contract.operation),
            target: call.callee,
            result: None,
            semantic_result: None,
            structural_result: None,
            scalar_arguments: Vec::new(),
            arguments,
            claim_transfers: call.claim_transfers.clone(),
            operation_ordinal: span.operation_ordinal,
            code_offset: host(call_span.offset)?,
            byte_count: call_span.bytes.len(),
        });
    }
    Ok(())
}
pub(super) fn settlement_attributions(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
) -> Result<Vec<SemanticCodeAttribution>, Error> {
    let function = selected(source, machine)?;
    let fragment = fragment(source, machine)?;
    let (abstracted, _) = source::function(source, machine)?;
    function
        .boundary_settlements
        .iter()
        .map(|row| {
            let site = SemanticCodeSite::Operation(row.settlement.operation);
            Ok(SemanticCodeAttribution {
                site,
                operation_ordinal: attribution::ordinal(abstracted, site)?,
                code_offset: settlement_offset(function, fragment, row)?,
                byte_count: 0,
            })
        })
        .collect()
}
pub(super) fn settlements(
    source: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<Vec<ObjectBoundarySettlement>, Error> {
    let mut result = Vec::new();
    for placed in &source.source().text_section().functions {
        let function = selected(source, placed.machine)?;
        let fragment = fragment(source, placed.machine)?;
        let (abstracted, _) = source::function(source, placed.machine)?;
        for located in &function.boundary_settlements {
            let row = &located.settlement;
            let offset = settlement_offset(function, fragment, located)?;
            let execution = machine_code::BoundaryExecutionRecord::AdmittedProvider(
                row.provider_execution.into(),
            );
            result.push(ObjectBoundarySettlement {
                machine: placed.machine,
                text_offset: host(placed.section_offset)?
                    .checked_add(offset)
                    .ok_or(Error::Overflow)?,
                settlement: machine_code::BoundarySettlementRecord {
                    psi_operation: row.operation,
                    boundary: row.boundary,
                    execution,
                    realization: target_operations::BoundaryRealization::ClaimCompletionOnly(
                        row.realization,
                    ),
                    scalar_arguments: Vec::new(),
                    runtime_scalar_arguments: Vec::new(),
                    arguments: row.arguments.clone(),
                    byte_sequence_arguments: Vec::new(),
                    completion_claim_sources: row.completion_claim_sources.clone(),
                    completion_receipts: row.completion_receipts.clone(),
                    completion_provider_custody: machine_code::derive_completion_provider_custody(
                        execution,
                        &row.completion_claim_sources,
                        &row.completion_receipts,
                    )
                    .ok_or(Error::Mismatch("completion custody"))?,
                    native_result: machine_code::BoundaryResultRecord::Unit,
                    operation_ordinal: attribution::ordinal(
                        abstracted,
                        SemanticCodeSite::Operation(row.operation),
                    )?,
                    code_offset: offset,
                    byte_count: 0,
                },
            });
        }
    }
    Ok(result)
}

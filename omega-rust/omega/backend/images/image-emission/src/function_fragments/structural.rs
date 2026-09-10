//! Structural signature and call records projected from ordinary function data.
//! Source replay checks complete ordered provenance. An instruction's final
//! operation owns its physical action; earlier operations may be zero-payload
//! constructors whose charges are settled here, never extra physical effects.
mod byte_input;
mod byte_output;
pub(super) use byte_input::cleanup_actions_match as read_result_cleanup_actions_match;
mod established_views;
mod primitive_locals;
pub(super) use primitive_locals::operation_retained as primitive_operation_retained;
mod process_exit;
mod scalar_result;
mod validation;
use super::{Error, attribution, host, source};
use crate::{ObjectBoundarySettlement, ObjectFunction};
use calling_conventions::{IndirectPointerLocation, ValueLocation, ValuePlacement};
use legalized_operations::{LegalizedCallUnitSource, LegalizedScalarArgument};
use machine_code::{
    InternalUnitCallArgumentRecord, InternalUnitCallRecord, InternalUnitCallSource,
    InternalUnitScalarArgumentSourceRecord, InternalUnitScalarCallArgumentRecord,
    InternalUnitStructuralArgumentSourceRecord, SemanticCodeAttribution, SemanticCodeSite,
    StructuralSourceLocation, UnitParameterHomeRecord, UnitParameterRecord,
};
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use selected_instructions::{
    SelectedBoundarySettlement, SelectedFunction, SelectedMemoryAccessRole,
};
use semantic_vocabulary::{MachineId, OperationId, PlaceId, ScalarType, ValueId};
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
fn scalar_type(function: &SelectedFunction, value: ValueId) -> Result<ScalarType, Error> {
    use selected_instructions::VirtualRegisterOrigin;
    let mut matching = function.virtual_registers.iter().filter(|register| {
        matches!(register.origin,
            VirtualRegisterOrigin::EntryParameter { source_value, .. }
            | VirtualRegisterOrigin::BlockParameter { source_value, .. }
            | VirtualRegisterOrigin::InstructionResult { source_value, .. }
            if source_value == value)
    });
    let scalar_type = matching
        .next()
        .ok_or(Error::Mismatch("Unit call has no scalar source"))?
        .scalar_type;
    // ABI copies retain the original SSA identity. They must agree on its type;
    // selected replay independently establishes each copy and its definition.
    if matching.any(|register| register.scalar_type != scalar_type) {
        return Err(Error::Mismatch("Unit call scalar source types disagree"));
    }
    Ok(scalar_type)
}

pub(super) fn published_call(contract: &selected_instructions::SelectedCallContract) -> bool {
    // Fresh aggregate results use complete selected call/storage replay. The
    // legacy singular structural-result record describes a different family
    // (whole-input returns); recording that here would misstate result custody.
    contract.call.structural_result.is_none()
        // Inline value fragments have no pointer/copy record in the legacy
        // projection. Their complete call operands and homes belong to the
        // mandatory selected graph replay, just like aggregate results.
        && !contract.call.arguments.iter().any(|argument| {
            matches!(argument, LegalizedScalarArgument::Structural { target, .. }
                if inline_owned_placement(target.access, &target.destination))
        })
        && (contract.call.result_placement.is_none()
            || contract
                .call
                .arguments
                .iter()
                .any(|argument| matches!(argument, LegalizedScalarArgument::Structural { .. })))
}

pub(super) fn inline_owned_placement(
    access: terminal_psi::StructuralAccess,
    placement: &ValuePlacement,
) -> bool {
    access == terminal_psi::StructuralAccess::Owned
        // Empty values have no pointer home either; full graph replay retains
        // their typed argument identity independently of the absent bytes.
        && ((placement.shape == calling_conventions::ValueShape::integer(0, 1)
            && placement.locations.is_empty())
        || !placement.locations.is_empty() && placement
            .locations
            .iter()
            .all(|location| matches!(location,
                ValueLocation::Register { .. } | ValueLocation::Stack { .. })))
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
    incoming_location(parameter.target.access, &parameter.target.placement)
}

fn incoming_location(
    access: terminal_psi::StructuralAccess,
    placement: &ValuePlacement,
) -> Result<StructuralSourceLocation, Error> {
    if access == terminal_psi::StructuralAccess::Owned {
        return Ok(StructuralSourceLocation::IncomingIndirectPointer {
            register: pointer(placement)?,
        });
    }
    if placement.shape.class != calling_conventions::ValueClass::BorrowedReference {
        return Err(Error::Mismatch("borrowed home has no reference ABI"));
    }
    let location = match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 8,
            },
        ] => IndirectPointerLocation::Register(*register),
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            },
        ] => IndirectPointerLocation::Stack {
            stack_byte_offset: *stack_byte_offset,
            alignment: 8,
        },
        [
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: None,
                byte_size,
                alignment,
            },
        ] if *byte_size == placement.shape.byte_size && *alignment == placement.shape.alignment => {
            *pointer
        }
        _ => {
            return Err(Error::Unsupported(
                "borrowed publication requires exact pointer placement",
            ));
        }
    };
    Ok(StructuralSourceLocation::IncomingBorrowedPointer { location })
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
        access.origin == selected_instructions::SelectedMemoryAccessOrigin::Operation(operation)
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
    let (abstracted, targeted) = source::function(source, function.machine)?;
    let unused_owned = source::unobserved_owned_arrivals(abstracted, targeted, selected);
    if let Some(contract) = &selected.structural
        && selected.ranked.is_none()
    {
        let (parameters, homes) = if function.mixed_structural_scalar_abi.is_some() {
            (
                &mut function.scalar_structural_parameters,
                &mut function.scalar_structural_parameter_homes,
            )
        } else {
            (
                &mut function.unit_parameters,
                &mut function.unit_parameter_homes,
            )
        };
        for parameter in &contract.parameters {
            let target = &parameter.target;
            parameters.push(UnitParameterRecord {
                place: target.place,
                structural_type: target.structural_type,
                multiplicity: target.multiplicity,
                access: target.access,
                shape: target.shape,
            });
            // Direct incoming values retain their complete ABI and captured
            // registers in graph replay; a pointer-only legacy home would lie
            // about their residence. Replay independently verifies each capture.
            if unused_owned || inline_owned_placement(target.access, &target.placement) {
                continue;
            }
            homes.push(UnitParameterHomeRecord {
                place: target.place,
                structural_type: target.structural_type,
                multiplicity: target.multiplicity,
                access: target.access,
                shape: target.shape,
                source: target.placement.clone(),
                location: incoming_location(target.access, &target.placement)?,
                indirect: true,
            });
        }
    }
    for contract in selected
        .calls
        .iter()
        .filter(|contract| published_call(contract))
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
        let mut scalar_arguments = Vec::new();
        for (parameter_index, argument) in call.arguments.iter().enumerate() {
            if let LegalizedScalarArgument::Scalar {
                source: value,
                placement,
            } = argument
            {
                scalar_arguments.push(InternalUnitScalarCallArgumentRecord {
                    parameter_index: u32::try_from(parameter_index).map_err(|_| Error::Overflow)?,
                    source: InternalUnitScalarArgumentSourceRecord::SelectedCall {
                        source_value: *value,
                        scalar_type: scalar_type(selected, *value)?,
                        instruction: contract.instruction,
                    },
                    destination: placement.clone(),
                    code_offset: host(call_span.offset)?,
                    byte_count: call_span.bytes.len(),
                });
                continue;
            }
            let LegalizedScalarArgument::Structural { target, .. } = argument else {
                return Err(Error::Unsupported("Unit call scalar publication"));
            };
            let (argument_source, location) = match &target.source {
                target_operations::TargetStructuralArgumentSource::StructuralHome { .. } => {
                    return Err(Error::Mismatch(
                        "aggregate home cannot use pointer-only call records",
                    ));
                }
                target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                    psi_operation,
                } => (
                    InternalUnitStructuralArgumentSourceRecord::EstablishedPrimitiveLocal {
                        psi_operation: *psi_operation,
                    },
                    primitive_locals::location(source, selected, target, *psi_operation)?,
                ),
                target_operations::TargetStructuralArgumentSource::Placement(placement) => (
                    InternalUnitStructuralArgumentSourceRecord::Placement(placement.clone()),
                    source_location(selected, target.place)?,
                ),
                target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                    psi_operation,
                } => (
                    InternalUnitStructuralArgumentSourceRecord::EstablishedByteView {
                        psi_operation: *psi_operation,
                    },
                    established_views::location(source, selected, target, *psi_operation)?,
                ),
                target_operations::TargetStructuralArgumentSource::BlockParameter {
                    block,
                    place,
                } => (
                    InternalUnitStructuralArgumentSourceRecord::BlockParameter {
                        block: *block,
                        place: *place,
                    },
                    established_views::block_location(source, selected, target, *block, *place)?,
                ),
            };
            let (code_offset, byte_count) =
                if target.access == terminal_psi::StructuralAccess::Owned {
                    copy_extent(selected, fragment, contract.operation, target.place)?
                } else {
                    // The call names this pointer use. Retained physical replay owns
                    // its projection and transport; no referent copy is implied.
                    (host(call_span.offset)?, call_span.bytes.len())
                };
            arguments.push(InternalUnitCallArgumentRecord {
                place: target.place,
                access: target.access,
                path: target.path.clone(),
                root_structural_type: target.root_structural_type,
                structural_type: target.structural_type,
                shape: target.shape,
                source_byte_offset: target.source_byte_offset,
                source_location: location,
                call_stack_bytes: frame_bytes,
                fixed_array_length: target.fixed_array_length,
                element_stride: target.element_stride,
                source: argument_source,
                destination: target.destination.clone(),
                code_offset,
                byte_count,
                bytes: fragment
                    .bytes
                    .get(code_offset..code_offset.checked_add(byte_count).ok_or(Error::Overflow)?)
                    .ok_or(Error::Mismatch("argument extent exceeds function"))?
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
        let semantic_result = scalar_result::result(source, selected, contract)?;
        function.internal_unit_calls.push(InternalUnitCallRecord {
            source: call_source,
            owner: CallSiteOwner::Operation(contract.operation),
            target: call.callee,
            result: semantic_result.map(|result| result.scalar_type),
            semantic_result,
            structural_result: None,
            scalar_arguments,
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
        .filter(|row| {
            matches!(
                row.settlement,
                selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(_)
            )
        })
        .map(|row| {
            let site = SemanticCodeSite::Operation(row.settlement.operation());
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
            let selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(row) =
                &located.settlement
            else {
                result.push(match located.settlement {
                    selected_instructions::SelectedBoundarySettlementPayload::HostedReadByte { .. } => byte_input::settlement(source, placed.machine, located)?,
                    selected_instructions::SelectedBoundarySettlementPayload::HostedExitProcessI32 { .. } => process_exit::settlement(source, placed.machine, located)?,
                    _ => byte_output::settlement(source, placed.machine, located)?,
                });
                continue;
            };
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

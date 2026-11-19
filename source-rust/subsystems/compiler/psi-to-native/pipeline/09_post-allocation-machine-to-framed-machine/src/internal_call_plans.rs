//! Prepare internal-call publication facts while physical source is still owned.
use InternalCallPlanError as Error;
use calling_conventions::{IndirectPointerLocation, ValueLocation, ValuePlacement};
use legalized_operations::{LegalizedScalarArgument, NativeCallOrigin};
use machine_code::internal_call_plans::*;
use machine_code::{
    InternalUnitCallSource, InternalUnitScalarArgumentSourceRecord,
    InternalUnitStructuralArgumentSourceRecord, StructuralSourceLocation, UnitParameterHomeRecord,
    UnitParameterRecord,
};
use selected_instructions::{SelectedFunction, SelectedMemoryAccessRole};
use semantic_vocabulary::{MachineId, PlaceId, ScalarType, ValueId};
use target_operations::CallSiteOwner;

struct PlanningSource<'a> {
    program: &'a post_allocation_machine::PostAllocationMachineProgram,
    frames: &'a machine_code::TargetFrameLayoutPlan,
    semantic: &'a target_vocabulary::publication::EmissionProgramFacts,
}

pub fn capture(
    program: &post_allocation_machine::PostAllocationMachineProgram,
    frames: &machine_code::TargetFrameLayoutPlan,
    semantic: &target_vocabulary::publication::EmissionProgramFacts,
) -> PhysicalInternalCallPlans {
    let source = PlanningSource {
        program,
        frames,
        semantic,
    };
    PhysicalInternalCallPlans {
        functions: program
            .functions
            .iter()
            .map(|function| PhysicalFunctionInternalCalls {
                machine: function.machine,
                unpublished_calls: function
                    .calls
                    .iter()
                    .filter(|call| !published_call(function, call))
                    .map(|call| call.instruction)
                    .collect(),
                publication: prepare(&source, function),
            })
            .collect(),
    }
}

mod source {
    use super::{Error, MachineId, PlanningSource};
    pub(super) fn frame<'a>(
        source: &'a PlanningSource<'_>,
        machine: MachineId,
    ) -> Result<Option<&'a machine_code::FunctionTargetFrameLayout>, Error> {
        let mut rows = source
            .frames
            .functions
            .iter()
            .filter(|row| row.machine == machine);
        match (rows.next(), rows.next()) {
            (Some(row), None) => Ok(Some(row)),
            _ => Err(Error::Mismatch(
                "shared frame has no unique function geometry",
            )),
        }
    }
    pub(super) fn function_facts<'a>(
        source: &'a PlanningSource<'_>,
        machine: MachineId,
    ) -> Result<&'a target_vocabulary::publication::EmissionFunctionFacts, Error> {
        let mut rows = source
            .semantic
            .functions
            .iter()
            .filter(|row| row.machine == machine);
        match (rows.next(), rows.next()) {
            (Some(row), None) => Ok(row),
            _ => Err(Error::Mismatch(
                "shared function has no unique current source",
            )),
        }
    }
    pub(super) fn function_abi<'a>(
        source: &'a PlanningSource<'_>,
        machine: MachineId,
    ) -> Result<&'a target_vocabulary::function_abi::LegalizedFunctionAbi, Error> {
        source
            .program
            .contracts
            .function_abi(machine)
            .ok_or(Error::Mismatch(
                "shared function has no current calling contract",
            ))
    }
    pub(super) fn call_target_matches(
        expected: operation_operands::calls::AbstractCallTarget,
        actual: target_operations::NativeCallTarget,
    ) -> bool {
        match (expected, actual) {
            (
                operation_operands::calls::AbstractCallTarget::Machine(expected),
                target_operations::NativeCallTarget::Machine(actual),
            ) => expected == actual,
            (
                operation_operands::calls::AbstractCallTarget::Dynamic {
                    selector,
                    interface,
                    slot,
                },
                target_operations::NativeCallTarget::Dynamic {
                    selector: other,
                    interface: other_interface,
                    slot: other_slot,
                },
            ) => selector == other && interface == other_interface && slot == other_slot,
            _ => false,
        }
    }
}

fn prepare(
    source: &PlanningSource<'_>,
    selected: &SelectedFunction,
) -> Result<InternalCallPublication, Error> {
    let mut publication = InternalCallPublication {
        parameters: Vec::new(),
        homes: Vec::new(),
        calls: Vec::new(),
    };
    if let Some(contract) = &selected.structural {
        let parameters = &mut publication.parameters;
        let homes = &mut publication.homes;
        for parameter in &contract.parameters {
            let target = &parameter.target;
            parameters.push(UnitParameterRecord {
                place: target.place,
                structural_type: target.structural_type,
                multiplicity: target.multiplicity,
                access: target.access,
                shape: target.shape,
            });
            if inline_owned_placement(target.access, &target.placement) {
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
        .filter(|contract| published_call(selected, contract))
    {
        let call = &contract.call;
        let frame_bytes = u32::try_from(
            source::frame(source, selected.machine)?.map_or(0, |frame| frame.frame_size_bytes),
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
                scalar_arguments.push(PhysicalScalarCallArgument {
                    parameter_index: u32::try_from(parameter_index).map_err(|_| Error::Overflow)?,
                    source: InternalUnitScalarArgumentSourceRecord::SelectedCall {
                        source_value: *value,
                        scalar_type: scalar_type(selected, *value)?,
                        instruction: contract.instruction,
                    },
                    destination: placement.clone(),
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
                target_operations::TargetStructuralArgumentSource::EstablishedElementView {
                    psi_operation,
                } => (
                    InternalUnitStructuralArgumentSourceRecord::EstablishedElementView {
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
            let span = if target.access == terminal_psi::StructuralAccess::Owned {
                let operation = contract
                    .owner
                    .operation()
                    .ok_or(Error::Mismatch("cleanup cannot own an outgoing value copy"))?;
                let memory = selected.memory_accesses.iter().filter(|access| {
                    access.origin
                        == selected_instructions::SelectedMemoryAccessOrigin::Operation(operation)
                        && access.place == target.place
                });
                PhysicalArgumentSpan::Copy {
                    extent: memory
                        .clone()
                        .filter(|row| {
                            !matches!(row.role, SelectedMemoryAccessRole::AddressOutgoing { .. })
                        })
                        .map(|row| row.instruction)
                        .collect(),
                    transfers: memory
                        .filter(|row| {
                            matches!(
                                row.role,
                                SelectedMemoryAccessRole::ReadPlace
                                    | SelectedMemoryAccessRole::WriteOutgoing { .. }
                            )
                        })
                        .map(|row| row.instruction)
                        .collect(),
                }
            } else {
                PhysicalArgumentSpan::Call
            };
            arguments.push(PhysicalStructuralCallArgument {
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
                span,
            });
        }
        let call_source = match &call.source {
            NativeCallOrigin::Authored => InternalUnitCallSource::Authored,
            NativeCallOrigin::ProgramEntry {
                boundary,
                entry,
                source_signature,
                completion_claim_sources,
                completion_receipts,
            } => InternalUnitCallSource::ProgramEntry {
                boundary: *boundary,
                entry: *entry,
                source_signature: *source_signature,
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
            },
            NativeCallOrigin::InstalledProvider {
                boundary,
                provider,
                assumption,
                completion_claim_sources,
                completion_receipts,
            } => InternalUnitCallSource::InstalledProvider {
                boundary: *boundary,
                provider: Box::new(provider.clone()),
                assumption: assumption.clone(),
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
            },
        };
        let semantic_result = scalar_result::result(source, selected, contract)?;
        publication.calls.push(PhysicalInternalCall {
            instruction: contract.instruction,
            source: call_source,
            owner: contract.owner,
            target: call.target.machine().ok_or(Error::Mismatch(
                "legacy call mirror requires a direct machine target",
            ))?,
            semantic_result,
            scalar_arguments,
            arguments,
            claim_transfers: call.claim_transfers.clone(),
        });
    }
    Ok(publication)
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
    if matching.any(|register| register.scalar_type != scalar_type) {
        return Err(Error::Mismatch("Unit call scalar source types disagree"));
    }
    Ok(scalar_type)
}

fn published_call(
    selected: &SelectedFunction,
    contract: &selected_instructions::SelectedCallContract,
) -> bool {
    if contract.call.target.machine().is_none() {
        return false;
    }
    if contract.call.arguments.iter().any(|argument| {
        matches!(argument, LegalizedScalarArgument::Structural { target, .. }
            if target.shape.class == calling_conventions::ValueClass::BorrowedScalarReference)
    }) {
        return false;
    }
    if !matches!(contract.owner, CallSiteOwner::Operation(_)) {
        return false;
    }
    contract.call.structural_result.is_none()
        && !contract.call.arguments.iter().any(|argument| {
            let LegalizedScalarArgument::Structural { target, .. } = argument else {
                return false;
            };
            if inline_owned_placement(target.access, &target.destination) {
                return true;
            }
            use target_operations::TargetStructuralArgumentSource;
            match &target.source {
                TargetStructuralArgumentSource::Placement(placement) => {
                    selected.structural.as_ref().is_some_and(|signature| {
                        signature.parameters.iter().any(|parameter| {
                            parameter.target.place == target.place
                                && parameter.target.placement == *placement
                                && inline_owned_placement(parameter.target.access, placement)
                        })
                    })
                }
                TargetStructuralArgumentSource::StructuralHome { .. } => true,
                TargetStructuralArgumentSource::BlockParameter { block, place } => {
                    let Some(signature) = &selected.structural else {
                        return true;
                    };
                    let Some(parameter) = selected.structural_block_parameter(*block, *place)
                    else {
                        return true;
                    };
                    parameter.place != target.place
                        || parameter.structural_type != target.root_structural_type
                        || structural_storage::address_joins::is_address_join_in(
                            parameter,
                            signature.structural_types.as_slice(),
                        )
                        || !signature.structural_types.iter().any(|declaration| {
                            declaration.id == target.root_structural_type
                                && matches!(
                                    declaration.shape,
                                    terminal_psi::StructuralTypeShape::ByteSequence(
                                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                                    ) | terminal_psi::StructuralTypeShape::ElementView { .. }
                                )
                        })
                }
                _ => false,
            }
        })
        && (contract.call.result_placement.is_none()
            || contract
                .call
                .arguments
                .iter()
                .any(|argument| matches!(argument, LegalizedScalarArgument::Structural { .. })))
}

fn inline_owned_placement(
    access: terminal_psi::StructuralAccess,
    placement: &ValuePlacement,
) -> bool {
    access == terminal_psi::StructuralAccess::Owned
        && ((placement.shape == calling_conventions::ValueShape::integer(0, 1)
            && placement.locations.is_empty())
            || !placement.locations.is_empty()
                && placement.locations.iter().all(|location| {
                    matches!(
                        location,
                        calling_conventions::register_or_stack_pair_pattern!(..)
                    )
                }))
}
fn source_location(
    function: &SelectedFunction,
    place: PlaceId,
) -> Result<StructuralSourceLocation, Error> {
    let parameters = function
        .structural
        .as_ref()
        .and_then(|contract| {
            let mut parameters = contract
                .parameters
                .iter()
                .filter(|parameter| parameter.target.place == place);
            let parameter = parameters.next()?;
            parameters.next().is_none().then_some(parameter)
        })
        .ok_or(Error::Mismatch("structural argument has no incoming root"))?;
    incoming_location(parameters.target.access, &parameters.target.placement)
}

fn incoming_location(
    access: terminal_psi::StructuralAccess,
    placement: &ValuePlacement,
) -> Result<StructuralSourceLocation, Error> {
    if access == terminal_psi::StructuralAccess::Owned {
        let [
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: Some(_),
                byte_size,
                alignment,
            },
        ] = placement.locations.as_slice()
        else {
            return Err(Error::Mismatch("owned incoming home has no ABI value copy"));
        };
        if !placement.shape.class.is_integer()
            || *byte_size != placement.shape.byte_size
            || *alignment != placement.shape.alignment
        {
            return Err(Error::Mismatch(
                "owned incoming home differs from value shape",
            ));
        }
        return Ok(match pointer {
            IndirectPointerLocation::Register(register) => {
                StructuralSourceLocation::IncomingIndirectPointer {
                    register: *register,
                }
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset,
                alignment,
            } => StructuralSourceLocation::IncomingIndirectStackPointer {
                stack_byte_offset: *stack_byte_offset,
                alignment: *alignment,
            },
        });
    }
    let scalar_descriptor =
        placement.shape.class == calling_conventions::ValueClass::BorrowedScalarReference;
    if !(placement.shape.class == calling_conventions::ValueClass::BorrowedReference
        || scalar_descriptor
            && placement.shape == calling_conventions::ValueShape::borrowed_scalar_reference())
    {
        return Err(Error::Mismatch("borrowed home has no reference ABI"));
    }
    let location = match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 8,
            },
        ] if !scalar_descriptor => IndirectPointerLocation::Register(*register),
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            },
        ] if !scalar_descriptor => IndirectPointerLocation::Stack {
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
        ] if *byte_size == placement.shape.byte_size
            && *alignment == placement.shape.alignment
            && indirect_pointer_supports_copy(pointer) =>
        {
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

fn indirect_pointer_supports_copy(pointer: &IndirectPointerLocation) -> bool {
    match pointer {
        IndirectPointerLocation::Register(_) => true,
        IndirectPointerLocation::Stack {
            stack_byte_offset,
            alignment: 8,
        } => stack_byte_offset.is_multiple_of(8),
        _ => false,
    }
}

#[cfg(test)]
mod incoming_tests {
    use super::*;
    use calling_conventions::{MachineRegister, ValueShape};
    use terminal_psi::StructuralAccess;

    #[test]
    fn owned_indirect_location_keeps_owned_copy_meaning() {
        let register = MachineRegister::Aarch64X(0);
        let mut placement = ValuePlacement {
            shape: ValueShape::integer(16, 8),
            locations: vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: Some(0),
                byte_size: 16,
                alignment: 8,
            }],
        };
        assert_eq!(
            incoming_location(StructuralAccess::Owned, &placement).unwrap(),
            StructuralSourceLocation::IncomingIndirectPointer { register }
        );
        let [ValueLocation::Indirect { pointer, .. }] = placement.locations.as_mut_slice() else {
            panic!("one indirect parameter");
        };
        *pointer = IndirectPointerLocation::Stack {
            stack_byte_offset: 32,
            alignment: 8,
        };
        assert_eq!(
            incoming_location(StructuralAccess::Owned, &placement).unwrap(),
            StructuralSourceLocation::IncomingIndirectStackPointer {
                stack_byte_offset: 32,
                alignment: 8
            }
        );
        placement.shape = ValueShape::integer(8, 8);
        assert!(incoming_location(StructuralAccess::Owned, &placement).is_err());
    }
}

mod established_views {
    use super::PlanningSource;
    use super::{Error, source};
    use calling_conventions::ValueShape;
    use machine_code::StructuralSourceLocation;
    use selected_instructions::{
        FrameStorageSlotId, LocalStorageSlotId, SelectedFunction, SelectedInstructionKind,
        SelectedMemoryAccessRole, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{BlockId, OperationId, PlaceId};
    use target_operations::TargetStructuralArgument;
    use terminal_psi::{ByteSequenceCarrier, StructuralAccess, StructuralTypeShape};

    pub(super) fn location(
        source: &PlanningSource<'_>,
        selected: &SelectedFunction,
        argument: &TargetStructuralArgument,
        producer: OperationId,
    ) -> Result<StructuralSourceLocation, Error> {
        let invalid =
            || Error::Mismatch("established view differs from current descriptor custody");
        let abstracted = source::function_facts(source, selected.machine)?;
        let types = &selected
            .structural
            .as_ref()
            .ok_or_else(invalid)?
            .structural_types;
        let mut producers = abstracted
            .descriptors
            .iter()
            .filter(|row| row.operation == producer)
            .map(|row| &row.descriptor);
        match producers.next() {
            Some(target_vocabulary::publication::DescriptorProducer::ByteSubslice(result))
                if result.place == argument.place
                    && result.structural_type == argument.root_structural_type
                    && matches!(
                        result.multiplicity,
                        terminal_psi::nonlinear_multiplicity_pair_pattern!()
                    )
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty() => {}
            Some(target_vocabulary::publication::DescriptorProducer::ElementView(result))
                if result.place == argument.place
                    && result.structural_type == argument.root_structural_type
                    && matches!(
                        result.multiplicity,
                        terminal_psi::nonlinear_multiplicity_pair_pattern!()
                    )
                    && result.qualifications.is_empty()
                    && terminal_semantics::view_descriptor_projected_qualifications(
                        types.iter(),
                        result.structural_type,
                        &result.projected_qualifications,
                    )
                    && result.claims.is_empty() => {}
            Some(target_vocabulary::publication::DescriptorProducer::ByteLiteral {
                place,
                structural_type,
            }) if *place == argument.place && *structural_type == argument.root_structural_type => {
            }
            _ => return Err(invalid()),
        };
        if producers.next().is_some() {
            return Err(invalid());
        }
        local_location(
            source,
            selected,
            argument,
            LocalStorageSlotId::Structural {
                operation: producer,
                place: argument.place,
            },
            selected_instructions::SelectedMemoryAccessOrigin::Operation(producer),
        )
    }

    pub(super) fn block_location(
        source: &PlanningSource<'_>,
        selected: &SelectedFunction,
        argument: &TargetStructuralArgument,
        block: BlockId,
        place: PlaceId,
    ) -> Result<StructuralSourceLocation, Error> {
        let invalid = || Error::Mismatch("block view differs from current descriptor custody");
        let parameter = selected
            .structural_block_parameter(block, place)
            .ok_or_else(invalid)?;
        let signature = selected.structural.as_ref().ok_or_else(invalid)?;
        let access = terminal_semantics::descriptor_parameter_access(
            parameter,
            signature.structural_types.iter(),
        )
        .ok_or_else(invalid)?;
        let owned_transfer = argument.access == StructuralAccess::Owned
            && parameter.access == StructuralAccess::Owned
            && terminal_semantics::descriptor_value_transfer_matches(
                signature.structural_types.iter(),
                parameter.structural_type,
                argument.structural_type,
                parameter.multiplicity,
            );
        if place != argument.place
            || parameter.structural_type != argument.root_structural_type
            || !(owned_transfer
                || matches!(
                    (access, argument.access),
                    (
                        StructuralAccess::SharedBorrow,
                        StructuralAccess::SharedBorrow
                    ) | (
                        StructuralAccess::MutableBorrow,
                        terminal_psi::reading_borrow_pair_pattern!()
                    )
                ))
            || !(parameter.qualifications.is_empty() || access == StructuralAccess::SharedBorrow)
        {
            return Err(invalid());
        }
        local_location(
            source,
            selected,
            argument,
            LocalStorageSlotId::StructuralBlockParameter { block, place },
            selected_instructions::SelectedMemoryAccessOrigin::Block(block),
        )
    }

    fn local_location(
        source: &PlanningSource<'_>,
        selected: &SelectedFunction,
        argument: &TargetStructuralArgument,
        slot: LocalStorageSlotId,
        origin: selected_instructions::SelectedMemoryAccessOrigin,
    ) -> Result<StructuralSourceLocation, Error> {
        let invalid = || Error::Mismatch("local view differs from current descriptor custody");
        let declarations = &selected
            .structural
            .as_ref()
            .ok_or_else(invalid)?
            .structural_types;
        let declaration =
            terminal_psi::find_structural_type(declarations, argument.root_structural_type)
                .ok_or_else(invalid)?;
        let byte_size = match declaration.shape {
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
            | StructuralTypeShape::ElementView { .. } => 16,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView { .. }) => 8,
            _ => return Err(invalid()),
        };
        let element_loan = !argument.path.is_empty();
        if element_loan {
            if argument.access == StructuralAccess::Owned
                || !argument.path.iter().all(|segment| {
                    matches!(
                        segment,
                        terminal_psi::StructuralPathSegment::Field(_)
                            | terminal_psi::StructuralPathSegment::FixedIndex(_)
                            | terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
                    )
                })
                || argument.shape.alignment == 0
                || argument.fixed_array_length.is_some()
                || argument.element_stride.is_some()
            {
                return Err(invalid());
            }
        } else if !matches!(
            argument.access,
            terminal_psi::reading_borrow_pair_pattern!()
        ) || !matches!(
            terminal_semantics::borrowed_referent_presentation(
                declarations.iter(),
                argument.root_structural_type,
                argument.structural_type,
            ),
            Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
        ) || argument.shape != ValueShape::borrowed_reference(byte_size, 8)
            || argument.source_byte_offset != 0
            || argument.fixed_array_length.is_some()
            || argument.element_stride.is_some()
        {
            return Err(invalid());
        }
        let mut selected_slots = selected
            .local_storage_slots
            .iter()
            .filter(|candidate| candidate.id == slot);
        let selected_slot = selected_slots.next().ok_or_else(invalid)?;
        if selected_slots.next().is_some()
            || selected_slot.byte_size != byte_size
            || selected_slot.alignment != 8
        {
            return Err(invalid());
        }
        let frame = source::frame(source, selected.machine)?.ok_or_else(invalid)?;
        let mut slots = frame
            .local_storage_slots
            .iter()
            .filter(|candidate| candidate.id == slot);
        let local = slots.next().ok_or_else(invalid)?;
        if frame.red_zone_resident_bytes != 0
            || slots.next().is_some()
            || local.size_bytes != byte_size
            || local.alignment_bytes != 8
            || local.frame_offset_bytes % 8 != 0
            || local
                .frame_offset_bytes
                .checked_add(u64::from(byte_size))
                .is_none_or(|end| end > frame.frame_size_bytes)
        {
            return Err(invalid());
        }
        let mut accesses = selected.memory_accesses.iter().filter(|access| {
            access.origin == origin
                && access.place == argument.place
                && access.role == (SelectedMemoryAccessRole::AddressLocal { slot })
                && access.byte_offset == 0
        });
        let access = accesses.next().ok_or_else(invalid)?;
        if accesses.next().is_some() || access.byte_offset != 0 || access.byte_count != byte_size {
            return Err(invalid());
        }
        let mut instructions = selected
            .instructions()
            .filter(|instruction| instruction.id == access.instruction);
        let instruction = instructions.next().ok_or_else(invalid)?;
        if instructions.next().is_some()
            || instruction.kind
                != (SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                })
            || match origin {
                selected_instructions::SelectedMemoryAccessOrigin::Operation(producer) => {
                    instruction.provenance.operations.last() != Some(&producer)
                }
                selected_instructions::SelectedMemoryAccessOrigin::Block(owner) => {
                    instruction.provenance != Default::default()
                        || !selected.blocks.iter().any(|block| {
                            matches!(block.origin,
                                selected_instructions::SelectedBlockOrigin::Source(source)
                                | selected_instructions::SelectedBlockOrigin::Operation { source, .. }
                                if source == owner)
                                && block.instruction(instruction.id).is_some()
                        })
                }
                selected_instructions::SelectedMemoryAccessOrigin::Edge(_) => true,
            }
            || instruction.operands.len() != 1
        {
            return Err(invalid());
        }
        let pointer = instruction.operands[0].virtual_register;
        let mut registers = selected
            .virtual_registers
            .iter()
            .filter(|register| register.id == pointer);
        let register = registers.next().ok_or_else(invalid)?;
        if registers.next().is_some()
            || register.origin
                != (VirtualRegisterOrigin::AbiTransport {
                    instruction: instruction.id,
                    place: argument.place,
                    byte_offset: 0,
                })
        {
            return Err(invalid());
        }
        Ok(StructuralSourceLocation::Stack {
            byte_offset: u32::try_from(local.frame_offset_bytes).map_err(|_| Error::Overflow)?,
        })
    }
}

mod primitive_locals {
    mod establishment {
        use selected_instructions::{
            FrameStorageSlotId, LocalStorageSlotId, SelectedFunction, SelectedInstructionKind,
            SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, VirtualRegisterOrigin,
        };
        use semantic_vocabulary::{OperationId, PlaceId, ScalarType};
        use terminal_psi::StructuralTypeShape;

        pub(super) fn primitive_size(scalar: ScalarType) -> Option<u8> {
            match scalar {
                ScalarType::Integer(integer) => match integer.bits() {
                    Some(bits @ (8 | 16 | 32 | 64)) => Some((bits / 8) as u8),
                    _ => None,
                },
                ScalarType::Boolean => Some(1),
                ScalarType::Callable(_) => Some(8),
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => Some(4),
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => Some(8),
            }
        }

        pub(super) fn establishment(
            producer: Option<(
                OperationId,
                terminal_psi::StructuralOperationResult,
                operation_operands::values::AbstractResult,
            )>,
            selected: &SelectedFunction,
            place: PlaceId,
        ) -> Option<(OperationId, u8)> {
            let (operation, result, value) = producer?;
            let size = primitive_size(value.scalar_type)?;
            let contract = selected.structural.as_ref()?;
            if result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                || !result.projected_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
                || !result.claims.is_empty()
                || contract
                    .structural_places
                    .iter()
                    .filter(|declaration| {
                        declaration.id == place
                            && declaration.kind
                                == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                                    producer: operation,
                                    structural_type: result.structural_type,
                                }
                    })
                    .count()
                    != 1
                || contract
                    .structural_types
                    .iter()
                    .filter(|declaration| {
                        declaration.id == result.structural_type
                            && declaration.shape
                                == StructuralTypeShape::PrimitiveScalar(value.scalar_type)
                    })
                    .count()
                    != 1
            {
                return None;
            }
            let slot = LocalStorageSlotId::Structural { operation, place };
            let mut slots = selected
                .local_storage_slots
                .iter()
                .filter(|row| row.id == slot);
            let home = slots.next()?;
            if slots.next().is_some()
                || home.byte_size != u32::from(size)
                || home.alignment != u16::from(size)
            {
                return None;
            }
            let mut accesses = selected
                .memory_accesses
                .iter()
                .filter(|row| row.origin == SelectedMemoryAccessOrigin::Operation(operation));
            let address = accesses.next()?;
            let store = accesses.next()?;
            if accesses.next().is_some()
                || address.place != place
                || address.byte_offset != 0
                || address.byte_count != u32::from(size)
                || address.role != (SelectedMemoryAccessRole::AddressLocal { slot })
                || store.place != place
                || store.byte_offset != 0
                || store.byte_count != u32::from(size)
                || store.role != SelectedMemoryAccessRole::WritePlace
            {
                return None;
            }
            let block = selected
                .blocks
                .iter()
                .find(|block| block.instruction(address.instruction).is_some())?;
            let address_position = block.instruction_index(address.instruction)?;
            let store_position = block.instruction_index(store.instruction)?;
            let address_instruction = &block.instructions[address_position];
            let store_instruction = &block.instructions[store_position];
            let pointer = address_instruction.operands.first()?.virtual_register;
            if address_position >= store_position
                || address_instruction.kind
                    != (SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Local(slot),
                        byte_offset: 0,
                    })
                || address_instruction.operands.len() != 1
                || store_instruction.kind
                    != (SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: size,
                    })
                || store_instruction.operands.len() != 2
                || store_instruction.operands[0].virtual_register != pointer
                || address_instruction.provenance.operations.last() != Some(&operation)
                || store_instruction.provenance.operations.last() != Some(&operation)
                || !store_instruction.provenance.values.contains(&value.value)
                || !selected.virtual_registers.iter().any(|register| {
                    register.id == store_instruction.operands[1].virtual_register
                        && register.scalar_type == value.scalar_type
                        && matches!(register.origin,
                            VirtualRegisterOrigin::EntryParameter { source_value, .. }
                            | VirtualRegisterOrigin::BlockParameter { source_value, .. }
                            | VirtualRegisterOrigin::InstructionResult { source_value, .. }
                            if source_value == value.value)
                })
                || !selected.virtual_registers.iter().any(|register| {
                    register.id == pointer
                        && register.origin
                            == (VirtualRegisterOrigin::AbiTransport {
                                instruction: address.instruction,
                                place,
                                byte_offset: 0,
                            })
                })
            {
                return None;
            }
            Some((operation, size))
        }
    }

    mod frame_location {
        use super::super::PlanningSource;
        use super::super::{Error, source};
        use super::establishment::establishment;
        use machine_code::StructuralSourceLocation;
        use selected_instructions::{LocalStorageSlotId, SelectedFunction};
        use semantic_vocabulary::{OperationId, ScalarType};
        use terminal_psi::{
            StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
        };

        pub(in crate::internal_call_plans) fn location(
            source: &PlanningSource<'_>,
            selected: &SelectedFunction,
            argument: &target_operations::TargetStructuralArgument,
            operation: OperationId,
        ) -> Result<StructuralSourceLocation, Error> {
            let invalid = || {
                Error::Mismatch(
                    "primitive local producer, initialization or frame identity differs",
                )
            };
            let function = source::function_facts(source, selected.machine)?;
            let fact = function
                .primitive_local(argument.place)
                .ok_or_else(invalid)?;
            let result = &fact.result;
            let value = fact.initializer;
            let (producer_operation, size) = establishment(
                Some((fact.operation, result.clone(), value)),
                selected,
                argument.place,
            )
            .ok_or_else(invalid)?;
            let contract = selected.structural.as_ref().ok_or_else(invalid)?;
            if producer_operation != operation
                || !argument_matches_source(
                    argument,
                    result.structural_type,
                    value.scalar_type,
                    &contract.structural_types,
                )
                || argument.source_byte_offset != 0
                || !matches!(
                    argument.access,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                )
                || argument.shape
                    != calling_conventions::ValueShape::borrowed_reference(
                        u32::from(u16::from(size)),
                        u16::from(size),
                    )
                || argument.fixed_array_length.is_some()
                || argument.element_stride.is_some()
            {
                return Err(invalid());
            }
            let frame = source::frame(source, selected.machine)?.ok_or_else(invalid)?;
            let slot = LocalStorageSlotId::Structural {
                operation,
                place: argument.place,
            };
            if frame.red_zone_resident_bytes != 0 {
                return Err(invalid());
            }
            frame_location(
                &frame.local_storage_slots,
                frame.frame_size_bytes,
                slot,
                size,
            )
        }

        pub(super) fn frame_location(
            homes: &[machine_code::LocalStorageFrameSlot],
            frame_size_bytes: u64,
            slot: LocalStorageSlotId,
            size: u8,
        ) -> Result<StructuralSourceLocation, Error> {
            let invalid = || Error::Mismatch("primitive local frame identity or extent differs");
            let mut slots = homes.iter().filter(|row| row.id == slot);
            let home = slots.next().ok_or_else(invalid)?;
            if slots.next().is_some()
                || home.size_bytes != u32::from(size)
                || home.alignment_bytes != u16::from(size)
                || !home.frame_offset_bytes.is_multiple_of(u64::from(size))
                || home
                    .frame_offset_bytes
                    .checked_add(u64::from(size))
                    .is_none_or(|end| end > frame_size_bytes)
            {
                return Err(invalid());
            }
            Ok(StructuralSourceLocation::Stack {
                byte_offset: u32::try_from(home.frame_offset_bytes).map_err(|_| Error::Overflow)?,
            })
        }

        pub(super) fn argument_matches_source(
            argument: &target_operations::TargetStructuralArgument,
            source_type: semantic_vocabulary::StructuralTypeId,
            source_scalar: ScalarType,
            types: &[StructuralTypeDeclaration],
        ) -> bool {
            let scalar = |identity| {
                let mut declarations = types.iter().filter(|row| row.id == identity);
                let declaration = declarations.next()?;
                if declarations.next().is_some() {
                    return None;
                }
                match declaration.shape {
                    StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
                    _ => None,
                }
            };
            if argument.root_structural_type != source_type
                || scalar(source_type) != Some(source_scalar)
            {
                return false;
            }
            if !argument.path.is_empty()
                && argument.path.last() != Some(&StructuralPathSegment::Referent)
            {
                return false;
            }
            if argument.structural_type == source_type {
                return true;
            }
            argument.path.last() == Some(&StructuralPathSegment::Referent)
                && scalar(argument.structural_type).is_some_and(|target| {
                    terminal_semantics::scalar_reference_view_admits(
                        argument.access,
                        source_scalar,
                        target,
                    )
                })
        }
    }

    pub(super) use frame_location::location;
}
mod scalar_result {
    use super::PlanningSource;
    use super::{Error, source};
    use selected_instructions::SelectedFunction;

    pub(super) fn result(
        source: &PlanningSource<'_>,
        selected: &SelectedFunction,
        contract: &selected_instructions::SelectedCallContract,
    ) -> Result<Option<operation_operands::values::AbstractResult>, Error> {
        let Some(callee_machine) = contract.call.target.machine() else {
            return Ok(None);
        };
        let Some(placement) = &contract.call.result_placement else {
            return Ok(None);
        };
        let invalid =
            || Error::Mismatch("structural scalar call result differs from current source");
        let abstracted = source::function_facts(source, selected.machine)?;
        let installed_boundary = match &contract.call.source {
            legalized_operations::NativeCallOrigin::InstalledProvider { boundary, .. }
            | legalized_operations::NativeCallOrigin::ProgramEntry { boundary, .. } => {
                Some(*boundary)
            }
            legalized_operations::NativeCallOrigin::Authored => None,
        };
        let mut operations = abstracted.scalar_call_results.iter().filter_map(|row| {
            if contract.owner != target_operations::CallSiteOwner::Operation(row.operation) {
                return None;
            }
            let matches = match row.target {
                target_vocabulary::publication::ScalarCallOrigin::Authored(callee) => {
                    installed_boundary.is_none()
                        && source::call_target_matches(callee, contract.call.target)
                }
                target_vocabulary::publication::ScalarCallOrigin::Boundary(boundary) => {
                    installed_boundary == Some(boundary)
                }
            };
            matches.then_some(row.result)
        });
        let result = operations.next().ok_or_else(invalid)?;
        let callee = source::function_facts(source, callee_machine)?;
        let abi = source::function_abi(source, callee_machine)?
            .mixed_structural_scalar
            .as_ref()
            .ok_or_else(invalid)?;
        if operations.next().is_some()
            || callee
                .result
                .scalar()
                .is_none_or(|value| value.scalar_type != result.scalar_type)
            || abi.result.scalar_type != result.scalar_type
            || abi.result.placement != *placement
            || abi.call_plan != contract.call.call_plan
        {
            return Err(invalid());
        }
        let mut instructions = selected
            .instructions()
            .filter(|instruction| instruction.id == contract.instruction);
        let instruction = instructions.next().ok_or_else(invalid)?;
        let output = instruction
            .operands
            .last()
            .ok_or_else(invalid)?
            .virtual_register;
        if instructions.next().is_some()
            || instruction.kind
                != (selected_instructions::SelectedInstructionKind::CallScalar {
                    target: selected_instructions::SelectedCallTarget::Machine(callee_machine),
                })
            || !selected.virtual_registers.iter().any(|register| {
                register.id == output && definition_is_exact(register, contract.instruction, result)
            })
        {
            return Err(invalid());
        }
        Ok(Some(result))
    }

    fn definition_is_exact(
        register: &selected_instructions::VirtualRegister,
        instruction: selected_instructions::SelectedInstructionId,
        result: operation_operands::values::AbstractResult,
    ) -> bool {
        register.scalar_type == result.scalar_type
            && matches!(register.origin,
                selected_instructions::VirtualRegisterOrigin::InstructionResult { instruction: defining, source_value }
                    if defining == instruction && source_value == result.value)
    }

    #[cfg(test)]
    mod tests {
        use super::definition_is_exact;
        use selected_instructions::{
            SelectedInstructionId, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
        };
        use semantic_vocabulary::{ScalarType, ValueId};

        #[test]
        fn scalar_call_result_requires_the_exact_call_definition_and_value() {
            let instruction = SelectedInstructionId(7);
            let result = operation_operands::values::AbstractResult {
                value: ValueId::new(9).unwrap(),
                scalar_type: ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                ),
            };
            let register = VirtualRegister {
                id: VirtualRegisterId(2),
                scalar_type: result.scalar_type,
                class: register_model::RegisterClassId(0),
                origin: VirtualRegisterOrigin::InstructionResult {
                    instruction,
                    source_value: result.value,
                },
                definition_site: None,
                entry_fixed_view: None,
            };
            assert!(definition_is_exact(&register, instruction, result));
            for mutation in 0..4 {
                let mut changed = register.clone();
                match mutation {
                    0 => changed.scalar_type = ScalarType::Boolean,
                    1 => {
                        changed.origin = VirtualRegisterOrigin::InstructionResult {
                            instruction: SelectedInstructionId(8),
                            source_value: result.value,
                        }
                    }
                    2 => {
                        changed.origin = VirtualRegisterOrigin::InstructionResult {
                            instruction,
                            source_value: ValueId::new(10).unwrap(),
                        }
                    }
                    _ => {
                        changed.origin = VirtualRegisterOrigin::EntryParameter {
                            source_value: result.value,
                            parameter_index: 0,
                        }
                    }
                }
                assert!(
                    !definition_is_exact(&changed, instruction, result),
                    "mutation {mutation}"
                );
            }
        }
    }
}

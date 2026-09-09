//! Explicit register transport across authored successor bindings.
use super::LivenessError;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedSuccessor, SelectedSuccessorRole,
    SelectedTerminator, SelectedValueTransport, VirtualRegisterId, VirtualRegisterOrigin,
};

pub(crate) fn has_edge_use(function: &SelectedFunction, register: VirtualRegisterId) -> bool {
    function.blocks.iter().any(|block| {
        let edges = match &block.terminator {
            SelectedTerminator::Jump { successor, .. } => vec![successor],
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => vec![when_nonzero, when_zero],
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            } => vec![when_less, when_not_less],
            SelectedTerminator::Return { .. } | SelectedTerminator::HostedExitProcess { .. } => {
                Vec::new()
            }
        };
        edges.iter().any(|edge| {
            edge.structural_case.as_ref().is_some_and(|case| case.payloads.iter().any(|payload| matches!(payload.transport,
                selected_instructions::SelectedCasePayloadTransport::Registers { argument, .. } if argument == register)))
            || edge.bindings.iter().any(|binding| {
                matches!(binding.transport,
            SelectedValueTransport::Registers {argument,..} if argument == register)
            })
        })
    })
}

pub(crate) fn validate_transports(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<(), LivenessError> {
    let mismatch = || LivenessError::FunctionMismatch {
        function: function_index,
    };
    for block in &function.blocks {
        let edges = match &block.terminator {
            SelectedTerminator::Jump { successor, .. } => vec![successor],
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => vec![when_nonzero, when_zero],
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            } => vec![when_less, when_not_less],
            SelectedTerminator::Return { .. } | SelectedTerminator::HostedExitProcess { .. } => {
                Vec::new()
            }
        };
        for edge in edges {
            match (block.origin, edge.role) {
                (SelectedBlockOrigin::Source(_) | SelectedBlockOrigin::CaseDispatch { .. }, SelectedSuccessorRole::Semantic) => {}
                (SelectedBlockOrigin::Source(_) | SelectedBlockOrigin::CaseDispatch { .. }, SelectedSuccessorRole::CaseDispatchContinuation)
                    if edge.fuel.is_empty() && edge.bindings.is_empty()
                        && edge.structural_bindings.is_empty() && edge.structural_case.is_none()
                        && matches!(block.terminator, SelectedTerminator::ConditionalBranch { .. }) => {}
                (
                    SelectedBlockOrigin::EdgeTransfer {
                        edge: owner,
                        target,
                    },
                    SelectedSuccessorRole::EdgeTransferContinuation,
                ) if owner == edge.psi_edge
                    && target == edge.source_target
                    && edge.fuel.is_empty()
                    && matches!(block.terminator, SelectedTerminator::Jump { .. }) => {}
                _ => return Err(mismatch()),
            }
            if function
                .blocks
                .iter()
                .filter(|target| {
                    target.id == edge.block && target.source_block() == edge.source_target
                })
                .count()
                != 1
            {
                return Err(mismatch());
            }
            let destination = function
                .blocks
                .iter()
                .find(|target| target.id == edge.block)
                .ok_or_else(mismatch)?;
            match destination.origin {
                SelectedBlockOrigin::Source(_) => {}
                SelectedBlockOrigin::CaseDispatch { source, .. }
                    if edge.role == SelectedSuccessorRole::CaseDispatchContinuation
                        && source == edge.source_target => {}
                SelectedBlockOrigin::EdgeTransfer {
                    edge: owner,
                    target,
                } if edge.role == SelectedSuccessorRole::Semantic
                    && owner == edge.psi_edge
                    && target == edge.source_target
                    && edge
                        .bindings
                        .iter()
                        .all(|binding| binding.transport == SelectedValueTransport::Unused) => {}
                _ => return Err(mismatch()),
            }
            validate_case_transport(function_index, function, edge, destination)?;
            for binding in &edge.bindings {
                if edge
                    .bindings
                    .iter()
                    .filter(|other| other.semantic.parameter == binding.semantic.parameter)
                    .count()
                    != 1
                {
                    return Err(mismatch());
                }
                let destinations = function
                    .virtual_registers
                    .iter()
                    .filter(|register| {
                        matches!(register.origin,
                    VirtualRegisterOrigin::BlockParameter {source_value,block,..}
                        if source_value == binding.semantic.parameter && block == edge.block)
                    })
                    .collect::<Vec<_>>();
                match binding.transport {
                    SelectedValueTransport::Unused if destinations.is_empty() => {}
                    SelectedValueTransport::Registers { parameter, .. }
                        if destinations.len() == 1 && destinations[0].id == parameter =>
                    {
                        incoming_argument(function_index, function, edge, parameter)?;
                    }
                    _ => return Err(mismatch()),
                }
            }
            for destination in function.virtual_registers.iter().filter(|register| {
                matches!(register.origin,
                VirtualRegisterOrigin::BlockParameter {block,..} if block == edge.block)
            }) {
                incoming_argument(function_index, function, edge, destination.id)?;
            }
        }
    }
    Ok(())
}

/// Live non-parameters pass through; a destination parameter uses its exact
/// selected transport, never the first or latest register sharing a ValueId.
pub(crate) fn incoming_argument(
    function_index: usize,
    function: &SelectedFunction,
    successor: &SelectedSuccessor,
    destination: VirtualRegisterId,
) -> Result<VirtualRegisterId, LivenessError> {
    let mismatch = || LivenessError::FunctionMismatch {
        function: function_index,
    };
    let mut destinations = function
        .virtual_registers
        .iter()
        .filter(|register| register.id == destination);
    let destination_register = destinations.next().ok_or_else(mismatch)?;
    if destinations.next().is_some() {
        return Err(mismatch());
    }
    let VirtualRegisterOrigin::BlockParameter {
        source_value,
        block,
        parameter_index,
    } = destination_register.origin
    else {
        return Ok(destination);
    };
    if block != successor.block {
        return Ok(destination);
    }
    if destination_register.definition_site
        != Some(optimization_unit::ValueDefinitionSite::BlockParameter {
            block: successor.source_target,
            position: u32::try_from(parameter_index).map_err(|_| mismatch())?,
        })
    {
        return Err(mismatch());
    }
    if let Some(case) = &successor.structural_case {
        let mut payloads = case
            .payloads
            .iter()
            .filter(|payload| payload.semantic.parameter.value == source_value);
        if let Some(payload) = payloads.next() {
            if payloads.next().is_some()
                || successor
                    .bindings
                    .iter()
                    .any(|binding| binding.semantic.parameter == source_value)
                || payload.semantic.parameter.scalar_type != destination_register.scalar_type
                || Some(payload.semantic.parameter.definition_site)
                    != destination_register.definition_site
            {
                return Err(mismatch());
            }
            let selected_instructions::SelectedCasePayloadTransport::Registers {
                argument,
                parameter,
            } = payload.transport
            else {
                return Err(mismatch());
            };
            if parameter != destination
                || successor.role != SelectedSuccessorRole::EdgeTransferContinuation
            {
                return Err(mismatch());
            }
            let mut sources = function
                .virtual_registers
                .iter()
                .filter(|register| register.id == argument);
            let source = sources.next().ok_or_else(mismatch)?;
            if sources.next().is_some()
                || source.scalar_type != destination_register.scalar_type
                || source.class != destination_register.class
                || source.definition_site.is_some()
                || !matches!(source.origin, VirtualRegisterOrigin::StructuralObservation { place, byte_offset, .. }
                    if Some(place) == case.slot.structural_place() && byte_offset == payload.semantic.field_byte_offset)
            {
                return Err(mismatch());
            }
            return Ok(argument);
        }
    }
    let mut bindings = successor
        .bindings
        .iter()
        .enumerate()
        .filter(|(_, binding)| binding.semantic.parameter == source_value);
    let (binding_position, binding) = bindings.next().ok_or_else(mismatch)?;
    if bindings.next().is_some()
        || binding_position != parameter_index
        || binding.semantic.scalar_type != destination_register.scalar_type
    {
        return Err(mismatch());
    }
    let SelectedValueTransport::Registers {
        argument,
        parameter,
    } = binding.transport
    else {
        return Err(mismatch());
    };
    if parameter != destination {
        return Err(mismatch());
    }
    let mut sources = function
        .virtual_registers
        .iter()
        .filter(|register| register.id == argument);
    let source = sources.next().ok_or_else(mismatch)?;
    if sources.next().is_some() {
        return Err(mismatch());
    }
    let value = match source.origin {
        VirtualRegisterOrigin::StructuralParameter { .. }
        | VirtualRegisterOrigin::SpillAddress { .. }
        | VirtualRegisterOrigin::StructuralObservation { .. }
        | VirtualRegisterOrigin::ScalarAbiAddress { .. }
        | VirtualRegisterOrigin::AbiTransport { .. } => return Err(mismatch()),
        VirtualRegisterOrigin::EntryParameter { source_value, .. }
        | VirtualRegisterOrigin::BlockParameter { source_value, .. }
        | VirtualRegisterOrigin::InstructionResult { source_value, .. } => source_value,
    };
    if value != binding.semantic.argument
        || source.scalar_type != binding.semantic.scalar_type
        || source.class != destination_register.class
    {
        return Err(mismatch());
    }
    Ok(argument)
}

fn validate_case_transport(
    function_index: usize,
    function: &SelectedFunction,
    edge: &SelectedSuccessor,
    destination: &selected_instructions::SelectedBlock,
) -> Result<(), LivenessError> {
    use selected_instructions::SelectedCasePayloadTransport;
    let mismatch = || LivenessError::FunctionMismatch {
        function: function_index,
    };
    let Some(case) = &edge.structural_case else {
        return Ok(());
    };
    if case.slot.structural_place().is_none()
        || case.case_tag < 0
        || (edge.role == SelectedSuccessorRole::EdgeTransferContinuation
            && !case.trivial_affine_discards.is_empty())
    {
        return Err(mismatch());
    }
    if matches!(destination.origin, SelectedBlockOrigin::EdgeTransfer { .. }) {
        let SelectedTerminator::Jump {
            successor: continuation,
            ..
        } = &destination.terminator
        else {
            return Err(mismatch());
        };
        let Some(next_case) = &continuation.structural_case else {
            return Err(mismatch());
        };
        if next_case.slot != case.slot
            || next_case.case != case.case
            || next_case.case_tag != case.case_tag
            || !next_case.trivial_affine_discards.is_empty()
            || next_case.payloads.len() != case.payloads.len()
            || next_case
                .payloads
                .iter()
                .zip(&case.payloads)
                .any(|(next, semantic)| next.semantic != semantic.semantic)
        {
            return Err(mismatch());
        }
    }
    for payload in &case.payloads {
        if case
            .payloads
            .iter()
            .filter(|other| other.semantic.parameter.value == payload.semantic.parameter.value)
            .count()
            != 1
            || case
                .payloads
                .iter()
                .filter(|other| other.semantic.field == payload.semantic.field)
                .count()
                != 1
            || edge
                .bindings
                .iter()
                .any(|binding| binding.semantic.parameter == payload.semantic.parameter.value)
        {
            return Err(mismatch());
        }
        let destinations = function.virtual_registers.iter().filter(|register| {
            matches!(register.origin, VirtualRegisterOrigin::BlockParameter { source_value, block, .. }
                if source_value == payload.semantic.parameter.value && block == edge.block)
        }).collect::<Vec<_>>();
        match payload.transport {
            SelectedCasePayloadTransport::Unused if destinations.is_empty() => {}
            SelectedCasePayloadTransport::Registers { parameter, .. }
                if edge.role == SelectedSuccessorRole::EdgeTransferContinuation
                    && destinations.len() == 1
                    && destinations[0].id == parameter =>
            {
                incoming_argument(function_index, function, edge, parameter)?;
            }
            _ => return Err(mismatch()),
        }
    }
    Ok(())
}

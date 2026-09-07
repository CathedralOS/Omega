//! Explicit register transport across authored successor bindings.
use super::LivenessError;
use selected_instructions::{
    SelectedFunction, SelectedSuccessor, SelectedTerminator, SelectedValueTransport,
    VirtualRegisterId, VirtualRegisterOrigin,
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
            SelectedTerminator::Return { .. } => Vec::new(),
        };
        edges.iter().any(|edge| {
            edge.bindings.iter().any(|binding| {
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
            SelectedTerminator::Return { .. } => Vec::new(),
        };
        for edge in edges {
            if function
                .blocks
                .iter()
                .filter(|target| {
                    target.id == edge.block && target.source_block == edge.source_target
                })
                .count()
                != 1
            {
                return Err(mismatch());
            }
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
        != (optimization_unit::ValueDefinitionSite::BlockParameter {
            block: successor.source_target,
            position: u32::try_from(parameter_index).map_err(|_| mismatch())?,
        })
    {
        return Err(mismatch());
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
        VirtualRegisterOrigin::EntryParameter { source_value, .. }
        | VirtualRegisterOrigin::BlockParameter { source_value, .. }
        | VirtualRegisterOrigin::InstructionResult { source_value, .. }
        | VirtualRegisterOrigin::LegalizationTemporary { source_value, .. } => source_value,
    };
    if value != binding.semantic.argument
        || source.scalar_type != binding.semantic.scalar_type
        || source.class != destination_register.class
    {
        return Err(mismatch());
    }
    Ok(argument)
}

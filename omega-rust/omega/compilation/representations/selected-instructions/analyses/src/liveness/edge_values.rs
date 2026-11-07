//! Explicit register transport across authored successor bindings.
use super::LivenessError;

#[cfg(test)]
mod tests;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedSuccessor,
    SelectedSuccessorRole, SelectedTerminator, SelectedValueTransport, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin, terminator_successors,
};
mod case_payload;

/// One function's block parameters and instructions, indexed once per
/// analysis. Transport checks run per edge and per destination; scanning the
/// whole function for each made every liveness and live-range analysis
/// quadratic in function size, and runtime spill recovery repeats those
/// analyses once per step. Blocks and registers need no index: their IDs are
/// their positions. A function that breaks that invariant resolves no block
/// or register, so its transports refuse.
pub(crate) struct FunctionIndex<'function> {
    function: &'function SelectedFunction,
    ids_are_positions: bool,
    /// `(block, position)` for every block-parameter register, sorted by block
    /// and then by function order.
    parameters: Vec<(SelectedBlockId, usize)>,
    /// Instruction IDs and their exact owning block and position, built lazily.
    instructions:
        std::sync::OnceLock<Vec<(selected_instructions::SelectedInstructionId, usize, usize)>>,
}

impl<'function> FunctionIndex<'function> {
    pub fn new(function: &'function SelectedFunction) -> Self {
        let mut parameters = function
            .virtual_registers
            .iter()
            .enumerate()
            .filter_map(|(position, register)| match register.origin {
                VirtualRegisterOrigin::BlockParameter { block, .. } => Some((block, position)),
                _ => None,
            })
            .collect::<Vec<_>>();
        parameters.sort_unstable();
        Self {
            function,
            ids_are_positions: function.ids_are_positions(),
            parameters,
            instructions: std::sync::OnceLock::new(),
        }
    }

    fn unique_instruction(
        &self,
        id: selected_instructions::SelectedInstructionId,
    ) -> Option<(
        &'function SelectedBlock,
        &'function selected_instructions::SelectedInstruction,
        usize,
    )> {
        let instructions = self.instructions.get_or_init(|| {
            let mut list = self
                .function
                .blocks
                .iter()
                .enumerate()
                .flat_map(|(block, row)| {
                    row.instructions
                        .iter()
                        .enumerate()
                        .map(move |(position, instruction)| (instruction.id, block, position))
                })
                .collect::<Vec<_>>();
            list.sort_unstable();
            list
        });
        let start = instructions.partition_point(|(other, _, _)| *other < id);
        let [(first, block, position), rest @ ..] = instructions.get(start..)? else {
            return None;
        };
        if *first != id || rest.first().is_some_and(|(next, _, _)| *next == id) {
            return None;
        }
        let block = self.function.blocks.get(*block)?;
        Some((block, block.instructions.get(*position)?, *position))
    }

    /// The one register carrying `id`, or `None` when no register or more
    /// than one carries it.
    pub(crate) fn unique_register(
        &self,
        id: VirtualRegisterId,
    ) -> Option<&'function VirtualRegister> {
        self.ids_are_positions
            .then(|| self.function.virtual_register(id))
            .flatten()
    }

    /// The block-parameter registers of `block`, in function order.
    pub(crate) fn parameters_of(
        &self,
        block: SelectedBlockId,
    ) -> impl Iterator<Item = &'function VirtualRegister> + '_ {
        let start = self.parameters.partition_point(|(other, _)| *other < block);
        self.parameters[start..]
            .iter()
            .take_while(move |(other, _)| *other == block)
            .filter_map(|(_, position)| self.function.virtual_registers.get(*position))
    }

    /// The position of the block keyed `id`.
    pub(crate) fn block_index(&self, id: SelectedBlockId) -> Option<usize> {
        self.ids_are_positions
            .then(|| self.function.block_index(id))
            .flatten()
    }

    /// The block keyed `id`.
    pub(crate) fn block(&self, id: SelectedBlockId) -> Option<&'function SelectedBlock> {
        self.block_index(id)
            .map(|index| &self.function.blocks[index])
    }
}

pub(crate) fn has_edge_use(function: &SelectedFunction, register: VirtualRegisterId) -> bool {
    function.blocks.iter().any(|block| {
        let edges = match &block.terminator {
            SelectedTerminator::Jump { successor, .. } => vec![successor],
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => vec![when_nonzero, when_zero],
            selected_instructions::conditional_branch_less_pair_pattern!(when_less,when_not_less, ..) => vec![when_less, when_not_less],
            selected_instructions::terminator_no_successor_pattern!(..) => {
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
    if !function.blocks.iter().any(|block| {
        matches!(block.origin, SelectedBlockOrigin::Operation { .. })
            || matches!(block.terminator, SelectedTerminator::OperationTrap { .. })
            || matches!(
                block.terminator,
                SelectedTerminator::OperationInvokeNoNormal { .. }
            )
            || !terminator_successors(&block.terminator).is_empty()
    }) {
        return Ok(());
    }
    let index = FunctionIndex::new(function);
    for block in &function.blocks {
        if let SelectedBlockOrigin::Operation { operation, .. } = block.origin
            && (!function.provenance.operations.contains(&operation)
                || block.id == function.entry_block)
        {
            return Err(mismatch());
        }
        if let SelectedTerminator::OperationInvokeNoNormal {
            instruction,
            operation,
        } = &block.terminator
        {
            let mut calls = function
                .calls
                .iter()
                .filter(|row| row.instruction == instruction.id);
            let (Some(call), None) = (calls.next(), calls.next()) else {
                return Err(mismatch());
            };
            if !matches!(block.origin, SelectedBlockOrigin::Operation { operation: owner, .. } if owner == *operation)
                || !matches!(
                    instruction.kind,
                    selected_instructions::SelectedInstructionKind::InvokeNoNormal { .. }
                )
                || instruction.kind.call_target()
                    != Some(selected_instructions::SelectedCallTarget::from_native(
                        call.call.target,
                    ))
                || call.owner
                    != (target_operations::CallSiteOwner::Installation {
                        operation: *operation,
                        role: target_operations::InstallationCallRole::Work,
                    })
                || instruction.provenance.operations.as_slice() != [*operation]
                || !instruction.provenance.edges.is_empty()
                || !instruction.provenance.work.is_empty()
            {
                return Err(mismatch());
            }
        }
        if let SelectedTerminator::OperationTrap {
            instruction,
            operation,
        } = &block.terminator
            && (!matches!(block.origin, SelectedBlockOrigin::Operation { operation: owner, .. } if owner == *operation)
                || !matches!(
                    instruction.kind,
                    selected_instructions::SelectedInstructionKind::Crash
                        | selected_instructions::SelectedInstructionKind::PrivilegedCrash
                )
                || instruction.provenance.operations.as_slice() != [*operation]
                || !instruction.provenance.edges.is_empty()
                || !instruction.provenance.work.is_empty())
        {
            return Err(mismatch());
        }
        let edges = terminator_successors(&block.terminator);
        for edge in edges {
            if edge.psi_edge.is_none()
                != matches!(
                    edge.role,
                    SelectedSuccessorRole::OperationContinuation { .. }
                )
            {
                return Err(mismatch());
            }
            match (block.origin, edge.role) {
                (
                    SelectedBlockOrigin::Source(_)
                    | SelectedBlockOrigin::CaseDispatch { .. }
                    | SelectedBlockOrigin::Operation { .. },
                    SelectedSuccessorRole::Semantic,
                ) => {}
                (
                    SelectedBlockOrigin::Source(_)
                    | SelectedBlockOrigin::CaseDispatch { .. }
                    | SelectedBlockOrigin::Operation { .. },
                    SelectedSuccessorRole::CaseDispatchContinuation,
                ) if edge.work.is_empty()
                    && edge.bindings.is_empty()
                    && edge.structural_bindings.is_empty()
                    && edge.structural_case.is_none()
                    && matches!(
                        block.terminator,
                        SelectedTerminator::ConditionalBranch { .. }
                    ) => {}
                (
                    SelectedBlockOrigin::Source(_) | SelectedBlockOrigin::Operation { .. },
                    SelectedSuccessorRole::OperationContinuation { operation },
                ) if edge.source_target == block.source_block()
                    && edge.work.is_empty()
                    && edge.bindings.is_empty()
                    && edge.structural_bindings.is_empty()
                    && edge.structural_case.is_none()
                    && selected_instructions::terminator_instruction(&block.terminator)
                        .provenance
                        .operations
                        .contains(&operation)
                    && selected_instructions::terminator_instruction(&block.terminator)
                        .provenance
                        .edges
                        .is_empty()
                    && selected_instructions::terminator_instruction(&block.terminator)
                        .provenance
                        .work
                        .is_empty() => {}
                (
                    SelectedBlockOrigin::EdgeTransfer {
                        edge: owner,
                        target,
                    },
                    SelectedSuccessorRole::EdgeTransferContinuation,
                ) if Some(owner) == edge.psi_edge
                    && target == edge.source_target
                    && edge.work.is_empty()
                    && matches!(block.terminator, SelectedTerminator::Jump { .. }) => {}
                _ => return Err(mismatch()),
            }
            let destination = index
                .block(edge.block)
                .filter(|target| target.source_block() == edge.source_target)
                .ok_or_else(mismatch)?;
            match destination.origin {
                SelectedBlockOrigin::Source(_)
                    if !matches!(
                        edge.role,
                        SelectedSuccessorRole::OperationContinuation { .. }
                    ) => {}
                SelectedBlockOrigin::Operation { source, operation }
                    if edge.role
                        == (SelectedSuccessorRole::OperationContinuation { operation })
                        && source == edge.source_target => {}
                SelectedBlockOrigin::CaseDispatch { source, .. }
                    if edge.role == SelectedSuccessorRole::CaseDispatchContinuation
                        && source == edge.source_target => {}
                SelectedBlockOrigin::EdgeTransfer {
                    edge: owner,
                    target,
                } if edge.role == SelectedSuccessorRole::Semantic
                    && Some(owner) == edge.psi_edge
                    && target == edge.source_target
                    && edge
                        .bindings
                        .iter()
                        .all(|binding| binding.transport == SelectedValueTransport::Unused) => {}
                _ => return Err(mismatch()),
            }
            validate_case_transport(function_index, &index, edge, destination)?;
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
                let mut destinations = index.parameters_of(edge.block).filter(|register| {
                    matches!(register.origin,
                VirtualRegisterOrigin::BlockParameter {source_value,..}
                    if source_value == binding.semantic.parameter)
                });
                let first_dest = destinations.next();
                match binding.transport {
                    SelectedValueTransport::Unused if first_dest.is_none() => {}
                    SelectedValueTransport::Registers { parameter, .. }
                        if first_dest.is_some_and(|d| d.id == parameter)
                            && destinations.next().is_none() =>
                    {
                        incoming_argument(function_index, &index, edge, parameter)?;
                    }
                    _ => return Err(mismatch()),
                }
            }
            for destination in index.parameters_of(edge.block) {
                incoming_argument(function_index, &index, edge, destination.id)?;
            }
        }
    }
    Ok(())
}

/// Live non-parameters pass through; a destination parameter uses its exact
/// selected transport, never the first or latest register sharing a ValueId.
pub(crate) fn incoming_argument(
    function_index: usize,
    index: &FunctionIndex<'_>,
    successor: &SelectedSuccessor,
    destination: VirtualRegisterId,
) -> Result<VirtualRegisterId, LivenessError> {
    let mismatch = || LivenessError::FunctionMismatch {
        function: function_index,
    };
    let destination_register = index.unique_register(destination).ok_or_else(mismatch)?;
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
            let source = index.unique_register(argument).ok_or_else(mismatch)?;
            if source.scalar_type != destination_register.scalar_type
                || source.class != destination_register.class
                || source.definition_site.is_some()
            {
                return Err(mismatch());
            }
            let observed =
                case_payload::observed_source(index, successor, source).ok_or_else(mismatch)?;
            if !matches!(observed.origin, VirtualRegisterOrigin::StructuralObservation { place, byte_offset, .. }
                if case.source.place() == Some(place) && byte_offset == payload.semantic.field_byte_offset)
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
    let source = index.unique_register(argument).ok_or_else(mismatch)?;
    let value = match source.origin {
        VirtualRegisterOrigin::StructuralParameter { .. }
        | VirtualRegisterOrigin::SpillAddress { .. }
        | VirtualRegisterOrigin::StructuralObservation { .. }
        | VirtualRegisterOrigin::ScalarAbiAddress { .. }
        | VirtualRegisterOrigin::InstructionScratch { .. }
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
    index: &FunctionIndex<'_>,
    edge: &SelectedSuccessor,
    destination: &SelectedBlock,
) -> Result<(), LivenessError> {
    use selected_instructions::SelectedCasePayloadTransport;
    let mismatch = || LivenessError::FunctionMismatch {
        function: function_index,
    };
    let Some(case) = &edge.structural_case else {
        return Ok(());
    };
    if case.source.place().is_none()
        || case.case_tag < 0
        || (edge.role == SelectedSuccessorRole::EdgeTransferContinuation
            && (!case.cleanup_actions.is_empty()))
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
        if next_case.source != case.source
            || next_case.case != case.case
            || next_case.case_tag != case.case_tag
            || !next_case.cleanup_actions.is_empty()
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
    // A payload row is keyed by the destination parameter it defines. One
    // field may define several parameters, as when an arm forwards the same
    // payload binding twice; each row then carries its own observation.
    for payload in &case.payloads {
        if case
            .payloads
            .iter()
            .filter(|other| other.semantic.parameter.value == payload.semantic.parameter.value)
            .count()
            != 1
            || edge
                .bindings
                .iter()
                .any(|binding| binding.semantic.parameter == payload.semantic.parameter.value)
        {
            return Err(mismatch());
        }
        let destinations = index
            .parameters_of(edge.block)
            .filter(|register| {
                matches!(register.origin, VirtualRegisterOrigin::BlockParameter { source_value, .. }
                if source_value == payload.semantic.parameter.value)
            })
            .collect::<Vec<_>>();
        match payload.transport {
            SelectedCasePayloadTransport::Unused if destinations.is_empty() => {}
            SelectedCasePayloadTransport::Registers { parameter, .. }
                if edge.role == SelectedSuccessorRole::EdgeTransferContinuation
                    && destinations.len() == 1
                    && destinations[0].id == parameter =>
            {
                incoming_argument(function_index, index, edge, parameter)?;
            }
            _ => return Err(mismatch()),
        }
    }
    Ok(())
}

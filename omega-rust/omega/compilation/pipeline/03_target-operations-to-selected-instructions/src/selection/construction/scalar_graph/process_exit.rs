//! Realize an admitted external completion at its exact terminal edge.
use super::{
    Builder, IntegerSign, ScalarType, SelectedBlockId, SelectedInstructionKind,
    SelectedInstructionProvenance,
};
use crate::SelectedInstructionError;
use legalized_operations::{LegalizedScalarBlock, LegalizedScalarTerminator};
use selected_instructions::{
    SelectedBoundarySettlement, SelectedBoundarySettlementPayload, SelectedTerminator,
};

pub(super) fn build(
    function: usize,
    source_function: &legalized_operations::LegalizedScalarFunction,
    block: &LegalizedScalarBlock,
    block_id: SelectedBlockId,
    start: usize,
    builder: &mut Builder<'_>,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<Option<SelectedTerminator>, SelectedInstructionError> {
    let LegalizedScalarTerminator::ExternalCompletion {
        psi_edge,
        boundary,
        kind,
        arguments,
        binding,
        structural_arguments,
        fuel,
        effect,
        ownership,
        ..
    } = &block.terminator
    else {
        return Ok(None);
    };
    let [argument] = arguments.as_slice() else {
        return Err(SelectedInstructionError::custody());
    };
    let source = argument.source.source_value();
    let (_, input, _, scalar_type) = builder
        .resolve(source)
        .ok_or(SelectedInstructionError::custody())?;
    if *kind != semantic_vocabulary::ExternalCompletionKind::ProcessExit
        || binding.boundary != *boundary
        || !matches!(
            binding.execution,
            target_operations::BoundaryExecutionBinding::AdmittedProvider(_)
                | target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                    target_operations::CompilerBuiltinExecution::HostedExitProcessI32
                )
        )
        || !structural_arguments.is_empty()
        || !matches!(scalar_type, ScalarType::Integer(integer) if integer.bits() == 32 && integer.sign() == IntegerSign::Signed)
    {
        return Err(SelectedInstructionError::custody());
    }
    if let target_operations::BoundarySettlementRealization::NormalizedForeignCall(foreign) =
        &binding.realization
    {
        let target_operations::BoundaryExecutionBinding::AdmittedProvider(provider_execution) =
            binding.execution
        else {
            return Err(SelectedInstructionError::custody());
        };
        let call = legalized_operations::LegalizedNormalizedForeignCall {
            boundary: *boundary,
            provider_execution,
            binding: foreign.clone(),
            scalar_arguments: arguments.clone(),
            structural_arguments: Vec::new(),
            structural_indices: Vec::new(),
            result: target_operations::TargetBoundaryResult::Unit,
            callbacks: Vec::new(),
        };
        super::normalized_foreign::emit_invocation(
            function,
            source_function,
            &call,
            super::normalized_foreign::Invocation::ExternalCompletion {
                edge: *psi_edge,
                effect: *effect,
                ownership,
                fuel,
            },
            environment,
            builder,
        )?;
        // The selected provider promises external completion. If it violates
        // that premise and returns, trap physically; this is not a source Crash
        // outcome and cannot establish crash permission or a normal successor.
        builder.emit(
            SelectedInstructionKind::Crash,
            builder.constraints.keys.crash,
            &[],
            SelectedInstructionProvenance {
                edges: vec![*psi_edge],
                ..Default::default()
            },
        )?;
        return Ok(Some(SelectedTerminator::HostedExitProcess {
            instruction: builder
                .instructions
                .last()
                .cloned()
                .ok_or(SelectedInstructionError::custody())?,
            completion_edge: *psi_edge,
        }));
    }
    if !matches!(
        binding.realization,
        target_operations::BoundarySettlementRealization::Builtin(
            target_operations::BoundaryRealization::HostedExitProcessI32(_)
        )
    ) {
        return Err(SelectedInstructionError::custody());
    }
    builder
        .transport
        .settlements
        .push(SelectedBoundarySettlement {
            block: block_id,
            instruction_index: u32::try_from(
                builder
                    .instructions
                    .len()
                    .checked_sub(start)
                    .ok_or(SelectedInstructionError::custody())?,
            )
            .map_err(|_| SelectedInstructionError::custody())?,
            settlement: SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                edge: *psi_edge,
                execution: binding.execution,
                boundary: *boundary,
                source,
            },
        });
    builder.emit(
        SelectedInstructionKind::HostedExitProcessI32,
        builder
            .constraints
            .keys
            .hosted_exit_process_i32
            .ok_or(SelectedInstructionError::custody())?,
        &[input],
        SelectedInstructionProvenance {
            edges: vec![*psi_edge],
            values: vec![source],
            fuel: fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(Some(SelectedTerminator::HostedExitProcess {
        instruction: builder
            .instructions
            .last()
            .cloned()
            .ok_or(SelectedInstructionError::custody())?,
        completion_edge: *psi_edge,
    }))
}

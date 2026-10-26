//! An ordinary declared-ABI invocation with no receiving result or return edge.
//! The caller activation and outbound storage remain funded while its callee
//! executes. Physical ABI result definitions have no Terminal value identity.
use super::{
    Builder, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use crate::SelectedInstructionError;
use legalized_operations::{
    LegalizedScalarBlock, LegalizedScalarFunction, LegalizedScalarTerminator,
};
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedTerminator};

fn scratch(
    builder: &mut Builder<'_>,
    class: register_model::RegisterClassId,
    scalar_type: semantic_vocabulary::ScalarType,
    operand: usize,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let id = VirtualRegisterId(
        builder
            .registers
            .len()
            .try_into()
            .map_err(|_| SelectedInstructionError::custody())?,
    );
    builder.registers.push(VirtualRegister {
        id,
        class,
        scalar_type,
        origin: VirtualRegisterOrigin::InstructionScratch {
            instruction: SelectedInstructionId(
                builder
                    .instructions
                    .len()
                    .try_into()
                    .map_err(|_| SelectedInstructionError::custody())?,
            ),
            operand: operand
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    Ok(id)
}

pub(super) fn build(
    function: usize,
    source: &LegalizedScalarFunction,
    block: &LegalizedScalarBlock,
    builder: &mut Builder<'_>,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<SelectedTerminator, SelectedInstructionError> {
    let LegalizedScalarTerminator::InvokeNoNormal {
        psi_edge,
        call,
        declared_result,
        fuel,
        effect,
        ownership,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::custody());
    };
    let owner = target_operations::CallSiteOwner::InvokeNoNormal(*psi_edge);
    call.validate_no_normal_shape()
        .map_err(|_| SelectedInstructionError::custody())?;
    call.validate_source(ownership)
        .map_err(|_| SelectedInstructionError::custody())?;
    let key = crate::selection::scalar_call_abi::no_normal_key(call, declared_result, environment)
        .ok_or_else(|| {
            SelectedInstructionError::custody_because(
                "no-normal declared ABI has no unique canonical constraint in its result family",
            )
        })?;
    let row = super::row(builder.catalog, key)?;
    crate::selection::scalar_call_abi::validate(
        function,
        source,
        call,
        owner,
        key,
        row,
        environment,
    )?;
    let site = super::call_arguments::TerminalInvocation {
        edge: *psi_edge,
        call,
        ownership,
        fuel,
    };
    let mut operands = super::call_arguments::marshal(
        source,
        &site,
        call,
        builder,
        SelectedInstructionError::custody,
    )?;
    let integer = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .map_err(|_| SelectedInstructionError::custody())?,
    );
    let provenance = SelectedInstructionProvenance {
        edges: vec![*psi_edge],
        ..Default::default()
    };
    if let Some(placement) = &call.result_placement {
        if crate::selection::aggregate_result_input::indirect_result(
            placement,
            call.call_plan.policy,
        )
        .is_some()
        {
            // This storage is private ABI scratch, not a returned place/home.
            let slot = LocalStorageSlotId::InvocationResult { edge: *psi_edge };
            if builder
                .transport
                .local_slots
                .iter()
                .any(|existing| existing.id == slot)
            {
                return Err(SelectedInstructionError::custody());
            }
            builder
                .transport
                .local_slots
                .push(selected_instructions::SelectedLocalStorageSlot {
                    id: slot,
                    byte_size: placement.shape.byte_size,
                    alignment: placement.shape.alignment,
                });
            let class = builder.class;
            let pointer = scratch(builder, class, integer, 0)?;
            builder.emit_frame_address(
                FrameStorageSlotId::Local(slot),
                pointer,
                provenance.clone(),
            )?;
            operands.push(pointer);
        } else {
            let scalar_type = match declared_result {
                abstract_operations::AbstractFunctionResult::Scalar(result) => result.scalar_type,
                _ => integer,
            };
            for _ in &placement.locations {
                let position = operands.len();
                let definition = row
                    .operands
                    .get(position)
                    .ok_or_else(SelectedInstructionError::custody)?;
                if definition.access != register_model::RegisterOperandAccess::Def {
                    return Err(SelectedInstructionError::custody());
                }
                operands.push(scratch(builder, definition.class, scalar_type, position)?);
            }
        }
    }
    super::call_arguments::emit_call(
        builder,
        owner,
        call,
        *effect,
        ownership.clone(),
        SelectedInstructionKind::InvokeNoNormal {
            target: selected_instructions::SelectedCallTarget::from_native(call.target),
        },
        key,
        &operands,
        SelectedInstructionProvenance {
            edges: vec![*psi_edge],
            values: call
                .arguments
                .iter()
                .filter_map(|argument| argument.scalar_source())
                .collect(),
            obligations: call.requirement_obligations.clone(),
            fuel: fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(SelectedTerminator::InvokeNoNormal {
        instruction: builder
            .instructions
            .last()
            .cloned()
            .ok_or_else(SelectedInstructionError::custody)?,
        psi_edge: *psi_edge,
    })
}

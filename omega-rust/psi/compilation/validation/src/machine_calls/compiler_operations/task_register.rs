//! Check the private core signatures before allowing task-register instructions.
//! The generic declaration is checked before its executable application closes
//! the table extent; neither a matching name nor an arbitrary service suffices.

use super::descriptor_declarations::{
    array_loan, boundary_state, extent_matches_application, fixed_array_length,
    machine_control_service, plain_primitive,
};
use crate::CheckingInput;
use language_core::descriptor_tables::DescriptorTableOperation;
use language_semantics::{ReferenceAccess, ServiceReachId};
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::machine::Machine;
use typed_trees::types::PrimitiveType;

#[derive(Clone, Copy)]
pub(super) enum TaskRegisterDeclaration {
    Activate {
        extent: SymbolHandle,
        service: ServiceReachId,
    },
    Selector {
        service: ServiceReachId,
    },
}

impl TaskRegisterDeclaration {
    pub(super) const fn service(self) -> ServiceReachId {
        match self {
            Self::Activate { service, .. } | Self::Selector { service } => service,
        }
    }
}

pub(super) fn task_register_declaration(
    program: &dyn CheckingInput,
    machine: &Machine,
) -> Result<Option<TaskRegisterDeclaration>, &'static str> {
    let original = program
        .machine_specialization(machine.symbol)
        .map_or(machine.symbol, |application| application.template);
    if program.symbol_toolchain_relative_path(original)
        != Some(std::path::Path::new("task_register.omg"))
    {
        return Ok(None);
    }
    let identity = program
        .normalized_hermetic_symbol_identity(original)
        .map_err(|_| "task-register declaration has no exact toolchain identity")?;
    let activate = match identity.as_str() {
        "toolchain::activate_task_register" => true,
        "toolchain::read_task_register" => false,
        _ => return Ok(None),
    };
    let declaration = program
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == original)
        .ok_or("task-register application lost its original declaration")?;
    if declaration.is_public {
        return Err("task-register primitive declaration must be private");
    }
    let state = boundary_state(program, declaration)?;
    let service = machine_control_service(program, declaration)?;
    if machine_control_service(program, machine)? != service {
        return Err("task-register application changed its MachineControl service");
    }
    let parameters = program.state_parameters(state);
    let telescope = program
        .machine_specialization(declaration.symbol)
        .map(|application| {
            program
                .data_type_parameters
                .span_or_empty(application.template_parameters)
        })
        .unwrap_or_else(|| program.machine_type_parameters(declaration));
    if !activate {
        if !parameters.is_empty()
            || !telescope.is_empty()
            || !plain_primitive(program, state.return_type, PrimitiveType::U16)
        {
            return Err("task-register read requires no arguments and an unqualified u16 result");
        }
        return Ok(Some(TaskRegisterDeclaration::Selector { service }));
    }
    let [extent] = telescope else {
        return Err("task-register activation requires one const-u64 table extent");
    };
    let TypeParameterKind::Const { type_reference } = extent.kind else {
        return Err("task-register extent must be a const-u64 parameter");
    };
    if !plain_primitive(program, type_reference, PrimitiveType::U64) {
        return Err("task-register extent must be an unqualified u64");
    }
    let [gdt, task_state, selector, scratch] = parameters else {
        return Err(
            "task-register activation requires GDT, task-state, selector and scratch operands",
        );
    };
    if !gdt.is_mutable || task_state.is_mutable || selector.is_mutable || !scratch.is_mutable {
        return Err(
            "task-register activation requires mutable GDT and scratch operands and read-only task-state and selector operands",
        );
    }
    let (_, length) = array_loan(
        program,
        gdt.type_reference,
        ReferenceAccess::Mutable,
        PrimitiveType::U64,
    )?;
    if !extent_matches_application(program, declaration, extent.symbol, length)?
        || fixed_array_length(
            program,
            task_state.type_reference,
            ReferenceAccess::Shared,
            PrimitiveType::U8,
        )? != 104
        || fixed_array_length(
            program,
            scratch.type_reference,
            ReferenceAccess::Mutable,
            PrimitiveType::U8,
        )? != 10
        || !plain_primitive(program, selector.type_reference, PrimitiveType::U16)
        || !plain_primitive(program, state.return_type, PrimitiveType::U8)
    {
        return Err(
            "task-register declaration changed its table extent, 104 task bytes, ten scratch bytes, u16 selector or byte result",
        );
    }
    Ok(Some(TaskRegisterDeclaration::Activate {
        extent: extent.symbol,
        service,
    }))
}

/// Executable lowering consumes a closed application of the checked declaration.
pub(super) fn task_register_operation(
    program: &dyn CheckingInput,
    machine: &Machine,
) -> Result<Option<(DescriptorTableOperation, ServiceReachId)>, &'static str> {
    let Some(contract) = task_register_declaration(program, machine)? else {
        return Ok(None);
    };
    let state = boundary_state(program, machine)?;
    let parameters = program.state_parameters(state);
    let operation = match contract {
        TaskRegisterDeclaration::Selector { .. } => {
            if !parameters.is_empty()
                || !plain_primitive(program, state.return_type, PrimitiveType::U16)
            {
                return Err("task-register read application changed its signature");
            }
            DescriptorTableOperation::ReadTask
        }
        TaskRegisterDeclaration::Activate { extent, .. } => {
            let application = program
                .machine_specialization(machine.symbol)
                .ok_or("task-register activation requires a closed table-extent application")?;
            if !matches!(
                program.specialization_binding(application, extent),
                Some(typed_trees::static_applications::StaticMachineBindingValue::Const(_))
            ) {
                return Err("task-register activation lost its exact const-extent substitution");
            }
            let [gdt, task_state, selector, scratch] = parameters else {
                return Err("task-register application changed its four runtime operands");
            };
            if !gdt.is_mutable
                || task_state.is_mutable
                || selector.is_mutable
                || !scratch.is_mutable
            {
                return Err("task-register application changed its operand mutability");
            }
            let (_, length) = array_loan(
                program,
                gdt.type_reference,
                ReferenceAccess::Mutable,
                PrimitiveType::U64,
            )?;
            if !extent_matches_application(program, machine, extent, length)? {
                return Err("task-register GDT extent differs from its retained const argument");
            }
            let entries = fixed_array_length(
                program,
                gdt.type_reference,
                ReferenceAccess::Mutable,
                PrimitiveType::U64,
            )?;
            if !(2..=8192).contains(&entries)
                || fixed_array_length(
                    program,
                    task_state.type_reference,
                    ReferenceAccess::Shared,
                    PrimitiveType::U8,
                )? != 104
                || fixed_array_length(
                    program,
                    scratch.type_reference,
                    ReferenceAccess::Mutable,
                    PrimitiveType::U8,
                )? != 10
                || !plain_primitive(program, selector.type_reference, PrimitiveType::U16)
                || !plain_primitive(program, state.return_type, PrimitiveType::U8)
            {
                return Err(
                    "task-register application requires 2..8192 GDT words, 104 task bytes, ten scratch bytes, u16 selector and byte result",
                );
            }
            DescriptorTableOperation::ActivateTask {
                entries: entries as u16,
            }
        }
    };
    Ok(Some((operation, contract.service())))
}

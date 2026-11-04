//! Check the private core signatures before allowing task-register instructions.
//! The generic declaration is checked before its executable application closes
//! the table extent; neither a matching name nor an arbitrary service suffices.

use crate::CheckingInput;
use language_core::task_register::{TASK_REGISTER_CORE_SOURCE, TaskRegisterOperation};
use language_semantics::{MachineSupplyMode, ReferenceAccess, ServiceReachId};
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::machine::Machine;
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Copy)]
pub enum TaskRegisterDeclaration {
    Activate {
        extent: SymbolHandle,
        service: ServiceReachId,
    },
    Selector {
        service: ServiceReachId,
    },
}

impl TaskRegisterDeclaration {
    pub const fn service(self) -> ServiceReachId {
        match self {
            Self::Activate { service, .. } | Self::Selector { service } => service,
        }
    }
}

pub fn task_register_declaration(
    program: &dyn CheckingInput,
    machine: &Machine,
) -> Result<Option<TaskRegisterDeclaration>, &'static str> {
    let original = program
        .machine_specialization(machine.symbol)
        .map_or(machine.symbol, |application| application.template);
    if program.symbol_toolchain_relative_path(original)
        != Some(std::path::Path::new(TASK_REGISTER_CORE_SOURCE))
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
pub fn task_register_operation(
    program: &dyn CheckingInput,
    machine: &Machine,
) -> Result<Option<(TaskRegisterOperation, ServiceReachId)>, &'static str> {
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
            TaskRegisterOperation::Selector
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
            TaskRegisterOperation::Activate {
                entries: entries as u16,
            }
        }
    };
    Ok(Some((operation, contract.service())))
}

fn boundary_state<'a>(
    program: &'a dyn CheckingInput,
    machine: &'a Machine,
) -> Result<&'a typed_trees::state::State, &'static str> {
    if machine.body_is_present
        || machine.supply_mode != MachineSupplyMode::Boundary
        || machine.suspends
        || machine.blocks
        || !machine.lifetime_parameters.is_empty()
        || !machine.contracts.is_empty()
        || !machine.invokes.is_empty()
        || machine.spelling.is_some()
        || !program.machine_trait_conformances(machine).is_empty()
        || machine.target.is_some()
        || machine.authored_target.is_some()
        || machine.target_family_interface
        || !machine.conformance_bounds.is_empty()
        || machine.purpose != typed_trees::machine::MachinePurpose::Runtime
        || !machine.owned_data.is_empty()
        || machine.admitted_same_stack_bytes != 0
        || machine.service_reach_is_installation_bound
        || (program.machine_specialization(machine.symbol).is_none() && machine.is_public)
    {
        return Err("task-register primitive requires its private bodyless core declaration");
    }
    let [state] = program.machine_states(machine) else {
        return Err("task-register declaration must have one entry signature");
    };
    if !state.contracts.is_empty() {
        return Err("task-register primitive must not add entry contracts");
    }
    if program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.is_const || parameter.relevance.is_erased())
    {
        return Err("task-register primitive must retain every runtime operand");
    }
    Ok(state)
}

fn machine_control_service(
    program: &dyn CheckingInput,
    machine: &Machine,
) -> Result<ServiceReachId, &'static str> {
    let [service] = program
        .service_reach_rows
        .services(machine.service_reach_row)
    else {
        return Err("task-register primitive must reach exactly MachineControl");
    };
    let definition = program
        .service_reaches
        .definition(*service)
        .ok_or("task-register MachineControl service has no declaration")?;
    if program.symbol_toolchain_relative_path(definition.symbol)
        != Some(std::path::Path::new("assembly.omg"))
        || program
            .normalized_hermetic_symbol_identity(definition.symbol)
            .ok()
            .as_deref()
            != Some("toolchain::MachineControl")
    {
        return Err("task-register primitive must reach the exact core MachineControl service");
    }
    Ok(*service)
}

fn plain_primitive(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
    primitive: PrimitiveType,
) -> bool {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return false;
    };
    let expected = match primitive {
        PrimitiveType::U8 => symbols::BuiltinTypeAtom::U8,
        PrimitiveType::U16 => symbols::BuiltinTypeAtom::U16,
        PrimitiveType::U64 => symbols::BuiltinTypeAtom::U64,
        _ => return false,
    };
    program.symbols.builtin_type_atom(*symbol) == Some(expected)
}

fn array_loan(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
    access: ReferenceAccess,
    element: PrimitiveType,
) -> Result<(TypeReferenceHandle, &FixedArrayLength), &'static str> {
    let TypeReferenceNode::Reference {
        referee,
        access: actual,
        ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return Err("task-register storage must be an ordinary reference");
    };
    let TypeReferenceNode::FixedArray {
        element_type,
        length,
    } = program.type_reference_table.type_reference(*referee)
    else {
        return Err("task-register storage must be an unqualified fixed array");
    };
    if *actual != access || !plain_primitive(program, *element_type, element) {
        return Err("task-register storage has different element or access authority");
    }
    Ok((*referee, length))
}
fn fixed_array_length(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
    access: ReferenceAccess,
    element: PrimitiveType,
) -> Result<usize, &'static str> {
    let (array, _) = array_loan(program, reference, access, element)?;
    program
        .closed_fixed_array_length(array)
        .ok_or("task-register storage extent is not closed")
}

// First specialization may reuse the template symbol. Compare the current
// extent with its retained argument instead of assuming the template stayed open.
fn extent_matches_application(
    program: &dyn CheckingInput,
    machine: &Machine,
    extent: SymbolHandle,
    length: &FixedArrayLength,
) -> Result<bool, &'static str> {
    let substitution = if let Some(application) = program.machine_specialization(machine.symbol) {
        let Some(typed_trees::static_applications::StaticMachineBindingValue::Const(actual)) =
            program.specialization_binding(application, extent)
        else {
            return Err("task-register extent has no exact retained const argument");
        };
        Some((extent, *actual))
    } else {
        None
    };
    let expected = FixedArrayLength::ConstParameter {
        symbol: extent,
        name: Default::default(),
    };
    let expected = program.fixed_array_length_identity_with_simultaneous_substitutions(
        &expected,
        substitution.as_slice(),
    );
    let actual = program.fixed_array_length_identity(length, &[], &[]);
    Ok(expected.is_some() && expected == actual)
}

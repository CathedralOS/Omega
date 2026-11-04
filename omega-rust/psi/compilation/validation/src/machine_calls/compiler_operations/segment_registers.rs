//! Check each private selector operation against its exact core declaration.
use super::descriptor_declarations::{
    array_loan, boundary_state, extent_matches_application, fixed_array_length,
    machine_control_service, plain_primitive,
};
use crate::CheckingInput;
use language_core::descriptor_tables::{DescriptorTableOperation, SegmentRegister, SegmentReload};
use language_semantics::{ReferenceAccess, ServiceReachId};
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::machine::Machine;
use typed_trees::types::PrimitiveType;

#[derive(Clone, Copy)]
pub(super) enum SegmentDeclaration {
    Reload {
        extent: SymbolHandle,
        kind: SegmentReload,
        service: ServiceReachId,
    },
    Read {
        register: SegmentRegister,
        service: ServiceReachId,
    },
}
impl SegmentDeclaration {
    pub(super) const fn service(self) -> ServiceReachId {
        match self {
            Self::Reload { service, .. } | Self::Read { service, .. } => service,
        }
    }
}

#[derive(Clone, Copy)]
enum SelectorOperation {
    Reload(SegmentReload),
    Read(SegmentRegister),
}

pub(super) fn declaration(
    program: &dyn CheckingInput,
    machine: &Machine,
) -> Result<Option<SegmentDeclaration>, &'static str> {
    let original = program
        .machine_specialization(machine.symbol)
        .map_or(machine.symbol, |application| application.template);
    let path = program.symbol_toolchain_relative_path(original);
    let code = path == Some(std::path::Path::new("code_segment.omg"));
    let data = path == Some(std::path::Path::new("data_segment.omg"));
    if !code && !data {
        return Ok(None);
    }
    let identity = program
        .normalized_hermetic_symbol_identity(original)
        .map_err(|_| "selector operation has no exact toolchain identity")?;
    let operation = match (code, identity.as_str()) {
        (true, "toolchain::reload_code_segment") => SelectorOperation::Reload(SegmentReload::Code),
        (true, "toolchain::read_code_selector") => SelectorOperation::Read(SegmentRegister::Code),
        (false, "toolchain::reload_data_segments") => {
            SelectorOperation::Reload(SegmentReload::Data)
        }
        (false, "toolchain::read_data_selector") => SelectorOperation::Read(SegmentRegister::Data),
        (false, "toolchain::read_extra_selector") => {
            SelectorOperation::Read(SegmentRegister::Extra)
        }
        (false, "toolchain::read_stack_selector") => {
            SelectorOperation::Read(SegmentRegister::Stack)
        }
        (false, "toolchain::read_fs_selector") => SelectorOperation::Read(SegmentRegister::Fs),
        (false, "toolchain::read_gs_selector") => SelectorOperation::Read(SegmentRegister::Gs),
        _ => return Ok(None),
    };
    let original_machine = program
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == original)
        .ok_or("selector operation lost its original declaration")?;
    if original_machine.is_public {
        return Err("selector primitive must be private");
    }
    let state = boundary_state(program, original_machine)?;
    let service = machine_control_service(program, original_machine)?;
    if machine_control_service(program, machine)? != service {
        return Err("selector application changed its exact MachineControl service");
    }
    let telescope = program
        .machine_specialization(original_machine.symbol)
        .map(|application| {
            program
                .data_type_parameters
                .span_or_empty(application.template_parameters)
        })
        .unwrap_or_else(|| program.machine_type_parameters(original_machine));
    let kind = match operation {
        SelectorOperation::Read(register) => {
            if !telescope.is_empty()
                || !program.state_parameters(state).is_empty()
                || !plain_primitive(program, state.return_type, PrimitiveType::U16)
            {
                return Err("selector read requires no arguments and an unqualified u16 result");
            }
            return Ok(Some(SegmentDeclaration::Read { register, service }));
        }
        SelectorOperation::Reload(kind) => kind,
    };
    let [extent] = telescope else {
        return Err("selector reload requires one const-u64 table extent");
    };
    let TypeParameterKind::Const { type_reference } = extent.kind else {
        return Err("selector table extent must be a const-u64 parameter");
    };
    if !plain_primitive(program, type_reference, PrimitiveType::U64) {
        return Err("selector table extent must be an unqualified u64");
    }
    reload_signature(program, original_machine, extent.symbol)?;
    Ok(Some(SegmentDeclaration::Reload {
        extent: extent.symbol,
        kind,
        service,
    }))
}

pub(super) fn operation(
    program: &dyn CheckingInput,
    machine: &Machine,
) -> Result<Option<(DescriptorTableOperation, ServiceReachId)>, &'static str> {
    let Some(contract) = declaration(program, machine)? else {
        return Ok(None);
    };
    let operation = match contract {
        SegmentDeclaration::Read { register, .. } => {
            let state = boundary_state(program, machine)?;
            if !program.state_parameters(state).is_empty()
                || !plain_primitive(program, state.return_type, PrimitiveType::U16)
            {
                return Err("selector read application changed its checked signature");
            }
            DescriptorTableOperation::ReadSegment { register }
        }
        SegmentDeclaration::Reload { extent, kind, .. } => {
            let application = program
                .machine_specialization(machine.symbol)
                .ok_or("selector reload requires a closed table-extent application")?;
            if !matches!(
                program.specialization_binding(application, extent),
                Some(typed_trees::static_applications::StaticMachineBindingValue::Const(_))
            ) {
                return Err("selector reload lost its exact const-extent substitution");
            }
            reload_signature(program, machine, extent)?;
            let state = boundary_state(program, machine)?;
            let gdt = &program.state_parameters(state)[0];
            let entries = fixed_array_length(
                program,
                gdt.type_reference,
                ReferenceAccess::Mutable,
                PrimitiveType::U64,
            )?;
            if !(1..=8192).contains(&entries) {
                return Err("selector reload requires 1..8192 GDT words");
            }
            DescriptorTableOperation::ReloadSegments {
                entries: entries as u16,
                kind,
            }
        }
    };
    Ok(Some((operation, contract.service())))
}

fn reload_signature(
    program: &dyn CheckingInput,
    machine: &Machine,
    extent: SymbolHandle,
) -> Result<(), &'static str> {
    let state = boundary_state(program, machine)?;
    let [gdt, selector, scratch] = program.state_parameters(state) else {
        return Err("selector reload requires GDT, selector and scratch operands");
    };
    if !gdt.is_mutable || selector.is_mutable || !scratch.is_mutable {
        return Err("selector reload requires mutable table/scratch and a read-only selector");
    }
    let (_, length) = array_loan(
        program,
        gdt.type_reference,
        ReferenceAccess::Mutable,
        PrimitiveType::U64,
    )?;
    if !extent_matches_application(program, machine, extent, length)?
        || !plain_primitive(program, selector.type_reference, PrimitiveType::U16)
        || !plain_primitive(program, state.return_type, PrimitiveType::U8)
        || fixed_array_length(
            program,
            scratch.type_reference,
            ReferenceAccess::Mutable,
            PrimitiveType::U8,
        )? != 10
    {
        return Err(
            "selector reload changed its table extent, u16 selector, ten scratch bytes or byte result",
        );
    }
    Ok(())
}

//! Independent catalog, fixed-view, and transfer-geometry replay.

use crate::selection::{constraints::row, shared::*};

pub(super) fn replay(
    selected: &SelectedProjectedStructuralCallReturn,
    selection: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let call_key = selection
        .projected_structural_call
        .ok_or(SelectedInstructionError::MissingProjectedStructuralCallConstraint)?;
    validate_call(selected, call_key, physical, catalog)?;
    validate_return(
        &selected.caller_return,
        selected,
        SelectedStructuralFragmentSite::CallerFunctionResult,
        selection.keys.return_i64,
        physical,
        catalog,
    )?;
    validate_return(
        &selected.callee_return,
        selected,
        SelectedStructuralFragmentSite::CalleeFunctionResult,
        selection.keys.return_i64,
        physical,
        catalog,
    )?;
    validate_transfer(
        &selected.caller_argument_transfer,
        selected,
        SelectedStructuralFragmentSite::CallerArgumentSource,
        SelectedStructuralFragmentSite::CallerArgumentDestination,
        selection.keys.copy_i64,
        physical,
        catalog,
    )?;
    validate_transfer(
        &selected.callee_return_transfer,
        selected,
        SelectedStructuralFragmentSite::CalleeReturnSource,
        SelectedStructuralFragmentSite::CalleeFunctionResult,
        selection.keys.copy_i64,
        physical,
        catalog,
    )?;
    validate_transfer(
        &selected.caller_return_transfer,
        selected,
        SelectedStructuralFragmentSite::CallerOperationResult,
        SelectedStructuralFragmentSite::CallerFunctionResult,
        selection.keys.copy_i64,
        physical,
        catalog,
    )
}

fn validate_call(
    selected: &SelectedProjectedStructuralCallReturn,
    key: RegisterConstraintKey,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let constraint = row(catalog, key)?;
    let argument_view = fragment_view(
        selected,
        SelectedStructuralFragmentSite::CallerArgumentDestination,
        physical,
    )?;
    let result_view = fragment_view(
        selected,
        SelectedStructuralFragmentSite::CallerOperationResult,
        physical,
    )?;
    if selected.call.key != key
        || !fixed_matches(
            &selected.call.argument,
            constraint,
            RegisterOperandAccess::Use,
            argument_view,
        )
        || !fixed_matches(
            &selected.call.result,
            constraint,
            RegisterOperandAccess::Def,
            result_view,
        )
        || selected.call.implicit_uses != constraint.implicit_uses
        || selected.call.implicit_defs != constraint.implicit_defs
        || selected.call.clobbers != constraint.clobbers
    {
        return Err(SelectedInstructionError::ProjectedStructuralCatalogMismatch);
    }
    Ok(())
}

fn validate_return(
    selected_return: &SelectedStructuralReturnConstraint,
    selected: &SelectedProjectedStructuralCallReturn,
    site: SelectedStructuralFragmentSite,
    key: RegisterConstraintKey,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let constraint = row(catalog, key)?;
    let view = fragment_view(selected, site, physical)?;
    if constraint.operands.len() != 1
        || selected_return.key != key
        || !fixed_matches(
            &selected_return.value,
            constraint,
            RegisterOperandAccess::Use,
            view,
        )
        || selected_return.implicit_uses != constraint.implicit_uses
        || selected_return.implicit_defs != constraint.implicit_defs
        || selected_return.clobbers != constraint.clobbers
    {
        return Err(SelectedInstructionError::ProjectedStructuralCatalogMismatch);
    }
    Ok(())
}

fn validate_transfer(
    transfer: &SelectedStructuralTransfer,
    selected: &SelectedProjectedStructuralCallReturn,
    source_site: SelectedStructuralFragmentSite,
    destination_site: SelectedStructuralFragmentSite,
    copy_key: RegisterConstraintKey,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let source = fragment_register(selected, source_site)?;
    let destination = fragment_register(selected, destination_site)?;
    match transfer {
        SelectedStructuralTransfer::SameViewNoCopy { register }
            if source == destination && *register == source =>
        {
            Ok(())
        }
        SelectedStructuralTransfer::FixedViewCopy {
            source: retained_source,
            destination: retained_destination,
            constraint,
        } if source != destination
            && *retained_source == source
            && *retained_destination == destination =>
        {
            validate_copy(constraint, copy_key, source, destination, physical, catalog)
        }
        _ => Err(SelectedInstructionError::ProjectedStructuralCatalogMismatch),
    }
}

fn validate_copy(
    selected: &SelectedStructuralCopyConstraint,
    key: RegisterConstraintKey,
    source: MachineRegister,
    destination: MachineRegister,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let constraint = row(catalog, key)?;
    let [source_operand, destination_operand] = constraint.operands.as_slice() else {
        return Err(SelectedInstructionError::ProjectedStructuralCatalogMismatch);
    };
    let source_view = register_view(physical, source)?;
    let destination_view = register_view(physical, destination)?;
    if selected.key != key
        || !copy_matches(&selected.source, source_operand, source_view)
        || !copy_matches(&selected.destination, destination_operand, destination_view)
        || selected.implicit_uses != constraint.implicit_uses
        || selected.implicit_defs != constraint.implicit_defs
        || selected.clobbers != constraint.clobbers
    {
        return Err(SelectedInstructionError::ProjectedStructuralCatalogMismatch);
    }
    Ok(())
}

fn fixed_matches(
    selected: &SelectedStructuralFixedOperand,
    row: &RegisterInstructionConstraint,
    access: RegisterOperandAccess,
    view: RegisterViewId,
) -> bool {
    row.operands
        .iter()
        .filter(|operand| operand.access == access && operand.fixed_view == Some(view))
        .count()
        == 1
        && row.operands.iter().any(|operand| {
            operand.operand == selected.operand
                && operand.access == selected.access
                && operand.class == selected.class
                && operand.fixed_view == Some(selected.fixed_view)
                && selected.access == access
                && selected.fixed_view == view
                && operand.tied_to.is_none()
                && !operand.early_clobber
        })
}

fn copy_matches(
    selected: &SelectedStructuralCopyOperand,
    row: &register_model::RegisterOperandConstraint,
    view: RegisterViewId,
) -> bool {
    selected.operand == row.operand
        && selected.access == row.access
        && selected.class == row.class
        && selected.row_fixed_view == row.fixed_view
        && selected.selected_view == view
        && selected.tied_to == row.tied_to
        && selected.early_clobber == row.early_clobber
        && row.fixed_view.is_none_or(|fixed| fixed == view)
}

fn fragment_view(
    selected: &SelectedProjectedStructuralCallReturn,
    site: SelectedStructuralFragmentSite,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<RegisterViewId, SelectedInstructionError> {
    register_view(physical, fragment_register(selected, site)?)
}

fn fragment_register(
    selected: &SelectedProjectedStructuralCallReturn,
    site: SelectedStructuralFragmentSite,
) -> Result<MachineRegister, SelectedInstructionError> {
    let Some(fragment) = selected
        .fragments
        .iter()
        .find(|fragment| fragment.site == site)
    else {
        return Err(SelectedInstructionError::ProjectedStructuralConstraintMismatch { site });
    };
    let [ValueLocation::Register { register, .. }] = fragment.placement.locations.as_slice() else {
        return Err(SelectedInstructionError::ProjectedStructuralConstraintMismatch { site });
    };
    Ok(*register)
}

fn register_view(
    physical: &ValidatedPhysicalRegisterModel,
    register: MachineRegister,
) -> Result<RegisterViewId, SelectedInstructionError> {
    let name = match register {
        MachineRegister::X86Rax => "rax".into(),
        MachineRegister::X86Rcx => "rcx".into(),
        MachineRegister::X86Rdx => "rdx".into(),
        MachineRegister::X86Rbx => "rbx".into(),
        MachineRegister::X86Rsp => "rsp".into(),
        MachineRegister::X86Rbp => "rbp".into(),
        MachineRegister::X86Rsi => "rsi".into(),
        MachineRegister::X86Rdi => "rdi".into(),
        MachineRegister::X86R8 => "r8".into(),
        MachineRegister::X86R9 => "r9".into(),
        MachineRegister::X86R10 => "r10".into(),
        MachineRegister::X86R11 => "r11".into(),
        MachineRegister::X86R12 => "r12".into(),
        MachineRegister::X86R13 => "r13".into(),
        MachineRegister::X86R14 => "r14".into(),
        MachineRegister::X86R15 => "r15".into(),
        MachineRegister::X86Xmm(index) => format!("xmm{index}"),
        MachineRegister::Aarch64X(index) => format!("x{index}"),
        MachineRegister::Aarch64V(index) => format!("v{index}"),
    };
    physical
        .model()
        .view_named(&name)
        .map(|view| view.id)
        .ok_or(SelectedInstructionError::ProjectedStructuralCatalogMismatch)
}

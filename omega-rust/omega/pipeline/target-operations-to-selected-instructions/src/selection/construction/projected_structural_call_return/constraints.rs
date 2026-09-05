//! Target constraint-row projection for the atomic structural closure.

use crate::selection::{constraints::row, shared::*};

pub(super) fn call(
    fragments: &[SelectedStructuralFragmentConstraint],
    selection: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedStructuralCallConstraint, SelectedInstructionError> {
    let key = selection
        .projected_structural_call
        .ok_or(SelectedInstructionError::MissingProjectedStructuralCallConstraint)?;
    let constraint = row(catalog, key)?;
    let argument_view = view_for(
        physical,
        fragment_register(
            fragments,
            SelectedStructuralFragmentSite::CallerArgumentDestination,
        )?,
    )?;
    let result_view = view_for(
        physical,
        fragment_register(
            fragments,
            SelectedStructuralFragmentSite::CallerOperationResult,
        )?,
    )?;
    Ok(SelectedStructuralCallConstraint {
        key,
        argument: unique_fixed(constraint, RegisterOperandAccess::Use, argument_view)?,
        result: unique_fixed(constraint, RegisterOperandAccess::Def, result_view)?,
        implicit_uses: constraint.implicit_uses.clone(),
        implicit_defs: constraint.implicit_defs.clone(),
        clobbers: constraint.clobbers.clone(),
    })
}

pub(super) fn return_constraint(
    fragments: &[SelectedStructuralFragmentConstraint],
    site: SelectedStructuralFragmentSite,
    key: RegisterConstraintKey,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedStructuralReturnConstraint, SelectedInstructionError> {
    let constraint = row(catalog, key)?;
    let view = view_for(physical, fragment_register(fragments, site)?)?;
    if constraint.operands.len() != 1 {
        return Err(SelectedInstructionError::ProjectedStructuralCatalogMismatch);
    }
    Ok(SelectedStructuralReturnConstraint {
        key,
        value: unique_fixed(constraint, RegisterOperandAccess::Use, view)?,
        implicit_uses: constraint.implicit_uses.clone(),
        implicit_defs: constraint.implicit_defs.clone(),
        clobbers: constraint.clobbers.clone(),
    })
}

pub(super) fn copy_constraint(
    key: RegisterConstraintKey,
    source: MachineRegister,
    destination: MachineRegister,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedStructuralCopyConstraint, SelectedInstructionError> {
    let constraint = row(catalog, key)?;
    let [source_operand, destination_operand] = constraint.operands.as_slice() else {
        return Err(SelectedInstructionError::ProjectedStructuralCatalogMismatch);
    };
    if source_operand.access != RegisterOperandAccess::Use
        || destination_operand.access != RegisterOperandAccess::Def
    {
        return Err(SelectedInstructionError::ProjectedStructuralCatalogMismatch);
    }
    let source_view = view_for(physical, source)?;
    let destination_view = view_for(physical, destination)?;
    if source_operand.class != physical_view(physical, source_view)?.class
        || destination_operand.class != physical_view(physical, destination_view)?.class
        || source_operand
            .fixed_view
            .is_some_and(|fixed| fixed != source_view)
        || destination_operand
            .fixed_view
            .is_some_and(|fixed| fixed != destination_view)
    {
        return Err(SelectedInstructionError::ProjectedStructuralCatalogMismatch);
    }
    Ok(SelectedStructuralCopyConstraint {
        key,
        source: copy_operand(source_operand, source_view),
        destination: copy_operand(destination_operand, destination_view),
        implicit_uses: constraint.implicit_uses.clone(),
        implicit_defs: constraint.implicit_defs.clone(),
        clobbers: constraint.clobbers.clone(),
    })
}

fn unique_fixed(
    row: &RegisterInstructionConstraint,
    access: RegisterOperandAccess,
    view: RegisterViewId,
) -> Result<SelectedStructuralFixedOperand, SelectedInstructionError> {
    let matches = row
        .operands
        .iter()
        .filter(|operand| operand.access == access && operand.fixed_view == Some(view))
        .collect::<Vec<_>>();
    let [operand] = matches.as_slice() else {
        return Err(SelectedInstructionError::ProjectedStructuralCatalogMismatch);
    };
    if operand.tied_to.is_some() || operand.early_clobber {
        return Err(SelectedInstructionError::ProjectedStructuralCatalogMismatch);
    }
    Ok(SelectedStructuralFixedOperand {
        operand: operand.operand,
        access: operand.access,
        class: operand.class,
        fixed_view: view,
    })
}

fn copy_operand(
    operand: &register_model::RegisterOperandConstraint,
    selected_view: RegisterViewId,
) -> SelectedStructuralCopyOperand {
    SelectedStructuralCopyOperand {
        operand: operand.operand,
        access: operand.access,
        class: operand.class,
        row_fixed_view: operand.fixed_view,
        selected_view,
        tied_to: operand.tied_to,
        early_clobber: operand.early_clobber,
    }
}

pub(super) fn fragment_register(
    fragments: &[SelectedStructuralFragmentConstraint],
    site: SelectedStructuralFragmentSite,
) -> Result<MachineRegister, SelectedInstructionError> {
    let Some(fragment) = fragments.iter().find(|fragment| fragment.site == site) else {
        return Err(SelectedInstructionError::ProjectedStructuralConstraintMismatch { site });
    };
    let [ValueLocation::Register { register, .. }] = fragment.placement.locations.as_slice() else {
        return Err(SelectedInstructionError::ProjectedStructuralConstraintMismatch { site });
    };
    Ok(*register)
}

fn view_for(
    physical: &ValidatedPhysicalRegisterModel,
    register: MachineRegister,
) -> Result<RegisterViewId, SelectedInstructionError> {
    let name = match register {
        MachineRegister::X86Rax => "rax",
        MachineRegister::X86Rcx => "rcx",
        MachineRegister::X86Rdx => "rdx",
        MachineRegister::X86Rbx => "rbx",
        MachineRegister::X86Rsp => "rsp",
        MachineRegister::X86Rbp => "rbp",
        MachineRegister::X86Rsi => "rsi",
        MachineRegister::X86Rdi => "rdi",
        MachineRegister::X86R8 => "r8",
        MachineRegister::X86R9 => "r9",
        MachineRegister::X86R10 => "r10",
        MachineRegister::X86R11 => "r11",
        MachineRegister::X86R12 => "r12",
        MachineRegister::X86R13 => "r13",
        MachineRegister::X86R14 => "r14",
        MachineRegister::X86R15 => "r15",
        MachineRegister::Aarch64X(index) => return numbered_view(physical, "x", index),
        MachineRegister::X86Xmm(index) => return numbered_view(physical, "xmm", index),
        MachineRegister::Aarch64V(index) => return numbered_view(physical, "v", index),
    };
    physical
        .model()
        .view_named(name)
        .map(|view| view.id)
        .ok_or(SelectedInstructionError::ProjectedStructuralCatalogMismatch)
}

fn numbered_view(
    physical: &ValidatedPhysicalRegisterModel,
    prefix: &str,
    index: u8,
) -> Result<RegisterViewId, SelectedInstructionError> {
    physical
        .model()
        .view_named(&format!("{prefix}{index}"))
        .map(|view| view.id)
        .ok_or(SelectedInstructionError::ProjectedStructuralCatalogMismatch)
}

fn physical_view(
    physical: &ValidatedPhysicalRegisterModel,
    id: RegisterViewId,
) -> Result<&register_model::RegisterView, SelectedInstructionError> {
    physical
        .model()
        .views
        .iter()
        .find(|view| view.id == id)
        .ok_or(SelectedInstructionError::ProjectedStructuralCatalogMismatch)
}

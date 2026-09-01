//! Canonical v13 identity encoding for atomic projected structural selections.

use crate::selection::shared::*;

pub(super) fn encode(bytes: &mut Vec<u8>, rows: &[SelectedProjectedStructuralCallReturn]) {
    super::encode_len(bytes, rows.len());
    for row in rows {
        bytes.push(match row.recipe {
            SelectedProjectedStructuralCallReturnRecipe::OwnedLinearIntegerFragmentV1 => 1,
        });
        bytes.extend_from_slice(&row.legalized_plan.bytes());
        bytes.extend_from_slice(&row.caller.get().to_le_bytes());
        bytes.extend_from_slice(&row.callee.get().to_le_bytes());
        super::encode_len(bytes, row.projected_qualifications.len());
        for qualification in &row.projected_qualifications {
            super::encode_len(bytes, qualification.path.len());
            for segment in &qualification.path {
                match segment {
                    psi_terminal::StructuralPathSegment::Field(name) => {
                        bytes.push(1);
                        super::encode_len(bytes, name.len());
                        bytes.extend_from_slice(name.as_bytes());
                    }
                    psi_terminal::StructuralPathSegment::FixedIndex(index) => {
                        bytes.push(2);
                        bytes.extend_from_slice(&index.to_le_bytes());
                    }
                }
            }
            bytes.extend_from_slice(&qualification.domain.get().to_le_bytes());
        }
        super::encode_len(bytes, row.fragments.len());
        for fragment in &row.fragments {
            bytes.push(site_tag(fragment.site));
            encode_placement(bytes, &fragment.placement);
        }
        encode_call(bytes, &row.call);
        encode_return(bytes, &row.caller_return);
        encode_return(bytes, &row.callee_return);
        encode_transfer(bytes, &row.caller_argument_transfer);
        encode_transfer(bytes, &row.callee_return_transfer);
        encode_transfer(bytes, &row.caller_return_transfer);
    }
}

fn encode_call(bytes: &mut Vec<u8>, row: &SelectedStructuralCallConstraint) {
    super::primitives::encode_constraint_key(bytes, row.key);
    encode_fixed(bytes, row.argument);
    encode_fixed(bytes, row.result);
    encode_effects(bytes, &row.implicit_uses, &row.implicit_defs, &row.clobbers);
}

fn encode_return(bytes: &mut Vec<u8>, row: &SelectedStructuralReturnConstraint) {
    super::encode_constraint_key(bytes, row.key);
    encode_fixed(bytes, row.value);
    encode_effects(bytes, &row.implicit_uses, &row.implicit_defs, &row.clobbers);
}

fn encode_transfer(bytes: &mut Vec<u8>, transfer: &SelectedStructuralTransfer) {
    match transfer {
        SelectedStructuralTransfer::SameViewNoCopy { register } => {
            bytes.push(1);
            super::encode_machine_register(bytes, *register);
        }
        SelectedStructuralTransfer::FixedViewCopy {
            source,
            destination,
            constraint,
        } => {
            bytes.push(2);
            super::encode_machine_register(bytes, *source);
            super::encode_machine_register(bytes, *destination);
            super::primitives::encode_constraint_key(bytes, constraint.key);
            encode_copy_operand(bytes, constraint.source);
            encode_copy_operand(bytes, constraint.destination);
            encode_effects(
                bytes,
                &constraint.implicit_uses,
                &constraint.implicit_defs,
                &constraint.clobbers,
            );
        }
    }
}

fn encode_fixed(bytes: &mut Vec<u8>, operand: SelectedStructuralFixedOperand) {
    bytes.extend_from_slice(&operand.operand.to_le_bytes());
    encode_access(bytes, operand.access);
    bytes.extend_from_slice(&operand.class.0.to_le_bytes());
    bytes.extend_from_slice(&operand.fixed_view.0.to_le_bytes());
}

fn encode_copy_operand(bytes: &mut Vec<u8>, operand: SelectedStructuralCopyOperand) {
    bytes.extend_from_slice(&operand.operand.to_le_bytes());
    encode_access(bytes, operand.access);
    bytes.extend_from_slice(&operand.class.0.to_le_bytes());
    super::encode_option_u16(bytes, operand.row_fixed_view.map(|view| view.0));
    bytes.extend_from_slice(&operand.selected_view.0.to_le_bytes());
    super::encode_option_u16(bytes, operand.tied_to);
    bytes.push(u8::from(operand.early_clobber));
}

fn encode_effects(
    bytes: &mut Vec<u8>,
    uses: &[omega_register_model::RegisterUnitId],
    defs: &[omega_register_model::RegisterUnitId],
    clobbers: &[omega_register_model::RegisterUnitId],
) {
    super::encode_u16s(bytes, uses.iter().map(|unit| unit.0));
    super::encode_u16s(bytes, defs.iter().map(|unit| unit.0));
    super::encode_u16s(bytes, clobbers.iter().map(|unit| unit.0));
}

fn encode_placement(bytes: &mut Vec<u8>, placement: &omega_calling_conventions::ValuePlacement) {
    bytes.extend_from_slice(&placement.shape.byte_size.to_le_bytes());
    bytes.extend_from_slice(&placement.shape.alignment.to_le_bytes());
    match placement.shape.class {
        omega_calling_conventions::ValueClass::Integer => bytes.push(1),
        omega_calling_conventions::ValueClass::Float => bytes.push(2),
        omega_calling_conventions::ValueClass::HomogeneousFloatAggregate { members } => {
            bytes.push(3);
            bytes.push(members);
        }
        omega_calling_conventions::ValueClass::SystemVAggregate { first, second } => {
            bytes.push(4);
            encode_eightbyte_class(bytes, first);
            encode_eightbyte_class(bytes, second);
        }
    }
    super::encode_len(bytes, placement.locations.len());
    for location in &placement.locations {
        match location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                bytes.push(1);
                super::encode_machine_register(bytes, *register);
                bytes.extend_from_slice(&value_byte_offset.to_le_bytes());
                bytes.extend_from_slice(&byte_size.to_le_bytes());
            }
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(2);
                bytes.extend_from_slice(&stack_byte_offset.to_le_bytes());
                bytes.extend_from_slice(&value_byte_offset.to_le_bytes());
                bytes.extend_from_slice(&byte_size.to_le_bytes());
                bytes.extend_from_slice(&alignment.to_le_bytes());
            }
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                bytes.push(3);
                match pointer {
                    omega_calling_conventions::IndirectPointerLocation::Register(register) => {
                        bytes.push(1);
                        super::encode_machine_register(bytes, *register);
                    }
                    omega_calling_conventions::IndirectPointerLocation::Stack {
                        stack_byte_offset,
                        alignment,
                    } => {
                        bytes.push(2);
                        bytes.extend_from_slice(&stack_byte_offset.to_le_bytes());
                        bytes.extend_from_slice(&alignment.to_le_bytes());
                    }
                }
                match copy_stack_byte_offset {
                    Some(offset) => {
                        bytes.push(1);
                        bytes.extend_from_slice(&offset.to_le_bytes());
                    }
                    None => bytes.push(0),
                }
                bytes.extend_from_slice(&byte_size.to_le_bytes());
                bytes.extend_from_slice(&alignment.to_le_bytes());
            }
        }
    }
}

fn encode_eightbyte_class(
    bytes: &mut Vec<u8>,
    class: omega_calling_conventions::SystemVEightbyteClass,
) {
    bytes.push(match class {
        omega_calling_conventions::SystemVEightbyteClass::Integer => 1,
        omega_calling_conventions::SystemVEightbyteClass::Sse => 2,
    });
}

fn encode_access(bytes: &mut Vec<u8>, access: RegisterOperandAccess) {
    bytes.push(match access {
        RegisterOperandAccess::Use => 1,
        RegisterOperandAccess::Def => 2,
        RegisterOperandAccess::UseDef => 3,
    });
}

fn site_tag(site: SelectedStructuralFragmentSite) -> u8 {
    match site {
        SelectedStructuralFragmentSite::CallerParameter => 1,
        SelectedStructuralFragmentSite::CallerArgumentSource => 2,
        SelectedStructuralFragmentSite::CallerArgumentDestination => 3,
        SelectedStructuralFragmentSite::CallerOperationResult => 4,
        SelectedStructuralFragmentSite::CallerFunctionResult => 5,
        SelectedStructuralFragmentSite::CalleeParameter => 6,
        SelectedStructuralFragmentSite::CalleeReturnSource => 7,
        SelectedStructuralFragmentSite::CalleeFunctionResult => 8,
    }
}

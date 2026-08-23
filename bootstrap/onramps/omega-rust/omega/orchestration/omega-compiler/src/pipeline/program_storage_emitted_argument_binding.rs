//! Receiver-free installed authority bound to one exact emitted wrapper.
//!
//! The earlier whole-root argument carrier proves authority, source
//! declaration, and address-free continuation ABI agreement. This transition
//! additionally requires final-image wrapper evidence and binds those same
//! placements to its exact physical-arrival rows. It still does not prove that
//! firmware supplied the roots, invoked the wrapper, or executed native code.

use super::{
    ProgramLocalStorageCustody, ProgramLocalStorageCustodyError, ProgramStorageEntryDiagnostic,
    ProgramStorageEntryEmittedWrapperEvidence, ProgramStorageEntryNativeBridgePlan,
    ProgramStorageEntryRootRole, ProgramStorageEntryWholeRootArgumentCarrier,
};
/// Non-cloneable installed authority plus the exact final wrapper certificate
/// that is capable of forwarding its two ordinary values to the selected
/// source continuation.
#[derive(Debug)]
pub struct ProgramStorageEntryEmittedWholeRootArgumentCarrier {
    arguments: ProgramStorageEntryWholeRootArgumentCarrier,
    emitted_wrapper: ProgramStorageEntryEmittedWrapperEvidence,
}

impl<'root, 'code>
    ProgramLocalStorageCustody<'root, 'code, ProgramStorageEntryEmittedWholeRootArgumentCarrier>
{
    /// Return to the argument stage after the emitted-wrapper check without
    /// separating the local account owner.
    pub fn into_arguments(
        self,
    ) -> ProgramLocalStorageCustody<'root, 'code, ProgramStorageEntryWholeRootArgumentCarrier> {
        let (emitted, registry) = self.into_parts();
        ProgramLocalStorageCustody::new(emitted.into_arguments(), registry)
    }
}

impl ProgramStorageEntryEmittedWholeRootArgumentCarrier {
    pub const fn arguments(&self) -> &ProgramStorageEntryWholeRootArgumentCarrier {
        &self.arguments
    }

    pub const fn emitted_wrapper(&self) -> &ProgramStorageEntryEmittedWrapperEvidence {
        &self.emitted_wrapper
    }

    pub fn into_arguments(self) -> ProgramStorageEntryWholeRootArgumentCarrier {
        self.arguments
    }
}

/// Bind an authority-bearing receiver-free argument carrier to final emitted
/// wrapper evidence. Rejection returns the intact authority carrier.
pub fn bind_program_storage_entry_emitted_whole_root_arguments(
    arguments: ProgramStorageEntryWholeRootArgumentCarrier,
    bridge: &ProgramStorageEntryNativeBridgePlan,
) -> Result<
    ProgramStorageEntryEmittedWholeRootArgumentCarrier,
    ProgramStorageEntryEmittedWholeRootArgumentError,
> {
    match validate_emitted_argument_binding(&arguments, bridge) {
        Ok(emitted_wrapper) => Ok(ProgramStorageEntryEmittedWholeRootArgumentCarrier {
            arguments,
            emitted_wrapper,
        }),
        Err(diagnostic) => Err(ProgramStorageEntryEmittedWholeRootArgumentError {
            arguments,
            diagnostic,
        }),
    }
}

pub fn bind_program_local_storage_entry_emitted_whole_root_arguments<'root, 'code>(
    custody: ProgramLocalStorageCustody<'root, 'code, ProgramStorageEntryWholeRootArgumentCarrier>,
    bridge: &ProgramStorageEntryNativeBridgePlan,
) -> Result<
    ProgramLocalStorageCustody<'root, 'code, ProgramStorageEntryEmittedWholeRootArgumentCarrier>,
    ProgramLocalStorageCustodyError<'root, 'code, ProgramStorageEntryWholeRootArgumentCarrier>,
> {
    let (arguments, registry) = custody.into_parts();
    match bind_program_storage_entry_emitted_whole_root_arguments(arguments, bridge) {
        Ok(emitted) => Ok(ProgramLocalStorageCustody::new(emitted, registry)),
        Err(error) => {
            let diagnostic = error.diagnostic().clone();
            Err(ProgramLocalStorageCustodyError::new(
                ProgramLocalStorageCustody::new(error.into_arguments(), registry),
                diagnostic,
            ))
        }
    }
}

fn validate_emitted_argument_binding(
    arguments: &ProgramStorageEntryWholeRootArgumentCarrier,
    bridge: &ProgramStorageEntryNativeBridgePlan,
) -> Result<ProgramStorageEntryEmittedWrapperEvidence, ProgramStorageEntryDiagnostic> {
    if arguments.authority().binding() != bridge.binding() {
        return Err(ProgramStorageEntryDiagnostic(
            "whole-root arguments do not belong to this exact emitted bridge binding".into(),
        ));
    }
    let emitted = bridge.emitted_wrapper_evidence().ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "whole-root arguments cannot bind a bridge without final emitted-wrapper evidence"
                .into(),
        )
    })?;
    let wrapper_identity = bridge.entry_function_identity();
    let continuation_identity = arguments.continuation_identity();
    let wrapper_range = bridge.entry_text_offset()
        ..bridge
            .entry_text_offset()
            .checked_add(bridge.entry_text_size())
            .ok_or_else(|| {
                ProgramStorageEntryDiagnostic(
                    "emitted whole-root wrapper interval overflows".into(),
                )
            })?;
    let continuation_range = bridge.continuation_text_offset()
        ..bridge
            .continuation_text_offset()
            .checked_add(bridge.continuation_text_size())
            .ok_or_else(|| {
                ProgramStorageEntryDiagnostic(
                    "emitted whole-root continuation interval overflows".into(),
                )
            })?;
    if wrapper_identity.program_storage_entry_continuation() != continuation_identity.source_key()
        || emitted.wrapper_identity() != wrapper_identity
        || emitted.continuation_identity() != continuation_identity
        || emitted.wrapper_symbol() != bridge.entry_symbol()
        || emitted.wrapper_section_offset() != wrapper_range.start
        || emitted.wrapper_byte_count() != wrapper_range.len()
        || emitted.continuation_symbol() != bridge.continuation_link_symbol()
        || emitted.continuation_section_offset() != continuation_range.start
        || emitted.continuation_byte_count() != continuation_range.len()
        || emitted.wrapper_byte_fingerprint() == 0
        || emitted.continuation_byte_fingerprint() == 0
        || emitted.executable_inventory_fingerprint() == 0
    {
        return Err(ProgramStorageEntryDiagnostic(
            "whole-root arguments drifted from their exact emitted wrapper or Source identity"
                .into(),
        ));
    }

    let arrival = emitted.arrival();
    if arrival.target() != arguments.target()
        || arrival.wrapper_identity() != wrapper_identity
        || arrival.boundary_contract_fingerprint()
            != arguments
                .authority()
                .binding()
                .boundary_contract_fingerprint()
    {
        return Err(ProgramStorageEntryDiagnostic(
            "whole-root arguments drifted from the emitted physical-arrival contract".into(),
        ));
    }

    let transfer_roots = bridge.wrapper_transfer().roots();
    for (index, ((argument, transfer), arrival_root)) in arguments
        .arguments()
        .iter()
        .zip(transfer_roots)
        .zip(arrival.roots())
        .enumerate()
    {
        let (expected_role, expected_copy_offsets) = match index {
            0 => (ProgramStorageEntryRootRole::Image, [32, 40]),
            1 => (ProgramStorageEntryRootRole::InitialStorage, [48, 56]),
            _ => unreachable!("sealed whole-root argument carrier has two rows"),
        };
        if argument.role() != expected_role
            || argument.visible_parameter_index() != index
            || argument.call_parameter_index() != index
            || transfer.role() != expected_role
            || transfer.arrival_parameter_index() != index
            || transfer.source_parameter_index() != index
            || arrival_root.role() != expected_role
            || arrival_root.arrival_parameter_index() != index
            || argument.placement() != transfer.physical_arrival_placement()
            || arrival_root.physical_arrival_placement() != argument.placement()
        {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "emitted whole-root {expected_role:?} placement or semantic index drifted"
            )));
        }
        for (field_index, (copy, expected_stack_offset)) in arrival_root
            .copies()
            .iter()
            .zip(expected_copy_offsets)
            .enumerate()
        {
            let expected_source_offset = (field_index * 8) as u32;
            let expected_bytes =
                omega_isa_x86_64::encode_entry_indirect_u64_to_outgoing_stack_copy_bytes(
                    match expected_role {
                        ProgramStorageEntryRootRole::Image => {
                            omega_calling_conventions::MachineRegister::X86Rcx
                        }
                        ProgramStorageEntryRootRole::InitialStorage => {
                            omega_calling_conventions::MachineRegister::X86Rdx
                        }
                    },
                    expected_source_offset,
                    expected_stack_offset,
                )
                .map_err(|diagnostic| ProgramStorageEntryDiagnostic(diagnostic.message))?;
            if copy.source_byte_offset() != expected_source_offset
                || copy.caller_copy_stack_byte_offset() != expected_stack_offset
                || copy.final_bytes() != &expected_bytes
                || copy.section_byte_range().start < wrapper_range.start
                || copy.section_byte_range().end > wrapper_range.end
            {
                return Err(ProgramStorageEntryDiagnostic(format!(
                    "emitted whole-root {expected_role:?} field {field_index} copy drifted"
                )));
            }
        }
    }
    Ok(emitted.clone())
}

#[derive(Debug)]
pub struct ProgramStorageEntryEmittedWholeRootArgumentError {
    arguments: ProgramStorageEntryWholeRootArgumentCarrier,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramStorageEntryEmittedWholeRootArgumentError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_arguments(self) -> ProgramStorageEntryWholeRootArgumentCarrier {
        self.arguments
    }
}

impl std::fmt::Display for ProgramStorageEntryEmittedWholeRootArgumentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageEntryEmittedWholeRootArgumentError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_root_copy_rows_reject_role_register_and_offset_drift() {
        let canonical = |role, field_index: u32| {
            let (register, stack_base) = match role {
                ProgramStorageEntryRootRole::Image => {
                    (omega_calling_conventions::MachineRegister::X86Rcx, 32u32)
                }
                ProgramStorageEntryRootRole::InitialStorage => {
                    (omega_calling_conventions::MachineRegister::X86Rdx, 48u32)
                }
            };
            omega_isa_x86_64::encode_entry_indirect_u64_to_outgoing_stack_copy_bytes(
                register,
                field_index * 8,
                stack_base + field_index * 8,
            )
            .unwrap()
        };
        assert_ne!(
            canonical(ProgramStorageEntryRootRole::Image, 0),
            canonical(ProgramStorageEntryRootRole::InitialStorage, 0)
        );
        assert_ne!(
            canonical(ProgramStorageEntryRootRole::Image, 0),
            canonical(ProgramStorageEntryRootRole::Image, 1)
        );
    }
}

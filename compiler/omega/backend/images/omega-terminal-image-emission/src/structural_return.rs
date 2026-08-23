//! Exact structural-return evidence and byte replay.
//!
//! This module independently validates the retained native ABI placements,
//! terminal-Psi provenance, fuel attribution, affine-place/discard custody, and
//! final architecture return bytes for the bounded whole-root structural-return
//! lane. It does not construct object functions or installation records.

use omega_target::{Architecture, NativeTarget};
use omega_terminal_machine_code::{
    TerminalNativeFuelAttribution, TerminalNativeFuelSite, TerminalStructuralReturnRecord,
};
use omega_terminal_target_operations::TerminalPsiProvenance;
use psi_core::MachineId;
use psi_terminal_fuel::TerminalFuelSchedule;

use super::TerminalObjectError;

pub(super) fn validate_structural_return_record(
    target: NativeTarget,
    machine: MachineId,
    provenance: &TerminalPsiProvenance,
    bytes: &[u8],
    fuel_attribution: &[TerminalNativeFuelAttribution],
    returned: &TerminalStructuralReturnRecord,
) -> Result<(), TerminalObjectError> {
    let architecture = target.architecture;
    let expected_call_plan = omega_calling_conventions::evaluate_call_plan(
        omega_calling_conventions::CallingPolicy::native_for_target(target),
        &omega_calling_conventions::CallSignature {
            parameters: returned
                .parameter_placements
                .iter()
                .map(|placement| placement.shape)
                .collect(),
            result: Some(returned.shape),
        },
    )
    .map_err(|_| TerminalObjectError::InvalidStructuralReturnEvidence(machine))?;
    let source_index = returned.parameters.first().map(|_| 0);
    let end = returned
        .code_offset
        .checked_add(returned.byte_count)
        .ok_or(TerminalObjectError::InvalidStructuralReturnEvidence(
            machine,
        ))?;
    if returned.code_offset != 0
        || end != bytes.len()
        || returned.byte_count == 0
        || fuel_attribution.len() != returned.trivial_affine_locals.len() + 1
        || returned
            .trivial_affine_locals
            .iter()
            .enumerate()
            .any(|(ordinal, (operation, _, _))| {
                fuel_attribution.get(ordinal).is_none_or(|attribution| {
                    attribution.schedule != TerminalFuelSchedule::CURRENT.identity()
                        || attribution.site != TerminalNativeFuelSite::Operation(*operation)
                        || attribution.units != 1
                        || attribution.operation_ordinal != ordinal
                        || attribution.code_offset != 0
                        || attribution.byte_count != 0
                })
            })
        || fuel_attribution.last().is_none_or(|attribution| {
            attribution.schedule != TerminalFuelSchedule::CURRENT.identity()
                || attribution.site != TerminalNativeFuelSite::Edge(returned.psi_edge)
                || attribution.units != 1
                || attribution.operation_ordinal != returned.trivial_affine_locals.len()
                || attribution.code_offset != 0
                || attribution.byte_count != returned.byte_count
        })
        || provenance.edges.as_slice() != [returned.psi_edge]
        || provenance.operations
            != returned
                .trivial_affine_locals
                .iter()
                .map(|(operation, _, _)| *operation)
                .collect::<Vec<_>>()
        || returned.source.structural_type != returned.result.structural_type
        || returned.source.multiplicity != returned.result.multiplicity
        || returned.source.qualifications != returned.result.qualifications
        || returned.shape != returned.source_placement.shape
        || returned.shape != returned.result_placement.shape
        || returned.shape.byte_size != 8
        || returned.returned_claims.len() != 1
        || returned
            .parameters
            .iter()
            .enumerate()
            .any(|(index, parameter)| {
                parameter.is_self || usize::try_from(parameter.position) != Ok(index)
            })
        || returned.parameters.first() != Some(&returned.source)
        || returned.parameters.iter().skip(1).any(|parameter| {
            parameter.place == returned.source.place
                || parameter.place == returned.result.place
                || !parameter.qualifications.is_empty()
        })
        || returned
            .parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != returned.parameters.len()
        || returned.trivial_affine_locals.iter().enumerate().any(|(index, (_, local, local_type))| {
            !matches!(
                local.kind,
                psi_core::StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal,
                    structural_type
                } if usize::try_from(declaration_ordinal) == Ok(index)
                    && structural_type == local_type.id
            ) || local.id == returned.source.place
                || local.id == returned.result.place
                || returned.parameters.iter().any(|parameter| parameter.place == local.id)
                || local_type.identity.is_empty()
                || !matches!(
                    local_type.shape,
                    psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty()
                )
        })
        || returned
            .trivial_affine_locals
            .iter()
            .map(|(_, local, _)| local.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != returned.trivial_affine_locals.len()
        || returned.trivial_affine_discards
            != returned
                .trivial_affine_locals
                .iter()
                .rev()
                .map(|(_, local, _)| local.id)
                .chain(
                    returned
                        .parameters
                        .iter()
                        .skip(1)
                        .rev()
                        .map(|parameter| parameter.place),
                )
                .collect::<Vec<_>>()
        || returned
            .parameters
            .iter()
            .skip(1)
            .any(|parameter| parameter.multiplicity != psi_terminal::StructuralMultiplicity::Affine)
        || returned.parameter_placements.len() != returned.parameters.len()
        || expected_call_plan.parameters != returned.parameter_placements
        || expected_call_plan.result.as_ref() != Some(&returned.result_placement)
        || source_index.and_then(|index| returned.parameter_placements.get(index))
            != Some(&returned.source_placement)
    {
        return Err(TerminalObjectError::InvalidStructuralReturnEvidence(
            machine,
        ));
    }
    let expected = match architecture {
        Architecture::X86_64 => {
            let [
                omega_calling_conventions::ValueLocation::Register {
                    register: source,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] = returned.source_placement.locations.as_slice()
            else {
                return Err(TerminalObjectError::InvalidStructuralReturnEvidence(
                    machine,
                ));
            };
            let [
                omega_calling_conventions::ValueLocation::Register {
                    register: omega_calling_conventions::MachineRegister::X86Rax,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] = returned.result_placement.locations.as_slice()
            else {
                return Err(TerminalObjectError::InvalidStructuralReturnEvidence(
                    machine,
                ));
            };
            match source {
                omega_calling_conventions::MachineRegister::X86Rdi => &[0x48, 0x89, 0xf8, 0xc3][..],
                omega_calling_conventions::MachineRegister::X86Rcx => &[0x48, 0x89, 0xc8, 0xc3][..],
                _ => {
                    return Err(TerminalObjectError::InvalidStructuralReturnEvidence(
                        machine,
                    ));
                }
            }
        }
        Architecture::Aarch64 => {
            let [
                omega_calling_conventions::ValueLocation::Register {
                    register: omega_calling_conventions::MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] = returned.source_placement.locations.as_slice()
            else {
                return Err(TerminalObjectError::InvalidStructuralReturnEvidence(
                    machine,
                ));
            };
            let [
                omega_calling_conventions::ValueLocation::Register {
                    register: omega_calling_conventions::MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] = returned.result_placement.locations.as_slice()
            else {
                return Err(TerminalObjectError::InvalidStructuralReturnEvidence(
                    machine,
                ));
            };
            &[0xc0, 0x03, 0x5f, 0xd6]
        }
    };
    if bytes != expected {
        return Err(TerminalObjectError::StructuralReturnBytesMismatch(machine));
    }
    Ok(())
}

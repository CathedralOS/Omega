//! Derives and composes exact compiler instruction footprints.

use super::*;

mod buffer_wire_text;
mod control_entry;
mod outbound_calls;
mod storage_place;

type CompilerInstructionFootprintParts = (
    omega_machine_instructions::BoundaryFootprintFragmentOrigin,
    omega_calling_conventions::RegisterSet,
    omega_calling_conventions::MachineStateSet,
);

type CompilerInstructionFootprint = (
    omega_machine_instructions::BoundaryFootprintFragmentOrigin,
    omega_calling_conventions::StateFootprintEvidence,
);

type CompilerInstructionFootprintFamily = fn(
    Architecture,
    &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    omega_machine_bytes::CompilerInstructionValidationKind,
) -> Option<CompilerInstructionFootprintParts>;

const COMPILER_INSTRUCTION_FOOTPRINT_FAMILIES: [CompilerInstructionFootprintFamily; 4] = [
    control_entry::control_entry_footprint_parts,
    storage_place::storage_place_footprint_parts,
    outbound_calls::outbound_call_footprint_parts,
    buffer_wire_text::buffer_wire_text_footprint_parts,
];

fn compiler_instruction_footprint(
    architecture: Architecture,
    runtime_value_operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Option<CompilerInstructionFootprint> {
    for resolve_family in COMPILER_INSTRUCTION_FOOTPRINT_FAMILIES {
        if let Some((origin, registers, additional_state)) =
            resolve_family(architecture, runtime_value_operands, kind.clone())
        {
            return Some((
                origin,
                omega_calling_conventions::StateFootprintEvidence::new(registers, additional_state),
            ));
        }
    }
    None
}

pub(super) fn require_compiler_instruction_footprint(
    architecture: Architecture,
    runtime_value_operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
    selected_instruction_index: u32,
) -> Result<
    (
        omega_machine_instructions::BoundaryFootprintFragmentOrigin,
        omega_calling_conventions::StateFootprintEvidence,
    ),
    Diagnostic,
> {
    compiler_instruction_footprint(architecture, runtime_value_operands, kind).ok_or_else(|| {
        Diagnostic::error(format!(
            "compiler instruction #{selected_instruction_index} has a retained final-byte validation identity but no target footprint derivation"
        ))
    })
}

pub(super) fn validate_compiler_composed_footprint(
    semantics: &omega_machine_bytes::EncodedMachineSemanticSummary,
    derived: &[(
        omega_machine_instructions::BoundaryFootprintFragmentOrigin,
        omega_calling_conventions::StateFootprintEvidence,
    )],
) -> Result<u64, Diagnostic> {
    let final_footprint = omega_calling_conventions::compose_state_footprints(
        derived.iter().map(|(_, evidence)| evidence),
    );
    let retained_footprint = omega_calling_conventions::compose_state_footprints(
        semantics
            .boundaries
            .footprints
            .fragments
            .iter()
            .filter(|fragment| {
                fragment.origin
                    != omega_machine_instructions::BoundaryFootprintFragmentOrigin::CheckedAssemblyCatalog
            })
            .map(|fragment| &fragment.evidence),
    );
    if final_footprint != retained_footprint {
        return Err(Diagnostic::error(format!(
            "complete final compiler-row footprint does not equal the StatePlan-validated semantic union: retained={retained_footprint:?}, replayed={final_footprint:?}"
        )));
    }
    Ok(final_footprint.evidence_fingerprint())
}

pub(super) fn validate_compiler_body_specification_footprints(
    semantics: &omega_machine_bytes::EncodedMachineSemanticSummary,
    derived: &[(
        omega_machine_instructions::BoundaryFootprintFragmentOrigin,
        omega_calling_conventions::StateFootprintEvidence,
    )],
) -> Result<(u64, u64), Diagnostic> {
    use omega_calling_conventions::compose_state_footprints;
    use omega_machine_instructions::BoundaryFootprintFragmentOrigin;

    let has_body_rows = derived.iter().any(|(origin, _)| {
        matches!(
            origin,
            BoundaryFootprintFragmentOrigin::DispatchScaffold
                | BoundaryFootprintFragmentOrigin::StaticGuardComparison
                | BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison
                | BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison
                | BoundaryFootprintFragmentOrigin::PlaceGuardComparison
                | BoundaryFootprintFragmentOrigin::ExitResultRegisters
                | BoundaryFootprintFragmentOrigin::EntryStorage
                | BoundaryFootprintFragmentOrigin::EntrySliceDescriptor
                | BoundaryFootprintFragmentOrigin::ExitIndirectResultCopy
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceCopy
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceIntegerWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyAtomicOperation
                | BoundaryFootprintFragmentOrigin::CompilerBodyConstantHostResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundFloatImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDereferencedImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundIndirectCall
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundOpenCreateImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeLineRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecArgument
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireLiteralByteAppend
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintAppend
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireTextBytesAppend
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarSliceAppend
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintAppend
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireExpectedByteRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireByteSliceRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedOpen
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedClose
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite
        )
    });
    let boundary_contract_fingerprint = if !has_body_rows {
        0
    } else {
        semantics
            .boundaries
            .footprints
            .boundary_contract_fingerprint
            .ok_or_else(|| {
                Diagnostic::error(
                    "final body-specification footprint rows have no StatePlan boundary-contract identity",
                )
            })?
    };
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(
        &mut fingerprint,
        &boundary_contract_fingerprint.to_le_bytes(),
    );
    for (tag, origin) in [
        (1u8, BoundaryFootprintFragmentOrigin::DispatchScaffold),
        (2u8, BoundaryFootprintFragmentOrigin::StaticGuardComparison),
        (
            3u8,
            BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
        ),
        (
            4u8,
            BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison,
        ),
        (5u8, BoundaryFootprintFragmentOrigin::PlaceGuardComparison),
        (6u8, BoundaryFootprintFragmentOrigin::ExitResultRegisters),
        (7u8, BoundaryFootprintFragmentOrigin::EntryStorage),
        (8u8, BoundaryFootprintFragmentOrigin::EntrySliceDescriptor),
        (9u8, BoundaryFootprintFragmentOrigin::ExitIndirectResultCopy),
        (10u8, BoundaryFootprintFragmentOrigin::CompilerBodyPlaceCopy),
        (
            11u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceIntegerWrite,
        ),
        (
            12u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite,
        ),
        (
            13u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite,
        ),
        (
            14u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite,
        ),
        (
            15u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
        ),
        (
            16u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite,
        ),
        (
            17u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
        ),
        (
            18u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite,
        ),
        (
            19u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyConstantHostResult,
        ),
        (
            20u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImport,
        ),
        (
            21u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImportResult,
        ),
        (
            22u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundFloatImportResult,
        ),
        (
            23u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImport,
        ),
        (
            24u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImportResult,
        ),
        (
            25u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall,
        ),
        (
            26u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult,
        ),
        (
            27u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments,
        ),
        (
            28u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments,
        ),
        (
            29u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments,
        ),
        (
            30u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments,
        ),
        (
            31u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecArgument,
        ),
        (
            32u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecResult,
        ),
        (
            33u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDereferencedImportResult,
        ),
        (
            34u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImport,
        ),
        (
            35u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImportResult,
        ),
        (
            36u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImport,
        ),
        (
            37u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImportResult,
        ),
        (
            38u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImport,
        ),
        (
            39u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImportResult,
        ),
        (
            40u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImport,
        ),
        (
            41u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImportResult,
        ),
        (
            42u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateResult,
        ),
        (
            43u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundOpenCreateImport,
        ),
        (
            44u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteRead,
        ),
        (
            45u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteWrite,
        ),
        (
            46u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeLineRead,
        ),
        (
            47u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireLiteralByteAppend,
        ),
        (
            48u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintAppend,
        ),
        (
            49u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireExpectedByteRead,
        ),
        (
            50u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintRead,
        ),
        (
            51u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireTextBytesAppend,
        ),
        (
            52u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarSliceAppend,
        ),
        (
            53u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintAppend,
        ),
        (
            54u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireByteSliceRead,
        ),
        (
            55u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedOpen,
        ),
        (
            56u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedClose,
        ),
        (
            57u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintRead,
        ),
        (
            58u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyAtomicOperation,
        ),
        (
            59u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundIndirectCall,
        ),
    ] {
        let evidence_rows = derived
            .iter()
            .filter_map(|(row_origin, evidence)| (*row_origin == origin).then_some(evidence))
            .collect::<Vec<_>>();
        let retained = semantics
            .boundaries
            .footprints
            .fragments
            .iter()
            .filter(|fragment| fragment.origin == origin)
            .collect::<Vec<_>>();
        if evidence_rows.is_empty() {
            let retains_valid_empty_entry_storage = origin
                == BoundaryFootprintFragmentOrigin::EntryStorage
                && retained.len() == 1
                && retained[0].evidence.registers().as_slice().is_empty()
                && retained[0].evidence.machine_state().is_empty();
            if !retained.is_empty() && !retains_valid_empty_entry_storage {
                return Err(Diagnostic::error(format!(
                    "retained {origin:?} footprint has no final target-specification instruction rows"
                )));
            }
            continue;
        }
        let composed = compose_state_footprints(evidence_rows.iter().copied());
        if retained.len() != 1 || retained[0].evidence != composed {
            return Err(Diagnostic::error(format!(
                "final {origin:?} target-specification footprint does not match its StatePlan-validated semantic fragment: retained={:?}, replayed={composed:?}",
                retained
                    .iter()
                    .map(|fragment| &fragment.evidence)
                    .collect::<Vec<_>>()
            )));
        }
        fingerprint_into(&mut fingerprint, &[tag]);
        fingerprint_into(
            &mut fingerprint,
            &(evidence_rows.len() as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut fingerprint,
            &composed.evidence_fingerprint().to_le_bytes(),
        );
    }
    Ok((boundary_contract_fingerprint, fingerprint))
}

pub(super) fn validate_compiler_fixed_mechanics_footprint(
    semantics: &omega_machine_bytes::EncodedMachineSemanticSummary,
    derived: &[(
        omega_machine_instructions::BoundaryFootprintFragmentOrigin,
        omega_calling_conventions::StateFootprintEvidence,
    )],
) -> Result<(u64, u64), Diagnostic> {
    use omega_calling_conventions::compose_state_footprints;
    use omega_machine_instructions::BoundaryFootprintFragmentOrigin;

    let evidence_rows = derived
        .iter()
        .filter_map(|(origin, evidence)| {
            (*origin == BoundaryFootprintFragmentOrigin::CallReturnMechanics).then_some(evidence)
        })
        .collect::<Vec<_>>();
    if evidence_rows.is_empty() {
        return Ok((0, 0xcbf2_9ce4_8422_2325u64));
    }
    let boundary_contract_fingerprint = semantics
        .boundaries
        .footprints
        .boundary_contract_fingerprint
        .ok_or_else(|| {
            Diagnostic::error(
                "final call-return footprint rows have no StatePlan boundary-contract identity",
            )
        })?;
    let retained = semantics
        .boundaries
        .footprints
        .fragments
        .iter()
        .filter(|fragment| fragment.origin == BoundaryFootprintFragmentOrigin::CallReturnMechanics)
        .collect::<Vec<_>>();
    let composed = compose_state_footprints(evidence_rows.iter().copied());
    if retained.len() != 1 || retained[0].evidence != composed {
        return Err(Diagnostic::error(
            "final CallReturnMechanics target-specification footprint does not match its StatePlan-validated semantic fragment",
        ));
    }
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(
        &mut fingerprint,
        &boundary_contract_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut fingerprint,
        &(evidence_rows.len() as u64).to_le_bytes(),
    );
    fingerprint_into(
        &mut fingerprint,
        &composed.evidence_fingerprint().to_le_bytes(),
    );
    Ok((boundary_contract_fingerprint, fingerprint))
}

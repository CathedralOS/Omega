use omega_machine_code::{
    FunctionFragment, FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan,
    FunctionFragmentInternalMachineFixup,
};
use omega_optimization_core::FunctionFragmentEmissionManifestIdentity;

use crate::TargetFrameProtocolEncodingPlan;

use super::{
    FunctionAppliedFrameProtocol, FunctionFragmentFrameApplication,
    FunctionFragmentFrameApplicationError, FunctionFragmentFrameApplicationIdentity,
};

pub(super) fn apply(
    source: &FunctionFragmentEmissionPlan,
    source_manifest: FunctionFragmentEmissionManifestIdentity,
    protocol: &TargetFrameProtocolEncodingPlan,
) -> Result<FunctionFragmentFrameApplication, FunctionFragmentFrameApplicationError> {
    if source.target != protocol.target {
        return Err(FunctionFragmentFrameApplicationError::RootMismatch);
    }
    if source.functions.len() != protocol.functions.len() {
        return Err(FunctionFragmentFrameApplicationError::FunctionRosterMismatch);
    }
    if !source.structural_unit_functions.is_empty() {
        return Err(FunctionFragmentFrameApplicationError::RootMismatch);
    }

    let mut fragments = source.clone();
    let mut applications = Vec::with_capacity(fragments.functions.len());
    for function in &mut fragments.functions {
        let mut rows = protocol
            .functions
            .iter()
            .filter(|row| row.machine == function.machine);
        let Some(row) = rows.next() else {
            return Err(FunctionFragmentFrameApplicationError::MissingFunction(
                function.machine,
            ));
        };
        if rows.next().is_some() {
            return Err(FunctionFragmentFrameApplicationError::FunctionRosterMismatch);
        }
        let prologue = row.prologue.bytes(&protocol.bytes).ok_or(
            FunctionFragmentFrameApplicationError::InvalidProtocolSpan(function.machine),
        )?;
        let epilogue = row.epilogue.bytes(&protocol.bytes).ok_or(
            FunctionFragmentFrameApplicationError::InvalidProtocolSpan(function.machine),
        )?;
        applications.push(apply_function(function, prologue, epilogue)?);
    }

    if protocol.functions.iter().any(|row| {
        !source
            .functions
            .iter()
            .any(|function| function.machine == row.machine)
    }) {
        return Err(FunctionFragmentFrameApplicationError::FunctionRosterMismatch);
    }

    fragments.identity = fragments.recomputed_identity();
    let mut application = FunctionFragmentFrameApplication {
        identity: FunctionFragmentFrameApplicationIdentity::from_bytes([0; 32]),
        source_fragment_manifest: source_manifest,
        source_fragments: source.identity,
        frame_protocol: crate::target_frame_protocol_encoding_identity(protocol),
        functions: applications,
        fragments,
    };
    application.identity = application.recomputed_identity();
    Ok(application)
}

fn apply_function(
    function: &mut FunctionFragment,
    prologue: &[u8],
    epilogue: &[u8],
) -> Result<FunctionAppliedFrameProtocol, FunctionFragmentFrameApplicationError> {
    if prologue.is_empty() && epilogue.is_empty() {
        return Ok(FunctionAppliedFrameProtocol {
            machine: function.machine,
            prologue_function_offset: 0,
            prologue_byte_count: 0,
            epilogue_function_offset: function.byte_count,
            epilogue_byte_count: 0,
        });
    }
    if prologue.is_empty() != epilogue.is_empty() || function.blocks.len() != 1 {
        return Err(
            FunctionFragmentFrameApplicationError::UnsupportedFramedControl(function.machine),
        );
    }

    let block = &mut function.blocks[0];
    let source_len = u64::try_from(function.bytes.len())
        .map_err(|_| FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    if block.offset != 0 || block.byte_count != source_len || function.byte_count != source_len {
        return Err(FunctionFragmentFrameApplicationError::SourceShapeMismatch(
            function.machine,
        ));
    }
    if block.instructions.iter().any(|row| row.branch.is_some()) {
        return Err(
            FunctionFragmentFrameApplicationError::UnsupportedFramedControl(function.machine),
        );
    }
    let Some(last) = block.instructions.last() else {
        return Err(FunctionFragmentFrameApplicationError::MissingFinalReturn(
            function.machine,
        ));
    };
    if !matches!(
        last.control,
        FunctionFragmentControlProvenance::Return { .. }
    ) || block
        .instructions
        .iter()
        .filter(|row| {
            matches!(
                row.control,
                FunctionFragmentControlProvenance::Return { .. }
            )
        })
        .count()
        != 1
    {
        return Err(FunctionFragmentFrameApplicationError::MissingFinalReturn(
            function.machine,
        ));
    }
    let return_offset = last.offset;
    let return_start = usize::try_from(return_offset)
        .map_err(|_| FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    if return_start > function.bytes.len()
        || last.offset.checked_add(
            u64::try_from(last.bytes.len())
                .map_err(|_| FunctionFragmentFrameApplicationError::OffsetOverflow)?,
        ) != Some(function.byte_count)
    {
        return Err(FunctionFragmentFrameApplicationError::SourceShapeMismatch(
            function.machine,
        ));
    }

    let prologue_len = u64::try_from(prologue.len())
        .map_err(|_| FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    let epilogue_len = u64::try_from(epilogue.len())
        .map_err(|_| FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    let epilogue_offset = prologue_len
        .checked_add(return_offset)
        .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?;

    let mut bytes = Vec::with_capacity(
        prologue
            .len()
            .checked_add(function.bytes.len())
            .and_then(|length| length.checked_add(epilogue.len()))
            .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?,
    );
    bytes.extend_from_slice(prologue);
    bytes.extend_from_slice(&function.bytes[..return_start]);
    bytes.extend_from_slice(epilogue);
    bytes.extend_from_slice(&function.bytes[return_start..]);

    for row in &mut block.instructions {
        row.offset = row
            .offset
            .checked_add(prologue_len)
            .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?;
        if matches!(
            row.control,
            FunctionFragmentControlProvenance::Return { .. }
        ) {
            row.offset = row
                .offset
                .checked_add(epilogue_len)
                .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?;
        }
        if let Some(fixup) = &mut row.internal_machine_fixup {
            shift_fixup(fixup, prologue_len)?;
        }
    }
    block.offset = prologue_len;
    block.byte_count = block
        .byte_count
        .checked_add(epilogue_len)
        .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    function.byte_count = function
        .byte_count
        .checked_add(prologue_len)
        .and_then(|count| count.checked_add(epilogue_len))
        .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    function.bytes = bytes;

    Ok(FunctionAppliedFrameProtocol {
        machine: function.machine,
        prologue_function_offset: 0,
        prologue_byte_count: prologue_len,
        epilogue_function_offset: epilogue_offset,
        epilogue_byte_count: epilogue_len,
    })
}

fn shift_fixup(
    fixup: &mut FunctionFragmentInternalMachineFixup,
    shift: u64,
) -> Result<(), FunctionFragmentFrameApplicationError> {
    fixup.opcode_function_offset = fixup
        .opcode_function_offset
        .checked_add(shift)
        .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    fixup.patch_function_offset = fixup
        .patch_function_offset
        .checked_add(shift)
        .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    fixup.reference_function_offset = fixup
        .reference_function_offset
        .checked_add(shift)
        .ok_or(FunctionFragmentFrameApplicationError::OffsetOverflow)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use omega_machine_code::{
        FunctionFragmentBlockSpan, FunctionFragmentInstructionSpan,
        FunctionFragmentInternalMachineFixupKind, FunctionFragmentInternalMachineFixupState,
    };
    use omega_optimization_core::{
        FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    };
    use omega_register_model::{PhysicalRegisterModelIdentity, TargetRegisterEnvironmentIdentity};
    use omega_selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
        SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
    };
    use omega_target::NativeTarget;
    use omega_target_operations::TerminalPsiProvenance;
    use psi_core::{EdgeId, FuelScheduleIdentity, MachineId};
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use crate::{
        FrameProtocolByteSpan, FunctionTargetFrameProtocolEncoding, TargetFrameLayoutIdentity,
        TargetFrameProtocolEncodingPolicy,
    };

    use super::*;

    fn source_plan() -> FunctionFragmentEmissionPlan {
        let machine = MachineId::new(1).unwrap();
        let callee = MachineId::new(2).unwrap();
        let call = FunctionFragmentInstructionSpan {
            instruction: SelectedInstructionId(1),
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::CallI64,
                variant: 0,
            },
            offset: 0,
            bytes: vec![0xe8, 0, 0, 0, 0],
            branch: None,
            internal_machine_fixup: Some(FunctionFragmentInternalMachineFixup {
                kind: FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
                state: FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1,
                callee,
                opcode_function_offset: 0,
                patch_function_offset: 1,
                reference_function_offset: 5,
                patch_byte_width: 4,
                addend: 0,
            }),
            provenance: SelectedInstructionProvenance::default(),
            control: FunctionFragmentControlProvenance::DirectInternalCall { callee },
        };
        let ret = FunctionFragmentInstructionSpan {
            instruction: SelectedInstructionId(2),
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::ReturnI64,
                variant: 0,
            },
            offset: 5,
            bytes: vec![0xc3],
            branch: None,
            internal_machine_fixup: None,
            provenance: SelectedInstructionProvenance::default(),
            control: FunctionFragmentControlProvenance::Return {
                psi_return_edge: EdgeId::new(1).unwrap(),
            },
        };
        let mut plan = FunctionFragmentEmissionPlan {
            identity: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"pending"),
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            selected: SelectedInstructionPlanIdentity::from_canonical_bytes(b"selected"),
            target: NativeTarget::linux_x64(),
            entry: machine,
            functions: vec![FunctionFragment {
                machine,
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                byte_count: 6,
                bytes: vec![0xe8, 0, 0, 0, 0, 0xc3],
                blocks: vec![FunctionFragmentBlockSpan {
                    block: SelectedBlockId(1),
                    offset: 0,
                    byte_count: 6,
                    instructions: vec![call, ret],
                }],
            }],
            structural_unit_functions: Vec::new(),
        };
        plan.identity = plan.recomputed_identity();
        plan
    }

    fn protocol(machine: MachineId) -> TargetFrameProtocolEncodingPlan {
        TargetFrameProtocolEncodingPlan {
            frame_layout: TargetFrameLayoutIdentity::from_bytes([4; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([5; 32]),
            physical_register_model: PhysicalRegisterModelIdentity::from_bytes([6; 32]),
            target: NativeTarget::linux_x64(),
            policy: TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1,
            functions: vec![FunctionTargetFrameProtocolEncoding {
                machine,
                prologue: FrameProtocolByteSpan {
                    offset: 0,
                    length: 1,
                },
                epilogue: FrameProtocolByteSpan {
                    offset: 1,
                    length: 1,
                },
            }],
            bytes: vec![0xaa, 0xbb],
        }
    }

    #[test]
    fn frame_bytes_shift_selected_rows_and_internal_call_fixups_exactly() {
        let source = source_plan();
        let machine = source.entry;
        let application = apply(
            &source,
            FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest"),
            &protocol(machine),
        )
        .unwrap();
        let function = &application.fragments.functions[0];
        assert_eq!(function.bytes, vec![0xaa, 0xe8, 0, 0, 0, 0, 0xbb, 0xc3]);
        assert_eq!(function.byte_count, 8);
        assert_eq!(function.blocks[0].offset, 1);
        assert_eq!(function.blocks[0].byte_count, 7);
        assert_eq!(function.blocks[0].instructions[0].offset, 1);
        assert_eq!(function.blocks[0].instructions[1].offset, 7);
        let fixup = function.blocks[0].instructions[0]
            .internal_machine_fixup
            .unwrap();
        assert_eq!(fixup.opcode_function_offset, 1);
        assert_eq!(fixup.patch_function_offset, 2);
        assert_eq!(fixup.reference_function_offset, 6);
        assert_eq!(application.functions[0].prologue_function_offset, 0);
        assert_eq!(application.functions[0].epilogue_function_offset, 6);
        assert_eq!(application.identity, application.recomputed_identity());
        assert_eq!(
            application.fragments.identity,
            application.fragments.recomputed_identity()
        );
    }

    #[test]
    fn framed_multi_block_and_nonfinal_return_shapes_fail_closed() {
        let mut source = source_plan();
        let machine = source.entry;
        let repeated = source.functions[0].blocks[0].clone();
        source.functions[0].blocks.push(repeated);
        assert_eq!(
            apply(
                &source,
                FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest"),
                &protocol(machine),
            ),
            Err(FunctionFragmentFrameApplicationError::UnsupportedFramedControl(machine))
        );

        let mut source = source_plan();
        source.functions[0].blocks[0].instructions.swap(0, 1);
        assert_eq!(
            apply(
                &source,
                FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest"),
                &protocol(machine),
            ),
            Err(FunctionFragmentFrameApplicationError::MissingFinalReturn(
                machine
            ))
        );
    }
}

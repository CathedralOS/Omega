use omega_machine_code::{
    FunctionFragment, FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan,
    FunctionFragmentInternalMachineFixup,
};
use omega_optimization_core::FunctionFragmentEmissionManifestIdentity;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_target::Architecture;

use crate::TargetFrameProtocolEncodingPlan;

use super::{
    FrameApplicationError, FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol,
    FunctionFragmentFrameApplication, FunctionFragmentFrameApplicationIdentity,
};

#[derive(Debug, Clone, Copy)]
struct ReturnSite {
    block: omega_selected_instructions::SelectedBlockId,
    instruction: omega_selected_instructions::SelectedInstructionId,
    psi_edge: psi_core::EdgeId,
    offset: u64,
}

pub(super) fn apply(
    source: &FunctionFragmentEmissionPlan,
    source_manifest: FunctionFragmentEmissionManifestIdentity,
    protocol: &TargetFrameProtocolEncodingPlan,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<FunctionFragmentFrameApplication, FrameApplicationError> {
    if source.target != protocol.target {
        return Err(FrameApplicationError::RootMismatch);
    }
    if source.functions.len() != protocol.functions.len() {
        return Err(FrameApplicationError::FunctionRosterMismatch);
    }
    if !source.structural_unit_functions.is_empty() {
        return Err(FrameApplicationError::RootMismatch);
    }

    let mut fragments = source.clone();
    let mut applications = Vec::with_capacity(fragments.functions.len());
    for function in &mut fragments.functions {
        let mut rows = protocol
            .functions
            .iter()
            .filter(|row| row.machine == function.machine);
        let Some(row) = rows.next() else {
            return Err(FrameApplicationError::MissingFunction(function.machine));
        };
        if rows.next().is_some() {
            return Err(FrameApplicationError::FunctionRosterMismatch);
        }
        let prologue = row
            .prologue
            .bytes(&protocol.bytes)
            .ok_or(FrameApplicationError::InvalidProtocolSpan(function.machine))?;
        let epilogue = row
            .epilogue
            .bytes(&protocol.bytes)
            .ok_or(FrameApplicationError::InvalidProtocolSpan(function.machine))?;
        applications.push(apply_function(
            function,
            prologue,
            epilogue,
            source.target.architecture,
            physical,
        )?);
    }

    if protocol.functions.iter().any(|row| {
        !source
            .functions
            .iter()
            .any(|function| function.machine == row.machine)
    }) {
        return Err(FrameApplicationError::FunctionRosterMismatch);
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
    architecture: Architecture,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<FunctionAppliedFrameProtocol, FrameApplicationError> {
    if prologue.is_empty() != epilogue.is_empty() {
        return Err(FrameApplicationError::UnsupportedFramedControl(
            function.machine,
        ));
    }

    let source_len =
        u64::try_from(function.bytes.len()).map_err(|_| FrameApplicationError::OffsetOverflow)?;
    if function.byte_count != source_len {
        return Err(FrameApplicationError::SourceShapeMismatch(function.machine));
    }
    let prologue_len =
        u64::try_from(prologue.len()).map_err(|_| FrameApplicationError::OffsetOverflow)?;
    let epilogue_len =
        u64::try_from(epilogue.len()).map_err(|_| FrameApplicationError::OffsetOverflow)?;
    let return_sites = validate_and_collect_returns(function)?;
    if return_sites.is_empty() {
        return Err(FrameApplicationError::MissingFinalReturn(function.machine));
    }

    let mut bytes = Vec::with_capacity(
        prologue
            .len()
            .checked_add(function.bytes.len())
            .and_then(|length| {
                epilogue
                    .len()
                    .checked_mul(return_sites.len())
                    .and_then(|epilogues| length.checked_add(epilogues))
            })
            .ok_or(FrameApplicationError::OffsetOverflow)?,
    );
    bytes.extend_from_slice(prologue);
    let mut source_cursor = 0_usize;
    let mut applications = Vec::with_capacity(return_sites.len());
    for (ordinal, site) in return_sites.iter().copied().enumerate() {
        let return_start =
            usize::try_from(site.offset).map_err(|_| FrameApplicationError::OffsetOverflow)?;
        bytes.extend_from_slice(&function.bytes[source_cursor..return_start]);
        let prior_epilogues = u64::try_from(ordinal)
            .map_err(|_| FrameApplicationError::OffsetOverflow)?
            .checked_mul(epilogue_len)
            .ok_or(FrameApplicationError::OffsetOverflow)?;
        let function_offset = prologue_len
            .checked_add(site.offset)
            .and_then(|offset| offset.checked_add(prior_epilogues))
            .ok_or(FrameApplicationError::OffsetOverflow)?;
        bytes.extend_from_slice(epilogue);
        applications.push(FunctionAppliedFrameEpilogue {
            block: site.block,
            return_instruction: site.instruction,
            psi_return_edge: site.psi_edge,
            function_offset,
            byte_count: epilogue_len,
        });
        source_cursor = return_start;
    }
    bytes.extend_from_slice(&function.bytes[source_cursor..]);

    for block in &mut function.blocks {
        let source_block_offset = block.offset;
        let prior_block_epilogues = insertion_count_before(&return_sites, source_block_offset)?;
        block.offset = shifted_offset(
            source_block_offset,
            prologue_len,
            prior_block_epilogues,
            epilogue_len,
        )?;
        let mut block_epilogue_count = 0_u64;
        for row in &mut block.instructions {
            let source_row_offset = row.offset;
            let prior_row_epilogues = insertion_count_before(&return_sites, source_row_offset)?;
            let is_return = matches!(
                row.control,
                FunctionFragmentControlProvenance::Return { .. }
            );
            if is_return {
                block_epilogue_count = block_epilogue_count
                    .checked_add(1)
                    .ok_or(FrameApplicationError::OffsetOverflow)?;
            }
            let applied_before_row = prior_row_epilogues
                .checked_add(u64::from(is_return))
                .ok_or(FrameApplicationError::OffsetOverflow)?;
            row.offset = shifted_offset(
                source_row_offset,
                prologue_len,
                applied_before_row,
                epilogue_len,
            )?;
            if let Some(fixup) = &mut row.internal_machine_fixup {
                shift_fixup(fixup, row.offset - source_row_offset)?;
            }
        }
        block.byte_count = block
            .byte_count
            .checked_add(
                block_epilogue_count
                    .checked_mul(epilogue_len)
                    .ok_or(FrameApplicationError::OffsetOverflow)?,
            )
            .ok_or(FrameApplicationError::OffsetOverflow)?;
    }
    function.byte_count = function
        .byte_count
        .checked_add(prologue_len)
        .and_then(|count| {
            epilogue_len
                .checked_mul(u64::try_from(return_sites.len()).ok()?)
                .and_then(|epilogues| count.checked_add(epilogues))
        })
        .ok_or(FrameApplicationError::OffsetOverflow)?;
    function.bytes = bytes;
    super::reflow::reencode_branches(function, architecture, physical)?;

    Ok(FunctionAppliedFrameProtocol {
        machine: function.machine,
        prologue_function_offset: 0,
        prologue_byte_count: prologue_len,
        epilogues: applications,
    })
}

fn validate_and_collect_returns(
    function: &FunctionFragment,
) -> Result<Vec<ReturnSite>, FrameApplicationError> {
    let mut expected_block_offset = 0_u64;
    let mut returns = Vec::new();
    for block in &function.blocks {
        if block.offset != expected_block_offset {
            return Err(FrameApplicationError::SourceShapeMismatch(function.machine));
        }
        let mut expected_row_offset = block.offset;
        for (index, row) in block.instructions.iter().enumerate() {
            if row.offset != expected_row_offset {
                return Err(FrameApplicationError::SourceShapeMismatch(function.machine));
            }
            let row_len = u64::try_from(row.bytes.len())
                .map_err(|_| FrameApplicationError::OffsetOverflow)?;
            expected_row_offset = expected_row_offset
                .checked_add(row_len)
                .ok_or(FrameApplicationError::OffsetOverflow)?;
            if let FunctionFragmentControlProvenance::Return { psi_return_edge } = row.control {
                if index + 1 != block.instructions.len() {
                    return Err(FrameApplicationError::MissingFinalReturn(function.machine));
                }
                returns.push(ReturnSite {
                    block: block.block,
                    instruction: row.instruction,
                    psi_edge: psi_return_edge,
                    offset: row.offset,
                });
            }
        }
        if expected_row_offset
            != block
                .offset
                .checked_add(block.byte_count)
                .ok_or(FrameApplicationError::OffsetOverflow)?
        {
            return Err(FrameApplicationError::SourceShapeMismatch(function.machine));
        }
        expected_block_offset = expected_row_offset;
    }
    if expected_block_offset != function.byte_count {
        return Err(FrameApplicationError::SourceShapeMismatch(function.machine));
    }
    Ok(returns)
}

fn insertion_count_before(
    return_sites: &[ReturnSite],
    source_offset: u64,
) -> Result<u64, FrameApplicationError> {
    u64::try_from(
        return_sites
            .iter()
            .take_while(|site| site.offset < source_offset)
            .count(),
    )
    .map_err(|_| FrameApplicationError::OffsetOverflow)
}

fn shifted_offset(
    source_offset: u64,
    prologue_len: u64,
    prior_epilogues: u64,
    epilogue_len: u64,
) -> Result<u64, FrameApplicationError> {
    source_offset
        .checked_add(prologue_len)
        .and_then(|offset| {
            prior_epilogues
                .checked_mul(epilogue_len)
                .and_then(|shift| offset.checked_add(shift))
        })
        .ok_or(FrameApplicationError::OffsetOverflow)
}

fn shift_fixup(
    fixup: &mut FunctionFragmentInternalMachineFixup,
    shift: u64,
) -> Result<(), FrameApplicationError> {
    fixup.opcode_function_offset = fixup
        .opcode_function_offset
        .checked_add(shift)
        .ok_or(FrameApplicationError::OffsetOverflow)?;
    fixup.patch_function_offset = fixup
        .patch_function_offset
        .checked_add(shift)
        .ok_or(FrameApplicationError::OffsetOverflow)?;
    fixup.reference_function_offset = fixup
        .reference_function_offset
        .checked_add(shift)
        .ok_or(FrameApplicationError::OffsetOverflow)?;
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
    use omega_register_model::{
        PhysicalRegisterModelIdentity, TargetRegisterEnvironmentIdentity,
        validate_physical_register_model,
    };
    use omega_selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
        SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
    };
    use omega_target::NativeTarget;
    use omega_target_operations::TerminalPsiProvenance;
    use psi_core::{EdgeId, FuelScheduleIdentity, MachineId};
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use crate::{
        FrameProtocolByteSpan, FunctionTargetFrameProtocolEncoding,
        TargetFrameProtocolEncodingPolicy,
    };
    use omega_machine_code::TargetFrameLayoutIdentity;

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

    fn physical() -> omega_register_model::ValidatedPhysicalRegisterModel {
        validate_physical_register_model(omega_isa_x86_64::x86_64_physical_register_model())
            .unwrap()
    }

    #[test]
    fn frame_bytes_shift_selected_rows_and_internal_call_fixups_exactly() {
        let source = source_plan();
        let machine = source.entry;
        let application = apply(
            &source,
            FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest"),
            &protocol(machine),
            &physical(),
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
        assert_eq!(application.functions[0].epilogues.len(), 1);
        assert_eq!(application.functions[0].epilogues[0].function_offset, 6);
        assert_eq!(application.functions[0].epilogues[0].byte_count, 1);
        assert_eq!(application.identity, application.recomputed_identity());
        assert_eq!(
            application.fragments.identity,
            application.fragments.recomputed_identity()
        );
    }

    #[test]
    fn malformed_block_and_nonfinal_return_shapes_fail_closed() {
        let mut source = source_plan();
        let machine = source.entry;
        let repeated = source.functions[0].blocks[0].clone();
        source.functions[0].blocks.push(repeated);
        assert_eq!(
            apply(
                &source,
                FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest"),
                &protocol(machine),
                &physical(),
            ),
            Err(FrameApplicationError::SourceShapeMismatch(machine))
        );

        let mut source = source_plan();
        let controls = &mut source.functions[0].blocks[0].instructions;
        controls[0].control = FunctionFragmentControlProvenance::Return {
            psi_return_edge: EdgeId::new(2).unwrap(),
        };
        controls[1].control = FunctionFragmentControlProvenance::DirectInternalCall {
            callee: MachineId::new(2).unwrap(),
        };
        assert_eq!(
            apply(
                &source,
                FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest"),
                &protocol(machine),
                &physical(),
            ),
            Err(FrameApplicationError::MissingFinalReturn(machine))
        );
    }
}

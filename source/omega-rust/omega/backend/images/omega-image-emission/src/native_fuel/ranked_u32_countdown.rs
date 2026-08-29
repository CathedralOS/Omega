//! Independent replay for charge-interleaved ranked semantic branches.

use omega_machine_code::{MachineCodeFunction, NativeFuelRankedU32CountdownRebaseRecord};
use omega_target::NativeTarget;
use psi_core::MachineId;
use psi_diagnostics::Diagnostic;

use super::{NativeFuelValidationError, ValidatedNativeFuelArtifact};
use crate::ObjectArtifact;

pub(super) fn classify(artifact: &ObjectArtifact) -> Option<MachineId> {
    artifact
        .functions()
        .iter()
        .find(|function| function.ranked_u32_countdown.is_some())
        .map(|function| function.machine)
}

pub(super) fn replay_rebased_branches(
    target: NativeTarget,
    source: &MachineCodeFunction,
    expected: &mut [u8],
    supplied: &[u8],
) -> Result<NativeFuelRankedU32CountdownRebaseRecord, NativeFuelValidationError> {
    let invalid = || NativeFuelValidationError::InvalidRankedCountdownRebasing(source.machine);
    let (source_preheader, source_header, source_exit_branch, source_backedge, source_exit) =
        if target == NativeTarget::linux_x64() {
            let decoded =
                omega_isa_x86_64::validate_x86_64_ranked_u32_countdown_in_edi(&source.bytes)
                    .map_err(|_| invalid())?;
            let layout = decoded.layout();
            (
                layout.preheader_branch().0,
                layout.header_offset(),
                layout.compare().0 + layout.compare().1,
                layout.backward_branch().0,
                layout.exit_offset(),
            )
        } else if target == NativeTarget::linux_arm64() {
            let decoded =
                omega_isa_aarch64::validate_aarch64_ranked_u32_countdown_in_w0(&source.bytes)
                    .map_err(|_| invalid())?;
            let layout = decoded.layout();
            (
                layout.preheader_branch().0,
                layout.header_offset(),
                layout.compare().0 + layout.compare().1,
                layout.backward_branch().0,
                layout.exit_offset(),
            )
        } else {
            return Err(invalid());
        };
    let hot_size = if target == NativeTarget::linux_x64() {
        omega_isa_x86_64::X86_NATIVE_FUEL_CHARGE_BYTE_COUNT
    } else {
        omega_isa_aarch64::AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT
    };
    let semantic = |offset| translate(offset, hot_size, &source.fuel_attribution, false);
    let entry = |offset| translate(offset, hot_size, &source.fuel_attribution, true);
    let record = NativeFuelRankedU32CountdownRebaseRecord {
        preheader_branch_code_offset: semantic(source_preheader).ok_or_else(invalid)?,
        header_charge_code_offset: entry(source_header).ok_or_else(invalid)?,
        exit_branch_code_offset: semantic(source_exit_branch).ok_or_else(invalid)?,
        exit_charge_code_offset: entry(source_exit).ok_or_else(invalid)?,
        backward_branch_code_offset: semantic(source_backedge).ok_or_else(invalid)?,
    };
    if target == NativeTarget::linux_x64() {
        let layout = omega_isa_x86_64::X86_64RankedU32CountdownRebasedBranchLayout {
            preheader_branch_offset: record.preheader_branch_code_offset,
            header_charge_offset: record.header_charge_code_offset,
            exit_branch_offset: record.exit_branch_code_offset,
            exit_charge_offset: record.exit_charge_code_offset,
            backward_branch_offset: record.backward_branch_code_offset,
        };
        omega_isa_x86_64::validate_x86_64_rebased_ranked_u32_countdown_branches(supplied, layout)
            .map_err(|_| invalid())?;
        accept(expected, supplied, layout.preheader_branch_offset, 5).ok_or_else(invalid)?;
        accept(expected, supplied, layout.exit_branch_offset, 6).ok_or_else(invalid)?;
        accept(expected, supplied, layout.backward_branch_offset, 5).ok_or_else(invalid)?;
    } else {
        let layout = omega_isa_aarch64::Aarch64RankedU32CountdownRebasedBranchLayout {
            preheader_branch_offset: record.preheader_branch_code_offset,
            header_charge_offset: record.header_charge_code_offset,
            exit_branch_offset: record.exit_branch_code_offset,
            exit_charge_offset: record.exit_charge_code_offset,
            backward_branch_offset: record.backward_branch_code_offset,
        };
        omega_isa_aarch64::validate_aarch64_rebased_ranked_u32_countdown_branches(supplied, layout)
            .map_err(|_| invalid())?;
        accept(expected, supplied, layout.preheader_branch_offset, 4).ok_or_else(invalid)?;
        accept(expected, supplied, layout.exit_branch_offset, 4).ok_or_else(invalid)?;
        accept(expected, supplied, layout.backward_branch_offset, 4).ok_or_else(invalid)?;
    }
    Ok(record)
}

pub(super) fn reject_final_image(artifact: &ValidatedNativeFuelArtifact) -> Result<(), Diagnostic> {
    if let Some(function) = artifact
        .functions()
        .iter()
        .find(|function| function.ranked_u32_countdown.is_some())
    {
        return Err(Diagnostic::error(format!(
            "ranked-u32 native-fuel function {} has metered object custody but no final-image replay",
            function.machine
        )));
    }
    Ok(())
}

fn translate(
    source_offset: usize,
    hot_size: usize,
    rows: &[omega_machine_code::NativeFuelAttribution],
    site_entry: bool,
) -> Option<usize> {
    let preceding = rows.partition_point(|row| {
        if site_entry {
            row.code_offset < source_offset
        } else {
            row.code_offset <= source_offset
        }
    });
    source_offset.checked_add(hot_size.checked_mul(preceding)?)
}

fn accept(expected: &mut [u8], supplied: &[u8], offset: usize, count: usize) -> Option<()> {
    expected
        .get_mut(offset..offset.checked_add(count)?)?
        .copy_from_slice(supplied.get(offset..offset.checked_add(count)?)?);
    Some(())
}

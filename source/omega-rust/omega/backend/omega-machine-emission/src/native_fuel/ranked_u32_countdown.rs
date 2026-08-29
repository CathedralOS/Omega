//! Producer-side branch rebasing for the exact ranked `u32` carrier.

use omega_machine_code::{
    MachineCodeFunction, MachineCodePlan, NativeFuelRankedU32CountdownRebaseRecord,
};
use omega_target::NativeTarget;
use psi_core::MachineId;

use super::general::NativeFuelInstrumentationError;

pub(super) fn classify(
    plan: &MachineCodePlan,
) -> Result<Option<MachineId>, NativeFuelInstrumentationError> {
    let mut ranked = None;
    for function in &plan.functions {
        if function.ranked_u32_countdown.is_some() {
            if ranked.replace(function.machine).is_some()
                || plan.functions.len() != 1
                || plan.entry != function.machine
            {
                return Err(NativeFuelInstrumentationError::InvalidRankedCountdownPlan(
                    function.machine,
                ));
            }
        } else if function.requires_ranked_countdown_replay() {
            return Err(
                NativeFuelInstrumentationError::MissingRankedCountdownCustody(function.machine),
            );
        }
    }
    Ok(ranked)
}

pub(super) fn rebase(
    target: NativeTarget,
    function: &MachineCodeFunction,
    metered_bytes: &mut [u8],
) -> Result<NativeFuelRankedU32CountdownRebaseRecord, NativeFuelInstrumentationError> {
    let invalid =
        || NativeFuelInstrumentationError::InvalidRankedCountdownRebasing(function.machine);
    let (source_preheader, source_header, source_exit_branch, source_backedge, source_exit) =
        if target == NativeTarget::linux_x64() {
            let decoded =
                omega_isa_x86_64::validate_x86_64_ranked_u32_countdown_in_edi(&function.bytes)
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
                omega_isa_aarch64::validate_aarch64_ranked_u32_countdown_in_w0(&function.bytes)
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
    let semantic = |offset| translate(offset, hot_size, &function.fuel_attribution, false);
    let entry = |offset| translate(offset, hot_size, &function.fuel_attribution, true);
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
        let branches =
            omega_isa_x86_64::encode_x86_64_rebased_ranked_u32_countdown_branches(layout)
                .map_err(|_| invalid())?;
        replace(
            metered_bytes,
            layout.preheader_branch_offset,
            &branches.preheader(),
        )
        .ok_or_else(invalid)?;
        replace(metered_bytes, layout.exit_branch_offset, &branches.exit()).ok_or_else(invalid)?;
        replace(
            metered_bytes,
            layout.backward_branch_offset,
            &branches.backward(),
        )
        .ok_or_else(invalid)?;
    } else {
        let layout = omega_isa_aarch64::Aarch64RankedU32CountdownRebasedBranchLayout {
            preheader_branch_offset: record.preheader_branch_code_offset,
            header_charge_offset: record.header_charge_code_offset,
            exit_branch_offset: record.exit_branch_code_offset,
            exit_charge_offset: record.exit_charge_code_offset,
            backward_branch_offset: record.backward_branch_code_offset,
        };
        let branches =
            omega_isa_aarch64::encode_aarch64_rebased_ranked_u32_countdown_branches(layout)
                .map_err(|_| invalid())?;
        replace(
            metered_bytes,
            layout.preheader_branch_offset,
            &branches.preheader(),
        )
        .ok_or_else(invalid)?;
        replace(metered_bytes, layout.exit_branch_offset, &branches.exit()).ok_or_else(invalid)?;
        replace(
            metered_bytes,
            layout.backward_branch_offset,
            &branches.backward(),
        )
        .ok_or_else(invalid)?;
    }
    Ok(record)
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

fn replace(bytes: &mut [u8], offset: usize, replacement: &[u8]) -> Option<()> {
    bytes
        .get_mut(offset..offset.checked_add(replacement.len())?)?
        .copy_from_slice(replacement);
    Some(())
}

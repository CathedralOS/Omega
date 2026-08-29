//! Target-decoder-owned reconstruction of ranked native-fuel branch coordinates.

use omega_machine_code::{NativeFuelAttribution, NativeFuelRankedU32CountdownRebaseRecord};
use omega_target::NativeTarget;

pub(super) fn reconstruct(
    target: NativeTarget,
    source_bytes: &[u8],
    source_fuel: &[NativeFuelAttribution],
) -> Option<NativeFuelRankedU32CountdownRebaseRecord> {
    let (source_preheader, source_header, source_exit_branch, source_backedge, source_exit) =
        if target == NativeTarget::linux_x64() {
            let decoded =
                omega_isa_x86_64::validate_x86_64_ranked_u32_countdown_in_edi(source_bytes).ok()?;
            let layout = decoded.layout();
            (
                layout.preheader_branch().0,
                layout.header_offset(),
                layout.compare().0.checked_add(layout.compare().1)?,
                layout.backward_branch().0,
                layout.exit_offset(),
            )
        } else if target == NativeTarget::linux_arm64() {
            let decoded =
                omega_isa_aarch64::validate_aarch64_ranked_u32_countdown_in_w0(source_bytes)
                    .ok()?;
            let layout = decoded.layout();
            (
                layout.preheader_branch().0,
                layout.header_offset(),
                layout.compare().0.checked_add(layout.compare().1)?,
                layout.backward_branch().0,
                layout.exit_offset(),
            )
        } else {
            return None;
        };
    let hot_size = if target == NativeTarget::linux_x64() {
        omega_isa_x86_64::X86_NATIVE_FUEL_CHARGE_BYTE_COUNT
    } else {
        omega_isa_aarch64::AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT
    };
    let semantic = |offset| translate(offset, hot_size, source_fuel, false);
    let entry = |offset| translate(offset, hot_size, source_fuel, true);
    Some(NativeFuelRankedU32CountdownRebaseRecord {
        preheader_branch_code_offset: semantic(source_preheader)?,
        header_charge_code_offset: entry(source_header)?,
        exit_branch_code_offset: semantic(source_exit_branch)?,
        exit_charge_code_offset: entry(source_exit)?,
        backward_branch_code_offset: semantic(source_backedge)?,
    })
}

pub(super) fn validate_final_branches(
    target: NativeTarget,
    bytes: &[u8],
    record: NativeFuelRankedU32CountdownRebaseRecord,
) -> bool {
    if target == NativeTarget::linux_x64() {
        omega_isa_x86_64::validate_x86_64_rebased_ranked_u32_countdown_branches(
            bytes,
            omega_isa_x86_64::X86_64RankedU32CountdownRebasedBranchLayout {
                preheader_branch_offset: record.preheader_branch_code_offset,
                header_charge_offset: record.header_charge_code_offset,
                exit_branch_offset: record.exit_branch_code_offset,
                exit_charge_offset: record.exit_charge_code_offset,
                backward_branch_offset: record.backward_branch_code_offset,
            },
        )
        .is_ok()
    } else if target == NativeTarget::linux_arm64() {
        omega_isa_aarch64::validate_aarch64_rebased_ranked_u32_countdown_branches(
            bytes,
            omega_isa_aarch64::Aarch64RankedU32CountdownRebasedBranchLayout {
                preheader_branch_offset: record.preheader_branch_code_offset,
                header_charge_offset: record.header_charge_code_offset,
                exit_branch_offset: record.exit_branch_code_offset,
                exit_charge_offset: record.exit_charge_code_offset,
                backward_branch_offset: record.backward_branch_code_offset,
            },
        )
        .is_ok()
    } else {
        false
    }
}

pub(super) fn branch_spans(
    target: NativeTarget,
    record: NativeFuelRankedU32CountdownRebaseRecord,
) -> Option<[(usize, usize); 3]> {
    if target.architecture == omega_target::Architecture::X86_64 {
        Some([
            (record.preheader_branch_code_offset, 5),
            (record.exit_branch_code_offset, 6),
            (record.backward_branch_code_offset, 5),
        ])
    } else if target.architecture == omega_target::Architecture::Aarch64 {
        Some([
            (record.preheader_branch_code_offset, 4),
            (record.exit_branch_code_offset, 4),
            (record.backward_branch_code_offset, 4),
        ])
    } else {
        None
    }
}

fn translate(
    source_offset: usize,
    hot_size: usize,
    rows: &[NativeFuelAttribution],
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

#[cfg(test)]
mod tests {
    use omega_machine_code::{NativeFuelAttribution, NativeFuelSite};
    use psi_core::OperationId;

    use super::*;

    fn fuel(offsets: [usize; 9]) -> Vec<NativeFuelAttribution> {
        offsets
            .into_iter()
            .enumerate()
            .map(|(ordinal, code_offset)| NativeFuelAttribution {
                schedule: psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                site: NativeFuelSite::Operation(
                    OperationId::new(u64::try_from(ordinal + 1).expect("small ordinal"))
                        .expect("nonzero operation"),
                ),
                units: 1,
                operation_ordinal: ordinal,
                code_offset,
                byte_count: usize::from(ordinal == 8),
            })
            .collect()
    }

    #[test]
    fn reconstructs_and_decodes_both_ranked_target_coordinate_sets() {
        let cases = [
            (
                NativeTarget::linux_x64(),
                omega_isa_x86_64::encode_ranked_u32_countdown_in_edi().to_vec(),
                fuel([0, 5, 5, 13, 13, 13, 15, 20, 20]),
                NativeFuelRankedU32CountdownRebaseRecord {
                    preheader_branch_code_offset: 36,
                    header_charge_code_offset: 41,
                    exit_branch_code_offset: 115,
                    exit_charge_code_offset: 272,
                    backward_branch_code_offset: 267,
                },
                345,
            ),
            (
                NativeTarget::linux_arm64(),
                omega_isa_aarch64::encode_ranked_u32_countdown_in_w0().to_vec(),
                fuel([0, 4, 4, 12, 12, 12, 16, 20, 20]),
                NativeFuelRankedU32CountdownRebaseRecord {
                    preheader_branch_code_offset: 36,
                    header_charge_code_offset: 40,
                    exit_branch_code_offset: 116,
                    exit_charge_code_offset: 272,
                    backward_branch_code_offset: 268,
                },
                348,
            ),
        ];

        for (target, source, fuel, expected, semantic_end) in cases {
            assert_eq!(reconstruct(target, &source, &fuel), Some(expected));
            let mut metered = vec![0_u8; semantic_end];
            if target == NativeTarget::linux_x64() {
                let layout = omega_isa_x86_64::X86_64RankedU32CountdownRebasedBranchLayout {
                    preheader_branch_offset: expected.preheader_branch_code_offset,
                    header_charge_offset: expected.header_charge_code_offset,
                    exit_branch_offset: expected.exit_branch_code_offset,
                    exit_charge_offset: expected.exit_charge_code_offset,
                    backward_branch_offset: expected.backward_branch_code_offset,
                };
                let branches =
                    omega_isa_x86_64::encode_x86_64_rebased_ranked_u32_countdown_branches(layout)
                        .expect("encode x86 branches");
                install(
                    &mut metered,
                    expected.preheader_branch_code_offset,
                    &branches.preheader(),
                );
                install(
                    &mut metered,
                    expected.exit_branch_code_offset,
                    &branches.exit(),
                );
                install(
                    &mut metered,
                    expected.backward_branch_code_offset,
                    &branches.backward(),
                );
            } else {
                let layout = omega_isa_aarch64::Aarch64RankedU32CountdownRebasedBranchLayout {
                    preheader_branch_offset: expected.preheader_branch_code_offset,
                    header_charge_offset: expected.header_charge_code_offset,
                    exit_branch_offset: expected.exit_branch_code_offset,
                    exit_charge_offset: expected.exit_charge_code_offset,
                    backward_branch_offset: expected.backward_branch_code_offset,
                };
                let branches =
                    omega_isa_aarch64::encode_aarch64_rebased_ranked_u32_countdown_branches(layout)
                        .expect("encode AArch64 branches");
                install(
                    &mut metered,
                    expected.preheader_branch_code_offset,
                    &branches.preheader(),
                );
                install(
                    &mut metered,
                    expected.exit_branch_code_offset,
                    &branches.exit(),
                );
                install(
                    &mut metered,
                    expected.backward_branch_code_offset,
                    &branches.backward(),
                );
            }
            assert!(validate_final_branches(target, &metered, expected));
            metered[expected.backward_branch_code_offset] ^= 1;
            assert!(!validate_final_branches(target, &metered, expected));
        }
    }

    fn install(bytes: &mut [u8], offset: usize, fragment: &[u8]) {
        bytes[offset..offset + fragment.len()].copy_from_slice(fragment);
    }
}

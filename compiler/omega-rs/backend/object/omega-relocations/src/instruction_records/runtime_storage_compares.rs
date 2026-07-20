use super::super::offsets::runtime_storage_compare_right_address_offset;
use super::context::InstructionRelocationContext;
use super::runtime_values::collect_runtime_value_operand_relocations;
use omega_instruction_selection::dispatch_guard_compare_static_width;
use omega_target_operations::{SelectedInstructionKind, StateGuardLowering, StateGuardOperator};

pub(super) fn collect_runtime_storage_compare_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator:
                StateGuardOperator::Equal
                | StateGuardOperator::NotEqual
                | StateGuardOperator::Greater
                | StateGuardOperator::GreaterOrEqual
                | StateGuardOperator::Less
                | StateGuardOperator::LessOrEqual
                | StateGuardOperator::GreaterUnsigned
                | StateGuardOperator::GreaterOrEqualUnsigned
                | StateGuardOperator::LessUnsigned
                | StateGuardOperator::LessOrEqualUnsigned,
            storage_region,
            byte_offset,
            byte_size,
            has_storage: true,
            is_float,
            ..
        } => {
            // The guard's Absolute64 storage-address relocation only exists when the guard
            // actually emits a storage load with an inline 64-bit address immediate. On targets
            // where `EvaluateDispatchGuard` lowers to a zero-byte instruction (e.g. x86_64, where
            // the comparison is folded into the following `DispatchCaseEnter`'s `cmp r12d, N`),
            // there is no immediate to relocate. The guard's text offset then coincides with the
            // *next* instruction (a `SetDispatchState` / `mov r12d, imm32`), so emitting a
            // relocation here would splatter the 8-byte storage address across that instruction's
            // 4-byte index immediate and corrupt the dispatch index — the `0xC0000005` crash.
            // Only anchor the relocation when the guard occupies real bytes.
            if dispatch_guard_compare_static_width(
                context.input.target.architecture,
                *byte_offset,
                *byte_size,
                *is_float,
            ) != 0
            {
                let symbol = context.storage_region_symbol_handle(*storage_region);
                context.insert_data_address_at_instruction_start(symbol);
            }
            true
        }
        SelectedInstructionKind::ComparePlaces {
            left,
            right,
            byte_size,
            operator,
            is_float,
        } => {
            match context.input.target.architecture {
                omega_target::Architecture::X86_64 => {
                    // Task #131: the LEFT place walks in the Source register
                    // (r14) and the RIGHT in the Target (r15) -- the sites
                    // patch by side + place region, the WritePlaceInteger
                    // discipline extended to two subjects.
                    let (_, sites) =
                        omega_instruction_selection::x86_64_encode_place_compare_with_sites(
                            left, right, *byte_size, *operator, *is_float,
                        )
                        .expect(
                            "ComparePlaces reached relocation with a shape the \
                             materializer refuses; layout/encoding would have failed first",
                        );
                    for (byte_offset, side) in sites.iter() {
                        let region = match side {
                            omega_instruction_selection::PlaceCopySide::Source => left.region,
                            omega_instruction_selection::PlaceCopySide::SourceIndex => left
                                .scaled_index_region()
                                .expect("a SourceIndex site implies a left ScaledIndex step"),
                            omega_instruction_selection::PlaceCopySide::SourceIndex2 => left
                                .scaled_index_regions()
                                .nth(1)
                                .expect("a SourceIndex2 site implies two left ScaledIndex steps"),
                            omega_instruction_selection::PlaceCopySide::Target => right.region,
                            omega_instruction_selection::PlaceCopySide::TargetIndex => right
                                .scaled_index_region()
                                .expect("a TargetIndex site implies a right ScaledIndex step"),
                            omega_instruction_selection::PlaceCopySide::TargetIndex2 => {
                                unreachable!(
                                    "a two-index right compare operand refuses at encoding"
                                )
                            }
                        };
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                }
                omega_target::Architecture::Aarch64 => {
                    // The transitional decompose serves DIRECT places only
                    // (encoding refuses anything else): the retained
                    // storage-compare positions.
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(left.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        runtime_storage_compare_right_address_offset(
                            context.input.target.architecture,
                            *byte_size,
                        ),
                        context.storage_region_symbol_handle(right.region),
                    );
                }
            }
            true
        }
        SelectedInstructionKind::ComparePlaceValue {
            place,
            byte_size,
            expected_value,
            operator,
        } => {
            match context.input.target.architecture {
                omega_target::Architecture::X86_64 => {
                    let (_, sites) =
                        omega_instruction_selection::x86_64_encode_place_value_compare_with_sites(
                            place,
                            *byte_size,
                            *expected_value,
                            *operator,
                        )
                        .expect(
                            "ComparePlaceValue reached relocation with a shape the \
                             materializer refuses; layout/encoding would have failed first",
                        );
                    for (byte_offset, side) in sites.iter() {
                        let region = match side {
                            omega_instruction_selection::PlaceCopySide::Target => place.region,
                            omega_instruction_selection::PlaceCopySide::TargetIndex => place
                                .scaled_index_region()
                                .expect("a TargetIndex site implies a ScaledIndex step"),
                            omega_instruction_selection::PlaceCopySide::TargetIndex2 => place
                                .scaled_index_regions()
                                .nth(1)
                                .expect("a TargetIndex2 site implies two ScaledIndex steps"),
                            _ => {
                                unreachable!("a value compare materializes only its subject place")
                            }
                        };
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                }
                omega_target::Architecture::Aarch64 => {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(place.region),
                    );
                }
            }
            true
        }
        SelectedInstructionKind::CompareRuntimeValues { left, right, .. } => {
            let base_offset = context.selected_text_offset;
            collect_runtime_value_operand_relocations(context, base_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                context.input.target.architecture,
                context.input.assigned_target_operations,
                *left,
            );
            collect_runtime_value_operand_relocations(context, base_offset + left_width, *right);
            true
        }
        _ => false,
    }
}

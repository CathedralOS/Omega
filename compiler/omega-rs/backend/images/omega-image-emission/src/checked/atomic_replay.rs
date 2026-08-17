//! Replays compiler atomic operations and derives recursive operand storage sites.

use super::*;

pub(super) fn compiler_runtime_value_compare_address_sites(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    left: omega_target_operations::RuntimeValueOperandHandle,
    right: omega_target_operations::RuntimeValueOperandHandle,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let mut sites = Vec::new();
    let mut visiting = Vec::new();
    collect_compiler_runtime_value_address_sites(
        architecture,
        operands,
        left,
        0,
        &mut visiting,
        &mut sites,
    )?;
    let right_offset = compiler_runtime_value_operand_width(architecture, operands, left)?;
    collect_compiler_runtime_value_address_sites(
        architecture,
        operands,
        right,
        right_offset,
        &mut visiting,
        &mut sites,
    )?;
    Ok(sites)
}

pub(super) fn replay_compiler_atomic_operation(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    operation: omega_machine_bytes::CompilerInstructionAtomicOperation,
) -> Result<
    (
        Vec<u8>,
        u8,
        Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    ),
    Diagnostic,
> {
    use omega_machine_bytes::CompilerInstructionAtomicOperation;

    let operand_start = match architecture {
        Architecture::X86_64 => 10,
        Architecture::Aarch64 => 8,
    };
    let mut sites = Vec::new();

    let (bytes, validation_tag) = match operation {
        CompilerInstructionAtomicOperation::Load {
            source_region,
            source_offset,
            byte_size,
            result_region,
            result_offset,
            ordering,
        } => {
            sites.push((0, source_region));
            sites.push((
                match architecture {
                    Architecture::X86_64 => {
                        omega_isa_x86_64::runtime_atomic_load_result_address_offset(byte_size)
                    }
                    Architecture::Aarch64 => {
                        omega_isa_aarch64::runtime_atomic_load_result_address_offset(source_offset)
                    }
                },
                result_region,
            ));
            let psi_language_core::AtomicOrderingPlan::Load(load_ordering) = ordering else {
                return Err(Diagnostic::error(
                    "final atomic-load replay retained a non-load ordering plan",
                ));
            };
            (
                match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::encode_atomic_load_to_storage(
                        source_offset,
                        byte_size,
                        result_offset,
                    )?,
                    Architecture::Aarch64 => omega_isa_aarch64::encode_atomic_load_to_storage(
                        source_offset,
                        byte_size,
                        result_offset,
                        load_ordering,
                    )?,
                },
                76,
            )
        }
        CompilerInstructionAtomicOperation::Store {
            target_region,
            target_offset,
            byte_size,
            value,
            ordering,
        } => {
            sites.push((0, target_region));
            collect_compiler_atomic_operand_address_sites(
                architecture,
                operands,
                value,
                operand_start,
                &mut sites,
            )?;
            let psi_language_core::AtomicOrderingPlan::Store(store_ordering) = ordering else {
                return Err(Diagnostic::error(
                    "final atomic-store replay retained a non-store ordering plan",
                ));
            };
            (
                match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::encode_atomic_store_from_operand(
                        operands,
                        target_offset,
                        byte_size,
                        value,
                        store_ordering == psi_language_core::MemoryOrdering::GlobalOrder,
                    )?,
                    Architecture::Aarch64 => omega_isa_aarch64::encode_atomic_store_from_operand(
                        operands,
                        target_offset,
                        byte_size,
                        value,
                        store_ordering,
                    )?,
                },
                77,
            )
        }
        CompilerInstructionAtomicOperation::FetchAdd {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            delta,
            ordering,
        } => {
            sites.push((0, target_region));
            collect_compiler_atomic_operand_address_sites(
                architecture,
                operands,
                delta,
                operand_start,
                &mut sites,
            )?;
            sites.push((
                match architecture {
                    Architecture::X86_64 => {
                        omega_isa_x86_64::runtime_atomic_fetch_add_result_address_offset(
                            operands, byte_size, delta,
                        )
                    }
                    Architecture::Aarch64 => {
                        omega_isa_aarch64::runtime_atomic_fetch_add_result_address_offset(
                            operands,
                            target_offset,
                            delta,
                        )
                    }
                },
                result_region,
            ));
            let psi_language_core::AtomicOrderingPlan::ReadModifyWrite(rmw_ordering) = ordering
            else {
                return Err(Diagnostic::error(
                    "final atomic fetch-add replay retained a non-RMW ordering plan",
                ));
            };
            (
                match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::encode_atomic_fetch_add(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        delta,
                    )?,
                    Architecture::Aarch64 => omega_isa_aarch64::encode_atomic_fetch_add(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        delta,
                        rmw_ordering,
                    )?,
                },
                78,
            )
        }
        CompilerInstructionAtomicOperation::FetchSub {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            delta,
            ordering,
        } => {
            sites.push((0, target_region));
            collect_compiler_atomic_operand_address_sites(
                architecture,
                operands,
                delta,
                operand_start,
                &mut sites,
            )?;
            sites.push((
                match architecture {
                    Architecture::X86_64 => {
                        omega_isa_x86_64::runtime_atomic_fetch_sub_result_address_offset(
                            operands, byte_size, delta,
                        )
                    }
                    Architecture::Aarch64 => {
                        omega_isa_aarch64::runtime_atomic_fetch_sub_result_address_offset(
                            operands,
                            target_offset,
                            delta,
                        )
                    }
                },
                result_region,
            ));
            let psi_language_core::AtomicOrderingPlan::ReadModifyWrite(rmw_ordering) = ordering
            else {
                return Err(Diagnostic::error(
                    "final atomic fetch-sub replay retained a non-RMW ordering plan",
                ));
            };
            (
                match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::encode_atomic_fetch_sub(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        delta,
                    )?,
                    Architecture::Aarch64 => omega_isa_aarch64::encode_atomic_fetch_sub(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        delta,
                        rmw_ordering,
                    )?,
                },
                79,
            )
        }
        CompilerInstructionAtomicOperation::FetchXor {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            value,
            ordering,
        } => {
            sites.push((0, target_region));
            collect_compiler_atomic_operand_address_sites(
                architecture,
                operands,
                value,
                operand_start,
                &mut sites,
            )?;
            sites.push((
                match architecture {
                    Architecture::X86_64 => {
                        omega_isa_x86_64::runtime_atomic_fetch_xor_result_address_offset(
                            operands, byte_size, value,
                        )
                    }
                    Architecture::Aarch64 => {
                        omega_isa_aarch64::runtime_atomic_fetch_xor_result_address_offset(
                            operands,
                            target_offset,
                            value,
                        )
                    }
                },
                result_region,
            ));
            let psi_language_core::AtomicOrderingPlan::ReadModifyWrite(rmw_ordering) = ordering
            else {
                return Err(Diagnostic::error(
                    "final atomic fetch-xor replay retained a non-RMW ordering plan",
                ));
            };
            (
                match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::encode_atomic_fetch_xor(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        value,
                    )?,
                    Architecture::Aarch64 => omega_isa_aarch64::encode_atomic_fetch_xor(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        value,
                        rmw_ordering,
                    )?,
                },
                80,
            )
        }
        CompilerInstructionAtomicOperation::FetchOr {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            value,
            ordering,
        } => {
            sites.push((0, target_region));
            collect_compiler_atomic_operand_address_sites(
                architecture,
                operands,
                value,
                operand_start,
                &mut sites,
            )?;
            sites.push((
                match architecture {
                    Architecture::X86_64 => {
                        omega_isa_x86_64::runtime_atomic_fetch_or_result_address_offset(
                            operands, byte_size, value,
                        )
                    }
                    Architecture::Aarch64 => {
                        omega_isa_aarch64::runtime_atomic_fetch_or_result_address_offset(
                            operands,
                            target_offset,
                            value,
                        )
                    }
                },
                result_region,
            ));
            let psi_language_core::AtomicOrderingPlan::ReadModifyWrite(rmw_ordering) = ordering
            else {
                return Err(Diagnostic::error(
                    "final atomic fetch-or replay retained a non-RMW ordering plan",
                ));
            };
            (
                match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::encode_atomic_fetch_or(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        value,
                    )?,
                    Architecture::Aarch64 => omega_isa_aarch64::encode_atomic_fetch_or(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        value,
                        rmw_ordering,
                    )?,
                },
                81,
            )
        }
        CompilerInstructionAtomicOperation::FetchAnd {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            value,
            ordering,
        } => {
            sites.push((0, target_region));
            collect_compiler_atomic_operand_address_sites(
                architecture,
                operands,
                value,
                operand_start,
                &mut sites,
            )?;
            sites.push((
                match architecture {
                    Architecture::X86_64 => {
                        omega_isa_x86_64::runtime_atomic_fetch_and_result_address_offset(
                            operands, byte_size, value,
                        )
                    }
                    Architecture::Aarch64 => {
                        omega_isa_aarch64::runtime_atomic_fetch_and_result_address_offset(
                            operands,
                            target_offset,
                            value,
                        )
                    }
                },
                result_region,
            ));
            let psi_language_core::AtomicOrderingPlan::ReadModifyWrite(rmw_ordering) = ordering
            else {
                return Err(Diagnostic::error(
                    "final atomic fetch-and replay retained a non-RMW ordering plan",
                ));
            };
            (
                match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::encode_atomic_fetch_and(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        value,
                    )?,
                    Architecture::Aarch64 => omega_isa_aarch64::encode_atomic_fetch_and(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        value,
                        rmw_ordering,
                    )?,
                },
                82,
            )
        }
        CompilerInstructionAtomicOperation::Swap {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            new_value,
            ordering,
        } => {
            sites.push((0, target_region));
            collect_compiler_atomic_operand_address_sites(
                architecture,
                operands,
                new_value,
                operand_start,
                &mut sites,
            )?;
            sites.push((
                match architecture {
                    Architecture::X86_64 => {
                        omega_isa_x86_64::runtime_atomic_swap_result_address_offset(
                            operands, byte_size, new_value,
                        )
                    }
                    Architecture::Aarch64 => {
                        omega_isa_aarch64::runtime_atomic_swap_result_address_offset(
                            operands,
                            target_offset,
                            new_value,
                        )
                    }
                },
                result_region,
            ));
            let psi_language_core::AtomicOrderingPlan::Swap(swap_ordering) = ordering else {
                return Err(Diagnostic::error(
                    "final atomic-swap replay retained a non-swap ordering plan",
                ));
            };
            (
                match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::encode_atomic_swap(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        new_value,
                    )?,
                    Architecture::Aarch64 => omega_isa_aarch64::encode_atomic_swap(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        new_value,
                        swap_ordering,
                    )?,
                },
                83,
            )
        }
        CompilerInstructionAtomicOperation::CompareExchange {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            expected,
            new_value,
            ordering,
        } => {
            sites.push((0, target_region));
            collect_compiler_atomic_operand_address_sites(
                architecture,
                operands,
                new_value,
                operand_start,
                &mut sites,
            )?;
            let expected_offset = operand_start
                + compiler_runtime_value_operand_width(architecture, operands, new_value)?
                + match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::BINARY_RIGHT_OPERAND_PUSH_WIDTH,
                    Architecture::Aarch64 => 0,
                };
            collect_compiler_atomic_operand_address_sites(
                architecture,
                operands,
                expected,
                expected_offset,
                &mut sites,
            )?;
            sites.push((
                match architecture {
                    Architecture::X86_64 => {
                        omega_isa_x86_64::runtime_atomic_compare_exchange_result_address_offset(
                            operands, byte_size, expected, new_value,
                        )
                    }
                    Architecture::Aarch64 => {
                        omega_isa_aarch64::runtime_atomic_compare_exchange_result_address_offset(
                            operands,
                            target_offset,
                            expected,
                            new_value,
                        )
                    }
                },
                result_region,
            ));
            let psi_language_core::AtomicOrderingPlan::CompareExchange { success, .. } = ordering
            else {
                return Err(Diagnostic::error(
                    "final atomic compare-exchange replay retained a non-CAS ordering plan",
                ));
            };
            (
                match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::encode_atomic_compare_exchange(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        expected,
                        new_value,
                    )?,
                    Architecture::Aarch64 => omega_isa_aarch64::encode_atomic_compare_exchange(
                        operands,
                        target_offset,
                        byte_size,
                        result_offset,
                        expected,
                        new_value,
                        success,
                    )?,
                },
                84,
            )
        }
    };
    Ok((bytes, validation_tag, sites))
}

fn collect_compiler_atomic_operand_address_sites(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    operand: omega_target_operations::RuntimeValueOperandHandle,
    operand_offset: usize,
    sites: &mut Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
) -> Result<(), Diagnostic> {
    collect_compiler_runtime_value_address_sites(
        architecture,
        operands,
        operand,
        operand_offset,
        &mut Vec::new(),
        sites,
    )
}

pub(super) fn collect_compiler_runtime_value_address_sites(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    operand_handle: omega_target_operations::RuntimeValueOperandHandle,
    operand_offset: usize,
    visiting: &mut Vec<omega_target_operations::RuntimeValueOperandHandle>,
    sites: &mut Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
) -> Result<(), Diagnostic> {
    use omega_target_operations::{RuntimeStorageRegion, RuntimeValueOperand};

    if !operands.is_valid(operand_handle) {
        return Err(Diagnostic::error(
            "final runtime-value guard retained an invalid operand handle",
        ));
    }
    if visiting.contains(&operand_handle) {
        return Err(Diagnostic::error(
            "final runtime-value guard retained a cyclic operand graph",
        ));
    }
    visiting.push(operand_handle);
    match operands.get(operand_handle) {
        RuntimeValueOperand::Immediate(_) => {}
        RuntimeValueOperand::Storage { region, .. }
        | RuntimeValueOperand::BitField { region, .. } => {
            sites.push((operand_offset, *region));
        }
        RuntimeValueOperand::Pointee { .. }
        | RuntimeValueOperand::FrameBaseIndexed { .. }
        | RuntimeValueOperand::FrameFixedIndexed { .. } => {
            sites.push((operand_offset, RuntimeStorageRegion::RuntimeFrame));
        }
        RuntimeValueOperand::FrameIndexed { index_region, .. } => {
            sites.push((operand_offset, RuntimeStorageRegion::RuntimeFrame));
            if *index_region == RuntimeStorageRegion::Machine {
                sites.push((
                    operand_offset
                        + match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET
                            }
                        },
                    RuntimeStorageRegion::Machine,
                ));
            }
        }
        RuntimeValueOperand::MachineIndexed { index_region, .. } => {
            sites.push((operand_offset, RuntimeStorageRegion::Machine));
            if *index_region == RuntimeStorageRegion::RuntimeFrame {
                sites.push((
                    operand_offset
                        + match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::MACHINE_INDEXED_OPERAND_FRAME_INDEX_BASE_OFFSET
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::MACHINE_INDEXED_OPERAND_FRAME_INDEX_BASE_OFFSET
                            }
                        },
                    RuntimeStorageRegion::RuntimeFrame,
                ));
            }
        }
        RuntimeValueOperand::Binary { left, right, .. } => {
            collect_compiler_runtime_value_address_sites(
                architecture,
                operands,
                *left,
                operand_offset,
                visiting,
                sites,
            )?;
            let left_width = compiler_runtime_value_operand_width(architecture, operands, *left)?;
            let right_gap = match architecture {
                Architecture::X86_64 => omega_isa_x86_64::BINARY_RIGHT_OPERAND_PUSH_WIDTH,
                Architecture::Aarch64 => 0,
            };
            collect_compiler_runtime_value_address_sites(
                architecture,
                operands,
                *right,
                operand_offset + left_width + right_gap,
                visiting,
                sites,
            )?;
        }
        RuntimeValueOperand::TextEquals {
            left_region,
            right_region,
            ..
        } => {
            sites.push((operand_offset, *left_region));
            sites.push((
                operand_offset
                    + match architecture {
                        Architecture::X86_64 => {
                            omega_isa_x86_64::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET
                        }
                    },
                *right_region,
            ));
        }
        RuntimeValueOperand::TextEqualsLiteral { place, .. } => {
            if !operands.is_valid(*place) {
                return Err(Diagnostic::error(
                    "final runtime-value text-literal operand retained an invalid place handle",
                ));
            }
            collect_compiler_runtime_value_address_sites(
                architecture,
                operands,
                *place,
                operand_offset,
                visiting,
                sites,
            )?;
        }
        RuntimeValueOperand::Convert { source, .. } => {
            collect_compiler_runtime_value_address_sites(
                architecture,
                operands,
                *source,
                operand_offset,
                visiting,
                sites,
            )?;
        }
    }
    visiting.pop();
    Ok(())
}

pub(super) fn compiler_runtime_value_operand_width(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    operand: omega_target_operations::RuntimeValueOperandHandle,
) -> Result<usize, Diagnostic> {
    if !operands.is_valid(operand) {
        return Err(Diagnostic::error(
            "final runtime-value guard retained an invalid operand handle",
        ));
    }
    Ok(match architecture {
        Architecture::X86_64 => omega_isa_x86_64::runtime_value_operand_width(operands, operand),
        Architecture::Aarch64 => omega_isa_aarch64::runtime_value_operand_width(operands, operand),
    })
}

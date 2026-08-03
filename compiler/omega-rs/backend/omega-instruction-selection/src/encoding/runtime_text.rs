use omega_calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, HostBindingMechanism, MachineRegister, ValueLocation,
    ValueShape, validate_call_plan,
};
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;
use omega_target_operations::RuntimeTextReadTarget;
use psi_diagnostics::Diagnostic;

use super::host::normalized_syscall_registers_with_plan;

fn validate_aarch64_runtime_import_plan(
    authoritative_plan: Option<&CallPlan>,
) -> Result<(), Diagnostic> {
    let Some(plan) = authoritative_plan else {
        return Ok(());
    };
    let word = ValueShape::integer(8, 8);
    validate_call_plan(
        plan,
        &CallSignature {
            parameters: vec![word; 3],
            result: Some(word),
        },
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "source-selected runtime import plan does not match the native read/write signature: {error}"
        ))
    })?;
    if plan.policy != CallingPolicy::Aapcs64 {
        return Err(Diagnostic::error(format!(
            "AArch64 runtime import encoder requires AAPCS64, got {:?}",
            plan.policy
        )));
    }
    for (index, placement) in plan.parameters.iter().enumerate() {
        let expected = MachineRegister::Aarch64X(index as u8);
        if !matches!(
            placement.locations.as_slice(),
            [ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 8,
            }] if *register == expected
        ) {
            return Err(Diagnostic::error(format!(
                "AArch64 runtime import parameter {index} requires {expected:?}, got {:?}",
                placement.locations
            )));
        }
    }
    if !matches!(
        plan.result
            .as_ref()
            .map(|result| result.locations.as_slice()),
        Some([ValueLocation::Register {
            register: MachineRegister::Aarch64X(0),
            value_byte_offset: 0,
            byte_size: 8,
        }])
    ) {
        return Err(Diagnostic::error(
            "AArch64 runtime import result requires the canonical x0 placement",
        ));
    }
    Ok(())
}

pub fn encode_runtime_text_literal_compare(
    architecture: Architecture,
    literal: &str,
    failure_branch_distances: impl ExactSizeIterator<Item = isize>,
    delimiter_failure_branch_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_literal_compare(
            literal,
            failure_branch_distances,
            delimiter_failure_branch_distance,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_text_literal_compare(
            literal,
            failure_branch_distances,
            delimiter_failure_branch_distance,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_text_storage_compare_bytes(
    architecture: Architecture,
    source_offset: usize,
    literal_len: usize,
    compare_failure_branch_distance: isize,
    delimiter_failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_storage_compare_bytes(
            source_offset,
            literal_len,
            compare_failure_branch_distance,
            delimiter_failure_branch_distance,
            branch_when_equal,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_text_storage_compare_bytes(
            source_offset,
            literal_len,
            compare_failure_branch_distance,
            branch_when_equal,
        ),
    }
}

pub fn encode_runtime_text_literal_write(
    architecture: Architecture,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_literal_write(literal),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_text_literal_segment_write(
    architecture: Architecture,
    byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_literal_segment_write(byte_offset, literal)
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_text_literal_segment_write(byte_offset, literal)
        }
    }
}

pub fn encode_runtime_text_stored_suffix_append(
    architecture: Architecture,
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_stored_suffix_append(
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_text_stored_suffix_append(
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
        ),
    }
}

pub fn encode_runtime_text_stored_place_append(
    architecture: Architecture,
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_stored_place_append(
            buffer_offset,
            source_offset,
            target_offset,
        ),
        Architecture::X86_64 => {
            let _ = buffer_offset;
            x86_64::encode_runtime_text_stored_place_append(source_offset, target_offset)
        }
    }
}

pub fn encode_runtime_text_stored_place_append_to_runtime_pointee(
    architecture: Architecture,
    buffer_offset: usize,
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_stored_place_append_to_runtime_pointee(
                buffer_offset,
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => {
            let _ = buffer_offset;
            x86_64::encode_runtime_text_stored_place_append_to_runtime_pointee(
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
            )
        }
    }
}

pub fn encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
    architecture: Architecture,
    buffer_offset: usize,
    source_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
                buffer_offset,
                source_offset,
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_text_literal_append(
    architecture: Architecture,
    buffer_offset: usize,
    target_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_literal_append(buffer_offset, target_offset, literal)
        }
        Architecture::X86_64 => {
            let _ = buffer_offset;
            x86_64::encode_runtime_text_literal_append(target_offset, literal)
        }
    }
}

pub fn encode_runtime_text_literal_append_to_runtime_pointee(
    architecture: Architecture,
    buffer_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_literal_append_to_runtime_pointee(
            buffer_offset,
            pointer_byte_offset,
            field_byte_offset,
            literal,
        ),
        Architecture::X86_64 => {
            let _ = buffer_offset;
            x86_64::encode_runtime_text_literal_append_to_runtime_pointee(
                pointer_byte_offset,
                field_byte_offset,
                literal,
            )
        }
    }
}

pub fn encode_runtime_text_literal_append_to_runtime_frame_indexed(
    architecture: Architecture,
    buffer_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_literal_append_to_runtime_frame_indexed(
                buffer_offset,
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                literal,
            )
        }
        Architecture::X86_64 => {
            let _ = buffer_offset;
            x86_64::encode_runtime_text_literal_append_to_runtime_frame_indexed(
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                literal,
            )
        }
    }
}

pub fn encode_runtime_text_buffer_materialize(
    architecture: Architecture,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_buffer_materialize(target_offset),
        Architecture::X86_64 => x86_64::encode_runtime_text_buffer_materialize(target_offset),
    }
}

pub fn encode_runtime_text_buffer_materialize_to_runtime_pointee(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_buffer_materialize_to_runtime_pointee(
                pointer_byte_offset,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
    architecture: Architecture,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

/// One stdin byte into a `ByteRead` sum slot (std console `read_byte()`).
/// X86_64 is not encoded yet (TASKS_FS #0a follow-up) -- loud by doctrine.
pub fn encode_runtime_byte_read(
    architecture: Architecture,
    target_offset: usize,
    payload_offset: usize,
    binding: &HostBindingMechanism,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_byte_read_with_plan(architecture, target_offset, payload_offset, binding, None)
}

pub fn encode_runtime_byte_read_with_plan(
    architecture: Architecture,
    target_offset: usize,
    payload_offset: usize,
    binding: &HostBindingMechanism,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => match binding {
            HostBindingMechanism::Import { .. } => {
                validate_aarch64_runtime_import_plan(authoritative_plan)?;
                aarch64::encode_runtime_byte_read_import(target_offset, payload_offset)
            }
            HostBindingMechanism::Syscall { number, .. } => {
                let registers = normalized_syscall_registers_with_plan(
                    architecture,
                    3,
                    true,
                    authoritative_plan,
                )?;
                aarch64::encode_runtime_byte_read_syscall(
                    target_offset,
                    payload_offset,
                    *number,
                    &registers.parameters,
                    registers.required_result()?,
                    registers.number,
                    registers.immediate,
                )
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => {
                Err(Diagnostic::error("read_byte cannot be vtable-bound"))
            }
        },
        Architecture::X86_64 => match binding {
            HostBindingMechanism::Import { .. } => {
                x86_64::encode_runtime_byte_read_import(target_offset, payload_offset)
            }
            HostBindingMechanism::Syscall { number, .. } => {
                let registers = normalized_syscall_registers_with_plan(
                    architecture,
                    3,
                    true,
                    authoritative_plan,
                )?;
                x86_64::encode_runtime_byte_read_syscall(
                    target_offset,
                    payload_offset,
                    *number,
                    &registers.parameters,
                    registers.required_result()?,
                    registers.number,
                    registers.immediate,
                )
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => {
                Err(Diagnostic::error("read_byte cannot be vtable-bound"))
            }
        },
    }
}

/// One byte to stdout (std console `write_byte(b)`); same conventions as
/// the read.
pub fn encode_runtime_byte_write(
    architecture: Architecture,
    source_offset: usize,
    binding: &HostBindingMechanism,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_byte_write_with_plan(architecture, source_offset, binding, None)
}

pub fn encode_runtime_byte_write_with_plan(
    architecture: Architecture,
    source_offset: usize,
    binding: &HostBindingMechanism,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => match binding {
            HostBindingMechanism::Import { .. } => {
                validate_aarch64_runtime_import_plan(authoritative_plan)?;
                aarch64::encode_runtime_byte_write_import(source_offset)
            }
            HostBindingMechanism::Syscall { number, .. } => {
                let registers = normalized_syscall_registers_with_plan(
                    architecture,
                    3,
                    true,
                    authoritative_plan,
                )?;
                aarch64::encode_runtime_byte_write_syscall(
                    source_offset,
                    *number,
                    &registers.parameters,
                    registers.required_result()?,
                    registers.number,
                    registers.immediate,
                )
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => {
                Err(Diagnostic::error("write_byte cannot be vtable-bound"))
            }
        },
        Architecture::X86_64 => match binding {
            HostBindingMechanism::Import { .. } => {
                x86_64::encode_runtime_byte_write_import(source_offset)
            }
            HostBindingMechanism::Syscall { number, .. } => {
                let registers = normalized_syscall_registers_with_plan(
                    architecture,
                    3,
                    true,
                    authoritative_plan,
                )?;
                x86_64::encode_runtime_byte_write_syscall(
                    source_offset,
                    *number,
                    &registers.parameters,
                    registers.required_result()?,
                    registers.number,
                    registers.immediate,
                )
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => {
                Err(Diagnostic::error("write_byte cannot be vtable-bound"))
            }
        },
    }
}

pub fn encode_runtime_text_line_read(
    architecture: Architecture,
    target_offset: usize,
    byte_capacity: usize,
    binding: &HostBindingMechanism,
    target: RuntimeTextReadTarget,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_text_line_read_with_plan(
        architecture,
        target_offset,
        byte_capacity,
        binding,
        target,
        None,
    )
}

pub fn encode_runtime_text_line_read_with_plan(
    architecture: Architecture,
    target_offset: usize,
    byte_capacity: usize,
    binding: &HostBindingMechanism,
    target: RuntimeTextReadTarget,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => match binding {
            HostBindingMechanism::Import { .. } => {
                validate_aarch64_runtime_import_plan(authoritative_plan)?;
                match target {
                    RuntimeTextReadTarget::BoundedByteBuffer => {
                        aarch64::encode_runtime_text_line_read_carrier_import(
                            target_offset,
                            byte_capacity,
                        )
                    }
                    RuntimeTextReadTarget::FixedByteArray => {
                        aarch64::encode_runtime_text_line_read_fixed_array_import(
                            target_offset,
                            byte_capacity,
                        )
                    }
                    RuntimeTextReadTarget::StringDescriptor => {
                        aarch64::encode_runtime_text_line_read_import(target_offset, byte_capacity)
                    }
                }
            }
            HostBindingMechanism::Syscall { number, .. } => {
                let registers = normalized_syscall_registers_with_plan(
                    architecture,
                    3,
                    true,
                    authoritative_plan,
                )?;
                let result_register = registers.required_result()?;
                match target {
                    RuntimeTextReadTarget::BoundedByteBuffer => {
                        aarch64::encode_runtime_text_line_read_carrier_syscall(
                            target_offset,
                            byte_capacity,
                            *number,
                            &registers.parameters,
                            result_register,
                            registers.number,
                            registers.immediate,
                        )
                    }
                    RuntimeTextReadTarget::FixedByteArray => {
                        aarch64::encode_runtime_text_line_read_fixed_array_syscall(
                            target_offset,
                            byte_capacity,
                            *number,
                            &registers.parameters,
                            result_register,
                            registers.number,
                            registers.immediate,
                        )
                    }
                    RuntimeTextReadTarget::StringDescriptor => {
                        aarch64::encode_runtime_text_line_read_syscall(
                            target_offset,
                            byte_capacity,
                            *number,
                            &registers.parameters,
                            result_register,
                            registers.number,
                            registers.immediate,
                        )
                    }
                }
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => {
                Err(Diagnostic::error("read_line cannot be vtable-bound"))
            }
        },
        Architecture::X86_64 => match binding {
            HostBindingMechanism::Import { .. } => match target {
                RuntimeTextReadTarget::BoundedByteBuffer => {
                    x86_64::encode_runtime_text_line_read_carrier(target_offset, byte_capacity)
                }
                RuntimeTextReadTarget::FixedByteArray => {
                    x86_64::encode_runtime_text_line_read_fixed_array(target_offset, byte_capacity)
                }
                RuntimeTextReadTarget::StringDescriptor => {
                    x86_64::encode_runtime_text_line_read(target_offset, byte_capacity)
                }
            },
            HostBindingMechanism::Syscall { number, .. } => {
                let registers = normalized_syscall_registers_with_plan(
                    architecture,
                    3,
                    true,
                    authoritative_plan,
                )?;
                let result_register = registers.required_result()?;
                match target {
                    RuntimeTextReadTarget::BoundedByteBuffer => {
                        x86_64::encode_runtime_text_line_read_syscall_carrier(
                            target_offset,
                            byte_capacity,
                            *number,
                            &registers.parameters,
                            result_register,
                            registers.number,
                            registers.immediate,
                        )
                    }
                    RuntimeTextReadTarget::FixedByteArray => {
                        x86_64::encode_runtime_text_line_read_syscall_fixed_array(
                            target_offset,
                            byte_capacity,
                            *number,
                            &registers.parameters,
                            result_register,
                            registers.number,
                            registers.immediate,
                        )
                    }
                    RuntimeTextReadTarget::StringDescriptor => {
                        x86_64::encode_runtime_text_line_read_syscall(
                            target_offset,
                            byte_capacity,
                            *number,
                            &registers.parameters,
                            result_register,
                            registers.number,
                            registers.immediate,
                        )
                    }
                }
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => {
                Err(Diagnostic::error("read_line cannot be vtable-bound"))
            }
        },
    }
}

fn unsupported_x86_64_encoding() -> Result<Vec<u8>, Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 runtime text encoding is not implemented",
    ))
}

#[cfg(test)]
mod plan_differential_tests {
    use super::*;
    use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    use std::sync::Arc;

    fn syscall_binding() -> HostBindingMechanism {
        HostBindingMechanism::Syscall {
            name: Arc::from("read_or_write"),
            number: 1,
        }
    }

    fn import_binding() -> HostBindingMechanism {
        HostBindingMechanism::Import {
            library: Arc::from("libSystem.B.dylib"),
            symbol: Arc::from("_read"),
        }
    }

    fn plan(architecture: Architecture) -> CallPlan {
        let policy = match architecture {
            Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
            Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
        };
        evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); 3],
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .expect("runtime text syscall plan")
    }

    #[test]
    fn composite_runtime_text_syscalls_equal_the_explicit_retained_plan() {
        let binding = syscall_binding();
        for architecture in [Architecture::X86_64, Architecture::Aarch64] {
            let plan = plan(architecture);

            let compatibility = encode_runtime_byte_read(architecture, 16, 24, &binding)
                .expect("compatibility byte read");
            let planned =
                encode_runtime_byte_read_with_plan(architecture, 16, 24, &binding, Some(&plan))
                    .expect("planned byte read");
            assert_eq!(compatibility, planned, "byte read {architecture:?}");
            assert_eq!(
                crate::runtime_byte_read_width(architecture, &binding),
                crate::runtime_byte_read_width_with_plan(
                    architecture,
                    &binding,
                    16,
                    24,
                    Some(&plan),
                ),
                "byte read width {architecture:?}"
            );

            let compatibility = encode_runtime_byte_write(architecture, 32, &binding)
                .expect("compatibility byte write");
            let planned =
                encode_runtime_byte_write_with_plan(architecture, 32, &binding, Some(&plan))
                    .expect("planned byte write");
            assert_eq!(compatibility, planned, "byte write {architecture:?}");
            assert_eq!(
                crate::runtime_byte_write_width(architecture, &binding, 32),
                crate::runtime_byte_write_width_with_plan(architecture, &binding, 32, Some(&plan),),
                "byte write width {architecture:?}"
            );

            for target in [
                RuntimeTextReadTarget::BoundedByteBuffer,
                RuntimeTextReadTarget::FixedByteArray,
                RuntimeTextReadTarget::StringDescriptor,
            ] {
                let compatibility =
                    encode_runtime_text_line_read(architecture, 40, 64, &binding, target)
                        .expect("compatibility line read");
                let planned = encode_runtime_text_line_read_with_plan(
                    architecture,
                    40,
                    64,
                    &binding,
                    target,
                    Some(&plan),
                )
                .expect("planned line read");
                assert_eq!(
                    compatibility, planned,
                    "line read {architecture:?} {target:?}"
                );
                assert_eq!(
                    crate::runtime_text_line_read_width(architecture, 64, &binding, target, 40,),
                    crate::runtime_text_line_read_width_with_plan(
                        architecture,
                        64,
                        &binding,
                        target,
                        40,
                        Some(&plan),
                    ),
                    "line read width {architecture:?} {target:?}"
                );
            }
        }
    }

    #[test]
    fn aarch64_runtime_text_imports_validate_the_retained_native_plan() {
        let binding = import_binding();
        let plan = evaluate_call_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); 3],
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .expect("AAPCS64 read/write plan");

        assert_eq!(
            encode_runtime_byte_read(Architecture::Aarch64, 16, 24, &binding)
                .expect("compatibility import byte read"),
            encode_runtime_byte_read_with_plan(
                Architecture::Aarch64,
                16,
                24,
                &binding,
                Some(&plan),
            )
            .expect("planned import byte read")
        );
        assert_eq!(
            encode_runtime_byte_write(Architecture::Aarch64, 32, &binding)
                .expect("compatibility import byte write"),
            encode_runtime_byte_write_with_plan(Architecture::Aarch64, 32, &binding, Some(&plan),)
                .expect("planned import byte write")
        );
        for target in [
            RuntimeTextReadTarget::BoundedByteBuffer,
            RuntimeTextReadTarget::FixedByteArray,
            RuntimeTextReadTarget::StringDescriptor,
        ] {
            assert_eq!(
                encode_runtime_text_line_read(Architecture::Aarch64, 40, 64, &binding, target)
                    .expect("compatibility import line read"),
                encode_runtime_text_line_read_with_plan(
                    Architecture::Aarch64,
                    40,
                    64,
                    &binding,
                    target,
                    Some(&plan),
                )
                .expect("planned import line read")
            );
        }

        let mut incompatible = plan;
        incompatible.parameters[1].locations = vec![ValueLocation::Register {
            register: MachineRegister::Aarch64X(3),
            value_byte_offset: 0,
            byte_size: 8,
        }];
        let error = encode_runtime_byte_read_with_plan(
            Architecture::Aarch64,
            16,
            24,
            &binding,
            Some(&incompatible),
        )
        .expect_err("hardcoded import placement must reject a changed retained plan");
        assert!(error.message.contains("requires Aarch64X(1)"));
        assert_eq!(
            crate::runtime_byte_read_width_with_plan(
                Architecture::Aarch64,
                &binding,
                16,
                24,
                Some(&incompatible),
            ),
            0,
            "layout must fail closed with emission"
        );
    }
}

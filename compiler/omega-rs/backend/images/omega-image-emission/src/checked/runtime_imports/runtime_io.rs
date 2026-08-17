//! Replays runtime byte, line, and text-boundary calls.

use super::syscalls::outbound_syscall_replay_registers;
use super::*;

fn validate_aarch64_runtime_import_replay_plan(
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(), Diagnostic> {
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, MachineRegister, ValueLocation, ValueShape,
    };

    let word = ValueShape::integer(8, 8);
    omega_calling_conventions::validate_call_plan(
        plan,
        &CallSignature {
            parameters: vec![word; 3],
            result: Some(word),
        },
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "final runtime-byte import replay retained an incompatible native read/write plan: {error}"
        ))
    })?;
    if plan.policy != CallingPolicy::Aapcs64 {
        return Err(Diagnostic::error(
            "final AArch64 runtime-byte import replay requires AAPCS64",
        ));
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
                "final AArch64 runtime-byte import parameter {index} lost its canonical x{index} placement"
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
            "final AArch64 runtime-byte import result lost its canonical x0 placement",
        ));
    }
    Ok(())
}

pub(in crate::checked) struct RuntimeTextReplay {
    pub(in crate::checked) bytes: Vec<u8>,
    pub(in crate::checked) call_sites: Vec<(usize, std::sync::Arc<str>, std::sync::Arc<str>)>,
    pub(in crate::checked) address_sites: Vec<(usize, OutboundCallRelocationTarget)>,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::checked) fn encode_runtime_byte_replay(
    architecture: Architecture,
    read: bool,
    target_or_source_offset: usize,
    payload_offset: usize,
    address_target: OutboundCallRelocationTarget,
    mechanism: &omega_calling_conventions::HostBindingMechanism,
    plan: &omega_calling_conventions::CallPlan,
    get_std_handle: Option<&omega_machine_bytes::CompilerRuntimeImportSubcall>,
) -> Result<RuntimeTextReplay, Diagnostic> {
    use omega_calling_conventions::HostBindingMechanism;

    let (mut bytes, mut call_sites) = match (architecture, mechanism) {
        (Architecture::Aarch64, HostBindingMechanism::Import { library, symbol }) => {
            if get_std_handle.is_some() {
                return Err(Diagnostic::error(
                    "final AArch64 runtime-byte replay unexpectedly retained GetStdHandle",
                ));
            }
            validate_aarch64_runtime_import_replay_plan(plan)?;
            let (bytes, call_site) = if read {
                (
                    omega_isa_aarch64::aarch64::encode_runtime_byte_read_import(
                        target_or_source_offset,
                        payload_offset,
                    )?,
                    omega_isa_aarch64::aarch64::runtime_byte_read_import_call_offset(),
                )
            } else {
                (
                    omega_isa_aarch64::aarch64::encode_runtime_byte_write_import(
                        target_or_source_offset,
                    )?,
                    omega_isa_aarch64::aarch64::runtime_byte_write_import_call_offset(
                        target_or_source_offset,
                    ),
                )
            };
            (
                bytes,
                vec![(
                    call_site,
                    std::sync::Arc::clone(library),
                    std::sync::Arc::clone(symbol),
                )],
            )
        }
        (Architecture::X86_64, HostBindingMechanism::Import { library, symbol }) => {
            let handle = get_std_handle.ok_or_else(|| {
                Diagnostic::error("final Win64 runtime-byte replay lost its GetStdHandle call plan")
            })?;
            omega_isa_x86_64::validate_win64_runtime_file_adapter_plans(&handle.plan, plan)?;
            let (bytes, handle_site, file_site) = if read {
                (
                    omega_isa_x86_64::encode_runtime_byte_read_import(
                        target_or_source_offset,
                        payload_offset,
                    )?,
                    omega_isa_x86_64::runtime_byte_read_get_std_handle_offset(),
                    omega_isa_x86_64::runtime_byte_read_read_file_offset(),
                )
            } else {
                (
                    omega_isa_x86_64::encode_runtime_byte_write_import(target_or_source_offset)?,
                    omega_isa_x86_64::runtime_byte_write_get_std_handle_offset(),
                    omega_isa_x86_64::runtime_byte_write_write_file_offset(),
                )
            };
            (
                bytes,
                vec![
                    (
                        handle_site,
                        std::sync::Arc::clone(&handle.library),
                        std::sync::Arc::clone(&handle.symbol),
                    ),
                    (
                        file_site,
                        std::sync::Arc::clone(library),
                        std::sync::Arc::clone(symbol),
                    ),
                ],
            )
        }
        (architecture, HostBindingMechanism::Syscall { number, .. }) => {
            if get_std_handle.is_some() {
                return Err(Diagnostic::error(
                    "final runtime-byte syscall replay unexpectedly retained GetStdHandle",
                ));
            }
            let registers = outbound_syscall_replay_registers(architecture, plan, 3)?;
            let bytes = match (architecture, read) {
                (Architecture::Aarch64, true) => {
                    omega_isa_aarch64::aarch64::encode_runtime_byte_read_syscall(
                        target_or_source_offset,
                        payload_offset,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::Aarch64, false) => {
                    omega_isa_aarch64::aarch64::encode_runtime_byte_write_syscall(
                        target_or_source_offset,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::X86_64, true) => omega_isa_x86_64::encode_runtime_byte_read_syscall(
                    target_or_source_offset,
                    payload_offset,
                    *number,
                    &registers.parameters,
                    registers.result,
                    registers.number,
                    registers.immediate,
                )?,
                (Architecture::X86_64, false) => {
                    omega_isa_x86_64::encode_runtime_byte_write_syscall(
                        target_or_source_offset,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
            };
            (bytes, Vec::new())
        }
        (
            _,
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. },
        ) => {
            return Err(Diagnostic::error(
                "final runtime-byte replay retained a non-import/non-syscall mechanism",
            ));
        }
    };

    let mut address_sites = vec![(0, address_target)];
    if mechanism.requires_float_control_restore() {
        let (prefix, suffix) = match architecture {
            Architecture::X86_64 => (
                omega_isa_x86_64::encode_foreign_float_control_prefix_bytes().to_vec(),
                omega_isa_x86_64::encode_foreign_float_control_suffix_bytes().to_vec(),
            ),
            Architecture::Aarch64 => (
                omega_isa_aarch64::aarch64::encode_foreign_float_control_prefix_bytes().to_vec(),
                omega_isa_aarch64::aarch64::encode_foreign_float_control_suffix_bytes().to_vec(),
            ),
        };
        let prefix_width = prefix.len();
        let mut wrapped = Vec::with_capacity(prefix.len() + bytes.len() + suffix.len());
        wrapped.extend(prefix);
        wrapped.extend(bytes);
        wrapped.extend(suffix);
        bytes = wrapped;
        for (site, _, _) in &mut call_sites {
            *site += prefix_width;
        }
        for (site, _) in &mut address_sites {
            *site += prefix_width;
        }
    }
    Ok(RuntimeTextReplay {
        bytes,
        call_sites,
        address_sites,
    })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::checked) fn encode_runtime_line_read_replay(
    architecture: Architecture,
    buffer_symbol: std::sync::Arc<str>,
    target_region: omega_target_operations::RuntimeStorageRegion,
    target_offset: usize,
    byte_capacity: usize,
    target: omega_target_operations::RuntimeTextReadTarget,
    mechanism: &omega_calling_conventions::HostBindingMechanism,
    plan: &omega_calling_conventions::CallPlan,
    get_std_handle: Option<&omega_machine_bytes::CompilerRuntimeImportSubcall>,
) -> Result<RuntimeTextReplay, Diagnostic> {
    use omega_calling_conventions::HostBindingMechanism;
    use omega_target_operations::RuntimeTextReadTarget;

    let (mut bytes, mut call_sites) = match (architecture, mechanism) {
        (Architecture::Aarch64, HostBindingMechanism::Import { library, symbol }) => {
            if get_std_handle.is_some() {
                return Err(Diagnostic::error(
                    "final AArch64 runtime line-read replay unexpectedly retained GetStdHandle",
                ));
            }
            validate_aarch64_runtime_import_replay_plan(plan)?;
            let (bytes, call_site) = match target {
                RuntimeTextReadTarget::BoundedByteBuffer => (
                    omega_isa_aarch64::aarch64::encode_runtime_text_line_read_carrier_import(
                        target_offset,
                        byte_capacity,
                    )?,
                    omega_isa_aarch64::aarch64::runtime_text_line_read_carrier_import_call_offset(
                        target_offset,
                    ),
                ),
                RuntimeTextReadTarget::FixedByteArray => (
                    omega_isa_aarch64::aarch64::encode_runtime_text_line_read_fixed_array_import(
                        target_offset,
                        byte_capacity,
                    )?,
                    omega_isa_aarch64::aarch64::runtime_text_line_read_fixed_array_import_call_offset(
                        target_offset,
                    ),
                ),
                RuntimeTextReadTarget::StringDescriptor => (
                    omega_isa_aarch64::aarch64::encode_runtime_text_line_read_import(
                        target_offset,
                        byte_capacity,
                    )?,
                    omega_isa_aarch64::aarch64::runtime_text_line_read_import_call_offset(),
                ),
            };
            (
                bytes,
                vec![(
                    call_site,
                    std::sync::Arc::clone(library),
                    std::sync::Arc::clone(symbol),
                )],
            )
        }
        (Architecture::X86_64, HostBindingMechanism::Import { library, symbol }) => {
            let handle = get_std_handle.ok_or_else(|| {
                Diagnostic::error(
                    "final Win64 runtime line-read replay lost its GetStdHandle call plan",
                )
            })?;
            omega_isa_x86_64::validate_win64_runtime_file_adapter_plans(&handle.plan, plan)?;
            let (bytes, handle_site, file_site) = match target {
                RuntimeTextReadTarget::BoundedByteBuffer => (
                    omega_isa_x86_64::encode_runtime_text_line_read_carrier(
                        target_offset,
                        byte_capacity,
                    )?,
                    omega_isa_x86_64::runtime_text_line_read_carrier_get_std_handle_call_offset(),
                    omega_isa_x86_64::runtime_text_line_read_carrier_read_file_call_offset(),
                ),
                RuntimeTextReadTarget::FixedByteArray => (
                    omega_isa_x86_64::encode_runtime_text_line_read_fixed_array(
                        target_offset,
                        byte_capacity,
                    )?,
                    omega_isa_x86_64::runtime_text_line_read_fixed_array_get_std_handle_call_offset(
                    ),
                    omega_isa_x86_64::runtime_text_line_read_fixed_array_read_file_call_offset(),
                ),
                RuntimeTextReadTarget::StringDescriptor => (
                    omega_isa_x86_64::encode_runtime_text_line_read(target_offset, byte_capacity)?,
                    omega_isa_x86_64::runtime_text_line_read_get_std_handle_call_offset(),
                    omega_isa_x86_64::runtime_text_line_read_read_file_call_offset(),
                ),
            };
            (
                bytes,
                vec![
                    (
                        handle_site,
                        std::sync::Arc::clone(&handle.library),
                        std::sync::Arc::clone(&handle.symbol),
                    ),
                    (
                        file_site,
                        std::sync::Arc::clone(library),
                        std::sync::Arc::clone(symbol),
                    ),
                ],
            )
        }
        (architecture, HostBindingMechanism::Syscall { number, .. }) => {
            if get_std_handle.is_some() {
                return Err(Diagnostic::error(
                    "final runtime line-read syscall replay unexpectedly retained GetStdHandle",
                ));
            }
            let registers = outbound_syscall_replay_registers(architecture, plan, 3)?;
            let bytes = match (architecture, target) {
                (Architecture::Aarch64, RuntimeTextReadTarget::BoundedByteBuffer) => {
                    omega_isa_aarch64::aarch64::encode_runtime_text_line_read_carrier_syscall(
                        target_offset,
                        byte_capacity,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::Aarch64, RuntimeTextReadTarget::FixedByteArray) => {
                    omega_isa_aarch64::aarch64::encode_runtime_text_line_read_fixed_array_syscall(
                        target_offset,
                        byte_capacity,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::Aarch64, RuntimeTextReadTarget::StringDescriptor) => {
                    omega_isa_aarch64::aarch64::encode_runtime_text_line_read_syscall(
                        target_offset,
                        byte_capacity,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::X86_64, RuntimeTextReadTarget::BoundedByteBuffer) => {
                    omega_isa_x86_64::encode_runtime_text_line_read_syscall_carrier(
                        target_offset,
                        byte_capacity,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::X86_64, RuntimeTextReadTarget::FixedByteArray) => {
                    omega_isa_x86_64::encode_runtime_text_line_read_syscall_fixed_array(
                        target_offset,
                        byte_capacity,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
                (Architecture::X86_64, RuntimeTextReadTarget::StringDescriptor) => {
                    omega_isa_x86_64::encode_runtime_text_line_read_syscall(
                        target_offset,
                        byte_capacity,
                        *number,
                        &registers.parameters,
                        registers.result,
                        registers.number,
                        registers.immediate,
                    )?
                }
            };
            (bytes, Vec::new())
        }
        (
            _,
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. },
        ) => {
            return Err(Diagnostic::error(
                "final runtime line-read replay retained a non-import/non-syscall mechanism",
            ));
        }
    };

    let mut address_sites = match target {
        RuntimeTextReadTarget::BoundedByteBuffer | RuntimeTextReadTarget::FixedByteArray => {
            vec![(0, OutboundCallRelocationTarget::Storage(target_region))]
        }
        RuntimeTextReadTarget::StringDescriptor => {
            let target_site = match (architecture, mechanism) {
                (Architecture::Aarch64, HostBindingMechanism::Import { .. }) => {
                    omega_isa_aarch64::aarch64::runtime_text_line_read_import_target_address_offset(
                    )
                }
                (Architecture::Aarch64, HostBindingMechanism::Syscall { number, .. }) => {
                    omega_isa_aarch64::aarch64::runtime_text_line_read_syscall_target_address_offset(
                        *number,
                    )
                }
                (Architecture::X86_64, HostBindingMechanism::Import { .. }) => {
                    omega_isa_x86_64::runtime_text_line_read_target_imm_offset()
                }
                (Architecture::X86_64, HostBindingMechanism::Syscall { .. }) => {
                    omega_isa_x86_64::runtime_text_line_read_syscall_target_imm_offset()
                }
                _ => unreachable!("runtime line read mechanism validated above"),
            };
            vec![
                (0, OutboundCallRelocationTarget::Data(buffer_symbol)),
                (
                    target_site,
                    OutboundCallRelocationTarget::Storage(target_region),
                ),
            ]
        }
    };
    if mechanism.requires_float_control_restore() {
        let (prefix, suffix) = match architecture {
            Architecture::X86_64 => (
                omega_isa_x86_64::encode_foreign_float_control_prefix_bytes().to_vec(),
                omega_isa_x86_64::encode_foreign_float_control_suffix_bytes().to_vec(),
            ),
            Architecture::Aarch64 => (
                omega_isa_aarch64::aarch64::encode_foreign_float_control_prefix_bytes().to_vec(),
                omega_isa_aarch64::aarch64::encode_foreign_float_control_suffix_bytes().to_vec(),
            ),
        };
        let prefix_width = prefix.len();
        let mut wrapped = Vec::with_capacity(prefix.len() + bytes.len() + suffix.len());
        wrapped.extend(prefix);
        wrapped.extend(bytes);
        wrapped.extend(suffix);
        bytes = wrapped;
        for (site, _, _) in &mut call_sites {
            *site += prefix_width;
        }
        for (site, _) in &mut address_sites {
            *site += prefix_width;
        }
    }
    Ok(RuntimeTextReplay {
        bytes,
        call_sites,
        address_sites,
    })
}

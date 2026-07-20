use crate::aarch64_call_operand;
use omega_calling_conventions::{
    CallSignature, CallingPolicy, EntryControl, HostOperationKey, MachineRegister, ValueLocation,
    ValueShape, evaluate_call_plan,
};
use omega_core::diagnostics::Diagnostic;
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::InstructionOperandLike;

pub(super) struct NormalizedSyscallRegisters {
    pub parameters: Vec<omega_calling_conventions::MachineRegister>,
    pub result: Option<omega_calling_conventions::MachineRegister>,
    pub number: omega_calling_conventions::MachineRegister,
    pub immediate: u16,
}

impl NormalizedSyscallRegisters {
    pub(super) fn required_result(
        &self,
    ) -> Result<omega_calling_conventions::MachineRegister, Diagnostic> {
        self.result.ok_or_else(|| {
            Diagnostic::error("normalized syscall plan did not place its required result")
        })
    }
}

pub(super) fn normalized_syscall_registers(
    architecture: Architecture,
    parameter_count: usize,
    has_result: bool,
) -> Result<NormalizedSyscallRegisters, Diagnostic> {
    let policy = match architecture {
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
    };
    let word = ValueShape::integer(8, 8);
    let signature = CallSignature {
        parameters: vec![word; parameter_count],
        result: has_result.then_some(word),
    };
    let plan = evaluate_call_plan(policy, &signature).map_err(|error| {
        Diagnostic::error(format!("cannot evaluate syscall call plan: {error}"))
    })?;
    let parameters = plan
        .parameters
        .iter()
        .enumerate()
        .map(|(index, placement)| full_width_register(&placement.locations, "parameter", index))
        .collect::<Result<Vec<_>, _>>()?;
    let result = plan
        .result
        .as_ref()
        .map(|placement| full_width_register(&placement.locations, "result", 0))
        .transpose()?;
    let EntryControl::SupervisorCall {
        number_register,
        immediate,
    } = plan.entry_control
    else {
        return Err(Diagnostic::error(
            "normalized syscall plan did not select supervisor-call entry control",
        ));
    };
    Ok(NormalizedSyscallRegisters {
        parameters,
        result,
        number: number_register,
        immediate,
    })
}

fn full_width_register(
    locations: &[ValueLocation],
    role: &str,
    index: usize,
) -> Result<omega_calling_conventions::MachineRegister, Diagnostic> {
    match locations {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 8,
            },
        ] => Ok(*register),
        locations => Err(Diagnostic::error(format!(
            "normalized syscall {role} {index} did not resolve to one full-width register: {locations:?}"
        ))),
    }
}

/// A VtableSlot call (provides-sourced, per-object dispatch). x86_64 only; an
/// aarch64 vtable call awaits its stub.
pub fn encode_vtable_call_sequence<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    index: i64,
) -> Result<Vec<u8>, Diagnostic> {
    match target.architecture {
        Architecture::Aarch64 => Err(Diagnostic::error(
            "AArch64 vtable-slot dispatch is not implemented (x86_64 only)",
        )),
        Architecture::X86_64
            if CallingPolicy::native_for_target(target) == CallingPolicy::MicrosoftX64 =>
        {
            x86_64::encode_win64_vtable_call(operands, index)
        }
        Architecture::X86_64 => Err(Diagnostic::error(
            "x86-64 vtable compatibility encoder requires the Microsoft x64 policy",
        )),
    }
}

/// The FIELD-MODEL flavor (extern brief SS12.1): the byte offset came from
/// the vtable struct's layout via the backend's vtable-field pass. When
/// `result_present`, operand 0 is the RESULT place and the store tail runs.
pub fn encode_vtable_call_sequence_at_offset<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    byte_offset: usize,
    result_present: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match target.architecture {
        Architecture::Aarch64 => Err(Diagnostic::error(
            "AArch64 vtable-field dispatch is not implemented (x86_64 only)",
        )),
        Architecture::X86_64
            if CallingPolicy::native_for_target(target) == CallingPolicy::MicrosoftX64 =>
        {
            x86_64::encode_win64_vtable_call_at_offset(
                operands,
                i64::try_from(byte_offset)
                    .map_err(|_| Diagnostic::error("vtable field offset overflows i64"))?,
                result_present,
            )
        }
        Architecture::X86_64 => Err(Diagnostic::error(
            "x86-64 vtable-field compatibility encoder requires the Microsoft x64 policy",
        )),
    }
}

/// A SERVICE-TABLE function call: field-model dispatch where the table
/// pointer is dispatch-only, never a wire argument (EFI table services take
/// no This; protocol/COM methods do).
pub fn encode_table_function_call_sequence<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    byte_offset: usize,
    result_present: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match target.architecture {
        Architecture::Aarch64 => Err(Diagnostic::error(
            "AArch64 table-function dispatch is not implemented (x86_64 only)",
        )),
        Architecture::X86_64
            if CallingPolicy::native_for_target(target) == CallingPolicy::MicrosoftX64 =>
        {
            x86_64::encode_win64_table_function_call(
                operands,
                i64::try_from(byte_offset)
                    .map_err(|_| Diagnostic::error("service table field offset overflows i64"))?,
                result_present,
            )
        }
        Architecture::X86_64 => Err(Diagnostic::error(
            "x86-64 table-function compatibility encoder requires the Microsoft x64 policy",
        )),
    }
}

pub fn encode_host_call_sequence<T: InstructionOperandLike>(
    target: NativeTarget,
    operation_key: HostOperationKey,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    match target.architecture {
        // Deref-result ops (errno) must be checked before the plain
        // value-returning arm: they share `returns_value()` but insert an extra
        // `ldr` to deref the returned pointer.
        Architecture::Aarch64 if operation_key.dereferences_result() => {
            let (arguments, result) =
                normalized_aarch64_import_registers(operands, Aarch64ImportResult::Integer, false)?;
            aarch64::encode_host_call_sequence_value_returning_deref_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
                result.expect("integer result requested"),
            )
        }
        // Stack-mode ops (`open_create`) also share `returns_value()` but bracket
        // the call with `sub sp`/`str [sp]`/`add sp` to pass the variadic `mode`
        // on the stack; checked before the plain value-returning arm.
        Architecture::Aarch64 if operation_key.passes_trailing_mode_on_stack() => {
            let (arguments, result) =
                normalized_aarch64_import_registers(operands, Aarch64ImportResult::Integer, true)?;
            aarch64::encode_host_call_sequence_value_returning_open_create_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
                result.expect("integer result requested"),
            )
        }
        // Float-returning ops (sqrt/hypot) also share `returns_value()` but the
        // result comes back in `d0`; the encoder inserts `fmov x0, d0` before the
        // result store. Checked before the plain value-returning arm.
        Architecture::Aarch64 if operation_key.returns_float() => {
            let (arguments, result) =
                normalized_aarch64_import_registers(operands, Aarch64ImportResult::Float, false)?;
            aarch64::encode_host_call_sequence_value_returning_float_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
                result.expect("float result requested"),
            )
        }
        // Constant-result ops have NO call (and no import relocation): the
        // constant materializes into x0 and stores through the normal result
        // tail. Checked before the plain value-returning arm (they share
        // `returns_value()`).
        Architecture::Aarch64 if operation_key.lowers_to_constant_result() => {
            aarch64::encode_host_call_sequence_constant_result_from_operands(
                operands.iter().map(aarch64_call_operand),
            )
        }
        Architecture::Aarch64 if operation_key.returns_value() => {
            let (arguments, result) =
                normalized_aarch64_import_registers(operands, Aarch64ImportResult::Integer, false)?;
            aarch64::encode_host_call_sequence_value_returning_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
                result.expect("integer result requested"),
            )
        }
        Architecture::Aarch64 => {
            let (arguments, result) =
                normalized_aarch64_import_registers(operands, Aarch64ImportResult::None, false)?;
            debug_assert!(result.is_none());
            aarch64::encode_host_call_sequence_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
            )
        }
        Architecture::X86_64 => x86_64::encode_host_call_sequence(
            CallingPolicy::native_for_target(target),
            operation_key,
            operands,
        ),
    }
}

/// A provides-authored / via-leaf IMPORT call (custom capability): the
/// emission-planning blocker enforces the result-binding shape, so the call
/// ALWAYS carries a leading result operand -- on aarch64 it routes to the
/// value-returning sequence directly (the capability-keyed returns_value()
/// catalog cannot know authored operations; routing by catalog sent the
/// result place into x0 and shifted every real argument -- the
/// import_call_argument_lost class). x86_64's encoder handles the key
/// itself (windows-session verified).
pub fn encode_authored_import_call_sequence<T: InstructionOperandLike>(
    target: NativeTarget,
    operation_key: HostOperationKey,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    match target.architecture {
        Architecture::Aarch64 => {
            let (arguments, result) =
                normalized_aarch64_import_registers(operands, Aarch64ImportResult::Integer, false)?;
            aarch64::encode_host_call_sequence_value_returning_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
                result.expect("integer result requested"),
            )
        }
        Architecture::X86_64 => x86_64::encode_host_call_sequence(
            CallingPolicy::native_for_target(target),
            operation_key,
            operands,
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Aarch64ImportResult {
    None,
    Integer,
    Float,
}

pub fn normalized_aarch64_host_argument_locations<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
    authored_import: bool,
) -> Result<Vec<ValueLocation>, Diagnostic> {
    let result_kind = if authored_import || operation_key.dereferences_result() {
        Aarch64ImportResult::Integer
    } else if operation_key.returns_float() {
        Aarch64ImportResult::Float
    } else if operation_key.returns_value() {
        Aarch64ImportResult::Integer
    } else {
        Aarch64ImportResult::None
    };
    normalized_aarch64_import_registers(
        operands,
        result_kind,
        operation_key.passes_trailing_mode_on_stack(),
    )
    .map(|(locations, _)| locations)
}

pub fn aarch64_host_call_stack_prefix_width(
    locations: &[ValueLocation],
    argument_count: usize,
) -> usize {
    omega_isa_aarch64::aarch64::host_call_stack_prefix_width(locations, argument_count)
}

pub fn aarch64_host_call_stack_total_width(locations: &[ValueLocation]) -> usize {
    omega_isa_aarch64::aarch64::host_call_stack_total_width(locations)
}

/// ENT2c: evaluate the AAPCS64 call surface from the actual selected operands.
/// The encoder receives exact register/stack locations and may no longer
/// reconstruct x0../v0.. or outgoing offsets independently. Scalar stack
/// placements are supported; float-stack and fragmented placements fail closed.
///
/// `trailing_variadic_stack` is the compatibility seam for Darwin `open`:
/// its anonymous `mode` argument is intentionally stack-passed by Apple's
/// variadic ABI and is not yet representable in `CallSignature`. The named
/// arguments and result still consume the normalized plan here; the final
/// stack operand remains with the existing checked special-case encoder.
fn normalized_aarch64_import_registers<T: InstructionOperandLike>(
    operands: &[T],
    result_kind: Aarch64ImportResult,
    trailing_variadic_stack: bool,
) -> Result<(Vec<ValueLocation>, Option<MachineRegister>), Diagnostic> {
    let aarch64_operands = operands
        .iter()
        .map(aarch64_call_operand)
        .collect::<Vec<_>>();
    let (result_operand, mut arguments) = match result_kind {
        Aarch64ImportResult::None => (None, aarch64_operands.as_slice()),
        Aarch64ImportResult::Integer | Aarch64ImportResult::Float => {
            let Some((result, arguments)) = aarch64_operands.split_first() else {
                return Err(Diagnostic::error(
                    "AArch64 value-returning import has no result storage operand",
                ));
            };
            (Some(*result), arguments)
        }
    };
    if trailing_variadic_stack {
        let Some((_, named_arguments)) = arguments.split_last() else {
            return Err(Diagnostic::error(
                "AArch64 variadic import is missing its stack argument",
            ));
        };
        arguments = named_arguments;
    }

    let signature = CallSignature {
        parameters: arguments
            .iter()
            .copied()
            .map(aarch64_operand_shape)
            .collect::<Result<Vec<_>, _>>()?,
        result: match (result_kind, result_operand) {
            (Aarch64ImportResult::None, None) => None,
            (Aarch64ImportResult::Integer, Some(operand)) => {
                Some(aarch64_result_shape(operand, false)?)
            }
            (Aarch64ImportResult::Float, Some(operand)) => {
                Some(aarch64_result_shape(operand, true)?)
            }
            _ => {
                return Err(Diagnostic::error(
                    "AArch64 import result classification is internally inconsistent",
                ));
            }
        },
    };
    let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature).map_err(|error| {
        Diagnostic::error(format!("cannot evaluate AAPCS64 import plan: {error}"))
    })?;
    let parameter_locations = plan
        .parameters
        .iter()
        .enumerate()
        .map(|(index, placement)| one_location(&placement.locations, "parameter", index))
        .collect::<Result<Vec<_>, _>>()?;
    let result_register = plan
        .result
        .as_ref()
        .map(|placement| one_register(&placement.locations, "result", 0))
        .transpose()?;
    Ok((parameter_locations, result_register))
}

fn aarch64_operand_shape(
    operand: omega_isa_aarch64::Aarch64CallOperand,
) -> Result<ValueShape, Diagnostic> {
    use omega_isa_aarch64::Aarch64CallOperand;
    match operand {
        Aarch64CallOperand::RuntimeScalarFloat { byte_count, .. } => {
            let byte_count = u16::try_from(byte_count)
                .map_err(|_| Diagnostic::error("AArch64 float import operand width exceeds u16"))?;
            Ok(ValueShape::float(byte_count))
        }
        Aarch64CallOperand::RuntimeScalarInteger { byte_count, .. } => {
            let byte_count = u16::try_from(byte_count).map_err(|_| {
                Diagnostic::error("AArch64 integer import operand width exceeds u16")
            })?;
            Ok(ValueShape::integer(byte_count, byte_count.max(1)))
        }
        Aarch64CallOperand::DataAddress
        | Aarch64CallOperand::RuntimeStringPointer { .. }
        | Aarch64CallOperand::RuntimeStringLength { .. }
        | Aarch64CallOperand::RuntimePointeeStringPointer { .. }
        | Aarch64CallOperand::RuntimePointeeStringLength { .. }
        | Aarch64CallOperand::RuntimeStorageAddress { .. }
        | Aarch64CallOperand::ImmediateInteger(_)
        | Aarch64CallOperand::ByteLength(_) => Ok(ValueShape::integer(8, 8)),
    }
}

fn aarch64_result_shape(
    operand: omega_isa_aarch64::Aarch64CallOperand,
    float: bool,
) -> Result<ValueShape, Diagnostic> {
    let omega_isa_aarch64::Aarch64CallOperand::RuntimeScalarInteger { byte_count, .. } = operand
    else {
        return Err(Diagnostic::error(
            "AArch64 import result place did not lower to scalar storage",
        ));
    };
    let byte_count = u16::try_from(byte_count)
        .map_err(|_| Diagnostic::error("AArch64 import result width exceeds u16"))?;
    Ok(if float {
        ValueShape::float(byte_count)
    } else {
        ValueShape::integer(byte_count, byte_count.max(1))
    })
}

fn one_register(
    locations: &[ValueLocation],
    role: &str,
    index: usize,
) -> Result<MachineRegister, Diagnostic> {
    match locations {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                ..
            },
        ] => Ok(*register),
        _ => Err(Diagnostic::error(format!(
            "AAPCS64 import {role} {index} is not a scalar register result; fragmented outbound results are not implemented yet"
        ))),
    }
}

fn one_location(
    locations: &[ValueLocation],
    role: &str,
    index: usize,
) -> Result<ValueLocation, Diagnostic> {
    match locations {
        [
            location @ ValueLocation::Register {
                value_byte_offset: 0,
                ..
            },
        ]
        | [
            location @ ValueLocation::Stack {
                value_byte_offset: 0,
                ..
            },
        ] => Ok(*location),
        _ => Err(Diagnostic::error(format!(
            "AAPCS64 import {role} {index} has a fragmented placement that the outbound encoder cannot realize: {locations:?}"
        ))),
    }
}

pub fn encode_syscall_sequence<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    let registers = normalized_syscall_registers(architecture, operands.len(), false)?;

    match architecture {
        Architecture::Aarch64 => aarch64::encode_syscall_sequence_from_operands(
            operands.iter().map(aarch64_call_operand),
            syscall_number,
            &registers.parameters,
            registers.number,
            registers.immediate,
        ),
        Architecture::X86_64 => x86_64::encode_syscall_sequence(
            operands,
            syscall_number,
            &registers.parameters,
            registers.number,
            registers.immediate,
        ),
    }
}

pub fn encode_function_enter_bytes(
    architecture: Architecture,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            let bytes = aarch64::encode_function_enter_bytes().to_vec();
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
        Architecture::X86_64 => Ok((Vec::new(), 0)),
    }
}

pub fn encode_return_bytes(architecture: Architecture) -> Result<(Vec<u8>, usize), Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            let bytes = aarch64::encode_return_bytes().to_vec();
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
        Architecture::X86_64 => {
            let bytes = x86_64::encode_return_bytes().to_vec();
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
    }
}

/// The privileged `hlt` (`asm { hlt }`) as target bytes: `hlt` (0xF4) on
/// x86_64, its idle analog `wfi` on AArch64. Position-independent, no
/// relocation site.
pub fn encode_machine_halt_bytes(architecture: Architecture) -> Vec<u8> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_machine_halt_bytes().to_vec(),
        Architecture::X86_64 => x86_64::encode_machine_halt_bytes().to_vec(),
    }
}

pub fn encode_memory_fence_bytes(
    architecture: Architecture,
    kind: omega_core::inline_assembly::AsmFenceKind,
) -> Option<Vec<u8>> {
    match architecture {
        Architecture::Aarch64 => None,
        Architecture::X86_64 => Some(x86_64::encode_memory_fence_bytes(kind).to_vec()),
    }
}

pub fn encode_interrupt_control_bytes(
    architecture: Architecture,
    kind: omega_core::inline_assembly::AsmInterruptControlKind,
) -> Option<Vec<u8>> {
    match architecture {
        Architecture::Aarch64 => None,
        Architecture::X86_64 => Some(x86_64::encode_interrupt_control_bytes(kind).to_vec()),
    }
}

pub fn encode_runtime_storage_copy_to_return_register_bytes(
    architecture: Architecture,
    register: omega_calling_conventions::MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if register.architecture() != architecture {
        return Err(Diagnostic::error(format!(
            "result register {register:?} does not belong to target architecture {architecture:?}"
        )));
    }
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_copy_to_return_register_bytes(
            register,
            byte_offset,
            byte_size,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_storage_copy_to_return_register_bytes(
            register,
            byte_offset,
            byte_size,
        ),
    }
}

/// The entry prologue's inbound argument unmarshal. The normalized call plan
/// names the exact register; target encoders only realize that selection.
pub fn encode_entry_argument_register_write_bytes(
    register: omega_calling_conventions::MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match register.architecture() {
        Architecture::Aarch64 => {
            aarch64::encode_entry_argument_register_write_bytes(register, byte_offset, byte_size)
        }
        Architecture::X86_64 => {
            x86_64::encode_entry_argument_register_write_bytes(register, byte_offset, byte_size)
        }
    }
}

/// Copy one normalized incoming stack-argument fragment into its entry-frame
/// destination. Target encoders add their ABI return-address/prologue bias.
pub fn encode_entry_stack_argument_write_bytes(
    architecture: Architecture,
    stack_byte_offset: u32,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_entry_stack_argument_write_bytes(
            stack_byte_offset,
            byte_offset,
            byte_size,
        ),
        Architecture::X86_64 => x86_64::encode_entry_stack_argument_write_bytes(
            stack_byte_offset,
            byte_offset,
            byte_size,
        ),
    }
}

/// The entry prologue's `args: &[u8]` descriptor write (x86_64 only).
pub fn encode_entry_arguments_slice_descriptor_write_bytes(
    architecture: Architecture,
    descriptor_offset: usize,
    spill_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_entry_arguments_slice_descriptor_write_bytes(
            descriptor_offset,
            spill_offset,
            byte_length,
        ),
        Architecture::X86_64 => x86_64::encode_entry_arguments_slice_descriptor_write_bytes(
            descriptor_offset,
            spill_offset,
            byte_length,
        ),
    }
}

pub fn encode_return_register_integer_write_bytes(
    architecture: Architecture,
    register: omega_calling_conventions::MachineRegister,
    byte_size: usize,
    value: i64,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    if register.architecture() != architecture {
        return Err(Diagnostic::error(format!(
            "result register {register:?} does not belong to target architecture {architecture:?}"
        )));
    }
    match architecture {
        Architecture::Aarch64 => {
            let bytes =
                aarch64::encode_return_register_integer_write_bytes(register, byte_size, value)?
                    .to_vec();
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
        Architecture::X86_64 => {
            let bytes =
                x86_64::encode_return_register_integer_write_bytes(register, byte_size, value)?;
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
    }
}

#[cfg(test)]
mod result_register_architecture_tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;

    #[test]
    fn result_registers_cannot_cross_target_architectures() {
        let error = encode_return_register_integer_write_bytes(
            Architecture::Aarch64,
            MachineRegister::X86Rax,
            4,
            0,
        )
        .expect_err("foreign result register must reject");
        assert!(
            error
                .message
                .contains("does not belong to target architecture")
        );
    }
}

#[cfg(test)]
mod aarch64_import_plan_tests {
    use super::*;
    use omega_target_operations::{
        RuntimeStorageRegion, TargetInstructionOperand, TargetInstructionOperandKind,
    };

    fn operand(kind: TargetInstructionOperandKind) -> TargetInstructionOperand {
        TargetInstructionOperand { kind }
    }

    #[test]
    fn mixed_import_arguments_use_independent_aapcs_register_banks() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 8,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(3)),
        ];

        let (parameters, result) =
            normalized_aarch64_import_registers(&operands, Aarch64ImportResult::None, false)
                .expect("register-resident mixed AAPCS call");

        assert_eq!(
            parameters,
            [
                ValueLocation::Register {
                    register: MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
                ValueLocation::Register {
                    register: MachineRegister::Aarch64V(0),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
                ValueLocation::Register {
                    register: MachineRegister::Aarch64X(1),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ]
        );
        assert_eq!(result, None);
    }

    #[test]
    fn outbound_stack_arguments_flow_to_the_encoder() {
        let operands = (0..9)
            .map(|value| operand(TargetInstructionOperandKind::ImmediateInteger(value)))
            .collect::<Vec<_>>();

        let (locations, result) =
            normalized_aarch64_import_registers(&operands, Aarch64ImportResult::None, false)
                .expect("ninth AAPCS integer argument has a scalar stack placement");

        assert_eq!(result, None);
        assert_eq!(
            locations[8],
            ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            }
        );
    }
}

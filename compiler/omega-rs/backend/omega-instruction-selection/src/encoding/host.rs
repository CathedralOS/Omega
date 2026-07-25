use crate::aarch64_call_operand;
use omega_calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, EntryControl, HostOperationKey, MachineRegister,
    ValueLocation, ValuePlacement, ValueShape, evaluate_call_plan, validate_call_plan,
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
    normalized_syscall_registers_with_plan(architecture, parameter_count, has_result, None)
}

fn normalized_syscall_registers_with_plan(
    architecture: Architecture,
    parameter_count: usize,
    has_result: bool,
    authoritative_plan: Option<&CallPlan>,
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
    let plan = if let Some(plan) = authoritative_plan {
        validate_call_plan(plan, &signature).map_err(|error| {
            Diagnostic::error(format!(
                "source-selected syscall plan does not match the lowered signature: {error}"
            ))
        })?;
        plan.clone()
    } else {
        evaluate_call_plan(policy, &signature).map_err(|error| {
            Diagnostic::error(format!("cannot evaluate syscall call plan: {error}"))
        })?
    };
    let (number, immediate) = validate_normalized_syscall_plan(architecture, &plan)?;
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
    Ok(NormalizedSyscallRegisters {
        parameters,
        result,
        number,
        immediate,
    })
}

fn validate_normalized_syscall_plan(
    architecture: Architecture,
    plan: &CallPlan,
) -> Result<(MachineRegister, u16), Diagnostic> {
    let expected_policy = match architecture {
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
    };
    let EntryControl::SupervisorCall {
        number_register,
        immediate,
    } = plan.entry_control
    else {
        return Err(Diagnostic::error(
            "normalized syscall plan did not select supervisor-call entry control",
        ));
    };
    if plan.policy != expected_policy
        || plan.stack_alignment != 16
        || plan.shadow_bytes != 0
        || (architecture == Architecture::X86_64 && immediate != 0)
    {
        return Err(Diagnostic::error(format!(
            "syscall encoder cannot realize plan policy={:?}, control={:?}, alignment={}, shadow_bytes={}",
            plan.policy, plan.entry_control, plan.stack_alignment, plan.shadow_bytes
        )));
    }

    let fixed_scratch = match architecture {
        Architecture::Aarch64 => &[][..],
        Architecture::X86_64 => &[MachineRegister::X86Rax, MachineRegister::X86R11][..],
    };
    for scratch in fixed_scratch.iter().copied().chain([number_register]) {
        if !plan.ordinary_clobbers.contains(scratch) {
            return Err(Diagnostic::error(format!(
                "syscall encoder scratch register {scratch:?} exceeds the plan's ordinary-clobber ceiling"
            )));
        }
    }
    if plan.parameters.iter().any(|placement| {
        placement.locations.iter().any(|location| {
            matches!(
                location,
                ValueLocation::Register { register, .. } if *register == number_register
            )
        })
    }) {
        return Err(Diagnostic::error(format!(
            "syscall number register {number_register:?} overlaps a parameter placement"
        )));
    }

    Ok((number_register, immediate))
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

/// A `Binding::VtableSlot` external-leaf call (per-object dispatch).
pub fn encode_vtable_call_sequence<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    index: i64,
) -> Result<Vec<u8>, Diagnostic> {
    encode_vtable_call_sequence_with_plan(target, operands, index, None)
}

pub fn encode_vtable_call_sequence_with_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    index: i64,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    match target.architecture {
        Architecture::Aarch64 => {
            let (placements, result) =
                normalized_aarch64_vtable_plan_with_plan(operands, false, authoritative_plan)?;
            debug_assert!(result.is_none());
            aarch64::encode_vtable_call_sequence_from_operands(
                operands.iter().map(aarch64_call_operand),
                &placements,
                index,
            )
        }
        Architecture::X86_64
            if authoritative_plan
                .map(|plan| plan.policy)
                .unwrap_or_else(|| CallingPolicy::native_for_target(target))
                == CallingPolicy::MicrosoftX64 =>
        {
            x86_64::encode_win64_vtable_call_with_plan(operands, index, authoritative_plan)
        }
        Architecture::X86_64
            if authoritative_plan
                .map(|plan| plan.policy)
                .unwrap_or_else(|| CallingPolicy::native_for_target(target))
                == CallingPolicy::SystemVAMD64 =>
        {
            let byte_offset = index
                .checked_mul(8)
                .ok_or_else(|| Diagnostic::error("vtable slot index overflows a byte offset"))?;
            x86_64::encode_sysv_vtable_call_with_plan(
                operands,
                byte_offset,
                false,
                authoritative_plan,
            )
        }
        Architecture::X86_64 => Err(Diagnostic::error(
            "x86-64 vtable compatibility encoder requires Microsoft x64 or SysV AMD64",
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
    encode_vtable_call_sequence_at_offset_with_plan(
        target,
        operands,
        byte_offset,
        result_present,
        None,
    )
}

pub fn encode_vtable_call_sequence_at_offset_with_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    byte_offset: usize,
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    match target.architecture {
        Architecture::Aarch64 => {
            let (arguments, result) = normalized_aarch64_vtable_plan_with_plan(
                operands,
                result_present,
                authoritative_plan,
            )?;
            if result_present {
                let result = result.as_ref().ok_or_else(|| {
                    Diagnostic::error("AAPCS64 vtable plan omitted its required result")
                })?;
                match result.shape.class {
                    omega_calling_conventions::ValueClass::Integer
                        if result.shape.byte_size > 16 =>
                    {
                        aarch64::encode_vtable_call_sequence_at_offset_indirect_returning_from_operands(
                            operands.iter().map(aarch64_call_operand),
                            &arguments,
                            result,
                            byte_offset,
                        )
                    }
                    omega_calling_conventions::ValueClass::Integer
                        if result.shape.byte_size > 8 =>
                    {
                        aarch64::encode_vtable_call_sequence_at_offset_small_aggregate_returning_from_operands(
                            operands.iter().map(aarch64_call_operand),
                            &arguments,
                            result,
                            byte_offset,
                        )
                    }
                    omega_calling_conventions::ValueClass::Integer => {
                        aarch64::encode_vtable_call_sequence_at_offset_value_returning_from_operands(
                            operands.iter().map(aarch64_call_operand),
                            &arguments,
                            scalar_result_register(Some(result), "vtable")?,
                            byte_offset,
                        )
                    }
                    omega_calling_conventions::ValueClass::Float => {
                        aarch64::encode_vtable_call_sequence_at_offset_float_returning_from_operands(
                            operands.iter().map(aarch64_call_operand),
                            &arguments,
                            scalar_result_register(Some(result), "vtable float")?,
                            byte_offset,
                        )
                    }
                    omega_calling_conventions::ValueClass::HomogeneousFloatAggregate { .. } => {
                        aarch64::encode_vtable_call_sequence_at_offset_hfa_returning_from_operands(
                            operands.iter().map(aarch64_call_operand),
                            &arguments,
                            result,
                            byte_offset,
                        )
                    }
                    omega_calling_conventions::ValueClass::SystemVAggregate { .. } => Err(
                        Diagnostic::error("SysV aggregate class reached AAPCS64 vtable encoding"),
                    ),
                }
            } else {
                debug_assert!(result.is_none());
                aarch64::encode_vtable_call_sequence_at_offset_from_operands(
                    operands.iter().map(aarch64_call_operand),
                    &arguments,
                    byte_offset,
                )
            }
        }
        Architecture::X86_64
            if authoritative_plan
                .map(|plan| plan.policy)
                .unwrap_or_else(|| CallingPolicy::native_for_target(target))
                == CallingPolicy::MicrosoftX64 =>
        {
            x86_64::encode_win64_vtable_call_at_offset_with_plan(
                operands,
                i64::try_from(byte_offset)
                    .map_err(|_| Diagnostic::error("vtable field offset overflows i64"))?,
                result_present,
                authoritative_plan,
            )
        }
        Architecture::X86_64
            if authoritative_plan
                .map(|plan| plan.policy)
                .unwrap_or_else(|| CallingPolicy::native_for_target(target))
                == CallingPolicy::SystemVAMD64 =>
        {
            x86_64::encode_sysv_vtable_call_with_plan(
                operands,
                i64::try_from(byte_offset)
                    .map_err(|_| Diagnostic::error("vtable field offset overflows i64"))?,
                result_present,
                authoritative_plan,
            )
        }
        Architecture::X86_64 => Err(Diagnostic::error(
            "x86-64 vtable-field compatibility encoder requires Microsoft x64 or SysV AMD64",
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
    encode_table_function_call_sequence_with_plan(
        target,
        operands,
        byte_offset,
        result_present,
        None,
    )
}

pub fn encode_table_function_call_sequence_with_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    byte_offset: usize,
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    match target.architecture {
        Architecture::Aarch64 => {
            let (arguments, result) = normalized_aarch64_table_function_plan_with_plan(
                operands,
                result_present,
                authoritative_plan,
            )?;
            match result.as_ref().map(|result| result.shape.class) {
                None => aarch64::encode_table_function_call_sequence_from_operands(
                    operands.iter().map(aarch64_call_operand),
                    &arguments,
                    None,
                    byte_offset,
                ),
                Some(omega_calling_conventions::ValueClass::Integer)
                    if result
                        .as_ref()
                        .is_some_and(|result| result.shape.byte_size > 16) =>
                {
                    aarch64::encode_table_function_call_sequence_indirect_returning_from_operands(
                        operands.iter().map(aarch64_call_operand),
                        &arguments,
                        result.as_ref().expect("matched present result"),
                        byte_offset,
                    )
                }
                Some(omega_calling_conventions::ValueClass::Integer)
                    if result
                        .as_ref()
                        .is_some_and(|result| result.shape.byte_size > 8) =>
                {
                    aarch64::encode_table_function_call_sequence_small_aggregate_returning_from_operands(
                        operands.iter().map(aarch64_call_operand),
                        &arguments,
                        result.as_ref().expect("matched present result"),
                        byte_offset,
                    )
                }
                Some(omega_calling_conventions::ValueClass::Integer) => {
                    aarch64::encode_table_function_call_sequence_from_operands(
                        operands.iter().map(aarch64_call_operand),
                        &arguments,
                        Some(scalar_result_register(result.as_ref(), "table-function")?),
                        byte_offset,
                    )
                }
                Some(omega_calling_conventions::ValueClass::Float) => {
                    aarch64::encode_table_function_call_sequence_float_returning_from_operands(
                        operands.iter().map(aarch64_call_operand),
                        &arguments,
                        scalar_result_register(result.as_ref(), "table-function float")?,
                        byte_offset,
                    )
                }
                Some(omega_calling_conventions::ValueClass::HomogeneousFloatAggregate {
                    ..
                }) => aarch64::encode_table_function_call_sequence_hfa_returning_from_operands(
                    operands.iter().map(aarch64_call_operand),
                    &arguments,
                    result.as_ref().expect("matched present result"),
                    byte_offset,
                ),
                Some(omega_calling_conventions::ValueClass::SystemVAggregate { .. }) => Err(
                    Diagnostic::error(
                        "SysV aggregate class reached AAPCS64 table-function encoding",
                    ),
                ),
            }
        }
        Architecture::X86_64
            if authoritative_plan
                .map(|plan| plan.policy)
                .unwrap_or_else(|| CallingPolicy::native_for_target(target))
                == CallingPolicy::MicrosoftX64 =>
        {
            x86_64::encode_win64_table_function_call_with_plan(
                operands,
                i64::try_from(byte_offset)
                    .map_err(|_| Diagnostic::error("service table field offset overflows i64"))?,
                result_present,
                authoritative_plan,
            )
        }
        Architecture::X86_64
            if authoritative_plan
                .map(|plan| plan.policy)
                .unwrap_or_else(|| CallingPolicy::native_for_target(target))
                == CallingPolicy::SystemVAMD64 =>
        {
            x86_64::encode_sysv_table_function_call_with_plan(
                operands,
                i64::try_from(byte_offset)
                    .map_err(|_| Diagnostic::error("service table field offset overflows i64"))?,
                result_present,
                authoritative_plan,
            )
        }
        Architecture::X86_64 => Err(Diagnostic::error(
            "x86-64 table-function compatibility encoder requires Microsoft x64 or SysV AMD64",
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
                normalized_aarch64_import_plan(operands, Aarch64ImportResult::Integer, false)?;
            aarch64::encode_host_call_sequence_value_returning_deref_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
                scalar_result_register(result.as_ref(), "integer")?,
            )
        }
        // Stack-mode ops (`open_create`) also share `returns_value()` but bracket
        // the call with `sub sp`/`str [sp]`/`add sp` to pass the variadic `mode`
        // on the stack; checked before the plain value-returning arm.
        Architecture::Aarch64 if operation_key.passes_trailing_mode_on_stack() => {
            let (arguments, result) =
                normalized_aarch64_import_plan(operands, Aarch64ImportResult::Integer, true)?;
            aarch64::encode_host_call_sequence_value_returning_open_create_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
                scalar_result_register(result.as_ref(), "integer")?,
            )
        }
        // Float-returning ops (sqrt/hypot) also share `returns_value()` but the
        // result comes back in `d0`; the encoder inserts `fmov x0, d0` before the
        // result store. Checked before the plain value-returning arm.
        Architecture::Aarch64 if operation_key.returns_float() => {
            let (arguments, result) =
                normalized_aarch64_import_plan(operands, Aarch64ImportResult::Float, false)?;
            aarch64::encode_host_call_sequence_value_returning_float_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
                scalar_result_register(result.as_ref(), "float")?,
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
                normalized_aarch64_import_plan(operands, Aarch64ImportResult::Integer, false)?;
            aarch64::encode_host_call_sequence_value_returning_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
                scalar_result_register(result.as_ref(), "integer")?,
            )
        }
        Architecture::Aarch64 => {
            let (arguments, result) =
                normalized_aarch64_import_plan(operands, Aarch64ImportResult::None, false)?;
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

/// A source-authored external IMPORT call (custom capability): the
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
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    match target.architecture {
        Architecture::Aarch64 => {
            let (arguments, result) = normalized_aarch64_import_plan_with_authoritative(
                operands,
                Aarch64ImportResult::Authored,
                false,
                authoritative_plan,
            )?;
            let result = result.as_ref().ok_or_else(|| {
                Diagnostic::error("AArch64 authored import has no normalized result placement")
            })?;
            match result.shape.class {
                omega_calling_conventions::ValueClass::HomogeneousFloatAggregate { .. } => {
                    aarch64::encode_host_call_sequence_hfa_returning_from_operands(
                        operands.iter().map(aarch64_call_operand),
                        &arguments,
                        result,
                    )
                }
                omega_calling_conventions::ValueClass::Float => {
                    aarch64::encode_host_call_sequence_authored_float_returning_from_operands(
                        operands.iter().map(aarch64_call_operand),
                        &arguments,
                        scalar_result_register(Some(result), "authored float")?,
                    )
                }
                omega_calling_conventions::ValueClass::Integer if result.shape.byte_size > 8 => {
                    if result.shape.byte_size > 16 {
                        aarch64::encode_host_call_sequence_indirect_returning_from_operands(
                            operands.iter().map(aarch64_call_operand),
                            &arguments,
                            result,
                        )
                    } else {
                        aarch64::encode_host_call_sequence_small_aggregate_returning_from_operands(
                            operands.iter().map(aarch64_call_operand),
                            &arguments,
                            result,
                        )
                    }
                }
                omega_calling_conventions::ValueClass::Integer => {
                    aarch64::encode_host_call_sequence_value_returning_from_operands(
                        operands.iter().map(aarch64_call_operand),
                        &arguments,
                        scalar_result_register(Some(result), "authored")?,
                    )
                }
                omega_calling_conventions::ValueClass::SystemVAggregate { .. } => Err(
                    Diagnostic::error("SysV aggregate class reached AAPCS64 import encoding"),
                ),
            }
        }
        Architecture::X86_64 => match authoritative_plan {
            Some(plan) => x86_64::encode_authored_import_call_sequence(plan, operands),
            None => x86_64::encode_host_call_sequence(
                CallingPolicy::native_for_target(target),
                operation_key,
                operands,
            ),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Aarch64ImportResult {
    None,
    Integer,
    Float,
    Authored,
}

pub fn normalized_aarch64_host_argument_placements<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
    authored_import: bool,
) -> Result<Vec<ValuePlacement>, Diagnostic> {
    normalized_aarch64_host_argument_placements_with_plan(
        operation_key,
        operands,
        authored_import,
        None,
    )
}

pub fn normalized_aarch64_host_argument_placements_with_plan<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
    authored_import: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<ValuePlacement>, Diagnostic> {
    let result_kind = if authored_import {
        Aarch64ImportResult::Authored
    } else if operation_key.dereferences_result() {
        Aarch64ImportResult::Integer
    } else if operation_key.returns_float() {
        Aarch64ImportResult::Float
    } else if operation_key.returns_value() {
        Aarch64ImportResult::Integer
    } else {
        Aarch64ImportResult::None
    };
    normalized_aarch64_import_plan_with_authoritative(
        operands,
        result_kind,
        operation_key.passes_trailing_mode_on_stack(),
        authoritative_plan,
    )
    .map(|(placements, _)| placements)
}

pub fn normalized_aarch64_vtable_argument_placements<T: InstructionOperandLike>(
    operands: &[T],
) -> Result<Vec<ValuePlacement>, Diagnostic> {
    normalized_aarch64_vtable_plan(operands, false).map(|(placements, _)| placements)
}

pub fn normalized_aarch64_vtable_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
) -> Result<(Vec<ValuePlacement>, Option<ValuePlacement>), Diagnostic> {
    normalized_aarch64_vtable_plan_with_plan(operands, result_present, None)
}

pub fn normalized_aarch64_vtable_plan_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<(Vec<ValuePlacement>, Option<ValuePlacement>), Diagnostic> {
    let (placements, result) = normalized_aarch64_import_plan_with_authoritative(
        operands,
        if result_present {
            Aarch64ImportResult::Authored
        } else {
            Aarch64ImportResult::None
        },
        false,
        authoritative_plan,
    )?;
    debug_assert_eq!(result.is_some(), result_present);
    if !matches!(
        placements
            .first()
            .map(|placement| placement.locations.as_slice()),
        Some([ValueLocation::Register {
            register: MachineRegister::Aarch64X(0),
            value_byte_offset: 0,
            byte_size: 8,
        }])
    ) {
        return Err(Diagnostic::error(
            "AAPCS64 vtable call requires one full-width receiver in x0",
        ));
    }
    if let Some(result) = result.as_ref() {
        validate_aarch64_field_result(result, "vtable")?;
    }
    Ok((placements, result))
}

pub fn normalized_aarch64_table_function_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
) -> Result<(Vec<ValuePlacement>, Option<ValuePlacement>), Diagnostic> {
    normalized_aarch64_table_function_plan_with_plan(operands, result_present, None)
}

pub fn normalized_aarch64_table_function_plan_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<(Vec<ValuePlacement>, Option<ValuePlacement>), Diagnostic> {
    let lowered = operands
        .iter()
        .map(aarch64_call_operand)
        .collect::<Vec<_>>();
    let table_index = usize::from(result_present);
    let Some(table) = lowered.get(table_index) else {
        return Err(Diagnostic::error(
            "AAPCS64 table-function call has no dispatch table operand",
        ));
    };
    if !matches!(
        table,
        omega_isa_aarch64::Aarch64CallOperand::RuntimeScalarInteger { byte_count: 8, .. }
    ) {
        return Err(Diagnostic::error(
            "AAPCS64 table-function dispatch table must be an eight-byte runtime scalar",
        ));
    }

    let mut wire_operands = Vec::with_capacity(lowered.len() - 1);
    if result_present {
        wire_operands.push(lowered[0]);
    }
    wire_operands.extend_from_slice(&lowered[table_index + 1..]);
    let (placements, result) =
        normalized_aarch64_import_plan_from_call_operands_with_authoritative(
            &wire_operands,
            if result_present {
                Aarch64ImportResult::Authored
            } else {
                Aarch64ImportResult::None
            },
            false,
            authoritative_plan,
        )?;
    debug_assert_eq!(result.is_some(), result_present);
    if let Some(result) = result.as_ref() {
        validate_aarch64_field_result(result, "table-function")?;
    }
    Ok((placements, result))
}

fn validate_aarch64_field_result(result: &ValuePlacement, label: &str) -> Result<(), Diagnostic> {
    let locations_match = match result.shape.class {
        omega_calling_conventions::ValueClass::Integer if result.shape.byte_size <= 8 => {
            matches!(
                result.locations.as_slice(),
                [ValueLocation::Register {
                    register: MachineRegister::Aarch64X(_),
                    value_byte_offset: 0,
                    ..
                }]
            )
        }
        omega_calling_conventions::ValueClass::Integer if result.shape.byte_size > 16 => {
            matches!(
                result.locations.as_slice(),
                [ValueLocation::Indirect {
                    pointer: omega_calling_conventions::IndirectPointerLocation::Register(
                        MachineRegister::Aarch64X(8)
                    ),
                    copy_stack_byte_offset: None,
                    byte_size,
                    alignment,
                }] if *byte_size == result.shape.byte_size
                    && *alignment == result.shape.alignment
            )
        }
        omega_calling_conventions::ValueClass::Integer => {
            result.locations.len() == usize::from(result.shape.byte_size.div_ceil(8))
                && result
                    .locations
                    .iter()
                    .enumerate()
                    .all(|(fragment, location)| {
                        matches!(
                            location,
                            ValueLocation::Register {
                                register: MachineRegister::Aarch64X(_),
                                value_byte_offset,
                                byte_size,
                            } if usize::from(*value_byte_offset) == fragment * 8
                                && usize::from(*byte_size)
                                    == (usize::from(result.shape.byte_size) - fragment * 8).min(8)
                        )
                    })
        }
        omega_calling_conventions::ValueClass::Float => matches!(
            result.locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::Aarch64V(_),
                value_byte_offset: 0,
                ..
            }]
        ),
        omega_calling_conventions::ValueClass::HomogeneousFloatAggregate { members } => {
            result.locations.len() == usize::from(members)
                && result.locations.iter().all(|location| {
                    matches!(
                        location,
                        ValueLocation::Register {
                            register: MachineRegister::Aarch64V(_),
                            ..
                        }
                    )
                })
        }
        omega_calling_conventions::ValueClass::SystemVAggregate { .. } => false,
    };
    if !locations_match {
        return Err(Diagnostic::error(format!(
            "AAPCS64 {label} result shape {:?} has unsupported placement {:?}",
            result.shape, result.locations
        )));
    }
    Ok(())
}

pub fn aarch64_host_call_stack_prefix_width_for_placements(
    placements: &[ValuePlacement],
    argument_count: usize,
) -> usize {
    omega_isa_aarch64::aarch64::host_call_stack_prefix_width_for_placements(
        placements,
        argument_count,
    )
}

pub fn aarch64_host_call_stack_total_width_for_placements(placements: &[ValuePlacement]) -> usize {
    omega_isa_aarch64::aarch64::host_call_stack_total_width_for_placements(placements)
}

/// ENT2c: evaluate the AAPCS64 call surface from the actual selected operands.
/// The encoder receives exact register/stack locations and may no longer
/// reconstruct x0../v0.. or outgoing offsets independently. Scalar integer,
/// pointer, and float stack placements plus register-resident flat HFA
/// fragments, contiguous HFA stack placements, and authored HFA results are
/// supported.
///
/// `trailing_variadic_stack` is the compatibility seam for Darwin `open`:
/// its anonymous `mode` argument is intentionally stack-passed by Apple's
/// variadic ABI and is not yet representable in `CallSignature`. The named
/// arguments and result still consume the normalized plan here; the final
/// stack operand remains with the existing checked special-case encoder.
fn normalized_aarch64_import_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_kind: Aarch64ImportResult,
    trailing_variadic_stack: bool,
) -> Result<(Vec<ValuePlacement>, Option<ValuePlacement>), Diagnostic> {
    normalized_aarch64_import_plan_with_authoritative(
        operands,
        result_kind,
        trailing_variadic_stack,
        None,
    )
}

fn normalized_aarch64_import_plan_with_authoritative<T: InstructionOperandLike>(
    operands: &[T],
    result_kind: Aarch64ImportResult,
    trailing_variadic_stack: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<(Vec<ValuePlacement>, Option<ValuePlacement>), Diagnostic> {
    let aarch64_operands = operands
        .iter()
        .map(aarch64_call_operand)
        .collect::<Vec<_>>();
    normalized_aarch64_import_plan_from_call_operands_with_authoritative(
        &aarch64_operands,
        result_kind,
        trailing_variadic_stack,
        authoritative_plan,
    )
}

fn normalized_aarch64_import_plan_from_call_operands_with_authoritative(
    aarch64_operands: &[omega_isa_aarch64::Aarch64CallOperand],
    result_kind: Aarch64ImportResult,
    trailing_variadic_stack: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<(Vec<ValuePlacement>, Option<ValuePlacement>), Diagnostic> {
    let (result_operand, mut arguments) = match result_kind {
        Aarch64ImportResult::None => (None, aarch64_operands),
        Aarch64ImportResult::Integer
        | Aarch64ImportResult::Float
        | Aarch64ImportResult::Authored => {
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
            (Aarch64ImportResult::Authored, Some(operand)) => Some(aarch64_operand_shape(operand)?),
            _ => {
                return Err(Diagnostic::error(
                    "AArch64 import result classification is internally inconsistent",
                ));
            }
        },
    };
    let plan = if let Some(plan) = authoritative_plan {
        validate_call_plan(plan, &signature).map_err(|error| {
            Diagnostic::error(format!(
                "source-selected AArch64 import plan does not match the lowered signature: {error}"
            ))
        })?;
        plan.clone()
    } else {
        evaluate_call_plan(CallingPolicy::Aapcs64, &signature).map_err(|error| {
            Diagnostic::error(format!("cannot evaluate AAPCS64 import plan: {error}"))
        })?
    };
    validate_aarch64_import_plan(&plan)?;
    for (index, placement) in plan.parameters.iter().enumerate() {
        if placement.locations.len() > 1
            && !matches!(
                placement.shape.class,
                omega_calling_conventions::ValueClass::HomogeneousFloatAggregate { .. }
            )
            && !(matches!(
                placement.shape.class,
                omega_calling_conventions::ValueClass::Integer
            ) && (9..=16).contains(&placement.shape.byte_size))
        {
            return Err(Diagnostic::error(format!(
                "AAPCS64 import parameter {index} has unsupported fragmented placement {:?}",
                placement.locations
            )));
        }
    }
    if let Some(result) = plan.result.as_ref()
        && result.locations.len() > 1
        && !matches!(
            result.shape.class,
            omega_calling_conventions::ValueClass::HomogeneousFloatAggregate { .. }
        )
        && !(matches!(
            result.shape.class,
            omega_calling_conventions::ValueClass::Integer
        ) && (9..=16).contains(&result.shape.byte_size))
    {
        return Err(Diagnostic::error(format!(
            "AAPCS64 import result has unsupported fragmented placement {:?}",
            result.locations
        )));
    }
    Ok((plan.parameters, plan.result))
}

fn validate_aarch64_import_plan(plan: &CallPlan) -> Result<(), Diagnostic> {
    if plan.policy != CallingPolicy::Aapcs64
        || plan.entry_control != EntryControl::CallReturn
        || plan.stack_alignment != 16
        || plan.shadow_bytes != 0
    {
        return Err(Diagnostic::error(format!(
            "AArch64 import encoder cannot realize plan policy={:?}, control={:?}, alignment={}, shadow_bytes={}",
            plan.policy, plan.entry_control, plan.stack_alignment, plan.shadow_bytes
        )));
    }

    // The current encoder family uses these caller-saved registers while
    // materializing stack arguments, large offsets, floating-point values,
    // and result stores. Keep that implementation footprint inside the
    // plan's ordinary-clobber ceiling instead of treating the placement
    // projection as the whole calling contract.
    for scratch in [
        MachineRegister::Aarch64X(0),
        MachineRegister::Aarch64X(9),
        MachineRegister::Aarch64X(10),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64V(31),
    ] {
        if !plan.ordinary_clobbers.contains(scratch) {
            return Err(Diagnostic::error(format!(
                "AArch64 import encoder scratch register {scratch:?} exceeds the plan's ordinary-clobber ceiling"
            )));
        }
    }

    Ok(())
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
        Aarch64CallOperand::RuntimeHomogeneousFloatAggregate {
            member_byte_count,
            members,
            ..
        } => {
            let member_byte_count = u16::try_from(member_byte_count)
                .map_err(|_| Diagnostic::error("AArch64 HFA import member width exceeds u16"))?;
            Ok(ValueShape::homogeneous_float_aggregate(
                member_byte_count,
                members,
            ))
        }
        Aarch64CallOperand::RuntimeSmallAggregate {
            byte_count,
            alignment,
            ..
        } => {
            let byte_count = u16::try_from(byte_count).map_err(|_| {
                Diagnostic::error("AArch64 small aggregate operand width exceeds u16")
            })?;
            let alignment = u16::try_from(alignment)
                .map_err(|_| Diagnostic::error("AArch64 small aggregate alignment exceeds u16"))?;
            Ok(ValueShape::integer(byte_count, alignment))
        }
        Aarch64CallOperand::RuntimeLargeAggregate {
            byte_count,
            alignment,
            ..
        } => {
            let byte_count = u16::try_from(byte_count).map_err(|_| {
                Diagnostic::error("AArch64 large aggregate operand width exceeds u16")
            })?;
            let alignment = u16::try_from(alignment)
                .map_err(|_| Diagnostic::error("AArch64 large aggregate alignment exceeds u16"))?;
            Ok(ValueShape::integer(byte_count, alignment))
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
            "AAPCS64 import {role} {index} does not have one scalar register location"
        ))),
    }
}

fn scalar_result_register(
    result: Option<&ValuePlacement>,
    result_kind: &str,
) -> Result<MachineRegister, Diagnostic> {
    let result = result.ok_or_else(|| {
        Diagnostic::error(format!(
            "AArch64 {result_kind}-returning import has no normalized result placement"
        ))
    })?;
    one_register(&result.locations, "result", 0)
}

pub fn encode_syscall_sequence<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    encode_syscall_sequence_with_plan(architecture, operands, syscall_number, None)
}

pub fn encode_syscall_sequence_with_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    let registers = normalized_syscall_registers_with_plan(
        architecture,
        operands.len(),
        false,
        authoritative_plan,
    )?;

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

fn encode_linux_timespec_syscall_with_site<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: Option<&CallPlan>,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    if operands.len() != 2 {
        return Err(Diagnostic::error(
            "Linux timespec lowering requires one semantic result and one injected clock id",
        ));
    }
    let registers =
        normalized_syscall_registers_with_plan(architecture, 2, true, authoritative_plan)?;
    let result_register = registers.required_result()?;
    match architecture {
        Architecture::Aarch64 => aarch64::encode_linux_timespec_syscall(
            &operands
                .iter()
                .map(aarch64_call_operand)
                .collect::<Vec<_>>(),
            syscall_number,
            &registers.parameters,
            result_register,
            registers.number,
            registers.immediate,
        ),
        Architecture::X86_64 => {
            let (bytes, site) = x86_64::encode_linux_timespec_syscall(
                operands,
                syscall_number,
                &registers.parameters,
                result_register,
                registers.number,
                registers.immediate,
            )?;
            Ok((bytes, site.byte_offset))
        }
    }
}

pub fn encode_linux_timespec_syscall_with_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    encode_linux_timespec_syscall_with_site(
        architecture,
        operands,
        syscall_number,
        authoritative_plan,
    )
    .map(|(bytes, _)| bytes)
}

pub fn linux_timespec_result_relocation_byte_offset<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: Option<&CallPlan>,
) -> Result<usize, Diagnostic> {
    encode_linux_timespec_syscall_with_site(
        architecture,
        operands,
        syscall_number,
        authoritative_plan,
    )
    .map(|(_, byte_offset)| byte_offset)
}

pub fn encode_constant_host_result<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_host_call_sequence_constant_result_from_operands(
            operands.iter().map(aarch64_call_operand),
        ),
        Architecture::X86_64 => x86_64::encode_constant_result(operands),
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
        Architecture::X86_64 => {
            let bytes = x86_64::encode_function_enter_bytes().to_vec();
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
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

pub fn encode_generated_idt_load_bytes(
    architecture: Architecture,
    pointer_register: omega_calling_conventions::MachineRegister,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Err(Diagnostic::error(
            "generated IDT load is x86_64-only; no AArch64 lowering exists",
        )),
        Architecture::X86_64 => x86_64::encode_generated_idt_load_bytes(pointer_register),
    }
}

pub fn encode_generated_idt_writer_bytes(
    architecture: Architecture,
    pointer_register: omega_calling_conventions::MachineRegister,
    byte_len: usize,
    little_endian: bool,
    context_abi: u64,
    source_slot_count: usize,
    steps: &[omega_target_operations::GeneratedIdtWriterStep],
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Err(Diagnostic::error(
            "generated IDT writer is x86_64-only; no AArch64 lowering exists",
        )),
        Architecture::X86_64 => omega_isa_x86_64::encode_generated_idt_writer_bytes(
            pointer_register,
            byte_len,
            little_endian,
            context_abi,
            source_slot_count,
            steps,
        ),
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

pub fn encode_entry_indirect_argument_write_bytes(
    architecture: Architecture,
    pointer: omega_calling_conventions::IndirectPointerLocation,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_entry_indirect_argument_write_bytes(pointer, byte_offset, byte_size)
        }
        Architecture::X86_64 => {
            x86_64::encode_entry_indirect_argument_write_bytes(pointer, byte_offset, byte_size)
        }
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
mod syscall_plan_contract_tests {
    use super::*;
    use omega_calling_conventions::RegisterSet;
    use omega_target_operations::{TargetInstructionOperand, TargetInstructionOperandKind};

    #[test]
    fn normalized_syscall_plans_cover_their_encoder_scratch() {
        for (architecture, policy) in [
            (Architecture::X86_64, CallingPolicy::LinuxSyscallX86_64),
            (Architecture::Aarch64, CallingPolicy::LinuxSyscallAarch64),
        ] {
            let plan = evaluate_call_plan(policy, &CallSignature::default())
                .expect("baseline syscall plan");
            validate_normalized_syscall_plan(architecture, &plan)
                .expect("encoder scratch is inside the plan ceiling");
        }
    }

    #[test]
    fn syscall_plan_rejects_scratch_above_its_clobber_ceiling() {
        let mut plan = evaluate_call_plan(
            CallingPolicy::LinuxSyscallAarch64,
            &CallSignature::default(),
        )
        .expect("baseline AArch64 syscall plan");
        plan.ordinary_clobbers = RegisterSet::new(
            plan.ordinary_clobbers
                .as_slice()
                .iter()
                .copied()
                .filter(|register| *register != MachineRegister::Aarch64X(8)),
        );

        let error = validate_normalized_syscall_plan(Architecture::Aarch64, &plan)
            .expect_err("number-register scratch above ceiling must reject");
        assert!(error.message.contains("Aarch64X(8)"));
        assert!(error.message.contains("ordinary-clobber ceiling"));
    }

    #[test]
    fn syscall_plan_rejects_an_incompatible_stack_contract() {
        let mut plan =
            evaluate_call_plan(CallingPolicy::LinuxSyscallX86_64, &CallSignature::default())
                .expect("baseline x86 syscall plan");
        plan.stack_alignment = 32;

        let error = validate_normalized_syscall_plan(Architecture::X86_64, &plan)
            .expect_err("unsupported stack alignment must reject");
        assert!(error.message.contains("alignment=32"));
    }

    #[test]
    fn source_selected_syscall_argument_registers_reach_both_encoders() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        };
        let operands = [TargetInstructionOperand {
            kind: TargetInstructionOperandKind::ImmediateInteger(7),
        }];

        let mut x86_plan = evaluate_call_plan(CallingPolicy::LinuxSyscallX86_64, &signature)
            .expect("baseline x86-64 syscall plan");
        x86_plan.parameters[0].locations[0] = ValueLocation::Register {
            register: MachineRegister::X86R10,
            value_byte_offset: 0,
            byte_size: 8,
        };
        let x86_bytes =
            encode_syscall_sequence_with_plan(Architecture::X86_64, &operands, 60, Some(&x86_plan))
                .expect("source-selected x86-64 syscall register");
        assert_eq!(&x86_bytes[..2], &[0x49, 0xba]);
        assert_eq!(
            x86_bytes.len(),
            crate::syscall_sequence_width_with_plan(
                Architecture::X86_64,
                &operands,
                60,
                Some(&x86_plan),
            )
        );

        let mut aarch64_plan = evaluate_call_plan(CallingPolicy::LinuxSyscallAarch64, &signature)
            .expect("baseline AArch64 syscall plan");
        aarch64_plan.parameters[0].locations[0] = ValueLocation::Register {
            register: MachineRegister::Aarch64X(3),
            value_byte_offset: 0,
            byte_size: 8,
        };
        let aarch64_bytes = encode_syscall_sequence_with_plan(
            Architecture::Aarch64,
            &operands,
            93,
            Some(&aarch64_plan),
        )
        .expect("source-selected AArch64 syscall register");
        assert_eq!(&aarch64_bytes[..4], &[0xe3, 0x00, 0x80, 0xd2]);
        assert_eq!(
            aarch64_bytes.len(),
            crate::syscall_sequence_width_with_plan(
                Architecture::Aarch64,
                &operands,
                93,
                Some(&aarch64_plan),
            )
        );
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
mod sysv_field_call_tests {
    use super::*;
    use omega_target_operations::{
        RuntimeStorageRegion, TargetInstructionOperand, TargetInstructionOperandKind,
    };

    fn scalar(byte_offset: usize) -> TargetInstructionOperand {
        TargetInstructionOperand {
            kind: TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count: 8,
            },
        }
    }

    #[test]
    fn linux_x64_routes_vtable_fields_through_the_sysv_encoder() {
        let operands = [scalar(0), scalar(8), scalar(16)];
        let target = omega_target::NativeTarget::linux_x64();
        let bytes = encode_vtable_call_sequence_at_offset(target, &operands, 24, true)
            .expect("SysV vtable-field call");

        assert_eq!(
            bytes.len(),
            crate::vtable_call_sequence_width_at_offset(target, &operands, 24, true)
        );
        assert!(
            bytes
                .windows(9)
                .any(|window| window == [0x48, 0x8b, 0x87, 24, 0, 0, 0, 0xff, 0xd0])
        );
    }

    #[test]
    fn source_selected_sysv_vtable_plan_overrides_a_windows_target() {
        let operands = [scalar(0), scalar(8), scalar(16)];
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 2],
            result: Some(ValueShape::integer(8, 8)),
        };
        let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
            .expect("source-selected SysV field plan");
        let target = omega_target::NativeTarget::windows_x64();
        let bytes = encode_vtable_call_sequence_at_offset_with_plan(
            target,
            &operands,
            24,
            true,
            Some(&plan),
        )
        .expect("SysV vtable-field call in a PE image");

        assert!(
            bytes
                .windows(9)
                .any(|window| window == [0x48, 0x8b, 0x87, 24, 0, 0, 0, 0xff, 0xd0]),
            "the dispatch receiver must be read from source-selected rdi"
        );
        assert_eq!(
            bytes.len(),
            crate::vtable_call_sequence_width_at_offset_with_plan(
                target,
                &operands,
                24,
                true,
                Some(&plan),
            )
        );
    }

    #[test]
    fn linux_x64_routes_table_functions_without_passing_the_table() {
        let operands = [scalar(0), scalar(8), scalar(16)];
        let target = omega_target::NativeTarget::linux_x64();
        let bytes = encode_table_function_call_sequence(target, &operands, 40, true)
            .expect("SysV table-function call");

        assert_eq!(
            bytes.len(),
            crate::table_function_call_sequence_width(target, &operands, 40, true)
        );
        assert!(
            bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xbb, 16, 0, 0, 0]),
            "the first declared argument must use rdi"
        );
    }

    #[test]
    fn source_selected_table_plan_excludes_dispatch_storage_on_windows() {
        let operands = [scalar(0), scalar(8), scalar(16)];
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: Some(ValueShape::integer(8, 8)),
        };
        let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
            .expect("source-selected SysV table-function plan");
        let target = omega_target::NativeTarget::windows_x64();
        let bytes =
            encode_table_function_call_sequence_with_plan(target, &operands, 40, true, Some(&plan))
                .expect("SysV table-function call in a PE image");

        assert!(
            bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xbb, 16, 0, 0, 0]),
            "the dispatch table must be excluded so the declared argument uses rdi"
        );
        assert_eq!(
            bytes.len(),
            crate::table_function_call_sequence_width_with_plan(
                target,
                &operands,
                40,
                true,
                Some(&plan),
            )
        );
    }
}

#[cfg(test)]
mod aarch64_import_plan_tests {
    use super::*;
    use omega_calling_conventions::RegisterSet;
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
            normalized_aarch64_import_plan(&operands, Aarch64ImportResult::None, false)
                .expect("register-resident mixed AAPCS call");

        assert_eq!(
            parameters
                .iter()
                .flat_map(|placement| placement.locations.iter().copied())
                .collect::<Vec<_>>(),
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
    fn vtable_receiver_is_selected_as_the_aapcs_x0_argument() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(7)),
        ];

        let placements = normalized_aarch64_vtable_argument_placements(&operands)
            .expect("AAPCS64 vtable placements");
        assert!(matches!(
            placements[0].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::Aarch64X(0),
                value_byte_offset: 0,
                byte_size: 8,
            }]
        ));
        assert!(matches!(
            placements[1].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::Aarch64X(1),
                value_byte_offset: 0,
                byte_size: 8,
            }]
        ));
    }

    #[test]
    fn source_selected_aarch64_vtable_plan_controls_non_receiver_arguments() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(7)),
        ];
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 2],
            result: None,
        };
        let mut plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
            .expect("baseline AAPCS64 vtable plan");
        plan.parameters[1].locations[0] = ValueLocation::Register {
            register: MachineRegister::Aarch64X(3),
            value_byte_offset: 0,
            byte_size: 8,
        };

        let target = omega_target::NativeTarget::linux_arm64();
        let bytes = encode_vtable_call_sequence_at_offset_with_plan(
            target,
            &operands,
            24,
            false,
            Some(&plan),
        )
        .expect("source-selected AAPCS64 vtable plan");

        assert!(
            bytes
                .windows(4)
                .any(|window| window == [0xe3, 0x00, 0x80, 0xd2])
        );
        assert_eq!(
            bytes.len(),
            crate::vtable_call_sequence_width_at_offset_with_plan(
                target,
                &operands,
                24,
                false,
                Some(&plan),
            )
        );
    }

    #[test]
    fn vtable_field_result_is_separate_from_the_x0_receiver_argument() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 4,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(7)),
        ];

        let (placements, result) =
            normalized_aarch64_vtable_plan(&operands, true).expect("AAPCS64 vtable field plan");
        assert!(matches!(
            result.expect("result placement").locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::Aarch64X(0),
                value_byte_offset: 0,
                byte_size: 4,
            }]
        ));
        assert!(matches!(
            placements[0].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::Aarch64X(0),
                value_byte_offset: 0,
                byte_size: 8,
            }]
        ));
        assert!(matches!(
            placements[1].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::Aarch64X(1),
                value_byte_offset: 0,
                byte_size: 8,
            }]
        ));

        let bytes = encode_vtable_call_sequence_at_offset(
            omega_target::NativeTarget::linux_arm64(),
            &operands,
            24,
            true,
        )
        .expect("encode AAPCS64 vtable field result");
        assert_eq!(
            bytes.len(),
            crate::vtable_call_sequence_width_at_offset(
                omega_target::NativeTarget::linux_arm64(),
                &operands,
                24,
                true,
            )
        );
        assert_eq!(
            crate::vtable_call_sequence_width_at_offset(
                omega_target::NativeTarget::linux_arm64(),
                &operands,
                32_768,
                true,
            ),
            0
        );
    }

    #[test]
    fn outbound_stack_arguments_flow_to_the_encoder() {
        let operands = (0..9)
            .map(|value| operand(TargetInstructionOperandKind::ImmediateInteger(value)))
            .collect::<Vec<_>>();

        let (locations, result) =
            normalized_aarch64_import_plan(&operands, Aarch64ImportResult::None, false)
                .expect("ninth AAPCS integer argument has a scalar stack placement");

        assert_eq!(result, None);
        assert_eq!(
            locations[8].locations[0],
            ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            }
        );
    }

    #[test]
    fn table_function_pointer_is_excluded_from_the_aapcs_signature() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 4,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 40,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(7)),
        ];

        let (placements, result) = normalized_aarch64_table_function_plan(&operands, true)
            .expect("AAPCS64 table-function plan");
        assert!(matches!(
            result.expect("result placement").locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::Aarch64X(0),
                value_byte_offset: 0,
                byte_size: 4,
            }]
        ));
        assert_eq!(placements.len(), 1);
        assert!(matches!(
            placements[0].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::Aarch64X(0),
                value_byte_offset: 0,
                byte_size: 8,
            }]
        ));

        let bytes = encode_table_function_call_sequence(
            omega_target::NativeTarget::linux_arm64(),
            &operands,
            24,
            true,
        )
        .expect("encode AAPCS64 table-function result");
        assert_eq!(
            bytes.len(),
            crate::table_function_call_sequence_width(
                omega_target::NativeTarget::linux_arm64(),
                &operands,
                24,
                true,
            )
        );
    }

    #[test]
    fn field_model_vector_results_match_layout_widths() {
        let vtable_operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            }),
        ];
        let vtable_bytes = encode_vtable_call_sequence_at_offset(
            omega_target::NativeTarget::linux_arm64(),
            &vtable_operands,
            24,
            true,
        )
        .expect("encode float-returning AAPCS64 vtable field");
        assert_eq!(vtable_bytes.len(), 36);
        assert_eq!(
            vtable_bytes.len(),
            crate::vtable_call_sequence_width_at_offset(
                omega_target::NativeTarget::linux_arm64(),
                &vtable_operands,
                24,
                true,
            )
        );

        let table_operands = [
            operand(
                TargetInstructionOperandKind::RuntimeHomogeneousFloatAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 64,
                    member_byte_count: 8,
                    members: 2,
                },
            ),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 40,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(7)),
        ];
        let table_bytes = encode_table_function_call_sequence(
            omega_target::NativeTarget::linux_arm64(),
            &table_operands,
            24,
            true,
        )
        .expect("encode HFA-returning AAPCS64 table function");
        assert_eq!(table_bytes.len(), 48);
        assert_eq!(
            table_bytes.len(),
            crate::table_function_call_sequence_width(
                omega_target::NativeTarget::linux_arm64(),
                &table_operands,
                24,
                true,
            )
        );
    }

    #[test]
    fn field_model_small_aggregate_results_match_layout_widths() {
        let aggregate_result = || {
            operand(TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
                byte_count: 16,
                alignment: 8,
            })
        };
        let receiver = || {
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            })
        };

        let vtable_operands = [aggregate_result(), receiver()];
        let vtable_bytes = encode_vtable_call_sequence_at_offset(
            omega_target::NativeTarget::linux_arm64(),
            &vtable_operands,
            24,
            true,
        )
        .expect("encode small-aggregate-returning AAPCS64 vtable field");
        assert_eq!(
            vtable_bytes.len(),
            crate::vtable_call_sequence_width_at_offset(
                omega_target::NativeTarget::linux_arm64(),
                &vtable_operands,
                24,
                true,
            )
        );

        let table_operands = [aggregate_result(), receiver()];
        let table_bytes = encode_table_function_call_sequence(
            omega_target::NativeTarget::linux_arm64(),
            &table_operands,
            24,
            true,
        )
        .expect("encode small-aggregate-returning AAPCS64 table function");
        assert_eq!(
            table_bytes.len(),
            crate::table_function_call_sequence_width(
                omega_target::NativeTarget::linux_arm64(),
                &table_operands,
                24,
                true,
            )
        );
    }

    #[test]
    fn field_model_large_aggregate_results_match_layout_widths() {
        let aggregate_result = || {
            operand(TargetInstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
                byte_count: 24,
                alignment: 8,
            })
        };
        let pointer = || {
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            })
        };

        let vtable_operands = [aggregate_result(), pointer()];
        let vtable_bytes = encode_vtable_call_sequence_at_offset(
            omega_target::NativeTarget::linux_arm64(),
            &vtable_operands,
            24,
            true,
        )
        .expect("encode indirect-returning AAPCS64 vtable field");
        assert_eq!(
            vtable_bytes.len(),
            crate::vtable_call_sequence_width_at_offset(
                omega_target::NativeTarget::linux_arm64(),
                &vtable_operands,
                24,
                true,
            )
        );

        let table_operands = [aggregate_result(), pointer()];
        let table_bytes = encode_table_function_call_sequence(
            omega_target::NativeTarget::linux_arm64(),
            &table_operands,
            24,
            true,
        )
        .expect("encode indirect-returning AAPCS64 table function");
        assert_eq!(
            table_bytes.len(),
            crate::table_function_call_sequence_width(
                omega_target::NativeTarget::linux_arm64(),
                &table_operands,
                24,
                true,
            )
        );
    }

    #[test]
    fn ninth_float_argument_has_an_aapcs_stack_slot() {
        let operands = (0..9)
            .map(|index| {
                operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: index * 8,
                    byte_count: 8,
                })
            })
            .collect::<Vec<_>>();

        let (locations, _) =
            normalized_aarch64_import_plan(&operands, Aarch64ImportResult::None, false)
                .expect("ninth AAPCS float argument has a stack placement");
        assert_eq!(
            locations[8].locations[0],
            ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            }
        );
    }

    #[test]
    fn hfa_operand_keeps_one_value_with_fragmented_vector_locations() {
        let operands = [operand(
            TargetInstructionOperandKind::RuntimeHomogeneousFloatAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 24,
                member_byte_count: 8,
                members: 3,
            },
        )];

        let (placements, result) =
            normalized_aarch64_import_plan(&operands, Aarch64ImportResult::None, false)
                .expect("three-member HFA plan");
        assert_eq!(result, None);
        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].locations.len(), 3);
        assert!(matches!(
            placements[0].locations[2],
            ValueLocation::Register {
                register: MachineRegister::Aarch64V(2),
                value_byte_offset: 16,
                byte_size: 8,
            }
        ));
    }

    #[test]
    fn authored_hfa_result_keeps_its_fragmented_vector_placement() {
        let operands = [
            operand(
                TargetInstructionOperandKind::RuntimeHomogeneousFloatAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 24,
                    member_byte_count: 8,
                    members: 2,
                },
            ),
            operand(TargetInstructionOperandKind::ImmediateInteger(7)),
        ];

        let (parameters, result) =
            normalized_aarch64_import_plan(&operands, Aarch64ImportResult::Authored, false)
                .expect("authored HFA result plan");
        let result = result.expect("fragmented result placement");

        assert_eq!(parameters.len(), 1);
        assert_eq!(result.locations.len(), 2);
        assert!(matches!(
            result.locations[0],
            ValueLocation::Register {
                register: MachineRegister::Aarch64V(0),
                value_byte_offset: 0,
                byte_size: 8,
            }
        ));
        assert!(matches!(
            result.locations[1],
            ValueLocation::Register {
                register: MachineRegister::Aarch64V(1),
                value_byte_offset: 8,
                byte_size: 8,
            }
        ));
    }

    #[test]
    fn authored_large_aggregate_call_uses_indirect_argument_and_result() {
        let aggregate = || {
            operand(TargetInstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
                byte_count: 24,
                alignment: 8,
            })
        };
        let operands = [aggregate(), aggregate()];
        let (parameters, result) =
            normalized_aarch64_import_plan(&operands, Aarch64ImportResult::Authored, false)
                .expect("authored large aggregate call plan");

        assert!(matches!(
            parameters[0].locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: omega_calling_conventions::IndirectPointerLocation::Register(
                    MachineRegister::Aarch64X(0)
                ),
                copy_stack_byte_offset: Some(0),
                ..
            }]
        ));
        assert!(matches!(
            result.expect("indirect result").locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: omega_calling_conventions::IndirectPointerLocation::Register(
                    MachineRegister::Aarch64X(8)
                ),
                copy_stack_byte_offset: None,
                ..
            }]
        ));

        let bytes = encode_authored_import_call_sequence(
            omega_target::NativeTarget::linux_arm64(),
            HostOperationKey::default(),
            &operands,
            None,
        )
        .expect("authored indirect aggregate call");
        assert_eq!(
            bytes.len(),
            crate::host_call_sequence_width(
                omega_target::NativeTarget::linux_arm64(),
                HostOperationKey::default(),
                &operands,
            )
        );
    }

    #[test]
    fn authored_scalar_float_result_routes_through_the_vector_store() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(7)),
        ];
        let bytes = encode_authored_import_call_sequence(
            omega_target::NativeTarget::linux_arm64(),
            HostOperationKey::default(),
            &operands,
            None,
        )
        .expect("authored scalar-float import");

        assert_eq!(bytes.len(), 24);
        assert_eq!(
            bytes.len(),
            crate::host_call_sequence_width(
                omega_target::NativeTarget::linux_arm64(),
                HostOperationKey::default(),
                &operands,
            )
        );
    }

    #[test]
    fn authored_import_uses_the_source_selected_aarch64_register() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(7)),
        ];
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: Some(ValueShape::integer(8, 8)),
        };
        let mut plan =
            evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("baseline AAPCS64 plan");
        plan.parameters[0].locations[0] = ValueLocation::Register {
            register: MachineRegister::Aarch64X(3),
            value_byte_offset: 0,
            byte_size: 8,
        };

        let bytes = encode_authored_import_call_sequence(
            omega_target::NativeTarget::linux_arm64(),
            HostOperationKey::default(),
            &operands,
            Some(&plan),
        )
        .expect("authored import with source-selected x3 argument");

        assert_eq!(&bytes[..4], &[0xe3, 0x00, 0x80, 0xd2]);
        assert_eq!(
            bytes.len(),
            crate::authored_import_call_sequence_width(
                omega_target::NativeTarget::linux_arm64(),
                HostOperationKey::default(),
                &operands,
                Some(&plan),
            )
        );
    }

    #[test]
    fn import_encoder_rejects_incompatible_plan_control_and_stack_contracts() {
        let mut plan = evaluate_call_plan(CallingPolicy::Aapcs64, &CallSignature::default())
            .expect("baseline AAPCS64 plan");

        plan.entry_control = EntryControl::InterruptReturn;
        let error = validate_aarch64_import_plan(&plan).expect_err("interrupt return must reject");
        assert!(error.message.contains("cannot realize plan"));

        plan.entry_control = EntryControl::CallReturn;
        plan.stack_alignment = 8;
        let error = validate_aarch64_import_plan(&plan).expect_err("weak alignment must reject");
        assert!(error.message.contains("alignment=8"));

        plan.stack_alignment = 16;
        plan.shadow_bytes = 32;
        let error = validate_aarch64_import_plan(&plan).expect_err("shadow space must reject");
        assert!(error.message.contains("shadow_bytes=32"));
    }

    #[test]
    fn import_encoder_rejects_scratch_above_the_plan_clobber_ceiling() {
        let mut plan = evaluate_call_plan(CallingPolicy::Aapcs64, &CallSignature::default())
            .expect("baseline AAPCS64 plan");
        plan.ordinary_clobbers = RegisterSet::new(
            plan.ordinary_clobbers
                .as_slice()
                .iter()
                .copied()
                .filter(|register| *register != MachineRegister::Aarch64X(17)),
        );

        let error = validate_aarch64_import_plan(&plan).expect_err("missing scratch must reject");
        assert!(error.message.contains("Aarch64X(17)"));
        assert!(error.message.contains("ordinary-clobber ceiling"));
    }
}

use crate::aarch64_call_operand;
use omega_calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, ConcreteVariadicCallSignature, EntryControl,
    HostCapability, HostOperation, HostOperationKey, MachineRegister, ValueLocation,
    ValuePlacement, ValueShape, evaluate_call_plan, evaluate_darwin_aapcs64_variadic_call_plan,
    validate_call_plan,
};
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::InstructionOperandLike;
use psi_diagnostics::Diagnostic;

pub(super) struct NormalizedSyscallRegisters {
    pub parameters: Vec<omega_calling_conventions::MachineRegister>,
    pub result: Option<omega_calling_conventions::MachineRegister>,
    pub number: omega_calling_conventions::MachineRegister,
    pub immediate: u16,
}

#[derive(Clone, Copy)]
enum HostImportPlan<'plan> {
    CompatibilityOracle,
    Authoritative(&'plan CallPlan),
}

impl<'plan> HostImportPlan<'plan> {
    fn parameter_shape(self, index: usize) -> Option<ValueShape> {
        match self {
            Self::CompatibilityOracle => None,
            Self::Authoritative(plan) => {
                plan.parameters.get(index).map(|placement| placement.shape)
            }
        }
    }

    fn result_shape(self) -> Option<ValueShape> {
        match self {
            Self::CompatibilityOracle => None,
            Self::Authoritative(plan) => plan.result.as_ref().map(|result| result.shape),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum SyscallPlan<'plan> {
    CompatibilityOracle,
    Authoritative(&'plan CallPlan),
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

pub(super) fn normalized_syscall_registers_for_plan(
    architecture: Architecture,
    parameter_count: usize,
    has_result: bool,
    plan_source: SyscallPlan<'_>,
) -> Result<NormalizedSyscallRegisters, Diagnostic> {
    let policy = match architecture {
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
    };
    let word = ValueShape::integer(8, 8);
    // `has_result` describes the lowered Omega operand list, not necessarily
    // the native ABI. Statement-shaped adapters may retain a native status or
    // count in their authoritative plan while intentionally discarding it.
    // Preserve that result in validation without requiring a synthetic leading
    // result operand; the statement encoder simply ignores its placement.
    let retained_result = match plan_source {
        SyscallPlan::Authoritative(plan) if !has_result => {
            plan.result.as_ref().map(|placement| placement.shape)
        }
        _ => has_result.then_some(word),
    };
    let signature = CallSignature {
        parameters: vec![word; parameter_count],
        result: retained_result,
    };
    let plan = match plan_source {
        SyscallPlan::Authoritative(plan) => {
            validate_call_plan(plan, &signature).map_err(|error| {
                Diagnostic::error(format!(
                    "source-selected syscall plan does not match the lowered signature: {error}"
                ))
            })?;
            plan.clone()
        }
        SyscallPlan::CompatibilityOracle => {
            evaluate_call_plan(policy, &signature).map_err(|error| {
                Diagnostic::error(format!("cannot evaluate syscall call plan: {error}"))
            })?
        }
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
pub fn encode_vtable_call_sequence_with_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    index: i64,
    authoritative_plan: &CallPlan,
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
        Architecture::X86_64 if authoritative_plan.policy == CallingPolicy::MicrosoftX64 => {
            x86_64::encode_win64_vtable_call_with_plan(operands, index, authoritative_plan)
        }
        Architecture::X86_64 if authoritative_plan.policy == CallingPolicy::SystemVAMD64 => {
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
            "x86-64 vtable encoder requires a Microsoft x64 or SysV AMD64 plan",
        )),
    }
}

/// The FIELD-MODEL flavor (extern brief SS12.1): the byte offset came from
/// the vtable struct's layout via the backend's vtable-field pass. When
/// `result_present`, operand 0 is the RESULT place and the store tail runs.
pub fn encode_vtable_call_sequence_at_offset_with_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    byte_offset: usize,
    result_present: bool,
    authoritative_plan: &CallPlan,
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
        Architecture::X86_64 if authoritative_plan.policy == CallingPolicy::MicrosoftX64 => {
            x86_64::encode_win64_vtable_call_at_offset_with_plan(
                operands,
                i64::try_from(byte_offset)
                    .map_err(|_| Diagnostic::error("vtable field offset overflows i64"))?,
                result_present,
                authoritative_plan,
            )
        }
        Architecture::X86_64 if authoritative_plan.policy == CallingPolicy::SystemVAMD64 => {
            x86_64::encode_sysv_vtable_call_with_plan(
                operands,
                i64::try_from(byte_offset)
                    .map_err(|_| Diagnostic::error("vtable field offset overflows i64"))?,
                result_present,
                authoritative_plan,
            )
        }
        Architecture::X86_64 => Err(Diagnostic::error(
            "x86-64 vtable-field encoder requires a Microsoft x64 or SysV AMD64 plan",
        )),
    }
}

/// A SERVICE-TABLE function call: field-model dispatch where the table
/// pointer is dispatch-only, never a wire argument (EFI table services take
/// no This; protocol/COM methods do).
pub fn encode_table_function_call_sequence_with_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    byte_offset: usize,
    result_present: bool,
    authoritative_plan: &CallPlan,
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
        Architecture::X86_64 if authoritative_plan.policy == CallingPolicy::MicrosoftX64 => {
            x86_64::encode_win64_table_function_call_with_plan(
                operands,
                i64::try_from(byte_offset)
                    .map_err(|_| Diagnostic::error("service table field offset overflows i64"))?,
                result_present,
                authoritative_plan,
            )
        }
        Architecture::X86_64 if authoritative_plan.policy == CallingPolicy::SystemVAMD64 => {
            x86_64::encode_sysv_table_function_call_with_plan(
                operands,
                i64::try_from(byte_offset)
                    .map_err(|_| Diagnostic::error("service table field offset overflows i64"))?,
                result_present,
                authoritative_plan,
            )
        }
        Architecture::X86_64 => Err(Diagnostic::error(
            "x86-64 table-function encoder requires a Microsoft x64 or SysV AMD64 plan",
        )),
    }
}

pub fn encode_host_call_sequence_no_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operation_key: HostOperationKey,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    encode_host_call_sequence_for_plan(
        target,
        operation_key,
        operands,
        HostImportPlan::CompatibilityOracle,
    )
}

pub fn encode_host_call_sequence_with_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operation_key: HostOperationKey,
    operands: &[T],
    authoritative_plan: &CallPlan,
) -> Result<Vec<u8>, Diagnostic> {
    encode_host_call_sequence_for_plan(
        target,
        operation_key,
        operands,
        HostImportPlan::Authoritative(authoritative_plan),
    )
}

fn encode_host_call_sequence_for_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operation_key: HostOperationKey,
    operands: &[T],
    plan_source: HostImportPlan<'_>,
) -> Result<Vec<u8>, Diagnostic> {
    let discards_native_result = operation_key.discards_native_result();
    let returns_float = match plan_source {
        HostImportPlan::Authoritative(plan) => {
            !discards_native_result
                && plan.result.as_ref().is_some_and(|result| {
                    matches!(
                        result.shape.class,
                        omega_calling_conventions::ValueClass::Float
                    )
                })
        }
        HostImportPlan::CompatibilityOracle => operation_key.returns_float(),
    };
    let returns_value = match plan_source {
        HostImportPlan::Authoritative(plan) => !discards_native_result && plan.result.is_some(),
        HostImportPlan::CompatibilityOracle => operation_key.returns_value(),
    };
    match target.architecture {
        // Deref-result ops (errno) must be checked before the plain
        // value-returning arm: they share `returns_value()` but insert an extra
        // `ldr` to deref the returned pointer.
        Architecture::Aarch64 if operation_key.dereferences_result() => {
            let (arguments, result) = normalized_aarch64_import_plan_for_plan(
                operands,
                Aarch64ImportResult::Integer,
                plan_source,
            )?;
            aarch64::encode_host_call_sequence_value_returning_deref_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
                scalar_result_register(result.as_ref(), "integer")?,
                scalar_result_byte_count(result.as_ref(), "integer")?,
            )
        }
        // Darwin `open_create` owns a concrete adapter subcall signature whose
        // promoted anonymous mode parameter is stack-placed by its normalized
        // Apple AAPCS64 plan.
        Architecture::Aarch64 if is_open_create(operation_key) => {
            let (arguments, result) = normalized_darwin_open_create_plan(operands, plan_source)?;
            aarch64::encode_host_call_sequence_value_returning_open_create_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
                scalar_result_register(result.as_ref(), "integer")?,
                scalar_result_byte_count(result.as_ref(), "integer")?,
            )
        }
        // Float-returning ops (sqrt/hypot) also share `returns_value()` but the
        // result comes back in `d0`; the encoder inserts `fmov x0, d0` before the
        // result store. Checked before the plain value-returning arm.
        Architecture::Aarch64 if returns_float => {
            let (arguments, result) = normalized_aarch64_import_plan_for_plan(
                operands,
                Aarch64ImportResult::Float,
                plan_source,
            )?;
            aarch64::encode_host_call_sequence_value_returning_float_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
                scalar_result_register(result.as_ref(), "float")?,
                scalar_result_byte_count(result.as_ref(), "float")?,
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
        Architecture::Aarch64 if returns_value => {
            let (arguments, result) = normalized_aarch64_import_plan_for_plan(
                operands,
                Aarch64ImportResult::Integer,
                plan_source,
            )?;
            aarch64::encode_host_call_sequence_value_returning_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
                scalar_result_register(result.as_ref(), "integer")?,
                scalar_result_byte_count(result.as_ref(), "integer")?,
            )
        }
        Architecture::Aarch64 => {
            let result_kind = if discards_native_result
                && matches!(plan_source, HostImportPlan::Authoritative(plan) if plan.result.is_some())
            {
                Aarch64ImportResult::Ignored
            } else {
                Aarch64ImportResult::None
            };
            let (arguments, result) =
                normalized_aarch64_import_plan_for_plan(operands, result_kind, plan_source)?;
            debug_assert_eq!(
                result.is_some(),
                result_kind == Aarch64ImportResult::Ignored
            );
            aarch64::encode_host_call_sequence_from_operands(
                operands.iter().map(aarch64_call_operand),
                &arguments,
            )
        }
        Architecture::X86_64 => match plan_source {
            HostImportPlan::Authoritative(plan) => x86_64::encode_host_call_sequence_with_plan(
                CallingPolicy::native_for_target(target),
                operation_key,
                operands,
                plan,
            ),
            HostImportPlan::CompatibilityOracle => x86_64::encode_host_call_sequence_no_plan(
                CallingPolicy::native_for_target(target),
                operation_key,
                operands,
            ),
        },
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
    operands: &[T],
    authoritative_plan: &CallPlan,
) -> Result<Vec<u8>, Diagnostic> {
    match target.architecture {
        Architecture::Aarch64 => {
            let (arguments, result) = normalized_aarch64_import_plan_for_plan(
                operands,
                Aarch64ImportResult::Authored,
                HostImportPlan::Authoritative(authoritative_plan),
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
                        scalar_result_byte_count(Some(result), "authored")?,
                    )
                }
                omega_calling_conventions::ValueClass::SystemVAggregate { .. } => Err(
                    Diagnostic::error("SysV aggregate class reached AAPCS64 import encoding"),
                ),
            }
        }
        Architecture::X86_64 => {
            x86_64::encode_authored_import_call_sequence(authoritative_plan, operands)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Aarch64ImportResult {
    None,
    Ignored,
    Integer,
    Float,
    Authored,
}

pub fn normalized_aarch64_host_argument_placements_no_plan<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
    authored_import: bool,
) -> Result<Vec<ValuePlacement>, Diagnostic> {
    normalized_aarch64_host_argument_placements_for_plan(
        operation_key,
        operands,
        authored_import,
        HostImportPlan::CompatibilityOracle,
    )
}

pub fn normalized_aarch64_host_argument_placements_with_plan<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
    authored_import: bool,
    authoritative_plan: &CallPlan,
) -> Result<Vec<ValuePlacement>, Diagnostic> {
    normalized_aarch64_host_argument_placements_for_plan(
        operation_key,
        operands,
        authored_import,
        HostImportPlan::Authoritative(authoritative_plan),
    )
}

fn normalized_aarch64_host_argument_placements_for_plan<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
    authored_import: bool,
    plan_source: HostImportPlan<'_>,
) -> Result<Vec<ValuePlacement>, Diagnostic> {
    if is_open_create(operation_key) {
        return normalized_darwin_open_create_plan(operands, plan_source)
            .map(|(placements, _)| placements);
    }
    let result_kind = if authored_import {
        Aarch64ImportResult::Authored
    } else if operation_key.dereferences_result() {
        Aarch64ImportResult::Integer
    } else if operation_key.discards_native_result()
        && matches!(plan_source, HostImportPlan::Authoritative(plan) if plan.result.is_some())
    {
        Aarch64ImportResult::Ignored
    } else if let HostImportPlan::Authoritative(plan) = plan_source {
        match plan.result.as_ref().map(|result| result.shape.class) {
            Some(omega_calling_conventions::ValueClass::Float) => Aarch64ImportResult::Float,
            Some(_) => Aarch64ImportResult::Integer,
            None => Aarch64ImportResult::None,
        }
    } else if operation_key.returns_float() {
        Aarch64ImportResult::Float
    } else if operation_key.returns_value() {
        Aarch64ImportResult::Integer
    } else {
        Aarch64ImportResult::None
    };
    normalized_aarch64_import_plan_for_plan(operands, result_kind, plan_source)
        .map(|(placements, _)| placements)
}

pub fn normalized_aarch64_vtable_plan_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
    authoritative_plan: &CallPlan,
) -> Result<(Vec<ValuePlacement>, Option<ValuePlacement>), Diagnostic> {
    let (placements, result) = normalized_aarch64_import_plan_for_plan(
        operands,
        if result_present {
            Aarch64ImportResult::Authored
        } else {
            Aarch64ImportResult::None
        },
        HostImportPlan::Authoritative(authoritative_plan),
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

pub fn normalized_aarch64_table_function_plan_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
    authoritative_plan: &CallPlan,
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
    let (placements, result) = normalized_aarch64_import_plan_from_call_operands_for_plan(
        &wire_operands,
        if result_present {
            Aarch64ImportResult::Authored
        } else {
            Aarch64ImportResult::None
        },
        HostImportPlan::Authoritative(authoritative_plan),
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

fn normalized_darwin_open_create_plan<T: InstructionOperandLike>(
    operands: &[T],
    plan_source: HostImportPlan<'_>,
) -> Result<(Vec<ValuePlacement>, Option<ValuePlacement>), Diagnostic> {
    use omega_isa_aarch64::Aarch64CallOperand;

    let operands = operands
        .iter()
        .map(aarch64_call_operand)
        .collect::<Vec<_>>();
    let Some((result, arguments)) = operands.split_first() else {
        return Err(Diagnostic::error(
            "Darwin open_create import has no result storage operand",
        ));
    };
    let [path, flags, mode] = arguments else {
        return Err(Diagnostic::error(
            "Darwin open_create import requires path, flags, and mode operands",
        ));
    };
    let result = aarch64_result_shape(*result, false, plan_source.result_shape())?;
    if result != ValueShape::integer(4, 4)
        || aarch64_operand_shape(*path)? != ValueShape::integer(8, 8)
        || !matches!(
            flags,
            Aarch64CallOperand::ImmediateInteger(_)
                | Aarch64CallOperand::RuntimeScalarInteger { byte_count: 4, .. }
        )
        || !matches!(mode, Aarch64CallOperand::ImmediateInteger(_))
    {
        return Err(Diagnostic::error(
            "Darwin open_create concrete subcall must be int open(pointer, int, promoted int)",
        ));
    }

    let signature = ConcreteVariadicCallSignature {
        fixed_parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(4, 4)],
        variadic_parameters: vec![ValueShape::integer(4, 4)],
        result: Some(result),
    };
    let evaluated = evaluate_darwin_aapcs64_variadic_call_plan(&signature).map_err(|error| {
        Diagnostic::error(format!(
            "cannot evaluate Darwin AAPCS64 open_create plan: {error}"
        ))
    })?;
    let plan = match plan_source {
        HostImportPlan::Authoritative(plan) => {
            validate_call_plan(plan, &signature.flattened()).map_err(|error| {
                Diagnostic::error(format!(
                    "retained Darwin open_create plan does not match its concrete signature: {error}"
                ))
            })?;
            if plan != &evaluated {
                return Err(Diagnostic::error(
                    "retained Darwin open_create plan does not preserve the Apple variadic parameter boundary",
                ));
            }
            plan.clone()
        }
        HostImportPlan::CompatibilityOracle => evaluated,
    };
    validate_aarch64_import_plan(&plan)?;
    Ok((plan.parameters, plan.result))
}

fn is_open_create(operation_key: HostOperationKey) -> bool {
    matches!(
        (operation_key.capability, operation_key.operation),
        (HostCapability::Filesystem, HostOperation::OpenCreate)
    )
}

/// ENT2c: evaluate the AAPCS64 call surface from the actual selected operands.
/// The encoder receives exact register/stack locations and may no longer
/// reconstruct x0../v0.. or outgoing offsets independently. Scalar integer,
/// pointer, and float stack placements plus register-resident flat HFA
/// fragments, contiguous HFA stack placements, and authored HFA results are
/// supported.
#[cfg(test)]
fn normalized_aarch64_import_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_kind: Aarch64ImportResult,
) -> Result<(Vec<ValuePlacement>, Option<ValuePlacement>), Diagnostic> {
    normalized_aarch64_import_plan_for_plan(
        operands,
        result_kind,
        HostImportPlan::CompatibilityOracle,
    )
}

fn normalized_aarch64_import_plan_for_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_kind: Aarch64ImportResult,
    plan_source: HostImportPlan<'_>,
) -> Result<(Vec<ValuePlacement>, Option<ValuePlacement>), Diagnostic> {
    let aarch64_operands = operands
        .iter()
        .map(aarch64_call_operand)
        .collect::<Vec<_>>();
    normalized_aarch64_import_plan_from_call_operands_for_plan(
        &aarch64_operands,
        result_kind,
        plan_source,
    )
}

fn normalized_aarch64_import_plan_from_call_operands_for_plan(
    aarch64_operands: &[omega_isa_aarch64::Aarch64CallOperand],
    result_kind: Aarch64ImportResult,
    plan_source: HostImportPlan<'_>,
) -> Result<(Vec<ValuePlacement>, Option<ValuePlacement>), Diagnostic> {
    let (result_operand, arguments) = match result_kind {
        Aarch64ImportResult::None | Aarch64ImportResult::Ignored => (None, aarch64_operands),
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
    let signature = CallSignature {
        parameters: arguments
            .iter()
            .copied()
            .enumerate()
            .map(|(index, operand)| {
                aarch64_operand_shape_with_context(operand, plan_source.parameter_shape(index))
            })
            .collect::<Result<Vec<_>, _>>()?,
        result: match (result_kind, result_operand) {
            (Aarch64ImportResult::None, None) => None,
            (Aarch64ImportResult::Ignored, None) => plan_source.result_shape(),
            (Aarch64ImportResult::Integer, Some(operand)) => Some(aarch64_result_shape(
                operand,
                false,
                plan_source.result_shape(),
            )?),
            (Aarch64ImportResult::Float, Some(operand)) => Some(aarch64_result_shape(
                operand,
                true,
                plan_source.result_shape(),
            )?),
            (Aarch64ImportResult::Authored, Some(operand)) => Some(aarch64_operand_shape(operand)?),
            _ => {
                return Err(Diagnostic::error(
                    "AArch64 import result classification is internally inconsistent",
                ));
            }
        },
    };
    let plan = match plan_source {
        HostImportPlan::Authoritative(plan) => {
            validate_call_plan(plan, &signature).map_err(|error| {
                Diagnostic::error(format!(
                    "source-selected AArch64 import plan does not match the lowered signature: {error}"
                ))
            })?;
            plan.clone()
        }
        HostImportPlan::CompatibilityOracle => {
            evaluate_call_plan(CallingPolicy::Aapcs64, &signature).map_err(|error| {
                Diagnostic::error(format!("cannot evaluate AAPCS64 import plan: {error}"))
            })?
        }
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

fn aarch64_operand_shape_with_context(
    operand: omega_isa_aarch64::Aarch64CallOperand,
    expected: Option<ValueShape>,
) -> Result<ValueShape, Diagnostic> {
    use omega_isa_aarch64::Aarch64CallOperand;

    if let Some(expected) = expected
        && matches!(
            expected.class,
            omega_calling_conventions::ValueClass::Integer
        )
        && expected.byte_size <= 8
    {
        match operand {
            Aarch64CallOperand::ImmediateInteger(_) => {
                // Integer literals have no storage width of their own. Their
                // checked call-site type has already selected the external
                // parameter, so the retained plan supplies the concrete ABI
                // width at this final seam.
                return Ok(expected);
            }
            Aarch64CallOperand::RuntimeScalarInteger { byte_count, .. }
                if usize::from(expected.byte_size) <= byte_count =>
            {
                // Runtime storage is compiler scratch capacity, not a second
                // ABI declaration. Checked lowering has already established
                // the call parameter's exact type (including any proved-safe
                // narrowing), so a wider slot must not override the selected
                // foreign parameter shape.
                return Ok(expected);
            }
            _ => {}
        }
    }
    aarch64_operand_shape(operand)
}

fn aarch64_result_shape(
    operand: omega_isa_aarch64::Aarch64CallOperand,
    float: bool,
    expected: Option<ValueShape>,
) -> Result<ValueShape, Diagnostic> {
    let omega_isa_aarch64::Aarch64CallOperand::RuntimeScalarInteger { byte_count, .. } = operand
    else {
        return Err(Diagnostic::error(
            "AArch64 import result place did not lower to scalar storage",
        ));
    };
    let storage_byte_count = u16::try_from(byte_count)
        .map_err(|_| Diagnostic::error("AArch64 import result width exceeds u16"))?;
    let derived = if float {
        ValueShape::float(storage_byte_count)
    } else {
        ValueShape::integer(storage_byte_count, storage_byte_count.max(1))
    };
    let Some(expected) = expected else {
        return Ok(derived);
    };
    if expected.class != derived.class || expected.byte_size > storage_byte_count {
        return Err(Diagnostic::error(format!(
            "AArch64 import result shape {expected:?} does not fit its lowered {derived:?} storage"
        )));
    }
    Ok(expected)
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

fn scalar_result_byte_count(
    result: Option<&ValuePlacement>,
    result_kind: &str,
) -> Result<usize, Diagnostic> {
    let result = result.ok_or_else(|| {
        Diagnostic::error(format!(
            "AArch64 {result_kind}-returning import has no normalized result placement"
        ))
    })?;
    Ok(usize::from(result.shape.byte_size))
}

pub fn encode_syscall_sequence_no_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    encode_syscall_sequence_for_plan(
        architecture,
        operands,
        syscall_number,
        SyscallPlan::CompatibilityOracle,
    )
}

pub fn encode_syscall_sequence_with_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: &CallPlan,
) -> Result<Vec<u8>, Diagnostic> {
    encode_syscall_sequence_for_plan(
        architecture,
        operands,
        syscall_number,
        SyscallPlan::Authoritative(authoritative_plan),
    )
}

fn encode_syscall_sequence_for_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    plan_source: SyscallPlan<'_>,
) -> Result<Vec<u8>, Diagnostic> {
    let registers =
        normalized_syscall_registers_for_plan(architecture, operands.len(), false, plan_source)?;

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

fn encode_value_syscall_sequence_with_site<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    plan_source: SyscallPlan<'_>,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    let Some((_, arguments)) = operands.split_first() else {
        return Err(Diagnostic::error(
            "value-returning syscall has no result storage operand",
        ));
    };
    let registers =
        normalized_syscall_registers_for_plan(architecture, arguments.len(), true, plan_source)?;
    let result_register = registers.required_result()?;
    match architecture {
        Architecture::Aarch64 => aarch64::encode_value_syscall_sequence(
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
        Architecture::X86_64 => x86_64::encode_value_syscall_sequence(
            operands,
            syscall_number,
            &registers.parameters,
            result_register,
            registers.number,
            registers.immediate,
        ),
    }
}

pub fn encode_value_syscall_sequence_no_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    encode_value_syscall_sequence_with_site(
        architecture,
        operands,
        syscall_number,
        SyscallPlan::CompatibilityOracle,
    )
    .map(|(bytes, _)| bytes)
}

pub fn encode_value_syscall_sequence_with_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: &CallPlan,
) -> Result<Vec<u8>, Diagnostic> {
    encode_value_syscall_sequence_with_site(
        architecture,
        operands,
        syscall_number,
        SyscallPlan::Authoritative(authoritative_plan),
    )
    .map(|(bytes, _)| bytes)
}

pub fn value_syscall_relocation_byte_offset_no_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    operand_index: usize,
    syscall_number: u32,
) -> Result<usize, Diagnostic> {
    value_syscall_relocation_byte_offset_for_plan(
        architecture,
        operands,
        operand_index,
        syscall_number,
        SyscallPlan::CompatibilityOracle,
    )
}

pub fn value_syscall_relocation_byte_offset_with_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    operand_index: usize,
    syscall_number: u32,
    authoritative_plan: &CallPlan,
) -> Result<usize, Diagnostic> {
    value_syscall_relocation_byte_offset_for_plan(
        architecture,
        operands,
        operand_index,
        syscall_number,
        SyscallPlan::Authoritative(authoritative_plan),
    )
}

fn value_syscall_relocation_byte_offset_for_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    operand_index: usize,
    syscall_number: u32,
    plan_source: SyscallPlan<'_>,
) -> Result<usize, Diagnostic> {
    let (_, result_site) = encode_value_syscall_sequence_with_site(
        architecture,
        operands,
        syscall_number,
        plan_source,
    )?;
    if operand_index == 0 {
        return Ok(result_site);
    }
    let arguments = operands.get(1..).ok_or_else(|| {
        Diagnostic::error("value-returning syscall has no argument operand range")
    })?;
    let argument_index = operand_index - 1;
    if argument_index >= arguments.len() {
        return Err(Diagnostic::error(
            "value-returning syscall relocation operand is out of range",
        ));
    }
    Ok(match architecture {
        Architecture::Aarch64 => arguments
            .iter()
            .take(argument_index)
            .map(|operand| crate::operand_width(architecture, operand))
            .sum(),
        Architecture::X86_64 => {
            x86_64::syscall_data_relocation_byte_offset(arguments, argument_index)
        }
    })
}

fn encode_linux_timespec_syscall_with_site<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    plan_source: SyscallPlan<'_>,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    if operands.len() != 2 {
        return Err(Diagnostic::error(
            "Linux timespec lowering requires one semantic result and one injected clock id",
        ));
    }
    let registers = normalized_syscall_registers_for_plan(architecture, 2, true, plan_source)?;
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

pub fn encode_linux_timespec_syscall_no_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    encode_linux_timespec_syscall_with_site(
        architecture,
        operands,
        syscall_number,
        SyscallPlan::CompatibilityOracle,
    )
    .map(|(bytes, _)| bytes)
}

pub fn encode_linux_timespec_syscall_with_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: &CallPlan,
) -> Result<Vec<u8>, Diagnostic> {
    encode_linux_timespec_syscall_with_site(
        architecture,
        operands,
        syscall_number,
        SyscallPlan::Authoritative(authoritative_plan),
    )
    .map(|(bytes, _)| bytes)
}

pub fn linux_timespec_result_relocation_byte_offset_no_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> Result<usize, Diagnostic> {
    linux_timespec_result_relocation_byte_offset_for_plan(
        architecture,
        operands,
        syscall_number,
        SyscallPlan::CompatibilityOracle,
    )
}

pub fn linux_timespec_result_relocation_byte_offset_with_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: &CallPlan,
) -> Result<usize, Diagnostic> {
    linux_timespec_result_relocation_byte_offset_for_plan(
        architecture,
        operands,
        syscall_number,
        SyscallPlan::Authoritative(authoritative_plan),
    )
}

fn linux_timespec_result_relocation_byte_offset_for_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    plan_source: SyscallPlan<'_>,
) -> Result<usize, Diagnostic> {
    encode_linux_timespec_syscall_with_site(architecture, operands, syscall_number, plan_source)
        .map(|(_, byte_offset)| byte_offset)
}

fn encode_linux_timespec_argument_syscall_with_site<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    plan_source: SyscallPlan<'_>,
) -> Result<(Vec<u8>, Option<usize>), Diagnostic> {
    if operands.len() != 1 {
        return Err(Diagnostic::error(
            "Linux timespec-argument lowering requires one semantic millisecond argument",
        ));
    }
    let registers = normalized_syscall_registers_for_plan(architecture, 2, true, plan_source)?;
    let result_register = registers.required_result()?;
    match architecture {
        Architecture::Aarch64 => aarch64::encode_linux_timespec_argument_syscall(
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
            let (bytes, site) = x86_64::encode_linux_timespec_argument_syscall(
                operands,
                syscall_number,
                &registers.parameters,
                result_register,
                registers.number,
                registers.immediate,
            )?;
            Ok((bytes, site.map(|site| site.byte_offset)))
        }
    }
}

pub fn encode_linux_timespec_argument_syscall_no_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    encode_linux_timespec_argument_syscall_with_site(
        architecture,
        operands,
        syscall_number,
        SyscallPlan::CompatibilityOracle,
    )
    .map(|(bytes, _)| bytes)
}

pub fn encode_linux_timespec_argument_syscall_with_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: &CallPlan,
) -> Result<Vec<u8>, Diagnostic> {
    encode_linux_timespec_argument_syscall_with_site(
        architecture,
        operands,
        syscall_number,
        SyscallPlan::Authoritative(authoritative_plan),
    )
    .map(|(bytes, _)| bytes)
}

pub fn linux_timespec_argument_relocation_byte_offset_no_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> Result<Option<usize>, Diagnostic> {
    linux_timespec_argument_relocation_byte_offset_for_plan(
        architecture,
        operands,
        syscall_number,
        SyscallPlan::CompatibilityOracle,
    )
}

pub fn linux_timespec_argument_relocation_byte_offset_with_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: &CallPlan,
) -> Result<Option<usize>, Diagnostic> {
    linux_timespec_argument_relocation_byte_offset_for_plan(
        architecture,
        operands,
        syscall_number,
        SyscallPlan::Authoritative(authoritative_plan),
    )
}

fn linux_timespec_argument_relocation_byte_offset_for_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    plan_source: SyscallPlan<'_>,
) -> Result<Option<usize>, Diagnostic> {
    encode_linux_timespec_argument_syscall_with_site(
        architecture,
        operands,
        syscall_number,
        plan_source,
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

pub const fn foreign_float_control_prefix_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH,
        Architecture::X86_64 => x86_64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH,
    }
}

pub const fn foreign_float_control_trampoline_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
                + aarch64::FOREIGN_FLOAT_CONTROL_SUFFIX_WIDTH
        }
        Architecture::X86_64 => {
            x86_64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH + x86_64::FOREIGN_FLOAT_CONTROL_SUFFIX_WIDTH
        }
    }
}

/// Surround one returning foreign-call instruction program with an aligned
/// save/restore envelope. The inner encoder keeps its original stack-relative
/// layout because both target envelopes preserve the ABI alignment modulus.
pub fn wrap_foreign_float_control(architecture: Architecture, inner: Vec<u8>) -> Vec<u8> {
    let mut bytes =
        Vec::with_capacity(inner.len() + foreign_float_control_trampoline_width(architecture));
    match architecture {
        Architecture::Aarch64 => {
            bytes.extend(aarch64::encode_foreign_float_control_prefix_bytes());
            bytes.extend(inner);
            bytes.extend(aarch64::encode_foreign_float_control_suffix_bytes());
        }
        Architecture::X86_64 => {
            bytes.extend(x86_64::encode_foreign_float_control_prefix_bytes());
            bytes.extend(inner);
            bytes.extend(x86_64::encode_foreign_float_control_suffix_bytes());
        }
    }
    bytes
}

#[cfg(test)]
mod foreign_float_control_tests {
    use super::*;

    #[test]
    fn envelopes_preserve_inner_bytes_and_target_alignment() {
        let inner = vec![0xaa, 0xbb, 0xcc, 0xdd];

        let x86 = wrap_foreign_float_control(Architecture::X86_64, inner.clone());
        assert_eq!(foreign_float_control_prefix_width(Architecture::X86_64), 8);
        assert_eq!(
            foreign_float_control_trampoline_width(Architecture::X86_64),
            16
        );
        assert_eq!(&x86[..8], &[0x48, 0x83, 0xec, 0x10, 0x0f, 0xae, 0x1c, 0x24]);
        assert_eq!(&x86[8..12], inner.as_slice());
        assert_eq!(
            &x86[12..],
            &[0x0f, 0xae, 0x14, 0x24, 0x48, 0x83, 0xc4, 0x10]
        );

        let aarch64 = wrap_foreign_float_control(Architecture::Aarch64, inner.clone());
        assert_eq!(
            foreign_float_control_prefix_width(Architecture::Aarch64),
            12
        );
        assert_eq!(
            foreign_float_control_trampoline_width(Architecture::Aarch64),
            24
        );
        assert_eq!(&aarch64[12..16], inner.as_slice());
        assert_eq!(&aarch64[..4], &0xD100_43FFu32.to_le_bytes());
        assert_eq!(&aarch64[4..8], &0xD53B_4410u32.to_le_bytes());
        assert_eq!(&aarch64[8..12], &0xF900_03F0u32.to_le_bytes());
        assert_eq!(&aarch64[16..20], &0xF940_03F0u32.to_le_bytes());
        assert_eq!(&aarch64[20..24], &0xD51B_4410u32.to_le_bytes());
        assert_eq!(&aarch64[24..28], &0x9100_43FFu32.to_le_bytes());
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
    kind: psi_language_core::inline_assembly::AsmFenceKind,
) -> Option<Vec<u8>> {
    match architecture {
        Architecture::Aarch64 => None,
        Architecture::X86_64 => Some(x86_64::encode_memory_fence_bytes(kind).to_vec()),
    }
}

pub fn encode_interrupt_control_bytes(
    architecture: Architecture,
    kind: psi_language_core::inline_assembly::AsmInterruptControlKind,
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

    fn scalar(byte_offset: usize) -> TargetInstructionOperand {
        TargetInstructionOperand {
            kind: TargetInstructionOperandKind::RuntimeScalarInteger {
                region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count: 8,
            },
        }
    }

    fn explicit_plan(
        architecture: Architecture,
        parameter_count: usize,
        has_result: bool,
    ) -> CallPlan {
        let policy = match architecture {
            Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
            Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
        };
        evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); parameter_count],
                result: has_result.then(|| ValueShape::integer(8, 8)),
            },
        )
        .expect("explicit native syscall plan")
    }

    #[test]
    fn syscall_compatibility_families_equal_the_explicit_native_plan() {
        let statement_operands = [scalar(0), scalar(8)];
        let value_operands = [scalar(16), scalar(24)];
        let timespec_operands = [
            scalar(32),
            TargetInstructionOperand {
                kind: TargetInstructionOperandKind::ImmediateInteger(1),
            },
        ];
        let timespec_argument_operands = [scalar(48)];

        for (architecture, number) in [(Architecture::X86_64, 228), (Architecture::Aarch64, 113)] {
            let statement_plan = explicit_plan(architecture, 2, false);
            let compatibility =
                encode_syscall_sequence_no_plan(architecture, &statement_operands, number)
                    .expect("compatibility statement syscall");
            let planned = encode_syscall_sequence_with_plan(
                architecture,
                &statement_operands,
                number,
                &statement_plan,
            )
            .expect("explicit-plan statement syscall");
            assert_eq!(compatibility, planned, "statement {architecture:?}");
            assert_eq!(
                crate::syscall_sequence_width_no_plan(architecture, &statement_operands, number,),
                crate::syscall_sequence_width_with_plan(
                    architecture,
                    &statement_operands,
                    number,
                    &statement_plan,
                ),
                "statement {architecture:?}"
            );

            let value_plan = explicit_plan(architecture, 1, true);
            let compatibility =
                encode_value_syscall_sequence_no_plan(architecture, &value_operands, number)
                    .expect("compatibility value syscall");
            let planned = encode_value_syscall_sequence_with_plan(
                architecture,
                &value_operands,
                number,
                &value_plan,
            )
            .expect("explicit-plan value syscall");
            assert_eq!(compatibility, planned, "value {architecture:?}");
            assert_eq!(
                crate::value_syscall_sequence_width_no_plan(architecture, &value_operands, number,),
                crate::value_syscall_sequence_width_with_plan(
                    architecture,
                    &value_operands,
                    number,
                    &value_plan,
                ),
                "value {architecture:?}"
            );
            for operand_index in 0..value_operands.len() {
                assert_eq!(
                    value_syscall_relocation_byte_offset_no_plan(
                        architecture,
                        &value_operands,
                        operand_index,
                        number,
                    )
                    .expect("compatibility value relocation"),
                    value_syscall_relocation_byte_offset_with_plan(
                        architecture,
                        &value_operands,
                        operand_index,
                        number,
                        &value_plan,
                    )
                    .expect("explicit-plan value relocation"),
                    "value relocation {architecture:?} operand {operand_index}"
                );
            }

            let timespec_plan = explicit_plan(architecture, 2, true);
            let compatibility =
                encode_linux_timespec_syscall_no_plan(architecture, &timespec_operands, number)
                    .expect("compatibility timespec result syscall");
            let planned = encode_linux_timespec_syscall_with_plan(
                architecture,
                &timespec_operands,
                number,
                &timespec_plan,
            )
            .expect("explicit-plan timespec result syscall");
            assert_eq!(compatibility, planned, "timespec result {architecture:?}");
            assert_eq!(
                crate::linux_timespec_syscall_sequence_width_no_plan(
                    architecture,
                    &timespec_operands,
                    number,
                ),
                crate::linux_timespec_syscall_sequence_width_with_plan(
                    architecture,
                    &timespec_operands,
                    number,
                    &timespec_plan,
                ),
                "timespec result {architecture:?}"
            );
            assert_eq!(
                linux_timespec_result_relocation_byte_offset_no_plan(
                    architecture,
                    &timespec_operands,
                    number,
                )
                .expect("compatibility timespec result relocation"),
                linux_timespec_result_relocation_byte_offset_with_plan(
                    architecture,
                    &timespec_operands,
                    number,
                    &timespec_plan,
                )
                .expect("explicit-plan timespec result relocation"),
                "timespec result relocation {architecture:?}"
            );

            let compatibility = encode_linux_timespec_argument_syscall_no_plan(
                architecture,
                &timespec_argument_operands,
                number,
            )
            .expect("compatibility timespec argument syscall");
            let planned = encode_linux_timespec_argument_syscall_with_plan(
                architecture,
                &timespec_argument_operands,
                number,
                &timespec_plan,
            )
            .expect("explicit-plan timespec argument syscall");
            assert_eq!(compatibility, planned, "timespec argument {architecture:?}");
            assert_eq!(
                crate::linux_timespec_argument_syscall_sequence_width_no_plan(
                    architecture,
                    &timespec_argument_operands,
                    number,
                ),
                crate::linux_timespec_argument_syscall_sequence_width_with_plan(
                    architecture,
                    &timespec_argument_operands,
                    number,
                    &timespec_plan,
                ),
                "timespec argument {architecture:?}"
            );
            assert_eq!(
                linux_timespec_argument_relocation_byte_offset_no_plan(
                    architecture,
                    &timespec_argument_operands,
                    number,
                )
                .expect("compatibility timespec argument relocation"),
                linux_timespec_argument_relocation_byte_offset_with_plan(
                    architecture,
                    &timespec_argument_operands,
                    number,
                    &timespec_plan,
                )
                .expect("explicit-plan timespec argument relocation"),
                "timespec argument relocation {architecture:?}"
            );
        }
    }

    #[test]
    fn statement_syscall_accepts_a_retained_discarded_native_result() {
        let operands = [
            TargetInstructionOperand {
                kind: TargetInstructionOperandKind::ImmediateInteger(1),
            },
            TargetInstructionOperand {
                kind: TargetInstructionOperandKind::ImmediateInteger(2),
            },
            TargetInstructionOperand {
                kind: TargetInstructionOperandKind::ImmediateInteger(3),
            },
        ];

        for (architecture, number) in [(Architecture::X86_64, 1), (Architecture::Aarch64, 64)] {
            let plan = explicit_plan(architecture, operands.len(), true);
            let bytes = encode_syscall_sequence_with_plan(architecture, &operands, number, &plan)
                .expect("statement syscall may discard its retained native result");
            assert!(!bytes.is_empty(), "{architecture:?}");
        }
    }

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
            encode_syscall_sequence_with_plan(Architecture::X86_64, &operands, 60, &x86_plan)
                .expect("source-selected x86-64 syscall register");
        assert_eq!(&x86_bytes[..2], &[0x49, 0xba]);
        assert_eq!(
            x86_bytes.len(),
            crate::syscall_sequence_width_with_plan(Architecture::X86_64, &operands, 60, &x86_plan,)
        );

        let mut aarch64_plan = evaluate_call_plan(CallingPolicy::LinuxSyscallAarch64, &signature)
            .expect("baseline AArch64 syscall plan");
        aarch64_plan.parameters[0].locations[0] = ValueLocation::Register {
            register: MachineRegister::Aarch64X(3),
            value_byte_offset: 0,
            byte_size: 8,
        };
        let aarch64_bytes =
            encode_syscall_sequence_with_plan(Architecture::Aarch64, &operands, 93, &aarch64_plan)
                .expect("source-selected AArch64 syscall register");
        assert_eq!(&aarch64_bytes[..4], &[0xe3, 0x00, 0x80, 0xd2]);
        assert_eq!(
            aarch64_bytes.len(),
            crate::syscall_sequence_width_with_plan(
                Architecture::Aarch64,
                &operands,
                93,
                &aarch64_plan,
            )
        );
    }

    #[test]
    fn value_syscall_relocations_distinguish_result_from_arguments() {
        let operands = [
            TargetInstructionOperand {
                kind: TargetInstructionOperandKind::RuntimeScalarInteger {
                    region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 8,
                    byte_count: 4,
                },
            },
            TargetInstructionOperand {
                kind: TargetInstructionOperandKind::RuntimeStorageAddress {
                    region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 32,
                },
            },
        ];

        for (architecture, number) in [(Architecture::X86_64, 3), (Architecture::Aarch64, 57)] {
            let bytes = encode_value_syscall_sequence_no_plan(architecture, &operands, number)
                .expect("value-returning syscall");
            let result_site =
                value_syscall_relocation_byte_offset_no_plan(architecture, &operands, 0, number)
                    .expect("result relocation");
            let argument_site =
                value_syscall_relocation_byte_offset_no_plan(architecture, &operands, 1, number)
                    .expect("argument relocation");
            assert!(result_site > argument_site);
            match architecture {
                Architecture::X86_64 => {
                    assert_eq!(&bytes[result_site..result_site + 8], &[0; 8]);
                    assert_eq!(&bytes[argument_site..argument_site + 8], &[0; 8]);
                }
                Architecture::Aarch64 => {
                    assert!(result_site + 8 <= bytes.len());
                    assert!(argument_site + 8 <= bytes.len());
                    assert_ne!(
                        &bytes[result_site..result_site + 8],
                        &bytes[argument_site..argument_site + 8]
                    );
                }
            }
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
mod compatibility_encoder_differential_tests {
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

    fn float(byte_offset: usize) -> TargetInstructionOperand {
        TargetInstructionOperand {
            kind: TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count: 8,
            },
        }
    }

    fn plan(target: NativeTarget, parameter_count: usize, result_present: bool) -> CallPlan {
        evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); parameter_count],
                result: result_present.then(|| ValueShape::integer(8, 8)),
            },
        )
        .expect("native compatibility plan")
    }

    #[test]
    fn built_in_import_compatibility_bytes_widths_and_x86_sites_equal_the_explicit_plan() {
        // A scalar built-in import exercises both argument and result storage
        // without crossing an adapter-internal composite native call.
        let operands = [scalar(0), scalar(8)];
        let operation = HostOperationKey::from_names("Filesystem", "close");
        for target in [NativeTarget::windows_x64(), NativeTarget::macos_arm64()] {
            let plan = plan(target, 1, true);
            let compatibility = encode_host_call_sequence_no_plan(target, operation, &operands)
                .expect("compatibility built-in import encoding");
            let planned = encode_host_call_sequence_with_plan(target, operation, &operands, &plan)
                .expect("explicit-plan built-in import encoding");
            assert_eq!(compatibility, planned, "target {target:?}");
            assert_eq!(
                crate::host_call_sequence_width_no_plan(target, operation, &operands),
                crate::host_call_sequence_width_with_plan(target, operation, &operands, &plan,),
                "target {target:?}"
            );
        }

        let target = NativeTarget::windows_x64();
        let plan = plan(target, 1, true);
        let policy = CallingPolicy::native_for_target(target);
        assert_eq!(
            omega_isa_x86_64::host_call_external_relocation_site_no_plan(
                policy, operation, &operands,
            ),
            omega_isa_x86_64::host_call_external_relocation_site_with_plan(
                policy, operation, &operands, &plan,
            )
        );
        for operand_index in 0..operands.len() {
            assert_eq!(
                omega_isa_x86_64::host_call_data_relocation_site_no_plan(
                    policy,
                    operation,
                    &operands,
                    operand_index,
                ),
                omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                    policy,
                    operation,
                    &operands,
                    operand_index,
                    &plan,
                ),
                "operand {operand_index}"
            );
        }
    }

    #[test]
    fn specialized_built_in_imports_equal_their_explicit_native_plans() {
        let void_operands = [scalar(8)];
        for (target, operation) in [
            (
                NativeTarget::windows_x64(),
                HostOperationKey::from_names("Process", "exit_process"),
            ),
            (
                NativeTarget::macos_arm64(),
                HostOperationKey::from_names("Process", "exit"),
            ),
        ] {
            let plan = plan(target, 1, false);
            assert_eq!(
                encode_host_call_sequence_no_plan(target, operation, &void_operands)
                    .expect("compatibility void import"),
                encode_host_call_sequence_with_plan(target, operation, &void_operands, &plan,)
                    .expect("planned void import"),
                "void target {target:?}"
            );
            assert_eq!(
                crate::host_call_sequence_width_no_plan(target, operation, &void_operands),
                crate::host_call_sequence_width_with_plan(target, operation, &void_operands, &plan,),
                "void width target {target:?}"
            );
        }

        let result_only = [scalar(0)];
        let dereference = HostOperationKey::from_names("Filesystem", "read_errno");
        for target in [NativeTarget::windows_x64(), NativeTarget::macos_arm64()] {
            let plan = plan(target, 0, true);
            assert_eq!(
                encode_host_call_sequence_no_plan(target, dereference, &result_only)
                    .expect("compatibility pointer-dereference import"),
                encode_host_call_sequence_with_plan(target, dereference, &result_only, &plan,)
                    .expect("planned pointer-dereference import"),
                "dereference target {target:?}"
            );
            assert_eq!(
                crate::host_call_sequence_width_no_plan(target, dereference, &result_only),
                crate::host_call_sequence_width_with_plan(target, dereference, &result_only, &plan,),
                "dereference width target {target:?}"
            );
        }

        let key_operands = [scalar(0), scalar(8)];
        let key_target = NativeTarget::windows_x64();
        let key_operation = HostOperationKey::from_names("Input", "key_state");
        let key_plan = plan(key_target, 1, true);
        assert_eq!(
            encode_host_call_sequence_no_plan(key_target, key_operation, &key_operands)
                .expect("compatibility key-state import"),
            encode_host_call_sequence_with_plan(
                key_target,
                key_operation,
                &key_operands,
                &key_plan,
            )
            .expect("planned key-state import")
        );
        assert_eq!(
            crate::host_call_sequence_width_no_plan(key_target, key_operation, &key_operands),
            crate::host_call_sequence_width_with_plan(
                key_target,
                key_operation,
                &key_operands,
                &key_plan,
            )
        );

        let float_operands = [scalar(0), float(8)];
        let float_target = NativeTarget::macos_arm64();
        let float_operation = HostOperationKey::from_names("Math", "square_root");
        let float_plan = evaluate_call_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: vec![ValueShape::float(8)],
                result: Some(ValueShape::float(8)),
            },
        )
        .expect("AAPCS64 float import plan");
        assert_eq!(
            encode_host_call_sequence_no_plan(float_target, float_operation, &float_operands)
                .expect("compatibility float import"),
            encode_host_call_sequence_with_plan(
                float_target,
                float_operation,
                &float_operands,
                &float_plan,
            )
            .expect("planned float import")
        );
        assert_eq!(
            crate::host_call_sequence_width_no_plan(float_target, float_operation, &float_operands),
            crate::host_call_sequence_width_with_plan(
                float_target,
                float_operation,
                &float_operands,
                &float_plan,
            )
        );
    }

    #[test]
    fn retained_plan_owns_aarch64_result_presence_and_class() {
        let target = NativeTarget::macos_arm64();
        let operands = [scalar(0), float(8)];
        let float_plan = evaluate_call_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: vec![ValueShape::float(8)],
                result: Some(ValueShape::float(8)),
            },
        )
        .expect("AAPCS64 float import plan");
        let catalog_float = encode_host_call_sequence_with_plan(
            target,
            HostOperationKey::from_names("Math", "square_root"),
            &operands,
            &float_plan,
        )
        .expect("catalog float import");
        let plan_only_float = encode_host_call_sequence_with_plan(
            target,
            HostOperationKey::default(),
            &operands,
            &float_plan,
        )
        .expect("plan-classified float import");
        assert_eq!(catalog_float, plan_only_float);

        let void_plan = plan(target, 1, false);
        let void_operands = [scalar(8)];
        let bytes = encode_host_call_sequence_with_plan(
            target,
            HostOperationKey::from_names("Math", "square_root"),
            &void_operands,
            &void_plan,
        )
        .expect("plan-classified void import");
        assert_eq!(
            bytes,
            encode_host_call_sequence_with_plan(
                target,
                HostOperationKey::from_names("Process", "exit"),
                &void_operands,
                &void_plan,
            )
            .expect("catalog void import")
        );
    }

    #[test]
    fn retained_console_result_is_validated_but_discarded() {
        let target = NativeTarget::macos_arm64();
        let operation = HostOperationKey::from_names("Stdout", "write");
        let operands = [scalar(0), scalar(8), scalar(16)];
        let native_plan = plan(target, 3, true);

        assert_eq!(
            encode_host_call_sequence_no_plan(target, operation, &operands)
                .expect("compatibility console write"),
            encode_host_call_sequence_with_plan(target, operation, &operands, &native_plan)
                .expect("planned console write with discarded native result")
        );
        assert_eq!(
            normalized_aarch64_host_argument_placements_with_plan(
                operation,
                &operands,
                false,
                &native_plan,
            )
            .expect("planned console placements")
            .len(),
            3
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

    fn sysv_plan(parameter_count: usize, result_present: bool) -> CallPlan {
        evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); parameter_count],
                result: result_present.then(|| ValueShape::integer(8, 8)),
            },
        )
        .expect("explicit SysV field-call plan")
    }

    #[test]
    fn linux_x64_routes_vtable_fields_through_the_sysv_encoder() {
        let operands = [scalar(0), scalar(8), scalar(16)];
        let target = omega_target::NativeTarget::linux_x64();
        let plan = sysv_plan(2, true);
        let bytes =
            encode_vtable_call_sequence_at_offset_with_plan(target, &operands, 24, true, &plan)
                .expect("SysV vtable-field call");

        assert_eq!(
            bytes.len(),
            crate::vtable_call_sequence_width_at_offset_with_plan(
                target, &operands, 24, true, &plan,
            )
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
        let bytes =
            encode_vtable_call_sequence_at_offset_with_plan(target, &operands, 24, true, &plan)
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
                target, &operands, 24, true, &plan,
            )
        );
    }

    #[test]
    fn linux_x64_routes_table_functions_without_passing_the_table() {
        let operands = [scalar(0), scalar(8), scalar(16)];
        let target = omega_target::NativeTarget::linux_x64();
        let plan = sysv_plan(1, true);
        let bytes =
            encode_table_function_call_sequence_with_plan(target, &operands, 40, true, &plan)
                .expect("SysV table-function call");

        assert_eq!(
            bytes.len(),
            crate::table_function_call_sequence_width_with_plan(target, &operands, 40, true, &plan,)
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
            encode_table_function_call_sequence_with_plan(target, &operands, 40, true, &plan)
                .expect("SysV table-function call in a PE image");

        assert!(
            bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xbb, 16, 0, 0, 0]),
            "the dispatch table must be excluded so the declared argument uses rdi"
        );
        assert_eq!(
            bytes.len(),
            crate::table_function_call_sequence_width_with_plan(target, &operands, 40, true, &plan,)
        );
    }
}

#[cfg(test)]
mod aarch64_import_plan_tests {
    use super::*;
    use omega_calling_conventions::{HostCapability, HostOperation, RegisterSet};
    use omega_target_operations::{
        RuntimeStorageRegion, TargetInstructionOperand, TargetInstructionOperandKind,
    };

    fn operand(kind: TargetInstructionOperandKind) -> TargetInstructionOperand {
        TargetInstructionOperand { kind }
    }

    fn aapcs64_plan(parameters: Vec<ValueShape>, result: Option<ValueShape>) -> CallPlan {
        evaluate_call_plan(
            CallingPolicy::Aapcs64,
            &CallSignature { parameters, result },
        )
        .expect("explicit AAPCS64 field-call plan")
    }

    #[test]
    fn darwin_open_create_consumes_one_complete_variadic_plan() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 4,
            }),
            operand(TargetInstructionOperandKind::DataAddress {
                data: psi_arena::Handle::invalid(),
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(0x201)),
            operand(TargetInstructionOperandKind::ImmediateInteger(0o644)),
        ];
        let (parameters, result) =
            normalized_darwin_open_create_plan(&operands, HostImportPlan::CompatibilityOracle)
                .expect("Darwin variadic open plan");

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
                    register: MachineRegister::Aarch64X(1),
                    value_byte_offset: 0,
                    byte_size: 4,
                },
                ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 4,
                    alignment: 8,
                },
            ]
        );
        assert_eq!(
            result.expect("open result").locations,
            [ValueLocation::Register {
                register: MachineRegister::Aarch64X(0),
                value_byte_offset: 0,
                byte_size: 4,
            }]
        );

        let operation =
            HostOperationKey::new(HostCapability::Filesystem, HostOperation::OpenCreate);
        let bytes =
            encode_host_call_sequence_no_plan(NativeTarget::macos_arm64(), operation, &operands)
                .expect("plan-driven Darwin open_create encoding");
        assert_eq!(
            bytes.len(),
            crate::host_call_sequence_width_no_plan(
                NativeTarget::macos_arm64(),
                operation,
                &operands,
            )
        );
        assert_eq!(&bytes[..4], &0xd100_43ff_u32.to_le_bytes());
        assert_eq!(&bytes[24..28], &0x9400_0000_u32.to_le_bytes());
        assert_eq!(&bytes[28..32], &0x9100_43ff_u32.to_le_bytes());

        let signature = ConcreteVariadicCallSignature {
            fixed_parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(4, 4)],
            variadic_parameters: vec![ValueShape::integer(4, 4)],
            result: Some(ValueShape::integer(4, 4)),
        };
        let retained = evaluate_darwin_aapcs64_variadic_call_plan(&signature)
            .expect("retained Darwin variadic plan");
        normalized_darwin_open_create_plan(&operands, HostImportPlan::Authoritative(&retained))
            .expect("retained Darwin variadic plan must validate");

        let mut flattened = retained;
        flattened.parameters[2].locations = vec![ValueLocation::Register {
            register: MachineRegister::Aarch64X(2),
            value_byte_offset: 0,
            byte_size: 4,
        }];
        let error = normalized_darwin_open_create_plan(
            &operands,
            HostImportPlan::Authoritative(&flattened),
        )
        .expect_err("flattening the anonymous mode into x2 must reject");
        assert!(error.message.contains("variadic parameter boundary"));
    }

    #[test]
    fn exact_i32_import_plan_accepts_wider_typed_scratch_and_contextual_literal() {
        let plan = aapcs64_plan(vec![ValueShape::integer(4, 4)], None);
        for argument in [
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 24,
                byte_count: 4,
            },
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 8,
            },
            TargetInstructionOperandKind::ImmediateInteger(70),
        ] {
            let operands = [operand(argument)];
            let (parameters, result) = normalized_aarch64_import_plan_for_plan(
                &operands,
                Aarch64ImportResult::None,
                HostImportPlan::Authoritative(&plan),
            )
            .expect("exact I32 import argument");
            assert_eq!(parameters[0].shape, ValueShape::integer(4, 4));
            assert!(result.is_none());
        }
    }

    #[test]
    fn exact_i32_returning_import_encodes_typed_result_and_argument() {
        let plan = aapcs64_plan(
            vec![ValueShape::integer(4, 4)],
            Some(ValueShape::integer(4, 4)),
        );
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 16,
                byte_count: 4,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 24,
                byte_count: 4,
            }),
        ];
        encode_host_call_sequence_with_plan(
            omega_target::NativeTarget::macos_arm64(),
            HostOperationKey::new(HostCapability::Filesystem, HostOperation::Close),
            &operands,
            &plan,
        )
        .expect("exact I32 returning import");
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
            normalized_aarch64_import_plan(&operands, Aarch64ImportResult::None)
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

        let plan = aapcs64_plan(vec![ValueShape::integer(8, 8); 2], None);
        let (placements, result) =
            normalized_aarch64_vtable_plan_with_plan(&operands, false, &plan)
                .expect("AAPCS64 vtable placements");
        assert!(result.is_none());
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
        let bytes =
            encode_vtable_call_sequence_at_offset_with_plan(target, &operands, 24, false, &plan)
                .expect("source-selected AAPCS64 vtable plan");

        assert!(
            bytes
                .windows(4)
                .any(|window| window == [0xe3, 0x00, 0x80, 0xd2])
        );
        assert_eq!(
            bytes.len(),
            crate::vtable_call_sequence_width_at_offset_with_plan(
                target, &operands, 24, false, &plan,
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

        let plan = aapcs64_plan(
            vec![ValueShape::integer(8, 8); 2],
            Some(ValueShape::integer(4, 4)),
        );
        let (placements, result) = normalized_aarch64_vtable_plan_with_plan(&operands, true, &plan)
            .expect("AAPCS64 vtable field plan");
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

        let bytes = encode_vtable_call_sequence_at_offset_with_plan(
            omega_target::NativeTarget::linux_arm64(),
            &operands,
            24,
            true,
            &plan,
        )
        .expect("encode AAPCS64 vtable field result");
        assert_eq!(
            bytes.len(),
            crate::vtable_call_sequence_width_at_offset_with_plan(
                omega_target::NativeTarget::linux_arm64(),
                &operands,
                24,
                true,
                &plan,
            )
        );
        assert_eq!(
            crate::vtable_call_sequence_width_at_offset_with_plan(
                omega_target::NativeTarget::linux_arm64(),
                &operands,
                32_768,
                true,
                &plan,
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
            normalized_aarch64_import_plan(&operands, Aarch64ImportResult::None)
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

        let plan = aapcs64_plan(
            vec![ValueShape::integer(8, 8)],
            Some(ValueShape::integer(4, 4)),
        );
        let (placements, result) =
            normalized_aarch64_table_function_plan_with_plan(&operands, true, &plan)
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

        let bytes = encode_table_function_call_sequence_with_plan(
            omega_target::NativeTarget::linux_arm64(),
            &operands,
            24,
            true,
            &plan,
        )
        .expect("encode AAPCS64 table-function result");
        assert_eq!(
            bytes.len(),
            crate::table_function_call_sequence_width_with_plan(
                omega_target::NativeTarget::linux_arm64(),
                &operands,
                24,
                true,
                &plan,
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
        let vtable_plan = aapcs64_plan(vec![ValueShape::integer(8, 8)], Some(ValueShape::float(8)));
        let vtable_bytes = encode_vtable_call_sequence_at_offset_with_plan(
            omega_target::NativeTarget::linux_arm64(),
            &vtable_operands,
            24,
            true,
            &vtable_plan,
        )
        .expect("encode float-returning AAPCS64 vtable field");
        assert_eq!(vtable_bytes.len(), 36);
        assert_eq!(
            vtable_bytes.len(),
            crate::vtable_call_sequence_width_at_offset_with_plan(
                omega_target::NativeTarget::linux_arm64(),
                &vtable_operands,
                24,
                true,
                &vtable_plan,
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
        let table_plan = aapcs64_plan(
            vec![ValueShape::integer(8, 8)],
            Some(ValueShape::homogeneous_float_aggregate(8, 2)),
        );
        let table_bytes = encode_table_function_call_sequence_with_plan(
            omega_target::NativeTarget::linux_arm64(),
            &table_operands,
            24,
            true,
            &table_plan,
        )
        .expect("encode HFA-returning AAPCS64 table function");
        assert_eq!(table_bytes.len(), 48);
        assert_eq!(
            table_bytes.len(),
            crate::table_function_call_sequence_width_with_plan(
                omega_target::NativeTarget::linux_arm64(),
                &table_operands,
                24,
                true,
                &table_plan,
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
        let vtable_plan = aapcs64_plan(
            vec![ValueShape::integer(8, 8)],
            Some(ValueShape::integer(16, 8)),
        );
        let vtable_bytes = encode_vtable_call_sequence_at_offset_with_plan(
            omega_target::NativeTarget::linux_arm64(),
            &vtable_operands,
            24,
            true,
            &vtable_plan,
        )
        .expect("encode small-aggregate-returning AAPCS64 vtable field");
        assert_eq!(
            vtable_bytes.len(),
            crate::vtable_call_sequence_width_at_offset_with_plan(
                omega_target::NativeTarget::linux_arm64(),
                &vtable_operands,
                24,
                true,
                &vtable_plan,
            )
        );

        let table_operands = [aggregate_result(), receiver()];
        let table_plan = aapcs64_plan(Vec::new(), Some(ValueShape::integer(16, 8)));
        let table_bytes = encode_table_function_call_sequence_with_plan(
            omega_target::NativeTarget::linux_arm64(),
            &table_operands,
            24,
            true,
            &table_plan,
        )
        .expect("encode small-aggregate-returning AAPCS64 table function");
        assert_eq!(
            table_bytes.len(),
            crate::table_function_call_sequence_width_with_plan(
                omega_target::NativeTarget::linux_arm64(),
                &table_operands,
                24,
                true,
                &table_plan,
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
        let vtable_plan = aapcs64_plan(
            vec![ValueShape::integer(8, 8)],
            Some(ValueShape::integer(24, 8)),
        );
        let vtable_bytes = encode_vtable_call_sequence_at_offset_with_plan(
            omega_target::NativeTarget::linux_arm64(),
            &vtable_operands,
            24,
            true,
            &vtable_plan,
        )
        .expect("encode indirect-returning AAPCS64 vtable field");
        assert_eq!(
            vtable_bytes.len(),
            crate::vtable_call_sequence_width_at_offset_with_plan(
                omega_target::NativeTarget::linux_arm64(),
                &vtable_operands,
                24,
                true,
                &vtable_plan,
            )
        );

        let table_operands = [aggregate_result(), pointer()];
        let table_plan = aapcs64_plan(Vec::new(), Some(ValueShape::integer(24, 8)));
        let table_bytes = encode_table_function_call_sequence_with_plan(
            omega_target::NativeTarget::linux_arm64(),
            &table_operands,
            24,
            true,
            &table_plan,
        )
        .expect("encode indirect-returning AAPCS64 table function");
        assert_eq!(
            table_bytes.len(),
            crate::table_function_call_sequence_width_with_plan(
                omega_target::NativeTarget::linux_arm64(),
                &table_operands,
                24,
                true,
                &table_plan,
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

        let (locations, _) = normalized_aarch64_import_plan(&operands, Aarch64ImportResult::None)
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
            normalized_aarch64_import_plan(&operands, Aarch64ImportResult::None)
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
            normalized_aarch64_import_plan(&operands, Aarch64ImportResult::Authored)
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
            normalized_aarch64_import_plan(&operands, Aarch64ImportResult::Authored)
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
            &operands,
            &aapcs64_plan(
                vec![ValueShape::integer(24, 8)],
                Some(ValueShape::integer(24, 8)),
            ),
        )
        .expect("authored indirect aggregate call");
        assert_eq!(
            bytes.len(),
            crate::authored_import_call_sequence_width(
                omega_target::NativeTarget::linux_arm64(),
                &operands,
                &aapcs64_plan(
                    vec![ValueShape::integer(24, 8)],
                    Some(ValueShape::integer(24, 8)),
                ),
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
        let plan = aapcs64_plan(vec![ValueShape::integer(8, 8)], Some(ValueShape::float(8)));
        let bytes = encode_authored_import_call_sequence(
            omega_target::NativeTarget::linux_arm64(),
            &operands,
            &plan,
        )
        .expect("authored scalar-float import");

        assert_eq!(bytes.len(), 24);
        assert_eq!(
            bytes.len(),
            crate::authored_import_call_sequence_width(
                omega_target::NativeTarget::linux_arm64(),
                &operands,
                &plan,
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
            &operands,
            &plan,
        )
        .expect("authored import with source-selected x3 argument");

        assert_eq!(&bytes[..4], &[0xe3, 0x00, 0x80, 0xd2]);
        assert_eq!(
            bytes.len(),
            crate::authored_import_call_sequence_width(
                omega_target::NativeTarget::linux_arm64(),
                &operands,
                &plan,
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

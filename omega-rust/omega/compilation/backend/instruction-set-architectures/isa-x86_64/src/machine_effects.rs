use register_model::{TargetRegisterEnvironmentConstraintKeys, ValidatedRegisterConstraintCatalog};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey, MachineBarrier,
    MachineCallEffect, MachineCleanupEffect, MachineEffectCatalog, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
    MachineMemoryEffect, MachineSemanticKind as Semantic, MachineSizeKnowledge as Size,
    MachineTrapBehavior, SaturatingCarrier, SaturatingOperation, SelectedConstraintKeys,
    isa_machine_effect_catalog_plumbing,
};
use target::{Architecture, NativeTarget, ObjectFormat};

mod memory;
mod scalar_call;
pub use scalar_call::x86_64_normalized_foreign_call_declaration;
pub(crate) use scalar_call::{normalized_foreign_constraint, user_direct_call_effects};

use scalar_call::declaration as scalar_call_declaration;

use crate::register_model::{units, units_of, view_of};
use crate::selected_form_encoding::saturating_forms::SaturatingForm;
use crate::selected_form_encoding::trapping_forms::TrappingShape;
use crate::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64, X86_64_COMPARE_I64_IMMEDIATE,
    X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH, X86_64_COPY_I64, X86_64_MATERIALIZE_I64,
    X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT, X86_64_SUBTRACT_I64,
    X86_64_SUBTRACT_I64_IMMEDIATE, X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT,
    x86_64_system_v_register_call_keys,
};

isa_machine_effect_catalog_plumbing!(
    X86_64MachineEffectCatalogValidationError,
    validate_x86_64_machine_effect_catalog,
    Architecture::X86_64,
    "x86-64",
    x86_64_machine_effect_catalog
);

pub fn x86_64_machine_effect_catalog(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<MachineEffectCatalog, X86_64MachineEffectCatalogValidationError> {
    if target.architecture != Architecture::X86_64
        || constraints.architecture() != Architecture::X86_64
    {
        return Err(X86_64MachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    let selected_keys = x86_64_selected_constraint_keys(target)
        .ok_or(X86_64MachineEffectCatalogValidationError::UnsupportedTargetAbi)?;
    Ok(MachineEffectCatalog {
        target,
        register_constraints: constraints.identity(),
        selected_keys: selected_keys.clone(),
        declarations: selected_keys
            .declaration_keys()
            .into_iter()
            .map(|(semantic, constraint)| {
                if semantic == Semantic::MemoryFence {
                    return Ok(crate::selected_form_encoding::memory_fence::declaration(
                        constraint,
                    ));
                }
                if let Some(operation) =
                    crate::selected_form_encoding::register_access::operation(semantic)
                {
                    return Ok(crate::selected_form_encoding::register_access::declaration(
                        operation, constraint,
                    ));
                }
                if semantic == Semantic::PortWrite {
                    return Ok(crate::selected_form_encoding::port_write::declaration(
                        constraint,
                    ));
                }
                if semantic == Semantic::AtomicFence {
                    return Ok(crate::selected_form_encoding::atomic_fence::declaration(
                        constraint,
                    ));
                }
                if semantic == Semantic::Crash {
                    return Ok(crate::selected_form_encoding::crash::declaration(
                        constraints.catalog().constraint(constraint).ok_or(
                            X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch,
                        )?,
                    ));
                }
                if let Semantic::FloatResultPolicy(form) = semantic {
                    return Ok(
                        crate::selected_form_encoding::float_result_policy::declaration(
                            form, constraint,
                        ),
                    );
                }
                if let Semantic::NearestIeeeFloatArithmetic(form) = semantic {
                    return crate::selected_form_encoding::ieee_float_arithmetic::declaration(
                        form, constraint,
                    )
                    .map_err(|_| {
                        X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch
                    });
                }
                if semantic == Semantic::HostedExitProcessI32 {
                    return crate::selected_form_encoding::hosted_exit_process::declaration(
                        target, constraint,
                    )
                    .map_err(|_| {
                        X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch
                    });
                }
                if semantic == Semantic::HostedReadByte {
                    return Ok(
                        crate::selected_form_encoding::hosted_read_byte::declaration(
                            target, constraint,
                        ),
                    );
                }
                if matches!(
                    semantic,
                    Semantic::SaveFloatingControl | Semantic::RestoreFloatingControl
                ) {
                    return Ok(
                        crate::selected_form_encoding::floating_control::declaration(
                            semantic, constraint,
                        ),
                    );
                }
                if semantic == Semantic::HostedWriteByteI32 {
                    return Ok(
                        crate::selected_form_encoding::hosted_write_byte::declaration(
                            target, constraint,
                        ),
                    );
                }
                Ok(if matches!(semantic, memory::declared_semantics!()) {
                    memory::declaration(semantic, constraint, constraints)
                } else if matches!(
                    semantic,
                    Semantic::CallScalar
                        | Semantic::CallAggregate
                        | Semantic::InvokeNoNormal
                        | Semantic::NormalizedForeignCall
                ) {
                    scalar_call_declaration(semantic, constraint, constraints)
                } else if matches!(semantic, Semantic::ReturnAggregate | Semantic::ReturnScalar) {
                    let mut returned = declaration(Semantic::ReturnScalar, &selected_keys);
                    returned.semantic = semantic;
                    returned.constraint = constraint;
                    returned.alternatives[0].key.family = semantic.into();
                    returned
                } else {
                    declaration(semantic, &selected_keys)
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

/// The x86-64 encoding matrix declares one call/return ABI family per
/// supported (architecture, object-format) pair: Linux System-V under ELF
/// and Microsoft x64 under COFF. Windows and UEFI share the COFF row because
/// their `NativeTarget` contracts are indistinguishable at this layer. An
/// undeclared pair — including every non-x86-64 architecture — fails closed
/// rather than silently inheriting one family's rows; adding a supported
/// x86-64 format extends this matrix deliberately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum X86_64SelectedAbi {
    SystemV,
    Microsoft,
}

/// Resolve the declared pair to its ABI family. Every selected-form encoding
/// that needs the call/return roster — the machine-effect catalog and the
/// scalar-call template validator alike — consults this matrix rather than
/// re-reading the target, so an undeclared pair fails closed once, here.
pub(crate) fn x86_64_selected_abi(
    target: NativeTarget,
) -> Result<X86_64SelectedAbi, X86_64MachineEffectCatalogValidationError> {
    match (target.architecture, target.object_format) {
        // Intel macOS C calls follow the same System V AMD64 roster as Linux.
        (Architecture::X86_64, ObjectFormat::Elf | ObjectFormat::MachO) => {
            Ok(X86_64SelectedAbi::SystemV)
        }
        (Architecture::X86_64, ObjectFormat::Coff) => Ok(X86_64SelectedAbi::Microsoft),
        (Architecture::Aarch64, ObjectFormat::Elf)
        | (Architecture::Aarch64, ObjectFormat::MachO)
        | (Architecture::Aarch64, ObjectFormat::Coff) => {
            Err(X86_64MachineEffectCatalogValidationError::UnsupportedTargetAbi)
        }
    }
}

/// The exact register-constraint key roster the x86-64 encoding matrix
/// declares for one target: every constraint row this target's selected forms
/// may use, selected by the ABI family its declared pair resolves to. The
/// machine-effect catalog and the allocator register environment both consume
/// this one roster instead of deriving their own, so the allocator cannot
/// drift from the effect declarations. `None` means the pair has no x86-64 ABI.
pub fn x86_64_selected_constraint_keys(target: NativeTarget) -> Option<SelectedConstraintKeys> {
    let Ok(abi) = x86_64_selected_abi(target) else {
        return None;
    };
    let microsoft = abi == X86_64SelectedAbi::Microsoft;
    let return_i64 = match abi {
        X86_64SelectedAbi::SystemV => X86_64_SYSTEM_V_RETURN,
        X86_64SelectedAbi::Microsoft => X86_64_MICROSOFT_RETURN,
    };
    let return_unit = match abi {
        X86_64SelectedAbi::SystemV => X86_64_SYSTEM_V_RETURN_UNIT,
        X86_64SelectedAbi::Microsoft => X86_64_MICROSOFT_RETURN_UNIT,
    };
    Some({
        let rows = TargetRegisterEnvironmentConstraintKeys {
            port_write: Some(crate::X86_64_PORT_WRITE),
            system_register_read: Some(crate::X86_64_FLAGS_SNAPSHOT),
            system_register_write: Some(crate::X86_64_FLAGS_RESTORE),
            control_register_read: Some(crate::X86_64_CONTROL_REGISTER_READ),
            control_register_write: Some(crate::X86_64_CONTROL_REGISTER_WRITE),
            port_register_read: Some(crate::X86_64_PORT_REGISTER_READ),
            port_register_write: Some(crate::X86_64_PORT_REGISTER_WRITE),
            model_specific_read: Some(crate::X86_64_MODEL_SPECIFIC_READ),
            model_specific_write: Some(crate::X86_64_MODEL_SPECIFIC_WRITE),
            processor_halt: Some(crate::X86_64_PROCESSOR_HALT),
            interrupt_control: Some(crate::X86_64_INTERRUPT_CONTROL),
            memory_fence: Some(crate::X86_64_MEMORY_FENCE),
            atomic_fence: Some(crate::X86_64_ATOMIC_FENCE),
            // LOCK is architectural on every x86-64 target; no feature custody
            // gates the family the way AArch64's LSE does.
            access_read: Some(crate::X86_64_ACCESSOR_READ),
            access_write: Some(crate::X86_64_ACCESSOR_WRITE),
            atomic_load: Some(crate::X86_64_ATOMIC_LOAD),
            atomic_store: Some(crate::X86_64_ATOMIC_STORE),
            atomic_update: Some(crate::X86_64_ATOMIC_UPDATE),
            atomic_bitwise_update: Some(crate::X86_64_ATOMIC_BITWISE_UPDATE),
            atomic_compare_exchange: Some(crate::X86_64_ATOMIC_COMPARE_EXCHANGE),
            crash: crate::X86_64_CRASH,
            load_packed: Some(crate::X86_64_LOAD_PACKED),
            store_packed: Some(crate::X86_64_STORE_PACKED),
            call_aggregate: match abi {
                X86_64SelectedAbi::SystemV => {
                    crate::register_model::x86_64_system_v_aggregate_call_keys()
                        .into_iter()
                        .chain(crate::x86_64_system_v_mixed_aggregate_call_keys())
                        .chain(crate::x86_64_indirect_aggregate_call_keys(false))
                        .collect()
                }
                X86_64SelectedAbi::Microsoft => {
                    crate::register_model::x86_64_microsoft_aggregate_call_keys()
                        .into_iter()
                        .chain(crate::x86_64_microsoft_mixed_aggregate_call_keys())
                        .chain(crate::x86_64_indirect_aggregate_call_keys(true))
                        .collect()
                }
            },
            return_aggregate: match abi {
                X86_64SelectedAbi::SystemV => {
                    crate::register_model::x86_64_system_v_aggregate_return_keys()
                }
                X86_64SelectedAbi::Microsoft => {
                    crate::register_model::x86_64_microsoft_aggregate_return_keys()
                }
            },
            hosted_read_byte: crate::hosted_syscall_target(target)
                .then_some(crate::X86_64_HOSTED_READ_BYTE),
            hosted_exit_process_i32: crate::hosted_syscall_target(target)
                .then_some(crate::X86_64_HOSTED_EXIT_PROCESS_I32),
            hosted_write_byte_i32: crate::hosted_syscall_target(target)
                .then_some(crate::X86_64_HOSTED_WRITE_BYTE_I32),
            load64: Some(crate::X86_64_LOAD64),
            load8: Some(crate::X86_64_LOAD8),
            load16: Some(crate::X86_64_LOAD16),
            load32: Some(crate::X86_64_LOAD32),
            load8_indexed: Some(crate::X86_64_LOAD8_INDEXED),
            copy_bytes: Some(crate::X86_64_COPY_BYTES),
            zero_bytes: Some(crate::X86_64_ZERO_BYTES),
            scan_byte_sequence: Some(crate::X86_64_SCAN_BYTE_SEQUENCE),
            load_encoded_scalar: Some(crate::X86_64_LOAD_ENCODED_SCALAR),
            store_encoded_scalar: Some(crate::X86_64_STORE_ENCODED_SCALAR),
            compare_byte_sequences: Some(crate::X86_64_COMPARE_BYTE_SEQUENCES),
            save_floating_control: Some(crate::X86_64_SAVE_FLOATING_CONTROL),
            restore_floating_control: Some(crate::X86_64_RESTORE_FLOATING_CONTROL),
            store: Some(crate::X86_64_STORE),
            address_offset: Some(crate::X86_64_ADDRESS_OFFSET),
            store64: Some(crate::X86_64_STORE64),
            frame_address: Some(crate::X86_64_FRAME_ADDRESS),
            byte_literal_image_address: Some(crate::X86_64_BYTE_LITERAL_IMAGE_ADDRESS),
            call_unit_mixed: match abi {
                X86_64SelectedAbi::SystemV => crate::x86_64_system_v_mixed_unit_call_keys(),
                X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_mixed_unit_call_keys(),
            },
            call_unit: match abi {
                X86_64SelectedAbi::SystemV => crate::x86_64_system_v_register_unit_call_keys(),
                X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_register_unit_call_keys(),
            },
            call_scalar: (match abi {
                X86_64SelectedAbi::SystemV => x86_64_system_v_register_call_keys(),
                X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_register_call_keys(),
            })
            .into_iter()
            .chain(crate::x86_64_float_scalar_call_keys(microsoft))
            .collect(),
            call_normalized_foreign: (match abi {
                X86_64SelectedAbi::SystemV => crate::x86_64_system_v_normalized_foreign_call_keys(),
                X86_64SelectedAbi::Microsoft => {
                    crate::x86_64_microsoft_normalized_foreign_call_keys()
                }
            })
            .into_iter()
            .chain(
                (target.object_format == ObjectFormat::Elf)
                    .then(crate::x86_64_linux_system_call_keys)
                    .into_iter()
                    .flatten(),
            )
            .collect(),
            materialize_i64: X86_64_MATERIALIZE_I64,
            materialize_boolean: crate::X86_64_MATERIALIZE_BOOLEAN,
            copy_i64: X86_64_COPY_I64,
            float32_to_bits: Some(crate::X86_64_FLOAT32_TO_BITS),
            float64_to_bits: Some(crate::X86_64_FLOAT64_TO_BITS),
            bits_to_float32: Some(crate::X86_64_BITS_TO_FLOAT32),
            bits_to_float64: Some(crate::X86_64_BITS_TO_FLOAT64),
            ieee_float_arithmetic: Some(crate::X86_64_IEEE_FLOAT_ARITHMETIC),
            ieee_float_directed_binary: Some(crate::X86_64_IEEE_FLOAT_DIRECTED_BINARY),
            ieee_float_directed_unary: Some(crate::X86_64_IEEE_FLOAT_DIRECTED_UNARY),
            ieee_float_unary: Some(crate::X86_64_IEEE_FLOAT_UNARY),
            ieee_float_conversion_control: Some(crate::X86_64_IEEE_FLOAT_CONVERSION_CONTROL),
            ieee_float_ternary: Some(crate::X86_64_IEEE_FLOAT_TERNARY),
            float_result_trapping: Some(crate::X86_64_FLOAT_RESULT_TRAPPING),
            float_result_saturating: Some(crate::X86_64_FLOAT_RESULT_SATURATING),
            add_i64: X86_64_ADD_I64,
            subtract_i64: X86_64_SUBTRACT_I64,
            multiply_i64: crate::X86_64_MULTIPLY_I64,
            saturating_subtract_unsigned:
                crate::register_model::X86_64_SATURATING_SUBTRACT_UNSIGNED,
            saturating_add_u64: crate::register_model::X86_64_SATURATING_ADD_U64,
            divide_u64: crate::register_model::X86_64_DIVIDE_U64,
            remainder_u64: crate::register_model::X86_64_REMAINDER_U64,
            remainder_i64: crate::register_model::X86_64_REMAINDER_I64,
            divide_i64: crate::register_model::X86_64_DIVIDE_I64,
            shift_i64: crate::register_model::X86_64_SHIFT_I64,
            saturating_add_clamped: crate::register_model::X86_64_SATURATING_ADD_CLAMPED,
            saturating_subtract_clamped: crate::register_model::X86_64_SATURATING_SUBTRACT_CLAMPED,
            saturating_divide_signed: crate::register_model::X86_64_SATURATING_DIVIDE_SIGNED,
            saturating_multiply_clamped: crate::register_model::X86_64_SATURATING_MULTIPLY_CLAMPED,
            saturating_multiply_u64: crate::register_model::X86_64_SATURATING_MULTIPLY_U64,
            trapping_binary: crate::X86_64_TRAPPING_BINARY,
            trapping_fixed_pair: crate::X86_64_TRAPPING_FIXED_PAIR,
            trapping_shift: crate::X86_64_TRAPPING_SHIFT,
            trapping_convert: crate::X86_64_TRAPPING_CONVERT,
            add_i64_immediate: X86_64_ADD_I64_IMMEDIATE,
            subtract_i64_immediate: X86_64_SUBTRACT_I64_IMMEDIATE,
            compare_i64_zero: X86_64_COMPARE_I64_ZERO,
            compare_i64: X86_64_COMPARE_I64,
            compare_i64_immediate: X86_64_COMPARE_I64_IMMEDIATE,
            conditional_branch: X86_64_CONDITIONAL_BRANCH,
            jump: crate::X86_64_JUMP,
            return_float: crate::x86_64_float_scalar_return_keys(microsoft),
            return_i64,
            return_unit,
        };
        let mut completed = SelectedConstraintKeys {
            rows,
            invoke_no_normal: Vec::new(),
        };
        completed.invoke_no_normal = completed.ordinary_call_constraints();
        completed
    })
}

fn declaration(semantic: Semantic, keys: &SelectedConstraintKeys) -> MachineEffectDeclaration {
    let always = |width| alternative(semantic, 0, MachineAlternativeApplicability::Always, width);
    let alternatives = match semantic {
        Semantic::ExactDivideU64
        | Semantic::ExactRemainderU64
        | Semantic::WrappingRemainderI64
        | Semantic::WrappingDivideI64
        | Semantic::ExactDivideI64
        | Semantic::ExactRemainderI64
        | Semantic::SaturatingAdd(_)
        | Semantic::SaturatingSubtract(_)
        | Semantic::SaturatingDivide(_)
        | Semantic::SaturatingRemainder(_)
        | Semantic::SaturatingMultiply(_)
        | Semantic::TrappingInteger(_)
        | Semantic::BitwiseAndI64
        | Semantic::BitwiseOrI64
        | Semantic::BitwiseNotI64
        | Semantic::BitwiseXorI64
        | Semantic::ByteViewAddress
        | Semantic::WrappingAddI64
        | Semantic::ExactAddI64 => {
            vec![always(size(semantic))]
        }
        Semantic::ExactSubtractI64 | Semantic::WrappingSubtractI64 => {
            binary_alias_ladder(semantic, [3, 3, 6, 6])
        }
        // `imul r64, r64` is one REX.W + 0F AF form in every alias case; only
        // the distinct-result variant prepends a three-byte `mov`.
        Semantic::ExactMultiplyI64 | Semantic::WrappingMultiplyI64 => {
            binary_alias_ladder(semantic, [4, 4, 4, 7])
        }
        Semantic::ConditionalBranchU64LessThan | Semantic::ConditionalBranchI64LessThan => {
            vec![always(Size::ExactBytes(6))]
        }
        // The early-clobber result never shares the pinned RCX count or the
        // value it is initialized from, so `mov result, value; shX result, cl`
        // is the only realization and applies to every allocation.
        Semantic::WrappingShiftLeftI64
        | Semantic::WrappingShiftRightI64
        | Semantic::WrappingShiftRightU64
        | Semantic::ExactShiftLeftI64
        | Semantic::ExactShiftRightI64
        | Semantic::ExactShiftRightU64 => {
            vec![always(Size::ExactBytes(6))]
        }
        // Variant 1 is the three-byte `xor r64, r64` zero idiom. The
        // legality rule never chooses it: `X86SelectXorZeroI64MaterializationV1`
        // rewrites a zero materialization onto it after proving RFLAGS dead.
        Semantic::MaterializeI64 => vec![
            always(size(semantic)),
            alternative(
                semantic,
                1,
                MachineAlternativeApplicability::OptimizationSelected {
                    optimization:
                        optimization_core::Optimization::X86SelectXorZeroI64MaterializationV1,
                },
                Size::ExactBytes(3),
            ),
        ],
        _ => vec![always(size(semantic))],
    };
    MachineEffectDeclaration {
        semantic,
        constraint: keys
            .for_semantic(semantic)
            .expect("required x86-64 machine semantic has a constraint"),
        memory: if matches!(semantic, Semantic::AtomicAccess(_)) {
            MachineMemoryEffect::AtomicPointerV1
        } else {
            MachineMemoryEffect::NoneV1
        },
        // A Trapping form may stop the process at its inline UD2, which is
        // as observable as a hosted effect: it is ordered like one and is
        // never dead. An atomic access dereferences a pointer the cell owns.
        trap: if matches!(semantic, Semantic::TrappingInteger(_)) {
            MachineTrapBehavior::TrappingIntegerV1
        } else if matches!(semantic, Semantic::AtomicAccess(_)) {
            MachineTrapBehavior::MayArchitecturalFaultV1
        } else {
            MachineTrapBehavior::NeverV1
        },
        barrier: if matches!(
            semantic,
            Semantic::ConditionalBranchNonZero
                | Semantic::ConditionalBranchU64LessThan
                | Semantic::ConditionalBranchI64LessThan
                | Semantic::ReturnScalar
                | Semantic::ReturnAggregate
                | Semantic::Jump
                | Semantic::ReturnUnit
        ) {
            MachineBarrier::ControlFlow
        } else if matches!(
            semantic,
            Semantic::TrappingInteger(_) | Semantic::AtomicAccess(_)
        ) {
            MachineBarrier::ExternalEffect
        } else {
            MachineBarrier::None
        },
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives,
    }
}

/// Two-input ops that define a third result home run the same four-way
/// ladder: both operands aliased together, each operand aliased alone and
/// the distinct-result variant. Only the encoded sizes differ per family.
fn binary_alias_ladder(semantic: Semantic, sizes: [u16; 4]) -> Vec<MachineAlternative> {
    vec![
        alternative(
            semantic,
            0,
            MachineAlternativeApplicability::ResultAliasesOperands {
                result: 2,
                left: 0,
                right: 1,
            },
            Size::ExactBytes(sizes[0]),
        ),
        alternative(
            semantic,
            1,
            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                result: 2,
                aliased_operand: 0,
                distinct_operand: 1,
            },
            Size::ExactBytes(sizes[1]),
        ),
        alternative(
            semantic,
            2,
            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                result: 2,
                aliased_operand: 1,
                distinct_operand: 0,
            },
            Size::ExactBytes(sizes[2]),
        ),
        alternative(
            semantic,
            3,
            MachineAlternativeApplicability::ResultDistinctFromOperands {
                result: 2,
                left: 0,
                right: 1,
            },
            Size::ExactBytes(sizes[3]),
        ),
    ]
}

fn alternative(
    semantic: Semantic,
    variant: u32,
    applicability: MachineAlternativeApplicability,
    size: Size,
) -> MachineAlternative {
    MachineAlternative {
        key: MachineAlternativeKey {
            family: semantic.into(),
            variant,
        },
        applicability,
        size,
        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
        encoded: encoded_effects(semantic, variant),
    }
}

fn encoded_effects(semantic: Semantic, variant: u32) -> MachineEncodedEffects {
    if semantic == Semantic::Crash {
        return selected_instructions::MachineEncodedEffects::crash_v1(&units("rip"));
    }
    if let Semantic::TrappingInteger(form) = semantic {
        return TrappingShape::of(form).encoded_effects();
    }
    if let Semantic::NearestIeeeFloatArithmetic(form) = semantic {
        return crate::selected_form_encoding::ieee_float_arithmetic::effects(form);
    }
    let (reads, writes) = match semantic {
        Semantic::PortRegisterRead
        | Semantic::PortRegisterWrite
        | Semantic::ModelSpecificRead
        | Semantic::ModelSpecificWrite
        | Semantic::ProcessorHalt
        | Semantic::InterruptControl
        | Semantic::ControlRegisterRead
        | Semantic::ControlRegisterWrite
        | Semantic::SystemRegisterRead => {
            return crate::selected_form_encoding::register_access::effects(
                crate::selected_form_encoding::register_access::operation(semantic)
                    .expect("x86 register semantic"),
            );
        }
        Semantic::SystemRegisterWrite => {
            return crate::selected_form_encoding::register_access::effects(
                crate::selected_form_encoding::register_access::operation(semantic)
                    .expect("x86 flags restore"),
            );
        }
        Semantic::AccessorScalarAccess(form) => {
            if form.write {
                (vec![0, 1], vec![])
            } else {
                (vec![0], vec![1])
            }
        }
        Semantic::PortWrite => {
            return crate::selected_form_encoding::port_write::effects();
        }
        Semantic::MemoryFence => return MachineEncodedEffects::catalog_fence_v1(),
        Semantic::AtomicFence => return MachineEncodedEffects::atomic_fence_v1(),
        Semantic::FloatResultPolicy(form) => {
            return crate::selected_form_encoding::float_result_policy::effects(form);
        }
        // LOCK-prefixed atomic forms read their address plus operation
        // inputs and write the observed-prior output alone.
        Semantic::AtomicAccess(form) => (
            (0..form.operation.input_count()).collect(),
            form.observed_operand().into_iter().collect(),
        ),
        Semantic::NearestIeeeFloatArithmetic(_) => {
            unreachable!("float arithmetic effects returned above")
        }
        Semantic::Crash => (Vec::new(), Vec::new()),
        Semantic::CompareI64Zero | Semantic::CompareI64Immediate => (vec![0], vec![]),
        Semantic::CompareI64 => (vec![0, 1], vec![]),
        Semantic::MaterializeI64 | Semantic::ImageAddress => (vec![], vec![0]),
        Semantic::MaterializeBooleanEqual
        | Semantic::MaterializeBooleanU64LessThan
        | Semantic::MaterializeBooleanI64LessThan
        | Semantic::MaterializeBooleanU64LessOrEqual
        | Semantic::MaterializeBooleanI64LessOrEqual => (vec![], vec![0]),
        Semantic::Float32ToBits
        | Semantic::Float64ToBits
        | Semantic::BitsToFloat32
        | Semantic::BitsToFloat64
        | Semantic::CopyI64
        | Semantic::ZeroExtendU8
        | Semantic::ZeroExtendU16
        | Semantic::SignExtendI8
        | Semantic::SignExtendI16
        | Semantic::SignExtendI32
        | Semantic::ZeroExtendU32 => (vec![0], vec![1]),
        Semantic::ExactDivideU64 => (vec![0, 1, 3], vec![2]),
        Semantic::WrappingRemainderI64
        | Semantic::ExactRemainderU64
        | Semantic::WrappingDivideI64
        | Semantic::ExactDivideI64
        | Semantic::ExactRemainderI64 => (vec![0, 1], vec![2, 3]),
        Semantic::SaturatingAdd(carrier)
        | Semantic::SaturatingSubtract(carrier)
        | Semantic::SaturatingDivide(carrier)
        | Semantic::SaturatingRemainder(carrier)
        | Semantic::SaturatingMultiply(carrier) => {
            saturating_form(semantic, carrier).operand_reads_and_writes()
        }
        Semantic::BitwiseAndI64
        | Semantic::BitwiseOrI64
        | Semantic::BitwiseXorI64
        | Semantic::ByteViewAddress
        | Semantic::WrappingAddI64
        | Semantic::ExactAddI64 => (vec![0, 1], vec![2]),
        Semantic::BitwiseNotI64 => (vec![0], vec![1]),
        Semantic::ExactAddI64Immediate | Semantic::ExactSubtractI64Immediate => (vec![0], vec![1]),
        Semantic::ExactSubtractI64 | Semantic::WrappingSubtractI64 if variant == 0 => {
            (vec![], vec![2])
        }
        Semantic::ExactSubtractI64 | Semantic::WrappingSubtractI64 => (vec![0, 1], vec![2]),
        // `imul r, r` genuinely reads its input to square it, so even the
        // fully-aliased variant 0 reads both external operands.
        Semantic::ExactMultiplyI64 | Semantic::WrappingMultiplyI64 => (vec![0, 1], vec![2]),
        Semantic::WrappingShiftLeftI64
        | Semantic::WrappingShiftRightI64
        | Semantic::WrappingShiftRightU64
        | Semantic::ExactShiftLeftI64
        | Semantic::ExactShiftRightI64
        | Semantic::ExactShiftRightU64 => (vec![0, 1], vec![2]),
        Semantic::ConditionalBranchNonZero
        | Semantic::ConditionalBranchU64LessThan
        | Semantic::ConditionalBranchI64LessThan
        | Semantic::ReturnScalar
        | Semantic::ReturnAggregate
        | Semantic::Jump
        | Semantic::ReturnUnit => (vec![], vec![]),
        // The family macro restates the memory module's whole arm set; the
        // placement form is matched by its own arm above.
        #[allow(unreachable_patterns)]
        Semantic::CallScalar
        | Semantic::InvokeNoNormal
        | Semantic::CallAggregate
        | Semantic::NormalizedForeignCall
        | Semantic::HostedExitProcessI32
        | Semantic::HostedReadByte
        | Semantic::SaveFloatingControl
        | Semantic::RestoreFloatingControl
        | Semantic::HostedWriteByteI32
        | memory::declared_semantics!() => {
            unreachable!("semantics with a dedicated declaration do not reach the generic effects")
        }
        Semantic::TrappingInteger(_) => {
            unreachable!("trapping forms return their shared encoded effects above")
        }
    };
    let (implicit_uses, implicit_defs, implicit_clobbers, memory, stack, trap, control) =
        match semantic {
            Semantic::MaterializeBooleanEqual
            | Semantic::MaterializeBooleanU64LessThan
            | Semantic::MaterializeBooleanI64LessThan
            | Semantic::MaterializeBooleanU64LessOrEqual
            | Semantic::MaterializeBooleanI64LessOrEqual => (
                units("rflags"),
                vec![],
                vec![],
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::NeverV1,
                MachineEncodedControlEffect::FallThroughV1,
            ),
            Semantic::CompareI64Zero | Semantic::CompareI64 | Semantic::CompareI64Immediate => (
                vec![],
                units("rflags"),
                vec![],
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::NeverV1,
                MachineEncodedControlEffect::FallThroughV1,
            ),
            Semantic::ExactDivideU64 | Semantic::SaturatingDivide(_) => (
                vec![],
                vec![],
                units_of(&["rdx", "rflags"]),
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                MachineEncodedControlEffect::FallThroughV1,
            ),
            Semantic::WrappingRemainderI64
            | Semantic::ExactRemainderU64
            | Semantic::WrappingDivideI64
            | Semantic::ExactDivideI64
            | Semantic::ExactRemainderI64
            | Semantic::SaturatingRemainder(_) => (
                vec![],
                vec![],
                units("rflags"),
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                MachineEncodedControlEffect::FallThroughV1,
            ),
            Semantic::BitwiseAndI64
            | Semantic::BitwiseOrI64
            | Semantic::BitwiseXorI64
            | Semantic::SaturatingAdd(_)
            | Semantic::SaturatingSubtract(_)
            | Semantic::SaturatingMultiply(_)
            | Semantic::ExactMultiplyI64
            | Semantic::WrappingMultiplyI64
            | Semantic::WrappingShiftLeftI64
            | Semantic::WrappingShiftRightI64
            | Semantic::WrappingShiftRightU64
            | Semantic::ExactShiftLeftI64
            | Semantic::ExactShiftRightI64
            | Semantic::ExactShiftRightU64
            | Semantic::ExactSubtractI64
            | Semantic::WrappingSubtractI64 => (
                vec![],
                vec![],
                units("rflags"),
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::NeverV1,
                MachineEncodedControlEffect::FallThroughV1,
            ),
            Semantic::Jump => (
                units("rip"),
                units("rip"),
                vec![],
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
            ),
            Semantic::ConditionalBranchNonZero
            | Semantic::ConditionalBranchU64LessThan
            | Semantic::ConditionalBranchI64LessThan => {
                let uses = units_of(&["rflags", "rip"]);
                (
                    uses,
                    units("rip"),
                    vec![],
                    MachineEncodedMemoryEffect::NoneV1,
                    MachineEncodedStackEffect::UnchangedV1,
                    MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    MachineEncodedControlEffect::ConditionalRelativeBranchV1,
                )
            }
            Semantic::ReturnScalar | Semantic::ReturnAggregate | Semantic::ReturnUnit => {
                let stack_pointer = view_of("rsp").id;
                let defs = units_of(&["rsp", "rip"]);
                (
                    units("rsp"),
                    defs,
                    vec![],
                    MachineEncodedMemoryEffect::ReadActivationStackV1 {
                        stack_pointer,
                        byte_count: 8,
                    },
                    MachineEncodedStackEffect::PopBytesV1 {
                        stack_pointer,
                        byte_count: 8,
                    },
                    MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    MachineEncodedControlEffect::ReturnFromActivationStackV1,
                )
            }
            Semantic::AtomicAccess(form) => {
                let encoded = crate::selected_form_encoding::atomic::effects(form);
                (
                    encoded.implicit_unit_uses,
                    encoded.implicit_unit_defs,
                    encoded.implicit_unit_clobbers,
                    encoded.memory,
                    encoded.stack,
                    encoded.trap,
                    encoded.control,
                )
            }
            // The xor-zero idiom defines its destination from no input and
            // clobbers RFLAGS; the baseline `mov r64, imm64` touches no flag.
            Semantic::MaterializeI64 if variant == 1 => (
                vec![],
                vec![],
                units("rflags"),
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::NeverV1,
                MachineEncodedControlEffect::FallThroughV1,
            ),
            _ => (
                vec![],
                vec![],
                vec![],
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::NeverV1,
                MachineEncodedControlEffect::FallThroughV1,
            ),
        };
    MachineEncodedEffects {
        external_operand_reads: reads,
        external_operand_writes: writes,
        implicit_unit_uses: implicit_uses,
        implicit_unit_defs: implicit_defs,
        implicit_unit_clobbers: implicit_clobbers,
        memory,
        stack,
        trap,
        control,
    }
}

/// The realization shape of a saturating semantic, which fixes its operand
/// reads and writes and its exact size.
fn saturating_form(semantic: Semantic, carrier: SaturatingCarrier) -> SaturatingForm {
    let operation = match semantic {
        Semantic::SaturatingAdd(_) => SaturatingOperation::Add,
        Semantic::SaturatingSubtract(_) => SaturatingOperation::Subtract,
        Semantic::SaturatingDivide(_) => SaturatingOperation::Divide,
        Semantic::SaturatingRemainder(_) => SaturatingOperation::Remainder,
        Semantic::SaturatingMultiply(_) => SaturatingOperation::Multiply,
        _ => unreachable!("saturating semantics carry their operation"),
    };
    SaturatingForm::of(operation, carrier)
}

fn size(semantic: Semantic) -> Size {
    match semantic {
        Semantic::PortRegisterRead
        | Semantic::PortRegisterWrite
        | Semantic::ModelSpecificRead
        | Semantic::ModelSpecificWrite
        | Semantic::SystemRegisterRead
        | Semantic::SystemRegisterWrite
        | Semantic::ControlRegisterRead
        | Semantic::ControlRegisterWrite => crate::encoder_resolved(1, Some(40)),
        Semantic::AccessorScalarAccess(_) => crate::encoder_resolved(7, Some(9)),
        Semantic::PortWrite => Size::ExactBytes(x86_encoding::IMMEDIATE_PORT_WRITE_WIDTH as u16),
        Semantic::AtomicFence | Semantic::MemoryFence => Size::ExactBytes(3),
        Semantic::ProcessorHalt | Semantic::InterruptControl => Size::ExactBytes(1),
        Semantic::FloatResultPolicy(form) => {
            crate::selected_form_encoding::float_result_policy::size(form)
        }
        Semantic::AtomicAccess(_) => crate::encoder_resolved(1, None),
        Semantic::NearestIeeeFloatArithmetic(_) => {
            unreachable!("float arithmetic uses its dedicated declaration")
        }
        Semantic::Crash => Size::ExactBytes(2),
        Semantic::MaterializeBooleanEqual
        | Semantic::MaterializeBooleanU64LessThan
        | Semantic::MaterializeBooleanI64LessThan
        | Semantic::MaterializeBooleanU64LessOrEqual
        | Semantic::MaterializeBooleanI64LessOrEqual => Size::ExactBytes(8),
        Semantic::Jump => Size::ExactBytes(5),
        Semantic::CompareI64Zero | Semantic::CompareI64 | Semantic::CopyI64 => Size::ExactBytes(3),
        Semantic::CompareI64Immediate => Size::ExactBytes(7),
        Semantic::Float32ToBits | Semantic::BitsToFloat32 => crate::encoder_resolved(4, Some(5)),
        Semantic::Float64ToBits | Semantic::BitsToFloat64 => Size::ExactBytes(5),
        Semantic::ZeroExtendU8
        | Semantic::ZeroExtendU16
        | Semantic::SignExtendI8
        | Semantic::SignExtendI16 => Size::ExactBytes(4),
        Semantic::ZeroExtendU32 | Semantic::SignExtendI32 => Size::ExactBytes(3),
        Semantic::MaterializeI64 => Size::ExactBytes(10),
        // `lea r64, [rip+disp32]` with the disp32 left for object construction.
        Semantic::ImageAddress => Size::ExactBytes(7),
        Semantic::ExactDivideU64 => Size::ExactBytes(3),
        Semantic::WrappingRemainderI64 => Size::ExactBytes(17),
        // `xor` the RDX high half, unsigned `div`, then move the remainder
        // from RDX into the RAX result home.
        Semantic::ExactRemainderU64 => Size::ExactBytes(9),
        // `cmp` the divisor against -1, `jne` to the divide, `neg` the
        // dividend for the wrapping MIN / -1 answer, `jmp` over `cqo; idiv`.
        Semantic::WrappingDivideI64 => Size::ExactBytes(16),
        // Bare `cqo; idiv` / `cqo; idiv; mov rdx, rax`: the proven exact
        // obligations exclude every faulting input the guard exists for.
        Semantic::ExactDivideI64 => Size::ExactBytes(5),
        Semantic::ExactRemainderI64 => Size::ExactBytes(8),
        Semantic::SaturatingAdd(carrier)
        | Semantic::SaturatingSubtract(carrier)
        | Semantic::SaturatingDivide(carrier)
        | Semantic::SaturatingRemainder(carrier)
        | Semantic::SaturatingMultiply(carrier) => {
            Size::ExactBytes(saturating_form(semantic, carrier).byte_count())
        }
        // Every instruction of a Trapping realization has a register-free
        // length, so each form has one exact size.
        Semantic::TrappingInteger(form) => Size::ExactBytes(TrappingShape::of(form).byte_count()),
        Semantic::BitwiseAndI64
        | Semantic::BitwiseOrI64
        | Semantic::BitwiseNotI64
        | Semantic::BitwiseXorI64 => crate::encoder_resolved(3, Some(6)),
        Semantic::ByteViewAddress | Semantic::WrappingAddI64 | Semantic::ExactAddI64 => {
            crate::encoder_resolved(4, Some(5))
        }
        Semantic::ExactAddI64Immediate => crate::encoder_resolved(4, Some(8)),
        Semantic::ExactSubtractI64Immediate => crate::encoder_resolved(4, Some(8)),
        Semantic::ConditionalBranchNonZero
        | Semantic::ConditionalBranchU64LessThan
        | Semantic::ConditionalBranchI64LessThan => crate::encoder_resolved(2, Some(6)),
        Semantic::ReturnScalar | Semantic::ReturnAggregate | Semantic::ReturnUnit => {
            Size::ExactBytes(1)
        }
        Semantic::ExactSubtractI64
        | Semantic::WrappingSubtractI64
        | Semantic::ExactMultiplyI64
        | Semantic::WrappingMultiplyI64
        | Semantic::WrappingShiftLeftI64
        | Semantic::WrappingShiftRightI64
        | Semantic::WrappingShiftRightU64
        | Semantic::ExactShiftLeftI64
        | Semantic::ExactShiftRightI64
        | Semantic::ExactShiftRightU64 => {
            unreachable!("subtraction, multiplication, and shifts declare explicit alternatives")
        }
        // The family macro restates the memory module's whole arm set; the
        // placement form is matched by its own arm above.
        #[allow(unreachable_patterns)]
        Semantic::CallScalar
        | Semantic::InvokeNoNormal
        | Semantic::CallAggregate
        | Semantic::NormalizedForeignCall
        | Semantic::HostedExitProcessI32
        | Semantic::HostedReadByte
        | Semantic::SaveFloatingControl
        | Semantic::RestoreFloatingControl
        | Semantic::HostedWriteByteI32
        | memory::declared_semantics!() => {
            unreachable!("semantics with a dedicated declaration have no generic size")
        }
    }
}

#[cfg(test)]
mod tests;

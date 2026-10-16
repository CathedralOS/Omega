use register_model::ValidatedRegisterConstraintCatalog;
use selected_instructions::{
    IsaCatalogValidation, MachineAlternative, MachineAlternativeApplicability,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalog, MachineEffectCatalogValidationError, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
    MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
    SaturatingCarrier, SaturatingOperation, SelectedConstraintKeys, ValidatedMachineEffectCatalog,
};
use target::{Architecture, NativeTarget, ObjectFormat};

mod memory;
mod scalar_call;
pub use scalar_call::x86_64_normalized_foreign_call_declaration;
pub(crate) use scalar_call::{normalized_foreign_constraint, user_direct_call_effects};

use scalar_call::declaration as scalar_call_declaration;

use crate::selected_form_encoding::saturating_forms::SaturatingForm;
use crate::selected_form_encoding::trapping_forms::TrappingShape;
use crate::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64, X86_64_COMPARE_I64_IMMEDIATE,
    X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH, X86_64_COPY_I64, X86_64_MATERIALIZE_I64,
    X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT, X86_64_SUBTRACT_I64,
    X86_64_SUBTRACT_I64_IMMEDIATE, X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT,
    x86_64_system_v_register_call_keys,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64MachineEffectCatalogValidationError {
    TargetArchitectureMismatch,
    UnsupportedTargetAbi,
    Structural(MachineEffectCatalogValidationError),
    TargetSemanticMismatch,
}

impl std::fmt::Display for X86_64MachineEffectCatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid x86-64 machine effects: {self:?}")
    }
}

impl std::error::Error for X86_64MachineEffectCatalogValidationError {}

pub fn x86_64_machine_effect_catalog(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<MachineEffectCatalog, X86_64MachineEffectCatalogValidationError> {
    if target.architecture != Architecture::X86_64
        || constraints.architecture() != Architecture::X86_64
    {
        return Err(X86_64MachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    let selected_keys = selected_keys(target)?;
    Ok(MachineEffectCatalog {
        target,
        register_constraints: constraints.identity(),
        selected_keys: selected_keys.clone(),
        declarations: selected_keys
            .declaration_keys()
            .into_iter()
            .map(|(semantic, constraint)| {
                if let Some(operation) =
                    crate::selected_form_encoding::register_access::operation(semantic)
                {
                    return Ok(crate::selected_form_encoding::register_access::declaration(
                        operation, constraint,
                    ));
                }
                if semantic == MachineSemanticKind::PortWrite {
                    return Ok(crate::selected_form_encoding::port_write::declaration(
                        constraint,
                    ));
                }
                if semantic == MachineSemanticKind::AtomicFence {
                    return Ok(crate::selected_form_encoding::atomic_fence::declaration(
                        constraint,
                    ));
                }
                if semantic == MachineSemanticKind::Crash {
                    return Ok(crate::selected_form_encoding::crash::declaration(
                        constraints.catalog().constraint(constraint).ok_or(
                            X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch,
                        )?,
                    ));
                }
                if let MachineSemanticKind::FloatResultPolicy(form) = semantic {
                    return Ok(
                        crate::selected_form_encoding::float_result_policy::declaration(
                            form, constraint,
                        ),
                    );
                }
                if let MachineSemanticKind::NearestIeeeFloatArithmetic(form) = semantic {
                    return crate::selected_form_encoding::ieee_float_arithmetic::declaration(
                        form, constraint,
                    )
                    .map_err(|_| {
                        X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch
                    });
                }
                if semantic == MachineSemanticKind::HostedExitProcessI32 {
                    return crate::selected_form_encoding::hosted_exit_process::declaration(
                        target, constraint,
                    )
                    .map_err(|_| {
                        X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch
                    });
                }
                if semantic == MachineSemanticKind::HostedReadByte {
                    return Ok(
                        crate::selected_form_encoding::hosted_read_byte::declaration(
                            target, constraint,
                        ),
                    );
                }
                if matches!(
                    semantic,
                    MachineSemanticKind::SaveFloatingControl
                        | MachineSemanticKind::RestoreFloatingControl
                ) {
                    return Ok(
                        crate::selected_form_encoding::floating_control::declaration(
                            semantic, constraint,
                        ),
                    );
                }
                if semantic == MachineSemanticKind::HostedWriteByteI32 {
                    return Ok(
                        crate::selected_form_encoding::hosted_write_byte::declaration(
                            target, constraint,
                        ),
                    );
                }
                if semantic == MachineSemanticKind::NormalizedForeignCall {
                    return Ok(scalar_call::normalized_foreign_declaration(
                        constraint,
                        constraints,
                    ));
                }
                Ok(
                    if matches!(
                        semantic,
                        MachineSemanticKind::PlacedScalarAccess(_)
                            | MachineSemanticKind::Load8
                            | MachineSemanticKind::LoadPacked3
                            | MachineSemanticKind::LoadPacked5
                            | MachineSemanticKind::LoadPacked6
                            | MachineSemanticKind::LoadPacked7
                            | MachineSemanticKind::StorePacked
                            | MachineSemanticKind::Load16
                            | MachineSemanticKind::Load32
                            | MachineSemanticKind::Load64
                            | MachineSemanticKind::Store
                            | MachineSemanticKind::AddressOffset
                            | MachineSemanticKind::Load8Indexed
                            | MachineSemanticKind::CopyBytes
                            | MachineSemanticKind::ZeroBytes
                            | MachineSemanticKind::ScanByteSequence
                            | MachineSemanticKind::LoadEncodedScalar
                            | MachineSemanticKind::StoreEncodedScalar
                            | MachineSemanticKind::CompareByteSequences
                            | MachineSemanticKind::Store64
                            | MachineSemanticKind::FrameAddress
                            | MachineSemanticKind::CallUnit
                    ) {
                        memory::declaration(semantic, constraint, constraints)
                    } else if matches!(
                        semantic,
                        MachineSemanticKind::CallScalar | MachineSemanticKind::CallAggregate
                    ) {
                        scalar_call_declaration(semantic, constraint, constraints)
                    } else if matches!(
                        semantic,
                        MachineSemanticKind::ReturnAggregate | MachineSemanticKind::ReturnScalar
                    ) {
                        let mut returned =
                            declaration(MachineSemanticKind::ReturnScalar, &selected_keys);
                        returned.semantic = semantic;
                        returned.constraint = constraint;
                        returned.alternatives[0].key.family = semantic.into();
                        returned
                    } else {
                        declaration(semantic, &selected_keys)
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub fn validate_x86_64_machine_effect_catalog(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: MachineEffectCatalog,
) -> Result<ValidatedMachineEffectCatalog, X86_64MachineEffectCatalogValidationError> {
    selected_instructions::validate_isa_machine_effect_catalog(
        Architecture::X86_64,
        target,
        constraints,
        catalog,
        x86_64_machine_effect_catalog,
    )
    .map_err(|failure| match failure {
        IsaCatalogValidation::ArchitectureMismatch => {
            X86_64MachineEffectCatalogValidationError::TargetArchitectureMismatch
        }
        IsaCatalogValidation::Structural(error) => {
            X86_64MachineEffectCatalogValidationError::Structural(error)
        }
        IsaCatalogValidation::SemanticMismatch => {
            X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch
        }
        IsaCatalogValidation::Builder(error) => error,
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

fn selected_keys(
    target: NativeTarget,
) -> Result<SelectedConstraintKeys, X86_64MachineEffectCatalogValidationError> {
    let abi = x86_64_selected_abi(target)?;
    let microsoft = abi == X86_64SelectedAbi::Microsoft;
    let return_i64 = match abi {
        X86_64SelectedAbi::SystemV => X86_64_SYSTEM_V_RETURN,
        X86_64SelectedAbi::Microsoft => X86_64_MICROSOFT_RETURN,
    };
    let return_unit = match abi {
        X86_64SelectedAbi::SystemV => X86_64_SYSTEM_V_RETURN_UNIT,
        X86_64SelectedAbi::Microsoft => X86_64_MICROSOFT_RETURN_UNIT,
    };
    Ok(SelectedConstraintKeys {
        port_write: Some(crate::X86_64_PORT_WRITE),
        system_register_read: None,
        system_register_write: None,
        port_register_read: Some(crate::X86_64_PORT_REGISTER_READ),
        port_register_write: Some(crate::X86_64_PORT_REGISTER_WRITE),
        model_specific_read: Some(crate::X86_64_MODEL_SPECIFIC_READ),
        model_specific_write: Some(crate::X86_64_MODEL_SPECIFIC_WRITE),
        atomic_fence: Some(crate::X86_64_ATOMIC_FENCE),
        // LOCK is architectural on every x86-64 target; no feature custody
        // gates the family the way AArch64's LSE does.
        placed_read: Some(crate::X86_64_PLACED_READ),
        placed_write: Some(crate::X86_64_PLACED_WRITE),
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
            X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_normalized_foreign_call_keys(),
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
        saturating_subtract_unsigned: crate::register_model::X86_64_SATURATING_SUBTRACT_UNSIGNED,
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
    })
}

fn declaration(
    semantic: MachineSemanticKind,
    keys: &SelectedConstraintKeys,
) -> MachineEffectDeclaration {
    let alternatives = match semantic {
        MachineSemanticKind::ExactDivideU64
        | MachineSemanticKind::ExactRemainderU64
        | MachineSemanticKind::WrappingRemainderI64
        | MachineSemanticKind::WrappingDivideI64
        | MachineSemanticKind::ExactDivideI64
        | MachineSemanticKind::ExactRemainderI64
        | MachineSemanticKind::SaturatingAdd(_)
        | MachineSemanticKind::SaturatingSubtract(_)
        | MachineSemanticKind::SaturatingDivide(_)
        | MachineSemanticKind::SaturatingRemainder(_)
        | MachineSemanticKind::SaturatingMultiply(_)
        | MachineSemanticKind::TrappingInteger(_) => {
            vec![alternative(
                semantic,
                0,
                MachineAlternativeApplicability::Always,
                size(semantic),
            )]
        }
        MachineSemanticKind::BitwiseAndI64
        | MachineSemanticKind::BitwiseOrI64
        | MachineSemanticKind::BitwiseNotI64
        | MachineSemanticKind::BitwiseXorI64
        | MachineSemanticKind::ByteViewAddress
        | MachineSemanticKind::WrappingAddI64
        | MachineSemanticKind::ExactAddI64 => {
            vec![alternative(
                semantic,
                0,
                MachineAlternativeApplicability::Always,
                size(semantic),
            )]
        }
        MachineSemanticKind::ExactSubtractI64 | MachineSemanticKind::WrappingSubtractI64 => {
            binary_alias_ladder(semantic, [3, 3, 6, 6])
        }
        // `imul r64, r64` is one REX.W + 0F AF form in every alias case; only
        // the distinct-result variant prepends a three-byte `mov`.
        MachineSemanticKind::ExactMultiplyI64 | MachineSemanticKind::WrappingMultiplyI64 => {
            binary_alias_ladder(semantic, [4, 4, 4, 7])
        }
        MachineSemanticKind::ConditionalBranchU64LessThan
        | MachineSemanticKind::ConditionalBranchI64LessThan => vec![alternative(
            semantic,
            0,
            MachineAlternativeApplicability::Always,
            MachineSizeKnowledge::ExactBytes(6),
        )],
        // The early-clobber result never shares the pinned RCX count or the
        // value it is initialized from, so `mov result, value; shX result, cl`
        // is the only realization and applies to every allocation.
        MachineSemanticKind::WrappingShiftLeftI64
        | MachineSemanticKind::WrappingShiftRightI64
        | MachineSemanticKind::WrappingShiftRightU64
        | MachineSemanticKind::ExactShiftLeftI64
        | MachineSemanticKind::ExactShiftRightI64
        | MachineSemanticKind::ExactShiftRightU64 => vec![alternative(
            semantic,
            0,
            MachineAlternativeApplicability::Always,
            MachineSizeKnowledge::ExactBytes(6),
        )],
        // Variant 1 is the three-byte `xor r64, r64` zero idiom. The
        // legality rule never chooses it: `X86SelectXorZeroI64MaterializationV1`
        // rewrites a zero materialization onto it after proving RFLAGS dead.
        MachineSemanticKind::MaterializeI64 => vec![
            alternative(
                semantic,
                0,
                MachineAlternativeApplicability::Always,
                size(semantic),
            ),
            alternative(
                semantic,
                1,
                MachineAlternativeApplicability::OptimizationSelected {
                    optimization:
                        optimization_core::Optimization::X86SelectXorZeroI64MaterializationV1,
                },
                MachineSizeKnowledge::ExactBytes(3),
            ),
        ],
        _ => vec![alternative(
            semantic,
            0,
            MachineAlternativeApplicability::Always,
            size(semantic),
        )],
    };
    MachineEffectDeclaration {
        semantic,
        constraint: keys
            .for_semantic(semantic)
            .expect("required x86-64 machine semantic has a constraint"),
        memory: if matches!(semantic, MachineSemanticKind::AtomicAccess(_)) {
            MachineMemoryEffect::AtomicPointerV1
        } else {
            MachineMemoryEffect::NoneV1
        },
        // A Trapping form may stop the process at its inline UD2, which is
        // as observable as a hosted effect: it is ordered like one and is
        // never dead. An atomic access dereferences a pointer the cell owns.
        trap: if matches!(semantic, MachineSemanticKind::TrappingInteger(_)) {
            MachineTrapBehavior::TrappingIntegerV1
        } else if matches!(semantic, MachineSemanticKind::AtomicAccess(_)) {
            MachineTrapBehavior::MayArchitecturalFaultV1
        } else {
            MachineTrapBehavior::NeverV1
        },
        barrier: if matches!(
            semantic,
            MachineSemanticKind::ConditionalBranchNonZero
                | MachineSemanticKind::ConditionalBranchU64LessThan
                | MachineSemanticKind::ConditionalBranchI64LessThan
                | MachineSemanticKind::ReturnScalar
                | MachineSemanticKind::ReturnAggregate
                | MachineSemanticKind::Jump
                | MachineSemanticKind::ReturnUnit
        ) {
            MachineBarrier::ControlFlow
        } else if matches!(
            semantic,
            MachineSemanticKind::TrappingInteger(_) | MachineSemanticKind::AtomicAccess(_)
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
fn binary_alias_ladder(semantic: MachineSemanticKind, sizes: [u16; 4]) -> Vec<MachineAlternative> {
    vec![
        alternative(
            semantic,
            0,
            MachineAlternativeApplicability::ResultAliasesOperands {
                result: 2,
                left: 0,
                right: 1,
            },
            MachineSizeKnowledge::ExactBytes(sizes[0]),
        ),
        alternative(
            semantic,
            1,
            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                result: 2,
                aliased_operand: 0,
                distinct_operand: 1,
            },
            MachineSizeKnowledge::ExactBytes(sizes[1]),
        ),
        alternative(
            semantic,
            2,
            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                result: 2,
                aliased_operand: 1,
                distinct_operand: 0,
            },
            MachineSizeKnowledge::ExactBytes(sizes[2]),
        ),
        alternative(
            semantic,
            3,
            MachineAlternativeApplicability::ResultDistinctFromOperands {
                result: 2,
                left: 0,
                right: 1,
            },
            MachineSizeKnowledge::ExactBytes(sizes[3]),
        ),
    ]
}

fn alternative(
    semantic: MachineSemanticKind,
    variant: u32,
    applicability: MachineAlternativeApplicability,
    size: MachineSizeKnowledge,
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

fn encoded_effects(semantic: MachineSemanticKind, variant: u32) -> MachineEncodedEffects {
    let physical = crate::x86_64_physical_register_model_ref();
    let units = |name: &str| {
        physical
            .view_named(name)
            .unwrap_or_else(|| panic!("canonical x86-64 model declares {name}"))
            .units
            .clone()
    };
    if semantic == MachineSemanticKind::Crash {
        return selected_instructions::MachineEncodedEffects::crash_v1(&units("rip"));
    }
    if let MachineSemanticKind::TrappingInteger(form) = semantic {
        return TrappingShape::of(form).encoded_effects();
    }
    if let MachineSemanticKind::NearestIeeeFloatArithmetic(form) = semantic {
        return crate::selected_form_encoding::ieee_float_arithmetic::effects(form);
    }
    let view = |name: &str| {
        physical
            .view_named(name)
            .unwrap_or_else(|| panic!("canonical x86-64 model declares {name}"))
            .id
    };
    let (reads, writes) = match semantic {
        MachineSemanticKind::PortRegisterRead
        | MachineSemanticKind::PortRegisterWrite
        | MachineSemanticKind::ModelSpecificRead
        | MachineSemanticKind::ModelSpecificWrite => {
            return crate::selected_form_encoding::register_access::effects(
                crate::selected_form_encoding::register_access::operation(semantic)
                    .expect("x86 register semantic"),
            );
        }
        MachineSemanticKind::SystemRegisterRead | MachineSemanticKind::SystemRegisterWrite => {
            unreachable!("AArch64 system-register semantics have no x86 constraint")
        }
        MachineSemanticKind::PlacedScalarAccess(form) => {
            if form.write {
                (vec![0, 1], vec![])
            } else {
                (vec![0], vec![1])
            }
        }
        MachineSemanticKind::PortWrite => {
            return crate::selected_form_encoding::port_write::effects();
        }
        MachineSemanticKind::AtomicFence => return MachineEncodedEffects::atomic_fence_v1(),
        MachineSemanticKind::FloatResultPolicy(form) => {
            return crate::selected_form_encoding::float_result_policy::effects(form);
        }
        // LOCK-prefixed atomic forms read their address plus operation
        // inputs and write the observed-prior output alone.
        MachineSemanticKind::AtomicAccess(form) => (
            (0..form.operation.input_count()).collect(),
            form.observed_operand().into_iter().collect(),
        ),
        MachineSemanticKind::NearestIeeeFloatArithmetic(_) => {
            unreachable!("float arithmetic effects returned above")
        }
        MachineSemanticKind::Crash => (Vec::new(), Vec::new()),
        MachineSemanticKind::CompareI64Zero => (vec![0], vec![]),
        MachineSemanticKind::CompareI64Immediate => (vec![0], vec![]),
        MachineSemanticKind::CompareI64 => (vec![0, 1], vec![]),
        MachineSemanticKind::MaterializeI64 => (vec![], vec![0]),
        MachineSemanticKind::ImmutableImageAddress => (vec![], vec![0]),
        MachineSemanticKind::MaterializeBooleanEqual
        | MachineSemanticKind::MaterializeBooleanU64LessThan
        | MachineSemanticKind::MaterializeBooleanI64LessThan
        | MachineSemanticKind::MaterializeBooleanU64LessOrEqual
        | MachineSemanticKind::MaterializeBooleanI64LessOrEqual => (vec![], vec![0]),
        MachineSemanticKind::Float32ToBits
        | MachineSemanticKind::Float64ToBits
        | MachineSemanticKind::BitsToFloat32
        | MachineSemanticKind::BitsToFloat64
        | MachineSemanticKind::CopyI64
        | MachineSemanticKind::ZeroExtendU8
        | MachineSemanticKind::ZeroExtendU16
        | MachineSemanticKind::SignExtendI8
        | MachineSemanticKind::SignExtendI16
        | MachineSemanticKind::SignExtendI32
        | MachineSemanticKind::ZeroExtendU32 => (vec![0], vec![1]),
        MachineSemanticKind::ExactDivideU64 => (vec![0, 1, 3], vec![2]),
        MachineSemanticKind::WrappingRemainderI64
        | MachineSemanticKind::ExactRemainderU64
        | MachineSemanticKind::WrappingDivideI64
        | MachineSemanticKind::ExactDivideI64
        | MachineSemanticKind::ExactRemainderI64 => (vec![0, 1], vec![2, 3]),
        MachineSemanticKind::SaturatingAdd(carrier)
        | MachineSemanticKind::SaturatingSubtract(carrier)
        | MachineSemanticKind::SaturatingDivide(carrier)
        | MachineSemanticKind::SaturatingRemainder(carrier)
        | MachineSemanticKind::SaturatingMultiply(carrier) => {
            saturating_form(semantic, carrier).operand_reads_and_writes()
        }
        MachineSemanticKind::BitwiseAndI64
        | MachineSemanticKind::BitwiseOrI64
        | MachineSemanticKind::BitwiseXorI64
        | MachineSemanticKind::ByteViewAddress
        | MachineSemanticKind::WrappingAddI64
        | MachineSemanticKind::ExactAddI64 => (vec![0, 1], vec![2]),
        MachineSemanticKind::BitwiseNotI64 => (vec![0], vec![1]),
        MachineSemanticKind::ExactAddI64Immediate
        | MachineSemanticKind::ExactSubtractI64Immediate => (vec![0], vec![1]),
        MachineSemanticKind::ExactSubtractI64 | MachineSemanticKind::WrappingSubtractI64
            if variant == 0 =>
        {
            (vec![], vec![2])
        }
        MachineSemanticKind::ExactSubtractI64 | MachineSemanticKind::WrappingSubtractI64 => {
            (vec![0, 1], vec![2])
        }
        // `imul r, r` genuinely reads its input to square it, so even the
        // fully-aliased variant 0 reads both external operands.
        MachineSemanticKind::ExactMultiplyI64 | MachineSemanticKind::WrappingMultiplyI64 => {
            (vec![0, 1], vec![2])
        }
        MachineSemanticKind::WrappingShiftLeftI64
        | MachineSemanticKind::WrappingShiftRightI64
        | MachineSemanticKind::WrappingShiftRightU64
        | MachineSemanticKind::ExactShiftLeftI64
        | MachineSemanticKind::ExactShiftRightI64
        | MachineSemanticKind::ExactShiftRightU64 => (vec![0, 1], vec![2]),
        MachineSemanticKind::ConditionalBranchNonZero
        | MachineSemanticKind::ConditionalBranchU64LessThan
        | MachineSemanticKind::ConditionalBranchI64LessThan
        | MachineSemanticKind::ReturnScalar
        | MachineSemanticKind::ReturnAggregate
        | MachineSemanticKind::Jump
        | MachineSemanticKind::ReturnUnit => (vec![], vec![]),
        MachineSemanticKind::CallScalar
        | MachineSemanticKind::CallAggregate
        | MachineSemanticKind::NormalizedForeignCall
        | MachineSemanticKind::Load8
        | MachineSemanticKind::Load16
        | MachineSemanticKind::LoadPacked3
        | MachineSemanticKind::LoadPacked5
        | MachineSemanticKind::LoadPacked6
        | MachineSemanticKind::LoadPacked7
        | MachineSemanticKind::StorePacked
        | MachineSemanticKind::Load32
        | MachineSemanticKind::Load64
        | MachineSemanticKind::Store
        | MachineSemanticKind::AddressOffset
        | MachineSemanticKind::Load8Indexed
        | MachineSemanticKind::CopyBytes
        | MachineSemanticKind::ZeroBytes
        | MachineSemanticKind::ScanByteSequence
        | MachineSemanticKind::LoadEncodedScalar
        | MachineSemanticKind::StoreEncodedScalar
        | MachineSemanticKind::CompareByteSequences
        | MachineSemanticKind::Store64
        | MachineSemanticKind::FrameAddress
        | MachineSemanticKind::HostedExitProcessI32
        | MachineSemanticKind::HostedReadByte
        | MachineSemanticKind::SaveFloatingControl
        | MachineSemanticKind::RestoreFloatingControl
        | MachineSemanticKind::HostedWriteByteI32
        | MachineSemanticKind::CallUnit => {
            unreachable!("scalar calls use their dedicated declaration")
        }
        MachineSemanticKind::TrappingInteger(_) => {
            unreachable!("trapping forms return their shared encoded effects above")
        }
    };
    let (implicit_uses, implicit_defs, implicit_clobbers, memory, stack, trap, control) =
        match semantic {
            MachineSemanticKind::MaterializeBooleanEqual
            | MachineSemanticKind::MaterializeBooleanU64LessThan
            | MachineSemanticKind::MaterializeBooleanI64LessThan
            | MachineSemanticKind::MaterializeBooleanU64LessOrEqual
            | MachineSemanticKind::MaterializeBooleanI64LessOrEqual => (
                units("rflags"),
                vec![],
                vec![],
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::NeverV1,
                MachineEncodedControlEffect::FallThroughV1,
            ),
            MachineSemanticKind::CompareI64Zero
            | MachineSemanticKind::CompareI64
            | MachineSemanticKind::CompareI64Immediate => (
                vec![],
                units("rflags"),
                vec![],
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::NeverV1,
                MachineEncodedControlEffect::FallThroughV1,
            ),
            MachineSemanticKind::ExactDivideU64 | MachineSemanticKind::SaturatingDivide(_) => (
                vec![],
                vec![],
                {
                    let mut clobbers = units("rdx");
                    clobbers.extend(units("rflags"));
                    clobbers.sort_unstable();
                    clobbers.dedup();
                    clobbers
                },
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                MachineEncodedControlEffect::FallThroughV1,
            ),
            MachineSemanticKind::WrappingRemainderI64
            | MachineSemanticKind::ExactRemainderU64
            | MachineSemanticKind::WrappingDivideI64
            | MachineSemanticKind::ExactDivideI64
            | MachineSemanticKind::ExactRemainderI64
            | MachineSemanticKind::SaturatingRemainder(_) => (
                vec![],
                vec![],
                units("rflags"),
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                MachineEncodedControlEffect::FallThroughV1,
            ),
            MachineSemanticKind::BitwiseAndI64
            | MachineSemanticKind::BitwiseOrI64
            | MachineSemanticKind::BitwiseXorI64
            | MachineSemanticKind::SaturatingAdd(_)
            | MachineSemanticKind::SaturatingSubtract(_)
            | MachineSemanticKind::SaturatingMultiply(_)
            | MachineSemanticKind::ExactMultiplyI64
            | MachineSemanticKind::WrappingMultiplyI64
            | MachineSemanticKind::WrappingShiftLeftI64
            | MachineSemanticKind::WrappingShiftRightI64
            | MachineSemanticKind::WrappingShiftRightU64
            | MachineSemanticKind::ExactShiftLeftI64
            | MachineSemanticKind::ExactShiftRightI64
            | MachineSemanticKind::ExactShiftRightU64
            | MachineSemanticKind::ExactSubtractI64
            | MachineSemanticKind::WrappingSubtractI64 => (
                vec![],
                vec![],
                units("rflags"),
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::NeverV1,
                MachineEncodedControlEffect::FallThroughV1,
            ),
            MachineSemanticKind::Jump => (
                units("rip"),
                units("rip"),
                vec![],
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
            ),
            MachineSemanticKind::ConditionalBranchNonZero
            | MachineSemanticKind::ConditionalBranchU64LessThan
            | MachineSemanticKind::ConditionalBranchI64LessThan => {
                let mut uses = units("rflags");
                uses.extend(units("rip"));
                uses.sort_unstable();
                uses.dedup();
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
            MachineSemanticKind::ReturnScalar
            | MachineSemanticKind::ReturnAggregate
            | MachineSemanticKind::ReturnUnit => {
                let stack_pointer = view("rsp");
                let mut defs = units("rsp");
                defs.extend(units("rip"));
                defs.sort_unstable();
                defs.dedup();
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
            MachineSemanticKind::AtomicAccess(form) => {
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
            MachineSemanticKind::MaterializeI64 if variant == 1 => (
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
fn saturating_form(semantic: MachineSemanticKind, carrier: SaturatingCarrier) -> SaturatingForm {
    let operation = match semantic {
        MachineSemanticKind::SaturatingAdd(_) => SaturatingOperation::Add,
        MachineSemanticKind::SaturatingSubtract(_) => SaturatingOperation::Subtract,
        MachineSemanticKind::SaturatingDivide(_) => SaturatingOperation::Divide,
        MachineSemanticKind::SaturatingRemainder(_) => SaturatingOperation::Remainder,
        MachineSemanticKind::SaturatingMultiply(_) => SaturatingOperation::Multiply,
        _ => unreachable!("saturating semantics carry their operation"),
    };
    SaturatingForm::of(operation, carrier)
}

fn size(semantic: MachineSemanticKind) -> MachineSizeKnowledge {
    match semantic {
        MachineSemanticKind::PortRegisterRead
        | MachineSemanticKind::PortRegisterWrite
        | MachineSemanticKind::ModelSpecificRead
        | MachineSemanticKind::ModelSpecificWrite => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 1,
            maximum_bytes: Some(40),
        },
        MachineSemanticKind::SystemRegisterRead | MachineSemanticKind::SystemRegisterWrite => {
            unreachable!("AArch64 system-register semantics have no x86 constraint")
        }
        MachineSemanticKind::PlacedScalarAccess(_) => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 7,
            maximum_bytes: Some(9),
        },
        MachineSemanticKind::PortWrite => {
            MachineSizeKnowledge::ExactBytes(x86_encoding::IMMEDIATE_PORT_WRITE_WIDTH as u16)
        }
        MachineSemanticKind::AtomicFence => MachineSizeKnowledge::ExactBytes(3),
        MachineSemanticKind::FloatResultPolicy(form) => {
            crate::selected_form_encoding::float_result_policy::size(form)
        }
        MachineSemanticKind::AtomicAccess(_) => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 1,
            maximum_bytes: None,
        },
        MachineSemanticKind::NearestIeeeFloatArithmetic(_) => {
            unreachable!("float arithmetic uses its dedicated declaration")
        }
        MachineSemanticKind::Crash => MachineSizeKnowledge::ExactBytes(2),
        MachineSemanticKind::MaterializeBooleanEqual
        | MachineSemanticKind::MaterializeBooleanU64LessThan
        | MachineSemanticKind::MaterializeBooleanI64LessThan
        | MachineSemanticKind::MaterializeBooleanU64LessOrEqual
        | MachineSemanticKind::MaterializeBooleanI64LessOrEqual => {
            MachineSizeKnowledge::ExactBytes(8)
        }
        MachineSemanticKind::Jump => MachineSizeKnowledge::ExactBytes(5),
        MachineSemanticKind::CompareI64Zero
        | MachineSemanticKind::CompareI64
        | MachineSemanticKind::CopyI64 => MachineSizeKnowledge::ExactBytes(3),
        MachineSemanticKind::CompareI64Immediate => MachineSizeKnowledge::ExactBytes(7),
        MachineSemanticKind::Float32ToBits | MachineSemanticKind::BitsToFloat32 => {
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(5),
            }
        }
        MachineSemanticKind::Float64ToBits | MachineSemanticKind::BitsToFloat64 => {
            MachineSizeKnowledge::ExactBytes(5)
        }
        MachineSemanticKind::ZeroExtendU8
        | MachineSemanticKind::ZeroExtendU16
        | MachineSemanticKind::SignExtendI8
        | MachineSemanticKind::SignExtendI16 => MachineSizeKnowledge::ExactBytes(4),
        MachineSemanticKind::ZeroExtendU32 | MachineSemanticKind::SignExtendI32 => {
            MachineSizeKnowledge::ExactBytes(3)
        }
        MachineSemanticKind::MaterializeI64 => MachineSizeKnowledge::ExactBytes(10),
        // `lea r64, [rip+disp32]` with the disp32 left for object construction.
        MachineSemanticKind::ImmutableImageAddress => MachineSizeKnowledge::ExactBytes(7),
        MachineSemanticKind::ExactDivideU64 => MachineSizeKnowledge::ExactBytes(3),
        MachineSemanticKind::WrappingRemainderI64 => MachineSizeKnowledge::ExactBytes(17),
        // `xor` the RDX high half, unsigned `div`, then move the remainder
        // from RDX into the RAX result home.
        MachineSemanticKind::ExactRemainderU64 => MachineSizeKnowledge::ExactBytes(9),
        // `cmp` the divisor against -1, `jne` to the divide, `neg` the
        // dividend for the wrapping MIN / -1 answer, `jmp` over `cqo; idiv`.
        MachineSemanticKind::WrappingDivideI64 => MachineSizeKnowledge::ExactBytes(16),
        // Bare `cqo; idiv` / `cqo; idiv; mov rdx, rax`: the proven exact
        // obligations exclude every faulting input the guard exists for.
        MachineSemanticKind::ExactDivideI64 => MachineSizeKnowledge::ExactBytes(5),
        MachineSemanticKind::ExactRemainderI64 => MachineSizeKnowledge::ExactBytes(8),
        MachineSemanticKind::SaturatingAdd(carrier)
        | MachineSemanticKind::SaturatingSubtract(carrier)
        | MachineSemanticKind::SaturatingDivide(carrier)
        | MachineSemanticKind::SaturatingRemainder(carrier)
        | MachineSemanticKind::SaturatingMultiply(carrier) => {
            MachineSizeKnowledge::ExactBytes(saturating_form(semantic, carrier).byte_count())
        }
        // Every instruction of a Trapping realization has a register-free
        // length, so each form has one exact size.
        MachineSemanticKind::TrappingInteger(form) => {
            MachineSizeKnowledge::ExactBytes(TrappingShape::of(form).byte_count())
        }
        MachineSemanticKind::BitwiseAndI64
        | MachineSemanticKind::BitwiseOrI64
        | MachineSemanticKind::BitwiseNotI64
        | MachineSemanticKind::BitwiseXorI64 => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 3,
            maximum_bytes: Some(6),
        },
        MachineSemanticKind::ByteViewAddress
        | MachineSemanticKind::WrappingAddI64
        | MachineSemanticKind::ExactAddI64 => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 4,
            maximum_bytes: Some(5),
        },
        MachineSemanticKind::ExactAddI64Immediate => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 4,
            maximum_bytes: Some(8),
        },
        MachineSemanticKind::ExactSubtractI64Immediate => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 4,
            maximum_bytes: Some(8),
        },
        MachineSemanticKind::ConditionalBranchNonZero
        | MachineSemanticKind::ConditionalBranchU64LessThan
        | MachineSemanticKind::ConditionalBranchI64LessThan => {
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 2,
                maximum_bytes: Some(6),
            }
        }
        MachineSemanticKind::ReturnScalar
        | MachineSemanticKind::ReturnAggregate
        | MachineSemanticKind::ReturnUnit => MachineSizeKnowledge::ExactBytes(1),
        MachineSemanticKind::ExactSubtractI64
        | MachineSemanticKind::WrappingSubtractI64
        | MachineSemanticKind::ExactMultiplyI64
        | MachineSemanticKind::WrappingMultiplyI64
        | MachineSemanticKind::WrappingShiftLeftI64
        | MachineSemanticKind::WrappingShiftRightI64
        | MachineSemanticKind::WrappingShiftRightU64
        | MachineSemanticKind::ExactShiftLeftI64
        | MachineSemanticKind::ExactShiftRightI64
        | MachineSemanticKind::ExactShiftRightU64 => {
            unreachable!("subtraction, multiplication, and shifts declare explicit alternatives")
        }
        MachineSemanticKind::CallScalar
        | MachineSemanticKind::CallAggregate
        | MachineSemanticKind::NormalizedForeignCall
        | MachineSemanticKind::Load8
        | MachineSemanticKind::Load16
        | MachineSemanticKind::LoadPacked3
        | MachineSemanticKind::LoadPacked5
        | MachineSemanticKind::LoadPacked6
        | MachineSemanticKind::LoadPacked7
        | MachineSemanticKind::StorePacked
        | MachineSemanticKind::Load32
        | MachineSemanticKind::Load64
        | MachineSemanticKind::Store
        | MachineSemanticKind::AddressOffset
        | MachineSemanticKind::Load8Indexed
        | MachineSemanticKind::CopyBytes
        | MachineSemanticKind::ZeroBytes
        | MachineSemanticKind::ScanByteSequence
        | MachineSemanticKind::LoadEncodedScalar
        | MachineSemanticKind::StoreEncodedScalar
        | MachineSemanticKind::CompareByteSequences
        | MachineSemanticKind::Store64
        | MachineSemanticKind::FrameAddress
        | MachineSemanticKind::HostedExitProcessI32
        | MachineSemanticKind::HostedReadByte
        | MachineSemanticKind::SaveFloatingControl
        | MachineSemanticKind::RestoreFloatingControl
        | MachineSemanticKind::HostedWriteByteI32
        | MachineSemanticKind::CallUnit => {
            unreachable!("scalar calls use their dedicated declaration")
        }
    }
}

#[cfg(test)]
mod tests;

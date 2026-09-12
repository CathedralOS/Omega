use register_model::ValidatedRegisterConstraintCatalog;
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey, MachineBarrier,
    MachineCallEffect, MachineCleanupEffect, MachineEffectCatalog,
    MachineEffectCatalogValidationError, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior, SelectedConstraintKeys,
    ValidatedMachineEffectCatalog, validate_machine_effect_catalog,
};
use target::{Architecture, NativeTarget, ObjectFormat};

mod memory;
mod scalar_call;

use scalar_call::declaration as scalar_call_declaration;

use crate::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64, X86_64_COMPARE_I64_ZERO,
    X86_64_CONDITIONAL_BRANCH, X86_64_COPY_I64, X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_RETURN,
    X86_64_MICROSOFT_RETURN_UNIT, X86_64_SUBTRACT_I64, X86_64_SUBTRACT_I64_IMMEDIATE,
    X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT, x86_64_system_v_register_call_keys,
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
                        crate::selected_form_encoding::hosted_read_byte::declaration(constraint),
                    );
                }
                if semantic == MachineSemanticKind::HostedWriteByteI32 {
                    return Ok(
                        crate::selected_form_encoding::hosted_write_byte::declaration(constraint),
                    );
                }
                Ok(
                    if matches!(
                        semantic,
                        MachineSemanticKind::Load8
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
    if target.architecture != Architecture::X86_64
        || constraints.architecture() != Architecture::X86_64
    {
        return Err(X86_64MachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    let canonical = x86_64_machine_effect_catalog(target, constraints)?;
    let validated = validate_machine_effect_catalog(constraints, catalog)
        .map_err(X86_64MachineEffectCatalogValidationError::Structural)?;
    if validated.catalog() != &canonical {
        return Err(X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch);
    }
    Ok(validated)
}

fn selected_keys(
    target: NativeTarget,
) -> Result<SelectedConstraintKeys, X86_64MachineEffectCatalogValidationError> {
    let return_i64 = match target.object_format {
        ObjectFormat::Elf => X86_64_SYSTEM_V_RETURN,
        ObjectFormat::Coff => X86_64_MICROSOFT_RETURN,
        ObjectFormat::MachO => {
            return Err(X86_64MachineEffectCatalogValidationError::UnsupportedTargetAbi);
        }
    };
    let return_unit = match target.object_format {
        ObjectFormat::Elf => X86_64_SYSTEM_V_RETURN_UNIT,
        ObjectFormat::Coff => X86_64_MICROSOFT_RETURN_UNIT,
        ObjectFormat::MachO => {
            return Err(X86_64MachineEffectCatalogValidationError::UnsupportedTargetAbi);
        }
    };
    Ok(SelectedConstraintKeys {
        load_packed: Some(crate::X86_64_LOAD_PACKED),
        store_packed: Some(crate::X86_64_STORE_PACKED),
        call_aggregate: if target.object_format == ObjectFormat::Elf {
            crate::register_model::x86_64_system_v_aggregate_call_keys()
                .into_iter()
                .chain(crate::x86_64_system_v_mixed_aggregate_call_keys())
                .chain(crate::x86_64_indirect_aggregate_call_keys(false))
                .collect()
        } else {
            crate::register_model::x86_64_microsoft_aggregate_call_keys()
                .into_iter()
                .chain(crate::x86_64_microsoft_mixed_aggregate_call_keys())
                .chain(crate::x86_64_indirect_aggregate_call_keys(true))
                .collect()
        },
        return_aggregate: if target.object_format == ObjectFormat::Elf {
            crate::register_model::x86_64_system_v_aggregate_return_keys()
        } else {
            crate::register_model::x86_64_microsoft_aggregate_return_keys()
        },
        hosted_read_byte: (target == NativeTarget::linux_x64())
            .then_some(crate::X86_64_HOSTED_READ_BYTE),
        hosted_exit_process_i32: (target == NativeTarget::linux_x64())
            .then_some(crate::X86_64_HOSTED_EXIT_PROCESS_I32),
        hosted_write_byte_i32: (target.object_format == ObjectFormat::Elf)
            .then_some(crate::X86_64_HOSTED_WRITE_BYTE_I32),
        load64: Some(crate::X86_64_LOAD64),
        load8: Some(crate::X86_64_LOAD8),
        load16: Some(crate::X86_64_LOAD16),
        load32: Some(crate::X86_64_LOAD32),
        load8_indexed: Some(crate::X86_64_LOAD8_INDEXED),
        store: Some(crate::X86_64_STORE),
        address_offset: Some(crate::X86_64_ADDRESS_OFFSET),
        store64: Some(crate::X86_64_STORE64),
        frame_address: Some(crate::X86_64_FRAME_ADDRESS),
        call_unit_mixed: if target.object_format == ObjectFormat::Elf {
            crate::x86_64_system_v_mixed_unit_call_keys()
        } else {
            crate::x86_64_microsoft_mixed_unit_call_keys()
        },
        call_unit: if target.object_format == ObjectFormat::Elf {
            crate::x86_64_system_v_register_unit_call_keys()
        } else {
            crate::x86_64_microsoft_register_unit_call_keys()
        },
        call_scalar: (if matches!(target.object_format, ObjectFormat::Elf) {
            x86_64_system_v_register_call_keys()
        } else {
            crate::x86_64_microsoft_register_call_keys()
        })
        .into_iter()
        .chain(crate::x86_64_float_scalar_call_keys(
            target.object_format == ObjectFormat::Coff,
        ))
        .collect(),
        materialize_i64: X86_64_MATERIALIZE_I64,
        materialize_boolean: crate::X86_64_MATERIALIZE_BOOLEAN,
        copy_i64: X86_64_COPY_I64,
        float32_to_bits: Some(crate::X86_64_FLOAT32_TO_BITS),
        float64_to_bits: Some(crate::X86_64_FLOAT64_TO_BITS),
        bits_to_float32: Some(crate::X86_64_BITS_TO_FLOAT32),
        bits_to_float64: Some(crate::X86_64_BITS_TO_FLOAT64),
        add_i64: X86_64_ADD_I64,
        subtract_i64: X86_64_SUBTRACT_I64,
        add_i64_immediate: X86_64_ADD_I64_IMMEDIATE,
        subtract_i64_immediate: X86_64_SUBTRACT_I64_IMMEDIATE,
        compare_i64_zero: X86_64_COMPARE_I64_ZERO,
        compare_i64: X86_64_COMPARE_I64,
        conditional_branch: X86_64_CONDITIONAL_BRANCH,
        jump: crate::X86_64_JUMP,
        return_float: crate::x86_64_float_scalar_return_keys(
            target.object_format == ObjectFormat::Coff,
        ),
        return_i64,
        return_unit,
    })
}

fn declaration(
    semantic: MachineSemanticKind,
    keys: &SelectedConstraintKeys,
) -> MachineEffectDeclaration {
    let alternatives = match semantic {
        MachineSemanticKind::BitwiseAndI64
        | MachineSemanticKind::BitwiseXorI64
        | MachineSemanticKind::ByteViewAddress
        | MachineSemanticKind::ExactAddI64 => {
            vec![alternative(
                semantic,
                0,
                MachineAlternativeApplicability::Always,
                size(semantic),
            )]
        }
        MachineSemanticKind::ExactSubtractI64 => vec![
            alternative(
                semantic,
                0,
                MachineAlternativeApplicability::ResultAliasesOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                },
                MachineSizeKnowledge::ExactBytes(3),
            ),
            alternative(
                semantic,
                1,
                MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: 2,
                    aliased_operand: 0,
                    distinct_operand: 1,
                },
                MachineSizeKnowledge::ExactBytes(3),
            ),
            alternative(
                semantic,
                2,
                MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: 2,
                    aliased_operand: 1,
                    distinct_operand: 0,
                },
                MachineSizeKnowledge::ExactBytes(6),
            ),
            alternative(
                semantic,
                3,
                MachineAlternativeApplicability::ResultDistinctFromOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                },
                MachineSizeKnowledge::ExactBytes(6),
            ),
        ],
        MachineSemanticKind::ConditionalBranchU64LessThan
        | MachineSemanticKind::ConditionalBranchI64LessThan => vec![alternative(
            semantic,
            0,
            MachineAlternativeApplicability::Always,
            MachineSizeKnowledge::ExactBytes(6),
        )],
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
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::NeverV1,
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
        } else {
            MachineBarrier::None
        },
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives,
    }
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
    let physical = crate::x86_64_physical_register_model();
    let units = |name: &str| {
        physical
            .view_named(name)
            .unwrap_or_else(|| panic!("canonical x86-64 model declares {name}"))
            .units
            .clone()
    };
    let view = |name: &str| {
        physical
            .view_named(name)
            .unwrap_or_else(|| panic!("canonical x86-64 model declares {name}"))
            .id
    };
    let (reads, writes) = match semantic {
        MachineSemanticKind::CompareI64Zero => (vec![0], vec![]),
        MachineSemanticKind::CompareI64 => (vec![0, 1], vec![]),
        MachineSemanticKind::MaterializeI64 => (vec![], vec![0]),
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
        MachineSemanticKind::BitwiseAndI64
        | MachineSemanticKind::BitwiseXorI64
        | MachineSemanticKind::ByteViewAddress
        | MachineSemanticKind::ExactAddI64 => (vec![0, 1], vec![2]),
        MachineSemanticKind::ExactAddI64Immediate
        | MachineSemanticKind::ExactSubtractI64Immediate => (vec![0], vec![1]),
        MachineSemanticKind::ExactSubtractI64 if variant == 0 => (vec![], vec![2]),
        MachineSemanticKind::ExactSubtractI64 => (vec![0, 1], vec![2]),
        MachineSemanticKind::ConditionalBranchNonZero
        | MachineSemanticKind::ConditionalBranchU64LessThan
        | MachineSemanticKind::ConditionalBranchI64LessThan
        | MachineSemanticKind::ReturnScalar
        | MachineSemanticKind::ReturnAggregate
        | MachineSemanticKind::Jump
        | MachineSemanticKind::ReturnUnit => (vec![], vec![]),
        MachineSemanticKind::CallScalar
        | MachineSemanticKind::CallAggregate
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
        | MachineSemanticKind::Store64
        | MachineSemanticKind::FrameAddress
        | MachineSemanticKind::HostedExitProcessI32
        | MachineSemanticKind::HostedReadByte
        | MachineSemanticKind::HostedWriteByteI32
        | MachineSemanticKind::CallUnit => {
            unreachable!("scalar calls use their dedicated declaration")
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
            MachineSemanticKind::CompareI64Zero | MachineSemanticKind::CompareI64 => (
                vec![],
                units("rflags"),
                vec![],
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::NeverV1,
                MachineEncodedControlEffect::FallThroughV1,
            ),
            MachineSemanticKind::BitwiseAndI64
            | MachineSemanticKind::BitwiseXorI64
            | MachineSemanticKind::ExactSubtractI64 => (
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

fn size(semantic: MachineSemanticKind) -> MachineSizeKnowledge {
    match semantic {
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
        MachineSemanticKind::BitwiseAndI64 | MachineSemanticKind::BitwiseXorI64 => {
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 3,
                maximum_bytes: Some(6),
            }
        }
        MachineSemanticKind::ByteViewAddress | MachineSemanticKind::ExactAddI64 => {
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(5),
            }
        }
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
        MachineSemanticKind::ExactSubtractI64 => {
            unreachable!("subtraction declares alias-dependent alternatives")
        }
        MachineSemanticKind::CallScalar
        | MachineSemanticKind::CallAggregate
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
        | MachineSemanticKind::Store64
        | MachineSemanticKind::FrameAddress
        | MachineSemanticKind::HostedExitProcessI32
        | MachineSemanticKind::HostedReadByte
        | MachineSemanticKind::HostedWriteByteI32
        | MachineSemanticKind::CallUnit => {
            unreachable!("scalar calls use their dedicated declaration")
        }
    }
}

#[cfg(test)]
mod tests {
    use register_model::validate_physical_register_model;

    use super::*;
    use crate::{
        X86_64RegisterConstraintCatalogValidationError,
        validate_x86_64_register_constraint_catalog, x86_64_physical_register_model,
        x86_64_register_constraint_catalog,
    };

    fn constraints() -> ValidatedRegisterConstraintCatalog {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        validate_x86_64_register_constraint_catalog(
            x86_64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap_or_else(|error: X86_64RegisterConstraintCatalogValidationError| panic!("{error}"))
    }

    #[test]
    fn integer_normalization_catalog_binds_identity_size_and_effects() {
        for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
            let constraints = constraints();
            let catalog = x86_64_machine_effect_catalog(target, &constraints).unwrap();
            validate_x86_64_machine_effect_catalog(target, &constraints, catalog.clone()).unwrap();
            for semantic in [
                MachineSemanticKind::ZeroExtendU16,
                MachineSemanticKind::SignExtendI8,
                MachineSemanticKind::SignExtendI16,
                MachineSemanticKind::SignExtendI32,
            ] {
                let declaration = catalog
                    .declarations
                    .iter()
                    .find(|row| row.semantic == semantic)
                    .unwrap();
                assert_eq!(declaration.constraint, X86_64_COPY_I64);
                assert_eq!(declaration.alternatives.len(), 1);
                let alternative = &declaration.alternatives[0];
                assert_eq!(alternative.key.family, semantic.into());
                assert_eq!(
                    alternative.size,
                    MachineSizeKnowledge::ExactBytes(
                        if semantic == MachineSemanticKind::SignExtendI32 {
                            3
                        } else {
                            4
                        }
                    )
                );
                assert_eq!(
                    alternative.encoded,
                    MachineEncodedEffects::fallthrough_v1(vec![0], vec![1])
                );
                for corruption in 0..3 {
                    let mut changed = catalog.clone();
                    let row = changed
                        .declarations
                        .iter_mut()
                        .find(|row| row.semantic == semantic)
                        .unwrap();
                    match corruption {
                        0 => row.alternatives[0].key.family = MachineSemanticKind::CopyI64.into(),
                        1 => row.alternatives[0].size = MachineSizeKnowledge::ExactBytes(1),
                        _ => row.alternatives[0].encoded.external_operand_reads.clear(),
                    }
                    assert!(
                        validate_x86_64_machine_effect_catalog(target, &constraints, changed)
                            .is_err()
                    );
                }
            }
        }
    }

    #[test]
    fn catalog_declares_alias_safe_subtraction_and_control_barriers() {
        for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
            let constraints = constraints();
            let catalog = x86_64_machine_effect_catalog(target, &constraints).unwrap();
            let subtract = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == MachineSemanticKind::ExactSubtractI64)
                .unwrap();
            let add = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == MachineSemanticKind::ExactAddI64)
                .unwrap();
            let less_than_branch = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == MachineSemanticKind::ConditionalBranchU64LessThan)
                .unwrap();
            let signed_less_than_branch = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == MachineSemanticKind::ConditionalBranchI64LessThan)
                .unwrap();
            assert_eq!(less_than_branch.constraint, X86_64_CONDITIONAL_BRANCH);
            assert_eq!(less_than_branch.alternatives.len(), 1);
            assert_eq!(
                less_than_branch.alternatives[0].size,
                MachineSizeKnowledge::ExactBytes(6)
            );
            assert_eq!(
                less_than_branch.alternatives[0].encoded.control,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            );
            assert_eq!(
                signed_less_than_branch.constraint,
                X86_64_CONDITIONAL_BRANCH
            );
            assert_eq!(signed_less_than_branch.alternatives.len(), 1);
            assert_eq!(
                signed_less_than_branch.alternatives[0].key.family,
                selected_instructions::MachineAlternativeFamily::ConditionalBranchI64LessThan
            );
            assert_eq!(
                signed_less_than_branch.alternatives[0].size,
                MachineSizeKnowledge::ExactBytes(6)
            );
            assert_eq!(
                add.alternatives[0].applicability,
                MachineAlternativeApplicability::Always
            );
            assert_eq!(subtract.constraint, X86_64_SUBTRACT_I64);
            let register_effects = constraints
                .catalog()
                .constraints
                .iter()
                .find(|row| row.key == subtract.constraint)
                .unwrap();
            assert!(register_effects.implicit_uses.is_empty());
            assert!(register_effects.implicit_defs.is_empty());
            assert!(!register_effects.clobbers.is_empty());
            assert_eq!(subtract.alternatives.len(), 4);
            assert_eq!(
                subtract.alternatives[0].applicability,
                MachineAlternativeApplicability::ResultAliasesOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                }
            );
            assert_eq!(
                subtract.alternatives[1].applicability,
                MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: 2,
                    aliased_operand: 0,
                    distinct_operand: 1,
                }
            );
            assert_eq!(
                subtract.alternatives[2].applicability,
                MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: 2,
                    aliased_operand: 1,
                    distinct_operand: 0,
                }
            );
            assert_eq!(
                subtract.alternatives[3].applicability,
                MachineAlternativeApplicability::ResultDistinctFromOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                }
            );
            assert!(catalog.declarations.iter().all(|row| {
                row.barrier
                    == if matches!(
                        row.semantic,
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
                        row.semantic,
                        MachineSemanticKind::CallScalar
                            | MachineSemanticKind::CallUnit
                            | MachineSemanticKind::CallAggregate
                    ) {
                        MachineBarrier::Call
                    } else if matches!(
                        row.semantic,
                        MachineSemanticKind::HostedReadByte
                            | MachineSemanticKind::HostedWriteByteI32
                            | MachineSemanticKind::HostedExitProcessI32
                    ) {
                        MachineBarrier::ExternalEffect
                    } else {
                        MachineBarrier::None
                    }
            }));
            let scalar_call = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == MachineSemanticKind::CallScalar);
            {
                let scalar_call = scalar_call.expect("supported target declares scalar call");
                assert_eq!(
                    scalar_call.call,
                    MachineCallEffect::DirectInternalNormalReturnV1 {
                        pre_call_stack_alignment: 16,
                    }
                );
                assert_eq!(
                    scalar_call.alternatives[0].size,
                    MachineSizeKnowledge::ExactBytes(5)
                );
                assert!(matches!(
                    scalar_call.alternatives[0].encoded.memory,
                    MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                        byte_count: 8,
                        ..
                    }
                ));
                assert!(matches!(
                    scalar_call.alternatives[0].encoded.stack,
                    MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                        return_address_byte_count: 8,
                        ..
                    }
                ));
            }
            let return_unit = catalog
                .declarations
                .iter()
                .find(|row| row.semantic == MachineSemanticKind::ReturnUnit)
                .unwrap();
            assert!(
                constraints
                    .catalog()
                    .constraints
                    .iter()
                    .find(|row| row.key == return_unit.constraint)
                    .unwrap()
                    .operands
                    .is_empty()
            );
            assert!(validate_x86_64_machine_effect_catalog(target, &constraints, catalog).is_ok());
            let ordinary = x86_64_machine_effect_catalog(target, &constraints).unwrap();
            for semantic in [
                MachineSemanticKind::Load64,
                MachineSemanticKind::Store64,
                MachineSemanticKind::FrameAddress,
                MachineSemanticKind::CallUnit,
            ] {
                assert!(
                    ordinary
                        .declarations
                        .iter()
                        .any(|row| row.semantic == semantic)
                );
            }
            let load = ordinary
                .declarations
                .iter()
                .find(|row| row.semantic == MachineSemanticKind::Load64)
                .unwrap();
            assert_eq!(load.memory, MachineMemoryEffect::ReadPointerV1);
            assert_eq!(
                load.alternatives[0].encoded.memory,
                MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: 8
                }
            );
            assert_eq!(
                load.alternatives[0].encoded.trap,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            );
        }
    }

    #[test]
    fn semantic_validator_rejects_structural_and_target_specific_corruption() {
        let target = NativeTarget::linux_x64();
        let constraints = constraints();
        let mut missing = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        missing.declarations.pop();
        assert!(matches!(
            validate_x86_64_machine_effect_catalog(target, &constraints, missing),
            Err(X86_64MachineEffectCatalogValidationError::Structural(
                MachineEffectCatalogValidationError::DeclarationRosterMismatch
            ))
        ));

        let mut wrong_size = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        wrong_size
            .declarations
            .iter_mut()
            .find(|row| row.semantic == MachineSemanticKind::ExactSubtractI64)
            .unwrap()
            .alternatives[0]
            .size = MachineSizeKnowledge::ExactBytes(4);
        assert_eq!(
            validate_x86_64_machine_effect_catalog(target, &constraints, wrong_size),
            Err(X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch)
        );

        let mut bad_alias = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        bad_alias
            .declarations
            .iter_mut()
            .find(|row| row.semantic == MachineSemanticKind::ExactSubtractI64)
            .unwrap()
            .alternatives[0]
            .applicability = MachineAlternativeApplicability::ResultAliasesOperand {
            result: 9,
            operand: 0,
        };
        assert!(matches!(
            validate_x86_64_machine_effect_catalog(target, &constraints, bad_alias),
            Err(X86_64MachineEffectCatalogValidationError::Structural(
                MachineEffectCatalogValidationError::InvalidAlternativeApplicability(
                    MachineSemanticKind::ExactSubtractI64
                )
            ))
        ));

        let target = NativeTarget::windows_x64();
        let mut wrong_memory = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        wrong_memory
            .declarations
            .iter_mut()
            .find(|row| row.semantic == MachineSemanticKind::Store64)
            .unwrap()
            .alternatives[0]
            .encoded
            .memory = MachineEncodedMemoryEffect::NoneV1;
        assert!(
            validate_x86_64_machine_effect_catalog(target, &constraints, wrong_memory).is_err()
        );
    }
}

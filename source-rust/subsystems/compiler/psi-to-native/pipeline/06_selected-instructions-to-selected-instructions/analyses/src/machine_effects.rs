/*
  Pre-allocation machine-effect analysis and catalog admission
  (`analyze_machine_effects`, `analyze_pre_allocation_machine_effects`,
  `prepared_machine_effect_catalog`, `ValidatedPreAllocationMachineEffects`).

  Derives per-instruction register-unit uses/defs/clobbers, memory footprints,
  trap behaviors, barriers, call effects, cleanup actions, and ISA encoding
  alternatives from a `SelectedInstructionPlan` and its target environment.
*/

mod analysis {
    use crate::ValidatedPreAllocationMachineEffects;
    use crate::analyze_pre_allocation_machine_effects;

    use register_environment::ValidatedTargetRegisterEnvironment;

    use super::MachineEffectStageError;
    use super::catalog::prepared_catalog;

    pub(super) fn analyze<S: crate::SelectedProgramView>(
        selected: &S,
        environment: &ValidatedTargetRegisterEnvironment,
    ) -> Result<ValidatedPreAllocationMachineEffects, MachineEffectStageError> {
        let catalog = prepared_catalog(selected.selected_plan().target, environment.constraints())?;
        analyze_pre_allocation_machine_effects(
            selected,
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            &catalog,
        )
        .map_err(MachineEffectStageError::Analysis)
    }
}
mod catalog {
    use std::sync::Arc;

    use register_model::PreparedRegisterConstraintCatalog;
    use selected_instructions::{PreparedMachineEffectCatalog, canonical_machine_effect_catalog};
    use target::NativeTarget;
    use target_aarch64::aarch64_machine_effect_catalog;
    use target_x86_64::x86_64_machine_effect_catalog;

    use super::MachineEffectStageError;

    pub(super) fn prepared_catalog(
        target: NativeTarget,
        constraints: &PreparedRegisterConstraintCatalog,
    ) -> Result<Arc<PreparedMachineEffectCatalog>, MachineEffectStageError> {
        // The ISA owner establishes its own output. Foreign-catalog validation is
        // a separate boundary, not a second construction during ordinary compilation.
        match target.architecture {
            target::Architecture::X86_64 => {
                canonical_machine_effect_catalog(target, constraints, x86_64_machine_effect_catalog)
                    .map_err(MachineEffectStageError::X86_64Catalog)
            }
            target::Architecture::Aarch64 => canonical_machine_effect_catalog(
                target,
                constraints,
                aarch64_machine_effect_catalog,
            )
            .map_err(MachineEffectStageError::Aarch64Catalog),
        }
    }
}
mod error {
    use crate::MachineEffectError;
    use target_aarch64::Aarch64MachineEffectCatalogValidationError;
    use target_x86_64::X86_64MachineEffectCatalogValidationError;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum MachineEffectStageError {
        X86_64Catalog(X86_64MachineEffectCatalogValidationError),
        Aarch64Catalog(Aarch64MachineEffectCatalogValidationError),
        Analysis(MachineEffectError),
    }

    failed_display!(MachineEffectStageError, "machine-effect analysis failed");
}
mod facts {

    mod compute {
        use crate::SelectedProgramView;
        use register_model::{
            PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys,
            TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
            ValidatedRegisterReservationProfile, target_register_environment_identity,
        };
        use selected_instructions::{
            MachineAlternative, MachineEffectDeclaration, MachineEncodedControlEffect,
            MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
            MachineSemanticKind, PreparedMachineEffectCatalog, SelectedFunction,
            SelectedInstruction, SelectedInstructionKind,
        };

        use crate::MachineEffectError;
        use selected_instructions::{
            BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects,
            PreAllocationMachineEffectPlan, pre_allocation_machine_effect_identity,
        };

        #[allow(clippy::too_many_arguments)]
        pub(super) fn compute_terminal_pre_allocation_machine_effects<S: SelectedProgramView>(
            selected: &S,
            register_environment: TargetRegisterEnvironmentIdentity,
            physical: &ValidatedPhysicalRegisterModel,
            constraints: &PreparedRegisterConstraintCatalog,
            reservations: &ValidatedRegisterReservationProfile,
            selected_keys: &TargetRegisterEnvironmentConstraintKeys,
            catalog: &PreparedMachineEffectCatalog,
        ) -> Result<PreAllocationMachineEffectPlan, MachineEffectError> {
            let source = selected.selected_plan();
            if source.work_schedule != selected.work_schedule_identity() {
                return Err(MachineEffectError::SelectedRootMismatch);
            }
            if target_register_environment_identity(
                source.target,
                physical,
                constraints,
                reservations,
                selected_keys,
            ) != register_environment
            {
                return Err(MachineEffectError::RegisterEnvironmentMismatch);
            }
            validate_catalog_roots(source.target, constraints, selected_keys, catalog)?;
            let declarations_by_key = {
                let mut map = std::collections::BTreeMap::<
                    (MachineSemanticKind, register_model::RegisterConstraintKey),
                    Option<&MachineEffectDeclaration>,
                >::new();
                for row in &catalog.catalog().declarations {
                    map.entry((row.semantic, row.constraint))
                        .and_modify(|existing| *existing = None)
                        .or_insert(Some(row));
                }
                map
            };
            let mut functions = Vec::with_capacity(source.functions.len());
            for function in &source.functions {
                let mut blocks = Vec::with_capacity(function.blocks.len());
                for block in &function.blocks {
                    let mut instructions = Vec::with_capacity(block.instructions.len() + 1);
                    for instruction in &block.instructions {
                        instructions.push(compute_instruction(
                            function,
                            instruction,
                            physical,
                            constraints,
                            &declarations_by_key,
                        )?);
                    }
                    let terminator =
                        selected_instructions::terminator_instruction(&block.terminator);
                    instructions.push(compute_instruction(
                        function,
                        terminator,
                        physical,
                        constraints,
                        &declarations_by_key,
                    )?);
                    blocks.push(BlockMachineEffects {
                        block: block.id,
                        instructions,
                    });
                }
                functions.push(FunctionMachineEffects {
                    machine: function.machine,
                    blocks,
                });
            }
            let mut plan = PreAllocationMachineEffectPlan {
                identity: selected_instructions::PreAllocationMachineEffectIdentity::from_bytes(
                    [0; 32],
                ),
                selected: selected.selected_identity(),
                optimization_unit: selected.optimization_unit_identity(),
                work_schedule: selected.work_schedule_identity(),
                target: source.target,
                register_environment,
                register_constraints: constraints.identity(),
                machine_effect_catalog: catalog.identity(),
                functions,
            };
            plan.identity = pre_allocation_machine_effect_identity(&plan);
            Ok(plan)
        }

        fn validate_catalog_roots(
            target: target::NativeTarget,
            constraints: &PreparedRegisterConstraintCatalog,
            selected_keys: &TargetRegisterEnvironmentConstraintKeys,
            catalog: &PreparedMachineEffectCatalog,
        ) -> Result<(), MachineEffectError> {
            if catalog.catalog().target != target {
                return Err(MachineEffectError::CatalogTargetMismatch);
            }
            if catalog.catalog().register_constraints != constraints.identity() {
                return Err(MachineEffectError::CatalogConstraintMismatch);
            }
            let catalog_keys = &catalog.catalog().selected_keys;
            if catalog_keys.rows != *selected_keys
                || catalog_keys.invoke_no_normal != catalog_keys.ordinary_call_constraints()
            {
                return Err(MachineEffectError::CatalogSelectedKeysMismatch);
            }
            Ok(())
        }

        fn check_operands(
            function: &SelectedFunction,
            operands: &[selected_instructions::SelectedOperand],
            required: &[register_model::RegisterOperandConstraint],
        ) -> Result<(), String> {
            if operands.len() != required.len() {
                return Err(format!(
                    "operand count is {}, expected {}",
                    operands.len(),
                    required.len()
                ));
            }
            for (operand, expected) in operands.iter().zip(required) {
                let number = expected.operand;
                let register_id = operand.virtual_register;
                let register = function
                    .virtual_registers
                    .get(register_id.0 as usize)
                    .ok_or_else(|| {
                        format!(
                            "operand {number} refers to missing virtual register {}",
                            register_id.0
                        )
                    })?;

                check_operand_field(number, "number", number, operand.operand)?;
                check_operand_field(number, "access", expected.access, operand.access)?;
                check_operand_field(number, "register class", expected.class, operand.class)?;
                check_operand_field(
                    number,
                    "fixed register view",
                    expected.fixed_view,
                    operand.fixed_view,
                )?;
                check_operand_field(number, "tied operand", expected.tied_to, operand.tied_to)?;
                check_operand_field(
                    number,
                    "early clobber",
                    expected.early_clobber,
                    operand.early_clobber,
                )?;
                check_operand_field(number, "virtual register ID", register_id.0, register.id.0)?;
                check_operand_field(
                    number,
                    "declared register class",
                    expected.class,
                    register.class,
                )?;
            }
            Ok(())
        }

        fn check_operand_field<T: Eq + std::fmt::Debug>(
            number: u16,
            field: &str,
            expected: T,
            actual: T,
        ) -> Result<(), String> {
            if actual == expected {
                Ok(())
            } else {
                Err(format!(
                    "operand {number} {field}: expected {expected:?}, found {actual:?}"
                ))
            }
        }

        fn compute_instruction(
            function: &SelectedFunction,
            instruction: &SelectedInstruction,
            physical: &ValidatedPhysicalRegisterModel,
            constraints: &PreparedRegisterConstraintCatalog,
            declarations_by_key: &std::collections::BTreeMap<
                (MachineSemanticKind, register_model::RegisterConstraintKey),
                Option<&MachineEffectDeclaration>,
            >,
        ) -> Result<InstructionMachineEffects, MachineEffectError> {
            if matches!(instruction.kind, SelectedInstructionKind::Store { byte_size, .. } if !matches!(byte_size, 1 | 2 | 4 | 8))
            {
                return Err(MachineEffectError::ConstraintEffectMismatch {
                    instruction: instruction.id,
                });
            }
            if !instruction.provenance.work_settles_only_claimed_sites() {
                return Err(MachineEffectError::WorkProvenanceMismatch {
                    instruction: instruction.id,
                });
            }
            let constraint =
                super::foreign_call::constraint(function, instruction, physical, constraints)?;
            check_operands(function, &instruction.operands, &constraint.operands).map_err(
                |detail| MachineEffectError::ConstraintOperandMismatch {
                    machine: function.machine,
                    instruction: instruction.id,
                    semantic: machine_semantic_kind(instruction.kind),
                    constraint: instruction.constraint,
                    detail,
                },
            )?;
            if instruction.implicit_uses != constraint.implicit_uses
                || instruction.implicit_defs != constraint.implicit_defs
                || instruction.clobbers != constraint.clobbers
            {
                return Err(MachineEffectError::ConstraintEffectMismatch {
                    instruction: instruction.id,
                });
            }
            let declaration = super::foreign_call::declaration(
                function,
                instruction,
                physical,
                &constraint,
                exact_declaration(function.machine, instruction, declarations_by_key)?,
            )?;
            Ok(InstructionMachineEffects {
                instruction: instruction.id,
                kind: instruction.kind,
                constraint: instruction.constraint,
                unit_uses: instruction.implicit_uses.clone(),
                unit_defs: instruction.implicit_defs.clone(),
                unit_clobbers: instruction.clobbers.clone(),
                memory: declaration.memory,
                trap: declaration.trap,
                barrier: declaration.barrier,
                call: declaration.call,
                cleanup: declaration.cleanup,
                provenance: instruction.provenance.clone(),
                alternatives: declaration.alternatives.clone(),
            })
        }

        fn exact_declaration<'a>(
            machine: semantic_vocabulary::MachineId,
            instruction: &SelectedInstruction,
            declarations_by_key: &std::collections::BTreeMap<
                (MachineSemanticKind, register_model::RegisterConstraintKey),
                Option<&'a MachineEffectDeclaration>,
            >,
        ) -> Result<&'a MachineEffectDeclaration, MachineEffectError> {
            let semantic = machine_semantic_kind(instruction.kind);
            match declarations_by_key.get(&(semantic, instruction.constraint)) {
                Some(Some(declaration)) => Ok(*declaration),
                Some(None) => Err(MachineEffectError::AmbiguousDeclaration {
                    instruction: instruction.id,
                }),
                None => Err(MachineEffectError::MissingDeclaration {
                    machine,
                    instruction: instruction.id,
                    semantic,
                    constraint: instruction.constraint,
                }),
            }
        }

        pub fn machine_semantic_kind(kind: SelectedInstructionKind) -> MachineSemanticKind {
            match kind {
                SelectedInstructionKind::InvokeNoNormal { .. } => {
                    MachineSemanticKind::InvokeNoNormal
                }
                SelectedInstructionKind::InstalledInterruptCpuCheck { .. } => {
                    MachineSemanticKind::InstalledInterruptCpuCheck
                }
                SelectedInstructionKind::InstalledInterruptTaskCommit { .. } => {
                    MachineSemanticKind::InstalledInterruptTaskCommit
                }
                SelectedInstructionKind::InstalledInterruptTablePublication { .. } => {
                    MachineSemanticKind::InstalledInterruptTablePublication
                }

                SelectedInstructionKind::RegisterAccess { operation } => match operation {
                    selected_instructions::RegisterOperation::TaskRegisterActivate => {
                        MachineSemanticKind::TaskRegisterActivate
                    }
                    selected_instructions::RegisterOperation::SegmentReload(
                        terminal_psi::SegmentReload::Code,
                    ) => MachineSemanticKind::CodeSegmentReload,
                    selected_instructions::RegisterOperation::SegmentReload(
                        terminal_psi::SegmentReload::Data,
                    ) => MachineSemanticKind::DataSegmentReload,
                    selected_instructions::RegisterOperation::SegmentRead(_) => {
                        MachineSemanticKind::SegmentSelectorRead
                    }
                    selected_instructions::RegisterOperation::TaskRegisterRead => {
                        MachineSemanticKind::TaskRegisterRead
                    }
                    selected_instructions::RegisterOperation::GlobalDescriptorTable(
                        selected_instructions::GlobalDescriptorTableOperation::Store,
                    ) => MachineSemanticKind::GlobalDescriptorTableStore,
                    selected_instructions::RegisterOperation::GlobalDescriptorTable(
                        selected_instructions::GlobalDescriptorTableOperation::Load,
                    ) => MachineSemanticKind::GlobalDescriptorTableLoad,
                    selected_instructions::RegisterOperation::SystemRead(
                        selected_instructions::SystemRegister::Control(_),
                    ) => MachineSemanticKind::ControlRegisterRead,
                    selected_instructions::RegisterOperation::SystemWrite(
                        selected_instructions::SystemRegister::Control(_),
                    ) => MachineSemanticKind::ControlRegisterWrite,
                    selected_instructions::RegisterOperation::SystemRead(_) => {
                        MachineSemanticKind::SystemRegisterRead
                    }
                    selected_instructions::RegisterOperation::SystemWrite(_) => {
                        MachineSemanticKind::SystemRegisterWrite
                    }
                    selected_instructions::RegisterOperation::PortRead => {
                        MachineSemanticKind::PortRegisterRead
                    }
                    selected_instructions::RegisterOperation::PortWrite => {
                        MachineSemanticKind::PortRegisterWrite
                    }
                    selected_instructions::RegisterOperation::ModelSpecificRead => {
                        MachineSemanticKind::ModelSpecificRead
                    }
                    selected_instructions::RegisterOperation::ModelSpecificWrite => {
                        MachineSemanticKind::ModelSpecificWrite
                    }
                    selected_instructions::RegisterOperation::ProcessorHalt => {
                        MachineSemanticKind::ProcessorHalt
                    }
                    selected_instructions::RegisterOperation::MemoryFence(_) => {
                        MachineSemanticKind::MemoryFence
                    }
                    selected_instructions::RegisterOperation::InterruptControl(_) => {
                        MachineSemanticKind::InterruptControl
                    }
                },
                SelectedInstructionKind::AccessorScalarAccess { form, .. } => {
                    MachineSemanticKind::AccessorScalarAccess(form)
                }
                SelectedInstructionKind::PortWrite { .. } => MachineSemanticKind::PortWrite,
                SelectedInstructionKind::AtomicFence => MachineSemanticKind::AtomicFence,
                SelectedInstructionKind::AtomicAccess { form } => {
                    MachineSemanticKind::AtomicAccess(form)
                }
                SelectedInstructionKind::NearestIeeeFloatArithmetic { form } => {
                    MachineSemanticKind::NearestIeeeFloatArithmetic(form)
                }
                SelectedInstructionKind::CompareI64Zero => MachineSemanticKind::CompareI64Zero,
                SelectedInstructionKind::CallAggregate { .. } => MachineSemanticKind::CallAggregate,
                SelectedInstructionKind::ReturnAggregate { .. } => {
                    MachineSemanticKind::ReturnAggregate
                }
                SelectedInstructionKind::CompareByteSequences => {
                    MachineSemanticKind::CompareByteSequences
                }
                SelectedInstructionKind::CompareI64 => MachineSemanticKind::CompareI64,
                SelectedInstructionKind::CompareI64Immediate { .. } => {
                    MachineSemanticKind::CompareI64Immediate
                }
                SelectedInstructionKind::MaterializeI64 { .. } => {
                    MachineSemanticKind::MaterializeI64
                }
                SelectedInstructionKind::CopyI64 => MachineSemanticKind::CopyI64,
                SelectedInstructionKind::Float32ToBits => MachineSemanticKind::Float32ToBits,
                SelectedInstructionKind::Float64ToBits => MachineSemanticKind::Float64ToBits,
                SelectedInstructionKind::BitsToFloat32 => MachineSemanticKind::BitsToFloat32,
                SelectedInstructionKind::BitsToFloat64 => MachineSemanticKind::BitsToFloat64,
                SelectedInstructionKind::ZeroExtendU8 => MachineSemanticKind::ZeroExtendU8,
                SelectedInstructionKind::ZeroExtendU32 => MachineSemanticKind::ZeroExtendU32,
                SelectedInstructionKind::ZeroExtendU16 => MachineSemanticKind::ZeroExtendU16,
                SelectedInstructionKind::SignExtendI8 => MachineSemanticKind::SignExtendI8,
                SelectedInstructionKind::SignExtendI16 => MachineSemanticKind::SignExtendI16,
                SelectedInstructionKind::SignExtendI32 => MachineSemanticKind::SignExtendI32,
                SelectedInstructionKind::MaterializeBooleanEqual => {
                    MachineSemanticKind::MaterializeBooleanEqual
                }
                SelectedInstructionKind::MaterializeBooleanU64LessThan => {
                    MachineSemanticKind::MaterializeBooleanU64LessThan
                }
                SelectedInstructionKind::MaterializeBooleanI64LessThan => {
                    MachineSemanticKind::MaterializeBooleanI64LessThan
                }
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => {
                    MachineSemanticKind::MaterializeBooleanU64LessOrEqual
                }
                SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
                    MachineSemanticKind::MaterializeBooleanI64LessOrEqual
                }

                SelectedInstructionKind::SaveFloatingControl { .. } => {
                    MachineSemanticKind::SaveFloatingControl
                }
                SelectedInstructionKind::RestoreFloatingControl { .. } => {
                    MachineSemanticKind::RestoreFloatingControl
                }
                SelectedInstructionKind::HostedWriteByteI32 { .. } => {
                    MachineSemanticKind::HostedWriteByteI32
                }
                SelectedInstructionKind::HostedReadByte { .. } => {
                    MachineSemanticKind::HostedReadByte
                }
                SelectedInstructionKind::ByteViewAddress => MachineSemanticKind::ByteViewAddress,
                SelectedInstructionKind::HostedExitProcessI32 => {
                    MachineSemanticKind::HostedExitProcessI32
                }
                SelectedInstructionKind::ExactAddI64 { .. } => MachineSemanticKind::ExactAddI64,
                SelectedInstructionKind::WrappingAddI64 => MachineSemanticKind::WrappingAddI64,
                SelectedInstructionKind::BitwiseAndI64 => MachineSemanticKind::BitwiseAndI64,
                SelectedInstructionKind::BitwiseXorI64 => MachineSemanticKind::BitwiseXorI64,
                SelectedInstructionKind::ExactDivideU64 { .. } => {
                    MachineSemanticKind::ExactDivideU64
                }
                SelectedInstructionKind::WrappingRemainderI64 { .. } => {
                    MachineSemanticKind::WrappingRemainderI64
                }
                SelectedInstructionKind::SaturatingAdd { carrier } => {
                    MachineSemanticKind::SaturatingAdd(carrier)
                }
                SelectedInstructionKind::SaturatingSubtract { carrier } => {
                    MachineSemanticKind::SaturatingSubtract(carrier)
                }
                SelectedInstructionKind::SaturatingDivide { carrier, .. } => {
                    MachineSemanticKind::SaturatingDivide(carrier)
                }
                SelectedInstructionKind::SaturatingRemainder { carrier, .. } => {
                    MachineSemanticKind::SaturatingRemainder(carrier)
                }
                SelectedInstructionKind::SaturatingMultiply { carrier } => {
                    MachineSemanticKind::SaturatingMultiply(carrier)
                }
                SelectedInstructionKind::TrappingInteger { form } => {
                    MachineSemanticKind::TrappingInteger(form)
                }
                SelectedInstructionKind::FloatResultPolicy { form } => {
                    MachineSemanticKind::FloatResultPolicy(form)
                }
                SelectedInstructionKind::ExactAddI64Immediate { .. } => {
                    MachineSemanticKind::ExactAddI64Immediate
                }
                SelectedInstructionKind::ExactSubtractI64 { .. } => {
                    MachineSemanticKind::ExactSubtractI64
                }
                SelectedInstructionKind::ExactSubtractI64Immediate { .. } => {
                    MachineSemanticKind::ExactSubtractI64Immediate
                }
                SelectedInstructionKind::ExactMultiplyI64 { .. } => {
                    MachineSemanticKind::ExactMultiplyI64
                }
                SelectedInstructionKind::ExactRemainderU64 { .. } => {
                    MachineSemanticKind::ExactRemainderU64
                }
                SelectedInstructionKind::WrappingSubtractI64 => {
                    MachineSemanticKind::WrappingSubtractI64
                }
                SelectedInstructionKind::WrappingMultiplyI64 => {
                    MachineSemanticKind::WrappingMultiplyI64
                }
                SelectedInstructionKind::WrappingDivideI64 { .. } => {
                    MachineSemanticKind::WrappingDivideI64
                }
                SelectedInstructionKind::ExactDivideI64 { .. } => {
                    MachineSemanticKind::ExactDivideI64
                }
                SelectedInstructionKind::ExactRemainderI64 { .. } => {
                    MachineSemanticKind::ExactRemainderI64
                }
                SelectedInstructionKind::WrappingShiftLeftI64 => {
                    MachineSemanticKind::WrappingShiftLeftI64
                }
                SelectedInstructionKind::WrappingShiftRightI64 => {
                    MachineSemanticKind::WrappingShiftRightI64
                }
                SelectedInstructionKind::WrappingShiftRightU64 => {
                    MachineSemanticKind::WrappingShiftRightU64
                }
                SelectedInstructionKind::ExactShiftLeftI64 { .. } => {
                    MachineSemanticKind::ExactShiftLeftI64
                }
                SelectedInstructionKind::ExactShiftRightI64 { .. } => {
                    MachineSemanticKind::ExactShiftRightI64
                }
                SelectedInstructionKind::ExactShiftRightU64 { .. } => {
                    MachineSemanticKind::ExactShiftRightU64
                }
                SelectedInstructionKind::BitwiseOrI64 => MachineSemanticKind::BitwiseOrI64,
                SelectedInstructionKind::BitwiseNotI64 => MachineSemanticKind::BitwiseNotI64,
                SelectedInstructionKind::ConditionalBranchNonZero => {
                    MachineSemanticKind::ConditionalBranchNonZero
                }
                SelectedInstructionKind::ConditionalBranchU64LessThan => {
                    MachineSemanticKind::ConditionalBranchU64LessThan
                }
                SelectedInstructionKind::ConditionalBranchI64LessThan => {
                    MachineSemanticKind::ConditionalBranchI64LessThan
                }
                SelectedInstructionKind::Jump => MachineSemanticKind::Jump,
                SelectedInstructionKind::ReturnScalar => MachineSemanticKind::ReturnScalar,
                SelectedInstructionKind::Crash => MachineSemanticKind::Crash,
                SelectedInstructionKind::PrivilegedCrash => MachineSemanticKind::PrivilegedCrash,
                SelectedInstructionKind::ReturnUnit => MachineSemanticKind::ReturnUnit,
                SelectedInstructionKind::CallScalar { .. } => MachineSemanticKind::CallScalar,
                SelectedInstructionKind::Load64 { .. } => MachineSemanticKind::Load64,
                SelectedInstructionKind::LoadPacked { width, .. } => match width {
                    selected_instructions::PackedByteWidth::Three => {
                        MachineSemanticKind::LoadPacked3
                    }
                    selected_instructions::PackedByteWidth::Five => {
                        MachineSemanticKind::LoadPacked5
                    }
                    selected_instructions::PackedByteWidth::Six => MachineSemanticKind::LoadPacked6,
                    selected_instructions::PackedByteWidth::Seven => {
                        MachineSemanticKind::LoadPacked7
                    }
                },
                SelectedInstructionKind::StorePacked { .. } => MachineSemanticKind::StorePacked,
                SelectedInstructionKind::Load8 { .. } => MachineSemanticKind::Load8,
                SelectedInstructionKind::Load16 { .. } => MachineSemanticKind::Load16,
                SelectedInstructionKind::Load32 { .. } => MachineSemanticKind::Load32,
                SelectedInstructionKind::Load8Indexed => MachineSemanticKind::Load8Indexed,
                SelectedInstructionKind::Load16Indexed => MachineSemanticKind::Load16Indexed,
                SelectedInstructionKind::Load32Indexed => MachineSemanticKind::Load32Indexed,
                SelectedInstructionKind::Load64Indexed => MachineSemanticKind::Load64Indexed,
                SelectedInstructionKind::PackedAddI16x8 => MachineSemanticKind::PackedAddI16x8,
                SelectedInstructionKind::PackedSubI16x8 => MachineSemanticKind::PackedSubI16x8,
                SelectedInstructionKind::PackedShuffleI16x8 { .. } => {
                    MachineSemanticKind::PackedShuffleI16x8
                }
                SelectedInstructionKind::PackedShuffleGeneralI16x8 { .. } => {
                    MachineSemanticKind::PackedShuffleGeneralI16x8
                }
                SelectedInstructionKind::PackedShiftRightArithmeticI16x8 { .. } => {
                    MachineSemanticKind::PackedShiftRightArithmeticI16x8
                }
                SelectedInstructionKind::PackedWidenU8x8ToI16x8 => {
                    MachineSemanticKind::PackedWidenU8x8ToI16x8
                }
                SelectedInstructionKind::PackedNarrowI16x8ToU8x8Saturating => {
                    MachineSemanticKind::PackedNarrowI16x8ToU8x8Saturating
                }
                SelectedInstructionKind::PackedLoadI16x4ZeroExtend => {
                    MachineSemanticKind::PackedLoadI16x4ZeroExtend
                }
                SelectedInstructionKind::PackedWidenU8x4ToI16x8 => {
                    MachineSemanticKind::PackedWidenU8x4ToI16x8
                }
                SelectedInstructionKind::PackedNarrowI16x8ToU8x4Saturating => {
                    MachineSemanticKind::PackedNarrowI16x8ToU8x4Saturating
                }
                SelectedInstructionKind::PackedAddU8x16 => MachineSemanticKind::PackedAddU8x16,
                SelectedInstructionKind::PackedMinI16x8 => MachineSemanticKind::PackedMinI16x8,
                SelectedInstructionKind::PackedCmpGtI16x8 => MachineSemanticKind::PackedCmpGtI16x8,
                SelectedInstructionKind::PackedBitAndI16x8 => {
                    MachineSemanticKind::PackedBitAndI16x8
                }
                SelectedInstructionKind::PackedBitAndNotI16x8 => {
                    MachineSemanticKind::PackedBitAndNotI16x8
                }
                SelectedInstructionKind::PackedAbsI16x8 => MachineSemanticKind::PackedAbsI16x8,
                SelectedInstructionKind::PackedLoad128 { .. } => MachineSemanticKind::PackedLoad128,
                SelectedInstructionKind::PackedStore128 { .. } => {
                    MachineSemanticKind::PackedStore128
                }
                SelectedInstructionKind::PackedLoad64ZeroExtend { .. } => {
                    MachineSemanticKind::PackedLoad64ZeroExtend
                }
                SelectedInstructionKind::PackedStore64 { .. } => MachineSemanticKind::PackedStore64,
                SelectedInstructionKind::PackedLoad32ZeroExtend { .. } => {
                    MachineSemanticKind::PackedLoad32ZeroExtend
                }
                SelectedInstructionKind::PackedStore32 { .. } => MachineSemanticKind::PackedStore32,
                SelectedInstructionKind::PackedMaterializeZero => {
                    MachineSemanticKind::PackedMaterializeZero
                }
                SelectedInstructionKind::PackedFromGpr64ZeroExtend => {
                    MachineSemanticKind::PackedFromGpr64ZeroExtend
                }
                SelectedInstructionKind::PackedFromGpr64Pair => {
                    MachineSemanticKind::PackedFromGpr64Pair
                }
                SelectedInstructionKind::PackedAddI16x8Reg => {
                    MachineSemanticKind::PackedAddI16x8Reg
                }
                SelectedInstructionKind::PackedSubI16x8Reg => {
                    MachineSemanticKind::PackedSubI16x8Reg
                }
                SelectedInstructionKind::PackedShiftRightArithmeticI16x8Reg { .. } => {
                    MachineSemanticKind::PackedShiftRightArithmeticI16x8Reg
                }
                SelectedInstructionKind::PackedShuffleI16x8Reg { .. } => {
                    MachineSemanticKind::PackedShuffleI16x8Reg
                }
                SelectedInstructionKind::PackedWidenU8x8ToI16x8Reg => {
                    MachineSemanticKind::PackedWidenU8x8ToI16x8Reg
                }
                SelectedInstructionKind::PackedNarrowI16x8ToU8x8SaturatingReg => {
                    MachineSemanticKind::PackedNarrowI16x8ToU8x8SaturatingReg
                }
                SelectedInstructionKind::PackedAddU8x16Reg => {
                    MachineSemanticKind::PackedAddU8x16Reg
                }
                SelectedInstructionKind::PackedMinI16x8Reg => {
                    MachineSemanticKind::PackedMinI16x8Reg
                }
                SelectedInstructionKind::PackedCmpGtI16x8Reg => {
                    MachineSemanticKind::PackedCmpGtI16x8Reg
                }
                SelectedInstructionKind::PackedBitAndI16x8Reg => {
                    MachineSemanticKind::PackedBitAndI16x8Reg
                }
                SelectedInstructionKind::PackedBitAndNotI16x8Reg => {
                    MachineSemanticKind::PackedBitAndNotI16x8Reg
                }
                SelectedInstructionKind::PackedAbsI16x8Reg => {
                    MachineSemanticKind::PackedAbsI16x8Reg
                }
                SelectedInstructionKind::CopyBytes => MachineSemanticKind::CopyBytes,
                SelectedInstructionKind::LoadEncodedScalar => {
                    MachineSemanticKind::LoadEncodedScalar
                }
                SelectedInstructionKind::StoreEncodedScalar => {
                    MachineSemanticKind::StoreEncodedScalar
                }
                SelectedInstructionKind::ZeroBytes => MachineSemanticKind::ZeroBytes,
                SelectedInstructionKind::ScanByteSequence { .. } => {
                    MachineSemanticKind::ScanByteSequence
                }
                SelectedInstructionKind::Store { .. } => MachineSemanticKind::Store,
                SelectedInstructionKind::AddressOffset { .. } => MachineSemanticKind::AddressOffset,
                SelectedInstructionKind::Store64 { .. } => MachineSemanticKind::Store64,
                SelectedInstructionKind::FrameAddress { .. } => MachineSemanticKind::FrameAddress,
                SelectedInstructionKind::ImageAddress { .. } => MachineSemanticKind::ImageAddress,
                SelectedInstructionKind::CallUnit { .. } => MachineSemanticKind::CallUnit,
                SelectedInstructionKind::NormalizedForeignCall { .. } => {
                    MachineSemanticKind::NormalizedForeignCall
                }
            }
        }

        pub fn machine_alternative_is_transparent(alternative: &MachineAlternative) -> bool {
            let encoded = &alternative.encoded;
            encoded.memory == MachineEncodedMemoryEffect::NoneV1
                && encoded.stack == MachineEncodedStackEffect::UnchangedV1
                && encoded.trap == MachineEncodedTrapBehavior::NeverV1
                && encoded.control == MachineEncodedControlEffect::FallThroughV1
                && encoded.implicit_unit_uses.is_empty()
                && encoded.implicit_unit_defs.is_empty()
                && encoded.implicit_unit_clobbers.is_empty()
        }
    }
    mod foreign_call {

        use std::borrow::Cow;

        use register_model::{
            PreparedRegisterConstraintCatalog, RegisterInstructionConstraint,
            ValidatedPhysicalRegisterModel,
        };
        use selected_instructions::{
            MachineEffectDeclaration, SelectedFunction, SelectedInstruction,
            SelectedInstructionKind,
        };

        use super::MachineEffectError;

        pub(super) fn constraint<'catalog>(
            function: &SelectedFunction,
            instruction: &SelectedInstruction,
            physical: &ValidatedPhysicalRegisterModel,
            catalog: &'catalog PreparedRegisterConstraintCatalog,
        ) -> Result<Cow<'catalog, RegisterInstructionConstraint>, MachineEffectError> {
            let invalid = || MachineEffectError::ConstraintEffectMismatch {
                instruction: instruction.id,
            };
            let base = register_model::find_register_constraint(
                &catalog.catalog().constraints,
                instruction.constraint,
            )
            .ok_or_else(invalid)?;
            if instruction
                .kind
                .call_target()
                .is_some_and(|target| target.indirect_code_offset().is_some())
            {
                selected_instructions::calls::dynamic::call_record(function, instruction)
                    .ok_or_else(invalid)?;
                return Ok(Cow::Owned(
                    selected_instructions::calls::dynamic::instantiate_constraint(base, physical)
                        .ok_or_else(invalid)?,
                ));
            }
            if !matches!(
                instruction.kind,
                SelectedInstructionKind::NormalizedForeignCall { .. }
            ) {
                return Ok(Cow::Borrowed(base));
            }
            let record = selected_instructions::calls::constraints::foreign_call_record(
                function,
                instruction,
            )
            .ok_or_else(invalid)?;
            let row =
                selected_instructions::calls::constraints::instantiate_foreign_call_constraint(
                    &record.call,
                    base,
                    physical,
                    |register| register_environment::fixed_register_view(physical, register),
                )
                .ok_or_else(invalid)?;
            Ok(Cow::Owned(row))
        }

        pub(super) fn declaration<'catalog>(
            function: &SelectedFunction,
            instruction: &SelectedInstruction,
            physical: &ValidatedPhysicalRegisterModel,
            constraint: &RegisterInstructionConstraint,
            base: &'catalog MachineEffectDeclaration,
        ) -> Result<Cow<'catalog, MachineEffectDeclaration>, MachineEffectError> {
            let invalid = || MachineEffectError::ConstraintEffectMismatch {
                instruction: instruction.id,
            };
            if instruction
                .kind
                .call_target()
                .is_some_and(|target| target.indirect_code_offset().is_some())
            {
                selected_instructions::calls::dynamic::call_record(function, instruction)
                    .ok_or_else(invalid)?;
                return Ok(Cow::Owned(
                    selected_instructions::calls::dynamic::instantiate_declaration(
                        base, constraint, physical,
                    )
                    .ok_or_else(invalid)?,
                ));
            }
            if !matches!(
                instruction.kind,
                SelectedInstructionKind::NormalizedForeignCall { .. }
            ) {
                return Ok(Cow::Borrowed(base));
            }
            let record = selected_instructions::calls::constraints::foreign_call_record(
                function,
                instruction,
            )
            .ok_or_else(invalid)?;
            let declaration = match physical.model().architecture {
                target::Architecture::Aarch64 => {
                    target_aarch64::aarch64_normalized_foreign_call_declaration(record, constraint)
                }
                target::Architecture::X86_64 => {
                    target_x86_64::x86_64_normalized_foreign_call_declaration(record, constraint)
                }
            }
            .ok_or_else(invalid)?;
            if declaration.semantic != base.semantic || declaration.constraint != base.constraint {
                return Err(invalid());
            }
            Ok(Cow::Owned(declaration))
        }
    }
    mod validate {
        use super::{MachineEffectError, ValidatedPreAllocationMachineEffects, receipt};
        use selected_instructions::PreAllocationMachineEffectPlan;

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn seal_pre_allocation_machine_effects(
            plan: PreAllocationMachineEffectPlan,
        ) -> Result<ValidatedPreAllocationMachineEffects, MachineEffectError> {
            let receipt = receipt(&plan);
            Ok(ValidatedPreAllocationMachineEffects::new(plan, receipt))
        }
    }

    pub use compute::{machine_alternative_is_transparent, machine_semantic_kind};
    pub use selected_instructions::{
        PreAllocationMachineEffectIdentity, PreAllocationMachineEffectPlan,
    };

    use crate::SelectedProgramView;
    use register_model::{
        PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys,
        TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
        ValidatedRegisterReservationProfile,
    };
    use selected_instructions::PreparedMachineEffectCatalog;
    use selected_instructions::{
        MachineEffectCatalogIdentity, SelectedInstructionId, SelectedInstructionPlanIdentity,
    };
    use std::sync::Arc;

    #[allow(clippy::too_many_arguments)]
    pub fn analyze_pre_allocation_machine_effects<S: SelectedProgramView>(
        selected: &S,
        register_environment: TargetRegisterEnvironmentIdentity,
        physical: &ValidatedPhysicalRegisterModel,
        constraints: &PreparedRegisterConstraintCatalog,
        reservations: &ValidatedRegisterReservationProfile,
        selected_keys: &TargetRegisterEnvironmentConstraintKeys,
        catalog: &PreparedMachineEffectCatalog,
    ) -> Result<ValidatedPreAllocationMachineEffects, MachineEffectError> {
        let plan = compute::compute_terminal_pre_allocation_machine_effects(
            selected,
            register_environment,
            physical,
            constraints,
            reservations,
            selected_keys,
            catalog,
        )?;
        validate::seal_pre_allocation_machine_effects(plan)
    }

    copy_receipt! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct PreAllocationMachineEffectReceipt {
            identity: PreAllocationMachineEffectIdentity,
            selected: SelectedInstructionPlanIdentity,
            register_environment: TargetRegisterEnvironmentIdentity,
            machine_effect_catalog: MachineEffectCatalogIdentity,
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedPreAllocationMachineEffects {
        plan: Arc<PreAllocationMachineEffectPlan>,
        receipt: PreAllocationMachineEffectReceipt,
    }

    impl ValidatedPreAllocationMachineEffects {
        pub fn plan(&self) -> &PreAllocationMachineEffectPlan {
            &self.plan
        }

        pub fn shared_plan(&self) -> Arc<PreAllocationMachineEffectPlan> {
            Arc::clone(&self.plan)
        }

        receipt_accessor_rows!(ref ValidatedPreAllocationMachineEffects {
            receipt: PreAllocationMachineEffectReceipt,
        });

        pub fn new(
            plan: PreAllocationMachineEffectPlan,
            receipt: PreAllocationMachineEffectReceipt,
        ) -> Self {
            Self {
                plan: Arc::new(plan),
                receipt,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum MachineEffectError {
        RegisterEnvironmentMismatch,
        CatalogTargetMismatch,
        CatalogConstraintMismatch,
        CatalogSelectedKeysMismatch,
        SelectedRootMismatch,
        MissingDeclaration {
            machine: semantic_vocabulary::MachineId,
            instruction: SelectedInstructionId,
            semantic: selected_instructions::MachineSemanticKind,
            constraint: register_model::RegisterConstraintKey,
        },
        AmbiguousDeclaration {
            instruction: SelectedInstructionId,
        },
        ConstraintEffectMismatch {
            instruction: SelectedInstructionId,
        },
        ConstraintOperandMismatch {
            machine: semantic_vocabulary::MachineId,
            instruction: SelectedInstructionId,
            semantic: selected_instructions::MachineSemanticKind,
            constraint: register_model::RegisterConstraintKey,
            detail: String,
        },
        WorkProvenanceMismatch {
            instruction: SelectedInstructionId,
        },
        NonCanonicalFunction,
        NonCanonicalBlock,
        InstructionMismatch {
            instruction: SelectedInstructionId,
        },
        IdentityMismatch,
    }

    failed_display!(MachineEffectError, "machine-effect analysis failed");

    fn receipt(plan: &PreAllocationMachineEffectPlan) -> PreAllocationMachineEffectReceipt {
        PreAllocationMachineEffectReceipt {
            identity: plan.identity,
            selected: plan.selected,
            register_environment: plan.register_environment,
            machine_effect_catalog: plan.machine_effect_catalog,
        }
    }
}

pub use error::MachineEffectStageError;
pub use facts::MachineEffectError;
pub use facts::ValidatedPreAllocationMachineEffects;
pub use facts::{
    analyze_pre_allocation_machine_effects, machine_alternative_is_transparent,
    machine_semantic_kind,
};

use crate::SelectedProgramView;
use register_environment::ValidatedTargetRegisterEnvironment;

pub fn prepared_machine_effect_catalog(
    target: target::NativeTarget,
    constraints: &register_model::PreparedRegisterConstraintCatalog,
) -> Result<
    std::sync::Arc<selected_instructions::PreparedMachineEffectCatalog>,
    MachineEffectStageError,
> {
    catalog::prepared_catalog(target, constraints)
}

pub fn analyze_machine_effects<S: SelectedProgramView>(
    selected: &S,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedPreAllocationMachineEffects, MachineEffectStageError> {
    analysis::analyze(selected, environment)
}

#[cfg(test)]
mod catalog_handoff_tests {
    use super::{MachineEffectStageError, prepared_machine_effect_catalog};
    use register_environment::baseline_target_register_environment;
    use selected_instructions::{
        MachineSemanticKind, MachineSizeKnowledge, canonical_machine_effect_catalog,
        machine_effect_catalog_identity,
    };
    use target::{Architecture, NativeTarget};

    fn hosted_targets() -> [NativeTarget; 5] {
        [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget::windows_arm64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ]
    }

    #[test]
    fn catalog_handoff_reuses_the_producers_catalog_and_identity() {
        for target in hosted_targets() {
            let environment = baseline_target_register_environment(target).unwrap();
            let prepared =
                prepared_machine_effect_catalog(target, environment.constraints()).unwrap();
            // Default preparation must populate the canonical owner, not wrap a
            // separately reconstructed/validated copy in another cache.
            let canonical = canonical_machine_effect_catalog::<&str>(
                target,
                environment.constraints(),
                |_, _| Err("the prepared catalog must already be retained"),
            )
            .unwrap();
            assert!(std::sync::Arc::ptr_eq(&prepared, &canonical));
            assert_eq!(
                prepared.identity(),
                machine_effect_catalog_identity(prepared.catalog())
            );
        }
    }

    #[test]
    fn catalog_handoff_keeps_supplied_catalogs_on_the_validation_path() {
        for target in hosted_targets() {
            let environment = baseline_target_register_environment(target).unwrap();
            let prepared =
                prepared_machine_effect_catalog(target, environment.constraints()).unwrap();
            let mut changed = prepared.catalog().clone();
            changed
                .declaration_by_semantic_mut(MachineSemanticKind::MaterializeI64)
                .unwrap()
                .alternatives[0]
                .size = MachineSizeKnowledge::ExactBytes(1);
            match target.architecture {
                Architecture::X86_64 => assert!(matches!(
                    target_x86_64::validate_x86_64_machine_effect_catalog(target, environment.constraints(), changed),
                    Err(target_x86_64::X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch)
                )),
                Architecture::Aarch64 => assert!(matches!(
                    target_aarch64::validate_aarch64_machine_effect_catalog(target, environment.constraints(), changed),
                    Err(target_aarch64::Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch)
                )),
            }
        }
    }

    #[test]
    fn catalog_handoff_propagates_an_architecture_mismatch() {
        let environment =
            baseline_target_register_environment(NativeTarget::windows_x64()).unwrap();
        let target = NativeTarget::windows_arm64();
        assert!(matches!(
            prepared_machine_effect_catalog(target, environment.constraints()),
            Err(MachineEffectStageError::Aarch64Catalog(
                target_aarch64::Aarch64MachineEffectCatalogValidationError::TargetArchitectureMismatch
            ))
        ));
    }
}

#[cfg(test)]
pub(crate) mod indirect_call_tests {
    use std::sync::Arc;

    use register_environment::{
        ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
    };
    use selected_instructions::{
        MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedTrapBehavior,
        MachineMemoryEffect, MachineTrapBehavior, SelectedBlock, SelectedBlockId,
        SelectedBlockOrigin, SelectedCallContract, SelectedCallTarget, SelectedFunction,
        SelectedFunctionContracts, SelectedInstruction, SelectedInstructionId,
        SelectedInstructionKind, SelectedInstructionPlan, SelectedInstructionProvenance,
        SelectedOperand, SelectedProgram, SelectedProgramContracts, SelectedTerminator,
        VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, CallableContractId, EdgeId, MachineId, OperationId, ValueId, WorkScheduleIdentity,
    };
    use target::NativeTarget;
    use target_vocabulary::NativeCallTarget;

    use super::{MachineEffectError, prepared_machine_effect_catalog};

    pub(crate) fn selected_call(
        environment: &ValidatedTargetRegisterEnvironment,
        target: NativeTarget,
        callee: NativeCallTarget,
    ) -> SelectedProgram {
        let keys = environment.allocation_constraint_keys();
        let base = register_model::find_register_constraint(
            &environment.constraints().catalog().constraints,
            keys.call_unit[0],
        )
        .unwrap();
        assert!(base.operands.is_empty());
        let constraint = if callee.selector().is_some() {
            selected_instructions::calls::dynamic::instantiate_constraint(
                base,
                environment.physical(),
            )
            .unwrap()
        } else {
            base.clone()
        };
        let instruction =
            |id, kind, row: &register_model::RegisterInstructionConstraint| SelectedInstruction {
                id,
                kind,
                constraint: row.key,
                operands: row
                    .operands
                    .iter()
                    .map(|operand| SelectedOperand {
                        operand: operand.operand,
                        virtual_register: VirtualRegisterId(u32::from(operand.operand)),
                        access: operand.access,
                        class: operand.class,
                        fixed_view: operand.fixed_view,
                        tied_to: operand.tied_to,
                        early_clobber: operand.early_clobber,
                    })
                    .collect(),
                implicit_uses: row.implicit_uses.clone(),
                implicit_defs: row.implicit_defs.clone(),
                clobbers: row.clobbers.clone(),
                provenance: SelectedInstructionProvenance::default(),
            };
        let call_id = SelectedInstructionId(0);
        let call = instruction(
            call_id,
            SelectedInstructionKind::CallUnit {
                target: SelectedCallTarget::from_native(callee),
            },
            &constraint,
        );
        let return_constraint = register_model::find_register_constraint(
            &environment.constraints().catalog().constraints,
            keys.return_unit,
        )
        .unwrap();
        assert!(return_constraint.operands.is_empty());
        let machine = MachineId::new(1).unwrap();
        let function = SelectedFunction {
            contracts: Arc::new(SelectedFunctionContracts {
                machine,
                attachment: None,
                provenance: Default::default(),
                structural: None,
                structural_block_parameters: Vec::new(),
                local_storage_slots: Vec::new(),
                outgoing_arguments: Vec::new(),
                calls: vec![SelectedCallContract {
                    instruction: call_id,
                    owner: target_vocabulary::CallSiteOwner::Operation(
                        OperationId::new(1).unwrap(),
                    ),
                    call: target_vocabulary::LegalizedScalarCall {
                        descriptor_origins: Vec::new(),
                        access_bindings: Vec::new(),
                        source: target_vocabulary::NativeCallOrigin::Authored,
                        target: callee,
                        call_plan: calling_conventions::evaluate_call_plan(
                            calling_conventions::CallingPolicy::native_for_target(target),
                            &calling_conventions::CallSignature::default(),
                        )
                        .unwrap(),
                        arguments: Vec::new(),
                        result_placement: None,
                        structural_result: None,
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    effect: operation_evidence::provenance::EffectLink {
                        input: 0,
                        output: 1,
                    },
                    ownership: Vec::new(),
                }],
                normalized_foreign_calls: Vec::new(),
                memory_accesses: Vec::new(),
                boundary_settlements: Vec::new(),
                virtual_registers: constraint
                    .operands
                    .iter()
                    .map(|operand| VirtualRegister {
                        id: VirtualRegisterId(u32::from(operand.operand)),
                        scalar_type: callee.operand_type().unwrap(),
                        class: operand.class,
                        origin: VirtualRegisterOrigin::EntryParameter {
                            source_value: callee.selector().unwrap(),
                            parameter_index: 0,
                        },
                        definition_site: None,
                        entry_fixed_view: None,
                    })
                    .collect(),
            }),
            entry_block: SelectedBlockId(0),
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: vec![call],
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        SelectedInstructionId(1),
                        SelectedInstructionKind::ReturnUnit,
                        return_constraint,
                    ),
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }]
            .into(),
        };
        let work_schedule = WorkScheduleIdentity::new(1).unwrap();
        let plan = SelectedInstructionPlan {
            contracts: Arc::new(SelectedProgramContracts::default()),
            psi: terminal_psi::TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
            },
            work_schedule,
            target,
            entry: machine,
            functions: vec![function].into(),
        };
        SelectedProgram {
            identity: selected_instructions::selected_instruction_plan_identity(&plan),
            plan: Arc::new(plan),
            optimization_unit: optimization_core::OptimizationUnitIdentity::from_bytes([2; 32]),
            work_schedule,
        }
    }

    fn analyze(
        selected: &SelectedProgram,
        environment: &ValidatedTargetRegisterEnvironment,
    ) -> Result<super::ValidatedPreAllocationMachineEffects, MachineEffectError> {
        let catalog =
            prepared_machine_effect_catalog(selected.plan.target, environment.constraints())
                .unwrap();
        super::analyze_pre_allocation_machine_effects(
            selected,
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            &catalog,
        )
    }

    fn changed(
        selected: &SelectedProgram,
        change: impl FnOnce(&mut SelectedFunction),
    ) -> SelectedProgram {
        let mut selected = selected.clone();
        let plan = Arc::make_mut(&mut selected.plan);
        change(&mut plan.functions[0]);
        selected.identity = selected_instructions::selected_instruction_plan_identity(plan);
        selected
    }

    fn indirect_targets() -> [NativeCallTarget; 2] {
        [
            NativeCallTarget::Callable {
                callee: ValueId::new(1).unwrap(),
                contract: CallableContractId::new(1).unwrap(),
            },
            NativeCallTarget::Dynamic {
                selector: ValueId::new(1).unwrap(),
                interface: 1,
                slot: 2,
            },
        ]
    }

    #[test]
    fn indirect_calls_retain_selector_and_pointer_effects() {
        for target in [NativeTarget::macos_arm64(), NativeTarget::linux_x64()] {
            let environment = baseline_target_register_environment(target).unwrap();
            for callee in indirect_targets() {
                let selected = selected_call(&environment, target, callee);
                let effects = analyze(&selected, &environment).unwrap();
                let call = &effects.plan().functions[0].blocks[0].instructions[0];
                assert_eq!(call.memory, MachineMemoryEffect::ReadPointerV1);
                assert_eq!(call.trap, MachineTrapBehavior::MayArchitecturalFaultV1);
                assert!(!call.alternatives.is_empty());
                for alternative in &call.alternatives {
                    assert_eq!(
                        alternative.encoded.memory,
                        MachineEncodedMemoryEffect::ReadPointerV1 {
                            pointer_operand: 0,
                            byte_count: 8,
                        }
                    );
                    assert_eq!(
                        alternative.encoded.control,
                        MachineEncodedControlEffect::IndirectTableCallOrTrapV1
                    );
                    assert_eq!(
                        alternative.encoded.trap,
                        MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                    );
                    assert_eq!(alternative.encoded.external_operand_reads, vec![0]);
                }
            }
            let ordinary = selected_call(
                &environment,
                target,
                NativeCallTarget::Machine(MachineId::new(2).unwrap()),
            );
            let effects = analyze(&ordinary, &environment).unwrap();
            let call = &effects.plan().functions[0].blocks[0].instructions[0];
            assert_eq!(call.memory, MachineMemoryEffect::NoneV1);
            assert!(
                ordinary.plan.functions[0].blocks[0].instructions[0]
                    .operands
                    .is_empty()
            );
            assert!(call.alternatives.iter().all(|alternative| {
                alternative.encoded.control
                    != MachineEncodedControlEffect::IndirectTableCallOrTrapV1
            }));
        }
    }

    #[test]
    fn indirect_calls_refuse_inexact_selector_receipt_and_register_effects() {
        for target in [NativeTarget::macos_arm64(), NativeTarget::linux_x64()] {
            let environment = baseline_target_register_environment(target).unwrap();
            for callee in indirect_targets() {
                let selected = selected_call(&environment, target, callee);
                let missing_selector = changed(&selected, |function| {
                    function.blocks[0].instructions[0].operands.clear();
                });
                let wrong_selector = changed(&selected, |function| {
                    function.blocks[0].instructions[0].operands[0].fixed_view = None;
                });
                let register_missing = changed(&selected, |function| {
                    function.virtual_registers.clear();
                });
                let register_renumbered = changed(&selected, |function| {
                    function.virtual_registers[0].id = VirtualRegisterId(7);
                });
                let instruction = &selected.plan.functions[0].blocks[0].instructions[0];
                let fixed_view = instruction.operands[0].fixed_view.unwrap();
                for (invalid, reason) in [
                    (
                        missing_selector,
                        "operand count is 0, expected 1".to_owned(),
                    ),
                    (
                        wrong_selector,
                        format!(
                            "operand 0 fixed register view: expected Some({fixed_view:?}), found None"
                        ),
                    ),
                    (
                        register_missing,
                        "operand 0 refers to missing virtual register 0".to_owned(),
                    ),
                    (
                        register_renumbered,
                        "operand 0 virtual register ID: expected 0, found 7".to_owned(),
                    ),
                ] {
                    let error = analyze(&invalid, &environment).unwrap_err();
                    let report =
                        super::MachineEffectStageError::Analysis(error.clone()).to_string();
                    assert!(report.contains(&reason), "{report}");
                    assert_eq!(
                        error,
                        MachineEffectError::ConstraintOperandMismatch {
                            machine: selected.plan.functions[0].machine,
                            instruction: instruction.id,
                            semantic: super::machine_semantic_kind(instruction.kind),
                            constraint: instruction.constraint,
                            detail: reason,
                        }
                    );
                }
                let missing_receipt = changed(&selected, |function| function.calls.clear());
                let duplicate_receipt = changed(&selected, |function| {
                    let record = function.calls[0].clone();
                    function.calls.push(record);
                });
                let wrong_receipt = changed(&selected, |function| {
                    function.calls[0].call.target =
                        NativeCallTarget::Machine(MachineId::new(2).unwrap());
                });
                let missing_effects = changed(&selected, |function| {
                    let call = &mut function.blocks[0].instructions[0];
                    assert!(!call.clobbers.is_empty());
                    call.clobbers.clear();
                });
                for invalid in [
                    missing_receipt,
                    duplicate_receipt,
                    wrong_receipt,
                    missing_effects,
                ] {
                    assert!(matches!(
                        analyze(&invalid, &environment),
                        Err(MachineEffectError::ConstraintEffectMismatch {
                            instruction: SelectedInstructionId(0)
                        })
                    ));
                }
            }
        }
    }
}

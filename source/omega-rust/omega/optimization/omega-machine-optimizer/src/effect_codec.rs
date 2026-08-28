use omega_optimization_core::{AcceptedObligationFactIdentity, OptimizationUnitIdentity};
use omega_optimization_unit::{FuelSettlement, PsiProvenance};
use omega_register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
    RegisterUnitId, TargetRegisterEnvironmentIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternative, TerminalMachineAlternativeApplicability,
    TerminalMachineAlternativeFamily, TerminalMachineAlternativeKey, TerminalMachineBarrier,
    TerminalMachineCallEffect, TerminalMachineCleanupEffect, TerminalMachineEffectCatalogIdentity,
    TerminalMachineEncodedControlEffect, TerminalMachineEncodedEffects,
    TerminalMachineEncodedMemoryEffect, TerminalMachineEncodedStackEffect,
    TerminalMachineEncodedTrapBehavior, TerminalMachineLatencyKnowledge,
    TerminalMachineMemoryEffect, TerminalMachineSizeKnowledge, TerminalMachineTrapBehavior,
    TerminalSelectedBlockId, TerminalSelectedInstructionId, TerminalSelectedInstructionKind,
    TerminalSelectedInstructionPlanIdentity, TerminalSelectedInstructionProvenance,
};
use psi_core::{
    EdgeId, FuelScheduleIdentity, IntegerValue, MachineId, ObligationId, OperationId, ValueId,
};

use crate::{
    TerminalBlockMachineEffects, TerminalFunctionMachineEffects, TerminalInstructionMachineEffects,
    TerminalPreAllocationMachineEffectIdentity, TerminalPreAllocationMachineEffectPlan,
    terminal_pre_allocation_machine_effect_identity,
};

const MAGIC: &[u8; 8] = b"OMGMFX\0\0";
const VERSION: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalPreAllocationMachineEffectDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidField,
    InvalidIdentity,
    TrailingBytes,
}

impl std::fmt::Display for TerminalPreAllocationMachineEffectDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid pre-allocation machine-effect artifact: {self:?}"
        )
    }
}

impl std::error::Error for TerminalPreAllocationMachineEffectDecodeError {}

pub(crate) fn encode_terminal_pre_allocation_machine_effect_plan(
    plan: &TerminalPreAllocationMachineEffectPlan,
) -> Vec<u8> {
    let content =
        crate::effect_identity::encode_terminal_pre_allocation_machine_effect_content(plan);
    let mut encoded = Vec::with_capacity(44 + content.len());
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&plan.identity.bytes());
    encoded.extend_from_slice(&content);
    encoded
}

pub(crate) fn decode_terminal_pre_allocation_machine_effect_plan(
    encoded: &[u8],
) -> Result<TerminalPreAllocationMachineEffectPlan, TerminalPreAllocationMachineEffectDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(TerminalPreAllocationMachineEffectDecodeError::WrongMagic);
    }
    let version = cursor.u32()?;
    if version != VERSION {
        return Err(TerminalPreAllocationMachineEffectDecodeError::UnsupportedVersion(version));
    }
    let identity = TerminalPreAllocationMachineEffectIdentity::from_bytes(cursor.array()?);
    let selected = TerminalSelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
    let fuel_schedule = FuelScheduleIdentity::new(cursor.u32()?)
        .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)?;
    let target = decode_target(&mut cursor)?;
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
    let register_constraints = RegisterConstraintCatalogIdentity::from_bytes(cursor.array()?);
    let machine_effect_catalog = TerminalMachineEffectCatalogIdentity::from_bytes(cursor.array()?);
    let function_count = cursor.length()?;
    let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
    for _ in 0..function_count {
        let machine = MachineId::new(cursor.u64()?)
            .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)?;
        let block_count = cursor.length()?;
        let mut blocks = Vec::with_capacity(block_count.min(cursor.remaining()));
        for _ in 0..block_count {
            let block = TerminalSelectedBlockId(cursor.u32()?);
            let instruction_count = cursor.length()?;
            let mut instructions = Vec::with_capacity(instruction_count.min(cursor.remaining()));
            for _ in 0..instruction_count {
                instructions.push(decode_instruction(&mut cursor)?);
            }
            blocks.push(TerminalBlockMachineEffects {
                block,
                instructions,
            });
        }
        functions.push(TerminalFunctionMachineEffects { machine, blocks });
    }
    if cursor.remaining() != 0 {
        return Err(TerminalPreAllocationMachineEffectDecodeError::TrailingBytes);
    }
    let plan = TerminalPreAllocationMachineEffectPlan {
        identity,
        selected,
        optimization_unit,
        fuel_schedule,
        target,
        register_environment,
        register_constraints,
        machine_effect_catalog,
        functions,
    };
    if plan.identity != terminal_pre_allocation_machine_effect_identity(&plan) {
        return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidIdentity);
    }
    Ok(plan)
}

fn decode_instruction(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalInstructionMachineEffects, TerminalPreAllocationMachineEffectDecodeError> {
    let instruction = TerminalSelectedInstructionId(cursor.u32()?);
    let kind = decode_kind(cursor)?;
    let constraint = decode_constraint_key(cursor)?;
    let unit_uses = decode_units(cursor)?;
    let unit_defs = decode_units(cursor)?;
    let unit_clobbers = decode_units(cursor)?;
    if cursor.byte()? != 0 || cursor.byte()? != 0 {
        return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField);
    }
    let barrier = match cursor.byte()? {
        0 => TerminalMachineBarrier::None,
        1 => TerminalMachineBarrier::ControlFlow,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    if cursor.byte()? != 0 || cursor.byte()? != 0 {
        return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField);
    }
    let provenance = decode_provenance(cursor)?;
    let alternative_count = cursor.length()?;
    let mut alternatives = Vec::with_capacity(alternative_count.min(cursor.remaining()));
    for _ in 0..alternative_count {
        alternatives.push(decode_alternative(cursor)?);
    }
    Ok(TerminalInstructionMachineEffects {
        instruction,
        kind,
        constraint,
        unit_uses,
        unit_defs,
        unit_clobbers,
        memory: TerminalMachineMemoryEffect::NoneV1,
        trap: TerminalMachineTrapBehavior::NeverV1,
        barrier,
        call: TerminalMachineCallEffect::NoneV1,
        cleanup: TerminalMachineCleanupEffect::NoneV1,
        provenance,
        alternatives,
    })
}

fn decode_kind(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalSelectedInstructionKind, TerminalPreAllocationMachineEffectDecodeError> {
    Ok(match cursor.byte()? {
        0 => TerminalSelectedInstructionKind::CompareI64Zero,
        1 => TerminalSelectedInstructionKind::MaterializeI64 {
            value: decode_integer(cursor)?,
        },
        2 => TerminalSelectedInstructionKind::CopyI64,
        3 => TerminalSelectedInstructionKind::ExactAddI64 {
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        4 => TerminalSelectedInstructionKind::ExactAddI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        5 => TerminalSelectedInstructionKind::ExactSubtractI64 {
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        6 => TerminalSelectedInstructionKind::ConditionalBranchNonZero,
        7 => TerminalSelectedInstructionKind::ReturnI64,
        8 => TerminalSelectedInstructionKind::ExactSubtractI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    })
}

fn decode_integer(
    cursor: &mut Cursor<'_>,
) -> Result<IntegerValue, TerminalPreAllocationMachineEffectDecodeError> {
    match cursor.byte()? {
        0 => Ok(IntegerValue::Signed(i128::from_le_bytes(cursor.array()?))),
        1 => Ok(IntegerValue::Unsigned(u128::from_le_bytes(cursor.array()?))),
        _ => Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    }
}

fn decode_provenance(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalSelectedInstructionProvenance, TerminalPreAllocationMachineEffectDecodeError> {
    let operations = decode_ids(cursor, OperationId::new)?;
    let values = decode_ids(cursor, ValueId::new)?;
    let edges = decode_ids(cursor, EdgeId::new)?;
    let obligations = decode_ids(cursor, ObligationId::new)?;
    let fuel_count = cursor.length()?;
    let mut fuel = Vec::with_capacity(fuel_count.min(cursor.remaining()));
    for _ in 0..fuel_count {
        let site_tag = cursor.byte()?;
        let raw = cursor.u64()?;
        let site = match site_tag {
            0 => PsiProvenance::Operation(
                OperationId::new(raw)
                    .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)?,
            ),
            1 => PsiProvenance::Edge(
                EdgeId::new(raw)
                    .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)?,
            ),
            _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
        };
        fuel.push(FuelSettlement {
            site,
            units: cursor.u64()?,
        });
    }
    Ok(TerminalSelectedInstructionProvenance {
        operations,
        values,
        edges,
        obligations,
        fuel,
    })
}

pub(crate) fn decode_alternative(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalMachineAlternative, TerminalPreAllocationMachineEffectDecodeError> {
    let family = match cursor.byte()? {
        0 => TerminalMachineAlternativeFamily::CompareI64Zero,
        1 => TerminalMachineAlternativeFamily::MaterializeI64,
        2 => TerminalMachineAlternativeFamily::CopyI64,
        3 => TerminalMachineAlternativeFamily::ExactAddI64,
        4 => TerminalMachineAlternativeFamily::ExactAddI64Immediate,
        5 => TerminalMachineAlternativeFamily::ExactSubtractI64,
        6 => TerminalMachineAlternativeFamily::ConditionalBranchNonZero,
        7 => TerminalMachineAlternativeFamily::ReturnI64,
        8 => TerminalMachineAlternativeFamily::ExactSubtractI64Immediate,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let key = TerminalMachineAlternativeKey {
        family,
        variant: cursor.u32()?,
    };
    let applicability = match cursor.byte()? {
        0 => TerminalMachineAlternativeApplicability::Always,
        1 => TerminalMachineAlternativeApplicability::ResultAliasesOperand {
            result: cursor.u16()?,
            operand: cursor.u16()?,
        },
        2 => TerminalMachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result: cursor.u16()?,
            aliased_operand: cursor.u16()?,
            distinct_operand: cursor.u16()?,
        },
        3 => TerminalMachineAlternativeApplicability::ResultAliasesOperands {
            result: cursor.u16()?,
            left: cursor.u16()?,
            right: cursor.u16()?,
        },
        4 => TerminalMachineAlternativeApplicability::ResultDistinctFromOperands {
            result: cursor.u16()?,
            left: cursor.u16()?,
            right: cursor.u16()?,
        },
        5 => TerminalMachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
            left: cursor.u16()?,
            right: cursor.u16()?,
            excluded_view: omega_register_model::RegisterViewId(cursor.u16()?),
        },
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let size = match cursor.byte()? {
        0 => TerminalMachineSizeKnowledge::ExactBytes(cursor.u16()?),
        1 => {
            let minimum_bytes = cursor.u16()?;
            let maximum_bytes = match cursor.byte()? {
                0 => None,
                1 => Some(cursor.u16()?),
                _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
            };
            TerminalMachineSizeKnowledge::EncoderResolved {
                minimum_bytes,
                maximum_bytes,
            }
        }
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    if cursor.byte()? != 0 {
        return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField);
    }
    let encoded = decode_encoded_effects(cursor)?;
    Ok(TerminalMachineAlternative {
        key,
        applicability,
        size,
        latency: TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
        encoded,
    })
}

fn decode_encoded_effects(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalMachineEncodedEffects, TerminalPreAllocationMachineEffectDecodeError> {
    let external_operand_reads = decode_u16s(cursor)?;
    let external_operand_writes = decode_u16s(cursor)?;
    let implicit_unit_uses = decode_units(cursor)?;
    let implicit_unit_defs = decode_units(cursor)?;
    let implicit_unit_clobbers = decode_units(cursor)?;
    let memory = match cursor.byte()? {
        0 => TerminalMachineEncodedMemoryEffect::NoneV1,
        1 => TerminalMachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer: omega_register_model::RegisterViewId(cursor.u16()?),
            byte_count: cursor.u16()?,
        },
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let stack = match cursor.byte()? {
        0 => TerminalMachineEncodedStackEffect::UnchangedV1,
        1 => TerminalMachineEncodedStackEffect::PopBytesV1 {
            stack_pointer: omega_register_model::RegisterViewId(cursor.u16()?),
            byte_count: cursor.u16()?,
        },
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let trap = match cursor.byte()? {
        0 => TerminalMachineEncodedTrapBehavior::NeverV1,
        1 => TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let control = match cursor.byte()? {
        0 => TerminalMachineEncodedControlEffect::FallThroughV1,
        1 => TerminalMachineEncodedControlEffect::ConditionalRelativeBranchV1,
        2 => TerminalMachineEncodedControlEffect::ReturnFromActivationStackV1,
        3 => TerminalMachineEncodedControlEffect::ReturnIndirectRegisterV1 {
            target: omega_register_model::RegisterViewId(cursor.u16()?),
        },
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    Ok(TerminalMachineEncodedEffects {
        external_operand_reads,
        external_operand_writes,
        implicit_unit_uses,
        implicit_unit_defs,
        implicit_unit_clobbers,
        memory,
        stack,
        trap,
        control,
    })
}

fn decode_u16s(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<u16>, TerminalPreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut values = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        values.push(cursor.u16()?);
    }
    Ok(values)
}

pub(crate) fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, TerminalPreAllocationMachineEffectDecodeError> {
    let architecture = match cursor.byte()? {
        0 => Architecture::Aarch64,
        1 => Architecture::X86_64,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let object_format = match cursor.byte()? {
        0 => ObjectFormat::Elf,
        1 => ObjectFormat::MachO,
        2 => ObjectFormat::Coff,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let pointer_size = usize::try_from(cursor.u64()?)
        .map_err(|_| TerminalPreAllocationMachineEffectDecodeError::InvalidField)?;
    let pointer_alignment = usize::try_from(cursor.u64()?)
        .map_err(|_| TerminalPreAllocationMachineEffectDecodeError::InvalidField)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

fn decode_constraint_key(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterConstraintKey, TerminalPreAllocationMachineEffectDecodeError> {
    let family = match cursor.byte()? {
        0 => RegisterConstraintFamily::Call,
        1 => RegisterConstraintFamily::Return,
        2 => RegisterConstraintFamily::SystemCall,
        3 => RegisterConstraintFamily::InlineAssembly,
        4 => RegisterConstraintFamily::Instruction,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    Ok(RegisterConstraintKey {
        family,
        variant: cursor.u32()?,
    })
}

pub(crate) fn decode_units(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<RegisterUnitId>, TerminalPreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut units = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        units.push(RegisterUnitId(cursor.u16()?));
    }
    Ok(units)
}

fn decode_ids<T>(
    cursor: &mut Cursor<'_>,
    constructor: impl Fn(u64) -> Option<T>,
) -> Result<Vec<T>, TerminalPreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut values = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        values.push(
            constructor(cursor.u64()?)
                .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)?,
        );
    }
    Ok(values)
}

fn decode_obligation(
    cursor: &mut Cursor<'_>,
) -> Result<ObligationId, TerminalPreAllocationMachineEffectDecodeError> {
    ObligationId::new(cursor.u64()?)
        .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)
}

pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(crate) fn take(
        &mut self,
        count: usize,
    ) -> Result<&'a [u8], TerminalPreAllocationMachineEffectDecodeError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(TerminalPreAllocationMachineEffectDecodeError::Truncated)?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(TerminalPreAllocationMachineEffectDecodeError::Truncated)?;
        self.position = end;
        Ok(bytes)
    }

    pub(crate) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], TerminalPreAllocationMachineEffectDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| TerminalPreAllocationMachineEffectDecodeError::Truncated)
    }

    pub(crate) fn byte(&mut self) -> Result<u8, TerminalPreAllocationMachineEffectDecodeError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, TerminalPreAllocationMachineEffectDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, TerminalPreAllocationMachineEffectDecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, TerminalPreAllocationMachineEffectDecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(crate) fn length(
        &mut self,
    ) -> Result<usize, TerminalPreAllocationMachineEffectDecodeError> {
        usize::try_from(self.u64()?)
            .map_err(|_| TerminalPreAllocationMachineEffectDecodeError::InvalidField)
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::OptimizationUnitIdentity;
    use omega_optimization_unit::{FuelSettlement, PsiProvenance};
    use omega_register_model::{
        RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
        TargetRegisterEnvironmentIdentity,
    };
    use omega_terminal_selected_instructions::{
        TerminalMachineAlternativeKey, TerminalMachineEffectCatalogIdentity,
        TerminalSelectedInstructionPlanIdentity,
    };
    use psi_core::{EdgeId, FuelScheduleIdentity, MachineId, ObligationId, OperationId, ValueId};

    use super::*;

    fn plan() -> TerminalPreAllocationMachineEffectPlan {
        let mut plan = TerminalPreAllocationMachineEffectPlan {
            identity: TerminalPreAllocationMachineEffectIdentity::from_bytes([0; 32]),
            selected: TerminalSelectedInstructionPlanIdentity::from_bytes([1; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target: NativeTarget::linux_x64(),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
            register_constraints: RegisterConstraintCatalogIdentity::from_bytes([4; 32]),
            machine_effect_catalog: TerminalMachineEffectCatalogIdentity::from_bytes([5; 32]),
            functions: vec![TerminalFunctionMachineEffects {
                machine: MachineId::new(1).unwrap(),
                blocks: vec![TerminalBlockMachineEffects {
                    block: TerminalSelectedBlockId(0),
                    instructions: vec![TerminalInstructionMachineEffects {
                        instruction: TerminalSelectedInstructionId(0),
                        kind: TerminalSelectedInstructionKind::CompareI64Zero,
                        constraint: RegisterConstraintKey {
                            family: RegisterConstraintFamily::Instruction,
                            variant: 4,
                        },
                        unit_uses: vec![RegisterUnitId(0)],
                        unit_defs: vec![RegisterUnitId(1)],
                        unit_clobbers: vec![RegisterUnitId(2)],
                        memory: TerminalMachineMemoryEffect::NoneV1,
                        trap: TerminalMachineTrapBehavior::NeverV1,
                        barrier: TerminalMachineBarrier::None,
                        call: TerminalMachineCallEffect::NoneV1,
                        cleanup: TerminalMachineCleanupEffect::NoneV1,
                        provenance: TerminalSelectedInstructionProvenance {
                            operations: vec![OperationId::new(2).unwrap()],
                            values: vec![ValueId::new(3).unwrap()],
                            edges: vec![EdgeId::new(4).unwrap()],
                            obligations: vec![ObligationId::new(5).unwrap()],
                            fuel: vec![
                                FuelSettlement {
                                    site: PsiProvenance::Operation(OperationId::new(2).unwrap()),
                                    units: 7,
                                },
                                FuelSettlement {
                                    site: PsiProvenance::Edge(EdgeId::new(4).unwrap()),
                                    units: 11,
                                },
                            ],
                        },
                        alternatives: vec![
                            TerminalMachineAlternative {
                                key: TerminalMachineAlternativeKey {
                                    family: TerminalMachineAlternativeFamily::CompareI64Zero,
                                    variant: 0,
                                },
                                applicability: TerminalMachineAlternativeApplicability::Always,
                                size: TerminalMachineSizeKnowledge::ExactBytes(3),
                                latency:
                                    TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
                                encoded: TerminalMachineEncodedEffects::fallthrough_v1(
                                    vec![0],
                                    vec![],
                                ),
                            },
                            TerminalMachineAlternative {
                                key: TerminalMachineAlternativeKey {
                                    family: TerminalMachineAlternativeFamily::CompareI64Zero,
                                    variant: 1,
                                },
                                applicability: TerminalMachineAlternativeApplicability::
                                    ResultAliasesOperandAndDistinctFromOperand {
                                        result: 0,
                                        aliased_operand: 1,
                                        distinct_operand: 2,
                                    },
                                size: TerminalMachineSizeKnowledge::EncoderResolved {
                                    minimum_bytes: 2,
                                    maximum_bytes: Some(6),
                                },
                                latency:
                                    TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
                                encoded: TerminalMachineEncodedEffects::fallthrough_v1(
                                    vec![0, 1],
                                    vec![2],
                                ),
                            },
                            TerminalMachineAlternative {
                                key: TerminalMachineAlternativeKey {
                                    family: TerminalMachineAlternativeFamily::CompareI64Zero,
                                    variant: 2,
                                },
                                applicability: TerminalMachineAlternativeApplicability::
                                    AtLeastOneOperandDoesNotAliasView {
                                        left: 0,
                                        right: 1,
                                        excluded_view: omega_register_model::RegisterViewId(12),
                                    },
                                size: TerminalMachineSizeKnowledge::ExactBytes(4),
                                latency:
                                    TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
                                encoded: TerminalMachineEncodedEffects {
                                    external_operand_reads: vec![],
                                    external_operand_writes: vec![],
                                    implicit_unit_uses: vec![RegisterUnitId(0)],
                                    implicit_unit_defs: vec![RegisterUnitId(1)],
                                    implicit_unit_clobbers: vec![],
                                    memory:
                                        TerminalMachineEncodedMemoryEffect::ReadActivationStackV1 {
                                            stack_pointer:
                                                omega_register_model::RegisterViewId(12),
                                            byte_count: 8,
                                        },
                                    stack: TerminalMachineEncodedStackEffect::PopBytesV1 {
                                        stack_pointer: omega_register_model::RegisterViewId(12),
                                        byte_count: 8,
                                    },
                                    trap: TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                                    control: TerminalMachineEncodedControlEffect::ReturnFromActivationStackV1,
                                },
                            },
                        ],
                    }],
                }],
            }],
        };
        plan.identity = terminal_pre_allocation_machine_effect_identity(&plan);
        plan
    }

    #[test]
    fn codec_round_trips_complete_effect_content() {
        let source = plan();
        let encoded = source.encode();

        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&encoded).unwrap(),
            source
        );
    }

    #[test]
    fn codec_rejects_framing_corruption_and_stale_identity() {
        let source = plan();
        let encoded = source.encode();

        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&wrong_magic),
            Err(TerminalPreAllocationMachineEffectDecodeError::WrongMagic)
        );

        let mut unsupported_version = encoded.clone();
        unsupported_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&unsupported_version),
            Err(TerminalPreAllocationMachineEffectDecodeError::UnsupportedVersion(2))
        );

        let mut stale_identity = encoded.clone();
        stale_identity[12] ^= 1;
        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&stale_identity),
            Err(TerminalPreAllocationMachineEffectDecodeError::InvalidIdentity)
        );

        let mut invalid_target = encoded.clone();
        invalid_target[112] = u8::MAX;
        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&invalid_target),
            Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField)
        );

        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&encoded[..encoded.len() - 1]),
            Err(TerminalPreAllocationMachineEffectDecodeError::Truncated)
        );

        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&trailing),
            Err(TerminalPreAllocationMachineEffectDecodeError::TrailingBytes)
        );
    }
}

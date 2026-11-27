//! Selected literal-fold records for in-process selected-instruction rewrites.
use crate::{AllocatorAvailabilityIdentity, RecoveryClassificationIdentity, SpillChoiceIdentity};
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterConstraintKey, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    LiteralFoldIdentity, MachineEffectCatalogIdentity, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlan, VirtualRegisterId,
};
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use std::sync::Arc;
use wire::codec_bytes::push_encoded;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LiteralFoldPolicy {
    enabled_rules: u32,
}

impl LiteralFoldPolicy {
    const EXACT_ADD_BIT: u32 = 1 << 0;
    const EXACT_SUBTRACT_BIT: u32 = 1 << 1;
    const COMPARE_BIT: u32 = 1 << 2;
    const EXTENSION_BIT: u32 = 1 << 3;
    const LOAD8_INDEXED_BIT: u32 = 1 << 4;
    const COPY_BIT: u32 = 1 << 5;
    const BYTE_VIEW_ADDRESS_BIT: u32 = 1 << 6;
    const EXACT_DIVIDE_BIT: u32 = 1 << 7;
    const WRAPPING_REMAINDER_BIT: u32 = 1 << 8;
    const BITWISE_AND_ZERO_BIT: u32 = 1 << 9;
    const BITWISE_XOR_ZERO_BIT: u32 = 1 << 10;
    const WRAPPING_ADD_ZERO_BIT: u32 = 1 << 11;
    const BITWISE_AND_ONES_BIT: u32 = 1 << 12;
    const WRAPPING_REMAINDER_ZERO_BIT: u32 = 1 << 13;
    const EXACT_DIVIDE_ZERO_BIT: u32 = 1 << 14;
    const SATURATING_ADD_ZERO_BIT: u32 = 1 << 15;
    const SATURATING_SUBTRACT_ZERO_BIT: u32 = 1 << 16;
    const SATURATING_DIVIDE_ONE_BIT: u32 = 1 << 17;
    const SATURATING_DIVIDE_ZERO_BIT: u32 = 1 << 18;
    const SATURATING_SUBTRACT_ZERO_MINUEND_BIT: u32 = 1 << 19;
    const SATURATING_ADD_UPPER_BOUND_BIT: u32 = 1 << 20;
    const WRAPPING_REMAINDER_MINUS_ONE_BIT: u32 = 1 << 21;
    const SATURATING_SUBTRACT_UPPER_BOUND_BIT: u32 = 1 << 22;
    const KNOWN_BITS: u32 = Self::EXACT_ADD_BIT
        | Self::EXACT_SUBTRACT_BIT
        | Self::COMPARE_BIT
        | Self::EXTENSION_BIT
        | Self::LOAD8_INDEXED_BIT
        | Self::COPY_BIT
        | Self::BYTE_VIEW_ADDRESS_BIT
        | Self::EXACT_DIVIDE_BIT
        | Self::WRAPPING_REMAINDER_BIT
        | Self::BITWISE_AND_ZERO_BIT
        | Self::BITWISE_XOR_ZERO_BIT
        | Self::WRAPPING_ADD_ZERO_BIT
        | Self::BITWISE_AND_ONES_BIT
        | Self::WRAPPING_REMAINDER_ZERO_BIT
        | Self::EXACT_DIVIDE_ZERO_BIT
        | Self::SATURATING_ADD_ZERO_BIT
        | Self::SATURATING_SUBTRACT_ZERO_BIT
        | Self::SATURATING_DIVIDE_ONE_BIT
        | Self::SATURATING_DIVIDE_ZERO_BIT
        | Self::SATURATING_SUBTRACT_ZERO_MINUEND_BIT
        | Self::SATURATING_ADD_UPPER_BOUND_BIT
        | Self::WRAPPING_REMAINDER_MINUS_ONE_BIT
        | Self::SATURATING_SUBTRACT_UPPER_BOUND_BIT;

    pub const EXACT_ADD_V1: Self = Self {
        enabled_rules: Self::EXACT_ADD_BIT,
    };
    pub const EXACT_SUBTRACT_V1: Self = Self {
        enabled_rules: Self::EXACT_SUBTRACT_BIT,
    };
    pub const COMPARE_V1: Self = Self {
        enabled_rules: Self::COMPARE_BIT,
    };
    pub const EXTENSION_V1: Self = Self {
        enabled_rules: Self::EXTENSION_BIT,
    };
    pub const LOAD8_INDEXED_V1: Self = Self {
        enabled_rules: Self::LOAD8_INDEXED_BIT,
    };
    pub const COPY_V1: Self = Self {
        enabled_rules: Self::COPY_BIT,
    };
    pub const BYTE_VIEW_ADDRESS_V1: Self = Self {
        enabled_rules: Self::BYTE_VIEW_ADDRESS_BIT,
    };
    pub const EXACT_DIVIDE_V1: Self = Self {
        enabled_rules: Self::EXACT_DIVIDE_BIT,
    };
    pub const WRAPPING_REMAINDER_V1: Self = Self {
        enabled_rules: Self::WRAPPING_REMAINDER_BIT,
    };
    pub const BITWISE_AND_ZERO_V1: Self = Self {
        enabled_rules: Self::BITWISE_AND_ZERO_BIT,
    };
    pub const BITWISE_XOR_ZERO_V1: Self = Self {
        enabled_rules: Self::BITWISE_XOR_ZERO_BIT,
    };
    pub const WRAPPING_ADD_ZERO_V1: Self = Self {
        enabled_rules: Self::WRAPPING_ADD_ZERO_BIT,
    };
    pub const BITWISE_AND_ONES_V1: Self = Self {
        enabled_rules: Self::BITWISE_AND_ONES_BIT,
    };
    pub const WRAPPING_REMAINDER_ZERO_V1: Self = Self {
        enabled_rules: Self::WRAPPING_REMAINDER_ZERO_BIT,
    };
    pub const EXACT_DIVIDE_ZERO_V1: Self = Self {
        enabled_rules: Self::EXACT_DIVIDE_ZERO_BIT,
    };
    pub const SATURATING_ADD_ZERO_V1: Self = Self {
        enabled_rules: Self::SATURATING_ADD_ZERO_BIT,
    };
    pub const SATURATING_SUBTRACT_ZERO_V1: Self = Self {
        enabled_rules: Self::SATURATING_SUBTRACT_ZERO_BIT,
    };
    pub const SATURATING_DIVIDE_ONE_V1: Self = Self {
        enabled_rules: Self::SATURATING_DIVIDE_ONE_BIT,
    };
    pub const SATURATING_DIVIDE_ZERO_V1: Self = Self {
        enabled_rules: Self::SATURATING_DIVIDE_ZERO_BIT,
    };
    pub const SATURATING_SUBTRACT_ZERO_MINUEND_V1: Self = Self {
        enabled_rules: Self::SATURATING_SUBTRACT_ZERO_MINUEND_BIT,
    };
    pub const SATURATING_ADD_UPPER_BOUND_V1: Self = Self {
        enabled_rules: Self::SATURATING_ADD_UPPER_BOUND_BIT,
    };
    pub const WRAPPING_REMAINDER_MINUS_ONE_V1: Self = Self {
        enabled_rules: Self::WRAPPING_REMAINDER_MINUS_ONE_BIT,
    };
    pub const SATURATING_SUBTRACT_UPPER_BOUND_V1: Self = Self {
        enabled_rules: Self::SATURATING_SUBTRACT_UPPER_BOUND_BIT,
    };

    pub const fn empty() -> Self {
        Self { enabled_rules: 0 }
    }

    pub const fn contains(self, other: Self) -> bool {
        other.enabled_rules != 0 && self.enabled_rules & other.enabled_rules == other.enabled_rules
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            enabled_rules: self.enabled_rules | other.enabled_rules,
        }
    }

    pub const fn canonical_bits(self) -> u32 {
        self.enabled_rules
    }

    pub const fn from_canonical_bits(bits: u32) -> Option<Self> {
        if bits == 0 || bits & !Self::KNOWN_BITS != 0 {
            None
        } else {
            Some(Self {
                enabled_rules: bits,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralFoldPlan {
    pub source_selected: Arc<SelectedInstructionPlan>,
    pub spill_choices: SpillChoiceIdentity,
    pub recovery_classifications: RecoveryClassificationIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub machine_effect_catalog: MachineEffectCatalogIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub policy: LiteralFoldPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionLiteralFold>,
    pub transformed_selected: Arc<SelectedInstructionPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLiteralFold {
    pub machine: MachineId,
    pub actions: Vec<LiteralFoldAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralFoldAction {
    pub block: SelectedBlockId,
    pub literal_instruction: SelectedInstructionId,
    pub victim: VirtualRegisterId,
    pub consumer_instruction: SelectedInstructionId,
    pub surviving: VirtualRegisterId,
    pub result: Option<VirtualRegisterId>,
    pub immediate: u64,
    pub immediate_constraint: RegisterConstraintKey,
}

pub fn literal_fold_identity(plan: &LiteralFoldPlan) -> LiteralFoldIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-literal-fold.v10\0");
    bytes.extend_from_slice(&encode_terminal_literal_fold_content(plan));
    LiteralFoldIdentity::from_canonical_bytes(&bytes)
}

fn encode_terminal_literal_fold_content(plan: &LiteralFoldPlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.spill_choices.bytes());
    bytes.extend_from_slice(&plan.recovery_classifications.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.machine_effect_catalog.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    push_encoded(&mut bytes, plan.work_schedule.marker().to_le_bytes());
    push_encoded(&mut bytes, plan.policy.canonical_bits().to_le_bytes());
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        push_encoded(&mut bytes, function.machine.get().to_le_bytes());
        length(&mut bytes, function.actions.len());
        for action in &function.actions {
            push_encoded(&mut bytes, action.block.0.to_le_bytes());
            push_encoded(&mut bytes, action.literal_instruction.0.to_le_bytes());
            push_encoded(&mut bytes, action.victim.0.to_le_bytes());
            push_encoded(&mut bytes, action.consumer_instruction.0.to_le_bytes());
            push_encoded(&mut bytes, action.surviving.0.to_le_bytes());
            match action.result {
                None => bytes.push(0),
                Some(result) => {
                    bytes.push(1);
                    push_encoded(&mut bytes, result.0.to_le_bytes());
                }
            }
            push_encoded(&mut bytes, action.immediate.to_le_bytes());
            register_model::encode_constraint_key(&mut bytes, action.immediate_constraint);
        }
    }
    bytes
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("literal-fold identity length fits u64")
            .to_le_bytes(),
    );
}

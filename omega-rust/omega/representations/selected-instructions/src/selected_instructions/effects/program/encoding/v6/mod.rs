//! Optimizer module role: stage group. V6-V9 pre-allocation machine-effect wire vocabulary.
//!
//! `framing` owns exact envelope and field order. Named payload leaves retain
//! independent structural, instruction, ownership, and shared-value decode
//! boundaries without changing rejection order.

use optimization_core::{AcceptedObligationFactIdentity, OptimizationUnitIdentity};

use super::super::identity;
use super::{Cursor, PreAllocationMachineEffectDecodeError};
use crate::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalogIdentity, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSizeKnowledge, MachineTrapBehavior,
    SelectedBlockId, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
    SelectedMicrosoftX64OwnedIndirectPairLayout, SelectedStructuralUnitIndirectBinding,
    StructuralUnitCallBarrier, StructuralUnitCallEffect, StructuralUnitCallEffectDeclaration,
    StructuralUnitCallFrameEffect, StructuralUnitCallMemoryEffect,
};
use optimization_unit::{EffectLink, FuelSettlement, OwnershipEvent, PsiProvenance};
use register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
    RegisterUnitId, TargetRegisterEnvironmentIdentity,
};
use semantic_vocabulary::{
    ClaimId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId, ObligationId, OperationId,
    PlaceId, StructuralTypeId, ValueId,
};
use target::{Architecture, NativeTarget, ObjectFormat};

use crate::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects,
    PreAllocationMachineEffectIdentity, PreAllocationMachineEffectPlan,
    StructuralUnitCallMachineEffects, StructuralUnitFunctionMachineEffects,
    pre_allocation_machine_effect_identity,
};

mod framing;
mod instruction;
mod ownership;
mod structural;
mod values;

pub use framing::{
    decode_terminal_pre_allocation_machine_effect_plan,
    encode_terminal_pre_allocation_machine_effect_plan,
};
use instruction::decode_instruction;
pub use instruction::{
    decode_alternative, decode_alternative_legacy, decode_alternative_without_jump,
    decode_alternative_without_scalar_call, decode_provenance,
};
pub use ownership::decode_ownership;
use structural::decode_structural_function;
pub use structural::{decode_effect_link, decode_structural_call};
use values::{decode_constraint_key, decode_ids, decode_machine, decode_obligation};
pub use values::{decode_target, decode_units};

const MAGIC: &[u8; 8] = b"OMGMFX\0\0";
const LEGACY_V6_VERSION: u32 = 6;
const LEGACY_V7_VERSION: u32 = 7;
const LEGACY_V8_VERSION: u32 = 8;
const LEGACY_V9_VERSION: u32 = 9;
const VERSION: u32 = 10;

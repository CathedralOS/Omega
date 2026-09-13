//! Optimizer module role: stage group. Current wire vocabulary in the retained V6 payload taxonomy.
//!
//! `framing` owns exact envelope and field order. Named payload leaves retain
//! independent structural, instruction, ownership, and shared-value decode
//! boundaries without changing rejection order.
//! Version 26 includes owned parameter-home slots. The shared slot decoder also
//! feeds physical plans, whose envelope must fence this vocabulary separately.
//! Outgoing frame tag 0 retains its exact argument bytes; tag 3 adds value copies.
//! Earlier readers reject tag 3, so this extension does not reinterpret old data.

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
    pre_allocation_machine_effect_identity,
};

mod framing;
mod instruction;
mod ownership;
mod values;

pub use framing::{
    decode_terminal_pre_allocation_machine_effect_plan,
    encode_terminal_pre_allocation_machine_effect_plan,
};
use instruction::decode_instruction;
pub use instruction::{
    decode_alternative, decode_alternative_legacy, decode_alternative_without_jump,
    decode_alternative_without_scalar_call, decode_local_storage_slot, decode_provenance,
};
pub use ownership::decode_effect_link;
pub use ownership::decode_ownership;
use values::{decode_constraint_key, decode_ids, decode_machine, decode_obligation};
pub use values::{decode_target, decode_units};

const MAGIC: &[u8; 8] = b"OMGMFX\0\0";
const VERSION: u32 = 26;

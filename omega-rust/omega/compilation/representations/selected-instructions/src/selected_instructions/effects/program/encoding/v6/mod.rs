//! Optimizer module role: stage group. Current wire vocabulary in the retained V6 payload taxonomy.
//!
//! `framing` owns exact envelope and field order. Named payload leaves retain
//! independent structural, instruction, ownership, and shared-value decode
//! boundaries without changing rejection order.
//! Version 26 includes owned parameter-home slots. The shared slot decoder also
//! feeds physical plans, whose envelope must fence this vocabulary separately.
//! Outgoing frame tag 0 retains its exact argument bytes; tag 3 adds value copies.
//! Earlier readers reject tag 3, so this extension does not reinterpret old data.
//! Version 29 adds the closed FloatResultPolicy family at tag 231. Current-only
//! framing rejects older envelopes even when their rows need no new tag. The
//! content identity domain stays unchanged because existing row bytes retain
//! their denotation; the envelope version is not part of that content.
//! Version 34 retains the owned subtree path of nominal cleanup actions.
//! Version 33 aligns cleanup runtime-index paths with the fixed-view action codec.
//! Older envelopes are rejected before the new path vocabulary is interpreted.
//! Version 32 retains explicit unbounded target retry work/control for bitwise atomics.
//! Version 31 retains exact consumed claim identities on nominal cleanup effects.
//! Version 30 adds supervisor call/control effects. Kernel entry has no
//! user-space return-address lifecycle; it cannot decode as an imported call.

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
use optimization_unit::{FuelSettlement, OwnershipEvent, PsiProvenance};
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

pub(crate) use framing::{
    decode_terminal_pre_allocation_machine_effect_plan,
    encode_terminal_pre_allocation_machine_effect_plan,
};
use instruction::decode_instruction;
pub use instruction::{
    decode_alternative, decode_alternative_legacy, decode_alternative_without_jump,
    decode_alternative_without_scalar_call, decode_local_storage_slot, decode_provenance,
    saturating_kind,
};
pub use ownership::decode_ownership;
use values::{
    decode_accepted_fact, decode_constraint_key, decode_ids, decode_machine, decode_obligation,
};
pub use values::{decode_target, decode_units};

const MAGIC: &[u8; 8] = b"OMGMFX\0\0";
// Version 35 retains tagged internal call targets and immutable image owners.
// Completion events retain the exact argument-relative ownership path.
// Older envelopes contained claim IDs only and cannot be reinterpreted.
// Version 39 widens semantic alternative-family tags, not instruction-kind
// tags or closed form payloads. Older one-byte families are never reinterpreted.
const VERSION: u32 = 40;

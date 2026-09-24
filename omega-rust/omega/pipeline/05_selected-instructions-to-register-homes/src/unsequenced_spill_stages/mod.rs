//! Optimizer module role: stage group. Spill boundaries validated but not yet sequenced by register allocation.
//!
//! `crate::register_allocation::stage_register_allocation` calls none of the
//! families below. Executable pressure recovery on the sequenced route is
//! `crate::assignment::runtime_spill`; these families are the bounded,
//! independently replayed spill boundaries that would extend it: abstract
//! spill insertion, reload-value and synthetic reload-value homes, recursive
//! and generalized recovery worklists, victim choices and logical actions,
//! spill-pseudo lowering, and abstract spill memory effects and access
//! constraints. Logical spill planning under
//! `crate::assignment::logical_spill_operations` and stack-slot coloring over
//! that plan under `crate::assignment::stack_slot_coloring` are likewise
//! validated but unsequenced: runtime-spill recovery rewrites physical
//! victims directly, so no executable route produces their plans.
//!
//! They are exercised by the native-differential `register_allocation` tests
//! and the architecture ladders, and machine emission's non-authoritative
//! spill-frame requirements accept the access-constraint output, but no
//! executable route produces these facts. Sequencing one means calling it from
//! `stage_register_allocation`, not re-declaring it beside the live stages.

pub mod abstract_spill_access_constraints;
pub mod abstract_spill_insertion;
pub mod abstract_spill_memory_effects;
pub mod generalized_reload_value_homes;
pub mod generalized_spill_insertion;
pub mod generalized_spill_recovery_actions;
pub mod generalized_spill_recovery_choice;
pub mod generalized_spill_recovery_worklist;
pub mod recursive_reload_value_homes;
pub mod recursive_spill_insertion;
pub mod reload_value_homes;
pub mod spill_pseudo_instructions;
pub mod spill_recovery_actions;
pub mod spill_recovery_choice;
pub mod spill_recovery_worklist;
pub mod synthetic_reload_values;

pub use abstract_spill_access_constraints::{
    AbstractSpillAccessConstraintError, AbstractSpillAccessConstraintPlan,
    AbstractSpillAccessConstraintPlanIdentity, AbstractSpillAccessConstraintPolicy,
    AbstractSpillAccessConstraintReceipt, AbstractSpillAccessDependency,
    AbstractSpillAccessDependencyReason, AbstractSpillAccessKind, AbstractSpillAccessPlacement,
    FunctionAbstractSpillAccessConstraints, ValidatedAbstractSpillAccessConstraints,
    abstract_spill_access_constraint_plan_identity, constrain_abstract_spill_accesses,
    validate_abstract_spill_access_constraints,
};
pub use abstract_spill_insertion::{
    AbstractSpillAreaReload, AbstractSpillAreaSlot, AbstractSpillAreaStore,
    AbstractSpillInsertionAction, AbstractSpillInsertionError, AbstractSpillInsertionIdentity,
    AbstractSpillInsertionPlan, AbstractSpillInsertionPolicy, AbstractSpillInsertionReceipt,
    FunctionAbstractSpillInsertion, ValidatedAbstractSpillInsertion,
    abstract_spill_insertion_identity, schedule_abstract_spill_insertion,
    validate_abstract_spill_insertion,
};
pub use abstract_spill_memory_effects::{
    AbstractSpillMemoryEffect, AbstractSpillMemoryEffectError, AbstractSpillMemoryEffectPlan,
    AbstractSpillMemoryEffectPlanIdentity, AbstractSpillMemoryEffectPolicy,
    AbstractSpillMemoryEffectReceipt, FunctionAbstractSpillMemoryEffects,
    ValidatedAbstractSpillMemoryEffects, abstract_spill_memory_effect_plan_identity,
    derive_abstract_spill_memory_effects, validate_abstract_spill_memory_effects,
};
pub use generalized_reload_value_homes::{
    FunctionGeneralizedReloadValueHomes, GeneralizedReloadCoexistingHome,
    GeneralizedReloadCoexistingValue, GeneralizedReloadValueHomeAssignment,
    GeneralizedReloadValueHomeError, GeneralizedReloadValueHomeIdentity,
    GeneralizedReloadValueHomeOutcome, GeneralizedReloadValueHomePlan,
    GeneralizedReloadValueHomePolicy, GeneralizedReloadValueHomeReceipt,
    GeneralizedReloadValuePressure, ValidatedGeneralizedReloadValueHomes,
    assign_generalized_reload_value_homes, generalized_reload_value_home_identity,
    validate_generalized_reload_value_homes,
};
pub use generalized_spill_insertion::{
    FunctionGeneralizedSpillInsertion, GeneralizedSpillActionId, GeneralizedSpillActionSource,
    GeneralizedSpillEvent, GeneralizedSpillInsertionError, GeneralizedSpillInsertionIdentity,
    GeneralizedSpillInsertionPlan, GeneralizedSpillInsertionPolicy,
    GeneralizedSpillInsertionReceipt, GeneralizedSpillSlot, ValidatedGeneralizedSpillInsertion,
    generalized_spill_insertion_identity, schedule_generalized_spill_insertion,
    validate_generalized_spill_insertion,
};
pub use generalized_spill_recovery_actions::{
    GeneralizedSpillRecoveryActionError, GeneralizedSpillRecoveryActionIdentity,
    GeneralizedSpillRecoveryActionPlan, GeneralizedSpillRecoveryActionPolicy,
    GeneralizedSpillRecoveryActionReceipt, GeneralizedSpillRecoveryLogicalAction,
    GeneralizedSpillRecoveryLogicalReload, GeneralizedSpillRecoveryLogicalStorage,
    GeneralizedSpillRecoveryLogicalStore, GeneralizedSpillRecoveryLogicalUseRewrite,
    GeneralizedSpillRecoveryVictim, ValidatedGeneralizedSpillRecoveryActions,
    generalized_spill_recovery_action_identity, plan_generalized_original_spill_recovery_actions,
    plan_generalized_spill_recovery_actions, validate_generalized_original_spill_recovery_actions,
    validate_generalized_spill_recovery_actions,
};
pub use generalized_spill_recovery_choice::{
    GeneralizedSpillRecoveryChoiceError, GeneralizedSpillRecoveryChoiceIdentity,
    GeneralizedSpillRecoveryChoicePlan, GeneralizedSpillRecoveryChoicePolicy,
    GeneralizedSpillRecoveryChoiceReceipt, GeneralizedSpillRecoveryContender,
    GeneralizedSpillRecoveryResident, GeneralizedSpillRecoveryVictimChoice,
    ValidatedGeneralizedSpillRecoveryChoices, choose_generalized_spill_recovery_victims,
    generalized_spill_recovery_choice_identity, validate_generalized_spill_recovery_choices,
};
pub use generalized_spill_recovery_worklist::{
    FunctionGeneralizedSpillRecoveryWorklist, GeneralizedSpillRecoveryWorkItem,
    GeneralizedSpillRecoveryWorkItemId, GeneralizedSpillRecoveryWorklistError,
    GeneralizedSpillRecoveryWorklistIdentity, GeneralizedSpillRecoveryWorklistPlan,
    GeneralizedSpillRecoveryWorklistPolicy, GeneralizedSpillRecoveryWorklistReceipt,
    ValidatedGeneralizedSpillRecoveryWorklist, generalized_spill_recovery_worklist_identity,
    seed_generalized_spill_recovery_worklist, validate_generalized_spill_recovery_worklist,
};
pub use recursive_reload_value_homes::{
    FunctionRecursiveReloadValueHomes, RecursiveReloadCoexistingHome,
    RecursiveReloadCoexistingValue, RecursiveReloadValueHomeAssignment,
    RecursiveReloadValueHomeError, RecursiveReloadValueHomeIdentity, RecursiveReloadValueHomePlan,
    RecursiveReloadValueHomePolicy, RecursiveReloadValueHomeReceipt,
    ValidatedRecursiveReloadValueHomes, assign_recursive_reload_value_homes,
    recursive_reload_value_home_identity, validate_recursive_reload_value_homes,
};
pub use recursive_spill_insertion::{
    FunctionRecursiveSpillInsertion, RecursiveSpillActionSource, RecursiveSpillEvent,
    RecursiveSpillInsertionError, RecursiveSpillInsertionIdentity, RecursiveSpillInsertionPlan,
    RecursiveSpillInsertionPolicy, RecursiveSpillInsertionReceipt, RecursiveSpillSlot,
    RecursiveSpillStoredValue, ValidatedRecursiveSpillInsertion,
    recursive_spill_insertion_identity, schedule_recursive_spill_insertion,
    validate_recursive_spill_insertion,
};
pub use reload_value_homes::{
    FunctionReloadValueHomes, ReloadCoexistingHome, ReloadValueHomeAssignment,
    ReloadValueHomeError, ReloadValueHomeIdentity, ReloadValueHomePlan, ReloadValueHomePolicy,
    ReloadValueHomeReceipt, ValidatedReloadValueHomes, assign_reload_value_homes,
    reload_value_home_identity, validate_reload_value_homes,
};
pub use spill_pseudo_instructions::{
    FunctionHomedSpillPseudoInstructions, FunctionSpillPseudoInstructions,
    HomedSpillPseudoInstruction, HomedSpillPseudoInstructionError, HomedSpillPseudoInstructionPlan,
    HomedSpillPseudoInstructionPlanIdentity, HomedSpillPseudoInstructionPolicy,
    HomedSpillPseudoInstructionReceipt, SpillPseudoInstruction, SpillPseudoInstructionError,
    SpillPseudoInstructionId, SpillPseudoInstructionPlan, SpillPseudoInstructionPlanIdentity,
    SpillPseudoInstructionPolicy, SpillPseudoInstructionReceipt, SpillPseudoOperandRewrite,
    SpillPseudoStorage, SpillPseudoStoredValue, ValidatedHomedSpillPseudoInstructions,
    ValidatedSpillPseudoInstructions, homed_spill_pseudo_instruction_plan_identity,
    lower_homed_recursive_spill_pseudos, lower_recursive_spill_pseudos,
    spill_pseudo_instruction_plan_identity, validate_homed_spill_pseudo_instructions,
    validate_spill_pseudo_instructions,
};
pub use spill_recovery_actions::{
    SpillRecoveryActionError, SpillRecoveryActionIdentity, SpillRecoveryActionPlan,
    SpillRecoveryActionPolicy, SpillRecoveryActionReceipt, SpillRecoveryLogicalAction,
    SpillRecoveryLogicalReload, SpillRecoveryLogicalReloadId, SpillRecoveryLogicalStorage,
    SpillRecoveryLogicalStorageId, SpillRecoveryLogicalStore, SpillRecoveryLogicalUseRewrite,
    ValidatedSpillRecoveryActions, plan_spill_recovery_actions, spill_recovery_action_identity,
    validate_spill_recovery_actions,
};
pub use spill_recovery_choice::{
    SpillRecoveryChoiceError, SpillRecoveryChoiceIdentity, SpillRecoveryChoicePlan,
    SpillRecoveryChoicePolicy, SpillRecoveryChoiceReceipt, SpillRecoveryContender,
    SpillRecoveryResident, SpillRecoveryVictimChoice, ValidatedSpillRecoveryChoices,
    choose_spill_recovery_victims, spill_recovery_choice_identity, validate_spill_recovery_choices,
};
pub use spill_recovery_worklist::{
    SpillRecoveryEpoch, SpillRecoveryWorkItem, SpillRecoveryWorklistError,
    SpillRecoveryWorklistIdentity, SpillRecoveryWorklistPlan, SpillRecoveryWorklistPolicy,
    SpillRecoveryWorklistReceipt, ValidatedSpillRecoveryWorklist, seed_spill_recovery_worklist,
    spill_recovery_worklist_identity, validate_spill_recovery_worklist,
};
pub use synthetic_reload_values::{
    FunctionSyntheticReloadValues, SyntheticReloadValueBinding, SyntheticReloadValueError,
    SyntheticReloadValueId, SyntheticReloadValuePlan, SyntheticReloadValuePlanIdentity,
    SyntheticReloadValuePolicy, SyntheticReloadValueReceipt, ValidatedSyntheticReloadValues,
    bind_synthetic_reload_values, synthetic_reload_value_plan_identity,
    validate_synthetic_reload_values,
};

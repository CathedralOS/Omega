use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FunctionAllocationLegality,
    FunctionRecoveryClassification, PressureRecoveryClassification, RecoveryClassification,
    RecoveryClassificationPlan, RecoveryClassificationPolicy, RecoveryFutureUse,
    RecoveryVictimRole, SpillChoiceIdentity, VirtualPointLegality,
    VirtualRegisterAllocationLegality,
};
use selected_instructions::{
    BlockPointDomain, FunctionLiveRanges, LiveRangeFragment, LiveRangeIdentity, LiveRangePlan,
    LiveRangePoint, LivenessIdentity, LivenessPosition, VirtualLiveRange, VirtualOccurrence,
};

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use register_model::{
    PhysicalRegisterModel, RegisterClass, RegisterClassId, RegisterConstraintFamily,
    RegisterConstraintId, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, RegisterOperandConstraint, RegisterUnit, RegisterUnitId,
    RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
    TargetRegisterEnvironmentIdentity, validate_physical_register_model,
};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, SelectedOperand, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ValueId,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

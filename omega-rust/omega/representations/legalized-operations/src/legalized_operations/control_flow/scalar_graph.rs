//! Ordinary scalar instructions and explicit return roles, independent of caller topology.
use calling_conventions::{CallPlan, ValuePlacement};
use optimization_unit::{EffectLink, FuelSettlement, OwnershipEvent, ValueDefinitionSite};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerType, IntegerValue, MachineId, ObligationId, OperationId,
    StructuralTypeId, ValueId,
};
use target_operations::TerminalPsiProvenance;
use terminal_psi::CrashRouteBucket;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub call_plan: CallPlan,
    pub parameters: Vec<LegalizedScalarParameter>,
    pub entry_block: BlockId,
    pub blocks: Vec<LegalizedScalarBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarParameter {
    pub value: ValueId,
    pub scalar_type: IntegerType,
    pub definition_site: ValueDefinitionSite,
    pub placement: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarBlock {
    pub id: BlockId,
    pub instructions: Vec<LegalizedScalarInstruction>,
    pub terminator: LegalizedScalarReturn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarInstruction {
    pub operation: OperationId,
    pub result: ValueId,
    pub scalar_type: IntegerType,
    pub definition_site: ValueDefinitionSite,
    pub kind: LegalizedScalarInstructionKind,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedScalarInstructionKind {
    Constant(IntegerValue),
    Call(LegalizedScalarCall),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarCall {
    pub callee: MachineId,
    pub call_plan: CallPlan,
    pub arguments: Vec<LegalizedScalarArgument>,
    pub result_placement: ValuePlacement,
    pub requirement_obligations: Vec<ObligationId>,
    pub crash_continuations: Vec<CrashRouteBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarArgument {
    pub source: ValueId,
    pub placement: ValuePlacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedScalarReturnValue {
    Unit,
    Value {
        value: ValueId,
        scalar_type: IntegerType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarReturn {
    pub edge: EdgeId,
    pub value: LegalizedScalarReturnValue,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

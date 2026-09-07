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

impl LegalizedScalarFunction {
    /// Whether a retained instruction or return reads this value.
    pub fn references_value(&self, value: ValueId) -> bool {
        self.blocks.iter().any(|block| {
            block.instructions.iter().any(|instruction| match &instruction.kind {
                LegalizedScalarInstructionKind::Constant(_) => false,
                LegalizedScalarInstructionKind::Call(call) =>
                    call.arguments.iter().any(|argument| argument.source == value),
                LegalizedScalarInstructionKind::ExactBinary { left, right, .. } =>
                    *left == value || *right == value,
            }) || matches!(block.terminator.value, LegalizedScalarReturnValue::Value { value: returned, .. } if returned == value)
        })
    }
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
    ExactBinary {
        operator: super::super::LegalizedExactIntegerOperator,
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
        accepted_fact: optimization_core::AcceptedObligationFactIdentity,
    },
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

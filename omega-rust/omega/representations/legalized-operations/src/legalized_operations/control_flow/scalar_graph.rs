//! Ordinary scalar instructions, block parameters and explicit control edges.
use abstract_operations::ValueBinding;
use calling_conventions::{CallPlan, ValuePlacement};
use optimization_unit::{EffectLink, FuelSettlement, OwnershipEvent, ValueDefinitionSite};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerType, IntegerValue, MachineId, ObligationId, OperationId, ScalarType,
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
    /// Whether a retained instruction, terminator or outgoing edge reads this value.
    pub fn references_value(&self, value: ValueId) -> bool {
        self.blocks.iter().any(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| match &instruction.kind {
                    LegalizedScalarInstructionKind::Constant(_) => false,
                    LegalizedScalarInstructionKind::Call(call) => call
                        .arguments
                        .iter()
                        .any(|argument| argument.source == value),
                    LegalizedScalarInstructionKind::ExactBinary { left, right, .. }
                    | LegalizedScalarInstructionKind::Compare { left, right, .. } => {
                        *left == value || *right == value
                    }
                })
                || block.terminator.references_value(value)
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
    pub parameters: Vec<optimization_unit::ValueDefinition>,
    pub instructions: Vec<LegalizedScalarInstruction>,
    pub terminator: LegalizedScalarTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarInstruction {
    pub operation: OperationId,
    pub result: ValueId,
    pub scalar_type: ScalarType,
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
    Compare {
        predicate: LegalizedScalarComparison,
        operand_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedScalarComparison {
    Equal,
    LessThan,
    LessOrEqual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarSuccessor {
    pub edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<ValueBinding>,
    pub fuel: Vec<FuelSettlement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedScalarTerminator {
    Return(LegalizedScalarReturn),
    Jump {
        successor: LegalizedScalarSuccessor,
        effect: EffectLink,
        ownership: Vec<OwnershipEvent>,
    },
    Conditional {
        condition: ValueId,
        when_true: LegalizedScalarSuccessor,
        when_false: LegalizedScalarSuccessor,
        effect: EffectLink,
        ownership: Vec<OwnershipEvent>,
    },
}

impl LegalizedScalarTerminator {
    /// Reads on the outgoing edges count even when the destination drops the value.
    pub fn references_value(&self, value: ValueId) -> bool {
        let binds = |successor: &LegalizedScalarSuccessor| {
            successor
                .bindings
                .iter()
                .any(|binding| binding.argument == value)
        };
        match self {
            Self::Return(returned) => matches!(returned.value,
                LegalizedScalarReturnValue::Value { value: returned, .. } if returned == value),
            Self::Jump { successor, .. } => binds(successor),
            Self::Conditional {
                condition,
                when_true,
                when_false,
                ..
            } => *condition == value || binds(when_true) || binds(when_false),
        }
    }
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

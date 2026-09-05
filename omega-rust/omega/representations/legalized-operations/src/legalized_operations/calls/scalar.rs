//! calls scalar in the legalized operations program.

use calling_conventions::CallPlan;
use optimization_unit::EffectLink;
use optimization_unit::FuelSettlement;
use optimization_unit::OwnershipEvent;
use optimization_unit::ValueDefinitionSite;
use semantic_vocabulary::BlockId;
use semantic_vocabulary::EdgeId;
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::MachineId;
use semantic_vocabulary::ObligationId;
use semantic_vocabulary::OperationId;
use semantic_vocabulary::ValueId;
use target_operations::TerminalPsiProvenance;
use terminal_psi::CrashRouteBucket;

/// Closed attached-Unit scalar-call legalization forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarCallUnitLegalizationRecipe {
    U64EqualityConditionalThreeCallChainThenReturnUnitV1,
}

/// Exact custody for two U64 constants and the three-call fork/join chain
/// that consumes them before returning Unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarCallUnitFunction {
    pub machine: MachineId,
    pub attachment: semantic_vocabulary::StructuralTypeId,
    pub provenance: TerminalPsiProvenance,
    pub recipe: ScalarCallUnitLegalizationRecipe,
    pub entry_block: BlockId,
    pub constants: [LegalizedScalarCallUnitConstant; 2],
    pub calls: [LegalizedScalarCallUnitCall; 3],
    pub return_edge: EdgeId,
    pub return_fuel: Vec<FuelSettlement>,
    pub return_effect: EffectLink,
    pub return_ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarCallUnitConstant {
    pub operation: OperationId,
    pub result: ValueId,
    pub scalar_type: IntegerType,
    pub value: IntegerValue,
    pub definition_site: ValueDefinitionSite,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarCallUnitCall {
    pub operation: OperationId,
    pub callee: MachineId,
    pub call_plan: CallPlan,
    pub result_home: target_operations::TargetUnitScalarHomeRequirement,
    pub result_definition_site: ValueDefinitionSite,
    pub arguments: [LegalizedScalarCallUnitArgument; 2],
    pub requirement_obligations: Vec<ObligationId>,
    pub crash_continuations: Vec<CrashRouteBucket>,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedScalarCallUnitArgument {
    pub parameter_index: u32,
    pub source: target_operations::TargetUnitScalarArgumentSource,
    pub placement: calling_conventions::ValuePlacement,
}

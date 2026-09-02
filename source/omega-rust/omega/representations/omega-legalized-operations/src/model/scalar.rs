use super::plan::{LegalizationRecipe, LegalizationTheorem, LegalizedTemporaryId};
use super::shared::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedFunction {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub recipe: LegalizationRecipe,
    pub condition_source: ValueId,
    pub condition: LegalizedCondition,
    pub entry_block: BlockId,
    pub true_block: BlockId,
    pub false_block: BlockId,
    pub branch_true_edge: EdgeId,
    pub branch_false_edge: EdgeId,
    pub branch_true_fuel: Vec<FuelSettlement>,
    pub branch_false_fuel: Vec<FuelSettlement>,
    pub branch_true_bindings: Vec<ValueBinding>,
    pub branch_false_bindings: Vec<ValueBinding>,
    pub when_true: LegalizedLeaf,
    pub when_false: LegalizedLeaf,
}

/// Exact condition custody retained across target legalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedCondition {
    DirectParameter {
        parameter_index: usize,
        register: MachineRegister,
        definition_site: ValueDefinitionSite,
    },
    IntegerEqualParametersV1 {
        operation: OperationId,
        result_definition_site: ValueDefinitionSite,
        fuel: Vec<FuelSettlement>,
        left: LegalizedConditionParameter,
        right: LegalizedConditionParameter,
    },
}

/// One ordered entry-parameter operand of a legalized condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedConditionParameter {
    pub source_value: ValueId,
    pub parameter_index: usize,
    pub register: MachineRegister,
    pub definition_site: ValueDefinitionSite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedLeaf {
    pub return_edge: EdgeId,
    pub source_value: ValueId,
    pub return_fuel: Vec<FuelSettlement>,
    pub value: LegalizedLeafValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedLeafValue {
    Immediate {
        value: IntegerValue,
        constant_operation: OperationId,
        definition_site: ValueDefinitionSite,
        constant_fuel: Vec<FuelSettlement>,
    },
    EntryParameter {
        parameter_index: usize,
        register: MachineRegister,
        definition_site: ValueDefinitionSite,
    },
    ExactAdd {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
        add_operation: OperationId,
        definition_site: ValueDefinitionSite,
        add_fuel: Vec<FuelSettlement>,
        left: LegalizedImmediate,
        right: LegalizedImmediate,
    },
    ExactSubtract {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
        subtract_operation: OperationId,
        definition_site: ValueDefinitionSite,
        subtract_fuel: Vec<FuelSettlement>,
        left: LegalizedImmediate,
        right: LegalizedImmediate,
    },
    WidenedExactAdd {
        source_type: IntegerType,
        target_type: IntegerType,
        theorem: LegalizationTheorem,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
        add_operation: OperationId,
        narrow_result: ValueId,
        add_definition_site: ValueDefinitionSite,
        add_fuel: Vec<FuelSettlement>,
        widen_operation: OperationId,
        widen_definition_site: ValueDefinitionSite,
        widen_fuel: Vec<FuelSettlement>,
        left_temporary: LegalizedTemporaryId,
        right_temporary: LegalizedTemporaryId,
        left: LegalizedImmediate,
        right: LegalizedImmediate,
    },
    WidenedExactSubtract {
        source_type: IntegerType,
        target_type: IntegerType,
        theorem: LegalizationTheorem,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
        subtract_operation: OperationId,
        narrow_result: ValueId,
        subtract_definition_site: ValueDefinitionSite,
        subtract_fuel: Vec<FuelSettlement>,
        widen_operation: OperationId,
        widen_definition_site: ValueDefinitionSite,
        widen_fuel: Vec<FuelSettlement>,
        left_temporary: LegalizedTemporaryId,
        right_temporary: LegalizedTemporaryId,
        left: LegalizedImmediate,
        right: LegalizedImmediate,
    },
    ActiveResidentExactAddChain(Box<LegalizedActiveResidentExactAddChain>),
    ActiveResidentExactAddBridgeChain(Box<LegalizedActiveResidentExactAddBridgeChain>),
    ActiveResidentExactAddOriginalVictimChain(
        Box<LegalizedActiveResidentExactAddOriginalVictimChain>,
    ),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedActiveResidentExactAddChain {
    pub resident: LegalizedImmediate,
    pub left: LegalizedImmediate,
    pub right: LegalizedImmediate,
    pub inner: LegalizedExactAdd,
    pub middle: LegalizedExactAdd,
    pub result: LegalizedExactAdd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedActiveResidentExactAddBridgeChain {
    pub resident: LegalizedImmediate,
    pub left: LegalizedImmediate,
    pub right: LegalizedImmediate,
    pub inner: LegalizedExactAdd,
    pub middle: LegalizedExactAdd,
    pub bridge: LegalizedExactAdd,
    pub result: LegalizedExactAdd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedActiveResidentExactAddOriginalVictimChain {
    pub resident: LegalizedImmediate,
    pub left: LegalizedImmediate,
    pub right: LegalizedImmediate,
    pub inner: LegalizedExactAdd,
    pub middle: LegalizedExactAdd,
    pub bridge: LegalizedExactAdd,
    pub join: LegalizedExactAdd,
    pub result: LegalizedExactAdd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedExactAdd {
    pub source_value: ValueId,
    pub obligation: ObligationId,
    pub accepted_fact: AcceptedObligationFactIdentity,
    pub operation: OperationId,
    pub definition_site: ValueDefinitionSite,
    pub fuel: Vec<FuelSettlement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedImmediate {
    pub source_value: ValueId,
    pub value: IntegerValue,
    pub constant_operation: OperationId,
    pub definition_site: ValueDefinitionSite,
    pub fuel: Vec<FuelSettlement>,
}

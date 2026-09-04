//! values conditions in the legalized operations program.

use crate::LegalizedImmediate;
use omega_optimization_unit::FuelSettlement;
use omega_optimization_unit::ValueDefinitionSite;
use omega_target_operations::MachineRegister;
use psi_core::OperationId;
use psi_core::ValueId;

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
    IntegerLessThanParametersV1 {
        operation: OperationId,
        result_definition_site: ValueDefinitionSite,
        fuel: Vec<FuelSettlement>,
        left: LegalizedConditionParameter,
        right: LegalizedConditionParameter,
    },
    IntegerLessOrEqualParametersV1 {
        operation: OperationId,
        result_definition_site: ValueDefinitionSite,
        fuel: Vec<FuelSettlement>,
        left: LegalizedConditionParameter,
        right: LegalizedConditionParameter,
    },
    IntegerNotEqualParametersV1 {
        equality_operation: OperationId,
        equality_result: ValueId,
        equality_result_definition_site: ValueDefinitionSite,
        equality_fuel: Vec<FuelSettlement>,
        boolean_not_operation: OperationId,
        boolean_not_result: ValueId,
        boolean_not_result_definition_site: ValueDefinitionSite,
        boolean_not_fuel: Vec<FuelSettlement>,
        left: LegalizedConditionParameter,
        right: LegalizedConditionParameter,
    },
    I64LessThanParametersV1 {
        operation: OperationId,
        result_definition_site: ValueDefinitionSite,
        fuel: Vec<FuelSettlement>,
        left: LegalizedConditionParameter,
        right: LegalizedConditionParameter,
    },
    I64LessOrEqualParametersV1 {
        operation: OperationId,
        result_definition_site: ValueDefinitionSite,
        fuel: Vec<FuelSettlement>,
        left: LegalizedConditionParameter,
        right: LegalizedConditionParameter,
    },
    U64EqualZeroParameterV1 {
        operation: OperationId,
        result_definition_site: ValueDefinitionSite,
        fuel: Vec<FuelSettlement>,
        parameter: LegalizedConditionParameter,
        zero: LegalizedImmediate,
    },
    U64NotEqualZeroParameterV1 {
        equality_operation: OperationId,
        equality_result: ValueId,
        equality_result_definition_site: ValueDefinitionSite,
        equality_fuel: Vec<FuelSettlement>,
        boolean_not_operation: OperationId,
        boolean_not_result: ValueId,
        boolean_not_result_definition_site: ValueDefinitionSite,
        boolean_not_fuel: Vec<FuelSettlement>,
        parameter: LegalizedConditionParameter,
        zero: LegalizedImmediate,
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

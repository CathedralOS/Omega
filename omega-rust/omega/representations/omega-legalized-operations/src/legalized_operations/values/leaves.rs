//! values leaves in the legalized operations program.

use crate::{LegalizationTheorem, LegalizedTemporaryId};
use omega_optimization_core::AcceptedObligationFactIdentity;
use omega_optimization_unit::FuelSettlement;
use omega_optimization_unit::ValueDefinitionSite;
use omega_target_operations::MachineRegister;
use psi_core::EdgeId;
use psi_core::IntegerType;
use psi_core::IntegerValue;
use psi_core::ObligationId;
use psi_core::OperationId;
use psi_core::ValueId;

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

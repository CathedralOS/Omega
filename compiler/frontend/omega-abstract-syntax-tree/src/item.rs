use crate::identifier::{Identifier, IdentifierPath};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Capability(CapabilityDefinition),
    Data(DataDefinition),
    Invariant(InvariantDefinition),
    TrustDefinition(TrustDefinition),
    Use(UseItem),
    Machine(Machine),
    Platform(Platform),
    Target(TargetDefinition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseItem {
    pub path: IdentifierPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantDefinition {
    pub name: Identifier,
    pub constraints: Vec<crate::types::TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustDefinition {
    pub name: Identifier,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDefinition {
    pub name: Identifier,
    pub members: Vec<CapabilityMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityMember {
    Field(CapabilityField),
    State(CapabilityState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityField {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityState {
    pub signature: StateSignature,
    pub contracts: Vec<CapabilityContract>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityContract {
    pub kind: CapabilityContractKind,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityContractKind {
    Ensures,
    Requires,
    Trusted(TrustLevel),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustLevel {
    Host,
    Named(Identifier),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDefinition {
    pub name: Identifier,
    pub host: Option<TargetHost>,
    pub trust_policies: Vec<TrustPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetHost {
    pub provider: IdentifierPath,
    pub settings: Vec<TargetHostSetting>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetHostSetting {
    pub name: Identifier,
    pub value: TargetHostSettingValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetHostSettingValue {
    Call {
        name: Identifier,
        argument_tokens: usize,
    },
    Named(Identifier),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustPolicy {
    pub mode: TrustMode,
    pub path: IdentifierPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustMode {
    Checked,
    Unchecked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDefinition {
    pub name: Identifier,
    pub members: Vec<DataMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataMember {
    Field(DataField),
    Variant(DataVariant),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataField {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVariant {
    pub name: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: Identifier,
    pub contains: Vec<Contains>,
    pub owned_data: Vec<OwnedData>,
    pub states: Vec<State>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contains {
    pub name: Identifier,
    pub type_name: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedData {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReference,
    pub initial_value: Option<crate::expression::Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub name: Identifier,
    pub parameters: Vec<StateParameter>,
    pub return_type: Option<crate::types::TypeReference>,
    pub statements: Vec<crate::statement::Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub name: Identifier,
    pub states: Vec<StateSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSignature {
    pub name: Identifier,
    pub parameters: Vec<StateParameter>,
    pub return_type: Option<crate::types::TypeReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateParameter {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReference,
    pub is_const: bool,
    pub is_mutable: bool,
    pub is_self: bool,
}

use crate::identifier::IdentifierPath;

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
    pub name: String,
    pub constraints: Vec<crate::types::TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustDefinition {
    pub name: String,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDefinition {
    pub name: String,
    pub members: Vec<CapabilityMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityMember {
    Field(CapabilityField),
    State(CapabilityState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityField {
    pub name: String,
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
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDefinition {
    pub name: String,
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
    pub name: String,
    pub value: TargetHostSettingValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetHostSettingValue {
    Call {
        name: String,
        argument_tokens: usize,
    },
    Named(String),
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
    pub name: String,
    pub members: Vec<DataMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataMember {
    Field(DataField),
    Variant(DataVariant),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataField {
    pub name: String,
    pub type_reference: crate::types::TypeReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVariant {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: String,
    pub contains: Vec<Contains>,
    pub owned_data: Vec<OwnedData>,
    pub states: Vec<State>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contains {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedData {
    pub name: String,
    pub type_reference: crate::types::TypeReference,
    pub initial_value: Option<crate::expression::Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub name: String,
    pub parameters: Vec<StateParameter>,
    pub return_type: Option<crate::types::TypeReference>,
    pub statements: Vec<crate::statement::Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub name: String,
    pub states: Vec<StateSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSignature {
    pub name: String,
    pub parameters: Vec<StateParameter>,
    pub return_type: Option<crate::types::TypeReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateParameter {
    pub name: String,
    pub type_reference: crate::types::TypeReference,
    pub is_const: bool,
    pub is_mutable: bool,
    pub is_self: bool,
}

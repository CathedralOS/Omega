use crate::identifier::{Identifier, IdentifierPath};
use omega_core::arena::{Arena, Handle, HandleSpan};

pub type ItemHandle = Handle<Item>;
pub type StateParameterHandle = Handle<StateParameterNode>;
pub type StateSignatureHandle = Handle<StateSignatureNode>;
pub type StateHandle = Handle<StateNode>;
pub type MachineHandle = Handle<MachineNode>;
pub type PlatformHandle = Handle<PlatformNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Capability(CapabilityDefinition),
    Data(DataDefinition),
    Invariant(InvariantDefinition),
    Library(LibraryDefinition),
    TrustDefinition(TrustDefinition),
    Use(UseItem),
    Machine(Machine),
    Platform(Platform),
    Target(TargetDefinition),
}

impl Default for Item {
    fn default() -> Self {
        Self::Use(UseItem::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseItem {
    pub path: IdentifierPath,
}

impl Default for UseItem {
    fn default() -> Self {
        Self {
            path: IdentifierPath::default(),
        }
    }
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
pub struct LibraryDefinition {
    pub name: Option<Identifier>,
    pub path: String,
    pub calling_convention: Identifier,
    pub functions: Vec<LibraryFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFunction {
    pub signature: StateSignature,
    pub symbol: Option<String>,
    pub calling_convention: Option<Identifier>,
    pub trusts: Vec<TrustLevel>,
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
    pub type_reference: crate::types::TypeReferenceHandle,
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
    pub type_parameters: Vec<TypeParameter>,
    pub members: Vec<DataMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParameter {
    pub name: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataMember {
    Field(DataField),
    Variant(DataVariant),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataField {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub initial_value: crate::expression::ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVariant {
    pub name: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: Identifier,
    pub states: Vec<State>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub statements: HandleSpan<crate::statement::StatementHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub name: Identifier,
    pub states: Vec<StateSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSignature {
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemTable {
    state_parameters: Arena<StateParameterNode>,
    state_signatures: Arena<StateSignatureNode>,
    states: Arena<StateNode>,
    state_parameter_handles: Arena<StateParameterHandle>,
    state_signature_handles: Arena<StateSignatureHandle>,
    statement_handles: Arena<crate::statement::StatementHandle>,
    machines: Arena<MachineNode>,
    platforms: Arena<PlatformNode>,
}

impl ItemTable {
    pub fn new() -> Self {
        Self {
            state_parameters: Arena::new(),
            state_signatures: Arena::new(),
            states: Arena::new(),
            state_parameter_handles: Arena::new(),
            state_signature_handles: Arena::new(),
            statement_handles: Arena::new(),
            machines: Arena::new(),
            platforms: Arena::new(),
        }
    }

    pub fn state_parameter(&self, handle: StateParameterHandle) -> &StateParameterNode {
        self.state_parameters.get(handle)
    }

    pub fn state_signature(&self, handle: StateSignatureHandle) -> &StateSignatureNode {
        self.state_signatures.get(handle)
    }

    pub fn state(&self, handle: StateHandle) -> &StateNode {
        self.states.get(handle)
    }

    pub fn machine(&self, handle: MachineHandle) -> &MachineNode {
        self.machines.get(handle)
    }

    pub fn platform(&self, handle: PlatformHandle) -> &PlatformNode {
        self.platforms.get(handle)
    }

    pub fn state_parameters(
        &self,
        span: HandleSpan<StateParameterHandle>,
    ) -> &[StateParameterHandle] {
        self.state_parameter_handles.span_or_empty(span)
    }

    pub fn state_signatures(
        &self,
        span: HandleSpan<StateSignatureHandle>,
    ) -> &[StateSignatureHandle] {
        self.state_signature_handles.span_or_empty(span)
    }

    pub fn statements(
        &self,
        span: HandleSpan<crate::statement::StatementHandle>,
    ) -> &[crate::statement::StatementHandle] {
        self.statement_handles.span_or_empty(span)
    }

    pub fn insert_state_parameter_node(
        &mut self,
        parameter: StateParameterNode,
    ) -> StateParameterHandle {
        self.state_parameters.append(parameter)
    }

    pub fn append_state_parameter_handle(
        &mut self,
        handle: StateParameterHandle,
    ) -> Handle<StateParameterHandle> {
        self.state_parameter_handles.append(handle)
    }

    pub fn append_statement_handle(
        &mut self,
        handle: crate::statement::StatementHandle,
    ) -> Handle<crate::statement::StatementHandle> {
        self.statement_handles.append(handle)
    }

    pub fn state_parameter_count(&self) -> usize {
        self.state_parameters.len()
    }

    pub fn state_signature_count(&self) -> usize {
        self.state_signatures.len()
    }

    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    pub fn machine_count(&self) -> usize {
        self.machines.len()
    }

    pub fn platform_count(&self) -> usize {
        self.platforms.len()
    }

    pub fn insert_state_signature_tree(
        &mut self,
        signature: &StateSignature,
        _type_references: &mut crate::types::TypeReferenceTable,
        _expressions: &mut crate::expression::ExpressionTable,
    ) -> StateSignatureHandle {
        self.state_signatures.append(StateSignatureNode {
            name: signature.name.clone(),
            parameters: signature.parameters,
            return_type: signature.return_type,
        })
    }

    pub fn insert_state_tree(
        &mut self,
        state: &State,
        _statements: &mut crate::statement::StatementTable,
        _type_references: &mut crate::types::TypeReferenceTable,
        _expressions: &mut crate::expression::ExpressionTable,
    ) -> StateHandle {
        self.states.append(StateNode {
            name: state.name.clone(),
            parameters: state.parameters,
            return_type: state.return_type,
            statements: state.statements,
        })
    }

    pub fn insert_machine_tree(
        &mut self,
        machine: &Machine,
        statements: &mut crate::statement::StatementTable,
        type_references: &mut crate::types::TypeReferenceTable,
        expressions: &mut crate::expression::ExpressionTable,
    ) -> MachineHandle {
        let states = self.insert_state_span_from_trees(
            &machine.states,
            statements,
            type_references,
            expressions,
        );
        self.machines.append(MachineNode {
            name: machine.name.clone(),
            states,
        })
    }

    pub fn insert_platform_tree(
        &mut self,
        platform: &Platform,
        type_references: &mut crate::types::TypeReferenceTable,
        expressions: &mut crate::expression::ExpressionTable,
    ) -> PlatformHandle {
        let states = self.insert_state_signature_span_from_trees(
            &platform.states,
            type_references,
            expressions,
        );
        self.platforms.append(PlatformNode {
            name: platform.name.clone(),
            states,
        })
    }

    fn insert_state_signature_span_from_trees(
        &mut self,
        signatures: &[StateSignature],
        type_references: &mut crate::types::TypeReferenceTable,
        expressions: &mut crate::expression::ExpressionTable,
    ) -> HandleSpan<StateSignatureHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for signature in signatures {
            let handle = self.insert_state_signature_tree(signature, type_references, expressions);
            let handle = self.state_signature_handles.append(handle);
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("state signature handle span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn insert_state_span_from_trees(
        &mut self,
        states: &[State],
        _statement_table: &mut crate::statement::StatementTable,
        _type_references: &mut crate::types::TypeReferenceTable,
        _expressions: &mut crate::expression::ExpressionTable,
    ) -> HandleSpan<StateNode> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for state in states {
            let handle = self.states.append(StateNode {
                name: state.name.clone(),
                parameters: state.parameters,
                return_type: state.return_type,
                statements: state.statements,
            });
            if count == 0 {
                start = handle;
            }
            count = count.checked_add(1).expect("state span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }
}

impl Default for ItemTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateParameterNode {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub is_const: bool,
    pub is_mutable: bool,
    pub is_self: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateSignatureNode {
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateNode {
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub statements: HandleSpan<crate::statement::StatementHandle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineNode {
    pub name: Identifier,
    pub states: HandleSpan<StateNode>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlatformNode {
    pub name: Identifier,
    pub states: HandleSpan<StateSignatureHandle>,
}

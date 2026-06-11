use crate::identifier::Identifier;
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::operator_spelling::{OperatorSpelling, ProviderCategory};

pub type ItemHandle = Handle<Item>;
pub type StateParameterHandle = Handle<StateParameterNode>;
pub type StateSignatureHandle = Handle<StateSignatureNode>;
pub type StateHandle = Handle<StateNode>;
pub type MachineHandle = Handle<MachineNode>;
pub type PlatformHandle = Handle<PlatformNode>;
pub type TraitHandle = Handle<TraitNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Capability(CapabilityDefinition),
    Conformance(ConformanceItem),
    Data(DataDefinition),
    Domain(DomainDefinition),
    Invariant(InvariantDefinition),
    Library(LibraryDefinition),
    Measure(MeasureDefinition),
    Module(ModuleDeclaration),
    Operator(OperatorDefinition),
    Package(PackageDeclaration),
    Provider(ProviderDeclaration),
    HostProvider(HostProviderDefinition),
    Export(ExportItem),
    Use(UseItem),
    Machine(Machine),
    Platform(Platform),
    Trait(TraitDefinition),
    Target(TargetDefinition),
    WireData(WireDataDefinition),
}

/// A boundary primitive provider declaration (frozen Wave 0 decision #4):
/// `provider <QualifiedName> : <Category>;`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDeclaration {
    pub name: HandleSpan<Identifier>,
    pub category: ProviderCategory,
}

impl Default for ProviderDeclaration {
    fn default() -> Self {
        Self {
            name: HandleSpan::empty(),
            category: ProviderCategory::SliceIndexing,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostProviderDefinition {
    pub target: Identifier,
    pub boundary_trait: HandleSpan<Identifier>,
    pub mappings: HandleSpan<HostProviderMapping>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostProviderMapping {
    pub machine: Identifier,
    pub kind: HostProviderMappingKind,
    pub value: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HostProviderMappingKind {
    #[default]
    Syscall,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireDataDefinition {
    pub name: Identifier,
    pub encoding: Option<Identifier>,
    pub members: HandleSpan<WireDataMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireDataMember {
    Field(WireDataField),
    Reserved(WireDataReserved),
    Version(WireDataVersion),
}

impl Default for WireDataMember {
    fn default() -> Self {
        Self::Reserved(WireDataReserved::default())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireDataField {
    pub number: i64,
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireDataReserved {
    pub number: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireDataVersion {
    pub name: Identifier,
    pub members: HandleSpan<WireDataMember>,
}

impl Default for Item {
    fn default() -> Self {
        Self::Use(UseItem::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseItem {
    pub path: HandleSpan<Identifier>,
}

impl Default for UseItem {
    fn default() -> Self {
        Self {
            path: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleDeclaration {
    pub path: HandleSpan<Identifier>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageDeclaration {
    pub path: HandleSpan<Identifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportItem {
    pub path: HandleSpan<Identifier>,
    pub alias: Option<Identifier>,
}

impl Default for ExportItem {
    fn default() -> Self {
        Self {
            path: HandleSpan::empty(),
            alias: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantDefinition {
    pub name: Identifier,
    pub constraints: HandleSpan<crate::types::TypeConstraintNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryDefinition {
    pub name: Option<Identifier>,
    pub path: String,
    pub calling_convention: Identifier,
    pub functions: HandleSpan<LibraryFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFunction {
    pub signature: StateSignature,
    pub symbol: Option<String>,
    pub calling_convention: Option<Identifier>,
    pub boundaries: HandleSpan<BoundaryLevel>,
}

impl Default for LibraryFunction {
    fn default() -> Self {
        Self {
            signature: StateSignature {
                name: Identifier::default(),
                is_default: false,
                parameters: HandleSpan::empty(),
                return_type: crate::types::TypeReferenceHandle::invalid(),
                effects: HandleSpan::empty(),
                contracts: HandleSpan::empty(),
            },
            symbol: None,
            calling_convention: None,
            boundaries: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasureDefinition {
    /// Fully-qualified declaration path, e.g. `Card::PowerOrder`.
    pub name: HandleSpan<Identifier>,
    /// The single parameter being measured (e.g. `card: Card`).
    pub parameter: StateParameterHandle,
    /// Well-founded domain type, always `usize` for now.
    pub return_type: crate::types::TypeReferenceHandle,
    /// `true` when declared with the `lexicographic { .. }` body form.
    pub lexicographic: bool,
    /// Body component expressions: exactly one for the simple form, or the
    /// ordered tuple of components for the lexicographic form.
    pub body: HandleSpan<crate::expression::ExpressionHandle>,
    pub token_count: usize,
}

impl Default for MeasureDefinition {
    fn default() -> Self {
        Self {
            name: HandleSpan::empty(),
            parameter: StateParameterHandle::invalid(),
            return_type: crate::types::TypeReferenceHandle::invalid(),
            lexicographic: false,
            body: HandleSpan::empty(),
            token_count: 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperatorDefinition {
    pub is_boundary: bool,
    pub name: HandleSpan<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub contracts: HandleSpan<CapabilityContract>,
    /// Optional `spelling <symbol>` clause (frozen Wave 0 decision #3).
    pub spelling: Option<OperatorSpelling>,
    /// Optional `provider <QualifiedName>` clause binding this boundary
    /// operator to a registered provider (frozen Wave 0 decision #4).
    pub provider: Option<HandleSpan<Identifier>>,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDefinition {
    pub name: Identifier,
    pub members: HandleSpan<CapabilityMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityMember {
    Field(CapabilityField),
    State(CapabilityState),
}

impl Default for CapabilityMember {
    fn default() -> Self {
        Self::Field(CapabilityField::default())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilityField {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityState {
    pub signature: StateSignature,
    pub contracts: HandleSpan<CapabilityContract>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityContract {
    pub kind: CapabilityContractKind,
    pub facts: HandleSpan<ProofFact>,
    pub token_count: usize,
}

impl Default for CapabilityContract {
    fn default() -> Self {
        Self {
            kind: CapabilityContractKind::default(),
            facts: HandleSpan::empty(),
            token_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityContractKind {
    Ensures,
    Requires,
    Boundary(BoundaryLevel),
}

impl Default for CapabilityContractKind {
    fn default() -> Self {
        Self::Requires
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryLevel {
    Host,
    Named(Identifier),
}

impl Default for BoundaryLevel {
    fn default() -> Self {
        Self::Named(Identifier::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDefinition {
    pub name: Identifier,
    pub host: Option<TargetHost>,
    pub boundary_policies: HandleSpan<BoundaryPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetHost {
    pub provider: HandleSpan<Identifier>,
    pub settings: HandleSpan<TargetHostSetting>,
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
pub struct BoundaryPolicy {
    pub mode: BoundaryMode,
    pub path: HandleSpan<Identifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryMode {
    Checked,
    Unchecked,
}

impl Default for TargetHostSetting {
    fn default() -> Self {
        Self {
            name: Identifier::default(),
            value: TargetHostSettingValue::default(),
        }
    }
}

impl Default for TargetHostSettingValue {
    fn default() -> Self {
        Self::Named(Identifier::default())
    }
}

impl Default for BoundaryPolicy {
    fn default() -> Self {
        Self {
            mode: BoundaryMode::default(),
            path: HandleSpan::empty(),
        }
    }
}

impl Default for BoundaryMode {
    fn default() -> Self {
        Self::Checked
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDefinition {
    pub name: Identifier,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub properties: DataProperties,
    pub members: HandleSpan<DataMember>,
}

/// A standalone conformance item (frozen decision 8): `Point satisfies
/// Equatable;` claims a whole trait for a data type. Validation checks the
/// type's written attached machines against the trait's requirements;
/// nothing trait-shaped appears on the data declaration itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConformanceItem {
    pub type_name: Identifier,
    pub trait_name: Identifier,
}

/// Declared type properties: lowercase facts in brackets on the data
/// declaration (`data Point [copy, zero_init]`). The known set is closed;
/// unknown names are parse errors, so downstream representations carry the
/// resolved flags rather than spellings. `sized` is computed from the shape
/// and may not be declared.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataProperties {
    /// Values copy bit-for-bit; verified: every field is itself copyable.
    pub copy: bool,
    /// Zero means empty: the zeroed value is the type's empty value; owns the
    /// zero-case-payload-free rule and rejects non-zero field defaults.
    pub zero_init: bool,
    /// Values may cross thread boundaries; verified structurally like `copy`
    /// until the concurrency model lands.
    pub send: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeParameter {
    pub name: Identifier,
    pub kind: TypeParameterKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TypeParameterKind {
    #[default]
    Type,
    Const {
        type_reference: crate::types::TypeReferenceHandle,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataMember {
    Field(DataField),
    Variant(DataVariant),
    Version(DataVersion),
}

impl Default for DataMember {
    fn default() -> Self {
        Self::Variant(DataVariant::default())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataVersion {
    pub name: Identifier,
    pub members: HandleSpan<DataMember>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataField {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub initial_value: crate::expression::ExpressionHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataVariant {
    pub name: Identifier,
    /// Named payload fields (`case Say(text: String);`). Payload-less cases have an
    /// empty span. Stored in their own arena so the parent's member span stays
    /// contiguous while a case's payload is parsed.
    pub payload: HandleSpan<DataField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainDefinition {
    pub name: Identifier,
    pub target_type: crate::types::TypeReferenceHandle,
    pub classifier: crate::expression::ExpressionHandle,
    pub facts: HandleSpan<ProofFact>,
    pub operators: HandleSpan<OperatorDefinition>,
    pub body_token_count: usize,
}

impl Default for DomainDefinition {
    fn default() -> Self {
        Self {
            name: Identifier::generated(""),
            target_type: crate::types::TypeReferenceHandle::invalid(),
            classifier: crate::expression::ExpressionHandle::invalid(),
            facts: HandleSpan::empty(),
            operators: HandleSpan::empty(),
            body_token_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofFact {
    Expression(crate::expression::ExpressionHandle),
    Membership(ProofMembershipFact),
}

impl Default for ProofFact {
    fn default() -> Self {
        Self::Expression(crate::expression::ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofMembershipFact {
    pub value: crate::expression::ExpressionHandle,
    pub domain: HandleSpan<Identifier>,
}

impl Default for ProofMembershipFact {
    fn default() -> Self {
        Self {
            value: crate::expression::ExpressionHandle::invalid(),
            domain: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    pub abi: Option<String>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub satisfies: HandleSpan<Identifier>,
    pub terminates: bool,
    pub decreases: HandleSpan<crate::expression::ExpressionHandle>,
    pub decrease_order: HandleSpan<Identifier>,
    pub effects: HandleSpan<Identifier>,
    pub contracts: HandleSpan<CapabilityContract>,
    pub states: HandleSpan<StateHandle>,
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
    pub states: HandleSpan<StateSignatureHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDefinition {
    pub is_boundary: bool,
    pub name: Identifier,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub invariants: HandleSpan<ProofFact>,
    pub requires: HandleSpan<Identifier>,
    pub machines: HandleSpan<StateSignatureHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSignature {
    pub name: Identifier,
    pub is_default: bool,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub effects: HandleSpan<Identifier>,
    pub contracts: HandleSpan<CapabilityContract>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemTable {
    items: Arena<Item>,
    state_storage: StateStorage,
    declaration_storage: DeclarationStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StateStorage {
    parameters: Arena<StateParameterNode>,
    signatures: Arena<StateSignatureNode>,
    states: Arena<StateNode>,
    parameter_handles: Arena<StateParameterHandle>,
    state_handles: Arena<StateHandle>,
    signature_handles: Arena<StateSignatureHandle>,
    statement_handles: Arena<crate::statement::StatementHandle>,
    machines: Arena<MachineNode>,
    platforms: Arena<PlatformNode>,
    traits: Arena<TraitNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeclarationStorage {
    identifier_path_members: Arena<Identifier>,
    type_parameters: Arena<TypeParameter>,
    boundary_levels: Arena<BoundaryLevel>,
    library_functions: Arena<LibraryFunction>,
    capability_members: Arena<CapabilityMember>,
    capability_contracts: Arena<CapabilityContract>,
    data_members: Arena<DataMember>,
    data_payload_fields: Arena<DataField>,
    host_provider_mappings: Arena<HostProviderMapping>,
    wire_data_members: Arena<WireDataMember>,
    operators: Arena<OperatorDefinition>,
    measures: Arena<MeasureDefinition>,
    proof_facts: Arena<ProofFact>,
    target_host_settings: Arena<TargetHostSetting>,
    boundary_policies: Arena<BoundaryPolicy>,
}

impl ItemTable {
    pub fn new() -> Self {
        Self {
            items: Arena::new(),
            state_storage: StateStorage::new(),
            declaration_storage: DeclarationStorage::new(),
        }
    }

    pub fn state_parameter(&self, handle: StateParameterHandle) -> &StateParameterNode {
        self.state_storage.parameters.get(handle)
    }

    pub fn item(&self, handle: ItemHandle) -> &Item {
        self.items.get(handle)
    }

    pub fn state_signature(&self, handle: StateSignatureHandle) -> &StateSignatureNode {
        self.state_storage.signatures.get(handle)
    }

    pub fn state(&self, handle: StateHandle) -> &StateNode {
        self.state_storage.states.get(handle)
    }

    pub fn machine(&self, handle: MachineHandle) -> &MachineNode {
        self.state_storage.machines.get(handle)
    }

    pub fn platform(&self, handle: PlatformHandle) -> &PlatformNode {
        self.state_storage.platforms.get(handle)
    }

    pub fn trait_definition(&self, handle: TraitHandle) -> &TraitNode {
        self.state_storage.traits.get(handle)
    }

    pub fn type_parameters(&self, span: HandleSpan<TypeParameter>) -> &[TypeParameter] {
        self.declaration_storage.type_parameters.span_or_empty(span)
    }

    pub fn identifier_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
        self.declaration_storage
            .identifier_path_members
            .span_or_empty(span)
    }

    pub fn library_functions(&self, span: HandleSpan<LibraryFunction>) -> &[LibraryFunction] {
        self.declaration_storage
            .library_functions
            .span_or_empty(span)
    }

    pub fn boundary_levels(&self, span: HandleSpan<BoundaryLevel>) -> &[BoundaryLevel] {
        self.declaration_storage.boundary_levels.span_or_empty(span)
    }

    pub fn capability_members(&self, span: HandleSpan<CapabilityMember>) -> &[CapabilityMember] {
        self.declaration_storage
            .capability_members
            .span_or_empty(span)
    }

    pub fn capability_contracts(
        &self,
        span: HandleSpan<CapabilityContract>,
    ) -> &[CapabilityContract] {
        self.declaration_storage
            .capability_contracts
            .span_or_empty(span)
    }

    pub fn data_members(&self, span: HandleSpan<DataMember>) -> &[DataMember] {
        self.declaration_storage.data_members.span_or_empty(span)
    }

    pub fn data_payload_fields(&self, span: HandleSpan<DataField>) -> &[DataField] {
        self.declaration_storage
            .data_payload_fields
            .span_or_empty(span)
    }

    pub fn host_provider_mappings(
        &self,
        span: HandleSpan<HostProviderMapping>,
    ) -> &[HostProviderMapping] {
        self.declaration_storage
            .host_provider_mappings
            .span_or_empty(span)
    }

    pub fn wire_data_members(&self, span: HandleSpan<WireDataMember>) -> &[WireDataMember] {
        self.declaration_storage
            .wire_data_members
            .span_or_empty(span)
    }

    pub fn operators(&self, span: HandleSpan<OperatorDefinition>) -> &[OperatorDefinition] {
        self.declaration_storage.operators.span_or_empty(span)
    }

    pub fn measures(&self, span: HandleSpan<MeasureDefinition>) -> &[MeasureDefinition] {
        self.declaration_storage.measures.span_or_empty(span)
    }

    pub fn proof_facts(&self, span: HandleSpan<ProofFact>) -> &[ProofFact] {
        self.declaration_storage.proof_facts.span_or_empty(span)
    }

    pub fn target_host_settings(
        &self,
        span: HandleSpan<TargetHostSetting>,
    ) -> &[TargetHostSetting] {
        self.declaration_storage
            .target_host_settings
            .span_or_empty(span)
    }

    pub fn boundary_policies(&self, span: HandleSpan<BoundaryPolicy>) -> &[BoundaryPolicy] {
        self.declaration_storage
            .boundary_policies
            .span_or_empty(span)
    }

    pub fn state_parameters(
        &self,
        span: HandleSpan<StateParameterHandle>,
    ) -> &[StateParameterHandle] {
        self.state_storage.parameter_handles.span_or_empty(span)
    }

    pub fn state_signatures(
        &self,
        span: HandleSpan<StateSignatureHandle>,
    ) -> &[StateSignatureHandle] {
        self.state_storage.signature_handles.span_or_empty(span)
    }

    pub fn state_handles(&self, span: HandleSpan<StateHandle>) -> &[StateHandle] {
        self.state_storage.state_handles.span_or_empty(span)
    }

    pub fn statements(
        &self,
        span: HandleSpan<crate::statement::StatementHandle>,
    ) -> &[crate::statement::StatementHandle] {
        self.state_storage.statement_handles.span_or_empty(span)
    }

    pub fn insert_state_parameter_node(
        &mut self,
        parameter: StateParameterNode,
    ) -> StateParameterHandle {
        self.state_storage.parameters.append(parameter)
    }

    pub fn append_state_parameter_handle(
        &mut self,
        handle: StateParameterHandle,
    ) -> Handle<StateParameterHandle> {
        self.state_storage.parameter_handles.append(handle)
    }

    pub fn append_state_handle(&mut self, handle: StateHandle) -> Handle<StateHandle> {
        self.state_storage.state_handles.append(handle)
    }

    pub fn append_state_signature_handle(
        &mut self,
        handle: StateSignatureHandle,
    ) -> Handle<StateSignatureHandle> {
        self.state_storage.signature_handles.append(handle)
    }

    pub fn append_statement_handle(
        &mut self,
        handle: crate::statement::StatementHandle,
    ) -> Handle<crate::statement::StatementHandle> {
        self.state_storage.statement_handles.append(handle)
    }

    pub fn append_item(&mut self, item: Item) -> ItemHandle {
        self.items.append(item)
    }

    pub fn insert_identifier_path_members(
        &mut self,
        members: impl IntoIterator<Item = Identifier>,
    ) -> HandleSpan<Identifier> {
        self.declaration_storage
            .identifier_path_members
            .insert_many(members)
    }

    pub fn append_identifier_path_member(&mut self, member: Identifier) -> Handle<Identifier> {
        self.declaration_storage
            .identifier_path_members
            .append(member)
    }

    pub fn append_boundary_policy(&mut self, policy: BoundaryPolicy) -> Handle<BoundaryPolicy> {
        self.declaration_storage.boundary_policies.append(policy)
    }

    pub fn append_type_parameter(
        &mut self,
        type_parameter: TypeParameter,
    ) -> Handle<TypeParameter> {
        self.declaration_storage
            .type_parameters
            .append(type_parameter)
    }

    pub fn append_boundary_level(
        &mut self,
        boundary_level: BoundaryLevel,
    ) -> Handle<BoundaryLevel> {
        self.declaration_storage
            .boundary_levels
            .append(boundary_level)
    }

    pub fn append_library_function(
        &mut self,
        function: LibraryFunction,
    ) -> Handle<LibraryFunction> {
        self.declaration_storage.library_functions.append(function)
    }

    pub fn append_capability_member(
        &mut self,
        member: CapabilityMember,
    ) -> Handle<CapabilityMember> {
        self.declaration_storage.capability_members.append(member)
    }

    pub fn append_capability_contract(
        &mut self,
        contract: CapabilityContract,
    ) -> Handle<CapabilityContract> {
        self.declaration_storage
            .capability_contracts
            .append(contract)
    }

    pub fn append_data_member(&mut self, member: DataMember) -> Handle<DataMember> {
        self.declaration_storage.data_members.append(member)
    }

    pub fn append_data_payload_field(&mut self, field: DataField) -> Handle<DataField> {
        self.declaration_storage.data_payload_fields.append(field)
    }

    pub fn append_host_provider_mapping(
        &mut self,
        mapping: HostProviderMapping,
    ) -> Handle<HostProviderMapping> {
        self.declaration_storage
            .host_provider_mappings
            .append(mapping)
    }

    pub fn append_wire_data_member(&mut self, member: WireDataMember) -> Handle<WireDataMember> {
        self.declaration_storage.wire_data_members.append(member)
    }

    pub fn append_operator(&mut self, operator: OperatorDefinition) -> Handle<OperatorDefinition> {
        self.declaration_storage.operators.append(operator)
    }

    pub fn append_measure(&mut self, measure: MeasureDefinition) -> Handle<MeasureDefinition> {
        self.declaration_storage.measures.append(measure)
    }

    pub fn append_proof_fact(&mut self, fact: ProofFact) -> Handle<ProofFact> {
        self.declaration_storage.proof_facts.append(fact)
    }

    pub fn append_target_host_setting(
        &mut self,
        setting: TargetHostSetting,
    ) -> Handle<TargetHostSetting> {
        self.declaration_storage
            .target_host_settings
            .append(setting)
    }

    pub fn state_count(&self) -> usize {
        self.state_storage.states.len()
    }

    pub fn machine_count(&self) -> usize {
        self.state_storage.machines.len()
    }

    pub fn insert_state_signature(&mut self, signature: &StateSignature) -> StateSignatureHandle {
        self.state_storage.signatures.append(StateSignatureNode {
            name: signature.name.clone(),
            is_default: signature.is_default,
            parameters: signature.parameters,
            return_type: signature.return_type,
            effects: signature.effects,
            contracts: signature.contracts,
        })
    }

    pub fn insert_state(&mut self, state: &State) -> StateHandle {
        self.state_storage.states.append(StateNode {
            name: state.name.clone(),
            parameters: state.parameters,
            return_type: state.return_type,
            statements: state.statements,
        })
    }

    pub fn insert_machine(&mut self, machine: &Machine) -> MachineHandle {
        self.state_storage.machines.append(MachineNode {
            name: machine.name.clone(),
            satisfies: machine.satisfies,
            effects: machine.effects,
            contracts: machine.contracts,
            states: machine.states,
        })
    }

    pub fn insert_platform(&mut self, platform: &Platform) -> PlatformHandle {
        self.state_storage.platforms.append(PlatformNode {
            name: platform.name.clone(),
            states: platform.states,
        })
    }

    pub fn insert_trait_definition(&mut self, trait_definition: &TraitDefinition) -> TraitHandle {
        self.state_storage.traits.append(TraitNode {
            is_boundary: trait_definition.is_boundary,
            name: trait_definition.name.clone(),
            requires: trait_definition.requires,
            machines: trait_definition.machines,
        })
    }
}

impl Default for ItemTable {
    fn default() -> Self {
        Self::new()
    }
}

impl StateStorage {
    fn new() -> Self {
        Self {
            parameters: Arena::new(),
            signatures: Arena::new(),
            states: Arena::new(),
            parameter_handles: Arena::new(),
            state_handles: Arena::new(),
            signature_handles: Arena::new(),
            statement_handles: Arena::new(),
            machines: Arena::new(),
            platforms: Arena::new(),
            traits: Arena::new(),
        }
    }
}

impl DeclarationStorage {
    fn new() -> Self {
        Self {
            identifier_path_members: Arena::new(),
            type_parameters: Arena::new(),
            boundary_levels: Arena::new(),
            library_functions: Arena::new(),
            capability_members: Arena::new(),
            capability_contracts: Arena::new(),
            data_members: Arena::new(),
            data_payload_fields: Arena::new(),
            host_provider_mappings: Arena::new(),
            wire_data_members: Arena::new(),
            operators: Arena::new(),
            measures: Arena::new(),
            proof_facts: Arena::new(),
            target_host_settings: Arena::new(),
            boundary_policies: Arena::new(),
        }
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
    pub is_default: bool,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub effects: HandleSpan<Identifier>,
    pub contracts: HandleSpan<CapabilityContract>,
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
    pub satisfies: HandleSpan<Identifier>,
    pub effects: HandleSpan<Identifier>,
    pub contracts: HandleSpan<CapabilityContract>,
    pub states: HandleSpan<StateHandle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlatformNode {
    pub name: Identifier,
    pub states: HandleSpan<StateSignatureHandle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraitNode {
    pub is_boundary: bool,
    pub name: Identifier,
    pub requires: HandleSpan<Identifier>,
    pub machines: HandleSpan<StateSignatureHandle>,
}

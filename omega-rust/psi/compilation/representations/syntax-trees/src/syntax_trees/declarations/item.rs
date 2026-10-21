use crate::identifier::Identifier;
use arena::{Handle, HandleSpan, PagedArenaSet as Arena};
use language_core::operator_spelling::OperatorSpelling;

pub type ItemHandle = Handle<Item>;
pub type StateParameterHandle = Handle<StateParameterNode>;
pub type StateSignatureHandle = Handle<StateSignatureNode>;
pub type StateHandle = Handle<StateNode>;
pub type MachineHandle = Handle<MachineNode>;
pub type TraitHandle = Handle<TraitNode>;
pub type MathematicalDefinitionHandle = Handle<MathematicalDefinition>;
pub type MathematicalParameterHandle = Handle<MathematicalParameterNode>;
pub type MathematicalTypeHandle = Handle<MathematicalTypeNode>;

mod capabilities;
mod data_items;
mod machine_items;
mod proof_items;
pub use capabilities::*;
pub use data_items::*;
pub use machine_items::*;
pub use proof_items::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Capability(CapabilityDefinition),
    Conformance(ConformanceItem),
    Const(ConstDefinition),
    Data(DataDefinition),
    Domain(DomainDefinition),
    Measure(MeasureDefinition),
    Module(ModuleDeclaration),
    Operator(OperatorDefinition),
    Package(PackageDeclaration),
    Proposition(PropositionDefinition),
    Use(UseItem),
    Machine(Machine),
    Trait(TraitDefinition),
}

impl Item {
    /// The data definition, if this item is one.
    pub fn as_data(&self) -> Option<&DataDefinition> {
        let Item::Data(data) = self else {
            return None;
        };
        Some(data)
    }

    /// The domain definition, if this item is one.
    pub fn as_domain(&self) -> Option<&DomainDefinition> {
        let Item::Domain(domain) = self else {
            return None;
        };
        Some(domain)
    }

    /// The machine definition, if this item is one.
    pub fn as_machine(&self) -> Option<&Machine> {
        let Item::Machine(machine) = self else {
            return None;
        };
        Some(machine)
    }
}

/// A named compile-time pure value (wiki/spec/language/constants.md), either
/// package/module-scoped or type-scoped:
/// `const EfiStatus::SUCCESS: EfiStatus = EfiStatus { code: 0 };` — declared
/// like a machine (`Type::NAME`), never a `data` member, so never in `sizeof`.
/// Semantic evaluation may normalize an initializer before resolution while
/// retaining its authored expression and selected origins. Every
/// use substitutes a fresh copy of the initializer, so typed trees and
/// everything downstream never see a const value. Resolution retains only a
/// declaration-provenance symbol for authored-selection/package custody.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConstDefinition {
    /// The type scope (`EfiStatus` in `EfiStatus::SUCCESS`); empty text for
    /// a package/module-scoped declaration.
    pub scope: Identifier,
    pub name: Identifier,
    pub is_public: bool,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub value: crate::expression::ExpressionHandle,
    /// Present when semantic evaluation has replaced the authored initializer.
    pub normalization: Option<ConstInitializerNormalization>,
    /// The compile-time owner's ordinary checked return judgment for this exact
    /// completed initializer and declared qualification. Private probe forests
    /// clear this judgment when changing either input.
    pub qualification: Option<ConstQualification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstQualification {
    pub destination: crate::types::TypeReferenceHandle,
    pub initializer: crate::expression::ExpressionHandle,
}

/// Custody at one preparation boundary. Exact rosters have one owner: source
/// declaration readiness must never become an invocation or effect grant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConstInitializerCustody {
    pub selections: Vec<crate::types::ConstArgumentOrigin>,
    pub builtin_operators: Vec<source::SourceSpan>,
    pub call_selections: Vec<(source::SourceSpan, source::SourceSpan)>,
}

/// Authored syntax and exact source custody retained when a constant initializer
/// becomes an evaluated value. Source coordinates remain unchanged on copying;
/// the authored expression is remapped into the destination syntax arena.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConstInitializerNormalization {
    /// Original source closure before private implementation realization.
    pub authored_custody: ConstInitializerCustody,
    /// Complete selected declaration preparation, including uninvoked members.
    pub declaration_custody: ConstInitializerCustody,
    pub authored_expression: crate::expression::ExpressionHandle,
    pub result_value: language_core::constant_value::EvaluatedConstant,
    /// Realized value custody, reconstructed again before receiving execution.
    pub selections: Vec<crate::types::ConstArgumentOrigin>,
    pub builtin_operators: Vec<source::SourceSpan>,
    /// Original call occurrence and exact selected declaration source. These
    /// remain declaration selections, not builtin execution assertions.
    pub call_selections: Vec<(source::SourceSpan, source::SourceSpan)>,
}

/// Transitional bootstrap representation for the former magic
/// `Binding::Case(...)` parser. Durable source uses an ordinary `via`
/// expression and evaluates the compiler-owned closed `Binding` data sum.
/// The string-backed `DllImport` variant is removed entirely: authored
/// imports evaluate the compiler-owned `ForeignBinding::DllImport` value through a
/// `via` producer machine, while `VtableSlot` still survives its own
/// authored-spelling retirement for decoded artifacts and snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalBinding {
    /// Linux's stable ABI is the number table: `ForeignBinding::Syscall(1)`.
    Syscall { number: i64 },
    /// Compiler-known target operation. The resolved realization symbol,
    /// normalized signature, and target key the sealed lowering catalog.
    CompilerIntrinsic,
    /// Compatibility-only ordinal dispatch retained for decoded artifacts and
    /// snapshots. The parser rejects authored `Binding::VtableSlot` before its
    /// payload; new source uses a validated named `VtableField` instead.
    VtableSlot { index: i64 },
    /// COM/UEFI per-object dispatch by FIELD NAME (the field model, decided
    /// 2026-07-04; extern brief SS12.1). The attached provider data type owns
    /// the table layout; the policy computes the offset without magic slots.
    VtableField { field: Identifier },
    /// A SERVICE-TABLE function: `get_memory_map -> TableFunction(get_memory_map)`
    /// dispatches through the table's fn-ptr field like `VtableField`, but the
    /// table pointer is DISPATCH-ONLY -- never a wire argument (EFI table
    /// services take no This; protocol/COM methods do).
    TableFunction { field: Identifier },
}

impl Default for ExternalBinding {
    fn default() -> Self {
        Self::Syscall { number: 0 }
    }
}

impl Default for Item {
    fn default() -> Self {
        Self::Use(UseItem::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UseItem {
    pub path: HandleSpan<Identifier>,
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
pub struct ItemTable {
    items: Arena<Item>,
    /// Top-level `let`/`boundary let` declarations, kept beside `items` on
    /// their own root collection until the separately owned resolution leg
    /// admits them into `Item` dispatch.
    mathematical_definitions: Arena<MathematicalDefinition>,
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
    traits: Arena<TraitNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeclarationStorage {
    identifier_path_members: Arena<Identifier>,
    conformance_members: Arena<ConformanceMember>,
    satisfies_clauses: Arena<SatisfiesClause>,
    type_parameters: Arena<TypeParameter>,
    capability_members: Arena<CapabilityMember>,
    capability_contracts: Arena<CapabilityContract>,
    data_members: Arena<DataMember>,
    data_payload_fields: Arena<DataField>,
    operators: Arena<OperatorDefinition>,
    measures: Arena<MeasureDefinition>,
    proof_facts: Arena<ProofFact>,
    proof_fact_source_spans: Arena<Option<source::SourceSpan>>,
    mathematical_types: Arena<MathematicalTypeNode>,
    mathematical_parameters: Arena<MathematicalParameterNode>,
    mathematical_parameter_handles: Arena<MathematicalParameterHandle>,
}

impl ItemTable {
    pub fn new() -> Self {
        Self::new_with_generation(1)
    }

    pub fn new_with_generation(generation: u32) -> Self {
        Self {
            items: Arena::with_generation(generation),
            mathematical_definitions: Arena::with_generation(generation),
            state_storage: StateStorage::new_with_generation(generation),
            declaration_storage: DeclarationStorage::new_with_generation(generation),
        }
    }

    pub(crate) fn adopt(&mut self, other: Self) {
        self.items.adopt(other.items);
        self.mathematical_definitions
            .adopt(other.mathematical_definitions);
        self.state_storage.adopt(other.state_storage);
        self.declaration_storage.adopt(other.declaration_storage);
    }

    pub(crate) fn begin_page(&mut self, generation: u32) {
        self.items.begin_page(generation);
        self.mathematical_definitions.begin_page(generation);
        self.state_storage.begin_page(generation);
        self.declaration_storage.begin_page(generation);
    }

    pub(crate) fn page_count(&self) -> usize {
        self.items.page_count()
    }

    pub fn state_parameter(&self, handle: StateParameterHandle) -> &StateParameterNode {
        self.state_storage.parameters.get(handle)
    }

    #[inline]
    pub fn item(&self, handle: ItemHandle) -> &Item {
        self.items.get(handle)
    }

    pub fn mathematical_definition(
        &self,
        handle: MathematicalDefinitionHandle,
    ) -> &MathematicalDefinition {
        self.mathematical_definitions.get(handle)
    }

    pub fn append_mathematical_definition(
        &mut self,
        definition: MathematicalDefinition,
    ) -> MathematicalDefinitionHandle {
        self.mathematical_definitions.append(definition)
    }

    pub fn mathematical_type(&self, handle: MathematicalTypeHandle) -> &MathematicalTypeNode {
        self.declaration_storage.mathematical_types.get(handle)
    }

    pub fn insert_mathematical_type(
        &mut self,
        node: MathematicalTypeNode,
    ) -> MathematicalTypeHandle {
        self.declaration_storage.mathematical_types.append(node)
    }

    pub fn mathematical_parameter(
        &self,
        handle: MathematicalParameterHandle,
    ) -> &MathematicalParameterNode {
        self.declaration_storage.mathematical_parameters.get(handle)
    }

    pub fn insert_mathematical_parameter(
        &mut self,
        parameter: MathematicalParameterNode,
    ) -> MathematicalParameterHandle {
        self.declaration_storage
            .mathematical_parameters
            .append(parameter)
    }

    pub fn mathematical_parameters(
        &self,
        span: HandleSpan<MathematicalParameterHandle>,
    ) -> &[MathematicalParameterHandle] {
        self.declaration_storage
            .mathematical_parameter_handles
            .span_or_empty(span)
    }

    pub fn append_mathematical_parameter_handle(
        &mut self,
        handle: MathematicalParameterHandle,
    ) -> Handle<MathematicalParameterHandle> {
        self.declaration_storage
            .mathematical_parameter_handles
            .append(handle)
    }

    /// In-place item rewrite, the item-level twin of
    /// `ExpressionStorage::replace_expression` -- used by pre-resolution
    /// stages (the target-machine filter clears the selected target's marker).
    pub fn replace_item(&mut self, handle: ItemHandle, item: Item) {
        *self.items.get_mut(handle) = item;
    }

    pub fn state_signature(&self, handle: StateSignatureHandle) -> &StateSignatureNode {
        self.state_storage.signatures.get(handle)
    }

    pub fn state(&self, handle: StateHandle) -> &StateNode {
        self.state_storage.states.get(handle)
    }

    pub fn state_mut(&mut self, handle: StateHandle) -> &mut StateNode {
        self.state_storage.states.get_mut(handle)
    }

    pub fn machine(&self, handle: MachineHandle) -> &MachineNode {
        self.state_storage.machines.get(handle)
    }

    pub fn trait_definition(&self, handle: TraitHandle) -> &TraitNode {
        self.state_storage.traits.get(handle)
    }

    pub fn type_parameters(&self, span: HandleSpan<TypeParameter>) -> &[TypeParameter] {
        self.declaration_storage.type_parameters.span_or_empty(span)
    }

    pub fn type_parameters_mut(&mut self, span: HandleSpan<TypeParameter>) -> &mut [TypeParameter] {
        self.declaration_storage
            .type_parameters
            .span_mut_or_empty(span)
    }

    pub fn identifier_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
        self.declaration_storage
            .identifier_path_members
            .span_or_empty(span)
    }

    pub fn conformance_members(&self, span: HandleSpan<ConformanceMember>) -> &[ConformanceMember] {
        self.declaration_storage
            .conformance_members
            .span_or_empty(span)
    }

    pub fn satisfies_clauses(&self, span: HandleSpan<SatisfiesClause>) -> &[SatisfiesClause] {
        self.declaration_storage
            .satisfies_clauses
            .span_or_empty(span)
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

    pub fn operators(&self, span: HandleSpan<OperatorDefinition>) -> &[OperatorDefinition] {
        self.declaration_storage.operators.span_or_empty(span)
    }

    pub fn measures(&self, span: HandleSpan<MeasureDefinition>) -> &[MeasureDefinition] {
        self.declaration_storage.measures.span_or_empty(span)
    }

    pub fn proof_facts(&self, span: HandleSpan<ProofFact>) -> &[ProofFact] {
        self.declaration_storage.proof_facts.span_or_empty(span)
    }

    pub fn proof_fact(&self, handle: Handle<ProofFact>) -> &ProofFact {
        self.declaration_storage.proof_facts.get(handle)
    }

    pub fn proof_fact_source_span(&self, handle: Handle<ProofFact>) -> Option<source::SourceSpan> {
        *self
            .declaration_storage
            .proof_fact_source_spans
            .get(proof_fact_source_span_handle(handle))
    }

    pub fn set_proof_fact_source_span(
        &mut self,
        handle: Handle<ProofFact>,
        source_span: source::SourceSpan,
    ) {
        *self
            .declaration_storage
            .proof_fact_source_spans
            .get_mut(proof_fact_source_span_handle(handle)) = Some(source_span);
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

    pub fn append_conformance_member(
        &mut self,
        member: ConformanceMember,
    ) -> Handle<ConformanceMember> {
        self.declaration_storage.conformance_members.append(member)
    }

    pub fn append_satisfies_clause(&mut self, clause: SatisfiesClause) -> Handle<SatisfiesClause> {
        self.declaration_storage.satisfies_clauses.append(clause)
    }

    pub fn append_type_parameter(
        &mut self,
        type_parameter: TypeParameter,
    ) -> Handle<TypeParameter> {
        self.declaration_storage
            .type_parameters
            .append(type_parameter)
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

    pub fn append_operator(&mut self, operator: OperatorDefinition) -> Handle<OperatorDefinition> {
        self.declaration_storage.operators.append(operator)
    }

    pub fn append_proof_fact(&mut self, fact: ProofFact) -> Handle<ProofFact> {
        let handle = self.declaration_storage.proof_facts.append(fact);
        let source_span = self
            .declaration_storage
            .proof_fact_source_spans
            .append(None);
        debug_assert_eq!(source_span.arena_index(), handle.arena_index());
        debug_assert_eq!(source_span.generation(), handle.generation());
        handle
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
            spelling: signature.spelling,
            lifetime_parameters: signature.lifetime_parameters.clone(),
            type_parameters: signature.type_parameters,
            is_default: signature.is_default,
            parameters: signature.parameters,
            native_callback_parameters: signature.native_callback_parameters.clone(),
            return_type: signature.return_type,
            service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
            service_reach_keyword_source_spans: signature
                .service_reach_keyword_source_spans
                .clone(),
            service_reaches: signature.service_reaches,
            invokes: signature.invokes,
            suspends_keyword_source_spans: signature.suspends_keyword_source_spans.clone(),
            blocks_keyword_source_spans: signature.blocks_keyword_source_spans.clone(),
            suspends: signature.suspends,
            blocks: signature.blocks,
            contracts: signature.contracts,
            default_body: signature.default_body,
            terminates_guarantee: signature.terminates_guarantee,
            where_facts: signature.where_facts,
        })
    }

    pub fn insert_state(&mut self, state: &State) -> StateHandle {
        self.state_storage.states.append(StateNode {
            name: state.name.clone(),
            parameters: state.parameters,
            return_type: state.return_type,
            contracts: state.contracts,
            statements: state.statements,
        })
    }

    pub fn insert_machine(&mut self, machine: &Machine) -> MachineHandle {
        self.state_storage.machines.append(MachineNode {
            name: machine.name.clone(),
            satisfies: machine.satisfies,
            service_reach_keyword_source_spans: machine.service_reach_keyword_source_spans.clone(),
            service_reaches: machine.service_reaches,
            invokes: machine.invokes,
            suspends_keyword_source_spans: machine.suspends_keyword_source_spans.clone(),
            blocks_keyword_source_spans: machine.blocks_keyword_source_spans.clone(),
            suspends: machine.suspends,
            blocks: machine.blocks,
            contracts: machine.contracts,
            states: machine.states,
        })
    }

    pub fn insert_trait_definition(&mut self, trait_definition: &TraitDefinition) -> TraitHandle {
        self.state_storage.traits.append(TraitNode {
            is_boundary: trait_definition.is_boundary,
            name: trait_definition.name.clone(),
            parents: trait_definition.parents,
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
    fn new_with_generation(generation: u32) -> Self {
        Self {
            parameters: Arena::with_generation(generation),
            signatures: Arena::with_generation(generation),
            states: Arena::with_generation(generation),
            parameter_handles: Arena::with_generation(generation),
            state_handles: Arena::with_generation(generation),
            signature_handles: Arena::with_generation(generation),
            statement_handles: Arena::with_generation(generation),
            machines: Arena::with_generation(generation),
            traits: Arena::with_generation(generation),
        }
    }

    fn adopt(&mut self, other: Self) {
        self.parameters.adopt(other.parameters);
        self.signatures.adopt(other.signatures);
        self.states.adopt(other.states);
        self.parameter_handles.adopt(other.parameter_handles);
        self.state_handles.adopt(other.state_handles);
        self.signature_handles.adopt(other.signature_handles);
        self.statement_handles.adopt(other.statement_handles);
        self.machines.adopt(other.machines);
        self.traits.adopt(other.traits);
    }

    fn begin_page(&mut self, generation: u32) {
        self.parameters.begin_page(generation);
        self.signatures.begin_page(generation);
        self.states.begin_page(generation);
        self.parameter_handles.begin_page(generation);
        self.state_handles.begin_page(generation);
        self.signature_handles.begin_page(generation);
        self.statement_handles.begin_page(generation);
        self.machines.begin_page(generation);
        self.traits.begin_page(generation);
    }
}

impl DeclarationStorage {
    fn new_with_generation(generation: u32) -> Self {
        Self {
            identifier_path_members: Arena::with_generation(generation),
            conformance_members: Arena::with_generation(generation),
            satisfies_clauses: Arena::with_generation(generation),
            type_parameters: Arena::with_generation(generation),
            capability_members: Arena::with_generation(generation),
            capability_contracts: Arena::with_generation(generation),
            data_members: Arena::with_generation(generation),
            data_payload_fields: Arena::with_generation(generation),
            operators: Arena::with_generation(generation),
            measures: Arena::with_generation(generation),
            proof_facts: Arena::with_generation(generation),
            proof_fact_source_spans: Arena::with_generation(generation),
            mathematical_types: Arena::with_generation(generation),
            mathematical_parameters: Arena::with_generation(generation),
            mathematical_parameter_handles: Arena::with_generation(generation),
        }
    }

    fn adopt(&mut self, other: Self) {
        self.identifier_path_members
            .adopt(other.identifier_path_members);
        self.conformance_members.adopt(other.conformance_members);
        self.satisfies_clauses.adopt(other.satisfies_clauses);
        self.type_parameters.adopt(other.type_parameters);
        self.capability_members.adopt(other.capability_members);
        self.capability_contracts.adopt(other.capability_contracts);
        self.data_members.adopt(other.data_members);
        self.data_payload_fields.adopt(other.data_payload_fields);
        self.operators.adopt(other.operators);
        self.measures.adopt(other.measures);
        self.proof_facts.adopt(other.proof_facts);
        self.proof_fact_source_spans
            .adopt(other.proof_fact_source_spans);
        self.mathematical_types.adopt(other.mathematical_types);
        self.mathematical_parameters
            .adopt(other.mathematical_parameters);
        self.mathematical_parameter_handles
            .adopt(other.mathematical_parameter_handles);
    }

    fn begin_page(&mut self, generation: u32) {
        self.identifier_path_members.begin_page(generation);
        self.conformance_members.begin_page(generation);
        self.satisfies_clauses.begin_page(generation);
        self.type_parameters.begin_page(generation);
        self.capability_members.begin_page(generation);
        self.capability_contracts.begin_page(generation);
        self.data_members.begin_page(generation);
        self.data_payload_fields.begin_page(generation);
        self.operators.begin_page(generation);
        self.measures.begin_page(generation);
        self.proof_facts.begin_page(generation);
        self.proof_fact_source_spans.begin_page(generation);
        self.mathematical_types.begin_page(generation);
        self.mathematical_parameters.begin_page(generation);
        self.mathematical_parameter_handles.begin_page(generation);
    }
}

fn proof_fact_source_span_handle(handle: Handle<ProofFact>) -> Handle<Option<source::SourceSpan>> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

/// One authored signature parameter. `relevance` is the binding
/// occurrence's `[erased]` marker (contracts.md#explicit-erased-bindings):
/// an erased parameter stays in checking, contracts, and semantic identity
/// but owns no runtime storage, transfer, or read.
/// Every parameter is explicit; carrier qualification creates no receiver row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateParameterNode {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub is_const: bool,
    pub is_mutable: bool,
    pub relevance: language_core::BindingRelevance,
}

/// One compiler-private native callback parameter declared on a bodyless
/// boundary requirement. It participates in the authored native telescope but
/// is not an Omega runtime parameter and therefore has no type-reference or
/// source-call argument.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeCallbackParameterNode {
    pub name: Identifier,
    pub binder: Identifier,
    pub native_ordinal: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateSignatureNode {
    pub name: Identifier,
    pub spelling: Option<crate::operator_spelling::OperatorSpelling>,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub is_default: bool,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub native_callback_parameters: Vec<NativeCallbackParameterNode>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub service_reach_is_installation_bound: bool,
    pub service_reach_keyword_source_spans: Vec<source::SourceSpan>,
    pub service_reaches: HandleSpan<Identifier>,
    pub invokes: HandleSpan<Identifier>,
    pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
    pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: HandleSpan<CapabilityContract>,
    pub default_body: HandleSpan<crate::statement::StatementHandle>,
    /// TPR4 (decision 23): the bodyless requirement's authored guarantee.
    pub terminates_guarantee: bool,
    /// Signature-level `where` proof facts retained verbatim through storage.
    pub where_facts: HandleSpan<ProofFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateNode {
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub contracts: HandleSpan<CapabilityContract>,
    pub statements: HandleSpan<crate::statement::StatementHandle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineNode {
    pub name: Identifier,
    pub satisfies: HandleSpan<SatisfiesClause>,
    pub service_reach_keyword_source_spans: Vec<source::SourceSpan>,
    pub service_reaches: HandleSpan<Identifier>,
    pub invokes: HandleSpan<Identifier>,
    pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
    pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: HandleSpan<CapabilityContract>,
    pub states: HandleSpan<StateHandle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraitNode {
    pub is_boundary: bool,
    pub name: Identifier,
    pub parents: HandleSpan<crate::types::TypeReferenceHandle>,
    pub requires: HandleSpan<Identifier>,
    pub machines: HandleSpan<StateSignatureHandle>,
}

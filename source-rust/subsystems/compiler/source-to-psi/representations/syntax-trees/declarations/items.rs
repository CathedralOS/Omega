use crate::expression::ExpressionHandle;
use crate::identifier::Identifier;
use crate::paged_roster::paged_roster;
use crate::statement::StatementHandle;
use crate::types::TypeReferenceHandle;
use arena::{Handle, HandleSpan};
use language_core::operator_spelling::OperatorSpelling;

pub type ItemHandle = Handle<Item>;
pub type StateParameterHandle = Handle<StateParameterNode>;
pub type StateSignatureHandle = Handle<StateSignature>;
pub type StateHandle = Handle<State>;
pub type MathematicalDefinitionHandle = Handle<MathematicalDefinition>;
pub type MathematicalParameterHandle = Handle<MathematicalParameterNode>;
pub type MathematicalTypeHandle = Handle<MathematicalTypeNode>;

pub use capabilities::*;
pub use conformances::*;
pub use constant_preparation::*;
pub use constants::*;
pub use data_items::*;
pub use domains::*;
pub use machine_items::*;
pub use parameters::*;
pub use proof_items::*;
pub use type_aliases::*;

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub enum Item {
    Capability(CapabilityDefinition),
    Conformance(ConformanceItem),
    Const(ConstDefinition),
    Data(DataDefinition),
    Domain(DomainDefinition),
    Measure(MeasureDefinition),
    Operator(OperatorDefinition),
    Package(PackageDeclaration),
    Proposition(PropositionDefinition),
    Use(UseItem),
    Machine(Machine),
    Trait(TraitDefinition),
    TypeAlias(TypeAliasDefinition),
}

impl Item {
    pub fn as_data(&self) -> Option<&DataDefinition> {
        let Item::Data(data) = self else {
            return None;
        };
        Some(data)
    }

    pub fn as_domain(&self) -> Option<&DomainDefinition> {
        let Item::Domain(domain) = self else {
            return None;
        };
        Some(domain)
    }

    pub fn as_machine(&self) -> Option<&Machine> {
        let Item::Machine(machine) = self else {
            return None;
        };
        Some(machine)
    }
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub enum ExternalBinding {
    Syscall { number: i64 },
    CompilerIntrinsic,
    VtableSlot { index: i64 },
    VtableField { field: Identifier },
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

#[derive(
    Debug,
    Default,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct UseItem {
    pub root: Option<Identifier>,
    pub components: HandleSpan<Identifier>,
    pub alias: Identifier,
    pub qualifiers: Vec<FileQualifier>,
    pub span: source_text::SourceSpan,
}

/// One written use of a file alias, at its opening bar in the importing file.
/// A designated Product operand names a declaration outside the running Build.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct FileQualifier {
    pub offset: usize,
    pub role: FileQualifierRole,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub enum FileQualifierRole {
    Code,
    Product,
    /// Ordinary Build reflection when the declaration is supplied to the Build.
    /// Otherwise the genuine core operation may select a Product declaration.
    Description,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct PackageDeclaration {
    pub path: HandleSpan<Identifier>,
}

paged_roster! {
    pub struct ItemTable => pub(crate) ItemTableAppendCheckpoint {
        arena items: Item,
        arena mathematical_definitions: MathematicalDefinition,
        roster state_storage: StateStorage => StateStorageAppendCheckpoint,
        roster declaration_storage: DeclarationStorage => DeclarationStorageAppendCheckpoint,
    }
}

paged_roster! {
    struct StateStorage => pub(crate) StateStorageAppendCheckpoint {
        arena parameters: StateParameterNode,
        arena signatures: StateSignature,
        arena states: State,
        arena parameter_handles: StateParameterHandle,
        arena state_handles: StateHandle,
        arena signature_handles: StateSignatureHandle,
        arena statement_handles: StatementHandle,
    }
}

paged_roster! {
    struct DeclarationStorage => pub(crate) DeclarationStorageAppendCheckpoint {
        arena identifier_path_members: Identifier,
        arena conformance_members: ConformanceMember,
        arena satisfies_clauses: SatisfiesClause,
        arena type_parameters: TypeParameter,
        arena capability_members: CapabilityMember,
        arena capability_contracts: CapabilityContract,
        arena data_members: DataMember,
        arena data_payload_fields: DataField,
        arena operators: OperatorDefinition,
        arena measures: MeasureDefinition,
        arena proof_facts: ProofFact,
        arena proof_fact_source_spans: Option<source_text::SourceSpan>,
        arena mathematical_types: MathematicalTypeNode,
        arena mathematical_parameters: MathematicalParameterNode,
        arena mathematical_parameter_handles: MathematicalParameterHandle,
    }
}

#[derive(Clone, Copy)]
pub enum StatementBody {
    State(StateHandle),
    TraitDefault(StateSignatureHandle),
}

impl ItemTable {
    pub fn new() -> Self {
        Self::new_with_generation(1)
    }

    pub(crate) fn page_count(&self) -> usize {
        self.items.page_count()
    }

    pub(crate) fn first_generation(&self) -> u32 {
        self.items.first_generation()
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

    pub fn replace_item(&mut self, handle: ItemHandle, item: Item) {
        *self.items.get_mut(handle) = item;
    }

    pub fn state_signature(&self, handle: StateSignatureHandle) -> &StateSignature {
        self.state_storage.signatures.get(handle)
    }

    pub fn state(&self, handle: StateHandle) -> &State {
        self.state_storage.states.get(handle)
    }

    pub fn state_mut(&mut self, handle: StateHandle) -> &mut State {
        self.state_storage.states.get_mut(handle)
    }

    pub fn type_parameter_entries(&self) -> impl Iterator<Item = &TypeParameter> {
        self.declaration_storage
            .type_parameters
            .iter()
            .map(|(_, parameter)| parameter)
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

    pub fn requirement_facts(&self, contracts: HandleSpan<CapabilityContract>) -> Vec<ProofFact> {
        self.capability_contracts(contracts)
            .iter()
            .filter(|contract| contract.kind == CapabilityContractKind::Requires)
            .flat_map(|contract| self.proof_facts(contract.facts).iter().cloned())
            .collect()
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

    pub fn proof_fact_source_span(
        &self,
        handle: Handle<ProofFact>,
    ) -> Option<source_text::SourceSpan> {
        *self
            .declaration_storage
            .proof_fact_source_spans
            .get(proof_fact_source_span_handle(handle))
    }

    pub fn set_proof_fact_source_span(
        &mut self,
        handle: Handle<ProofFact>,
        source_span: source_text::SourceSpan,
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

    /// Bodies published by declarations. A nested machine owns a state just like a root machine.
    pub fn statement_bodies(
        &self,
    ) -> impl Iterator<Item = (StatementBody, HandleSpan<StatementHandle>)> {
        self.state_storage
            .states
            .iter()
            .map(|(owner, state)| (StatementBody::State(owner), state.statements))
            .chain(
                self.state_storage
                    .signatures
                    .iter()
                    .filter_map(|(owner, signature)| {
                        let body = signature.default_body?;
                        Some((StatementBody::TraitDefault(owner), body.statements))
                    }),
            )
    }

    pub fn set_statement_body(&mut self, owner: StatementBody, body: HandleSpan<StatementHandle>) {
        match owner {
            StatementBody::State(owner) => {
                self.state_storage.states.get_mut(owner).statements = body
            }
            StatementBody::TraitDefault(owner) => {
                self.state_storage.signatures.get_mut(owner)
                    .default_body.as_mut().expect("selected trait default").statements = body
            }
        }
    }

    pub fn statements(&self, span: HandleSpan<StatementHandle>) -> &[StatementHandle] {
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

    pub fn append_statement_handle(&mut self, handle: StatementHandle) -> Handle<StatementHandle> {
        self.state_storage.statement_handles.append(handle)
    }

    /// The position the next body statement handle is appended at.
    pub fn statement_handle_frontier(&self) -> arena::PagedArenaFrontier<StatementHandle> {
        self.state_storage.statement_handles.frontier()
    }

    /// Remove the body statement handles appended since `frontier` and return
    /// them in order. A block value parses its statements with the ordinary
    /// statement grammar, which appends to the enclosing body's span; the
    /// block takes them back so that span stays contiguous.
    pub fn detach_statement_handles_since(
        &mut self,
        frontier: arena::PagedArenaFrontier<StatementHandle>,
    ) -> Vec<StatementHandle> {
        let appended = self.state_storage.statement_handles.span_since(frontier);
        let handles = self
            .state_storage
            .statement_handles
            .span_or_empty(appended)
            .to_vec();
        self.state_storage
            .statement_handles
            .rewind_appended(frontier);
        handles
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

    pub fn insert_state_signature(&mut self, signature: &StateSignature) -> StateSignatureHandle {
        self.state_storage.signatures.append(signature.clone())
    }

    pub fn insert_state(&mut self, state: &State) -> StateHandle {
        self.state_storage.states.append(state.clone())
    }
}

impl Default for ItemTable {
    fn default() -> Self {
        Self::new()
    }
}

fn proof_fact_source_span_handle(
    handle: Handle<ProofFact>,
) -> Handle<Option<source_text::SourceSpan>> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct StateParameterNode {
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub is_const: bool,
    pub is_mutable: bool,
    pub relevance: language_core::BindingRelevance,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct NativeCallbackParameterNode {
    pub name: Identifier,
    pub binder: Identifier,
    pub native_ordinal: u32,
}

#[path = "constants.rs"]
pub(crate) mod constants;

#[path = "constant_preparation.rs"]
pub(crate) mod constant_preparation;

#[path = "capabilities.rs"]
pub(crate) mod capabilities;

#[path = "conformances.rs"]
pub(crate) mod conformances;

#[path = "domains.rs"]
pub(crate) mod domains;

#[path = "parameters.rs"]
pub(crate) mod parameters;

#[path = "data.rs"]
pub(crate) mod data_items;

#[path = "machines.rs"]
pub(crate) mod machine_items;

#[path = "proofs.rs"]
pub(crate) mod proof_items;

#[path = "type_aliases.rs"]
pub(crate) mod type_aliases;

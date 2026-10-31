use crate::identifier::Identifier;
use crate::syntax_trees::paged_roster::paged_roster;
use arena::{Handle, HandleSpan};
use language_core::operator_spelling::OperatorSpelling;

pub type ItemHandle = Handle<Item>;
pub type StateParameterHandle = Handle<StateParameterNode>;
pub type StateSignatureHandle = Handle<StateSignature>;
pub type StateHandle = Handle<State>;
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

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
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
#[derive(
    Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
)]
pub struct ConstDefinition {
    /// The type scope (`EfiStatus` in `EfiStatus::SUCCESS`); empty text for
    /// a package/module-scoped declaration.
    pub scope: Identifier,
    pub name: Identifier,
    pub is_public: bool,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub value: crate::expression::ExpressionHandle,
    /// Declaration readiness found a target observation in this initializer
    /// closure. This delays evaluation; it grants no target or call authority.
    pub pending_target_observation: bool,
    /// Present when semantic evaluation has replaced the authored initializer.
    pub normalization: Option<ConstInitializerNormalization>,
    /// The compile-time owner's ordinary checked return judgment for this exact
    /// completed initializer and declared qualification. Private probe forests
    /// clear this judgment when changing either input.
    pub qualification: Option<ConstQualification>,
    /// An encoded private payload still owes this exact full destination.
    /// No public/index consumer may retire it without ordinary checking.
    pub pending_qualification: Option<ConstDestinationObligation>,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    compiler_storage::Record,
)]
pub struct ConstQualification {
    pub destination: crate::types::TypeReferenceHandle,
    pub initializer: crate::expression::ExpressionHandle,
}

/// A required checked destination judgment, never an accepted receipt.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    compiler_storage::Record,
)]
pub struct ConstDestinationObligation {
    pub declaration: source::SourceSpan,
    pub destination: crate::types::TypeReferenceHandle,
    pub initializer: crate::expression::ExpressionHandle,
}

/// Custody at one preparation boundary. Exact rosters have one owner: source
/// declaration readiness must never become an invocation or effect grant.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ConstInitializerCustody {
    pub selections: Vec<crate::types::ConstArgumentOrigin>,
    pub builtin_operators: Vec<source::SourceSpan>,
    pub call_selections: Vec<(source::SourceSpan, source::SourceSpan)>,
}

/// Authored syntax and exact source custody retained when a constant initializer
/// becomes an evaluated value. Source coordinates remain unchanged on copying;
/// the authored expression is remapped into the destination syntax arena.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    compiler_storage::Record,
)]
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
#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
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

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    syntax_trees_derive::PlainSyntaxEquivalent,
    compiler_storage::Record,
)]
pub struct UseItem {
    pub path: HandleSpan<Identifier>,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    syntax_trees_derive::PlainSyntaxEquivalent,
    compiler_storage::Record,
)]
pub struct ModuleDeclaration {
    pub path: HandleSpan<Identifier>,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    syntax_trees_derive::PlainSyntaxEquivalent,
    compiler_storage::Record,
)]
pub struct PackageDeclaration {
    pub path: HandleSpan<Identifier>,
}

paged_roster! {
    /// The item roster: `items`, the top-level `let`/`boundary let`
    /// `mathematical_definitions` kept beside them on their own root collection
    /// until the separately owned resolution leg admits them into `Item`
    /// dispatch, and the state and declaration storages they point into.
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
        arena statement_handles: crate::statement::StatementHandle,
        arena machines: MachineNode,
        arena traits: TraitNode,
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
        arena proof_fact_source_spans: Option<source::SourceSpan>,
        arena mathematical_types: MathematicalTypeNode,
        arena mathematical_parameters: MathematicalParameterNode,
        arena mathematical_parameter_handles: MathematicalParameterHandle,
    }
}

impl ItemTable {
    pub fn new() -> Self {
        Self::new_with_generation(1)
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

    pub fn state_signature(&self, handle: StateSignatureHandle) -> &StateSignature {
        self.state_storage.signatures.get(handle)
    }

    pub fn state(&self, handle: StateHandle) -> &State {
        self.state_storage.states.get(handle)
    }

    pub fn state_mut(&mut self, handle: StateHandle) -> &mut State {
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

    pub fn data_members_mut(&mut self, span: HandleSpan<DataMember>) -> &mut [DataMember] {
        self.declaration_storage
            .data_members
            .span_mut_or_empty(span)
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
        self.state_storage.signatures.append(signature.clone())
    }

    pub fn insert_state(&mut self, state: &State) -> StateHandle {
        self.state_storage.states.append(state.clone())
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

fn proof_fact_source_span_handle(handle: Handle<ProofFact>) -> Handle<Option<source::SourceSpan>> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

/// One authored signature parameter. `relevance` is the binding
/// occurrence's `[erased]` marker (contracts.md#explicit-erased-bindings):
/// an erased parameter stays in checking, contracts, and semantic identity
/// but owns no runtime storage, transfer, or read.
/// Every parameter is explicit; carrier qualification creates no receiver row.
#[derive(
    Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
)]
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
#[derive(
    Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
)]
pub struct NativeCallbackParameterNode {
    pub name: Identifier,
    pub binder: Identifier,
    pub native_ordinal: u32,
}

#[derive(
    Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
)]
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

#[derive(
    Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
)]
pub struct TraitNode {
    pub is_boundary: bool,
    pub name: Identifier,
    pub parents: HandleSpan<crate::types::TypeReferenceHandle>,
    pub requires: HandleSpan<Identifier>,
    pub machines: HandleSpan<StateSignatureHandle>,
}

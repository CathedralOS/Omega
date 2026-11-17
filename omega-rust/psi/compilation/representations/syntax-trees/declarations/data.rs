use crate::item::*;

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct DataDefinition {
    pub name: Identifier,
    pub is_public: bool,
    pub supply_mode: language_core::DataSupplyMode,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub generic_instance: Option<crate::types::TypeReferenceHandle>,
    pub conformance_bounds: Vec<crate::item::GenericConformanceBound>,
    pub properties: DataProperties,
    pub quotient: Option<QuotientDefinition>,
    pub cleanup_selection: Option<crate::expression::StaticMachineArgument>,
    pub requires_facts: HandleSpan<ProofFact>,
    pub members: HandleSpan<DataMember>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct QuotientDefinition {
    pub carrier: crate::types::TypeReferenceHandle,
    pub relation: HandleSpan<Identifier>,
    pub equivalence: Option<QuotientEquivalenceSelection>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct QuotientEquivalenceSelection {
    pub relation: HandleSpan<Identifier>,
    pub trait_name: Identifier,
    pub trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub conformance_name: Identifier,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct ConformanceItem {
    pub is_public: bool,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub trait_name: Identifier,
    pub trait_lifetime_arguments: Vec<Identifier>,
    pub trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub alias: Option<Identifier>,
    pub members: HandleSpan<ConformanceMember>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum ConformanceMember {
    Machine(Machine),
    TraitDefault {
        declaring_trait: Identifier,
        requirement_ordinal: usize,
        machine: Machine,
    },
    Synthesized(Machine),
    /*
        A selection row, `Trait::member = Existing::machine;`. The
        implementation is a record of selected operations
        (wiki/spec/language/conformances.md, "Implementations are
        operation records"), so this slot holds the named machine
        itself: no member machine is declared, and the selected
        declaration keeps its own identity, contract and laws.
    */
    Selection {
        declaring_trait: Identifier,
        requirement: Identifier,
        target: HandleSpan<Identifier>,
    },
}

impl Default for ConformanceMember {
    fn default() -> Self {
        Self::Machine(Machine::default())
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct DataProperties {
    pub multiplicity: language_core::Multiplicity,
    pub carry: Option<language_core::CarryPolicy>,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct TypeParameter {
    pub name: Identifier,
    pub kind: TypeParameterKind,
    pub bounds: DataProperties,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum TypeParameterKind {
    #[default]
    Type,
    Implementation,
    Const {
        type_reference: crate::types::TypeReferenceHandle,
    },
    Value {
        type_reference: crate::types::TypeReferenceHandle,
    },
    Machine {
        contract: Option<MachineParameterContract>,
    },
    Proposition {
        contract: Option<PropositionParameterSignature>,
    },
}

impl TypeParameterKind {
    pub fn is_const_or_value(&self) -> bool {
        matches!(self, Self::Const { .. } | Self::Value { .. })
    }
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum MachineParameterContract {
    /// An authored `const F: machine`; exact typed uses must supply its type.
    Inferred,
    RequirementIdentity,
    Structural(StateSignature),
    Nominal {
        requirement: HandleSpan<Identifier>,
        trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    },
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct PropositionParameterSignature {
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameterHandle>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum DataMember {
    Field(DataField),
    Variant(DataVariant),
    Retired(u64),
}

impl Default for DataMember {
    fn default() -> Self {
        Self::Variant(DataVariant::default())
    }
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct DataField {
    pub identity: Option<u64>,
    pub name: Identifier,
    pub relevance: language_core::BindingRelevance,
    pub type_reference: crate::types::TypeReferenceHandle,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct DataVariant {
    pub identity: Option<u64>,
    pub name: Identifier,
    pub payload: HandleSpan<DataField>,
    pub requires_facts: HandleSpan<ProofFact>,
    pub retired_payload_identities: Vec<u64>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct DomainDefinition {
    pub name: Identifier,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub target_type: crate::types::TypeReferenceHandle,
    pub index_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub is_public: bool,
    pub alias: Option<DomainAliasDefinition>,
    pub cleanup_selection: Option<crate::expression::StaticMachineArgument>,
    pub authored_routes: Vec<Vec<Identifier>>,
    /// The policy named after `access`: its plan states who may
    /// introduce the domain and who may read or write what it governs.
    pub access_policy: Option<crate::expression::StaticMachineArgument>,
    pub classification: Option<language_core::DomainClassification>,
    pub predicate_presence: language_core::DomainPredicatePresence,
    pub facts: HandleSpan<ProofFact>,
    pub operators: HandleSpan<OperatorDefinition>,
    pub semantic_clause_token_count: usize,
}

#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct DomainAliasDefinition {
    pub constituents: Vec<HandleSpan<Identifier>>,
}

impl Default for DomainDefinition {
    fn default() -> Self {
        Self {
            name: Identifier::generated(""),
            type_parameters: HandleSpan::empty(),
            target_type: crate::types::TypeReferenceHandle::invalid(),
            index_arguments: HandleSpan::empty(),
            is_public: false,
            alias: None,
            cleanup_selection: None,
            authored_routes: Vec::new(),
            access_policy: None,
            classification: None,
            predicate_presence: language_core::DomainPredicatePresence::Absent,
            facts: HandleSpan::empty(),
            operators: HandleSpan::empty(),
            semantic_clause_token_count: 0,
        }
    }
}

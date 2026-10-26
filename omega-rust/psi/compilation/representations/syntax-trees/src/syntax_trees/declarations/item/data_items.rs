use super::*;

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub struct DataDefinition {
    pub name: Identifier,
    pub is_public: bool,
    pub supply_mode: language_core::DataSupplyMode,
    /// Erased borrow-region parameters declared in the shared `<>` list.
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    /// Exact structural origin for a concrete generic instance synthesized by
    /// normalization. Authored data declarations never populate this field.
    pub generic_instance: Option<crate::types::TypeReferenceHandle>,
    pub properties: DataProperties,
    /// N6: a proof-only quotient declaration (`data Q = Carrier % relation;`).
    /// Quotients have no authored members: their values are equivalence classes
    /// of carrier values, and the relation path names the ordinary proof machine
    /// whose equivalence obligations admit the declaration.
    pub quotient: Option<QuotientDefinition>,
    /// R2 rung 1 (ch12 "Dependent Data"): the DEFAULT-DOMAIN facts --
    /// `data M where count * stride <= len, { ... }` -- bare field names,
    /// any number of facts, holding at every observation. Parsed and
    /// stored; the syntax->resolved lowering refuses them loudly until R2
    /// rung 2 consumes the model (never a silent drop).
    /// Owner-authored exact independent cleanup application.
    pub cleanup_selection: Option<crate::expression::StaticMachineArgument>,
    pub where_facts: HandleSpan<ProofFact>,
    pub members: HandleSpan<DataMember>,
}

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub struct QuotientDefinition {
    pub carrier: crate::types::TypeReferenceHandle,
    pub relation: HandleSpan<Identifier>,
    /// Exact declaration-site selection from
    /// `where R satisfies Equivalence<C, R> as Name;`.
    pub equivalence: Option<QuotientEquivalenceSelection>,
}

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub struct QuotientEquivalenceSelection {
    pub relation: HandleSpan<Identifier>,
    pub trait_name: Identifier,
    pub trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub conformance_name: Identifier,
}

/// A named implementation: `Primary satisfies Shape<Point> { ... }`. The name
/// identifies a closed map of machines; it owns no data subject, and the
/// trait's types are ordinary arguments of its application. Every declaration
/// carries its complete member block, so no row is discovered from machines
/// attached elsewhere.
#[derive(Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent)]
pub struct ConformanceItem {
    /// Source visibility for the complete name-first conformance declaration.
    /// Exact machine requirement-satisfier edges own no standalone visibility.
    pub is_public: bool,
    /// Generic binders belong to the declared implementation name, not to its
    /// trait application.
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub trait_name: Identifier,
    /// Complete declaration-site lifetime application of `trait_name`, in the
    /// target trait's declaration order. Every name must refer to this
    /// conformance's own lifetime telescope.
    pub trait_lifetime_arguments: Vec<Identifier>,
    pub trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub alias: Option<Identifier>,
    /// The authored, closed implementation surface.
    pub members: HandleSpan<ConformanceMember>,
}

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub enum ConformanceMember {
    /// A machine declared lexically inside the block. Its declaration name is
    /// the requirement slot it fills; later normalization retains an internal
    /// conformance-qualified realization identity.
    Machine(Machine),
    /// A trait-authored default instantiated for this closed conformance.
    /// This variant is synthesized before symbol resolution; it is never
    /// accepted directly from source. Keeping the origin trait explicit makes
    /// inherited same-name requirements and artifact provenance exact.
    TraitDefault {
        declaring_trait: Identifier,
        /// Source-order identity of the exact requirement declaration within
        /// its declaring trait. This survives only until symbol assignment;
        /// the normalized row retains the requirement symbol instead.
        requirement_ordinal: usize,
        machine: Machine,
    },
    /// A sealed core-trait row the compiler emits as an ordinary checked
    /// machine body: the structural `equals` of an `Equatable<T>`
    /// implementation that authors no `equals` member or reference row. It is
    /// synthesized before symbol resolution and never accepted from source;
    /// typed lowering expands uses of this row structurally.
    Synthesized(Machine),
    /// An explicit row reference such as
    /// `Ranked::rank_value = Card::stable_rank_value;`.
    Reference {
        declaring_trait: Identifier,
        requirement: Identifier,
        target: HandleSpan<Identifier>,
    },
}

impl Default for ConformanceMember {
    fn default() -> Self {
        Self::Reference {
            declaring_trait: Identifier::generated(""),
            requirement: Identifier::generated(""),
            target: HandleSpan::empty(),
        }
    }
}

/// Declared type properties: lowercase facts in brackets on the data
/// declaration (`data Point [copy]`). The known set is closed;
/// unknown names are parse errors, so downstream representations carry the
/// resolved properties rather than spellings. `sized` is computed from the
/// shape and may not be declared.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, syntax_trees_derive::PlainSyntaxEquivalent,
)]
pub struct DataProperties {
    /// Usage multiplicity. Ordinary data defaults to affine; `[copy]` selects
    /// unrestricted and `[linear]` selects exact consumption. Keeping the enum
    /// here prevents syntax lowering from reconstructing semantic identity
    /// from compatibility booleans.
    pub multiplicity: language_core::Multiplicity,
    /// Authored carry-policy floor. Omission remains distinct from an authored
    /// strict policy so transparent derivation and opaque admission can choose
    /// their respective establishment paths later.
    pub carry: Option<language_core::CarryPolicy>,
}

#[derive(Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent)]
pub struct TypeParameter {
    pub name: Identifier,
    pub kind: TypeParameterKind,
    /// Property bounds in brackets after the parameter name (frozen decision
    /// 13): `data Box<T [copy]>`. Brackets attach to what they follow, so the
    /// bound list reuses the declared-property shape of the data declaration.
    pub bounds: DataProperties,
}

#[derive(Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent)]
pub enum TypeParameterKind {
    #[default]
    Type,
    /// A trait's explicitly selected implementation argument. Its exact
    /// contract is retained in the owner's conformance bounds; keeping this
    /// slot in the telescope preserves mixed argument declaration order.
    Implementation,
    Const {
        type_reference: crate::types::TypeReferenceHandle,
    },
    /// A runtime-capable value binder (`Count: u32`, no `const` keyword).
    /// Unlike `Const` it may be supplied at runtime; the dynamic realization
    /// path is not implemented yet, so only static arguments specialize.
    Value {
        type_reference: crate::types::TypeReferenceHandle,
    },
    /// A compile-time machine-symbol parameter. The authored `where machine`
    /// requirement is mandatory and stored with the parameter rather than
    /// inferred from uses or instantiations. `None` exists only while the
    /// parser is between `<machine M>` and its declaration-site validation.
    Machine {
        contract: Option<MachineParameterContract>,
    },
    /// A proof-formula parameter with a mandatory declaration-site
    /// application signature. It is never an executable callable.
    Proposition {
        contract: Option<PropositionParameterSignature>,
    },
}

impl TypeParameterKind {
    /// The parameter binds a compile-time value — a static const identity or
    /// a runtime-capable value that today only specializes statically.
    pub fn is_const_or_value(&self) -> bool {
        matches!(self, Self::Const { .. } | Self::Value { .. })
    }
}

/// Authored declaration-site contract for a static machine parameter.
/// Structural contracts own their complete signature locally. Nominal
/// contracts retain only the exact signature-free requirement path; symbol
/// resolution binds that path once declarations and trait requirements exist.
#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub enum MachineParameterContract {
    /// A trait-level static parameter whose arguments must name one exact
    /// signature-free trait requirement. This is declaration identity, not an
    /// executable machine contract.
    RequirementIdentity,
    Structural(StateSignature),
    /// `where machine Step satisfies Visitor<Context>::visit;`: the exact
    /// requirement path plus the trait application the binder's callable
    /// contract is instantiated under. Empty arguments for a parameterless
    /// trait.
    Nominal {
        requirement: HandleSpan<Identifier>,
        trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    },
}

#[derive(Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent)]
pub struct PropositionParameterSignature {
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameterHandle>,
}

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
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

#[derive(Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent)]
pub struct DataField {
    pub identity: Option<u64>,
    pub name: Identifier,
    /// Relevance belongs to this field occurrence, not to its referenced type.
    pub relevance: language_core::BindingRelevance,
    pub type_reference: crate::types::TypeReferenceHandle,
}

#[derive(Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent)]
pub struct DataVariant {
    pub identity: Option<u64>,
    pub name: Identifier,
    /// Named payload fields (`case Say(text: String);`). Payload-less cases have an
    /// empty span. Stored in their own arena so the parent's member span stays
    /// contiguous while a case's payload is parsed.
    pub payload: HandleSpan<DataField>,
    /// Case-local `where` facts (`case Range(lo: u64, hi: u64) where lo <= hi;`),
    /// proved at construction of that case. Empty when the case declares none.
    pub where_facts: HandleSpan<ProofFact>,
    pub retired_payload_identities: Vec<u64>,
}

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub struct DomainDefinition {
    pub name: Identifier,
    /// Lexical name of the classified value, not a runtime or generic parameter.
    pub subject: Option<Identifier>,
    /// Generic carrier binder followed by proof-static const binders. Empty
    /// for an ordinary monomorphic domain.
    pub type_parameters: HandleSpan<TypeParameter>,
    pub target_type: crate::types::TypeReferenceHandle,
    /// Const binders selected into family identity (`Quantity<U>`).
    pub index_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    /// Retained for transparent-alias publication legality.
    pub is_public: bool,
    /// An authored transparent predicate alias. Kept independently from
    /// predicate facts so an alias can never be mistaken for a bodyless
    /// establishment route.
    pub alias: Option<DomainAliasDefinition>,
    /// Exact trait-requirement paths authored by `established by`. These
    /// are establishment alternatives, independent from predicate facts.
    /// Owner-authored exact independent cleanup application.
    pub cleanup_selection: Option<crate::expression::StaticMachineArgument>,
    pub authored_routes: Vec<Vec<Identifier>>,
    /// Closed compiler-owned semantics selected by `satisfies` immediately
    /// after the domain head. This is independent from ordinary conformances.
    pub classification: Option<language_core::DomainClassification>,
    pub predicate_body: language_core::DomainPredicateBody,
    pub facts: HandleSpan<ProofFact>,
    pub operators: HandleSpan<OperatorDefinition>,
    pub semantic_clause_token_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, syntax_trees_derive::PlainSyntaxEquivalent)]
pub struct DomainAliasDefinition {
    /// Nonempty by grammar. Each constituent is an authored domain-name path.
    pub constituents: Vec<HandleSpan<Identifier>>,
}

impl Default for DomainDefinition {
    fn default() -> Self {
        Self {
            name: Identifier::generated(""),
            subject: None,
            type_parameters: HandleSpan::empty(),
            target_type: crate::types::TypeReferenceHandle::invalid(),
            index_arguments: HandleSpan::empty(),
            is_public: false,
            alias: None,
            cleanup_selection: None,
            authored_routes: Vec::new(),
            classification: None,
            predicate_body: language_core::DomainPredicateBody::Bodyless,
            facts: HandleSpan::empty(),
            operators: HandleSpan::empty(),
            semantic_clause_token_count: 0,
        }
    }
}

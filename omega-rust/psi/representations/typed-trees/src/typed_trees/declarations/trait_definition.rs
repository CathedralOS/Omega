use crate::name::Identifier;
use crate::signature::StateSignature;
use arena::HandleSpan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDefinition {
    pub symbol: SymbolHandle,
    pub is_boundary: bool,
    /// Source visibility retained independently from nominal and callable
    /// identity.
    pub is_public: bool,
    pub name: Identifier,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub conformance_bounds: Vec<crate::machine::GenericConformanceBound>,
    pub requires: HandleSpan<TraitRequirement>,
    pub machines: HandleSpan<StateSignature>,
    /// The `= Base` head of a transparent refinement. A refinement is a
    /// structural bound over an existing base conformance, never a nominal
    /// conformance target; `None` marks an ordinary trait.
    pub refines: Option<TraitRequirement>,
    /// `machine *` / `machine Base::requirement` narrowing clauses carrying
    /// operational axes only (`reaches`, `suspends`, `blocks`, `terminates`).
    pub refinement_clauses: Vec<TraitRefinementClause>,
}

impl Default for TraitDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            is_boundary: false,
            is_public: false,
            name: Identifier::default(),
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            requires: HandleSpan::empty(),
            machines: HandleSpan::empty(),
            refines: None,
            refinement_clauses: Vec::new(),
        }
    }
}

/// One `machine *` or `machine Base::requirement` narrowing clause of a
/// transparent refinement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitRefinementClause {
    /// The authored base requirement name; `None` for the `machine *`
    /// wildcard covering every base requirement.
    pub requirement: Option<Identifier>,
    /// Operational axes only.
    pub signature: StateSignature,
    /// Authored `reaches` names retained until the bound fit check consumes
    /// them; clause reach rows are not interned with signature reach rows.
    pub service_reaches: Vec<Identifier>,
    /// How the clause's `reaches` axis binds, resolved at lowering.
    pub service_reach: TraitRefinementReach,
}

/// How one refinement clause's `reaches` axis binds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TraitRefinementReach {
    /// The clause omits `reaches`; covered requirements inherit the base row.
    #[default]
    Inherited,
    /// Authored `reaches a + b;` (or authored-empty `reaches;`): a concrete
    /// row interned at the clause location.
    Concrete(language_semantics::ServiceReachRowId),
    /// Authored `reaches _;`: one independent abstract row per covered base
    /// requirement, each bounded by that requirement's inherited row and
    /// correlated with no other requirement's row.
    IndependentBounded(Vec<ClauseAbstractReachRow>),
}

/// One covered base requirement's independent abstract reach row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseAbstractReachRow {
    /// The covered base requirement's name.
    pub requirement: Identifier,
    /// The abstract row minted at this clause location, bounded by the
    /// requirement's inherited row.
    pub row: language_semantics::ServiceReachRowId,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConformanceSubject {
    #[default]
    Subjectless,
    Carrier(Identifier),
}

/// One whole conformance. Every closed implementation retains its exact
/// inherited requirement rows. Carrier-owned closed forms alone are eligible
/// for local dynamic dispatch; subjectless forms remain proof evidence. The
/// bodyless form remains a carrier-owned static declaration whose satisfiers
/// are validated separately.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conformance {
    pub symbol: SymbolHandle,
    /// Source visibility retained independently from semantic conformance
    /// identity and private realization rows.
    pub is_public: bool,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub subject: ConformanceSubject,
    /// Exact carrier declaration for a carrier-owned conformance. Subjectless
    /// proof evidence retains the invalid symbol.
    pub carrier_symbol: SymbolHandle,
    pub trait_name: Identifier,
    /// Exact trait declaration selected by `trait_name`.
    pub trait_symbol: SymbolHandle,
    /// Alpha-normalized declaration-order ordinals selecting the conformance
    /// lifetime supplied to each target-trait lifetime parameter.
    pub trait_lifetime_arguments: Vec<u32>,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub alias: Option<Identifier>,
    pub implementation: ConformanceImplementation,
}

impl Conformance {
    pub fn carrier_name(&self) -> Option<&Identifier> {
        match &self.subject {
            ConformanceSubject::Carrier(name) => Some(name),
            ConformanceSubject::Subjectless => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConformanceImplementation {
    #[default]
    AttachedRequirementMachines,
    Closed {
        rows: Vec<ConformanceRow>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceRow {
    pub declaring_trait: SymbolHandle,
    pub declaring_trait_name: Identifier,
    pub requirement: SymbolHandle,
    pub requirement_name: Identifier,
    /// Exact authored or per-conformance default realization. Closed frontend
    /// lowering instantiates trait defaults before this representation; an
    /// invalid survivor is an incomplete internal row and fails closed.
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_name: Identifier,
    pub source: ConformanceRowSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformanceRowSource {
    Inline,
    Reference,
    TraitDefault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitRequirement {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    /// Erased borrow-region arguments retained independently from runtime type
    /// arguments.
    pub lifetime_arguments: Vec<Identifier>,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    /// Authored relationship location retained for declaration-site semantic
    /// diagnostics after source-backed names are lowered to owned text.
    pub source_span: source::SourceSpan,
}

/// The semantic role of a trait-composition edge. It is derived from the
/// referenced trait, never authored: boundary parents extend service reach;
/// ordinary parents contribute policy/requirement identity only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraitCompositionKind {
    Policy,
    ServiceReach,
}

/// Why a requirement is absent from the signature-derived portion of a local
/// dynamic trait surface. Later contract/lifetime/envelope judgments remain
/// independent and may exclude an otherwise signature-eligible requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicSignatureIneligibility {
    BoundaryRequirement,
    RequirementLocalGenerics,
    MissingBorrowedReceiver,
    ByValueReceiver,
    MultipleReceivers,
    SelfOutsideReceiver,
    SelfResult,
}

impl Default for TraitRequirement {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            lifetime_arguments: Vec::new(),
            arguments: HandleSpan::empty(),
            source_span: source::SourceSpan::default(),
        }
    }
}

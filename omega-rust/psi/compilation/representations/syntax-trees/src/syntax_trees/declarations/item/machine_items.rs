use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: Identifier,
    /// Exact authored template token for a method cloned onto closed generic
    /// data. Empty on authored declarations and other synthesis families.
    pub generic_data_template: Identifier,
    /// Logical path of the trait whose authored default body this
    /// compiler-instantiated machine carries. That body stays lexically inside
    /// its trait, so its bare requirement names select the trait's
    /// requirements rather than the instantiating carrier's machines. Empty on
    /// authored declarations and other synthesis families.
    pub default_body_trait: Identifier,
    pub attached_data: Option<Identifier>,
    /// The optional fixed operator token written immediately after `machine`
    /// (`machine + Name(...)`, `boundary machine + Name(...);`). `None` is an
    /// ordinary named machine. The token vocabulary is closed and validated
    /// at parse; semantic-home ownership and duplicate token/shape checks are
    /// downstream (OPERATOR-MACHINE-SUPPLY).
    pub spelling: Option<OperatorSpelling>,
    /// Source-level package visibility. This is independent from `boundary`:
    /// an ordinary public library callable remains checked Omega code while
    /// publishing strict effect and operational ceilings.
    pub is_public: bool,
    /// A contract with no checked body. This is retained independently from
    /// both the exported-callable/admission boundary marker and the explicit
    /// top-level requirement marker below; the item parser enforces which
    /// bodyless source forms are legal.
    pub bodyless: bool,
    /// Compiler-generated callable head for a product target family. It owns
    /// no implementation or admission claim; each authored target keeps its
    /// separate body, and resolved member contracts determine this interface.
    pub target_family_interface: bool,
    /// Exact implementation target. Product family preparation retains this
    /// on every member; the independent interface occupies the portable path.
    /// No requested realization set changes which bodies Psi checks.
    pub target: Option<Identifier>,
    /// Authored target head, independent from Build's execution selection.
    pub authored_target: Option<Identifier>,
    /// The EXPORTED-CALLABLE marking (`boundary machine ...`): this machine
    /// is a callable surface the platform (or a foreign caller) invokes; its
    /// parameters are the boundary-trusted shape over the arrival bytes.
    pub boundary: bool,
    /// The explicit carrier-owned required-body declaration
    /// (`boundary requirement Package::operation(...);`). This is distinct
    /// from `boundary machine`: it declares a selectable requirement slot,
    /// not an exported callable or admission-bearing bodyless claim.
    pub is_top_level_boundary_requirement: bool,
    /// Erased borrow-region parameters declared in the shared `<>` list.
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub satisfies: HandleSpan<SatisfiesClause>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    /// Structural static equations retained until every application is discharged.
    pub where_facts: HandleSpan<ProofFact>,
    /// TPR2 (decision 23): the machine authored BARE `terminates;` — the
    /// public eventual-terminal guarantee. `terminates by ...` supplies only
    /// the private ranking witness and does not set this.
    pub terminates_guarantee: bool,
    pub ranking_subjects: HandleSpan<crate::expression::ExpressionHandle>,
    pub ranking_view: HandleSpan<Identifier>,
    /// TPR3: an ARGUMENTED view's arguments (`-> Nat::IncreasingTo(limit)`),
    /// in order; empty for plain views. The bound is part of the view.
    pub ranking_view_arguments: HandleSpan<crate::expression::ExpressionHandle>,
    /// TPR1: the witness clause's optional `in <range>` (decision 23's
    /// rank-range constraint). Invalid = absent. Parsed and stored here;
    /// the syntax->resolved lowering refuses it loudly until TPR3's cycle
    /// checker consumes ranges (never silently dropped).
    pub ranking_range: crate::expression::ExpressionHandle,
    /// Exact `reaches` keyword occurrences. Nonempty means the author wrote a
    /// ceiling even when `service_reaches` is empty.
    pub service_reach_keyword_source_spans: Vec<source::SourceSpan>,
    pub service_reaches: HandleSpan<Identifier>,
    /// `reaches <= Bound` on one top-level bodyless boundary requirement.
    /// The written row is a conservative upper bound; installation supplies
    /// the exact row selected for this requirement path.
    pub service_reach_is_installation_bound: bool,
    /// Direct synchronous boundary bindings this callable may enter before
    /// returning. Bodyful machines infer this set and use an authored list as
    /// their published ceiling; bodyless surfaces must declare it exactly.
    pub invokes: HandleSpan<Identifier>,
    /// Independent authored operational may-clauses (decision 22). These are
    /// never members of `service_reaches`.
    pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
    pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
    pub suspends: bool,
    pub blocks: bool,
    /// Authored same-stack demand of a bodyless `via` realization, in bytes.
    /// Zero means the author wrote no `stack N bytes` clause. The reserve for
    /// an unmeasured foreign leaf stays downstream; this field does not invent one.
    pub admitted_same_stack_bytes: u64,
    pub contracts: HandleSpan<CapabilityContract>,
    pub states: HandleSpan<StateHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
    /// Explicit arrival assumptions. States admit `requires` only: every
    /// incoming edge must prove them and the state body may assume them.
    pub contracts: HandleSpan<CapabilityContract>,
    pub statements: HandleSpan<crate::statement::StatementHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDefinition {
    pub is_boundary: bool,
    /// Source visibility for the trait declaration. This does not participate
    /// in trait or requirement identity.
    pub is_public: bool,
    pub name: Identifier,
    /// Erased borrow-region parameters declared in the shared `<>` list.
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    /// Header composition (`trait X: A + Policy<C>`). These normalize to the
    /// same requirement graph as body-level `requires A;`, while preserving
    /// generic arguments for policy identity and later substitution.
    pub parents: HandleSpan<crate::types::TypeReferenceHandle>,
    pub requires: HandleSpan<Identifier>,
    pub machines: HandleSpan<StateSignatureHandle>,
    /// The `= Base` head of a transparent trait refinement
    /// (`pub trait LocalLogger = Logger`). A refinement is a structural bound
    /// over an existing base conformance, never a new nominal conformance
    /// target; `None` marks an ordinary trait.
    pub refines: Option<crate::types::TypeReferenceHandle>,
    /// `machine Base::requirement` narrowing clauses authored inside a
    /// refinement body. Each signature retains only operational axes
    /// (`reaches`, `suspends`, `blocks`, `terminates`); the parser rejects
    /// parameters, results, bodies, contract clauses, and `requires` members.
    /// `requirement` carries the authored member name for resolution against
    /// the base trait.
    pub refinement_clauses: Vec<TraitRefinementClause>,
}

/// One `machine *` or `machine Base::requirement` narrowing clause inside a
/// transparent trait refinement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitRefinementClause {
    /// The authored base requirement name (`None` for the `machine *`
    /// wildcard). The qualifier, when present, must name the base trait.
    pub requirement: Option<Identifier>,
    /// Operational axes only; the parser rejects every other signature part.
    pub signature: StateSignature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSignature {
    pub name: Identifier,
    /// Fixed token owned by this trait requirement, when any. Implementing
    /// conformance members never repeat or replace this binding.
    pub spelling: Option<crate::operator_spelling::OperatorSpelling>,
    /// Erased borrow-region parameters owned by this callable requirement.
    pub lifetime_parameters: Vec<Identifier>,
    /// Generic parameters owned by this callable requirement. N7 uses this
    /// on a `where machine` signature so a required machine may itself accept
    /// compile-time machine symbols.
    pub type_parameters: HandleSpan<TypeParameter>,
    pub is_default: bool,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub native_callback_parameters: Vec<NativeCallbackParameterNode>,
    pub return_type: crate::types::TypeReferenceHandle,
    /// `reaches <= Bound`: the listed row is an upper bound whose exact row is
    /// supplied by installation. Only bodyless boundary-trait requirements may
    /// carry this marker.
    pub service_reach_is_installation_bound: bool,
    /// Exact `reaches` keyword occurrences. This preserves an authored empty
    /// ceiling independently from an omitted inferred row.
    pub service_reach_keyword_source_spans: Vec<source::SourceSpan>,
    pub service_reaches: HandleSpan<Identifier>,
    /// Bodyless direct synchronous invocation ceiling. Members name callable
    /// parameters (or a boundary-trait identity when no parameter path exists).
    pub invokes: HandleSpan<Identifier>,
    /// Exact authored operational-clause keyword occurrences.
    pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
    pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: HandleSpan<CapabilityContract>,
    /// Statements authored in a trait machine body. Empty for ordinary
    /// requirements and all non-trait signatures.
    pub default_body: HandleSpan<crate::statement::StatementHandle>,
    /// TPR4 (decision 23): the bodyless requirement authored bare
    /// `terminates;` -- the PUBLIC eventual-terminal guarantee its
    /// implementations inherit. A requirement never carries a witness
    /// (`terminates by ...` is rejected at parse: the witness belongs to
    /// implementations).
    pub terminates_guarantee: bool,
    /// Signature-level `where` proof facts. A finite generic method family is
    /// authored as explicit equality disjunctions over the signature's own
    /// value binders (for example `where Width == 16 || Width == 32`); other
    /// fact shapes remain ordinary proof obligations recorded here so the
    /// later stages can reject them for dispatch purposes.
    pub where_facts: HandleSpan<ProofFact>,
}

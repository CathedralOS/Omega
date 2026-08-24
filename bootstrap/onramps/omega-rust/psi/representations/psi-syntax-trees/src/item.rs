use crate::identifier::Identifier;
use psi_arena::{Arena, Handle, HandleSpan};
use psi_language_core::operator_spelling::OperatorSpelling;

pub type ItemHandle = Handle<Item>;
pub type StateParameterHandle = Handle<StateParameterNode>;
pub type StateSignatureHandle = Handle<StateSignatureNode>;
pub type StateHandle = Handle<StateNode>;
pub type MachineHandle = Handle<MachineNode>;
pub type TraitHandle = Handle<TraitNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Capability(CapabilityDefinition),
    Conformance(ConformanceItem),
    Const(ConstDefinition),
    Data(DataDefinition),
    Domain(DomainDefinition),
    Invariant(InvariantDefinition),
    Library(LibraryDefinition),
    Measure(MeasureDefinition),
    Module(ModuleDeclaration),
    Operator(OperatorDefinition),
    Package(PackageDeclaration),
    Proposition(PropositionDefinition),
    Export(ExportItem),
    Use(UseItem),
    Machine(Machine),
    Trait(TraitDefinition),
    Target(TargetDefinition),
    WireData(WireDataDefinition),
}

/// A named compile-time PURE VALUE (design brief static_root_and_constants.md,
/// SETTLED 2026-07-04; built as const-v0, TASKS_TIME.md D15). Type-scoped:
/// `const EfiStatus::SUCCESS: EfiStatus = EfiStatus { code: 0 };` — declared
/// like a machine (`Type::NAME`), never a `data` member, so never in `sizeof`.
/// v0 initializers are LITERAL-ONLY (scalars, negated scalars, struct/array
/// literals of literals — build-time evaluation of richer expressions is its
/// own arc). Consts exist only until symbol resolution: every use substitutes
/// a fresh copy of the initializer, so typed trees and everything downstream
/// never see a const — the copied-at-each-use semantics the brief specifies.
/// (Free-floating `const NAME: T = ...;` parses but is rejected until the
/// local-shadowing walk lands: a bare-name substitution could silently win
/// over a like-named local; a `Type::NAME` path cannot.)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConstDefinition {
    /// The type scope (`EfiStatus` in `EfiStatus::SUCCESS`); EMPTY text for
    /// the not-yet-accepted free-floating form.
    pub scope: Identifier,
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
    pub value: crate::expression::ExpressionHandle,
}

/// The compiler-known, CLOSED external `Binding` sum: each leaf binds a
/// boundary-trait method to ONE mechanism the compiler knows how to lower. A
/// new mechanism = a new case + new lowering, never user-invented --
/// same discipline as `FieldPlan`. Each kind also implies the edge's calling
/// convention (`Syscall` -> the syscall plan; `DllImport`/`VtableSlot` -> the C
/// plan), so nobody names a convention in the common case (`calling_plans.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalBinding {
    /// Linux's stable ABI is the number table: `Binding::Syscall(1)`.
    Syscall { number: i64 },
    /// Bootstrap string carrier for Windows DLL exports. The settled model uses
    /// one nominal `DllImportId`; OWNER Q1 and `NOMINAL-FOREIGN-BINDINGS` track
    /// the missing declaration/metadata surface and this migration.
    DllImport { module: String, symbol: String },
    /// Compiler-known target operation. The resolved realization symbol,
    /// normalized signature, and target key the sealed lowering catalog.
    CompilerIntrinsic,
    /// COM/UEFI per-object dispatch: `Binding::VtableSlot(1)` (deref `this`, read the
    /// vtable pointer, read slot N, call at the declared convention).
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
    pub number: u64,
    pub name: Identifier,
    pub relevance: psi_language_core::BindingRelevance,
    pub type_reference: crate::types::TypeReferenceHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireDataReserved {
    pub number: u64,
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

/// A proof-formula declaration. It has no result, executable body, effects, or
/// runtime representation; its parameters are an erased proof-side telescope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropositionDefinition {
    pub name: Identifier,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub body: PropositionBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropositionBody {
    /// Owner-declared fact with no recoverable witness interface.
    Primitive,
    /// Exactly one owner-authorized carrierless evidence interface.
    Witness {
        evidence: crate::types::TypeReferenceHandle,
    },
    /// Source/debug alias expanded before normalized semantic identity.
    Transparent {
        proposition: crate::expression::ExpressionHandle,
    },
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
                spelling: None,
                lifetime_parameters: Vec::new(),
                type_parameters: HandleSpan::empty(),
                is_default: false,
                parameters: HandleSpan::empty(),
                return_type: crate::types::TypeReferenceHandle::invalid(),
                service_reach_is_installation_bound: false,
                service_reaches: HandleSpan::empty(),
                invokes: HandleSpan::empty(),
                suspends: false,
                blocks: false,
                contracts: HandleSpan::empty(),
                default_body: HandleSpan::empty(),
                terminates_guarantee: false,
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
    /// Erased borrow-region parameters declared in the shared `<>` list.
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub contracts: HandleSpan<CapabilityContract>,
    /// Optional compiler-owned fixed token from the declaration head.
    pub spelling: Option<OperatorSpelling>,
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
    /// An explicit erased evidence-term binding (`requires proof: P` or
    /// `ensures proof: P`). Empty clauses remain ordinary ambient facts.
    pub binding: Option<Identifier>,
    pub facts: HandleSpan<ProofFact>,
    pub token_count: usize,
}

impl Default for CapabilityContract {
    fn default() -> Self {
        Self {
            kind: CapabilityContractKind::default(),
            binding: None,
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
    Crashes { cause: CrashCause },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashCause {
    Trap,
    Abort,
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
    pub supply_mode: psi_language_core::DataSupplyMode,
    /// Erased borrow-region parameters declared in the shared `<>` list.
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
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
    pub where_facts: HandleSpan<ProofFact>,
    pub members: HandleSpan<DataMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotientDefinition {
    pub carrier: crate::types::TypeReferenceHandle,
    pub relation: HandleSpan<Identifier>,
    /// Exact declaration-site selection from
    /// `where R satisfies Equivalence<C, R> as Name;`.
    pub equivalence: Option<QuotientEquivalenceSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotientEquivalenceSelection {
    pub relation: HandleSpan<Identifier>,
    pub trait_name: Identifier,
    pub trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub conformance_name: Identifier,
}

/// The subject of a whole-trait conformance. Carrier-owned conformances inherit
/// the carrier's static telescope. Subjectless conformances own proof evidence
/// only and therefore never infer an arbitrary parameter as a carrier.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConformanceSubject {
    #[default]
    Subjectless,
    Carrier(Identifier),
}

/// A whole-trait conformance. A name-first bodyless declaration checks
/// separately attached exact-requirement machines; a block owns a closed
/// member map: `Primary: Point satisfies Shape { ... }`. The concrete
/// subjectless form is `EvidenceName: satisfies Evidence { ... }`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConformanceItem {
    /// Generic binders belong to the declared conformance name, not to its
    /// carrier or trait application.
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub subject: ConformanceSubject,
    pub trait_name: Identifier,
    pub trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub alias: Option<Identifier>,
    pub body: ConformanceBody,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConformanceBody {
    /// Bodyless static declaration whose rows are discovered from separately
    /// attached exact-requirement machines. It remains distinct so a parsed
    /// empty closed block can never silently fall back to ambient lookup.
    #[default]
    AttachedRequirementMachines,
    /// The authored, closed implementation surface.
    Closed {
        members: HandleSpan<ConformanceMember>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataProperties {
    /// Usage multiplicity. Ordinary data defaults to affine; `[copy]` selects
    /// unrestricted and `[linear]` selects exact consumption. Keeping the enum
    /// here prevents syntax lowering from reconstructing semantic identity
    /// from compatibility booleans.
    pub multiplicity: psi_language_core::Multiplicity,
    /// Authored carry-policy floor. Omission remains distinct from an authored
    /// strict policy so transparent derivation and opaque admission can choose
    /// their respective establishment paths later.
    pub carry: Option<psi_language_core::CarryPolicy>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeParameter {
    pub name: Identifier,
    pub kind: TypeParameterKind,
    /// Property bounds in brackets after the parameter name (frozen decision
    /// 13): `data Box<T [copy]>`. Brackets attach to what they follow, so the
    /// bound list reuses the declared-property shape of the data declaration.
    pub bounds: DataProperties,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TypeParameterKind {
    #[default]
    Type,
    Const {
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

/// Authored declaration-site contract for a static machine parameter.
/// Structural contracts own their complete signature locally. Nominal
/// contracts retain only the exact signature-free requirement path; symbol
/// resolution binds that path once declarations and trait requirements exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineParameterContract {
    Structural(StateSignature),
    Nominal { requirement: HandleSpan<Identifier> },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropositionParameterSignature {
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameterHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataField {
    pub identity: Option<u64>,
    pub name: Identifier,
    /// Relevance belongs to this field occurrence, not to its referenced type.
    pub relevance: psi_language_core::BindingRelevance,
    pub type_reference: crate::types::TypeReferenceHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataVariant {
    pub identity: Option<u64>,
    pub name: Identifier,
    /// Named payload fields (`case Say(text: String);`). Payload-less cases have an
    /// empty span. Stored in their own arena so the parent's member span stays
    /// contiguous while a case's payload is parsed.
    pub payload: HandleSpan<DataField>,
    pub retired_payload_identities: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainDefinition {
    pub name: Identifier,
    /// Generic carrier binder followed by proof-static const binders. Empty
    /// for an ordinary monomorphic domain.
    pub type_parameters: HandleSpan<TypeParameter>,
    pub target_type: crate::types::TypeReferenceHandle,
    /// Const binders selected into family identity (`Quantity<U>`).
    pub index_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    /// Retained for transparent-alias publication legality. General module
    /// export semantics remain owned by explicit `export` items.
    pub is_public: bool,
    /// An authored transparent predicate alias. Kept independently from
    /// predicate facts so an alias can never be mistaken for a bodyless
    /// establishment route.
    pub alias: Option<DomainAliasDefinition>,
    /// Exact trait-requirement paths authored by `established by`. These
    /// are establishment alternatives, independent from predicate facts.
    pub authored_routes: Vec<Vec<Identifier>>,
    /// Closed compiler-owned semantics selected by `satisfies` immediately
    /// after the domain head. This is independent from ordinary conformances.
    pub classification: Option<psi_language_core::DomainClassification>,
    pub predicate_body: psi_language_core::DomainPredicateBody,
    pub facts: HandleSpan<ProofFact>,
    pub operators: HandleSpan<OperatorDefinition>,
    pub semantic_clause_token_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainAliasDefinition {
    /// Nonempty by grammar. Each constituent is an authored domain-name path.
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
            authored_routes: Vec::new(),
            classification: None,
            predicate_body: psi_language_core::DomainPredicateBody::Bodyless,
            facts: HandleSpan::empty(),
            operators: HandleSpan::empty(),
            semantic_clause_token_count: 0,
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

/// One `satisfies` binding on a machine (rearrange settle 2026-07-18):
/// `satisfies Trait::requirement` or `satisfies Trait::requirement as Alias`.
/// A REQUIREMENT-named binding
/// conforms this machine to that single requirement (the machine-by-machine
/// carrier model; the alias names the satisfier for plural algebras -- Nat
/// under (max, add) is the tropical semiring).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SatisfiesClause {
    pub trait_name: Identifier,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub requirement: Option<Identifier>,
    pub alias: Option<Identifier>,
    /// PRV4 step 1: `satisfies Requirement via <Binding>` -- the irreducible
    /// EXTERNAL LEAF. The binding expression is the closed compile-time sum;
    /// bootstrap lowering currently renders it as text, while the destination
    /// retains a structured nominal binding beside the exact realization
    /// symbol. Only legal on a BODYLESS machine (a composite lowering is an
    /// ordinary checked body).
    pub via: Option<ExternalBinding>,
}

/// A generic `where T satisfies Trait<Args>` test of an already-declared
/// nominal conformance. A qualified carrier path (`T satisfies Card::Order`)
/// selects the named conformance `Order` declared for `Card`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GenericConformanceBound {
    /// `Some(Order)` for an explicit telescope binder
    /// `Order: Subject satisfies Trait`; `None` for the older anonymous
    /// `where Subject satisfies Trait` test.
    pub binder: Option<Identifier>,
    pub subject: Identifier,
    pub carrier: Identifier,
    pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub conformance: Option<Identifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    /// Source-level package visibility. This is independent from `boundary`:
    /// an ordinary public library callable remains checked Omega code while
    /// publishing strict effect and operational ceilings.
    pub is_public: bool,
    /// CH10 ACCEPTED FORM (GR6d): a contract with NO body (`boundary
    /// machine f(..) ensures ..;`) -- the accepted-axiom tier. Only legal
    /// with `boundary`; the item parser enforces the pairing.
    pub bodyless: bool,
    /// TARGET-SCOPED implementation machine (`<target> machine ...`, the fs
    /// portable-contract settle 2026-07-18): the machine participates in the
    /// program only when this target is SELECTED. The pre-resolution filter
    /// clears the marker on the selected target's machine and validates the
    /// loud edges (duplicate / zero implementations for the selected target);
    /// a machine still carrying `Some` at resolution is inert.
    pub target: Option<Identifier>,
    /// The EXPORTED-CALLABLE marking (`boundary machine ...`): this machine
    /// is a callable surface the platform (or a foreign caller) invokes; its
    /// parameters are the boundary-trusted shape over the arrival bytes.
    pub boundary: bool,
    /// Erased borrow-region parameters declared in the shared `<>` list.
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub satisfies: HandleSpan<SatisfiesClause>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
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
    pub suspends: bool,
    pub blocks: bool,
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
    pub name: Identifier,
    /// Erased borrow-region parameters declared in the shared `<>` list.
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub conformance_bounds: Vec<GenericConformanceBound>,
    /// Header composition (`trait X: A + Policy<C>`). These normalize to the
    /// same requirement graph as body-level `requires A;`, while preserving
    /// generic arguments for policy identity and later substitution.
    pub parents: HandleSpan<crate::types::TypeReferenceHandle>,
    pub invariants: HandleSpan<ProofFact>,
    pub requires: HandleSpan<Identifier>,
    pub machines: HandleSpan<StateSignatureHandle>,
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
    pub return_type: crate::types::TypeReferenceHandle,
    /// `reaches <= Bound`: the listed row is an upper bound whose exact row is
    /// supplied by installation. Only bodyless boundary-trait requirements may
    /// carry this marker.
    pub service_reach_is_installation_bound: bool,
    pub service_reaches: HandleSpan<Identifier>,
    /// Bodyless direct synchronous invocation ceiling. Members name callable
    /// parameters (or a boundary-trait identity when no parameter path exists).
    pub invokes: HandleSpan<Identifier>,
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
    traits: Arena<TraitNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeclarationStorage {
    identifier_path_members: Arena<Identifier>,
    conformance_members: Arena<ConformanceMember>,
    satisfies_clauses: Arena<SatisfiesClause>,
    type_parameters: Arena<TypeParameter>,
    boundary_levels: Arena<BoundaryLevel>,
    library_functions: Arena<LibraryFunction>,
    capability_members: Arena<CapabilityMember>,
    capability_contracts: Arena<CapabilityContract>,
    data_members: Arena<DataMember>,
    data_payload_fields: Arena<DataField>,
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

    pub fn append_conformance_member(
        &mut self,
        member: ConformanceMember,
    ) -> Handle<ConformanceMember> {
        self.declaration_storage.conformance_members.append(member)
    }

    pub fn append_satisfies_clause(&mut self, clause: SatisfiesClause) -> Handle<SatisfiesClause> {
        self.declaration_storage.satisfies_clauses.append(clause)
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
            spelling: signature.spelling,
            lifetime_parameters: signature.lifetime_parameters.clone(),
            type_parameters: signature.type_parameters,
            is_default: signature.is_default,
            parameters: signature.parameters,
            return_type: signature.return_type,
            service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
            service_reaches: signature.service_reaches,
            invokes: signature.invokes,
            suspends: signature.suspends,
            blocks: signature.blocks,
            contracts: signature.contracts,
            default_body: signature.default_body,
            terminates_guarantee: signature.terminates_guarantee,
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
            service_reaches: machine.service_reaches,
            invokes: machine.invokes,
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
            traits: Arena::new(),
        }
    }
}

impl DeclarationStorage {
    fn new() -> Self {
        Self {
            identifier_path_members: Arena::new(),
            conformance_members: Arena::new(),
            satisfies_clauses: Arena::new(),
            type_parameters: Arena::new(),
            boundary_levels: Arena::new(),
            library_functions: Arena::new(),
            capability_members: Arena::new(),
            capability_contracts: Arena::new(),
            data_members: Arena::new(),
            data_payload_fields: Arena::new(),
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
    pub spelling: Option<crate::operator_spelling::OperatorSpelling>,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub is_default: bool,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub service_reach_is_installation_bound: bool,
    pub service_reaches: HandleSpan<Identifier>,
    pub invokes: HandleSpan<Identifier>,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: HandleSpan<CapabilityContract>,
    pub default_body: HandleSpan<crate::statement::StatementHandle>,
    /// TPR4 (decision 23): the bodyless requirement's authored guarantee.
    pub terminates_guarantee: bool,
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
    pub service_reaches: HandleSpan<Identifier>,
    pub invokes: HandleSpan<Identifier>,
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

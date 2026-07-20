use crate::identifier::Identifier;
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::operator_spelling::{OperatorSpelling, ProviderCategory};

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
    Provider(ProviderDeclaration),
    HostProvider(HostProviderDefinition),
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
    /// The vtable STRUCT whose fn-ptr fields the arms bind (the field model,
    /// extern brief SS12.1): `uefi_x64 provides TextOutput over
    /// EfiTextOutputProtocol { ... }`. EMPTY when the block has no `over`
    /// clause (the ZII default; required for `VtableField` arms, unused by
    /// the static mechanisms).
    pub vtable_struct: Identifier,
    pub mappings: HandleSpan<HostProviderMapping>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct HostProviderMapping {
    /// The boundary-trait method this arm binds (the `output_string` in
    /// `output_string -> VtableSlot(1)`).
    pub machine: Identifier,
    pub binding: HostProviderMappingKind,
}

impl HostProviderMappingKind {
    /// PRV4: the NORMALIZED rendering -- the compile-time-evaluable binding
    /// expression's canonical text, the ExternalRealization identity the
    /// interner keys on. Exactly one spelling per binding value.
    /// The exact inverse of `normalized_rendering` (round-trip pinned):
    /// the merge seam re-materializes a leaf's structured binding from the
    /// interned rendering. `None` = unrecognized (refuses at extraction).
    pub fn from_normalized_rendering(rendering: &str) -> Option<Self> {
        let (case, rest) = rendering.split_once('(')?;
        let payload = rest.strip_suffix(')')?;
        match case {
            "Syscall" => payload.parse().ok().map(|number| Self::Syscall { number }),
            "VtableSlot" => payload.parse().ok().map(|index| Self::VtableSlot { index }),
            "Value" => payload.parse().ok().map(|value| Self::Value { value }),
            "DllImport" => {
                let (module, symbol) = payload.split_once(',')?;
                Some(Self::DllImport {
                    module: module.into(),
                    symbol: symbol.into(),
                })
            }
            "VtableField" => Some(Self::VtableField {
                field: Identifier::generated(payload.to_owned()),
            }),
            "TableFunction" => Some(Self::TableFunction {
                field: Identifier::generated(payload.to_owned()),
            }),
            _ => None,
        }
    }

    pub fn normalized_rendering(&self) -> String {
        match self {
            Self::Syscall { number } => format!("Syscall({number})"),
            Self::DllImport { module, symbol } => {
                format!("DllImport({module},{symbol})")
            }
            Self::VtableSlot { index } => format!("VtableSlot({index})"),
            Self::VtableField { field } => format!("VtableField({})", field.as_str()),
            Self::TableFunction { field } => {
                format!("TableFunction({})", field.as_str())
            }
            Self::Value { value } => format!("Value({value})"),
        }
    }
}

/// The compiler-known, CLOSED `Binding` sum (extern brief §12.1): each provides
/// arm binds a boundary-trait method to ONE mechanism the compiler knows how to
/// lower. A new mechanism = a new case + new lowering, never user-invented --
/// same discipline as `FieldPlan`. Each kind also implies the edge's calling
/// convention (`Syscall` -> the syscall plan; `DllImport`/`VtableSlot` -> the C
/// plan), so nobody names a convention in the common case (`calling_plans.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostProviderMappingKind {
    /// Linux's stable ABI is the number table: `-> Syscall(1)`.
    Syscall { number: i64 },
    /// Windows' stable ABI is named DLL exports: `-> DllImport("kernel32", "ExitProcess")`.
    DllImport { module: String, symbol: String },
    /// COM/UEFI per-object dispatch: `-> VtableSlot(1)` (deref `this`, read the
    /// vtable pointer, read slot N, call at the declared convention).
    VtableSlot { index: i64 },
    /// COM/UEFI per-object dispatch by FIELD NAME (the field model, decided
    /// 2026-07-04; extern brief SS12.1): `output_string -> output_string`
    /// names a fn-ptr FIELD of the block's `over` struct; the layout policy
    /// computes the offset -- no magic slot counts, headers fall out free.
    VtableField { field: Identifier },
    /// A SERVICE-TABLE function: `get_memory_map -> TableFunction(get_memory_map)`
    /// dispatches through the table's fn-ptr field like `VtableField`, but the
    /// table pointer is DISPATCH-ONLY -- never a wire argument (EFI table
    /// services take no This; protocol/COM methods do).
    TableFunction { field: Identifier },
    /// A per-target named CONSTANT, not a call mechanism: `O_CREATE -> 32768`
    /// (portable-values settle, 2026-07-07 -- the libc-crate half of the Rust
    /// split). The row supplies the number a boundary trait's declared const
    /// resolves to on this target; it never lowers to a call.
    Value { value: i64 },
}

impl Default for HostProviderMappingKind {
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
    pub supply_mode: omega_core::semantics::DataSupplyMode,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub properties: DataProperties,
    /// R2 rung 1 (ch12 "Dependent Data"): the DEFAULT-DOMAIN facts --
    /// `data M where count * stride <= len, { ... }` -- bare field names,
    /// any number of facts, holding at every observation. Parsed and
    /// stored; the syntax->resolved lowering refuses them loudly until R2
    /// rung 2 consumes the model (never a silent drop).
    pub where_facts: HandleSpan<ProofFact>,
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
/// resolved properties rather than spellings. `sized` is computed from the
/// shape and may not be declared.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataProperties {
    /// Usage multiplicity. Ordinary data defaults to affine; `[copy]` selects
    /// unrestricted and `[linear]` selects exact consumption. Keeping the enum
    /// here prevents syntax lowering from reconstructing semantic identity
    /// from compatibility booleans.
    pub multiplicity: omega_core::semantics::Multiplicity,
    /// Zero means empty: the zeroed value is the type's empty value; owns the
    /// zero-case-payload-free rule and rejects non-zero field defaults.
    pub zero_init: bool,
    /// Authored carry-policy floor. Omission remains distinct from an authored
    /// strict policy so transparent derivation and opaque admission can choose
    /// their respective establishment paths later.
    pub carry: Option<omega_core::semantics::CarryPolicy>,
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
    Machine { contract: Option<StateSignature> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataMember {
    Field(DataField),
    Variant(DataVariant),
}

impl Default for DataMember {
    fn default() -> Self {
        Self::Variant(DataVariant::default())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataField {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
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

/// One `satisfies` binding on a machine (rearrange settle 2026-07-18):
/// `satisfies Trait`, `satisfies Trait::requirement`, or
/// `satisfies Trait::requirement as Alias`. A REQUIREMENT-named binding
/// conforms this machine to that single requirement (the machine-by-machine
/// carrier model; the alias names the satisfier for plural algebras -- Nat
/// under (max, add) is the tropical semiring); a bare trait name keeps the
/// whole-trait semantics for data-attached machines and binds a FREE machine
/// to the requirement matching its own name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SatisfiesClause {
    pub trait_name: Identifier,
    pub requirement: Option<Identifier>,
    pub alias: Option<Identifier>,
    /// PRV4 step 1: `satisfies Requirement via <Binding>` -- the irreducible
    /// EXTERNAL LEAF. The binding expression is the closed compile-time sum
    /// (the provides grammar's RHS); its normalized rendering becomes the
    /// machine's ExternalRealization supply identity. Only legal on a
    /// BODYLESS machine (a composite lowering is an ordinary checked body).
    pub via: Option<HostProviderMappingKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    /// CH10 ACCEPTED FORM (GR6d): a contract with NO body (`boundary
    /// machine f(..) ensures ..;`) -- the accepted-axiom tier. Only legal
    /// with `boundary`; the item parser enforces the pairing.
    pub bodyless: bool,
    /// TARGET-SCOPED implementation machine (`<target> machine ...`, the fs
    /// portable-contract settle 2026-07-18): the machine participates in the
    /// program only when this target is SELECTED. The pre-resolution filter
    /// clears the marker on the selected target's machine and validates the
    /// loud edges (duplicate / zero implementations for the selected target);
    /// a machine still carrying `Some` at resolution is inert, exactly like a
    /// non-selected provides row.
    pub target: Option<Identifier>,
    /// The EXPORTED-CALLABLE marking (`boundary machine ...`): this machine
    /// is a callable surface the platform (or a foreign caller) invokes; its
    /// parameters are the boundary-trusted shape over the arrival bytes.
    pub boundary: bool,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub satisfies: HandleSpan<SatisfiesClause>,
    pub terminates: bool,
    /// TPR2 (decision 23): the machine authored BARE `terminates;` — the
    /// public eventual-terminal guarantee. `terminates by ...` supplies only
    /// the private ranking witness and does NOT set this; `terminates`
    /// above stays the compatibility bool (true for either spelling) until
    /// the TPR6 firewall retires it.
    pub terminates_guarantee: bool,
    pub decreases: HandleSpan<crate::expression::ExpressionHandle>,
    pub decrease_order: HandleSpan<Identifier>,
    /// TPR3: an ARGUMENTED view's arguments (`-> Nat::IncreasingTo(limit)`),
    /// in order; empty for plain views. The bound is part of the view.
    pub decrease_view_arguments: HandleSpan<crate::expression::ExpressionHandle>,
    /// TPR1: the witness clause's optional `in <range>` (decision 23's
    /// rank-range constraint). Invalid = absent. Parsed and stored here;
    /// the syntax->resolved lowering refuses it loudly until TPR3's cycle
    /// checker consumes ranges (never silently dropped).
    pub decrease_range: crate::expression::ExpressionHandle,
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
pub struct TraitDefinition {
    pub is_boundary: bool,
    pub name: Identifier,
    pub type_parameters: HandleSpan<TypeParameter>,
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
    pub is_default: bool,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub effects: HandleSpan<Identifier>,
    pub contracts: HandleSpan<CapabilityContract>,
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
    satisfies_clauses: Arena<SatisfiesClause>,
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
            terminates_guarantee: signature.terminates_guarantee,
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
            satisfies_clauses: Arena::new(),
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
    /// TPR4 (decision 23): the bodyless requirement's authored guarantee.
    pub terminates_guarantee: bool,
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
    pub satisfies: HandleSpan<SatisfiesClause>,
    pub effects: HandleSpan<Identifier>,
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

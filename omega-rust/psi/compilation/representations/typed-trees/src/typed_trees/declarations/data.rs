use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use arena::HandleSpan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDefinition {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub is_public: bool,
    pub supply_mode: language_semantics::DataSupplyMode,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    /// Exact, resolved structural origin of a generated concrete generic
    /// instance. The generated nominal symbol remains the runtime carrier.
    pub generic_instance: Option<TypeReferenceHandle>,
    pub properties: DataProperties,
    /// Selected default-domain cleanup, independent from the data namespace.
    pub cleanup_selection: Option<crate::domain::CleanupSelection>,
    /// N6 proof-only quotient metadata, retained through typing so proof and
    /// validation consumers share the exact carrier/relation identity.
    pub quotient: Option<QuotientDefinition>,
    /// R2 rung 2 slice 2 (ch12): the ADMITTED zero-satisfying
    /// default-domain facts, copied from the resolved record. INERT until
    /// rung 3 wires entailment hypotheses + write obligations ATOMICALLY.
    pub where_facts: HandleSpan<crate::domain::ProofFact>,
    /// R2 rung 2b: zero violates the default domain (copied; see the
    /// resolved record).
    pub zero_gated: bool,
    pub retired_identities: Vec<u64>,
    pub members: HandleSpan<DataMember>,
}

impl Default for DataDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            is_public: false,
            supply_mode: language_semantics::DataSupplyMode::CheckedShape,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            generic_instance: None,
            properties: DataProperties::default(),
            cleanup_selection: None,
            quotient: None,
            where_facts: HandleSpan::empty(),
            zero_gated: false,
            retired_identities: Vec::new(),
            members: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotientDefinition {
    pub carrier: TypeReferenceHandle,
    pub relation: Vec<Identifier>,
    pub relation_symbol: SymbolHandle,
    pub equivalence: Option<QuotientEquivalenceSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotientEquivalenceSelection {
    pub relation: Vec<Identifier>,
    pub relation_symbol: SymbolHandle,
    pub trait_name: Identifier,
    pub trait_symbol: SymbolHandle,
    pub trait_arguments: HandleSpan<TypeReferenceHandle>,
    pub conformance_name: Identifier,
    pub conformance_symbol: SymbolHandle,
}

/// Declared type properties (`data Point [copy]`). The spelling set is closed
/// at parse time; validation verifies unrestricted multiplicity and carry
/// structurally.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataProperties {
    pub carry: Option<language_semantics::CarryPolicy>,
    /// The first-class usage model (`[copy]` -> Unrestricted, ordinary data ->
    /// Affine, `[linear]` -> Linear).
    pub multiplicity: language_semantics::Multiplicity,
}

impl DataDefinition {
    pub fn shape_kind_from_members(members: &[DataMember]) -> DataShapeKind {
        let mut has_fields = false;
        let mut has_variants = false;

        for member in members {
            match member {
                DataMember::Field(_) => has_fields = true,
                DataMember::Variant(_) => has_variants = true,
            }
        }

        match (has_fields, has_variants) {
            (false, false) => DataShapeKind::Empty,
            (true, false) => DataShapeKind::Record,
            (false, true) => DataShapeKind::Enum,
            (true, true) => DataShapeKind::Mixed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataShapeKind {
    Empty,
    Enum,
    Mixed,
    Record,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataMember {
    Field(DataField),
    Variant(DataVariant),
}

impl DataMember {
    /// The field record, if this member is one.
    pub fn as_field(&self) -> Option<&DataField> {
        let DataMember::Field(field) = self else {
            return None;
        };
        Some(field)
    }

    /// The variant record, if this member is one.
    pub fn as_variant(&self) -> Option<&DataVariant> {
        let DataMember::Variant(variant) = self else {
            return None;
        };
        Some(variant)
    }
}

impl Default for DataMember {
    fn default() -> Self {
        Self::Variant(DataVariant::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParameter {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub kind: TypeParameterKind,
    /// Property bounds (`data Box<T [copy]>`). A bounded parameter satisfies
    /// the structural copy/carry walk inside its owner, and every
    /// instantiation argument must carry the bound.
    pub bounds: DataProperties,
}

/// The roster position of the type parameter keyed `symbol`, if present.
pub fn type_parameter_position(
    parameters: &[TypeParameter],
    symbol: SymbolHandle,
) -> Option<usize> {
    parameters
        .iter()
        .position(|parameter| parameter.symbol == symbol)
}

/// Whether a provider-side type parameter requires properties not guaranteed
/// by the requirement-side parameter.
///
/// Affine with no carry policy is the unconstrained default. A provider may
/// weaken to that default or repeat the requirement exactly; it may not demand
/// a different multiplicity or carry policy from its callers.
pub fn type_parameter_demands_stronger_properties(
    requirement: &TypeParameter,
    provider: &TypeParameter,
) -> bool {
    (provider.bounds.multiplicity != language_semantics::Multiplicity::Affine
        && requirement.bounds.multiplicity != provider.bounds.multiplicity)
        || provider.bounds.carry.is_some() && provider.bounds.carry != requirement.bounds.carry
}

impl Default for TypeParameter {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            kind: TypeParameterKind::Type,
            bounds: DataProperties::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TypeParameterKind {
    #[default]
    Type,
    /// Positional trait implementation binder. It selects code, never a
    /// stored type; the owner's conformance bound carries its exact contract.
    Implementation,
    Const {
        type_reference: TypeReferenceHandle,
    },
    /// Runtime-capable value binder (`Count: u32`); unlike `Const` it may be
    /// supplied at runtime. Dynamic realization is not implemented, so only
    /// static arguments specialize through the const path.
    Value {
        type_reference: TypeReferenceHandle,
    },
    /// Static machine-symbol parameter and the declaration-site contract
    /// against which generic bodies and later instantiations are checked.
    Machine {
        contract: MachineParameterContract,
    },
    /// Generic proof-formula family with an authored value-parameter
    /// signature. It carries no executable state or runtime result.
    Proposition {
        contract: PropositionParameterSignature,
    },
}

impl TypeParameterKind {
    /// The parameter binds a compile-time value — a static const identity or
    /// a runtime-capable value that today only specializes statically.
    pub fn is_const_or_value(&self) -> bool {
        matches!(self, Self::Const { .. } | Self::Value { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineParameterContract {
    RequirementIdentity,
    Structural(crate::signature::StateSignature),
    Nominal {
        trait_definition: SymbolHandle,
        requirement: SymbolHandle,
        /// The trait application the binder is declared under
        /// (`Visitor<Context>::visit`); empty for a parameterless trait.
        trait_arguments: Vec<crate::types::TypeReferenceHandle>,
        /// The requirement's signature with the trait's parameters replaced
        /// by `trait_arguments`, built once every trait is lowered. Absent when
        /// the trait takes no arguments: the requirement's own signature is
        /// the binder's callable contract then.
        instantiated: Option<Box<crate::signature::StateSignature>>,
    },
}

impl Default for MachineParameterContract {
    fn default() -> Self {
        Self::Structural(crate::signature::StateSignature::default())
    }
}

impl MachineParameterContract {
    pub fn structural(&self) -> Option<&crate::signature::StateSignature> {
        match self {
            Self::Structural(signature) => Some(signature),
            Self::RequirementIdentity | Self::Nominal { .. } => None,
        }
    }

    pub fn structural_mut(&mut self) -> Option<&mut crate::signature::StateSignature> {
        match self {
            Self::Structural(signature) => Some(signature),
            Self::RequirementIdentity | Self::Nominal { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MachineParameterContractView<'program> {
    Structural(&'program crate::signature::StateSignature),
    Nominal {
        trait_definition: &'program crate::trait_definition::TraitDefinition,
        requirement: &'program crate::signature::StateSignature,
    },
}

impl<'program> MachineParameterContractView<'program> {
    pub fn signature(self) -> &'program crate::signature::StateSignature {
        match self {
            Self::Structural(signature)
            | Self::Nominal {
                requirement: signature,
                ..
            } => signature,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropositionParameterSignature {
    pub name: Identifier,
    pub parameters: HandleSpan<crate::signature::StateParameter>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataField {
    pub identity: Option<u64>,
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub relevance: language_core::BindingRelevance,
    pub type_reference: TypeReferenceHandle,
}

/// The canonical structural path segment spelling for a data member: its
/// schema ordinal `#n` when one was assigned, else its authored name. Checked
/// structural paths, their lowering, and their validation all match members by
/// this one spelling.
fn member_path_identity(identity: Option<u64>, name: &Identifier) -> String {
    identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| name.as_str().to_owned())
}

/// One data member's structural path segment identity: its schema ordinal
/// `#n` when one was assigned, else its authored name. Equality follows the
/// `member_path_identity` spelling — two members match when both carry
/// ordinals and the ordinals are equal, or when neither does and the names
/// match — while the field itself holds no rendered text. Shared by checked
/// structural paths, predicate paths, and their producers.
#[derive(Debug, Clone, Default, Eq)]
pub struct DataMemberIdentity {
    pub ordinal: Option<u64>,
    pub name: Identifier,
}

impl DataMemberIdentity {
    pub fn of_member(ordinal: Option<u64>, name: Identifier) -> Self {
        Self { ordinal, name }
    }

    /// Reconstructs the handle from a canonical path spelling (`#n` or the
    /// authored name) for producers that only ever held the rendered text.
    pub fn of_spelling(spelling: &str) -> Self {
        if let Some(ordinal) = spelling
            .strip_prefix('#')
            .and_then(|digits| digits.parse::<u64>().ok())
        {
            return Self {
                ordinal: Some(ordinal),
                name: Identifier::default(),
            };
        }
        Self {
            ordinal: None,
            name: Identifier::generated(spelling),
        }
    }

    /// Zero-member absence: no schema ordinal and no authored name.
    pub fn is_valid(&self) -> bool {
        self.ordinal.is_some() || !self.name.as_str().is_empty()
    }

    /// The member's canonical path spelling (`#n` or the authored name),
    /// rendered where a boundary or diagnostic still wants text.
    pub fn spelling(&self) -> String {
        member_path_identity(self.ordinal, &self.name)
    }
}

impl PartialEq for DataMemberIdentity {
    fn eq(&self, other: &Self) -> bool {
        match (self.ordinal, other.ordinal) {
            (Some(own), Some(theirs)) => own == theirs,
            (None, None) => self.name == other.name,
            _ => false,
        }
    }
}

// Ordering follows the canonical spelling so sorted rosters stay stable
// across the handle migration; paths are few-member and not allocation hot.
impl Ord for DataMemberIdentity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.spelling().cmp(&other.spelling())
    }
}

impl PartialOrd for DataMemberIdentity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// Hashing the spelling keeps segment-keyed sets consistent with Ord.
impl std::hash::Hash for DataMemberIdentity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.spelling().hash(state);
    }
}

impl DataField {
    /// This field's structural path segment spelling.
    pub fn path_identity(&self) -> String {
        member_path_identity(self.identity, &self.name)
    }

    /// This field's structural path segment identity handle.
    pub fn member_identity(&self) -> DataMemberIdentity {
        DataMemberIdentity::of_member(self.identity, self.name.clone())
    }
}

impl DataVariant {
    /// This case's structural path segment spelling.
    pub fn path_identity(&self) -> String {
        member_path_identity(self.identity, &self.name)
    }

    /// This case's structural path segment identity handle.
    pub fn member_identity(&self) -> DataMemberIdentity {
        DataMemberIdentity::of_member(self.identity, self.name.clone())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataVariant {
    pub identity: Option<u64>,
    pub symbol: SymbolHandle,
    pub name: Identifier,
    /// Named payload fields (`case Say(text: String);`); empty for payload-less cases.
    /// Stored in the `data_payload_fields` arena, separate from the parent's member span.
    pub payload: HandleSpan<DataField>,
    /// Case-local `where` facts proved at construction of this case; empty when
    /// the case declares none.
    pub where_facts: HandleSpan<crate::domain::ProofFact>,
    pub retired_payload_identities: Vec<u64>,
}

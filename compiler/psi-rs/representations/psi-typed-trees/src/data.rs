use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDefinition {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub supply_mode: psi_language_semantics::DataSupplyMode,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub properties: DataProperties,
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
            supply_mode: psi_language_semantics::DataSupplyMode::CheckedShape,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            properties: DataProperties::default(),
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
    pub carry: Option<psi_language_semantics::CarryPolicy>,
    /// The first-class usage model (`[copy]` -> Unrestricted, ordinary data ->
    /// Affine, `[linear]` -> Linear).
    pub multiplicity: psi_language_semantics::Multiplicity,
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
    Const {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineParameterContract {
    Structural(crate::signature::StateSignature),
    Nominal {
        trait_definition: SymbolHandle,
        requirement: SymbolHandle,
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
            Self::Nominal { .. } => None,
        }
    }

    pub fn structural_mut(&mut self) -> Option<&mut crate::signature::StateSignature> {
        match self {
            Self::Structural(signature) => Some(signature),
            Self::Nominal { .. } => None,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataField {
    pub identity: Option<u64>,
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub relevance: psi_language_core::BindingRelevance,
    pub type_reference: TypeReferenceHandle,
}

impl Default for DataField {
    fn default() -> Self {
        Self {
            identity: None,
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            relevance: psi_language_core::BindingRelevance::default(),
            type_reference: TypeReferenceHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVariant {
    pub identity: Option<u64>,
    pub symbol: SymbolHandle,
    pub name: Identifier,
    /// Named payload fields (`case Say(text: String);`); empty for payload-less cases.
    /// Stored in the `data_payload_fields` arena, separate from the parent's member span.
    pub payload: HandleSpan<DataField>,
    pub retired_payload_identities: Vec<u64>,
}

impl Default for DataVariant {
    fn default() -> Self {
        Self {
            identity: None,
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            payload: HandleSpan::empty(),
            retired_payload_identities: Vec::new(),
        }
    }
}

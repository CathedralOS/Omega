use crate::name::DiagnosticName;
use crate::types::TypeReference;
use arena::HandleSpan;
use std::ops::{Deref, DerefMut};
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataDefinition {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub is_public: bool,
    pub storage: DataDefinitionStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataDefinitionStorage {
    pub supply_mode: language_semantics::DataSupplyMode,
    pub lifetime_parameters: Vec<DiagnosticName>,
    pub type_parameters: HandleSpan<TypeParameter>,
    /// Resolved structural origin of a generated concrete generic instance.
    pub generic_instance: Option<TypeReference>,
    pub properties: DataProperties,
    /// N6 proof-only quotient metadata. The carrier is resolved like an
    /// ordinary type reference; the relation keeps its authored path and its
    /// resolved machine symbol so validation never re-resolves by text.
    pub quotient: Option<QuotientDefinition>,
    /// R2 (ch12): DEFAULT-DOMAIN facts. Facts that hold at zero are born
    /// established; facts that reject zero gate value establishment while
    /// leaving the representation zero-expressible. Construction and writes
    /// must prove the facts before the value is observable.
    pub where_facts: HandleSpan<crate::domain::ProofFact>,
    /// R2 rung 2b: zero VIOLATES the default domain -- the type is GATED
    /// (not zero-constructible; literals must prove the domain; reading
    /// zeroed storage as the type is refused by rung 3's access gate).
    pub zero_gated: bool,
    pub retired_identities: Vec<u64>,
    pub members: HandleSpan<DataMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotientDefinition {
    pub carrier: TypeReference,
    pub relation: Vec<DiagnosticName>,
    pub relation_symbol: SymbolHandle,
    pub equivalence: Option<QuotientEquivalenceSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotientEquivalenceSelection {
    pub relation: Vec<DiagnosticName>,
    pub relation_symbol: SymbolHandle,
    pub trait_name: DiagnosticName,
    pub trait_symbol: SymbolHandle,
    pub trait_arguments: HandleSpan<TypeReference>,
    pub conformance_name: DiagnosticName,
    pub conformance_symbol: SymbolHandle,
}

/// Declared type properties (`data Point [copy]`). The spelling set is closed
/// at parse time, so only normalized semantic properties travel here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataProperties {
    pub carry: Option<language_semantics::CarryPolicy>,
    pub multiplicity: language_semantics::Multiplicity,
}

impl Deref for DataDefinition {
    type Target = DataDefinitionStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for DataDefinition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeParameter {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub kind: TypeParameterKind,
    /// Property bounds (`data Box<T [copy]>`, frozen decision 13).
    pub bounds: DataProperties,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TypeParameterKind {
    #[default]
    Type,
    Const {
        type_reference: TypeReference,
    },
    /// Static machine-symbol parameter with its mandatory authored
    /// requirement. Structural contracts carry their signature inline;
    /// nominal contracts retain the exact canonical trait-requirement row.
    Machine {
        contract: MachineParameterContract,
    },
    /// Generic proof-formula family with an authored value-parameter
    /// signature. This is not an executable machine contract.
    Proposition {
        contract: PropositionParameterSignature,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineParameterContract {
    RequirementIdentity,
    Structural(crate::signature::StateSignature),
    /// Temporary authored spelling retained until all declarations have
    /// symbols. The syntax-to-resolved finisher must eliminate this variant.
    AuthoredNominal {
        requirement: Vec<DiagnosticName>,
    },
    Nominal {
        trait_definition: SymbolHandle,
        requirement: SymbolHandle,
        /// Exact source-backed `Trait::requirement` path retained until typed
        /// lowering records both authored declaration selections.
        authored_path: Vec<DiagnosticName>,
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
            Self::RequirementIdentity | Self::AuthoredNominal { .. } | Self::Nominal { .. } => None,
        }
    }

    pub fn structural_mut(&mut self) -> Option<&mut crate::signature::StateSignature> {
        match self {
            Self::Structural(signature) => Some(signature),
            Self::RequirementIdentity | Self::AuthoredNominal { .. } | Self::Nominal { .. } => None,
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
    pub name: DiagnosticName,
    pub parameters: HandleSpan<crate::signature::StateParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataField {
    pub identity: Option<u64>,
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub relevance: language_core::BindingRelevance,
    pub type_reference: TypeReference,
}

impl Default for DataField {
    fn default() -> Self {
        Self {
            identity: None,
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
            relevance: language_core::BindingRelevance::default(),
            type_reference: TypeReference::Unit,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataVariant {
    pub identity: Option<u64>,
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    /// Named payload fields (`case Say(text: String);`); empty for payload-less cases.
    pub payload: HandleSpan<DataField>,
    pub retired_payload_identities: Vec<u64>,
}

use crate::name::DiagnosticName;
use crate::types::TypeReference;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataDefinition {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub storage: DataDefinitionStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataDefinitionStorage {
    pub type_parameters: HandleSpan<TypeParameter>,
    pub properties: DataProperties,
    /// R2 (ch12): DEFAULT-DOMAIN facts. Facts that hold at zero are born
    /// established; facts that reject zero gate value establishment while
    /// leaving the representation zero-expressible. Construction and writes
    /// must prove the facts before the value is observable.
    pub where_facts: HandleSpan<crate::domain::ProofFact>,
    /// R2 rung 2b: zero VIOLATES the default domain -- the type is GATED
    /// (not zero-constructible; literals must prove the domain; reading
    /// zeroed storage as the type is refused by rung 3's access gate).
    pub zero_gated: bool,
    pub members: HandleSpan<DataMember>,
}

/// Declared type properties (`data Point [copy, zero_init]`). The spelling
/// set is closed at parse time, so only the resolved flags travel here.
/// STR3: `multiplicity` is the first-class usage model (`[copy]` ->
/// Unrestricted, ordinary data -> Affine, `[linear]` -> Linear);
/// `copy` survives as the compatibility bool until STR7 retires it —
/// consumers migrate to the multiplicity, never the other way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataProperties {
    pub copy: bool,
    pub zero_init: bool,
    pub carry: Option<omega_core::semantics::CarryPolicy>,
    pub multiplicity: omega_core::semantics::Multiplicity,
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
    /// requirement. The signature is carried inline so no use-site or
    /// instantiation-dependent inference can redefine the abstraction.
    Machine {
        contract: crate::signature::StateSignature,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataField {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub type_reference: TypeReference,
}

impl Default for DataField {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
            type_reference: TypeReference::Unit,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataVariant {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    /// Named payload fields (`case Say(text: String);`); empty for payload-less cases.
    pub payload: HandleSpan<DataField>,
}

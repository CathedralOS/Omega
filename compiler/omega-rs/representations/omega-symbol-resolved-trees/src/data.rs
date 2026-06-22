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
    pub members: HandleSpan<DataMember>,
}

/// Declared type properties (`data Point [copy, zero_init]`). The spelling
/// set is closed at parse time, so only the resolved flags travel here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataProperties {
    pub copy: bool,
    pub zero_init: bool,
    pub send: bool,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataField {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub type_reference: TypeReference,
    pub initial_value: crate::expression::ExpressionHandle,
}

impl Default for DataField {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
            type_reference: TypeReference::Unit,
            initial_value: crate::expression::ExpressionHandle::invalid(),
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

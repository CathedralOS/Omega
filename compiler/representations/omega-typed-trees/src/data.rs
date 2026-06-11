use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDefinition {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub properties: DataProperties,
    pub members: HandleSpan<DataMember>,
}

impl Default for DataDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            type_parameters: HandleSpan::empty(),
            properties: DataProperties::default(),
            members: HandleSpan::empty(),
        }
    }
}

/// Declared type properties (`data Point [copy, zero_init]`). The spelling
/// set is closed at parse time; validation verifies the declared facts
/// (`copy`/`send` structurally, `zero_init` via the zero-means-empty rules).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataProperties {
    pub copy: bool,
    pub zero_init: bool,
    pub send: bool,
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
    /// Property bounds (`data Box<T [copy]>`, frozen decision 13). A bounded
    /// parameter satisfies the structural copy/send/zero_init walk inside its
    /// owner, and every instantiation argument must carry the bound.
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataField {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub initial_value: crate::expression::ExpressionHandle,
}

impl Default for DataField {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            type_reference: TypeReferenceHandle::invalid(),
            initial_value: crate::expression::ExpressionHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVariant {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    /// Named payload fields (`case Say(text: String);`); empty for payload-less cases.
    /// Stored in the `data_payload_fields` arena, separate from the parent's member span.
    pub payload: HandleSpan<DataField>,
}

impl Default for DataVariant {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            payload: HandleSpan::empty(),
        }
    }
}

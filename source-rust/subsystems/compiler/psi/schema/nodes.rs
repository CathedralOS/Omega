#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, record_storage::Record)]
pub struct SchemaNodeHandle(pub(crate) u32);

impl SchemaNodeHandle {
    pub const INVALID: Self = Self(0);

    pub const fn invalid() -> Self {
        Self::INVALID
    }

    /// A coordinate in this schema's own node table. It describes data;
    /// callers still authenticate the complete owning schema before use.
    pub const fn from_index(index: u32) -> Self {
        Self(index)
    }

    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SchemaNode {
    Invalid,
    Declaration(DeclarationDescription),
    Field(FieldDescription),
    Case(CaseDescription),
    NominalReference(NominalReferenceDescription),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclarationDescription {
    pub owner_identity: String,
    pub identity_is_hermetic: bool,
    pub name: String,
    pub shape: SchemaShape,
    pub supply: String,
    pub multiplicity: String,
    pub carry: Option<String>,
    pub type_parameters: Vec<TypeParameterDescription>,
    pub lifetime_parameters: Vec<String>,
    pub application_identity: Option<String>,
    pub members: Vec<SchemaNodeHandle>,
    pub retired_identities: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeParameterDescription {
    pub name: String,
    pub kind: &'static str,
    pub type_identity: Option<String>,
    pub bounds: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldDescription {
    pub member_identity: String,
    pub name: String,
    pub declaration_position: u32,
    pub stable_number: Option<u64>,
    pub erased: bool,
    pub qualified_type: String,
    pub nominal_references: Vec<SchemaNodeHandle>,
    pub owner_case: SchemaNodeHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CaseDescription {
    pub member_identity: String,
    pub name: String,
    pub declaration_position: u32,
    pub stable_number: Option<u64>,
    pub payload_fields: Vec<SchemaNodeHandle>,
    pub retired_payload_identities: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NominalReferenceDescription {
    pub owner_identity: String,
    pub owner_kind: String,
    pub name: String,
    pub application_identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SchemaShape {
    Empty,
    Record,
    Sum,
    Mixed,
    Primitive(symbols::BuiltinTypeAtom),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SchemaQueryAuthority {
    OwningScope { requester_identity: String },
    ForeignScope { requester_identity: String },
}

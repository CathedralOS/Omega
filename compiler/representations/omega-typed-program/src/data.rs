use crate::types::TypeReference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDefinition {
    pub name: String,
    pub members: Vec<DataMember>,
}

impl DataDefinition {
    pub fn shape_kind(&self) -> DataShapeKind {
        let mut has_fields = false;
        let mut has_variants = false;

        for member in &self.members {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataField {
    pub name: String,
    pub type_reference: TypeReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVariant {
    pub name: String,
}

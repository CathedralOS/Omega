use crate::identifier::Identifier;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeReference {
    Constrained {
        base_type: Box<TypeReference>,
        constraints: Vec<TypeConstraint>,
    },
    FixedArray {
        element_type: Box<TypeReference>,
        length: usize,
    },
    Generic {
        base_name: Identifier,
        arguments: Vec<TypeReference>,
    },
    Named(Identifier),
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraint {
    Named(Identifier),
    Range {
        minimum: crate::expression::Expression,
        maximum: crate::expression::Expression,
    },
}

impl TypeReference {
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(Identifier::generated(name))
    }
}

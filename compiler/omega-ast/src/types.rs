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
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraint {
    Named(String),
    Range {
        minimum: crate::expression::Expression,
        maximum: crate::expression::Expression,
    },
}

impl TypeReference {
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(name.into())
    }
}

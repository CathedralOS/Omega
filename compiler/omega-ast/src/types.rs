#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeReference {
    Constrained {
        base_type: Box<TypeReference>,
        constraints: String,
    },
    FixedArray {
        element_type: Box<TypeReference>,
        length: usize,
    },
    Named(String),
}

impl TypeReference {
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(name.into())
    }
}

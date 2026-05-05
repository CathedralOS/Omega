#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeReference {
    FixedArray {
        element_type: Box<TypeReference>,
        length: usize,
    },
    Named(String),
}

impl TypeReference {
    pub fn display_name(&self) -> String {
        match self {
            TypeReference::FixedArray {
                element_type,
                length,
            } => {
                format!("[{}; {}]", element_type.display_name(), length)
            }
            TypeReference::Named(name) => name.clone(),
        }
    }
}

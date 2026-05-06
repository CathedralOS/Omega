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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    Bool,
    F32,
    F64,
    I32,
    String,
    U32,
    U64,
    Usize,
}

impl TypeReference {
    pub fn display_name(&self) -> String {
        match self {
            TypeReference::Constrained {
                base_type,
                constraints,
            } => {
                format!("{}[{}]", base_type.display_name(), constraints)
            }
            TypeReference::FixedArray {
                element_type,
                length,
            } => {
                format!("[{}; {}]", element_type.display_name(), length)
            }
            TypeReference::Named(name) => name.clone(),
        }
    }

    pub fn primitive_type(&self) -> Option<PrimitiveType> {
        match self {
            TypeReference::Constrained { base_type, .. } => base_type.primitive_type(),
            TypeReference::Named(name) => PrimitiveType::from_name(name),
            TypeReference::FixedArray { .. } => None,
        }
    }
}

impl PrimitiveType {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "bool" => Some(Self::Bool),
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            "i32" => Some(Self::I32),
            "String" => Some(Self::String),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "usize" => Some(Self::Usize),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::I32 => "i32",
            Self::String => "String",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::Usize => "usize",
        }
    }

    pub fn accepts_integer_literal(self) -> bool {
        matches!(self, Self::I32 | Self::U32 | Self::U64 | Self::Usize)
    }

    pub fn accepts_float_literal(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
}

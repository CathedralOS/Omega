use omega_core::arena::{Arena, HandleSpan};

use crate::name::ProgramName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeReference {
    Constrained {
        base_type: Box<TypeReference>,
        constraints: HandleSpan<TypeConstraint>,
    },
    FixedArray {
        element_type: Box<TypeReference>,
        length: usize,
    },
    Generic {
        base_name: ProgramName,
        arguments: Vec<TypeReference>,
    },
    Named(ProgramName),
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraint {
    Named(ProgramName),
    Range {
        minimum: crate::expression::Expression,
        maximum: crate::expression::Expression,
    },
}

impl Default for TypeConstraint {
    fn default() -> Self {
        Self::Named(ProgramName::default())
    }
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
                format!(
                    "{}[{}]",
                    base_type.display_name(),
                    match constraints.count() {
                        1 => "1 constraint".to_owned(),
                        count => format!("{count} constraints"),
                    }
                )
            }
            TypeReference::FixedArray {
                element_type,
                length,
            } => {
                format!("[{}; {}]", element_type.display_name(), length)
            }
            TypeReference::Generic {
                base_name,
                arguments,
            } => {
                let arguments = arguments
                    .iter()
                    .map(TypeReference::display_name)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{base_name}<{arguments}>")
            }
            TypeReference::Named(name) => name.to_string(),
            TypeReference::Unit => "()".to_owned(),
        }
    }

    pub fn display_name_with_constraints(
        &self,
        type_constraints: &Arena<TypeConstraint>,
    ) -> String {
        match self {
            TypeReference::Constrained {
                base_type,
                constraints,
            } => {
                let constraints = type_constraints.span(*constraints).unwrap_or(&[]);
                format!(
                    "{}[{}]",
                    base_type.display_name_with_constraints(type_constraints),
                    constraints
                        .iter()
                        .map(TypeConstraint::display_name)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            TypeReference::FixedArray {
                element_type,
                length,
            } => {
                format!(
                    "[{}; {}]",
                    element_type.display_name_with_constraints(type_constraints),
                    length
                )
            }
            TypeReference::Generic {
                base_name,
                arguments,
            } => {
                let arguments = arguments
                    .iter()
                    .map(|argument| argument.display_name_with_constraints(type_constraints))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{base_name}<{arguments}>")
            }
            TypeReference::Named(name) => name.to_string(),
            TypeReference::Unit => "()".to_owned(),
        }
    }

    pub fn primitive_type(&self) -> Option<PrimitiveType> {
        match self {
            TypeReference::Constrained { base_type, .. } => base_type.primitive_type(),
            TypeReference::Named(name) => PrimitiveType::from_name(name),
            TypeReference::FixedArray { .. }
            | TypeReference::Generic { .. }
            | TypeReference::Unit => None,
        }
    }
}

impl TypeConstraint {
    pub fn display_name(&self) -> String {
        match self {
            TypeConstraint::Named(name) => name.to_string(),
            TypeConstraint::Range { minimum, maximum } => {
                format!(
                    "range<{}, {}>",
                    minimum.display_name(),
                    maximum.display_name()
                )
            }
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

    pub fn accepts_range_constraint(self) -> bool {
        matches!(
            self,
            Self::F32 | Self::F64 | Self::I32 | Self::U32 | Self::U64 | Self::Usize
        )
    }

    pub fn accepts_finite_constraint(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
}

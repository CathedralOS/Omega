use num_bigint::BigInt;
use omega_resolved::DeclarationId;
use std::{fmt, sync::Arc};

/// A canonical owned snapshot. Aggregate fields follow the declaration's field order.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum StaticValue {
    Integer(BigInt),
    Bool(bool),
    Array(Arc<[StaticValue]>),
    Record {
        owner: DeclarationId,
        fields: Arc<[StaticValue]>,
    },
    Sum {
        owner: DeclarationId,
        case: DeclarationId,
        fields: Arc<[StaticValue]>,
    },
}

impl From<BigInt> for StaticValue {
    fn from(value: BigInt) -> Self {
        Self::Integer(value)
    }
}

impl fmt::Display for StaticValue {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let (start, children, end) = match self {
            Self::Integer(value) => return write!(f, "{value}"),
            Self::Bool(value) => return write!(f, "{value}"),
            Self::Array(elements) => ("[", elements, "]"),
            Self::Record { owner, fields } => {
                write!(f, "{} ", owner.path)?;
                ("{", fields, "}")
            }
            Self::Sum { case, fields, .. } => {
                write!(f, "{} ", case.path)?;
                ("{", fields, "}")
            }
        };
        write!(f, "{start}")?;
        for (position, child) in children.iter().enumerate() {
            if position != 0 {
                write!(f, ", ")?;
            }
            write!(f, "{child}")?;
        }
        write!(f, "{end}")
    }
}

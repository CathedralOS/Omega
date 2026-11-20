use crate::ValueType;
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

impl StaticValue {
    /// Admit one node and pair its active children with their exact carriers.
    /// The caller supplies the field authority for this application and case:
    /// L3 specializes declaration fields; a completed forest already has them.
    /// This checks representation, not a construction or canonical-identity proof.
    pub fn children_at<'a, T, I>(
        &'a self,
        carrier: &'a ValueType,
        borrow: impl Fn(&'a ValueType) -> T,
        fields: impl FnOnce(&'a DeclarationId, &'a [ValueType], Option<&'a DeclarationId>) -> Option<I>,
    ) -> Option<impl Iterator<Item = (&'a StaticValue, T)>>
    where
        I: ExactSizeIterator<Item = T>,
    {
        let (values, element, mut declared): (&[Self], _, _) = match (self, carrier) {
            (Self::Integer(value), ty) if ty.contains_integer(value) => (&[], carrier, None),
            (Self::Bool(_), ValueType::Bool) => (&[], carrier, None),
            (Self::Array(values), ValueType::Array(element, length)) if u64::try_from(values.len()) == Ok(*length) => {
                (values, element.as_ref(), None)
            }
            (Self::Record { owner, fields: values } | Self::Sum { owner, fields: values, .. }, _) => {
                let (declaration, arguments) = carrier.nominal_parts()?;
                if owner != declaration {
                    return None;
                }
                let case = match self {
                    Self::Sum { case, .. } => Some(case),
                    _ => None,
                };
                let types = fields(declaration, arguments, case)?;
                if values.len() != types.len() {
                    return None;
                }
                (values, carrier, Some(types))
            }
            _ => return None,
        };
        let types = std::iter::from_fn(move || match &mut declared {
            Some(types) => types.next(),
            None => Some(borrow(element)),
        });
        Some(values.iter().zip(types))
    }
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

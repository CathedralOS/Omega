use crate::ValueType;
use crate::declarations::declaration_catalog::DeclarationCatalog;
use num_bigint::BigInt;
use omega_resolver::DeclarationId;
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

impl ValueType {
    pub(crate) fn supports_closed_snapshot(&self) -> bool {
        matches!(
            self.carrier(),
            Self::Bool | Self::Array(..) | Self::IndexedArray(..) | Self::Nominal(_) | Self::Applied(..)
        )
    }
}

impl StaticValue {
    pub(super) fn matches_carrier(
        &self,
        carrier: &ValueType,
        catalog: &DeclarationCatalog<'_>,
    ) -> bool {
        match (self, carrier.carrier()) {
            (Self::Integer(value), ty) => ty.contains_integer(value),
            (Self::Bool(_), ValueType::Bool) => true,
            (Self::Array(values), ValueType::Array(element, length)) => {
                u64::try_from(values.len()) == Ok(*length)
                    && values
                        .iter()
                        .all(|value| value.matches_carrier(element, catalog))
            }
            (value, ty) => {
                let Some((declaration, arguments)) = ty.nominal_parts() else {
                    return false;
                };
                let (shape, values) = match value {
                    Self::Record { owner, fields } if owner == declaration && !catalog.cases.contains_key(owner) => (owner, fields),
                    Self::Sum { owner, case, fields }
                        if owner == declaration
                            && catalog
                                .cases
                                .get(owner)
                                .is_some_and(|cases| cases.contains(&case)) =>
                    {
                        (case, fields)
                    }
                    _ => return false,
                };
                let Some(fields) = catalog.fields.get(shape) else {
                    return false;
                };
                let fields = &fields.ordered;
                fields.len() == values.len()
                    && fields.iter().zip(values.iter()).all(|(field, value)| {
                        let ty = catalog
                            .types
                            .apply_type_arguments(&field.ty, declaration, arguments);
                        value.matches_carrier(&ty, catalog)
                    })
            }
        }
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

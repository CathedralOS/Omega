use std::sync::Arc;

use num_bigint::BigInt;
use omega_resolved::DeclarationId;
use omega_target_contract::FloatFormat;
use omega_typed::{Site, StaticValue};

/// The exact IEEE representation when the computation determines one.
/// An arithmetic NaN still has semantic meaning when its bits are unspecified.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedFloat {
    Binary32(u32),
    Binary64(u64),
    UnspecifiedNaN(FloatFormat),
}

impl PreparedFloat {
    pub fn format(self) -> FloatFormat {
        match self {
            Self::Binary32(_) => FloatFormat::Binary32,
            Self::Binary64(_) => FloatFormat::Binary64,
            Self::UnspecifiedNaN(format) => format,
        }
    }
}

/// A frozen semantic value. Aggregate fields are in declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparedValue {
    Integer(BigInt),
    Bool(bool),
    Float {
        value: PreparedFloat,
        origin: Site,
    },
    Array(Arc<[PreparedValue]>),
    Tuple(Arc<[PreparedValue]>),
    Record {
        owner: DeclarationId,
        fields: Arc<[PreparedValue]>,
    },
    Sum {
        owner: DeclarationId,
        case: DeclarationId,
        fields: Arc<[PreparedValue]>,
    },
}

impl PreparedValue {
    pub fn float(&self) -> Option<PreparedFloat> {
        match self {
            Self::Float { value, .. } => Some(*value),
            _ => None,
        }
    }

    /// Check that the active semantic components determine their observable values.
    /// A consumer still needs a selected layout before writing runtime bytes.
    pub fn check_materializable(&self) -> Result<(), MaterializationError> {
        enum Component<'a> {
            Element(usize),
            Field(&'a DeclarationId, usize),
        }
        let mut stack = vec![(self, None, 0)];
        let mut path = Vec::new();
        while let Some((value, component, depth)) = stack.pop() {
            path.truncate(depth);
            path.extend(component);
            let fields = match value {
                Self::Float {
                    value: PreparedFloat::UnspecifiedNaN(_),
                    origin,
                } => {
                    return Err(MaterializationError {
                        path: path
                            .into_iter()
                            .map(|component| match component {
                                Component::Element(index) => MaterializationComponent::Element(index),
                                Component::Field(owner, index) => MaterializationComponent::Field { owner: owner.clone(), index },
                            })
                            .collect(),
                        origin: origin.clone(),
                    });
                }
                Self::Integer(_) | Self::Bool(_) | Self::Float { .. } => continue,
                Self::Array(fields) | Self::Tuple(fields) => (fields, None),
                Self::Record { owner, fields } => (fields, Some(owner)),
                Self::Sum { case, fields, .. } => (fields, Some(case)),
            };
            let (fields, owner) = fields;
            stack.extend(fields.iter().enumerate().rev().map(|(index, field)| {
                let component = owner.map_or(Component::Element(index), |owner| Component::Field(owner, index));
                (field, Some(component), path.len())
            }));
        }
        Ok(())
    }
}

/// Path from the constant's root to the first component with unspecified NaN bits.
#[derive(Debug, Eq, PartialEq)]
pub struct MaterializationError {
    pub path: Vec<MaterializationComponent>,
    pub origin: Site,
}

#[derive(Debug, Eq, PartialEq)]
pub enum MaterializationComponent {
    Element(usize),
    /// A declaration-order slot in a record or an active sum case.
    Field {
        owner: DeclarationId,
        index: usize,
    },
}

impl From<&StaticValue> for PreparedValue {
    fn from(value: &StaticValue) -> Self {
        match value {
            StaticValue::Integer(value) => Self::Integer(value.clone()),
            StaticValue::Bool(value) => Self::Bool(*value),
            StaticValue::Array(values) => Self::Array(values.iter().map(Self::from).collect()),
            StaticValue::Record { owner, fields } => Self::Record {
                owner: owner.clone(),
                fields: fields.iter().map(Self::from).collect(),
            },
            StaticValue::Sum { owner, case, fields } => Self::Sum {
                owner: owner.clone(),
                case: case.clone(),
                fields: fields.iter().map(Self::from).collect(),
            },
        }
    }
}

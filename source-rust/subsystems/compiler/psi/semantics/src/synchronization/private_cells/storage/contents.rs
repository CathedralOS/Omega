//! Values a private cell can hold at a control-flow join. Two branches can
//! store different words that both enable the same wait.

use super::Location;
use crate::entry_values::TerminalScalarValue as Scalar;
use crate::synchronization::progress::ScalarFact;
use crate::synchronization::spin::CellUpdate;
use semantic_vocabulary::{ScalarType, StructuralCaseId};
use std::sync::Arc;
use terminal_psi::StructuralPathSegment as Segment;

#[derive(Clone, Default)]
pub(in super::super) enum Stored {
    Zero,
    Scalar(ScalarFact),
    Alternatives {
        includes_zero: bool,
        values: Vec<ScalarFact>,
    },
    Reference(Location),
    Aggregate(Arc<Aggregate>),
    #[default]
    Unknown,
}

#[derive(Clone)]
pub(in super::super) struct Aggregate {
    tag: Option<StructuralCaseId>,
    zeroed: bool,
    fields: Vec<(Segment, Stored)>,
}

impl Stored {
    pub(in super::super) fn variant(case: StructuralCaseId) -> Self {
        Self::Aggregate(Arc::new(Aggregate {
            tag: Some(case),
            zeroed: false,
            fields: Vec::new(),
        }))
    }

    pub(in super::super) fn tag(&self) -> Option<StructuralCaseId> {
        match self {
            Self::Aggregate(value) => value.tag,
            _ => None,
        }
    }

    pub(super) fn field(&self, key: &Segment) -> &Self {
        match self {
            Self::Zero => &Self::Zero,
            Self::Aggregate(record) => record
                .fields
                .iter()
                .find(|(field, _)| field == key)
                .map(|(_, value)| value)
                .unwrap_or(if record.zeroed {
                    &Self::Zero
                } else {
                    &Self::Unknown
                }),
            _ => &Self::Unknown,
        }
    }

    /// Shared copies retain one description until the corresponding field
    /// is written. Nesting a record never expands all possible element paths.
    pub(super) fn field_mut(&mut self, key: &Segment) -> &mut Self {
        if !matches!(self, Self::Aggregate(_)) {
            *self = Self::Aggregate(Arc::new(Aggregate {
                tag: None,
                zeroed: matches!(self, Self::Zero),
                fields: Vec::new(),
            }));
        }
        let Self::Aggregate(record) = self else {
            unreachable!()
        };
        let record = Arc::make_mut(record);
        let index = match record.fields.iter().position(|(field, _)| field == key) {
            Some(index) => index,
            None => {
                let index = record.fields.len();
                record.fields.push((
                    key.clone(),
                    if record.zeroed {
                        Self::Zero
                    } else {
                        Self::Unknown
                    },
                ));
                index
            }
        };
        &mut record.fields[index].1
    }

    // Identical cells keep all values established at incoming edges. Zero
    // remains untyped until a checked operation supplies its scalar type.
    pub(super) fn join(&mut self, other: &Self) {
        match (&*self, other) {
            (Self::Unknown, _) | (Self::Zero, Self::Zero) => return,
            (Self::Scalar(a), Self::Scalar(b)) if a == b => return,
            (Self::Reference(a), Self::Reference(b)) if a == b => return,
            (Self::Aggregate(a), Self::Aggregate(b)) if Arc::ptr_eq(a, b) => return,
            (Self::Aggregate(_) | Self::Zero, Self::Aggregate(_) | Self::Zero) => {
                let fields = |value: &Self| match value {
                    Self::Aggregate(a) => a
                        .fields
                        .iter()
                        .map(|(key, _)| key.clone())
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                let zeroed = |value: &Self| {
                    matches!(value, Self::Zero) || matches!(value, Self::Aggregate(a) if a.zeroed)
                };
                let mut result = Self::Aggregate(Arc::new(Aggregate {
                    tag: self.tag().filter(|case| other.tag() == Some(*case)),
                    zeroed: zeroed(self) && zeroed(other),
                    fields: Vec::new(),
                }));
                let mut keys = fields(self);
                keys.extend(fields(other));
                keys.sort_unstable();
                keys.dedup();
                for key in keys {
                    let mut value = self.field(&key).clone();
                    value.join(other.field(&key));
                    *result.field_mut(&key) = value;
                }
                *self = result;
            }
            (
                Self::Zero | Self::Scalar(_) | Self::Alternatives { .. },
                Self::Zero | Self::Scalar(_) | Self::Alternatives { .. },
            ) => {
                let mut includes_zero = false;
                let mut values = Vec::new();
                for item in [&*self, other] {
                    match item {
                        Self::Zero => includes_zero = true,
                        Self::Scalar(value) => values.push(*value),
                        Self::Alternatives {
                            includes_zero: zero,
                            values: alternatives,
                        } => {
                            includes_zero |= zero;
                            values.extend(alternatives);
                        }
                        _ => unreachable!(),
                    }
                }
                let mut unique = Vec::new();
                for value in values {
                    if !unique.contains(&value) {
                        unique.push(value);
                    }
                }
                *self = Self::Alternatives {
                    includes_zero,
                    values: unique,
                };
            }
            _ => *self = Self::Unknown,
        }
    }

    /// Apply the instruction's transfer independently to every admitted
    /// resident. A mismatch preserves the actual observed word.
    pub(in super::super::super) fn after_update(&self, update: CellUpdate) -> Self {
        let expected = match update {
            CellUpdate::Observe => return self.clone(),
            CellUpdate::Replace(value) => return Self::Scalar(ScalarFact::Literal(value)),
            CellUpdate::CompareExchange {
                expected,
                replacement,
            } if expected == replacement => {
                return self.clone();
            }
            CellUpdate::CompareExchange { expected, .. } => expected,
        };
        let mut result: Option<Self> = None;
        let mapped = self.visit_scalars(expected.scalar_type(), |prior| {
            let value = match prior {
                ScalarFact::Literal(bits) => Self::Scalar(ScalarFact::Literal(update.apply(bits))),
                ScalarFact::EntryInput(_) => Self::Unknown,
            };
            if let Some(existing) = &mut result {
                existing.join(&value);
            } else {
                result = Some(value);
            }
        });
        mapped.and(result).unwrap_or_default()
    }

    pub(in super::super) fn visit_scalars(
        &self,
        ty: ScalarType,
        mut visit: impl FnMut(ScalarFact),
    ) -> Option<()> {
        if let Self::Alternatives {
            includes_zero,
            values,
        } = self
        {
            if *includes_zero {
                visit(ScalarFact::Literal(Scalar::zero(ty)?));
            }
            for &value in values {
                if value.ty() != ty {
                    return None;
                }
                visit(value);
            }
        } else {
            visit(self.scalar(ty)?);
        }
        Some(())
    }

    pub(in super::super) fn scalar(&self, ty: ScalarType) -> Option<ScalarFact> {
        match self {
            Self::Zero => Scalar::zero(ty).map(ScalarFact::Literal),
            Self::Scalar(value) if value.ty() == ty => Some(*value),
            Self::Alternatives {
                includes_zero,
                values,
            } => {
                let value = if *includes_zero {
                    ScalarFact::Literal(Scalar::zero(ty)?)
                } else {
                    *values.first()?
                };
                (value.ty() == ty && values.iter().all(|&candidate| candidate == value))
                    .then_some(value)
            }
            _ => None,
        }
    }
}

impl From<Option<ScalarFact>> for Stored {
    fn from(value: Option<ScalarFact>) -> Self {
        value.map_or(Self::Unknown, Self::Scalar)
    }
}

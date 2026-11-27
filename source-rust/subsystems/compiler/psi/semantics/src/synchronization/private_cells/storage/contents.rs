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
    tags: Vec<StructuralCaseId>,
    zeroed: bool,
    fields: Vec<(Segment, Stored)>,
}

impl Stored {
    pub(in super::super) fn variant(case: StructuralCaseId) -> Self {
        Self::Aggregate(Arc::new(Aggregate {
            tags: vec![case],
            zeroed: false,
            fields: Vec::new(),
        }))
    }

    pub(in super::super) fn tags(&self) -> &[StructuralCaseId] {
        match self {
            Self::Aggregate(value) => &value.tags,
            _ => &[],
        }
    }

    pub(in super::super) fn tag(&self) -> Option<StructuralCaseId> {
        match self.tags() {
            [tag] => Some(*tag),
            _ => None,
        }
    }

    pub(in super::super) fn select_case(&self, case: StructuralCaseId) -> Self {
        let mut result = match self {
            Self::Aggregate(aggregate) => aggregate.clone(),
            _ => Arc::new(Aggregate {
                tags: Vec::new(),
                zeroed: matches!(self, Self::Zero),
                fields: Vec::new(),
            }),
        };
        let value = Arc::make_mut(&mut result);
        value.tags.clear();
        value.tags.push(case);
        Self::Aggregate(result)
    }

    pub(super) fn project(&self, path: &[Segment]) -> &Self {
        path.iter().fold(self, |value, field| value.field(field))
    }

    pub(super) fn project_mut(&mut self, path: &[Segment]) -> &mut Self {
        path.iter()
            .fold(self, |value, field| value.field_mut(field))
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
                tags: Vec::new(),
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
    pub(in super::super) fn join(&mut self, other: &Self) {
        self.join_with(other, &mut Location::join);
    }

    pub(super) fn join_with(
        &mut self,
        other: &Self,
        borrow: &mut impl FnMut(&mut Location, &Location),
    ) {
        match (&*self, other) {
            (Self::Unknown, _) | (Self::Zero, Self::Zero) => return,
            (Self::Scalar(a), Self::Scalar(b)) if a == b => return,
            (Self::Reference(a), Self::Reference(b)) => {
                let mut home = a.clone();
                borrow(&mut home, b);
                *self = Self::Reference(home);
            }
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
                let mut tags = Vec::new();
                if !self.tags().is_empty() && !other.tags().is_empty() {
                    tags.extend_from_slice(self.tags());
                    tags.extend_from_slice(other.tags());
                    tags.sort_unstable();
                    tags.dedup();
                }
                let mut result = Self::Aggregate(Arc::new(Aggregate {
                    tags,
                    zeroed: zeroed(self) && zeroed(other),
                    fields: Vec::new(),
                }));
                let mut keys = fields(self);
                keys.extend(fields(other));
                keys.sort_unstable();
                keys.dedup();
                for key in keys {
                    // A payload field is available only in its own case. A
                    // predecessor that constructed another case contributes
                    // no value to this field.
                    let admits = |v: &Self| match &key {
                        Segment::Case(tag) if !v.tags().is_empty() => {
                            v.tags().binary_search(tag).is_ok()
                        }
                        _ => true,
                    };
                    let value = match (admits(self), admits(other)) {
                        (true, true) => {
                            let mut value = self.field(&key).clone();
                            value.join_with(other.field(&key), borrow);
                            value
                        }
                        (true, false) => self.field(&key).clone(),
                        (false, true) => other.field(&key).clone(),
                        (false, false) => continue,
                    };
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
            CellUpdate::CompareExchange { expected, .. }
            | CellUpdate::BitwiseAnd(expected)
            | CellUpdate::BitwiseOr(expected) => expected,
        };
        self.map_scalars(expected.scalar_type(), |prior| match prior {
            ScalarFact::Literal(bits) => update.apply(bits).map(ScalarFact::Literal),
            ScalarFact::EntryInput(_) => None,
        })
    }

    pub(in super::super) fn map_scalars(
        &self,
        ty: ScalarType,
        mut apply: impl FnMut(ScalarFact) -> Option<ScalarFact>,
    ) -> Self {
        let mut result: Option<Self> = None;
        let mapped = self.visit_scalars(ty, |prior| {
            let value = Self::from(apply(prior));
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

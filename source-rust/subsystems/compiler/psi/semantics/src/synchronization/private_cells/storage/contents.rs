//! Values a private cell can hold at a control-flow join. Two branches can
//! store different words that both enable the same wait.

use super::Location;
use crate::entry_values::TerminalScalarValue as Scalar;
use crate::synchronization::progress::{ProgressFailure, ScalarFact};
use crate::synchronization::spin::{CellUpdate, SpinWait};
use semantic_vocabulary::{Proposition, ScalarType, StructuralCaseId};
use std::sync::Arc;
use terminal_psi::StructuralPathSegment as Segment;

#[derive(Clone, Default)]
pub(in super::super) enum Stored {
    /// Initial bytes supplied by the caller, still untouched on every arrival.
    Incoming,
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
    fill: FieldDefault,
    fields: Vec<(Segment, Stored)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldDefault {
    Zero,
    Incoming,
    Unknown,
}

impl FieldDefault {
    fn contents(self) -> &'static Stored {
        match self {
            Self::Zero => &Stored::Zero,
            Self::Incoming => &Stored::Incoming,
            Self::Unknown => &Stored::Unknown,
        }
    }
}

impl Stored {
    fn fill(&self) -> FieldDefault {
        match self {
            Self::Zero => FieldDefault::Zero,
            Self::Incoming => FieldDefault::Incoming,
            Self::Aggregate(record) => record.fill,
            _ => FieldDefault::Unknown,
        }
    }

    pub(in super::super) fn variant(case: StructuralCaseId) -> Self {
        Self::Aggregate(Arc::new(Aggregate {
            tags: vec![case],
            fill: FieldDefault::Unknown,
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
                fill: self.fill(),
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
            Self::Zero | Self::Incoming => self,
            Self::Aggregate(record) => record
                .fields
                .iter()
                .find(|(field, _)| field == key)
                .map(|(_, value)| value)
                .unwrap_or(record.fill.contents()),
            _ => &Self::Unknown,
        }
    }

    /// Shared copies retain one description until the corresponding field
    /// is written. Nesting a record never expands all possible element paths.
    pub(super) fn field_mut(&mut self, key: &Segment) -> &mut Self {
        if !matches!(self, Self::Aggregate(_)) {
            *self = Self::Aggregate(Arc::new(Aggregate {
                tags: Vec::new(),
                fill: self.fill(),
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
                record
                    .fields
                    .push((key.clone(), record.fill.contents().clone()));
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
            (Self::Unknown, _) | (Self::Zero, Self::Zero) | (Self::Incoming, Self::Incoming) => {
                return;
            }
            (Self::Scalar(a), Self::Scalar(b)) if a == b => return,
            (Self::Reference(a), Self::Reference(b)) => {
                let mut home = a.clone();
                borrow(&mut home, b);
                *self = Self::Reference(home);
            }
            (Self::Aggregate(a), Self::Aggregate(b)) if Arc::ptr_eq(a, b) => return,
            (Self::Aggregate(_), Self::Aggregate(_) | Self::Zero | Self::Incoming)
            | (Self::Zero | Self::Incoming, Self::Aggregate(_)) => {
                let fields = |value: &Self| match value {
                    Self::Aggregate(a) => a
                        .fields
                        .iter()
                        .map(|(key, _)| key.clone())
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
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
                    fill: if self.fill() == other.fill() {
                        self.fill()
                    } else {
                        FieldDefault::Unknown
                    },
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

    pub(in super::super) fn wait_failure(
        &self,
        wait: &SpinWait,
        requires: &[Proposition],
    ) -> Option<ProgressFailure> {
        let mut any_blocked = false;
        let mut all_blocked = true;
        let visited = self.visit_scalars(wait.while_value.value.scalar_type(), |value| {
            let failure = wait.while_value.failure(Some(value), wait.update, requires);
            any_blocked |= failure.is_some();
            all_blocked &= failure == Some(ProgressFailure::Deadlock);
        });
        if visited.is_none() {
            return wait.while_value.failure(None, wait.update, requires);
        }
        if all_blocked {
            return Some(ProgressFailure::Deadlock);
        }
        any_blocked.then_some(ProgressFailure::Unproved)
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

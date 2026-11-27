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
    Uniform(FieldDefault),
    Scalar(ScalarFact),
    Alternatives {
        pending_provider: bool,
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
pub(in super::super) enum FieldDefault {
    Zero,
    Incoming,
    PendingProvider,
    PendingOrZero,
    Unknown,
}

impl FieldDefault {
    fn join(self, other: Self) -> Self {
        if self == other {
            return self;
        }
        if matches!(
            self,
            Self::Zero | Self::PendingProvider | Self::PendingOrZero
        ) && matches!(
            other,
            Self::Zero | Self::PendingProvider | Self::PendingOrZero
        ) {
            return Self::PendingOrZero;
        }
        Self::Unknown
    }

    fn contents(self) -> &'static Stored {
        match self {
            Self::Zero => &Stored::ZERO,
            Self::Incoming => &Stored::INCOMING,
            Self::PendingProvider => &Stored::PENDING_PROVIDER,
            Self::PendingOrZero => &Stored::Uniform(Self::PendingOrZero),
            Self::Unknown => &Stored::Unknown,
        }
    }
}

impl Stored {
    pub(in super::super) const ZERO: Self = Self::Uniform(FieldDefault::Zero);
    /// Bytes supplied by an exclusive caller, untouched on every arrival.
    pub(in super::super) const INCOMING: Self = Self::Uniform(FieldDefault::Incoming);
    /// Bytes lent to a provider that will be selected for the target.
    pub(in super::super) const PENDING_PROVIDER: Self =
        Self::Uniform(FieldDefault::PendingProvider);

    pub(in super::super) fn is_zero(&self) -> bool {
        matches!(self, Self::Uniform(FieldDefault::Zero))
    }

    pub(in super::super) fn is_incoming(&self) -> bool {
        matches!(self, Self::Uniform(FieldDefault::Incoming))
    }

    fn fill(&self) -> FieldDefault {
        match self {
            Self::Uniform(fill) => *fill,
            Self::Alternatives {
                includes_zero: true,
                pending_provider: true,
                values,
            } if values.is_empty() => FieldDefault::PendingOrZero,
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
            Self::Uniform(_) => self,
            Self::Aggregate(record) => record
                .fields
                .iter()
                .find(|(field, _)| field == key)
                .map(|(_, value)| value)
                .unwrap_or(record.fill.contents()),
            _ => self.fill().contents(),
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
            (Self::Unknown, _) => return,
            (Self::Uniform(left), Self::Uniform(right)) => {
                *self = left.join(*right).contents().clone();
            }
            (Self::Scalar(a), Self::Scalar(b)) if a == b => return,
            (Self::Reference(a), Self::Reference(b)) => {
                let mut home = a.clone();
                borrow(&mut home, b);
                *self = Self::Reference(home);
            }
            (Self::Aggregate(a), Self::Aggregate(b)) if Arc::ptr_eq(a, b) => return,
            (Self::Aggregate(_), Self::Aggregate(_) | Self::Uniform(_))
            | (Self::Uniform(_), Self::Aggregate(_)) => {
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
                    fill: self.fill().join(other.fill()),
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
                Self::Uniform(
                    FieldDefault::Zero
                    | FieldDefault::PendingProvider
                    | FieldDefault::PendingOrZero,
                )
                | Self::Scalar(_)
                | Self::Alternatives { .. },
                Self::Uniform(
                    FieldDefault::Zero
                    | FieldDefault::PendingProvider
                    | FieldDefault::PendingOrZero,
                )
                | Self::Scalar(_)
                | Self::Alternatives { .. },
            ) => {
                let mut pending_provider = false;
                let mut includes_zero = false;
                let mut values = Vec::new();
                for item in [&*self, other] {
                    match item {
                        Self::Uniform(FieldDefault::Zero) => includes_zero = true,
                        Self::Uniform(FieldDefault::PendingProvider) => pending_provider = true,
                        Self::Uniform(FieldDefault::PendingOrZero) => {
                            pending_provider = true;
                            includes_zero = true;
                        }
                        Self::Scalar(value) => values.push(*value),
                        Self::Alternatives {
                            includes_zero: zero,
                            values: alternatives,
                            pending_provider: pending,
                        } => {
                            includes_zero |= zero;
                            pending_provider |= pending;
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
                    pending_provider,
                    includes_zero,
                    values: unique,
                };
            }
            _ => *self = Self::Unknown,
        }
    }

    /// Apply the instruction's transfer independently to every admitted
    /// resident. A mismatch preserves the actual observed word.
    pub(in super::super::super) fn after_update(&self, update: CellUpdate<ScalarFact>) -> Self {
        let expected = match update {
            CellUpdate::Observe => return self.clone(),
            CellUpdate::Replace(value) => return Self::Scalar(value),
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
        self.map_scalars(expected.ty(), |prior| update.next(prior))
    }

    pub(in super::super) fn map_scalars(
        &self,
        ty: ScalarType,
        mut apply: impl FnMut(ScalarFact) -> Option<ScalarFact>,
    ) -> Self {
        if matches!(self, Self::Uniform(FieldDefault::PendingProvider)) {
            return Self::Uniform(FieldDefault::PendingProvider);
        }
        let mut result = self
            .includes_pending_provider()
            .then_some(Self::Uniform(FieldDefault::PendingProvider));
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

    pub(in super::super) fn includes_pending_provider(&self) -> bool {
        matches!(
            self,
            Self::Uniform(FieldDefault::PendingProvider)
                | Self::Uniform(FieldDefault::PendingOrZero)
                | Self::Alternatives {
                    pending_provider: true,
                    ..
                }
        )
    }

    /// Visit the locally established alternatives, with the unresolved provider alternative
    /// counted separately by the caller.
    pub(in super::super) fn visit_scalars(
        &self,
        ty: ScalarType,
        mut visit: impl FnMut(ScalarFact),
    ) -> Option<()> {
        if matches!(self, Self::Uniform(FieldDefault::PendingOrZero)) {
            visit(ScalarFact::Literal(Scalar::zero(ty)?));
            return Some(());
        }
        if let Self::Alternatives {
            includes_zero,
            values,
            ..
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
            Self::Uniform(FieldDefault::Zero) => Scalar::zero(ty).map(ScalarFact::Literal),
            Self::Scalar(value) if value.ty() == ty => Some(*value),
            Self::Alternatives {
                includes_zero,
                values,
                pending_provider: false,
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

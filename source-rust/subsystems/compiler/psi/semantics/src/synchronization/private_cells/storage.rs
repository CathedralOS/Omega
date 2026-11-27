//! Known contents of activation-local storage. A by-value copy gets a new
//! home. References inside the copy still select the original allocation.

use super::super::facts::Facts;
use crate::entry_values::TerminalScalarValue as Scalar;
use semantic_vocabulary::{PlaceId, ScalarType};
use std::sync::Arc;
use terminal_psi::StructuralPathSegment as Segment;

#[derive(Clone)]
pub(super) struct Location {
    allocation: usize,
    path: Vec<Segment>,
}

#[derive(Clone, Default)]
pub(super) enum Stored {
    Zero,
    Scalar(Scalar),
    Reference(Location),
    Aggregate(Arc<Aggregate>),
    #[default]
    Unknown,
}

#[derive(Clone)]
pub(super) struct Aggregate {
    zeroed: bool,
    fields: Vec<(Segment, Stored)>,
}

impl Stored {
    fn field(&self, key: &Segment) -> &Self {
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
    fn field_mut(&mut self, key: &Segment) -> &mut Self {
        if !matches!(self, Self::Aggregate(_)) {
            *self = Self::Aggregate(Arc::new(Aggregate {
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

    pub(super) fn scalar(&self, ty: ScalarType) -> Option<Scalar> {
        match self {
            Self::Zero => Scalar::zero(ty),
            Self::Scalar(value) if value.scalar_type() == ty => Some(*value),
            _ => None,
        }
    }
}

impl From<Option<Scalar>> for Stored {
    fn from(value: Option<Scalar>) -> Self {
        value.map_or(Self::Unknown, Self::Scalar)
    }
}

#[derive(Default)]
pub(super) struct Storage(Vec<Stored>);

impl Storage {
    pub(super) fn allocate(&mut self, contents: Stored) -> Location {
        let allocation = self.0.len();
        self.0.push(contents);
        Location {
            allocation,
            path: Vec::new(),
        }
    }

    pub(super) fn locate(
        &self,
        places: &Facts<PlaceId, Location>,
        root: PlaceId,
        path: &[Segment],
    ) -> Option<Location> {
        self.project(places.get(&root)?.clone(), path)
    }

    pub(super) fn project(&self, mut location: Location, path: &[Segment]) -> Option<Location> {
        for segment in path {
            match segment {
                Segment::Field(_) | Segment::FixedIndex(_) => location.path.push(segment.clone()),
                Segment::Referent => {
                    let Stored::Reference(referent) = self.get(&location) else {
                        return None;
                    };
                    location = referent.clone();
                }
                _ => return None,
            }
        }
        Some(location)
    }

    pub(super) fn get(&self, location: &Location) -> &Stored {
        let mut value = &self.0[location.allocation];
        for field in &location.path {
            value = value.field(field);
        }
        value
    }

    pub(super) fn set(&mut self, location: &Location, new: Stored) {
        let mut value = &mut self.0[location.allocation];
        for field in &location.path {
            value = value.field_mut(field);
        }
        *value = new;
    }
}

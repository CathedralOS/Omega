use omega_typer::ValueType;
use std::{fmt, num::NonZeroU64};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeId(NonZeroU64);

impl TypeId {
    pub fn get(self) -> u64 {
        self.0.get()
    }

    fn from_index(index: usize) -> Result<Self, TypeError> {
        let id = u64::try_from(index)
            .ok()
            .and_then(|index| index.checked_add(1))
            .and_then(NonZeroU64::new)
            .ok_or(TypeError::TooManyTypes)?;
        Ok(Self(id))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScalarType {
    Boolean,
    Integer { signed: bool, width: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypeShape {
    PrimitiveScalar(ScalarType),
    FixedArray { element: TypeId, length: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeError {
    MissingIntegerWidth { signed: bool },
    ZeroIntegerWidth { signed: bool },
    OpenIntegerWidth,
    OpenArrayExtent,
    NoResult,
    UnsupportedCarrier(String),
    MissingCarrier(TypeShape),
    TooManyTypes,
}

impl fmt::Display for TypeError {
    fn fmt(
        &self,
        output: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::MissingIntegerWidth { signed } => {
                write!(output, "integer carrier {} needs a fixed width", if *signed { "i" } else { "u" })
            }
            Self::ZeroIntegerWidth { signed } => {
                write!(output, "integer carrier {}0 needs a positive width", if *signed { "i" } else { "u" })
            }
            Self::OpenIntegerWidth => output.write_str("integer width is still open after preparation"),
            Self::OpenArrayExtent => output.write_str("array extent is still open after preparation"),
            Self::NoResult => output.write_str("an absent result has no runtime carrier"),
            Self::UnsupportedCarrier(carrier) => write!(output, "no Psi lowering for carrier {carrier}"),
            Self::MissingCarrier(shape) => write!(output, "carrier {shape:?} is absent from the Psi type table"),
            Self::TooManyTypes => output.write_str("too many Psi type declarations"),
        }
    }
}

impl std::error::Error for TypeError {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeTable {
    rows: Vec<TypeShape>,
}

impl TypeTable {
    pub fn from_carriers<'a>(carriers: impl IntoIterator<Item = &'a ValueType>) -> Result<Self, (usize, TypeError)> {
        let mut scalars = Vec::new();
        let mut frontiers: Vec<Vec<(usize, u64)>> = Vec::new();
        let mut lengths = Vec::new();
        for (index, carrier) in carriers.into_iter().enumerate() {
            let scalar = array_dimensions(carrier, &mut lengths).map_err(|error| (index, error))?;
            scalars.push((TypeShape::PrimitiveScalar(scalar), index));
            frontiers.resize_with(frontiers.len().max(lengths.len()), Vec::new);
            for (frontier, length) in frontiers.iter_mut().zip(lengths.drain(..).rev()) {
                frontier.push((index, length));
            }
        }

        let mut table = Self::default();
        let mut resolved = vec![None; scalars.len()];
        table.intern_frontier(&mut scalars, &mut resolved)?;
        for frontier in frontiers {
            scalars.clear();
            scalars.extend(frontier.into_iter().map(|(index, length)| {
                let element = resolved[index].expect("array element has already been interned");
                (TypeShape::FixedArray { element, length }, index)
            }));
            table.intern_frontier(&mut scalars, &mut resolved)?;
        }
        Ok(table)
    }

    pub fn id_for(
        &self,
        carrier: &ValueType,
    ) -> Result<TypeId, TypeError> {
        let mut lengths = Vec::new();
        let scalar = array_dimensions(carrier, &mut lengths)?;
        let mut id = self.id_for_shape(TypeShape::PrimitiveScalar(scalar))?;
        for length in lengths.into_iter().rev() {
            id = self.id_for_shape(TypeShape::FixedArray { element: id, length })?;
        }
        Ok(id)
    }

    pub fn get(
        &self,
        id: TypeId,
    ) -> Option<&TypeShape> {
        self.rows.get(usize::try_from(id.get() - 1).ok()?)
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = (TypeId, &TypeShape)> {
        self.rows.iter().enumerate().map(|(index, shape)| {
            let id = TypeId::from_index(index).expect("type table contains only representable ids");
            (id, shape)
        })
    }

    fn intern_frontier(
        &mut self,
        frontier: &mut [(TypeShape, usize)],
        resolved: &mut [Option<TypeId>],
    ) -> Result<(), (usize, TypeError)> {
        // Every deeper array refers to an element with a greater ID than any
        // shallower array uses. Sorting each depth therefore keeps all rows sorted.
        frontier.sort_unstable();
        for &(shape, input_index) in frontier.iter() {
            let already_interned = self.rows.last() == Some(&shape);
            let row_index = if already_interned { self.rows.len() - 1 } else { self.rows.len() };
            let id = TypeId::from_index(row_index).map_err(|error| (input_index, error))?;
            if !already_interned {
                self.rows.push(shape);
            }
            resolved[input_index] = Some(id);
        }
        Ok(())
    }

    fn id_for_shape(
        &self,
        shape: TypeShape,
    ) -> Result<TypeId, TypeError> {
        let index = self
            .rows
            .binary_search(&shape)
            .map_err(|_| TypeError::MissingCarrier(shape))?;
        TypeId::from_index(index)
    }
}

fn array_dimensions(
    mut carrier: &ValueType,
    lengths: &mut Vec<u64>,
) -> Result<ScalarType, TypeError> {
    while let ValueType::Array(element, length) = carrier {
        lengths.push(*length);
        carrier = element;
    }
    scalar_carrier(carrier)
}

fn scalar_carrier(carrier: &ValueType) -> Result<ScalarType, TypeError> {
    match carrier {
        ValueType::Bool => Ok(ScalarType::Boolean),
        ValueType::Integer { signed, width: Some(0) } => Err(TypeError::ZeroIntegerWidth { signed: *signed }),
        ValueType::Integer { signed, width: Some(width) } => Ok(ScalarType::Integer {
            signed: *signed,
            width: *width,
        }),
        ValueType::Integer { signed, width: None } => Err(TypeError::MissingIntegerWidth { signed: *signed }),
        ValueType::IndexedInteger { .. } => Err(TypeError::OpenIntegerWidth),
        ValueType::IndexedArray(..) => Err(TypeError::OpenArrayExtent),
        ValueType::NoResult => Err(TypeError::NoResult),
        other => Err(TypeError::UnsupportedCarrier(other.to_string())),
    }
}

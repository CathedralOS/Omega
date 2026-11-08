/*
We represent immutable structured values (`BuildTimeValue`) exchanged across the
compile-time argument and result boundaries, together with exact recursive cell
and text-payload retained-size calculations (`retained_cell_count`,
`retained_text_byte_count`). Every value contributes one root cell, while struct
fields, case payloads, and array elements contribute their recursively retained
cells and text bytes.
*/

/*
Compile-time evaluation admission and result elaboration need a representation
that preserves deterministic field order and descriptor labels without coupling
to source program IRs, execution engines, host services, or backend layouts.
Keeping retained-size accounting on `BuildTimeValue` lets us measure result
custody in value cells and payload bytes directly, while keeping terminal work
metering inside the shared interpreter.
*/

/*
We depend only on `numerics`, `layout_plans`, and `compiler_storage`. Field and
variant labels remain pure result descriptors that issue no nominal identity or
host authority; source-bound callers join actual declaration handles before
admitting or executing these values.
*/

#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, compiler_storage::Record)]
pub enum BuildTimeValue {
    Unit,
    LayoutTarget(layout_plans::LayoutTargetFacts), // Compiler-issued opaque context; ordinary source constructors cannot issue it.
    Int(i64),
    Bool(bool),
    Float(f64),
    UndeterminedNaN(numerics::literals::FloatFormat), // Evaluation may transport this meaning, but cannot publish its bits.
    Text(Vec<u8>), // Text/byte content (Omega text is bytes that are only UTF-8 at domain boundaries).
    Struct {
        type_name: String,
        fields: Vec<(String, BuildTimeValue)>,
    },
    Case {
        variant: String,
        payload: Vec<(String, BuildTimeValue)>,
    },
    Array(Vec<BuildTimeValue>),
}

impl BuildTimeValue {
    pub fn retained_cell_count(&self) -> Option<u64> {
        let children = match self {
            Self::Struct { fields, .. }
            | Self::Case {
                payload: fields, ..
            } => fields.iter().try_fold(0u64, |count, (_, value)| {
                count.checked_add(value.retained_cell_count()?)
            })?,
            Self::Array(elements) => elements.iter().try_fold(0u64, |count, value| {
                count.checked_add(value.retained_cell_count()?)
            })?,
            Self::LayoutTarget(_)
            | Self::Unit
            | Self::Int(_)
            | Self::Bool(_)
            | Self::Float(_)
            | Self::UndeterminedNaN(_)
            | Self::Text(_) => 0,
        };
        children.checked_add(1)
    }

    pub fn retained_text_byte_count(&self) -> Option<u64> {
        match self {
            Self::Text(bytes) => u64::try_from(bytes.len()).ok(),
            Self::LayoutTarget(facts) => u64::try_from(facts.profile.len()).ok(),
            Self::Struct { fields, .. }
            | Self::Case {
                payload: fields, ..
            } => fields.iter().try_fold(0u64, |count, (_, value)| {
                count.checked_add(value.retained_text_byte_count()?)
            }),
            Self::Array(elements) => elements.iter().try_fold(0u64, |count, value| {
                count.checked_add(value.retained_text_byte_count()?)
            }),
            Self::Unit
            | Self::Int(_)
            | Self::Bool(_)
            | Self::Float(_)
            | Self::UndeterminedNaN(_) => Some(0),
        }
    }
}

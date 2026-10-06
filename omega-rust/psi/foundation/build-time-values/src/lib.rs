#![forbid(unsafe_code)]

//! Immutable values exchanged by compile-time admission and result elaboration.
//!
//! This owner contains data and recursive retained-size calculations only. It
//! depends on numeric vocabulary, never source program IRs, execution engines,
//! host services or scheduler/accounting identities. Field and variant labels
//! are result descriptors; they issue no nominal identity or host authority.
//! Source-bound callers join actual declaration handles before admitting or
//! executing these values. Converting interpreter storage is an engine concern.
//!
//! Retained-size functions count this value's cells and text payload, not work
//! performed by any particular engine. Terminal fuel remains independently
//! measured by the shared interpreter; these counts do not substitute for it.

/// A structured value crossing the compile-time argument/result boundary in
/// either direction: compiler-built arguments in, the machine's terminal
/// value out. Field order is preserved for deterministic reporting; struct
/// fields retain descriptor labels without depending on backend layout.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildTimeValue {
    Unit,
    Int(i64),
    Bool(bool),
    Float(f64),
    /// Evaluation may transport this meaning, but cannot publish its bits.
    UndeterminedNaN(numerics::literals::FloatFormat),
    /// Text/byte content (Omega text is bytes that are only Utf8 at domain
    /// boundaries).
    Text(Vec<u8>),
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
    /// Canonical retained result-cell count. Every value
    /// contributes one root cell; aggregate fields, case payloads, and array
    /// elements contribute their recursively retained cells as well. Text is
    /// one value cell regardless of byte length; its exact retained payload is
    /// measured separately.
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
            Self::Unit
            | Self::Int(_)
            | Self::Bool(_)
            | Self::Float(_)
            | Self::UndeterminedNaN(_)
            | Self::Text(_) => 0,
        };
        children.checked_add(1)
    }

    /// Exact Text payload bytes retained by this result. Structural names and
    /// Rust allocation overhead are compiler metadata/implementation details,
    /// not fabricated semantic byte size.
    pub fn retained_text_byte_count(&self) -> Option<u64> {
        match self {
            Self::Text(bytes) => u64::try_from(bytes.len()).ok(),
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

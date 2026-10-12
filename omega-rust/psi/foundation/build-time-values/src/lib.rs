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
    /// Compare retained value representation, independently of language-level
    /// floating equality. Execution records must distinguish signed zero and
    /// preserve NaN payload bits, including inside aggregates.
    pub fn has_same_representation(&self, other: &Self) -> bool {
        let fields_equal = |left: &[(String, Self)], right: &[(String, Self)]| {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|((left_name, left), (right_name, right))| {
                        left_name == right_name && left.has_same_representation(right)
                    })
        };
        match (self, other) {
            (Self::Unit, Self::Unit) => true,
            (Self::Int(left), Self::Int(right)) => left == right,
            (Self::Bool(left), Self::Bool(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => left.to_bits() == right.to_bits(),
            (Self::UndeterminedNaN(left), Self::UndeterminedNaN(right)) => left == right,
            (Self::Text(left), Self::Text(right)) => left == right,
            (
                Self::Struct {
                    type_name: left_name,
                    fields: left,
                },
                Self::Struct {
                    type_name: right_name,
                    fields: right,
                },
            ) => left_name == right_name && fields_equal(left, right),
            (
                Self::Case {
                    variant: left_name,
                    payload: left,
                },
                Self::Case {
                    variant: right_name,
                    payload: right,
                },
            ) => left_name == right_name && fields_equal(left, right),
            (Self::Array(left), Self::Array(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| left.has_same_representation(right))
            }
            _ => false,
        }
    }

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

#[cfg(test)]
mod tests {
    use super::BuildTimeValue;

    #[test]
    fn retained_nested_floating_content_preserves_zero_sign_and_nan_payload() {
        let retained = |bits| BuildTimeValue::Struct {
            type_name: "Snapshot".into(),
            fields: vec![(
                "samples".into(),
                BuildTimeValue::Array(vec![BuildTimeValue::Float(f64::from_bits(bits))]),
            )],
        };
        let nan = retained(0x7ff8_0000_0000_0042);
        assert!(nan.has_same_representation(&nan.clone()));
        assert!(!nan.has_same_representation(&retained(0x7ff8_0000_0000_0043)));
        assert!(!retained(0).has_same_representation(&retained(1_u64 << 63)));
        // Ordinary floating comparison still has Omega's numeric semantics.
        assert_eq!(BuildTimeValue::Float(0.0), BuildTimeValue::Float(-0.0));
        assert_ne!(nan, nan.clone());
    }
}

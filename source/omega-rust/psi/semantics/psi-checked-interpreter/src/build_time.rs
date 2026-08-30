//! The BUILD-TIME EVALUATION bridge (design_briefs/build_time_evaluation.md):
//! the compiler invokes an effect-free machine through the reference
//! interpreter with compiler-built arguments and reads back a STRUCTURED
//! value. No keyword marks such machines -- the POSITION makes the evaluation
//! build-time, decision 12's transitive effect surface is the legality gate
//! (owned by the caller, as with `evaluate_const_machine`), and the trait
//! signature carries the stability contract. First heavyweight client: the
//! Layout machinery -- the compiler builds a `Schema` value, calls a policy's
//! `plan()`, and reads back a `Plan` (programmable_layouts.md).

use std::collections::BTreeMap;

use crate::value::{Cell, Value};

/// A structured value crossing the compiler <-> interpreter boundary in
/// EITHER direction: compiler-built arguments in, the machine's terminal
/// value out. Field order is preserved for deterministic reporting; struct
/// fields are name-addressed (matching the interpreter's own representation,
/// which never depends on backend layout).
#[derive(Debug, Clone, PartialEq)]
pub enum BuildTimeValue {
    Unit,
    Int(i64),
    Bool(bool),
    Float(f64),
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
    /// Canonical result-cell count for evaluator usage schema v1. Every value
    /// contributes one root cell; aggregate fields, case payloads, and array
    /// elements contribute their recursively retained cells as well. Text is
    /// one value cell regardless of byte length; its exact retained payload is
    /// measured separately.
    pub(crate) fn retained_cell_count(&self) -> Option<u64> {
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
            Self::Unit | Self::Int(_) | Self::Bool(_) | Self::Float(_) | Self::Text(_) => 0,
        };
        children.checked_add(1)
    }

    /// Exact Text payload bytes retained by this result. Structural names and
    /// Rust allocation overhead are compiler metadata/implementation details,
    /// not fabricated semantic byte size.
    pub(crate) fn retained_text_byte_count(&self) -> Option<u64> {
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
            Self::Unit | Self::Int(_) | Self::Bool(_) | Self::Float(_) => Some(0),
        }
    }

    /// Materialize into an interpreter value tree using the evaluator's
    /// allocation authority. Build-time arguments never alias compiler state.
    pub(crate) fn into_value_with<E>(
        self,
        allocate: &impl Fn(Value) -> Result<Cell, E>,
        allocate_text: &impl Fn(Vec<u8>) -> Result<Value, E>,
    ) -> Result<Value, E> {
        match self {
            BuildTimeValue::Unit => Ok(Value::Unit),
            BuildTimeValue::Int(value) => Ok(Value::Int(value)),
            BuildTimeValue::Bool(value) => Ok(Value::Bool(value)),
            BuildTimeValue::Float(value) => Ok(Value::Float(value)),
            BuildTimeValue::Text(bytes) => allocate_text(bytes),
            BuildTimeValue::Struct { type_name, fields } => {
                let mut cells: BTreeMap<String, Cell> = BTreeMap::new();
                for (name, value) in fields {
                    let value = value.into_value_with(allocate, allocate_text)?;
                    cells.insert(name, allocate(value)?);
                }
                Ok(Value::Struct {
                    type_symbol: psi_symbols::SymbolHandle::invalid(),
                    type_name,
                    fields: cells,
                })
            }
            BuildTimeValue::Case { variant, payload } => Ok(Value::Enum {
                // The build-time boundary carries no type identity (same as
                // the Struct arm above); tag-ordinal resolution falls back to
                // the name-global scan for these values.
                type_symbol: psi_symbols::SymbolHandle::invalid(),
                variant_name: variant,
                payload: payload
                    .into_iter()
                    .map(|(name, value)| {
                        let value = value.into_value_with(allocate, allocate_text)?;
                        Ok((name, allocate(value)?))
                    })
                    .collect::<Result<_, E>>()?,
            }),
            BuildTimeValue::Array(elements) => Ok(Value::Array(
                elements
                    .into_iter()
                    .map(|element| {
                        let value = element.into_value_with(allocate, allocate_text)?;
                        allocate(value)
                    })
                    .collect::<Result<_, E>>()?,
            )),
        }
    }

    /// Deep-read an interpreter value back out. References deref (a build-time
    /// result is a VALUE; nothing it points at survives the evaluation).
    pub(crate) fn from_value(value: &Value) -> BuildTimeValue {
        match value {
            Value::Unit => BuildTimeValue::Unit,
            Value::Int(value) => BuildTimeValue::Int(*value),
            Value::Bool(value) => BuildTimeValue::Bool(*value),
            Value::Float(value) => BuildTimeValue::Float(*value),
            Value::Str(bytes) => BuildTimeValue::Text(bytes.borrow().clone()),
            Value::Struct {
                type_name, fields, ..
            } => BuildTimeValue::Struct {
                type_name: type_name.clone(),
                fields: fields
                    .iter()
                    .map(|(name, cell)| (name.clone(), Self::from_value(&cell.borrow())))
                    .collect(),
            },
            Value::Enum {
                variant_name,
                payload,
                ..
            } => BuildTimeValue::Case {
                variant: variant_name.clone(),
                payload: payload
                    .iter()
                    .map(|(name, cell)| (name.clone(), Self::from_value(&cell.borrow())))
                    .collect(),
            },
            Value::Array(elements) => BuildTimeValue::Array(
                elements
                    .iter()
                    .map(|cell| Self::from_value(&cell.borrow()))
                    .collect(),
            ),
            Value::Ref(cell) => Self::from_value(&cell.borrow()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BuildTimeValue;

    #[test]
    fn result_cell_count_is_recursive_and_text_is_one_cell() {
        let value = BuildTimeValue::Struct {
            type_name: "Envelope".to_owned(),
            fields: vec![
                (
                    "payload".to_owned(),
                    BuildTimeValue::Case {
                        variant: "Ready".to_owned(),
                        payload: vec![(
                            "items".to_owned(),
                            BuildTimeValue::Array(vec![
                                BuildTimeValue::Int(1),
                                BuildTimeValue::Text(vec![0; 4096]),
                            ]),
                        )],
                    },
                ),
                ("valid".to_owned(), BuildTimeValue::Bool(true)),
            ],
        };

        // Struct + case + array + two elements + bool.
        assert_eq!(value.retained_cell_count(), Some(6));
        assert_eq!(value.retained_text_byte_count(), Some(4096));
    }
}

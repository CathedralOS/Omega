//! The BUILD-TIME EVALUATION bridge (design_briefs/build_time_evaluation.md):
//! the compiler invokes an effect-free machine through the reference
//! interpreter with compiler-built arguments and reads back a STRUCTURED
//! value. No keyword marks such machines -- the POSITION makes the evaluation
//! build-time, decision 12's transitive effect surface is the legality gate
//! (owned by the caller, as with `evaluate_const_machine`), and the trait
//! signature carries the stability contract. First heavyweight client: the
//! Layout machinery -- the compiler builds a `Schema` value, calls a policy's
//! `plan()`, and reads back a `Plan` (programmable_layouts.md).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

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
    /// Materialize into an interpreter value tree (fresh cells throughout --
    /// build-time arguments never alias compiler state).
    pub(crate) fn into_value(self) -> Value {
        match self {
            BuildTimeValue::Unit => Value::Unit,
            BuildTimeValue::Int(value) => Value::Int(value),
            BuildTimeValue::Bool(value) => Value::Bool(value),
            BuildTimeValue::Float(value) => Value::Float(value),
            BuildTimeValue::Text(bytes) => Value::Str(Rc::new(RefCell::new(bytes))),
            BuildTimeValue::Struct { type_name, fields } => {
                let mut cells: BTreeMap<String, Cell> = BTreeMap::new();
                for (name, value) in fields {
                    cells.insert(name, value.into_value().cell());
                }
                Value::Struct {
                    type_symbol: omega_core::symbols::SymbolHandle::invalid(),
                    type_name,
                    fields: cells,
                }
            }
            BuildTimeValue::Case { variant, payload } => Value::Enum {
                variant_name: variant,
                payload: payload
                    .into_iter()
                    .map(|(name, value)| (name, value.into_value().cell()))
                    .collect(),
            },
            BuildTimeValue::Array(elements) => Value::Array(
                elements
                    .into_iter()
                    .map(|element| element.into_value().cell())
                    .collect(),
            ),
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

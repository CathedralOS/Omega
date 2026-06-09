use omega_core::symbols::SymbolHandle;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// A storage cell. The interpreter models every place (local, field, array element,
/// machine instance) as an `Rc<RefCell<Value>>`. Aliasing -- the whole reason this
/// oracle exists -- is correct BY CONSTRUCTION: a `&mut x` argument is a `Value::Ref`
/// that holds a CLONE of the same `Rc`, so a write through the reference mutates the
/// original cell. This is exactly the property the native backend gets wrong.
pub type Cell = Rc<RefCell<Value>>;

/// A semantic runtime value. Width/signedness of integers is tracked separately by the
/// evaluator from the declared type; the value itself stores an `i64` payload (skeleton:
/// full i64 semantics, refined per-width later).
#[derive(Debug, Clone)]
pub enum Value {
    Unit,
    Int(i64),
    Bool(bool),
    Float(f64),
    Str(Rc<RefCell<String>>),
    /// A struct / data record / machine instance. Fields are addressed by name so the
    /// interpreter can resolve `self.field` without depending on backend layout. Each
    /// field is its own cell (so `&mut self.field` aliases correctly).
    Struct {
        type_symbol: SymbolHandle,
        type_name: String,
        fields: BTreeMap<String, Cell>,
    },
    /// An enum value identified by its variant name (Omega enums are unit variants).
    Enum {
        variant_name: String,
    },
    /// A mutable reference: holds the SAME cell as the place it points at.
    Ref(Cell),
}

impl Value {
    pub fn cell(self) -> Cell {
        Rc::new(RefCell::new(self))
    }

    pub fn str(value: impl Into<String>) -> Value {
        Value::Str(Rc::new(RefCell::new(value.into())))
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(value) => Some(*value),
            Value::Bool(value) => Some(*value as i64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(*value),
            Value::Int(value) => Some(*value != 0),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(value) => Some(*value),
            Value::Int(value) => Some(*value as f64),
            _ => None,
        }
    }

    /// Follow a `Ref` to the underlying cell, returning a clone of the same `Rc` so the
    /// aliasing is preserved. Non-references return `None`.
    pub fn as_ref_cell(&self) -> Option<Cell> {
        match self {
            Value::Ref(cell) => Some(Rc::clone(cell)),
            _ => None,
        }
    }
}

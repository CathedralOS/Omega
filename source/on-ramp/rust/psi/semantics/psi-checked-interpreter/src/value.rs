use psi_symbols::SymbolHandle;
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
    /// Text: a `&[u8]` view, a string literal, OR an owned `[u8; N]` carrier. Stored as raw
    /// BYTES (not a Rust `String`) because Omega text is `&[u8]` -- bytes that need only be
    /// valid Utf8 at domain boundaries, not at every intermediate step. Bytes (vs a UTF-8
    /// String) let a carrier be byte-indexed and byte-WRITTEN (`out[i] = ch`) directly. The
    /// runtime length is the vec length; a carrier's static capacity `N` is a compile-time
    /// bound the native side enforces and the interpreter does not need to track.
    Str(Rc<RefCell<Vec<u8>>>),
    /// A struct / data record / machine instance. Fields are addressed by name so the
    /// interpreter can resolve `self.field` without depending on backend layout. Each
    /// field is its own cell (so `&mut self.field` aliases correctly).
    Struct {
        type_symbol: SymbolHandle,
        type_name: String,
        fields: BTreeMap<String, Cell>,
    },
    /// A case (enum) value, identified by its case name -- the TAG -- optionally
    /// carrying NAMED payload field cells (`case Say(text: String)` constructs
    /// `Enum { "Say", [("text", cell)] }`). Payload-less cases and bare case
    /// references (`Command::Quit`, including those used as tag-compare operands)
    /// have an empty payload. Equality between enum values compares the TAG only,
    /// matching the native backend's constant tag compare.
    ///
    /// `type_symbol` names the DECLARING data type so tag-ORDINAL resolution
    /// (the value-position `match` desugar's tag arithmetic) is type-local --
    /// same-name variants at different ordinals across enums must not
    /// cross-resolve. INVALID when the provenance cannot name a type (the
    /// build-time value boundary); resolution then falls back to the
    /// name-global scan.
    Enum {
        type_symbol: SymbolHandle,
        variant_name: String,
        payload: Vec<(String, Cell)>,
    },
    /// A fixed array or a slice view. Both are an ordered list of element CELLS; a slice
    /// shares the array's element `Rc`s (so writes through the slice alias the array). The
    /// interpreter does not distinguish their static type -- indexing and `.len` work the
    /// same -- which is enough for the slice/array canaries.
    Array(Vec<Cell>),
    /// A mutable reference: holds the SAME cell as the place it points at.
    Ref(Cell),
}

impl Value {
    pub fn cell(self) -> Cell {
        Rc::new(RefCell::new(self))
    }

    pub fn str(value: impl Into<String>) -> Value {
        Value::Str(Rc::new(RefCell::new(value.into().into_bytes())))
    }

    /// Construct text directly from raw bytes (carrier byte content that need not be valid
    /// UTF-8 mid-computation).
    pub fn bytes(value: impl Into<Vec<u8>>) -> Value {
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

    /// Clone with VALUE SEMANTICS: a `Struct`/`Enum`/`Array` gets FRESH element cells (a deep
    /// copy), so mutating a field of the copy does NOT alias the original. A `Ref` is preserved
    /// (it shares the referent cell -- a reference aliases by design), and scalars/strings use
    /// the ordinary derived clone. The derived `Clone` is SHALLOW (it `Rc::clone`s the field
    /// cells, sharing them), which is correct for `&mut` aliasing but WRONG for a value-semantic
    /// copy like `self.f = self.arr[1]; self.f.x = 50` -- that must not touch `arr[1]`. Used by
    /// the evaluator at every value-semantic copy site (assignment, `let` initializer).
    pub fn deep_clone(&self) -> Value {
        match self {
            Value::Struct {
                type_symbol,
                type_name,
                fields,
            } => Value::Struct {
                type_symbol: *type_symbol,
                type_name: type_name.clone(),
                fields: fields
                    .iter()
                    .map(|(name, cell)| (name.clone(), cell.borrow().deep_clone().cell()))
                    .collect(),
            },
            Value::Enum {
                type_symbol,
                variant_name,
                payload,
            } => Value::Enum {
                type_symbol: *type_symbol,
                variant_name: variant_name.clone(),
                payload: payload
                    .iter()
                    .map(|(name, cell)| (name.clone(), cell.borrow().deep_clone().cell()))
                    .collect(),
            },
            Value::Array(elements) => Value::Array(
                elements
                    .iter()
                    .map(|cell| cell.borrow().deep_clone().cell())
                    .collect(),
            ),
            // Scalars keep their value; a `Str` keeps its shared buffer (status quo -- not the
            // subject of this fix); a `Ref` MUST keep sharing the referent cell.
            Value::Unit
            | Value::Int(_)
            | Value::Bool(_)
            | Value::Float(_)
            | Value::Str(_)
            | Value::Ref(_) => self.clone(),
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

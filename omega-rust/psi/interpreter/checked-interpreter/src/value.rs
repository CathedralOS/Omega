use crate::build_evaluation_sponsor::{
    BuildEvaluationLiveCellLease, BuildEvaluationLiveTextByteLease, BuildEvaluationSponsor,
};
use std::cell::{Cell as CounterCell, Ref, RefCell};
use std::collections::BTreeMap;
use std::ops::{Deref, Range};
use std::rc::Rc;
use symbols::SymbolHandle;

/// A storage cell. Every alias clones the same allocation, so a write through a
/// reference mutates the original cell. A sponsored cell also owns one lifetime
/// lease; cloning does not double-charge it and the final alias retires it.
#[derive(Clone)]
pub struct Cell(Rc<CellAllocation>);

#[derive(Debug)]
struct CellAllocation {
    value: RefCell<Value>,
    _lease: Option<LiveCellLease>,
}

impl std::fmt::Debug for Cell {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("Cell").field(&self.0.value).finish()
    }
}

impl Deref for Cell {
    type Target = RefCell<Value>;

    fn deref(&self) -> &Self::Target {
        &self.0.value
    }
}

impl Cell {
    fn new(value: Value, lease: Option<LiveCellLease>) -> Self {
        Self(Rc::new(CellAllocation {
            value: RefCell::new(value),
            _lease: lease,
        }))
    }

    pub(crate) fn ptr_eq(left: &Self, right: &Self) -> bool {
        Rc::ptr_eq(&left.0, &right.0)
    }
}

#[derive(Debug, Default)]
struct CellMeterAccount {
    live: CounterCell<u64>,
    peak: CounterCell<u64>,
}

/// Per-evaluator exact cell-lifetime meter. The optional sponsor adds one
/// closure-wide reservation before each allocation; neither count is a byte or
/// resident-memory estimate.
#[derive(Debug, Clone)]
pub(crate) struct CellMeter {
    account: Rc<CellMeterAccount>,
    sponsor: Option<BuildEvaluationSponsor>,
}

impl CellMeter {
    pub(crate) fn new(sponsor: Option<BuildEvaluationSponsor>) -> Self {
        Self {
            account: Rc::new(CellMeterAccount::default()),
            sponsor,
        }
    }

    pub(crate) fn allocate(&self, value: Value) -> Result<Cell, String> {
        let sponsor_lease = self
            .sponsor
            .as_ref()
            .map(BuildEvaluationSponsor::reserve_live_cell)
            .transpose()?;
        let live = self
            .account
            .live
            .get()
            .checked_add(1)
            .ok_or_else(|| "evaluator live-cell accounting overflowed".to_owned())?;
        self.account.live.set(live);
        self.account.peak.set(self.account.peak.get().max(live));
        Ok(Cell::new(
            value,
            Some(LiveCellLease {
                meter: self.clone(),
                _sponsor_lease: sponsor_lease,
            }),
        ))
    }

    pub(crate) fn peak(&self) -> u64 {
        self.account.peak.get()
    }
}

#[derive(Debug)]
struct LiveCellLease {
    meter: CellMeter,
    _sponsor_lease: Option<BuildEvaluationLiveCellLease>,
}

impl Drop for LiveCellLease {
    fn drop(&mut self) {
        let live = self.meter.account.live.get();
        debug_assert!(live > 0);
        self.meter.account.live.set(live.saturating_sub(1));
    }
}

/// One shared interpreter Text backing buffer. Aliases share both bytes and
/// one exact logical-byte lifetime lease; only the final alias releases it.
/// Bounded views retain their extent and prevent backing length changes.
#[derive(Clone)]
pub struct TextBuffer {
    allocation: Rc<TextAllocation>,
    window: Option<Rc<TextWindowLease>>,
}

#[derive(Debug)]
struct TextAllocation {
    bytes: RefCell<Vec<u8>>,
    lease: RefCell<Option<LiveTextByteLease>>,
    live_windows: CounterCell<usize>,
}

/// Clones share one window lease; nested subslices register their own lease.
/// Keeping the allocation alive here makes release independent of field order.
struct TextWindowLease {
    allocation: Rc<TextAllocation>,
    extent: Range<usize>,
}

impl Drop for TextWindowLease {
    fn drop(&mut self) {
        let live_windows = self.allocation.live_windows.get();
        debug_assert!(live_windows > 0);
        self.allocation.live_windows.set(live_windows - 1);
    }
}

impl std::fmt::Debug for TextBuffer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = formatter.debug_tuple("TextBuffer");
        match self.allocation.bytes.try_borrow() {
            Ok(bytes) => debug.field(&&bytes[self.extent(bytes.len())]).finish(),
            Err(_) => debug.field(&"<borrowed>").finish(),
        }
    }
}

impl TextBuffer {
    fn new(bytes: Vec<u8>, lease: Option<LiveTextByteLease>) -> Self {
        Self {
            allocation: Rc::new(TextAllocation {
                bytes: RefCell::new(bytes),
                lease: RefCell::new(lease),
                live_windows: CounterCell::new(0),
            }),
            window: None,
        }
    }

    fn extent(&self, backing_length: usize) -> Range<usize> {
        match &self.window {
            Some(window) => window.extent.clone(),
            None => 0..backing_length,
        }
    }

    pub(crate) fn borrow(&self) -> Ref<'_, [u8]> {
        Ref::map(self.allocation.bytes.borrow(), |bytes| {
            &bytes[self.extent(bytes.len())]
        })
    }

    /// Share the exclusive byte range relative to this view. Empty ranges also
    /// retain the allocation and prevent resizing until their final alias drops.
    pub(crate) fn subslice(&self, start: usize, end: usize) -> Result<Self, String> {
        let extent = self.extent(self.allocation.bytes.borrow().len());
        let length = extent.len();
        if start > end || end > length {
            return Err(format!(
                "Text subslice range {start}..{end} is outside view length {length}"
            ));
        }
        let live_windows = self
            .allocation
            .live_windows
            .get()
            .checked_add(1)
            .ok_or_else(|| "Text live-window count overflowed".to_owned())?;
        // Both offsets are bounded by the existing extent, so adding its start
        // cannot exceed the original backing length.
        let window = Rc::new(TextWindowLease {
            allocation: self.allocation.clone(),
            extent: extent.start + start..extent.start + end,
        });
        self.allocation.live_windows.set(live_windows);
        Ok(Self {
            allocation: self.allocation.clone(),
            window: Some(window),
        })
    }

    pub(crate) fn write_byte(&self, byte_index: usize, byte: u8) -> Result<(), usize> {
        let mut backing = self.allocation.bytes.borrow_mut();
        let extent = self.extent(backing.len());
        let length = extent.len();
        let Some(slot) = backing[extent].get_mut(byte_index) else {
            return Err(length);
        };
        *slot = byte;
        Ok(())
    }

    pub(crate) fn write_prefix(&self, bytes: &[u8]) -> Result<(), String> {
        let mut backing = self.allocation.bytes.borrow_mut();
        let extent = self.extent(backing.len());
        let length = extent.len();
        let Some(prefix) = backing[extent].get_mut(..bytes.len()) else {
            return Err(format!(
                "Text prefix length {} exceeds view length {length}",
                bytes.len()
            ));
        };
        prefix.copy_from_slice(bytes);
        Ok(())
    }

    /// Replace shared backing bytes while preserving aliases and resizing the
    /// exact logical-byte lease before any growth becomes interpreter state.
    /// A bounded view requires equal-length replacement within its extent;
    /// full-buffer aliases cannot resize while any bounded view remains live.
    pub(crate) fn replace(&self, bytes: Vec<u8>) -> Result<(), String> {
        // Acquire mutation custody before changing either meter. A conflicting
        // internal borrow may panic, but cannot leave accounting advanced.
        let mut backing = self.allocation.bytes.borrow_mut();
        let extent = self.extent(backing.len());
        if self.allocation.live_windows.get() > 0 {
            if bytes.len() != extent.len() {
                return Err(format!(
                    "cannot replace Text view length {} with {} bytes while bounded views are live",
                    extent.len(),
                    bytes.len()
                ));
            }
            backing[extent].copy_from_slice(&bytes);
            return Ok(());
        }
        let old_len = u64::try_from(backing.len())
            .map_err(|_| "evaluator live-Text-byte count overflowed".to_owned())?;
        let new_len = u64::try_from(bytes.len())
            .map_err(|_| "evaluator live-Text-byte count overflowed".to_owned())?;
        let mut lease = self.allocation.lease.borrow_mut();
        if let Some(lease) = lease.as_mut()
            && new_len > old_len
        {
            lease.grow(new_len - old_len)?;
        }
        *backing = bytes;
        if let Some(lease) = lease.as_mut()
            && old_len > new_len
        {
            lease.shrink(old_len - new_len);
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct TextByteMeterAccount {
    live: CounterCell<u64>,
    peak: CounterCell<u64>,
}

/// Exact logical-byte meter for interpreter-owned Text backing buffers. This
/// deliberately excludes Vec capacity and every non-Text temporary buffer.
#[derive(Debug, Clone)]
pub(crate) struct TextByteMeter {
    account: Rc<TextByteMeterAccount>,
    sponsor: Option<BuildEvaluationSponsor>,
}

impl TextByteMeter {
    pub(crate) fn new(sponsor: Option<BuildEvaluationSponsor>) -> Self {
        Self {
            account: Rc::new(TextByteMeterAccount::default()),
            sponsor,
        }
    }

    pub(crate) fn allocate(&self, bytes: Vec<u8>) -> Result<Value, String> {
        let byte_count = u64::try_from(bytes.len())
            .map_err(|_| "evaluator live-Text-byte count overflowed".to_owned())?;
        let sponsor_lease = self
            .sponsor
            .as_ref()
            .map(|sponsor| sponsor.reserve_live_text_bytes(byte_count))
            .transpose()?;
        let live = self
            .account
            .live
            .get()
            .checked_add(byte_count)
            .ok_or_else(|| "evaluator live-Text-byte accounting overflowed".to_owned())?;
        self.account.live.set(live);
        self.account.peak.set(self.account.peak.get().max(live));
        Ok(Value::Str(TextBuffer::new(
            bytes,
            Some(LiveTextByteLease {
                meter: self.clone(),
                bytes: byte_count,
                _sponsor_lease: sponsor_lease,
            }),
        )))
    }

    pub(crate) fn peak(&self) -> u64 {
        self.account.peak.get()
    }
}

#[derive(Debug)]
struct LiveTextByteLease {
    meter: TextByteMeter,
    bytes: u64,
    _sponsor_lease: Option<BuildEvaluationLiveTextByteLease>,
}

impl Drop for LiveTextByteLease {
    fn drop(&mut self) {
        let live = self.meter.account.live.get();
        debug_assert!(live >= self.bytes);
        self.meter.account.live.set(live.saturating_sub(self.bytes));
    }
}

impl LiveTextByteLease {
    fn grow(&mut self, bytes: u64) -> Result<(), String> {
        let live = self
            .meter
            .account
            .live
            .get()
            .checked_add(bytes)
            .ok_or_else(|| "evaluator live-Text-byte accounting overflowed".to_owned())?;
        if let Some(sponsor_lease) = self._sponsor_lease.as_mut() {
            sponsor_lease.grow(bytes)?;
        }
        self.meter.account.live.set(live);
        self.meter
            .account
            .peak
            .set(self.meter.account.peak.get().max(live));
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .expect("local Text meter rejected byte-count overflow");
        Ok(())
    }

    fn shrink(&mut self, bytes: u64) {
        debug_assert!(self.bytes >= bytes);
        let live = self.meter.account.live.get();
        debug_assert!(live >= bytes);
        self.bytes = self.bytes.saturating_sub(bytes);
        self.meter.account.live.set(live.saturating_sub(bytes));
        if let Some(sponsor_lease) = self._sponsor_lease.as_mut() {
            sponsor_lease.shrink(bytes);
        }
    }
}

/// A declared name a value carries: its data type, a field, or a case. Values
/// copy and clone often (every array default and deep copy), and a shared name
/// makes each copy a count bump rather than a fresh string.
pub type ValueName = Rc<str>;

/// A semantic runtime value. Width/signedness of integers is tracked separately by the
/// evaluator from the declared type; the value itself stores an `i64` payload (skeleton:
/// full i64 semantics, refined per-width later).
#[derive(Debug, Clone)]
pub enum Value {
    Unit,
    Int(i64),
    Bool(bool),
    Float(f64),
    /// A semantic NaN has a landed format but no observable representation.
    UndeterminedNaN(numerics::literals::FloatFormat),
    /// Text: a `&[u8]` view, a string literal, OR an owned `[u8; N]` carrier. Stored as raw
    /// BYTES (not a Rust `String`) because Omega text is `&[u8]` -- bytes that need only be
    /// valid Utf8 at domain boundaries, not at every intermediate step. Bytes (vs a UTF-8
    /// String) let a carrier be byte-indexed and byte-WRITTEN (`out[i] = ch`) directly. The
    /// runtime length is the visible buffer extent; subslices share bounded windows of
    /// the original bytes. A carrier's static capacity `N` remains a compile-time bound.
    Str(TextBuffer),
    /// A struct / data record / machine instance. Fields are addressed by name so the
    /// interpreter can resolve `self.field` without depending on backend layout. Each
    /// field is its own cell (so `&mut self.field` aliases correctly).
    Struct {
        type_symbol: SymbolHandle,
        type_name: ValueName,
        fields: BTreeMap<ValueName, Cell>,
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
        variant_name: ValueName,
        payload: Vec<(ValueName, Cell)>,
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

    pub(crate) fn is_float(&self) -> bool {
        matches!(self, Self::Float(_) | Self::UndeterminedNaN(_))
    }

    /// Semantic observations do not require a NaN payload. Concrete projections
    /// continue to use `as_float`, which deliberately declines undetermined NaN.
    pub(crate) fn float_meaning(&self) -> Option<numerics::float_semantics::FloatMeaning> {
        use numerics::float_semantics::FloatMeaning;
        match self {
            Self::UndeterminedNaN(_) => Some(FloatMeaning::NaN),
            _ => self.as_float().map(FloatMeaning::from_f64),
        }
    }

    /// Clone with VALUE SEMANTICS: a `Struct`/`Enum`/`Array` gets FRESH element cells (a deep
    /// copy), so mutating a field of the copy does NOT alias the original. A `Ref` is preserved
    /// (it shares the referent cell -- a reference aliases by design), and scalars/strings use
    /// the ordinary derived clone. The derived `Clone` is SHALLOW (it `Rc::clone`s the field
    /// cells, sharing them), which is correct for `&mut` aliasing but WRONG for a value-semantic
    /// copy like `self.f = self.arr[1]; self.f.x = 50` -- that must not touch `arr[1]`. Used by
    /// the evaluator at every value-semantic copy site (assignment, `let` initializer).
    pub(crate) fn deep_clone_with<E>(
        &self,
        allocate: &impl Fn(Value) -> Result<Cell, E>,
    ) -> Result<Value, E> {
        match self {
            Value::Struct {
                type_symbol,
                type_name,
                fields,
            } => Ok(Value::Struct {
                type_symbol: *type_symbol,
                type_name: type_name.clone(),
                fields: fields
                    .iter()
                    .map(|(name, cell)| {
                        let value = cell.borrow().deep_clone_with(allocate)?;
                        Ok((name.clone(), allocate(value)?))
                    })
                    .collect::<Result<_, E>>()?,
            }),
            Value::Enum {
                type_symbol,
                variant_name,
                payload,
            } => Ok(Value::Enum {
                type_symbol: *type_symbol,
                variant_name: variant_name.clone(),
                payload: payload
                    .iter()
                    .map(|(name, cell)| {
                        let value = cell.borrow().deep_clone_with(allocate)?;
                        Ok((name.clone(), allocate(value)?))
                    })
                    .collect::<Result<_, E>>()?,
            }),
            Value::Array(elements) => Ok(Value::Array(
                elements
                    .iter()
                    .map(|cell| {
                        let value = cell.borrow().deep_clone_with(allocate)?;
                        allocate(value)
                    })
                    .collect::<Result<_, E>>()?,
            )),
            // Scalars keep their value; a `Str` keeps its shared buffer (status quo -- not the
            // subject of this fix); a `Ref` MUST keep sharing the referent cell.
            Value::Unit
            | Value::Int(_)
            | Value::Bool(_)
            | Value::Float(_)
            | Value::UndeterminedNaN(_)
            | Value::Str(_)
            | Value::Ref(_) => Ok(self.clone()),
        }
    }

    /// Follow a `Ref` to the underlying cell, returning a clone of the same allocation so the
    /// aliasing is preserved. Non-references return `None`.
    pub fn as_ref_cell(&self) -> Option<Cell> {
        match self {
            Value::Ref(cell) => Some(cell.clone()),
            _ => None,
        }
    }
}

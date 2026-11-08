/*
    Allocation-owned semantic cells, reference descriptors, and runtime values
    for the checked-tree interpreter.

    Every storage place is an allocation-owned `Cell` so `&mut` borrows share
    the underlying cell by construction while encoded reference views retain
    their backing, projection path, and physical codec.
*/

use crate::build_evaluation_sponsor::{
    BuildEvaluationLiveCellLease, BuildEvaluationLiveTextByteLease, BuildEvaluationSponsor,
};
use std::cell::{Cell as CounterCell, Ref, RefCell};
use std::collections::BTreeMap;
use std::ops::{Deref, Range};
use std::rc::Rc;
use symbols::SymbolHandle;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

#[derive(Debug, Clone)]
pub struct Reference {
    pub(crate) cell: Cell,
    pub(crate) storage: Option<ReferenceStorage>,
}

impl Reference {
    pub fn new(cell: Cell) -> Self {
        Self {
            cell,
            storage: None,
        }
    }

    pub(crate) fn view(cell: Cell, storage: ReferenceStorage) -> Self {
        Self {
            cell,
            storage: Some(storage),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ReferenceStorage {
    Direct {
        source: PrimitiveType,
        target: PrimitiveType,
    },
    ByteRegion {
        cells: Vec<Cell>,
        offset: usize,
        target: PrimitiveType,
    },
    AggregateByteRegion {
        cells: Vec<Cell>,
        projection: ReferenceProjection,
    },
    AggregateTyped {
        source: Cell,
        source_type: TypeReferenceHandle,
        projection: ReferenceProjection,
    },
}

impl ReferenceStorage {
    pub(crate) fn target(&self) -> Option<PrimitiveType> {
        match self {
            Self::Direct { target, .. } | Self::ByteRegion { target, .. } => Some(*target),
            Self::AggregateByteRegion { .. } | Self::AggregateTyped { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ReferenceProjection {
    pub(crate) offset: usize,
    pub(crate) type_reference: TypeReferenceHandle,
    pub(crate) stored_integer: Option<typed_trees::PlanLaidIntegerField>,
}

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

    pub(crate) fn replace(&self, bytes: Vec<u8>) -> Result<(), String> {
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

pub type ValueName = Rc<str>;

#[derive(Debug, Clone)]
pub enum Value {
    Unit,
    Int(i64),
    Bool(bool),
    Float(f64),
    UndeterminedNaN(numerics::literals::FloatFormat),
    Str(TextBuffer),
    Struct {
        type_symbol: SymbolHandle,
        type_name: ValueName,
        fields: BTreeMap<ValueName, Cell>,
    },
    Enum {
        type_symbol: SymbolHandle,
        variant_name: ValueName,
        payload: Vec<(ValueName, Cell)>,
    },
    Array(Vec<Cell>),
    Ref(Reference),
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

    pub(crate) fn float_meaning(&self) -> Option<numerics::float_semantics::FloatMeaning> {
        use numerics::float_semantics::FloatMeaning;
        match self {
            Self::UndeterminedNaN(_) => Some(FloatMeaning::NaN),
            _ => self.as_float().map(FloatMeaning::from_f64),
        }
    }

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
            Value::Unit
            | Value::Int(_)
            | Value::Bool(_)
            | Value::Float(_)
            | Value::UndeterminedNaN(_)
            | Value::Str(_)
            | Value::Ref(_) => Ok(self.clone()),
        }
    }

    pub fn reference(cell: Cell) -> Self {
        Self::Ref(Reference::new(cell))
    }
}

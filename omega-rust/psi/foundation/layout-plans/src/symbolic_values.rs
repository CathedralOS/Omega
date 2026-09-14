//! Compiler-issued symbolic values: entry stub and data symbol identities,
//! relocation targets, symbolic field paths, and interior layout carriers.
//!
//! Numeric identities are never callable addresses and never reach source
//! programs.

use crate::layout_reports::LayoutPlanReport;
use crate::materialization::MaterializationDiagnostic;

/// Compiler-issued identity of an inbound entry stub. The numeric identity is
/// never a callable address and cannot be used for arithmetic or control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryStubId(u64);

impl EntryStubId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, MaterializationDiagnostic> {
        nonzero_identity("entry stub", identity).map(Self)
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Compiler-issued identity of statically placed data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DataSymbolId(u64);

impl DataSymbolId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, MaterializationDiagnostic> {
        nonzero_identity("data symbol", identity).map(Self)
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Closed source vocabulary for a toolchain-resolved value. Runtime-created
/// addresses remain ordinary `addr` data and do not enter this plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelocationTarget {
    Data(DataSymbolId),
    Entry(EntryStubId),
}

impl RelocationTarget {
    pub const fn normalized_identity(self) -> u64 {
        match self {
            Self::Data(identity) => identity.normalized_identity(),
            Self::Entry(identity) => identity.normalized_identity(),
        }
    }
}

/// One hop of a nested symbolic field path. `SymbolicFieldValue` spells the
/// outer hop; each segment adds the next `field` below the record stored by
/// the previous hop, carrying its own optional stable member identity and
/// element index. A segment may itself carry the next segment, so record
/// depth is data in the path rather than a family of depth-specific types;
/// derivation bounds the walk by [`CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT`].
/// Each boundary's placement comes from a [`SymbolicFieldInnerLayout`]
/// carrier supplied at derivation, so the exact inner offset stays symbolic
/// until assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicFieldPathSegment {
    pub field: String,
    pub(crate) member_identity: Option<u64>,
    pub(crate) element_index: Option<u64>,
    /// The next record boundary below this segment; `None` ends the path.
    inner: Option<Box<SymbolicFieldPathSegment>>,
}

impl SymbolicFieldPathSegment {
    /// One inner hop named by field. `outer.field` selects every placement the
    /// inner layout retains for `field`.
    pub fn new(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            member_identity: None,
            element_index: None,
            inner: None,
        }
    }

    /// `new` carrying the compiler-retained stable member identity. The field
    /// spelling remains diagnostic presentation.
    pub fn new_numbered(field: impl Into<String>, member_identity: u64) -> Self {
        Self {
            member_identity: Some(member_identity),
            ..Self::new(field)
        }
    }

    /// `new` addressed to the `element_index`-th element of a repeated inner
    /// field. The bound is checked against the inner layout's element
    /// placements when the plan is derived, not when the caller spells it.
    pub fn new_indexed(field: impl Into<String>, element_index: u64) -> Self {
        Self {
            element_index: Some(element_index),
            ..Self::new(field)
        }
    }

    /// `new_indexed` carrying the compiler-retained stable member identity.
    pub fn new_indexed_numbered(
        field: impl Into<String>,
        member_identity: u64,
        element_index: u64,
    ) -> Self {
        Self {
            member_identity: Some(member_identity),
            element_index: Some(element_index),
            ..Self::new(field)
        }
    }

    /// The exact element index preserved on this segment, if any. `None` means
    /// the segment covers every placement the inner layout retains.
    pub const fn element_index(&self) -> Option<u64> {
        self.element_index
    }

    /// Extends this segment with the next hop of the record path, so
    /// `outer.field.sub` (or `outer[index].field[index].sub`) selects inside
    /// the record stored in `field`. The next boundary's interior comes from
    /// a nested [`SymbolicFieldInnerLayout`] carrier; no concrete address or
    /// offset is baked into the path.
    pub fn with_inner_segment(mut self, inner: SymbolicFieldPathSegment) -> Self {
        self.inner = Some(Box::new(inner));
        self
    }

    /// The next hop of this record path, if any.
    pub const fn inner(&self) -> Option<&SymbolicFieldPathSegment> {
        match &self.inner {
            Some(inner) => Some(&**inner),
            None => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicFieldValue {
    pub field: String,
    pub(crate) member_identity: Option<u64>,
    pub(crate) element_index: Option<u64>,
    /// The optional second hop of a nested `outer.field` path. Each segment
    /// may carry the next, so record depth is data; `None` keeps this value
    /// on the flat `field[index]` surface.
    pub(crate) inner: Option<SymbolicFieldPathSegment>,
    pub width_bits: u16,
    pub target: RelocationTarget,
}

impl SymbolicFieldValue {
    pub fn new(
        field: impl Into<String>,
        width_bits: u16,
        target: RelocationTarget,
    ) -> Result<Self, MaterializationDiagnostic> {
        if width_bits == 0 || width_bits > 64 {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field width {width_bits} is outside 1..=64 bits"
            )));
        }
        Ok(Self {
            field: field.into(),
            member_identity: None,
            element_index: None,
            inner: None,
            width_bits,
            target,
        })
    }

    /// Constructs a symbolic value carrying its compiler-retained stable
    /// member identity. The field spelling remains diagnostic presentation.
    pub fn new_numbered(
        field: impl Into<String>,
        member_identity: u64,
        width_bits: u16,
        target: RelocationTarget,
    ) -> Result<Self, MaterializationDiagnostic> {
        let mut value = Self::new(field, width_bits, target)?;
        value.member_identity = Some(member_identity);
        Ok(value)
    }

    /// Constructs a symbolic value addressed to the `element_index`-th element
    /// of a repeated field. The exact index is preserved symbolically until
    /// materialization assigns it the element's `At` offset; physical lowering
    /// may choose that offset but cannot change which element is accessed.
    /// Index bounds are checked against the field's element placements when the
    /// plan is derived, not when the caller builds this value.
    pub fn new_indexed(
        field: impl Into<String>,
        element_index: u64,
        width_bits: u16,
        target: RelocationTarget,
    ) -> Result<Self, MaterializationDiagnostic> {
        let mut value = Self::new(field, width_bits, target)?;
        value.element_index = Some(element_index);
        Ok(value)
    }

    /// `new_indexed` carrying the compiler-retained stable member identity.
    pub fn new_indexed_numbered(
        field: impl Into<String>,
        member_identity: u64,
        element_index: u64,
        width_bits: u16,
        target: RelocationTarget,
    ) -> Result<Self, MaterializationDiagnostic> {
        let mut value = Self::new_indexed(field, element_index, width_bits, target)?;
        value.member_identity = Some(member_identity);
        Ok(value)
    }

    /// The exact element index preserved on this symbolic field/index path, if
    /// any. `None` means the symbolic value covers every placement of the field.
    pub const fn element_index(&self) -> Option<u64> {
        self.element_index
    }

    /// Extends this symbolic value with a second path segment, spelling the
    /// `field.inner` (or `field[index].inner`) hop into the record stored in
    /// `field`. The segment may itself carry further segments, so a path like
    /// `field.inner.sub` spells each record boundary as data. Each boundary's
    /// placement comes from a [`SymbolicFieldInnerLayout`] carrier supplied
    /// to [`derive_symbolic_materialization_with_inner_layouts`]; no concrete
    /// address or inner offset is baked into the value itself.
    pub fn with_inner_segment(mut self, inner: SymbolicFieldPathSegment) -> Self {
        self.inner = Some(inner);
        self
    }

    /// The second hop of this nested field path, if any.
    pub const fn inner(&self) -> Option<&SymbolicFieldPathSegment> {
        self.inner.as_ref()
    }
}

/// The compiler-derived interior layout of the record stored in one outer
/// field, bound to that field's identity. A nested record's member offsets are
/// compiler-derived interior geometry shared with typed-owned encoding, not
/// policy-chosen placements, so the flat outer [`LayoutPlanReport`] deliberately
/// does not carry them. This carrier retains the inner layout beside the outer
/// plan without flattening inner rows into the outer schema;
/// [`derive_symbolic_materialization_with_inner_layouts`] consults it only for
/// symbolic values spelling an inner path segment through that outer field.
///
/// When the stored record itself contains record fields, [`Self::with_inner_layout`]
/// binds each nested record's interior under this carrier, so the carrier tree
/// mirrors the record boundaries a path crosses. Depth is data in the tree
/// rather than a family of depth-specific carriers; derivation bounds it by
/// [`CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT`]. Every supplied carrier must be
/// traversed by some symbolic path: a carrier that outlives the semantic path
/// it describes would let a stale interior join a renamed or reshaped schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicFieldInnerLayout {
    /// Outer field name. Report/diagnostic presentation; the member identity
    /// joins when the outer schema is numbered.
    pub field: String,
    pub(crate) member_identity: Option<u64>,
    /// The nested record's complete compiler-derived interior layout.
    pub inner_layout: LayoutPlanReport,
    /// Carriers bound to record fields inside `inner_layout`, supplying the
    /// interior of the next record boundary down.
    pub(crate) inner_layouts: Vec<SymbolicFieldInnerLayout>,
}

impl SymbolicFieldInnerLayout {
    /// Binds `inner_layout` to the outer field named `field`.
    pub fn new(field: impl Into<String>, inner_layout: LayoutPlanReport) -> Self {
        Self {
            field: field.into(),
            member_identity: None,
            inner_layout,
            inner_layouts: Vec::new(),
        }
    }

    /// `new` carrying the outer field's compiler-retained stable member
    /// identity, so a renamed outer schema still joins the carrier.
    pub fn new_numbered(
        field: impl Into<String>,
        member_identity: u64,
        inner_layout: LayoutPlanReport,
    ) -> Self {
        Self {
            member_identity: Some(member_identity),
            ..Self::new(field, inner_layout)
        }
    }

    /// Binds the interior layout of a record field inside this carrier's
    /// `inner_layout`, so a symbolic path continuing through
    /// `field.<nested>` finds the next record boundary's carrier.
    pub fn with_inner_layout(mut self, inner: SymbolicFieldInnerLayout) -> Self {
        self.inner_layouts.push(inner);
        self
    }

    /// The carriers bound to record fields inside this interior layout, in
    /// supply order.
    pub fn inner_layouts(&self) -> &[SymbolicFieldInnerLayout] {
        &self.inner_layouts
    }
}

pub(crate) fn nonzero_identity(
    kind: &str,
    identity: u64,
) -> Result<u64, MaterializationDiagnostic> {
    if identity == 0 {
        Err(MaterializationDiagnostic(format!(
            "normalized {kind} identity cannot be zero"
        )))
    } else {
        Ok(identity)
    }
}

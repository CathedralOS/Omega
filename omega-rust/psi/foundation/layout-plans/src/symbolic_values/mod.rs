//! Compiler-issued symbolic values: entry stub and data symbol identities,
//! relocation targets, symbolic field paths, and interior layout carriers.
//!
//! Numeric identities are never callable addresses and never reach source
//! programs.

use crate::layout_reports::{ConventionalSumLayoutReport, LayoutPlanReport};
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
/// until assignment. Below a conventional sum boundary the next hop spells
/// the selected case and the final hop spells that case's payload field;
/// a case payload always ends a sum path, so it never carries a carrier or
/// a further segment.
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
    /// `field.inner.sub` spells each record boundary as data. When the bound
    /// carrier describes a conventional sum, the following two segments spell
    /// `field.Case.payload` (or `field[index].Case.payload` over a repeated
    /// sum field); the same hop vocabulary carries that boundary too. Each
    /// boundary's placement comes from a [`SymbolicFieldInnerLayout`] carrier
    /// supplied to [`derive_symbolic_materialization_with_inner_layouts`]; no
    /// concrete address or inner offset is baked into the value itself.
    pub fn with_inner_segment(mut self, inner: SymbolicFieldPathSegment) -> Self {
        self.inner = Some(inner);
        self
    }

    /// The second hop of this nested field path, if any.
    pub const fn inner(&self) -> Option<&SymbolicFieldPathSegment> {
        self.inner.as_ref()
    }
}

/// The compiler-derived interior layout bound to one outer field. A nested
/// record's member offsets and a conventional sum's case/payload geometry are
/// compiler-derived interior geometry shared with typed-owned encoding, not
/// policy-chosen placements, so the flat outer [`LayoutPlanReport`]
/// deliberately carries neither. The kind of interior the field stores is
/// data on the carrier, not a separate carrier family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolicFieldInteriorLayout {
    /// The field stores one nested record; this is its complete
    /// compiler-derived interior plan. Record fields inside it may carry
    /// their own carriers through
    /// [`SymbolicFieldInnerLayout::with_inner_layout`], so the carrier tree
    /// mirrors the record boundaries a path crosses.
    Record(LayoutPlanReport),
    /// The field stores one conventional pure sum; this is the compiler-owned
    /// tag-prefixed overlay shared with build-time const materialization. A
    /// path crossing the boundary spells the selected case and that case's
    /// payload field as its last two hops; neither hop carries an element
    /// index. The tag and the inactive cases' payload bytes stay staged
    /// content: the writer only realizes the addressed payload slot.
    Sum(ConventionalSumLayoutReport),
    /// The field repeats one conventional pure sum at a constant byte stride.
    /// The outer plan retains the field's whole array extent as one `At`
    /// placement, so the path's element index composes
    /// `index * element_stride` inside that extent before the case and
    /// payload hops resolve inside the addressed element. `element_stride`
    /// must cover the complete `element_layout` extent so repeated elements
    /// cannot overlap.
    SumArray {
        /// One array element's complete conventional sum overlay.
        element_layout: ConventionalSumLayoutReport,
        /// The literal element count the field's extent covers.
        element_count: u64,
        /// The constant byte distance between consecutive elements.
        element_stride: u64,
    },
}

/// The compiler-derived interior layout stored by one outer field, bound to
/// that field's identity. This carrier retains the interior beside the outer
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
    /// The interior bound to `field`: a nested record's plan, one conventional
    /// sum's overlay, or a repeated conventional sum's element geometry.
    pub inner_layout: SymbolicFieldInteriorLayout,
    /// Carriers bound to record fields inside a `Record` interior layout,
    /// supplying the interior of the next record boundary down. Sum interiors
    /// carry no field namespace, so they never hold nested carriers.
    pub(crate) inner_layouts: Vec<SymbolicFieldInnerLayout>,
}

impl SymbolicFieldInnerLayout {
    /// Binds the nested record `inner_layout` to the outer field named
    /// `field`.
    pub fn new(field: impl Into<String>, inner_layout: LayoutPlanReport) -> Self {
        Self {
            field: field.into(),
            member_identity: None,
            inner_layout: SymbolicFieldInteriorLayout::Record(inner_layout),
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

    /// Binds one direct conventional sum interior to the outer field named
    /// `field`. A symbolic path crossing the boundary spells the selected
    /// case and that case's payload field as its last two hops.
    pub fn new_sum(field: impl Into<String>, sum_layout: ConventionalSumLayoutReport) -> Self {
        Self {
            field: field.into(),
            member_identity: None,
            inner_layout: SymbolicFieldInteriorLayout::Sum(sum_layout),
            inner_layouts: Vec::new(),
        }
    }

    /// `new_sum` carrying the outer field's compiler-retained stable member
    /// identity.
    pub fn new_sum_numbered(
        field: impl Into<String>,
        member_identity: u64,
        sum_layout: ConventionalSumLayoutReport,
    ) -> Self {
        Self {
            member_identity: Some(member_identity),
            ..Self::new_sum(field, sum_layout)
        }
    }

    /// Binds a repeated conventional sum interior to the outer field named
    /// `field`. The outer plan retains the field's whole array extent as one
    /// `At` placement; a symbolic path crossing the boundary carries the
    /// element index on the field hop, then the selected case and that
    /// case's payload field as its last two hops.
    pub fn new_sum_array(
        field: impl Into<String>,
        element_layout: ConventionalSumLayoutReport,
        element_count: u64,
        element_stride: u64,
    ) -> Self {
        Self {
            field: field.into(),
            member_identity: None,
            inner_layout: SymbolicFieldInteriorLayout::SumArray {
                element_layout,
                element_count,
                element_stride,
            },
            inner_layouts: Vec::new(),
        }
    }

    /// `new_sum_array` carrying the outer field's compiler-retained stable
    /// member identity.
    pub fn new_sum_array_numbered(
        field: impl Into<String>,
        member_identity: u64,
        element_layout: ConventionalSumLayoutReport,
        element_count: u64,
        element_stride: u64,
    ) -> Self {
        Self {
            member_identity: Some(member_identity),
            ..Self::new_sum_array(field, element_layout, element_count, element_stride)
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

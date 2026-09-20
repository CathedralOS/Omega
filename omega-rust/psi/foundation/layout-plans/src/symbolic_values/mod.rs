//! Compiler-issued symbolic values: entry stub and data symbol identities,
//! relocation targets, symbolic field paths, and interior layout carriers.
//!
//! Numeric identities are never callable addresses and never reach source
//! programs.

use crate::layout_reports::{
    CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT, ConventionalRecordSumChildHop,
    ConventionalRecordSumChildInterior, ConventionalRecursiveRecordSumPathsLayoutReport,
    ConventionalSumLayoutReport, LayoutPlanReport,
};
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
/// a further segment. Below a mixed common-field/case shape a single hop may
/// instead spell one common field directly — common members sit between the
/// tag and the shared overlay — and that hop likewise ends the path.
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
    /// sum field); a mixed shape's common field is the one-segment spelling
    /// `field.common` (or `field[index].common`). The same hop vocabulary
    /// carries that boundary too. Each
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

/// The interior geometry a field binds below its hop: a nested record's
/// complete compiler-derived plan, or one conventional sum's tag/case
/// overlay — pure or mixed common-field/case. A nested record's member
/// offsets and a conventional sum's case/payload geometry are
/// compiler-derived interior geometry shared with typed-owned encoding, not
/// policy-chosen placements, so the flat outer [`LayoutPlanReport`]
/// deliberately carries neither. The kind of interior the field stores is
/// data on the carrier, not a separate carrier family.
///
/// Record interiors may carry their own carriers through
/// [`SymbolicFieldInnerLayout::with_inner_layout`], so the carrier tree
/// mirrors the record boundaries a path crosses. A sum interior's boundary
/// spelling is fixed by the overlay: the path spells the selected case and
/// that case's payload field as its last two hops, or one mixed common
/// field as its last hop, and neither spelling carries an element index.
/// The tag and the inactive cases' payload bytes stay staged content: the
/// writer only realizes the addressed member slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolicFieldInterior {
    /// The field's element stores one nested record; this is its complete
    /// compiler-derived interior plan.
    Record(LayoutPlanReport),
    /// The field's element stores one conventional sum — a pure case overlay
    /// or a mixed common-field/case shape; the compiler-owned tag-prefixed
    /// layout shared with build-time const materialization.
    Sum(ConventionalSumLayoutReport),
}

/// The compiler-derived interior layout bound to one outer field. The
/// carrier factors each boundary into the path segment that reaches it —
/// [`ConventionalRecordSumChildHop`] spells a bare field hop or a literal
/// index hop carrying the field's repeated count and stride — and the
/// interior geometry bound below it. Repetition is hop data rather than a
/// variant per interior kind, so `field.<inner>` and `field[i].<inner>`
/// differ only in the hop the path spells. An `Index` hop's
/// `element_stride` covers the complete interior extent so repeated
/// elements cannot overlap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicFieldInteriorLayout {
    /// How the path segment reaches the bound interior.
    pub hop: ConventionalRecordSumChildHop,
    /// The interior geometry bound below the hop.
    pub interior: SymbolicFieldInterior,
}

impl SymbolicFieldInteriorLayout {
    /// The interior the addressed element occupies, beside whichever hop
    /// the path spelled to reach it.
    pub const fn interior(&self) -> &SymbolicFieldInterior {
        &self.interior
    }

    /// `Some((element_count, element_stride))` when the field repeats the
    /// bound interior at a constant byte stride.
    pub const fn repetition(&self) -> Option<(u64, u64)> {
        match self.hop {
            ConventionalRecordSumChildHop::Field => None,
            ConventionalRecordSumChildHop::Index {
                element_count,
                element_stride,
            } => Some((element_count, element_stride)),
        }
    }
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
/// [`CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT`]. [`Self::from_recursive_sum_paths`]
/// folds the recursive record/sum projection report into this tree directly.
/// Every supplied carrier must be traversed by some symbolic path: a carrier
/// that outlives the semantic path it describes would let a stale interior
/// join a renamed or reshaped schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicFieldInnerLayout {
    /// Outer field name. Report/diagnostic presentation; the member identity
    /// joins when the outer schema is numbered.
    pub field: String,
    pub(crate) member_identity: Option<u64>,
    /// The interior bound to `field`: the hop that reaches it beside the
    /// record plan or sum overlay the hop addresses.
    pub inner_layout: SymbolicFieldInteriorLayout,
    /// Carriers bound to record fields inside a record interior layout —
    /// either a direct `Record` interior's own plan or an `Index`-hopped
    /// record element's plan — supplying the interior of the next record
    /// boundary down. Sum interiors carry no field namespace, so they never
    /// hold nested carriers.
    pub(crate) inner_layouts: Vec<SymbolicFieldInnerLayout>,
}

impl SymbolicFieldInnerLayout {
    /// Binds the nested record `inner_layout` to the outer field named
    /// `field`.
    pub fn new(field: impl Into<String>, inner_layout: LayoutPlanReport) -> Self {
        Self {
            field: field.into(),
            member_identity: None,
            inner_layout: SymbolicFieldInteriorLayout {
                hop: ConventionalRecordSumChildHop::Field,
                interior: SymbolicFieldInterior::Record(inner_layout),
            },
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
            inner_layout: SymbolicFieldInteriorLayout {
                hop: ConventionalRecordSumChildHop::Field,
                interior: SymbolicFieldInterior::Sum(sum_layout),
            },
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
    /// `field`. The outer plan retains the field's array extent either as
    /// one whole `At` placement or as one `At` per element replaying this
    /// stride; a symbolic path crossing the boundary carries the element
    /// index on the field hop, then the selected case and that case's
    /// payload field as its last two hops.
    pub fn new_sum_array(
        field: impl Into<String>,
        element_layout: ConventionalSumLayoutReport,
        element_count: u64,
        element_stride: u64,
    ) -> Self {
        Self {
            field: field.into(),
            member_identity: None,
            inner_layout: SymbolicFieldInteriorLayout {
                hop: ConventionalRecordSumChildHop::Index {
                    element_count,
                    element_stride,
                },
                interior: SymbolicFieldInterior::Sum(element_layout),
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

    /// Binds a repeated record interior to the outer field named `field`.
    /// The outer plan retains the field's array extent either as one whole
    /// `At` placement or as one `At` per element replaying this stride; a
    /// symbolic path crossing the boundary carries the element index on the
    /// field hop, then resolves the next segment inside the addressed
    /// element's record interior — `field[index].member` — composing the
    /// same `index * element_stride` hop a repeated sum field spells before
    /// its case and payload hops.
    pub fn new_record_array(
        field: impl Into<String>,
        element_layout: LayoutPlanReport,
        element_count: u64,
        element_stride: u64,
    ) -> Self {
        Self {
            field: field.into(),
            member_identity: None,
            inner_layout: SymbolicFieldInteriorLayout {
                hop: ConventionalRecordSumChildHop::Index {
                    element_count,
                    element_stride,
                },
                interior: SymbolicFieldInterior::Record(element_layout),
            },
            inner_layouts: Vec::new(),
        }
    }

    /// `new_record_array` carrying the outer field's compiler-retained stable
    /// member identity.
    pub fn new_record_array_numbered(
        field: impl Into<String>,
        member_identity: u64,
        element_layout: LayoutPlanReport,
        element_count: u64,
        element_stride: u64,
    ) -> Self {
        Self {
            member_identity: Some(member_identity),
            ..Self::new_record_array(field, element_layout, element_count, element_stride)
        }
    }

    /// Folds one record-boundary level of a recursive conventional record/sum
    /// path report into the carriers symbolic derivation binds there.
    ///
    /// The projection report already carries record depth as data on its
    /// `children` channel, so this fold is one structural recursion over it:
    /// each child row binds the carrier its own hop and interior spell — a
    /// `Field` hop binds the direct boundary, an `Index` hop binds the
    /// repeated row, a `Sum` interior binds the overlay itself, and a
    /// `Record` interior retains the child's complete outer layout with the
    /// child report's own carriers nested under it through
    /// [`Self::with_inner_layout`]. The top-level call supplies the carriers
    /// for the report's `outer_layout`, which is the flat plan passed to
    /// [`derive_symbolic_materialization_with_inner_layouts`]. Carrier
    /// binding, interior bounds, and the path depth limit stay with
    /// derivation — a folded carrier naming a field the enclosing plan never
    /// placed still rejects there, so this fold adds no admission rule of
    /// its own.
    ///
    /// [`derive_symbolic_materialization_with_inner_layouts`]: crate::derive_symbolic_materialization_with_inner_layouts
    ///
    /// The fold walks the report's own recursion under the same
    /// [`CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT`] resource bound carrier
    /// preparation enforces, so a report nesting past the deepest admissible
    /// carrier rejects here instead of overflowing the fold.
    pub fn from_recursive_sum_paths(
        report: &ConventionalRecursiveRecordSumPathsLayoutReport,
    ) -> Result<Vec<Self>, MaterializationDiagnostic> {
        Self::fold_recursive_sum_paths(report, 0)
    }

    fn fold_recursive_sum_paths(
        report: &ConventionalRecursiveRecordSumPathsLayoutReport,
        depth: usize,
    ) -> Result<Vec<Self>, MaterializationDiagnostic> {
        let mut carriers = Vec::new();
        carriers
            .try_reserve_exact(report.children.len())
            .map_err(|_| {
                MaterializationDiagnostic(
                    "recursive record/sum path carrier fold exceeds compiler resources".into(),
                )
            })?;
        for child in &report.children {
            let carrier = match (&child.hop, &child.interior) {
                (
                    ConventionalRecordSumChildHop::Field,
                    ConventionalRecordSumChildInterior::Sum(layout),
                ) => match child.member_identity {
                    Some(identity) => {
                        Self::new_sum_numbered(child.field.clone(), identity, layout.clone())
                    }
                    None => Self::new_sum(child.field.clone(), layout.clone()),
                },
                (
                    ConventionalRecordSumChildHop::Index {
                        element_count,
                        element_stride,
                    },
                    ConventionalRecordSumChildInterior::Sum(layout),
                ) => match child.member_identity {
                    Some(identity) => Self::new_sum_array_numbered(
                        child.field.clone(),
                        identity,
                        layout.clone(),
                        *element_count,
                        *element_stride,
                    ),
                    None => Self::new_sum_array(
                        child.field.clone(),
                        layout.clone(),
                        *element_count,
                        *element_stride,
                    ),
                },
                (hop, ConventionalRecordSumChildInterior::Record(inner)) => {
                    if depth + 1 >= CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT {
                        return Err(MaterializationDiagnostic(format!(
                            "recursive record/sum path report nests beyond the compiler's {CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT}-segment record path bound"
                        )));
                    }
                    let mut carrier = match (hop, child.member_identity) {
                        (ConventionalRecordSumChildHop::Field, Some(identity)) => {
                            Self::new_numbered(
                                child.field.clone(),
                                identity,
                                inner.outer_layout.clone(),
                            )
                        }
                        (ConventionalRecordSumChildHop::Field, None) => {
                            Self::new(child.field.clone(), inner.outer_layout.clone())
                        }
                        (
                            ConventionalRecordSumChildHop::Index {
                                element_count,
                                element_stride,
                            },
                            Some(identity),
                        ) => Self::new_record_array_numbered(
                            child.field.clone(),
                            identity,
                            inner.outer_layout.clone(),
                            *element_count,
                            *element_stride,
                        ),
                        (
                            ConventionalRecordSumChildHop::Index {
                                element_count,
                                element_stride,
                            },
                            None,
                        ) => Self::new_record_array(
                            child.field.clone(),
                            inner.outer_layout.clone(),
                            *element_count,
                            *element_stride,
                        ),
                    };
                    for nested in Self::fold_recursive_sum_paths(inner, depth + 1)? {
                        carrier = carrier.with_inner_layout(nested);
                    }
                    carrier
                }
            };
            carriers.push(carrier);
        }
        Ok(carriers)
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

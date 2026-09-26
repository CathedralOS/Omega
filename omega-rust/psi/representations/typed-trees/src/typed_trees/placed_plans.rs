//! Placed view, field and accessor plans and the plan-laid layouts.

use crate::typed_trees::ClosedConformanceApplication;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedViewPlan {
    pub data_name: String,
    /// Exact synthesized placed-view data identity. `data_name` is diagnostic
    /// presentation and must not be used as the semantic join key.
    pub data_symbol: symbols::SymbolHandle,
    pub policy_name: String,
    /// Exact nominal placement-policy data identity.
    pub policy_symbol: symbols::SymbolHandle,
    /// Exact build-time `Policy::plan` machine that produced `placement`.
    pub policy_plan_machine_symbol: symbols::SymbolHandle,
    pub schema_name: String,
    /// Exact source schema identity whose fields the plan interprets.
    pub schema_symbol: symbols::SymbolHandle,
    pub placement: access_plans::ValidatedPlacementPlan,
    pub fields: Vec<PlacedFieldPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedFieldPlan {
    pub field_name: String,
    /// Stable numbered identity when the source schema numbers this field.
    /// The spelling remains diagnostic presentation only in that case.
    pub member_identity: Option<u64>,
    pub field_symbol: symbols::SymbolHandle,
    pub accessor_name: String,
    /// Exact synthesized accessor type reference. Shell-aware typed lookup
    /// rejoins through this handle rather than `accessor_name`.
    pub accessor_type: crate::types::TypeReferenceHandle,
    /// Exact generated accessor data definition. `accessor_name` is retained
    /// for diagnostics and source-oriented artifact presentation only.
    pub accessor_data_symbol: symbols::SymbolHandle,
    /// Exact generated operation targets for non-atomic placed accessors.
    /// Atomic operations retain their separate typed carrier and therefore
    /// have no cloned `PlacedField` operation target rows here.
    pub accessor_targets: Vec<PlacedAccessorTarget>,
    pub value_type: crate::types::TypeReferenceHandle,
    pub access: access_plans::FieldAccess,
    /// Checked, non-authorizing resident/result-shape evidence retained only
    /// when an Atomic placed field admits an observing compare-exchange axis.
    /// Try-only fields remain `None`; this does not carry a resident value, an
    /// operation attempt, selected encoding law, or target-lowering authority.
    /// Current source-formable Atomic scalar residents are unrestricted because
    /// aggregate fields remain behind the Inaccessible fence, but formation
    /// still consumes the exact normalized multiplicity rather than assuming it.
    pub atomic_resident: Option<PlacedAtomicResidentContract>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedAtomicResidentContract {
    /// Exact source field whose resident type is described.
    pub field_symbol: symbols::SymbolHandle,
    /// Exact semantic resident type retained by the source schema field.
    pub resident_type: crate::types::TypeReferenceHandle,
    /// Normalized source multiplicity used to admit observing failure arms.
    pub multiplicity: language_semantics::Multiplicity,
    /// Exact normalized Atomic transfer width.
    pub transfer_width_bits: u16,
    /// Exact independently admitted observing permission axes.
    pub compare_exchange: bool,
    pub compare_exchange_once: bool,
    /// Canonical decisive-then-single-attempt result-shape rows.
    pub observing_results: Vec<PlacedAtomicObservingResultContract>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedAtomicObservingResultContract {
    pub operation: language_core::atomic::AtomicObservingCompareExchangeOperation,
    pub result_shape: language_core::atomic::AtomicObservingCompareExchangeResultShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedAccessorTarget {
    pub operation: String,
    pub machine_symbol: symbols::SymbolHandle,
    pub state_symbol: symbols::SymbolHandle,
}

pub(crate) fn named_type_reference_through_shells(
    table: &crate::types::TypeReferenceTable,
    handle: crate::types::TypeReferenceHandle,
) -> Option<crate::types::TypeReferenceHandle> {
    match table.type_reference(handle) {
        crate::types::TypeReferenceNode::Named { .. } => Some(handle),
        crate::types::TypeReferenceNode::Reference { referee, .. } => {
            named_type_reference_through_shells(table, *referee)
        }
        crate::types::TypeReferenceNode::Constrained { base_type, .. } => {
            named_type_reference_through_shells(table, *base_type)
        }
        _ => None,
    }
}

/// One validated, FULLY-STATIC layout plan applied to a synthesized data
/// definition (the compiler-generated `Policy<Schema>` instance). Offsets are
/// per field in declaration order; the plan was validated (bounds, overlap,
/// alignment) before it was recorded here, so the layout builder may trust it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanLaidLayout {
    /// Name of the synthesized data definition (e.g. `CLayout<GdtEntryish>`).
    /// Diagnostic/source-oriented presentation only.
    pub data_name: String,
    /// Exact synthesized data definition whose physical layout this plan owns.
    pub data_symbol: symbols::SymbolHandle,
    /// Exact runtime field identities in the same order as `offsets`.
    pub field_symbols: Vec<symbols::SymbolHandle>,
    /// Exact source schema and runtime field identities reflected into the
    /// synthesized value type.
    pub schema_symbol: symbols::SymbolHandle,
    pub schema_field_symbols: Vec<symbols::SymbolHandle>,
    /// Exact nominal layout policy and build-time plan machine that produced
    /// this geometry. These identities do not grant runtime authority.
    pub policy_symbol: symbols::SymbolHandle,
    pub policy_plan_machine_symbol: symbols::SymbolHandle,
    /// Exact validated target-neutral geometry from which the host-sized
    /// consumer projections below were derived.
    pub validated_layout: layout_plans::LayoutPlanReport,
    /// Source-authored semantic-field-free callback destinations. These rows
    /// intentionally stop at exact declaration identity plus offset; the
    /// selected target calling plan supplies pointer extent and completes
    /// bounds/non-overlap validation before materialization.
    pub private_callback_demands:
        Vec<layout_plans::PrivateCallbackLayoutDemandReport<ClosedConformanceApplication>>,
    /// Byte offset of each field, in declaration order.
    pub offsets: Vec<usize>,
    /// Fragmented scalar fields keyed by declaration-order field index.
    /// Empty for ordinary byte-aligned plans. The compiler's layout
    /// validator has already proved complete source tiling, non-overlapping
    /// destinations, and in-bounds containers.
    pub bit_fields: Vec<PlanLaidBitField>,
    /// Fixed-width integer fields whose physical encoding is narrower than
    /// their semantic carrier. The validated plan has already proved that
    /// every stored bit pattern decodes into the carrier.
    pub integer_fields: Vec<PlanLaidIntegerField>,
    /// Outer fixed-array fields whose validated plan places one complete
    /// compiler-sized element at each destination in one constant-stride
    /// sequence. The ordinary field offset remains the first destination;
    /// consumers use this row only while indexing that outer array.
    pub repeated_fields: Vec<PlanLaidRepeatedField>,
    /// Total value size (fixed by the value-type gate).
    pub size: usize,
    pub align: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanLaidRepeatedField {
    pub field_index: usize,
    pub element_stride: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanLaidBitField {
    pub field_index: usize,
    pub fragments: Vec<PlanLaidBitFragment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanLaidIntegerField {
    pub field_index: usize,
    pub stored_width_bits: u16,
    pub interpretation: layout_plans::IntegerInterpretation,
    /// Every value admitted by the semantic field type has an encoding at the
    /// stored width. Mutation may truncate only when this validated fact holds.
    pub write_is_total: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlanLaidBitFragment {
    pub container_byte_offset: usize,
    pub container_width_bits: u16,
    pub destination_lsb: u16,
    pub source_lsb: u16,
    pub width: u16,
}

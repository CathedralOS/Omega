use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

mod field_paths;
mod sum_materialization;

mod builder;
mod packing;
mod sizing;

pub use builder::{build_layout_plan, layout_type_reference};
pub use field_paths::{field_data_layout_fields, field_machine_layout, field_path_offset};
pub use sizing::primitive_layout;
pub use sum_materialization::{
    project_conventional_record_with_depth_three_nested_sum_materialization_layout,
    project_conventional_record_with_depth_two_nested_sum_materialization_layout,
    project_conventional_record_with_depth_two_nested_sums_materialization_layout,
    project_conventional_record_with_nested_sum_record_materialization_layout,
    project_conventional_record_with_nested_sum_records_materialization_layout,
    project_conventional_record_with_sum_array_materialization_layout,
    project_conventional_record_with_sum_arrays_materialization_layout,
    project_conventional_record_with_sum_materialization_layout,
    project_conventional_sum_materialization_layout,
};

/// Size (and alignment) of the i32 case tag that prefixes every enum-shaped
/// data value. Comparing an enum value against a case constant compares ONLY
/// this prefix (tag-only equality); payload bytes never participate.
pub const ENUM_TAG_BYTES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeLayout {
    pub size: usize,
    pub alignment: usize,
}

impl Default for TypeLayout {
    fn default() -> Self {
        Self {
            size: 0,
            alignment: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeLayoutDescriptor {
    Reference {
        referee: Box<TypeLayoutDescriptor>,
        is_mutable: bool,
    },
    Constrained {
        base_type: Box<TypeLayoutDescriptor>,
        domain: psi_numerics::arithmetic::ArithmeticDomain,
    },
    FixedArray {
        element_type: Box<TypeLayoutDescriptor>,
        length: usize,
    },
    /// An owned, variable-fill bounded byte buffer: `[u8; N]` carrying a runtime
    /// `len <= N`. Laid out as `{ len, [element; capacity] }` INLINE -- a value,
    /// NOT a `{ptr,len}` descriptor: the length is an explicit leading word and
    /// the bytes follow inline, so reads view it as `{ &bytes, len }` and writes
    /// copy content in + set `len`. Distinct from `FixedArray` (always-full, no
    /// len word) and from `Slice`/`Reference` (borrowed `{ptr,len}`). Produced for
    /// a `[u8; N] in <text-domain>` field -- the owned text carrier (#66) -- since
    /// the text domain is stripped before the backend descriptor, so the carrier
    /// needs its own variant to survive.
    BoundedByteBuffer {
        element_type: Box<TypeLayoutDescriptor>,
        capacity: usize,
    },
    Slice {
        element_type: Box<TypeLayoutDescriptor>,
    },
    DynamicTrait {
        /// Dynamic trait whose requirement surface owns the private table.
        symbol: SymbolHandle,
        name: Identifier,
        /// Exact selected nominal conformance when the source spelling named
        /// one. Bare coercions retain their checked selection in Psi facts.
        conformance: Option<SymbolHandle>,
        conformance_carrier: Option<Identifier>,
        conformance_name: Option<Identifier>,
    },
    Named {
        symbol: SymbolHandle,
        name: Identifier,
    },
    Unit,
}

impl Default for TypeLayoutDescriptor {
    fn default() -> Self {
        Self::Unit
    }
}

impl TypeLayoutDescriptor {
    pub fn storage_symbol(&self) -> SymbolHandle {
        match self {
            Self::Reference { referee, .. } => referee.storage_symbol(),
            Self::Constrained { base_type, .. } => base_type.storage_symbol(),
            Self::FixedArray { element_type, .. } => element_type.storage_symbol(),
            Self::BoundedByteBuffer { element_type, .. } => element_type.storage_symbol(),
            Self::Slice { element_type } => element_type.storage_symbol(),
            Self::DynamicTrait { symbol, .. } => *symbol,
            Self::Named { symbol, .. } => *symbol,
            Self::Unit => SymbolHandle::invalid(),
        }
    }

    pub fn fixed_array(&self) -> Option<(&Self, usize)> {
        match self {
            Self::Constrained { base_type, .. } => base_type.fixed_array(),
            Self::Reference { referee, .. } => referee.fixed_array(),
            Self::FixedArray {
                element_type,
                length,
            } => Some((element_type, *length)),
            _ => None,
        }
    }

    pub fn reference_referee(&self) -> Option<&Self> {
        match self {
            Self::Constrained { base_type, .. } => base_type.reference_referee(),
            Self::Reference { referee, .. } => Some(referee),
            _ => None,
        }
    }

    pub fn element_type(&self) -> Option<&Self> {
        match self {
            Self::Constrained { base_type, .. } => base_type.element_type(),
            Self::Reference { referee, .. } => referee.element_type(),
            Self::FixedArray { element_type, .. }
            | Self::BoundedByteBuffer { element_type, .. }
            | Self::Slice { element_type } => Some(element_type),
            _ => None,
        }
    }

    /// The arithmetic domain (Wrapping/Saturating/Trapping) declared on this
    /// type via `T in <Domain>`. Defaults to `Exact` for unconstrained types.
    /// Looks through a leading `&`/`&mut` reference so a `&mut (u8 in Saturating)`
    /// target still reports its domain at the binary-write site.
    pub fn arithmetic_domain(&self) -> psi_numerics::arithmetic::ArithmeticDomain {
        match self {
            Self::Constrained { domain, .. } => *domain,
            Self::Reference { referee, .. } => referee.arithmetic_domain(),
            _ => psi_numerics::arithmetic::ArithmeticDomain::Exact,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLayout {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub offset: usize,
    pub type_symbol: SymbolHandle,
    pub type_name: Arc<str>,
    pub type_descriptor: TypeLayoutDescriptor,
    pub layout: TypeLayout,
}

impl Default for FieldLayout {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            offset: 0,
            type_symbol: SymbolHandle::invalid(),
            type_name: Arc::from(""),
            type_descriptor: TypeLayoutDescriptor::default(),
            layout: TypeLayout::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitFieldFragment {
    /// Byte offset relative to the containing record.
    pub container_byte_offset: usize,
    pub container_width_bits: u16,
    pub destination_lsb: u16,
    pub source_lsb: u16,
    pub width: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitFieldLayout {
    pub field: SymbolHandle,
    pub fragments: Vec<BitFieldFragment>,
}

/// Physical integer encoding selected by a validated plan-laid layout. The
/// field's ordinary `FieldLayout` continues to describe its semantic carrier;
/// storage consumers must use this record for the exact load width and
/// extension rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredIntegerLayout {
    pub field: SymbolHandle,
    pub stored_width_bits: u16,
    pub interpretation: psi_layout_plans::IntegerInterpretation,
    pub write_is_total: bool,
}

/// Physical spacing between consecutive elements of one plan-laid outer
/// fixed-array field. The field's ordinary layout retains the semantic array
/// width; only indexing that outer array consumes this stride.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatedFieldLayout {
    pub field: SymbolHandle,
    pub element_stride: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantLayout {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    /// Payload field layouts for this case. Offsets are ABSOLUTE within the
    /// enum value (the tag-prefixed overlay: tag at 0, every case's payload
    /// packed from the shared payload base offset). Empty for payload-less cases.
    pub fields: HandleSpan<FieldLayout>,
}

impl Default for VariantLayout {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            fields: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataShape {
    /// Case-bearing data: pure sums AND mixed shapes (common fields + cases).
    ///
    /// Layout (owned HERE, decided once): the i32 tag sits at offset 0 for
    /// EVERY case-bearing value, the common fields pack immediately after the
    /// tag, and every case's payload fields overlay each other from a shared
    /// base after the common fields. Tag-first (rather than common-fields-
    /// first) is deliberate: the backend's tag-only compares and writes treat
    /// "the first `ENUM_TAG_BYTES` of the value" as the tag WITHOUT consulting
    /// the layout plan, so the tag offset must stay the universal constant 0.
    /// Common-field offsets are still case-independent constants, and ZII
    /// holds: a zeroed value is tag 0 (the first case) with zeroed common
    /// fields and zeroed payload. Pure sums have an empty `common_fields`
    /// span and degenerate to the historical tag-plus-overlay layout.
    Enum {
        common_fields: HandleSpan<FieldLayout>,
        variants: HandleSpan<VariantLayout>,
    },
    Record {
        fields: HandleSpan<FieldLayout>,
    },
}

impl Default for DataShape {
    fn default() -> Self {
        Self::Record {
            fields: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataLayout {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub shape: DataShape,
    pub layout: TypeLayout,
}

impl Default for DataLayout {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            shape: DataShape::default(),
            layout: TypeLayout::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineLayout {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    pub fields: HandleSpan<FieldLayout>,
    pub layout: TypeLayout,
}

/// One semantic-field-free callback destination after the selected target has
/// supplied its function-pointer extent and alignment. The canonical strings
/// remain audit provenance; outbound plan validation consumes only the nominal
/// layout/slot/requirement identities and never the physical offset as an
/// authored calling-plan coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetClosedPrivateCallbackDemand {
    pub data_symbol: SymbolHandle,
    pub slot_identity: Arc<str>,
    pub layout_subject_identity: Arc<str>,
    pub callback_requirement_identity: Arc<str>,
    pub layout: omega_calling_conventions::LayoutPlanId,
    pub slot: omega_calling_conventions::LayoutSlotId,
    pub requirement: omega_calling_conventions::CallbackRequirementId,
    pub offset: usize,
    pub byte_size: usize,
    pub alignment: usize,
}

/// Exact target-closed identity of one plan-laid data layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetClosedPlanLaidDataLayoutIdentity {
    pub data_symbol: SymbolHandle,
    pub data_identity: Arc<str>,
    pub layout_subject_identity: Arc<str>,
    pub layout: omega_calling_conventions::LayoutPlanId,
    pub physical: TypeLayout,
}

/// Exact two-hop proof from a plan-laid root through one inline named record
/// field to a terminal private callback slot in a plan-laid child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetClosedTwoHopPrivateCallbackPath {
    pub root_layout_index: usize,
    pub root_layout: TargetClosedPlanLaidDataLayoutIdentity,
    pub field_symbol: SymbolHandle,
    pub field: psi_arena::Handle<FieldLayout>,
    pub field_layout: FieldLayout,
    pub field_identity: Arc<str>,
    pub field_slot: omega_calling_conventions::LayoutSlotId,
    pub field_relative_offset: usize,
    pub field_extent: usize,
    pub field_alignment: usize,
    pub child_layout_index: usize,
    pub child_layout: TargetClosedPlanLaidDataLayoutIdentity,
    pub terminal_demand_index: usize,
    pub terminal_demand: TargetClosedPrivateCallbackDemand,
    pub composed_offset: usize,
}

impl TargetClosedTwoHopPrivateCallbackPath {
    pub fn native_demand(
        &self,
        parameter: omega_calling_conventions::NativeParameterId,
    ) -> omega_calling_conventions::NativeCallbackDemand {
        omega_calling_conventions::NativeCallbackDemand {
            destination: omega_calling_conventions::NativePlace::Field {
                parameter,
                layout: self.root_layout.layout,
                field_path: vec![self.field_slot, self.terminal_demand.slot],
            },
            requirement: self.terminal_demand.requirement,
        }
    }
}

impl TargetClosedPrivateCallbackDemand {
    pub fn native_demand(
        &self,
        parameter: omega_calling_conventions::NativeParameterId,
    ) -> omega_calling_conventions::NativeCallbackDemand {
        omega_calling_conventions::NativeCallbackDemand {
            destination: omega_calling_conventions::NativePlace::Field {
                parameter,
                layout: self.layout,
                field_path: vec![self.slot],
            },
            requirement: self.requirement,
        }
    }
}

impl Default for MachineLayout {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            attached_data: None,
            fields: HandleSpan::empty(),
            layout: TypeLayout::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutPlan {
    pub data_layouts: Arena<DataLayout>,
    pub fields: Arena<FieldLayout>,
    pub bit_fields: Vec<BitFieldLayout>,
    pub stored_integers: Vec<StoredIntegerLayout>,
    pub repeated_fields: Vec<RepeatedFieldLayout>,
    pub machine_layouts: Arena<MachineLayout>,
    pub variants: Arena<VariantLayout>,
    pub private_callback_demands: Vec<TargetClosedPrivateCallbackDemand>,
    pub plan_laid_layout_identities: Vec<TargetClosedPlanLaidDataLayoutIdentity>,
    pub two_hop_private_callback_paths: Vec<TargetClosedTwoHopPrivateCallbackPath>,
}

impl LayoutPlan {
    pub fn bit_field(&self, field: SymbolHandle) -> Option<&BitFieldLayout> {
        self.bit_fields.iter().find(|layout| layout.field == field)
    }

    pub fn stored_integer(&self, field: SymbolHandle) -> Option<&StoredIntegerLayout> {
        self.stored_integers
            .iter()
            .find(|layout| layout.field == field)
    }

    pub fn repeated_field(&self, field: SymbolHandle) -> Option<&RepeatedFieldLayout> {
        self.repeated_fields
            .iter()
            .find(|layout| layout.field == field)
    }
}

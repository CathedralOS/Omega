/*
We define the layout-plan vocabulary: `LayoutPlan` and the `TypeLayoutDescriptor`,
`FieldLayout`, `BitFieldLayout`, `StoredIntegerLayout`, `RepeatedFieldLayout`,
`DataShape`, `DataLayout`, and target-closed callback demand records it carries.

`TypeLayoutDescriptor::BoundedByteBuffer` represents an owned, variable-fill
bounded byte buffer (`[u8; N] in <text-domain>`) laid out inline as
`{ len, [element; capacity] }` so the text carrier survives after the text
domain is stripped. `DataShape::Enum` places the `i32` case tag at constant
offset 0 for every case-bearing value (pure sums and mixed shapes alike),
followed by common fields and then the shared-base payload overlay, so tag-only
equality compares the first `ENUM_TAG_BYTES` without consulting a layout plan
and zero initialization remains valid.
*/

use arena::{Arena, HandleSpan};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::name::Identifier;

pub(crate) const ENUM_TAG_BYTES: usize = runtime_abi::CASE_TAG_BYTE_SIZE as usize;

pub use layout::TypeLayout;

pub(crate) const fn type_layout(size: usize, alignment: usize) -> TypeLayout {
    TypeLayout { size, alignment }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TypeLayoutDescriptor {
    Reference {
        referee: Box<TypeLayoutDescriptor>,
        is_mutable: bool,
    },
    Constrained {
        base_type: Box<TypeLayoutDescriptor>,
        domain: numerics::arithmetic::ArithmeticDomain,
    },
    FixedArray {
        element_type: Box<TypeLayoutDescriptor>,
        length: usize,
    },
    BoundedByteBuffer {
        element_type: Box<TypeLayoutDescriptor>,
        capacity: usize,
    },
    Slice {
        element_type: Box<TypeLayoutDescriptor>,
    },
    DynamicTrait {
        symbol: SymbolHandle,
        name: Identifier,
    },
    Named {
        symbol: SymbolHandle,
        name: Identifier,
    },
    #[default]
    Unit,
}

impl TypeLayoutDescriptor {
    pub fn inline_data_symbol(&self) -> Option<SymbolHandle> {
        match self {
            Self::Named { symbol, .. } => Some(*symbol),
            Self::Constrained { base_type, .. } => base_type.inline_data_symbol(),
            _ => None,
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
    pub container_byte_offset: usize, // byte offset relative to the containing record
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredIntegerLayout {
    pub field: SymbolHandle,
    pub stored_width_bits: u16,
    pub interpretation: layout_plans::IntegerInterpretation,
    pub write_is_total: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatedFieldLayout {
    pub field: SymbolHandle,
    pub element_stride: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantLayout {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub fields: HandleSpan<FieldLayout>, // absolute offsets within the tag-prefixed enum value
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
pub struct TargetClosedPrivateCallbackDemand {
    pub data_symbol: SymbolHandle,
    pub slot_application: language_semantics::callback_declarations::PrivateCallbackSlotSelection,
    pub slot_identity: Arc<str>,
    pub layout_subject_identity: Arc<str>,
    pub callback_requirement_identity: Arc<str>,
    pub layout: calling_conventions::LayoutPlanId,
    pub slot: calling_conventions::LayoutSlotId,
    pub requirement: calling_conventions::CallbackRequirementId,
    pub placements: calling_conventions::PrivateCallbackPlacements,
    pub byte_size: usize,
    pub alignment: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetClosedDataLayoutIdentity {
    pub data_symbol: SymbolHandle,
    pub data_identity: Arc<str>,
    pub layout_subject_identity: Option<Arc<str>>,
    pub layout: calling_conventions::LayoutPlanId,
    pub physical: TypeLayout,
}

impl TargetClosedPrivateCallbackDemand {
    pub fn native_demand(
        &self,
        parameter: calling_conventions::NativeParameterId,
    ) -> calling_conventions::NativeCallbackDemand {
        calling_conventions::NativeCallbackDemand {
            destination: calling_conventions::NativePlace::Field {
                parameter,
                layout: self.layout,
                field_path: vec![self.slot],
            },
            requirement: self.requirement,
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
    pub variants: Arena<VariantLayout>,
    pub private_callback_demands: Vec<TargetClosedPrivateCallbackDemand>,
    pub data_layout_identities: Vec<TargetClosedDataLayoutIdentity>,
    pub private_callback_layouts: Vec<crate::data_layout_queries::PrivateCallbackLayout>,
}

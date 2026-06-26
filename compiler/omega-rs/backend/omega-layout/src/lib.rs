use omega_checked_trees::name::Identifier;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use std::sync::Arc;

mod builder;
mod packing;
mod sizing;

pub use builder::build_layout_plan;

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
        domain: omega_core::arithmetic::ArithmeticDomain,
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
        symbol: SymbolHandle,
        name: Identifier,
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
    pub fn arithmetic_domain(&self) -> omega_core::arithmetic::ArithmeticDomain {
        match self {
            Self::Constrained { domain, .. } => *domain,
            Self::Reference { referee, .. } => referee.arithmetic_domain(),
            _ => omega_core::arithmetic::ArithmeticDomain::Exact,
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
    pub machine_layouts: Arena<MachineLayout>,
    pub variants: Arena<VariantLayout>,
}

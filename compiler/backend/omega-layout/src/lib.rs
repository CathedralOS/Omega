use omega_checked_trees::name::ProgramName;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use std::sync::Arc;

mod builder;
mod packing;
mod sizing;

pub use builder::build_layout_plan;

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
    },
    FixedArray {
        element_type: Box<TypeLayoutDescriptor>,
        length: usize,
    },
    Slice {
        element_type: Box<TypeLayoutDescriptor>,
    },
    Named {
        symbol: SymbolHandle,
        name: ProgramName,
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
            Self::Constrained { base_type } => base_type.storage_symbol(),
            Self::FixedArray { element_type, .. } => element_type.storage_symbol(),
            Self::Slice { element_type } => element_type.storage_symbol(),
            Self::Named { symbol, .. } => *symbol,
            Self::Unit => SymbolHandle::invalid(),
        }
    }

    pub fn fixed_array(&self) -> Option<(&Self, usize)> {
        match self {
            Self::Constrained { base_type } => base_type.fixed_array(),
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
            Self::Constrained { base_type } => base_type.reference_referee(),
            Self::Reference { referee, .. } => Some(referee),
            _ => None,
        }
    }

    pub fn element_type(&self) -> Option<&Self> {
        match self {
            Self::Constrained { base_type } => base_type.element_type(),
            Self::Reference { referee, .. } => referee.element_type(),
            Self::FixedArray { element_type, .. } | Self::Slice { element_type } => {
                Some(element_type)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLayout {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
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
            name: ProgramName::default(),
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
    pub name: ProgramName,
}

impl Default for VariantLayout {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataShape {
    Enum { variants: HandleSpan<VariantLayout> },
    Record { fields: HandleSpan<FieldLayout> },
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
    pub name: ProgramName,
    pub shape: DataShape,
    pub layout: TypeLayout,
}

impl Default for DataLayout {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            shape: DataShape::default(),
            layout: TypeLayout::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineLayout {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub fields: HandleSpan<FieldLayout>,
    pub layout: TypeLayout,
}

impl Default for MachineLayout {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
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

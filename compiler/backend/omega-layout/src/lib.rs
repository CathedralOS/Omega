use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::name::ProgramName;

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
pub struct FieldLayout {
    pub name: ProgramName,
    pub offset: usize,
    pub type_name: String,
    pub layout: TypeLayout,
}

impl Default for FieldLayout {
    fn default() -> Self {
        Self {
            name: ProgramName::default(),
            offset: 0,
            type_name: String::new(),
            layout: TypeLayout::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataShape {
    Enum { variants: Vec<ProgramName> },
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
    pub name: ProgramName,
    pub shape: DataShape,
    pub layout: TypeLayout,
}

impl Default for DataLayout {
    fn default() -> Self {
        Self {
            name: ProgramName::default(),
            shape: DataShape::default(),
            layout: TypeLayout::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineLayout {
    pub name: ProgramName,
    pub fields: HandleSpan<FieldLayout>,
    pub layout: TypeLayout,
}

impl Default for MachineLayout {
    fn default() -> Self {
        Self {
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
}

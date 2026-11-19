//! Scratch tables used only while native layout prepares a boundary signature.

use arena::{Arena, Handle, HandleSpan};

#[derive(Default)]
pub struct BoundaryValueRecipes {
    pub(super) values: Arena<BoundaryValueRecipe>,
    pub(super) fields: Arena<BoundaryFieldRecipe>,
}

#[derive(Default)]
pub enum BoundaryValueRecipe {
    #[default]
    Absent,
    Integer {
        byte_size: u16,
    },
    Float {
        byte_size: u16,
    },
    Reference {
        descriptor: bool,
    },
    FixedArray {
        element: Handle<Self>,
        length: u64,
    },
    Record {
        name: String,
        fields: HandleSpan<BoundaryFieldRecipe>,
        storage: Option<BoundaryRecordStorage>,
    },
    Opaque {
        name: String,
        carrier_name: String,
        selection_ordinal: usize,
        carrier: Handle<Self>,
    },
}

#[derive(Default)]
pub struct BoundaryFieldRecipe {
    pub value: Handle<BoundaryValueRecipe>,
    pub byte_offset: Option<u64>,
}

pub struct BoundaryRecordStorage {
    pub byte_size: u64,
    pub byte_alignment: u64,
}

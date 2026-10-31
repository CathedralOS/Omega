//! Target-neutral storage recipes for exact boundary value occurrences.
//!
//! Psi closes source applications into this graph. Omega supplies address
//! geometry and packing, then classifies the resulting graph for an ABI. No
//! source handle or target choice occurs here. Opaque selection ordinals refer
//! to the explicitly supplied representation roster, not executable authority.

use arena::{Arena, Handle, HandleSpan};

#[derive(Default)]
pub struct BoundaryValueRecipes {
    pub values: Arena<BoundaryValueRecipe>,
    pub fields: Arena<BoundaryFieldRecipe>,
    pub targets: Arena<BoundaryTargetRecipe>,
}

#[derive(Default)]
pub enum BoundaryValueRecipe {
    #[default]
    Absent,
    Rejected(String),
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
    TargetLayouts {
        name: String,
        alternatives: HandleSpan<BoundaryTargetRecipe>,
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
    /// An invariant authored layout fixes the offset; ordinary fields leave
    /// packing to the target owner. Erased members have no storage recipe.
    pub byte_offset: Option<u64>,
}

pub struct BoundaryRecordStorage {
    pub byte_size: u64,
    pub byte_alignment: u64,
}

#[derive(Default)]
pub struct BoundaryTargetRecipe {
    /// Exact deployment identity retained as data. Psi never picks a member.
    pub target: String,
    pub value: Handle<BoundaryValueRecipe>,
}

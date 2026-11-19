//! Default integer storage geometry for reflection and native calls.
#![forbid(unsafe_code)]
use layout_plans::StorageGeometry;
pub use layout_plans::packing::{conventional_record_geometry, conventional_sum_geometry};
use semantic_vocabulary::ClosedIntegerType;

// Schema reflection and native placement must report the same chosen
// integer storage. These are the current default realizations, not a
// language promise about the alignment of every representation.
pub fn standalone_integer_geometry(integer: ClosedIntegerType) -> StorageGeometry {
    integer_geometry(integer, 8)
}

pub fn field_integer_geometry(integer: ClosedIntegerType) -> StorageGeometry {
    integer_geometry(integer, 16)
}

fn integer_geometry(integer: ClosedIntegerType, alignment_cap: u16) -> StorageGeometry {
    let bytes = integer.bits().div_ceil(8);
    StorageGeometry {
        byte_size: u64::from(bytes),
        byte_alignment: u64::from(bytes.next_power_of_two().min(alignment_cap)),
    }
}

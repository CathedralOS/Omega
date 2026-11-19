//! Default integer storage geometry for reflection and native calls.
#![forbid(unsafe_code)]
use layout_plans::StorageGeometry;
pub use layout_plans::packing::{conventional_record_geometry, conventional_sum_geometry};

/// A standalone integer uses the default alignment up to eight bytes.
/// The current compiler can supply closed integer widths from 1 through 128 bits.
pub fn standalone_integer_geometry(bits: u16) -> Option<StorageGeometry> {
    integer_geometry(bits, 8)
}

/// A record field uses the same storage size, with alignment up to sixteen bytes.
pub fn field_integer_geometry(bits: u16) -> Option<StorageGeometry> {
    integer_geometry(bits, 16)
}

fn integer_geometry(bits: u16, alignment_cap: u16) -> Option<StorageGeometry> {
    if !(1..=128).contains(&bits) {
        return None;
    }
    let bytes = bits.div_ceil(8);
    Some(StorageGeometry {
        byte_size: u64::from(bytes),
        byte_alignment: u64::from(bytes.next_power_of_two().min(alignment_cap)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_closed_host_widths_have_default_storage() {
        for bits in [0, 129, u16::MAX] {
            assert_eq!(standalone_integer_geometry(bits), None);
            assert_eq!(field_integer_geometry(bits), None);
        }
        for (bits, size, standalone, field) in [
            (1, 1, 1, 1),
            (17, 3, 4, 4),
            (65, 9, 8, 16),
            (128, 16, 8, 16),
        ] {
            assert_eq!(
                standalone_integer_geometry(bits),
                Some(StorageGeometry {
                    byte_size: size,
                    byte_alignment: standalone
                })
            );
            assert_eq!(
                field_integer_geometry(bits),
                Some(StorageGeometry {
                    byte_size: size,
                    byte_alignment: field
                })
            );
        }
    }
}

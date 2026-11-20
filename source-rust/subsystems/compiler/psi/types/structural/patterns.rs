//! Public pattern macros for structural shapes.

#[macro_export]
macro_rules! record_or_mixed_shape_pattern {
    ($fields:ident) => {
        $crate::StructuralTypeShape::Record {
            fields: $fields,
            ..
        } | $crate::StructuralTypeShape::Mixed {
            fields: $fields,
            ..
        }
    };
}

#[macro_export]
macro_rules! record_or_fixed_array_shape_pattern {
    () => {
        $crate::StructuralTypeShape::Record { .. }
            | $crate::StructuralTypeShape::FixedArray { .. }
    };
}

#[macro_export]
macro_rules! byte_sequence_view_carrier_pattern {
    () => {
        $crate::ByteSequenceCarrier::BorrowedView { .. }
            | $crate::ByteSequenceCarrier::BoundedView { .. }
    };
}

#[macro_export]
macro_rules! sum_or_mixed_shape_pattern {
    () => {
        $crate::StructuralTypeShape::Sum { .. } | $crate::StructuralTypeShape::Mixed { .. }
    };
}

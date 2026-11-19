#[macro_export]
macro_rules! static_call_structural_argument_variants {
    ($arguments:ident) => {
        $crate::OperationKind::CallUnit {
            structural_arguments: $arguments,
            ..
        } | $crate::OperationKind::CallStructuralScalar {
            structural_arguments: $arguments,
            ..
        } | $crate::OperationKind::CallStructural {
            structural_arguments: $arguments,
            ..
        } | $crate::OperationKind::CallStructuralWithScalarArguments {
            structural_arguments: $arguments,
            ..
        } | $crate::OperationKind::BoundaryCall {
            structural_arguments: $arguments,
            ..
        }
    };
}

#[macro_export]
macro_rules! call_structural_argument_variants {
    ($arguments:ident) => {
        $crate::static_call_structural_argument_variants!($arguments)
            | $crate::OperationKind::CallDynamic {
                structural_arguments: $arguments,
                ..
            }
            | $crate::OperationKind::CallMachine {
                structural_arguments: $arguments,
                ..
            }
    };
}

#[macro_export]
macro_rules! integer_arithmetic_operations {
    () => {
        $crate::OperationKind::ExactIntegerAdd { .. }
            | $crate::OperationKind::WrappingIntegerAdd { .. }
            | $crate::OperationKind::ExactIntegerSubtract { .. }
            | $crate::OperationKind::WrappingIntegerSubtract { .. }
            | $crate::OperationKind::ExactIntegerMultiply { .. }
            | $crate::OperationKind::WrappingIntegerMultiply { .. }
            | $crate::OperationKind::ExactIntegerDivide { .. }
            | $crate::OperationKind::ExactIntegerRemainder { .. }
            | $crate::OperationKind::WrappingIntegerDivide { .. }
            | $crate::OperationKind::WrappingIntegerRemainder { .. }
            | $crate::OperationKind::SaturatingIntegerDivide { .. }
            | $crate::OperationKind::SaturatingIntegerRemainder { .. }
            | $crate::OperationKind::SaturatingIntegerAdd { .. }
            | $crate::OperationKind::SaturatingIntegerSubtract { .. }
            | $crate::OperationKind::SaturatingIntegerMultiply { .. }
    };
}

#[macro_export]
macro_rules! move_or_store_structural_field_pattern {
    () => {
        $crate::OperationKind::MoveStructuralField { .. }
            | $crate::OperationKind::StoreStructuralField { .. }
    };
}

#[macro_export]
macro_rules! structural_leaf_copy_pair_pattern {
    () => {
        $crate::OperationKind::StructuralLeafCopy { .. }
            | $crate::OperationKind::StructuralCaseLeafCopy { .. }
    };
}

#[macro_export]
macro_rules! call_structural_pair_pattern {
    ($($fields:tt)*) => {
        $crate::OperationKind::CallStructural { $($fields)* }
            | $crate::OperationKind::CallStructuralWithScalarArguments { $($fields)* }
    };
}

#[macro_export]
macro_rules! integer_less_or_equal_pair_pattern {
    ($($fields:tt)*) => {
        $crate::OperationKind::IntegerLessThan { $($fields)* }
            | $crate::OperationKind::IntegerLessOrEqual { $($fields)* }
    };
}

#[macro_export]
macro_rules! element_view_establish_pair_pattern {
    ($($fields:tt)*) => {
        $crate::OperationKind::EstablishElementView { $($fields)* }
            | $crate::OperationKind::EstablishMutableElementView { $($fields)* }
    };
}

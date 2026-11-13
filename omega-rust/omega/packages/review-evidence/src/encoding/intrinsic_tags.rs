// Emission and recovery share one intrinsic vocabulary. Both the execution-to-tag
// match and recovery's tag match must stay exhaustive when a variant is added.

use crate::record::PackageReviewCompilerIntrinsicExecution as Execution;

macro_rules! intrinsic_tags {
    ($($variant:ident ($pattern:pat) = $code:literal => $label:literal),+ $(,)?) => {
        #[derive(Clone, Copy)]
        #[repr(u8)]
        pub(super) enum IntrinsicTag {
            $($variant = $code,)+
        }

        impl IntrinsicTag {
            pub(super) fn of(execution: &Execution) -> Self {
                match execution {
                    $($pattern => Self::$variant,)+
                }
            }

            pub(super) fn from_code(code: u8) -> Option<Self> {
                match code {
                    $($code => Some(Self::$variant),)+
                    _ => None,
                }
            }

            pub(super) fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label,)+
                }
            }
        }
    };
}

intrinsic_tags! {
    BuiltinFunction(Execution::BuiltinFunction(_)) = 0 => "builtin_function",
    NamedFloatNegation(Execution::NamedFloatNegation(_)) = 1 => "named_float_negation",
    NamedFloatConversion(Execution::NamedFloatConversion { .. }) = 2 => "named_float_conversion",
    PrimitiveFloatBinary(Execution::PrimitiveFloatBinary { .. }) = 3 => "primitive_float_binary",
    HostedExitProcessI32(Execution::HostedExitProcessI32) = 4 => "hosted_exit_process_i32",
    HostedWriteByteI32(Execution::HostedWriteByteI32) = 5 => "hosted_write_byte_i32",
    HostedReadByte(Execution::HostedReadByte) = 6 => "hosted_read_byte",
    PrimitiveIntegerComparison(Execution::PrimitiveIntegerComparison { .. }) = 7 => "primitive_integer_comparison",
    AccessorScalarAccess(Execution::AccessorScalarAccess { .. }) = 8 => "accessor_scalar_access",
    ProgramImageRetainedSuffixStart(Execution::ProgramImageRetainedSuffixStart) = 9 => "program_image_retained_suffix_start",
    ProgramStorageEntry(Execution::ProgramStorageEntry) = 10 => "program_storage_entry",
    AccessLifecycle(Execution::AccessLifecycle(_)) = 11 => "access_lifecycle",
    TaskRegisterActivate(Execution::TaskRegisterActivate) = 12 => "task_register_activate",
    TaskRegisterRead(Execution::TaskRegisterRead) = 13 => "task_register_read",
    SegmentReload(Execution::SegmentReload(_)) = 14 => "segment_reload",
    SegmentRead(Execution::SegmentRead(_)) = 15 => "segment_read",
    EmptyRunRelease(Execution::EmptyRunRelease) = 16 => "empty_run_release",
    StaticInterrupt(Execution::StaticInterrupt(_)) = 17 => "static_interrupt",
}

//! Package-relative identities used by the declaration catalogs. Only the source
//! producer embeds the files and compares their complete bytes.

#[macro_export]
macro_rules! bundled_library_files {
    ($catalog:ident) => {
        $catalog! {
            CoreAccess => "core/access/access.omg",
            CoreActivation => "core/activation.omg",
            CoreAssembly => "core/assembly.omg",
            CoreAssemblyModel => "core/assembly_model.omg",
            CoreAtomic => "core/Atomic/atomic.omg",
            CoreByteObservation => "core/byte_observation/byte_observation.omg",
            CoreCodeLayout => "core/code_layout.omg",
            CoreCodeSegment => "core/code_segment.omg",
            CoreContent => "core/content.omg",
            CoreDataSegment => "core/data_segment.omg",
            CoreEquality => "core/equality.omg",
            CoreExtent => "core/extent.omg",
            CoreExternalBinding => "core/external_binding.omg",
            CoreFilesystemAuthority => "core/filesystem_authority.omg",
            CoreFixedVec => "core/fixed_vec.omg",
            CoreFloatFormat => "core/float_format.omg",
            CoreFloatMeaning => "core/float_meaning.omg",
            CoreFloatOperations => "core/float_operations.omg",
            CoreFloatProvider => "core/float_provider.omg",
            CoreIndexing => "core/indexing/indexing.omg",
            CoreInteger => "core/integer/integer.omg",
            CoreInterrupt => "core/interrupt.omg",
            CoreLayout => "core/layout.omg",
            CoreNat => "core/nat.omg",
            CoreNever => "core/never.omg",
            CoreProcessAuthority => "core/process_authority.omg",
            CoreProcessExit => "core/process_exit.omg",
            CoreProcessExitNative => "core/process_exit_native.omg",
            CoreProgramImage => "core/program_image.omg",
            CoreReflect => "core/reflect/reflect.omg",
            CoreRelation => "core/relation.omg",
            CoreRepresentation => "core/representation.omg",
            CoreRun => "core/run.omg",
            CoreStaticInterrupt => "core/static_interrupt.omg",
            CoreTask => "core/task.omg",
            CoreTaskRegister => "core/task_register.omg",
        }
    };
}

macro_rules! define_files {
    ($($variant:ident => $path:literal,)+) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum BundledFile { $($variant,)+ }
    };
}
bundled_library_files!(define_files);

/// Retained declarations refer to these core owners by their canonical revision.
/// The producer tests every revision against its exact installed library bytes.
pub mod core_revision {
    pub const EXTENT: [u8; 32] = [
        0x35, 0xc8, 0xee, 0xa1, 0x01, 0xe3, 0x3b, 0xf7, 0x93, 0xdb, 0xde, 0x34, 0xeb, 0x71, 0x36,
        0xe3, 0x6d, 0xe9, 0xd7, 0xcd, 0xcc, 0xd2, 0x02, 0x30, 0xa4, 0x37, 0xee, 0xe9, 0xa6, 0xe3,
        0xd5, 0xd7,
    ];
    pub const PROGRAM_IMAGE: [u8; 32] = [
        0x0e, 0x18, 0xbd, 0xb2, 0x2b, 0xf0, 0x11, 0x82, 0x9c, 0x67, 0xf9, 0xab,
        0x7d, 0x9a, 0x37, 0xe8, 0x5b, 0x7f, 0x93, 0x22, 0xff, 0x4e, 0x8e, 0x1d,
        0xd4, 0xa6, 0x0c, 0xd6, 0xa0, 0x6d, 0x99, 0x52,
    ];
    pub const LAYOUT: [u8; 32] = [
        0xf6, 0xf8, 0x28, 0xcf, 0x82, 0x63, 0xf7, 0x7d, 0x12, 0x2d, 0x88, 0x8a, 0x51, 0xaa, 0x85,
        0x1e, 0xc8, 0x1b, 0xcb, 0x41, 0x96, 0x4a, 0xb5, 0x2a, 0x92, 0x1d, 0xd4, 0x06, 0x98, 0xaf,
        0xc5, 0xdf,
    ];
    pub const FILESYSTEM_AUTHORITY: [u8; 32] = [
        0x64, 0xae, 0xc9, 0x29, 0x68, 0xd5, 0xd4, 0x72, 0xa5, 0x59, 0x1b, 0x60, 0x7a, 0x38, 0xc0,
        0x8a, 0x9b, 0x0d, 0xf3, 0xca, 0x0d, 0xc1, 0x52, 0x7a, 0x07, 0xd0, 0x24, 0x29, 0xad, 0x73,
        0xd6, 0x87,
    ];
    pub const FLOAT_OPERATIONS: [u8; 32] = [
        0xa6, 0x11, 0x99, 0xcb, 0xb6, 0x8c, 0xd7, 0x76, 0x63, 0x08, 0x01, 0x25, 0x5b, 0x5c, 0x18,
        0x01, 0xbb, 0x8b, 0x05, 0x33, 0x49, 0xc8, 0x13, 0x78, 0x25, 0xf3, 0xe6, 0x18, 0x18, 0x7c,
        0x37, 0x6f,
    ];
    pub const PROCESS_EXIT_NATIVE: [u8; 32] = [
        0x4f, 0x49, 0x13, 0xb4, 0x98, 0xb5, 0x61, 0x62, 0x97, 0x58, 0x7e, 0x20, 0xfa, 0x2c, 0xab,
        0x63, 0x54, 0x4a, 0x19, 0xed, 0x63, 0xc2, 0x19, 0x7d, 0x94, 0xe7, 0x38, 0xff, 0x60, 0xa5,
        0x29, 0xac,
    ];
    pub const PROCESS_EXIT: [u8; 32] = [
        0x19, 0x79, 0xca, 0x9e, 0x48, 0xa8, 0xb7, 0x4e, 0x87, 0xa6, 0xce, 0x22, 0x85, 0xac, 0x35,
        0x81, 0x4c, 0x5e, 0xf9, 0xf0, 0xf3, 0xf4, 0xb5, 0x5d, 0x6e, 0xb2, 0x58, 0xc9, 0xd9, 0xe9,
        0x30, 0xce,
    ];
}

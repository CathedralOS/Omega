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
            CoreAssemblyPackedInteger => "core/assembly_packed_integer.omg",
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
            CoreFoundation => "core/foundation.omg",
            CoreIndexing => "core/indexing/indexing.omg",
            CoreInteger => "core/integer/integer.omg",
            CoreInterrupt => "core/interrupt.omg",
            CoreLayout => "core/layout.omg",
            CoreNat => "core/nat.omg",
            CoreNativeExecutionAuthority => "core/native_execution_authority.omg",
            CoreNetworkAuthority => "core/network_authority.omg",
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
            CoreUnsafe => "core/unsafe.omg",
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

/// Expected source revisions for the core owners retained in saved programs.
pub mod core_revision {
    include!(concat!(env!("OUT_DIR"), "/core_revisions.rs"));
}

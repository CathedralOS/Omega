//! Exact library bytes used to recognize declarations after source admission
//! establishes toolchain ownership.
use language_core::bundled_files::{BuildCompanionFile, BundledFile};

pub mod core_vocabulary;
pub mod sealed_intrinsics;

macro_rules! source_bytes {
    ($($variant:ident => $path:literal,)+) => {
        pub const fn bytes(file: BundledFile) -> &'static [u8] {
            match file {
                $(BundledFile::$variant => include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"), "/../../../../../../../source/library/", $path
                )),)+
            }
        }
    };
}
language_core::bundled_library_files!(source_bytes);

/// Recognize an admitted core source by its package-relative path and exact
/// bundled bytes. Callers must first verify toolchain origin and that the file
/// belongs to no admitted package graph.
pub fn recognize_core_file(relative_path: &std::path::Path, source: &[u8]) -> Option<BundledFile> {
    let file = BundledFile::from_core_relative_path(relative_path)?;
    (bytes(file) == source).then_some(file)
}

/// Exact installed bytes of a Build companion file.
pub const fn build_companion_bytes(file: BuildCompanionFile) -> &'static [u8] {
    match file {
        BuildCompanionFile::SandboxStreams => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../../source/library/core/sandbox/streams.omg"
        )),
        BuildCompanionFile::BuildProgram => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../../source/library/build/program.omg"
        )),
        BuildCompanionFile::SystemFilesystem => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../../source/library/system/filesystem.omg"
        )),
    }
}

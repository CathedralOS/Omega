//! Exact library bytes used to recognize declarations after source admission
//! establishes toolchain ownership.
use language_core::bundled_files::BundledFile;

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

/// Exact installed declarations for the Build interpreter’s designated standard streams.
pub const BUILD_STREAMS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../../source/library/core/sandbox/streams.omg"
));

/// Host recognition first checks the installed Build-file origin and path.
pub const BUILD_PROGRAM: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../../source/library/build/program.omg"
));

/// A Build may bind this requirement only in the installed system package.
pub const SYSTEM_FILESYSTEM: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../../source/library/system/filesystem.omg"
));

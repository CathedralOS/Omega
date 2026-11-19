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

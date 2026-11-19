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
                    env!("CARGO_MANIFEST_DIR"), "/../../../../../source/library/", $path
                )),)+
            }
        }
    };
}
language_core::bundled_library_files!(source_bytes);

#[cfg(test)]
mod tests {
    use super::*;
    use language_core::bundled_files::core_revision;

    #[test]
    fn saved_program_owners_match_the_producers_canonical_revisions() {
        for (file, name, expected) in [
            (
                BundledFile::CoreFilesystemAuthority,
                "filesystem_authority.omg",
                core_revision::FILESYSTEM_AUTHORITY,
            ),
            (
                BundledFile::CoreFloatOperations,
                "float_operations.omg",
                core_revision::FLOAT_OPERATIONS,
            ),
            (
                BundledFile::CoreProcessExitNative,
                "process_exit_native.omg",
                core_revision::PROCESS_EXIT_NATIVE,
            ),
            (
                BundledFile::CoreProcessExit,
                "process_exit.omg",
                core_revision::PROCESS_EXIT,
            ),
        ] {
            let actual = declaration_contracts::source_identity::toolchain_owned_identity_digest(
                "core",
                vec![name.to_owned()],
                bytes(file),
            )
            .unwrap();
            assert_eq!(actual, expected, "{name}");
        }
    }
}

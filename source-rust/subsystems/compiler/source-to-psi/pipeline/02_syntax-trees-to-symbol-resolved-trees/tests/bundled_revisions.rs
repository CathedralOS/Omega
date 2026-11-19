use bundled_declarations::bytes;
use language_core::bundled_files::{BundledFile, core_revision};

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

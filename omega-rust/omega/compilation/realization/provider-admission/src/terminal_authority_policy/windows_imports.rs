//! Classify reviewed Windows operations without relying on their authored names.
//!
//! Error-state access, local handle representation and text conversion exercise
//! no closed terminal class. File-information queries remain metadata queries;
//! querying an I/O mode does not justify an empty row for every information
//! class. Generic reads can consume console input and echo it to output; generic
//! writes can produce console output. Their rows include those classes beside
//! content access. The admitted calling plan remains part of every emitted key;
//! storage custody remains separate.
//!
//! The portable open cohort conservatively permits all six filesystem facets.
//! Neither an attribute-only spelling nor its declared reach narrows CreateFileW
//! here. DuplicateHandle may close its source, and CloseHandle accepts objects
//! beyond files: both stay unclassified until checked argument/lifecycle
//! evidence establishes a distinct constrained occurrence. Generic CRT reads
//! and writes also remain unclassified: their descriptors can name consoles,
//! and invalid arguments can dispatch an application-selected parameter handler.
//! Neither a filesystem-only row nor a guessed handler union is justified.

use std::sync::OnceLock;

use effects::{
    NormalizedForeignArgumentContract, NormalizedForeignTerminalMechanismIdentity,
    PortableFilesystemAuthorityFacet as Facet, TerminalAuthorityClass as Class,
    TerminalAuthorityDisposition,
};

use super::TerminalAuthorityPolicyRow;

struct ReviewedImport {
    locator: target::ForeignLocatorIdentityDigest,
    disposition: TerminalAuthorityDisposition,
}

/// Fixed native API meanings, not grants derived from source effects. The
/// import's complete normalized locator includes target and case-sensitive
/// export bytes. An unrecognized DLL/export never supplies a row.
fn reviewed_imports() -> &'static [ReviewedImport] {
    static IMPORTS: OnceLock<Vec<ReviewedImport>> = OnceLock::new();
    IMPORTS.get_or_init(|| {
        let filesystem = Facet::ALL.map(Class::from);
        let imports: &[(&[u8], &[u8], &[Class])] = &[
            (b"kernel32.dll", b"GetLastError", &[]),
            (b"kernel32.dll", b"SetLastError", &[]),
            (b"kernel32.dll", b"GetCurrentProcess", &[]),
            (b"ntdll.dll", b"RtlNtStatusToDosError", &[]),
            (b"kernel32.dll", b"WideCharToMultiByte", &[]),
            // Preserve the existing Time and Console cohort coverage while
            // closing the Windows path to method-name classification.
            (b"kernel32.dll", b"Sleep", &[]),
            (
                b"kernel32.dll",
                b"ExitProcess",
                &[Class::ProcessTermination],
            ),
            (b"kernel32.dll", b"CreateFileW", &filesystem),
            (b"ntdll.dll", b"NtOpenFile", &filesystem),
            (
                b"kernel32.dll",
                b"ReadFile",
                &[
                    Class::FilesystemContentRead,
                    Class::ProcessInput,
                    Class::ProcessOutput,
                ],
            ),
            (
                b"kernel32.dll",
                b"WriteFile",
                &[Class::FilesystemContentWrite, Class::ProcessOutput],
            ),
            (
                b"ntdll.dll",
                b"NtQueryInformationFile",
                &[Class::FilesystemMetadataQuery],
            ),
            (
                b"kernel32.dll",
                b"GetFinalPathNameByHandleW",
                &[Class::FilesystemMetadataQuery],
            ),
            (
                b"kernel32.dll",
                b"CreateDirectoryW",
                &[
                    Class::FilesystemNamespaceMutation,
                    Class::FilesystemMetadataMutation,
                ],
            ),
            (
                b"kernel32.dll",
                b"DeleteFileW",
                &[Class::FilesystemNamespaceMutation],
            ),
            (
                b"kernel32.dll",
                b"RemoveDirectoryW",
                &[Class::FilesystemNamespaceMutation],
            ),
        ];
        imports
            .iter()
            .map(|&(library, export, classes)| {
                let locator = target::normalize_foreign_locator(
                    target::ForeignLocatorCandidate::PeByName {
                        library: library.to_vec(),
                        export: export.to_vec(),
                    },
                    target::TargetProfile::WindowsX64,
                )
                .expect("reviewed Windows API locator");
                ReviewedImport {
                    locator: locator.identity_digest(),
                    disposition: TerminalAuthorityDisposition::from_classes(
                        classes.iter().copied(),
                    ),
                }
            })
            .collect()
    })
}

pub(super) fn mechanism_row(
    foreign: NormalizedForeignTerminalMechanismIdentity,
) -> Option<TerminalAuthorityPolicyRow> {
    if foreign.target() != target::TargetProfile::WindowsX64
        || foreign.implementation_contract().is_zero()
        || foreign.argument_contract() != NormalizedForeignArgumentContract::AdmittedPlan
    {
        return None;
    }
    let import = reviewed_imports()
        .iter()
        .find(|import| import.locator == foreign.locator_identity())?;
    Some(TerminalAuthorityPolicyRow::new(
        foreign.into(),
        import.disposition.clone(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use effects::provider_plan::BoundaryCallingPlanCommitment;
    use effects::{CheckedSyscallArgumentContractIdentity, TerminalAuthorityClass};

    fn imported(
        library: &[u8],
        export: &[u8],
        contract: u8,
    ) -> NormalizedForeignTerminalMechanismIdentity {
        let locator = target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::PeByName {
                library: library.to_vec(),
                export: export.to_vec(),
            },
            target::TargetProfile::WindowsX64,
        )
        .unwrap();
        NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
            &locator,
            BoundaryCallingPlanCommitment::from_digest([contract; 32]),
        )
    }

    #[test]
    fn reviewed_error_observation_is_explicitly_empty() {
        let mechanism = imported(b"kernel32.dll", b"GetLastError", 1);
        let row = mechanism_row(mechanism).unwrap();
        assert_eq!(row.mechanism, mechanism.into());
        assert!(row.disposition.classes().is_empty());
    }

    #[test]
    fn wrong_dll_export_and_unproved_handle_operations_stay_unknown() {
        for (library, export) in [
            (b"other.dll".as_slice(), b"GetLastError".as_slice()),
            (b"kernel32.dll", b"getlasterror"),
            (b"kernel32.dll", b"UnknownOperation"),
            (b"kernel32.dll", b"DuplicateHandle"),
            (b"kernel32.dll", b"CloseHandle"),
        ] {
            assert!(mechanism_row(imported(library, export, 1)).is_none());
        }
    }

    #[test]
    fn metadata_queries_do_not_inherit_an_empty_declared_reach() {
        let row = mechanism_row(imported(b"ntdll.dll", b"NtQueryInformationFile", 1)).unwrap();
        assert_eq!(
            row.disposition.classes(),
            [TerminalAuthorityClass::FilesystemMetadataQuery]
        );
        let open = mechanism_row(imported(b"kernel32.dll", b"CreateFileW", 1)).unwrap();
        assert_eq!(open.disposition.classes().len(), 6);
    }

    #[test]
    fn parent_dispatch_preserves_reviewed_windows_cohort_coverage() {
        use effects::provider_plan::ServiceMethod;

        let method = ServiceMethod {
            name: "read".to_owned(),
            ..Default::default()
        };
        let read = imported(b"kernel32.dll", b"ReadFile", 1);
        let row =
            super::super::cohort_mechanism_row(read.into(), &Default::default(), &method).unwrap();
        assert_eq!(row.mechanism, read.into());
        assert!(
            row.disposition
                .classes()
                .contains(&Class::FilesystemContentRead)
        );
        assert!(row.disposition.classes().contains(&Class::ProcessInput));
        assert!(row.disposition.classes().contains(&Class::ProcessOutput));
        assert!(
            super::super::cohort_mechanism_row(
                imported(b"other.dll", b"ReadFile", 1).into(),
                &Default::default(),
                &method
            )
            .is_none()
        );

        let method = ServiceMethod {
            name: "open".to_owned(),
            ..Default::default()
        };
        let open = imported(b"ntdll.dll", b"NtOpenFile", 1);
        let row =
            super::super::cohort_mechanism_row(open.into(), &Default::default(), &method).unwrap();
        assert_eq!(row.mechanism, open.into());
        assert_eq!(
            row.disposition,
            TerminalAuthorityDisposition::from_filesystem_facets(Facet::ALL)
        );

        // These actual std imports previously entered the Time and Console
        // cohorts after filesystem lookup. Preserve their physical meanings
        // while preventing those method names from classifying other exports.
        for (name, export, classes) in [
            ("sleep", b"Sleep".as_slice(), &[][..]),
            ("exit_process", b"ExitProcess", &[Class::ProcessTermination]),
        ] {
            let method = ServiceMethod {
                name: name.to_owned(),
                ..Default::default()
            };
            let row = super::super::cohort_mechanism_row(
                imported(b"kernel32.dll", export, 1).into(),
                &Default::default(),
                &method,
            )
            .unwrap();
            assert_eq!(
                row.disposition,
                TerminalAuthorityDisposition::from_classes(classes.iter().copied())
            );
            assert!(
                super::super::cohort_mechanism_row(
                    imported(b"other.dll", export, 1).into(),
                    &Default::default(),
                    &method,
                )
                .is_none()
            );
        }
    }

    #[test]
    fn unconstrained_console_capable_imports_cannot_earn_filesystem_only_admission() {
        let filesystem_only = TerminalAuthorityDisposition::from_filesystem_facets(Facet::ALL);
        for export in [b"ReadFile".as_slice(), b"WriteFile".as_slice()] {
            let row = mechanism_row(imported(b"kernel32.dll", export, 1)).unwrap();
            assert!(!filesystem_only.contains_all(&row.disposition));
            assert!(row.disposition.classes().contains(&Class::ProcessOutput));
        }
        // No complete argument/handler contract is claimed for generic CRT
        // descriptors. Refusal must not be replaced by a filesystem-only row.
        for (name, export) in [
            ("read", b"_read".as_slice()),
            ("write", b"_write".as_slice()),
        ] {
            let method = effects::provider_plan::ServiceMethod {
                name: name.to_owned(),
                ..Default::default()
            };
            assert!(
                super::super::cohort_mechanism_row(
                    imported(b"msvcrt.dll", export, 1).into(),
                    &Default::default(),
                    &method,
                )
                .is_none()
            );
        }
    }

    #[test]
    fn calling_contracts_keep_distinct_keys_and_cannot_assert_narrowing() {
        let first = imported(b"kernel32.dll", b"GetLastError", 1);
        let second = imported(b"kernel32.dll", b"GetLastError", 2);
        assert_ne!(
            mechanism_row(first).unwrap().mechanism,
            mechanism_row(second).unwrap().mechanism
        );
        assert!(mechanism_row(imported(b"kernel32.dll", b"GetLastError", 0)).is_none());
        assert!(
            mechanism_row(first.with_checked_argument_contract(
                CheckedSyscallArgumentContractIdentity::from_digest([3; 32]),
            ))
            .is_none()
        );
    }
}

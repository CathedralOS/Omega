//! A source method cannot establish what a different library symbol or system
//! call will do. Both provider routes classify the exact selected operation.
//! Reads, writes and closes on an arbitrary descriptor need evidence about
//! the installed object before they can receive a narrower classification.

use runtime_abi::{DescriptorOperation, descriptor_system_call};
use semantic_vocabulary::{TerminalAuthorityClass as Class, TerminalAuthorityDisposition};
use std::sync::OnceLock;
use target::{LinuxFilesystemOperation as Op, TargetProfile};
use terminal_authority_receipts::{
    NormalizedForeignArgumentContract, NormalizedForeignTerminalMechanismIdentity,
    SyscallTerminalMechanismIdentity, TerminalMechanismIdentity,
};

use super::TerminalAuthorityPolicyRow;

const METADATA: &[Class] = &[Class::FilesystemMetadataQuery];
const SET_METADATA: &[Class] = &[Class::FilesystemMetadataMutation];
const NAMESPACE: &[Class] = &[Class::FilesystemNamespaceMutation];
const DIRECTORY: &[Class] = &[Class::DirectoryEnumeration];
const CREATE_DIRECTORY: &[Class] = &[
    Class::FilesystemNamespaceMutation,
    Class::FilesystemMetadataMutation,
];
const SET_LENGTH: &[Class] = &[Class::FilesystemContentWrite];

fn linux_operation(target: TargetProfile, number: u32) -> Option<&'static [Class]> {
    if matches!(
        (target, number),
        (TargetProfile::LinuxX64, 60 | 231) | (TargetProfile::LinuxArm64, 93 | 94)
    ) {
        return Some(&[Class::ProcessTermination]);
    }
    linux_filesystem_classes(Op::from_syscall(target, i64::from(number))?)
}

fn linux_filesystem_classes(operation: Op) -> Option<&'static [Class]> {
    Some(match operation {
        Op::Fstat | Op::Newfstatat | Op::Readlinkat => METADATA,
        Op::Fchmod | Op::Fchmodat | Op::Fchown | Op::Fchownat | Op::Utimensat => SET_METADATA,
        Op::Linkat | Op::Renameat | Op::Symlinkat | Op::Unlinkat => NAMESPACE,
        Op::Getdents64 => DIRECTORY,
        Op::Mkdirat => CREATE_DIRECTORY,
        Op::Ftruncate => SET_LENGTH,
        Op::Dup | Op::Fsync | Op::Fdatasync | Op::Lseek | Op::Flock => &[],
        Op::Openat | Op::Close | Op::Pread64 | Op::Pwrite64 => return None,
    })
}

/// An arbitrary descriptor has no host-independent filesystem classification.
/// Inside the private namespace its only possible objects are this invocation's
/// files, directories and captured pipes. Reuse the target's adapter table.
pub(crate) fn private_linux_syscall_authority(
    call: SyscallTerminalMechanismIdentity,
) -> Option<TerminalAuthorityDisposition> {
    if call.checked_argument_contract().is_zero() {
        return None;
    }
    let target = call.target();
    let number = i64::from(call.number());
    let classes: &[Class] = if let Some((_, operation)) = descriptor_system_call(target, number) {
        match operation {
            DescriptorOperation::Read => &[Class::FilesystemContentRead, Class::ProcessInput],
            DescriptorOperation::Write => &[Class::FilesystemContentWrite, Class::ProcessOutput],
        }
    } else {
        match Op::from_syscall(target, number)? {
            Op::Openat => &[
                Class::FilesystemContentRead,
                Class::FilesystemContentWrite,
                Class::FilesystemNamespaceMutation,
                Class::FilesystemMetadataMutation,
            ],
            Op::Close => &[],
            Op::Pread64 => &[Class::FilesystemContentRead],
            Op::Pwrite64 => &[Class::FilesystemContentWrite],
            // These have host-independent classifications in the ordinary policy.
            other => linux_filesystem_classes(other)?,
        }
    };
    Some(TerminalAuthorityDisposition::from_classes(
        classes.iter().copied(),
    ))
}

struct PosixImport {
    locator: target::ForeignLocatorIdentityDigest,
    classes: &'static [Class],
}

fn reviewed_import(
    foreign: NormalizedForeignTerminalMechanismIdentity,
) -> Option<&'static [Class]> {
    use TargetProfile::{LinuxArm64, LinuxX64, MacosArm64, MacosX64};
    if !matches!(
        foreign.target(),
        LinuxArm64 | LinuxX64 | MacosArm64 | MacosX64
    ) || foreign.implementation_contract().is_zero()
        || foreign.argument_contract() != NormalizedForeignArgumentContract::AdmittedPlan
    {
        return None;
    }
    static IMPORTS: OnceLock<Vec<PosixImport>> = OnceLock::new();
    let imports = IMPORTS.get_or_init(|| {
        let operations: &[(&[u8], &[Class])] = &[
            (b"___error", &[]),
            (b"_dup", &[]),
            (b"_lseek", &[]),
            (b"_fsync", &[]),
            (b"_fdatasync", &[]),
            (b"_flock", &[]),
            (b"_fstat", METADATA),
            (b"_fstatat", METADATA),
            (b"_readlink", METADATA),
            (b"_chmod", SET_METADATA),
            (b"_fchmod", SET_METADATA),
            (b"_futimens", SET_METADATA),
            (b"_chown", SET_METADATA),
            (b"_lchown", SET_METADATA),
            (b"_fchown", SET_METADATA),
            (b"_unlink", NAMESPACE),
            (b"_unlinkat", NAMESPACE),
            (b"_link", NAMESPACE),
            (b"_rename", NAMESPACE),
            (b"_rmdir", NAMESPACE),
            (b"_symlink", NAMESPACE),
            (b"_mkdir", CREATE_DIRECTORY),
            (b"___getdirentries64", DIRECTORY),
            (b"_ftruncate", SET_LENGTH),
            (b"__exit", &[Class::ProcessTermination]),
        ];
        let mut imports = Vec::with_capacity(operations.len() * 2 + 2);
        for target in [MacosArm64, MacosX64] {
            for &(symbol, classes) in operations {
                let locator = target::normalize_foreign_locator(
                    target::ForeignLocatorCandidate::MachODylibSymbol {
                        install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                        symbol: symbol.to_vec(),
                    },
                    target,
                )
                .expect("reviewed libSystem import");
                imports.push(PosixImport {
                    locator: locator.identity_digest(),
                    classes,
                });
            }
        }
        for (target, version) in [
            (LinuxX64, b"GLIBC_2.2.5".as_slice()),
            (LinuxArm64, b"GLIBC_2.17"),
        ] {
            let locator = target::normalize_foreign_locator(
                target::ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.6".to_vec(),
                    symbol: b"__errno_location".to_vec(),
                    version: version.to_vec(),
                },
                target,
            )
            .expect("reviewed glibc error-state import");
            imports.push(PosixImport {
                locator: locator.identity_digest(),
                classes: &[],
            });
        }
        imports
    });
    Some(
        imports
            .iter()
            .find(|row| row.locator == foreign.locator_identity())?
            .classes,
    )
}

pub(crate) fn mechanism_row(
    mechanism: TerminalMechanismIdentity,
) -> Option<TerminalAuthorityPolicyRow> {
    let classes = match mechanism {
        TerminalMechanismIdentity::NormalizedForeign(import) => reviewed_import(import)?,
        TerminalMechanismIdentity::Syscall(call) if !call.checked_argument_contract().is_zero() => {
            linux_operation(call.target(), call.number())?
        }
        _ => return None,
    };
    Some(TerminalAuthorityPolicyRow::new(
        mechanism,
        TerminalAuthorityDisposition::from_classes(classes.iter().copied()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use provider_plans::BoundaryCallingPlanCommitment;
    use terminal_authority_receipts::{
        CheckedSyscallArgumentContractIdentity, SyscallTerminalMechanismIdentity,
    };

    fn darwin(
        library: &[u8],
        symbol: &[u8],
        plan: u8,
    ) -> NormalizedForeignTerminalMechanismIdentity {
        let locator = target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::MachODylibSymbol {
                install_name: library.to_vec(),
                symbol: symbol.to_vec(),
            },
            TargetProfile::MacosArm64,
        )
        .unwrap();
        NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
            &locator,
            BoundaryCallingPlanCommitment::from_digest([plan; 32]),
        )
    }

    #[test]
    fn readlink_uses_the_selected_library_and_preserves_each_admitted_calling_plan() {
        for plan in [1, 2] {
            let readlink = darwin(b"/usr/lib/libSystem.B.dylib", b"_readlink", plan);
            let row = mechanism_row(readlink.into()).unwrap();
            assert_eq!(row.mechanism(), readlink.into());
            assert_eq!(row.disposition().classes(), METADATA);
        }
        let other = darwin(b"/fixture/other.dylib", b"_readlink", 1);
        assert!(mechanism_row(other.into()).is_none());
        for symbol in [b"_read".as_slice(), b"_write", b"_close", b"_other"] {
            let import = darwin(b"/usr/lib/libSystem.B.dylib", symbol, 1);
            assert!(mechanism_row(import.into()).is_none());
        }
    }

    #[test]
    fn linux_error_state_binds_the_libc_object_and_version_for_each_target() {
        for (target, version) in [
            (TargetProfile::LinuxX64, b"GLIBC_2.2.5".as_slice()),
            (TargetProfile::LinuxArm64, b"GLIBC_2.17"),
        ] {
            let errno = |object: &[u8], version: &[u8]| {
                let locator = target::normalize_foreign_locator(
                    target::ForeignLocatorCandidate::ElfVersioned {
                        object: object.to_vec(),
                        symbol: b"__errno_location".to_vec(),
                        version: version.to_vec(),
                    },
                    target,
                )
                .unwrap();
                NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
                    &locator,
                    BoundaryCallingPlanCommitment::from_digest([4; 32]),
                )
            };
            let import = errno(b"libc.so.6", version);
            let row = mechanism_row(import.into()).unwrap();
            assert_eq!(row.mechanism(), import.into());
            assert!(row.disposition().classes().is_empty());
            assert!(mechanism_row(errno(b"libother.so", version).into()).is_none());
            assert!(mechanism_row(errno(b"libc.so.6", b"OTHER_1.0").into()).is_none());
        }
    }

    #[test]
    fn joined_native_operations_keep_the_receivers_supplied_policy() {
        let readlink = darwin(b"/usr/lib/libSystem.B.dylib", b"_readlink", 3);
        let fstat = darwin(b"/usr/lib/libSystem.B.dylib", b"_fstat", 3);
        let supplied = TerminalAuthorityDisposition::from_classes([Class::FilesystemContentRead]);
        let receiver = super::super::terminal_authority_policy_with_rows(vec![
            TerminalAuthorityPolicyRow::new(fstat.into(), supplied.clone()),
        ])
        .unwrap();
        let policy = receiver
            .with_reviewed_operations(Vec::new(), [readlink.into(), fstat.into(), readlink.into()])
            .unwrap()
            .unwrap();
        assert_eq!(policy.classify(readlink).unwrap().classes(), METADATA);
        assert_eq!(policy.classify(fstat).unwrap(), supplied);
        assert!(
            policy
                .with_reviewed_operations(Vec::new(), [readlink.into()])
                .unwrap()
                .is_none()
        );
        assert!(receiver.classify(readlink).is_err());
        assert_eq!(
            policy
                .classify(native_intrinsics::CompilerIntrinsicExecutionIdentity::HostedWriteByteI32)
                .unwrap()
                .classes(),
            &[Class::ProcessOutput],
        );
    }

    #[test]
    fn linux_filesystem_calls_require_the_right_target_and_checked_contract() {
        for (target, number) in [
            (TargetProfile::LinuxX64, 267),
            (TargetProfile::LinuxArm64, 78),
        ] {
            let checked = CheckedSyscallArgumentContractIdentity::from_digest([7; 32]);
            let readlink = SyscallTerminalMechanismIdentity::new(target, number, checked);
            let row = mechanism_row(readlink.into()).unwrap();
            assert_eq!(row.mechanism(), readlink.into());
            assert_eq!(row.disposition().classes(), METADATA);
            let unchecked = SyscallTerminalMechanismIdentity::new(
                target,
                number,
                CheckedSyscallArgumentContractIdentity::from_digest([0; 32]),
            );
            assert!(mechanism_row(unchecked.into()).is_none());
        }
        let checked = CheckedSyscallArgumentContractIdentity::from_digest([7; 32]);
        // The same number means fchown on x86-64 and exit on arm64.
        for (target, classes) in [
            (TargetProfile::LinuxX64, SET_METADATA),
            (TargetProfile::LinuxArm64, &[Class::ProcessTermination][..]),
        ] {
            let call = SyscallTerminalMechanismIdentity::new(target, 93, checked);
            assert_eq!(
                mechanism_row(call.into()).unwrap().disposition().classes(),
                classes
            );
        }
        for (target, number) in [
            (TargetProfile::LinuxArm64, 267),
            (TargetProfile::LinuxX64, 1),
            (TargetProfile::LinuxArm64, 57),
        ] {
            let call = SyscallTerminalMechanismIdentity::new(target, number, checked);
            assert!(mechanism_row(call.into()).is_none());
        }
    }
}

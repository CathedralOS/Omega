// We define exact Windows OS entry contracts (kernel32.dll and ntdll.dll),
// qualifying each overload's complete semantic record and calling/storage
// recipes before any native effect.

use super::super::super::native_provider::IntegerRecordField;
use super::super::super::{admit_exact_foreign_call, matches_platform_c_symbol};
use super::super::matches_signature;
use super::{Effect, Formal, TerminalRuntimeError};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::IntegerSign;
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{TerminalImportSignature, TerminalProgram};
use terminal_psi::{BoundaryMachineDeclaration, StructuralAccess};

pub(super) struct Contract {
    pub effect: Effect,
    pub symbol: &'static [u8],
}

pub(super) fn select(
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    layouts: &[NativeBorrowedStorageLayout],
) -> Result<Option<Contract>, TerminalRuntimeError> {
    use Formal::{ByteView, IntegerRecord, IntegerReference, Signed, Unsigned, UnsignedArray};
    use IntegerRecordField::{Array, ReferenceArray, ReferenceRecord, Scalar};
    use IntegerSign::{Signed as I, Unsigned as U};
    use StructuralAccess::{MutableBorrow, SharedBorrow};
    // A path is any borrowed WCHAR array: the import declares its own length,
    // and `Call::validate_path` requires the terminator inside that length
    // before the native call, so no domain on the caller's carrier is trusted.
    let path = Formal::UnsignedWindow {
        bits: 16,
        minimum: 1,
        access: SharedBorrow,
    };
    let rows: &[(&[u8], &[u8], &[Formal], Formal, Effect)] = &[
        (
            b"kernel32.dll",
            b"CreateFileW",
            &[
                path,
                Unsigned(32),
                Unsigned(32),
                Unsigned(64),
                Unsigned(32),
                Unsigned(32),
                Unsigned(64),
            ],
            Signed(64),
            Effect::Acquire,
        ),
        (
            b"kernel32.dll",
            b"CloseHandle",
            &[Signed(64)],
            Signed(32),
            Effect::Close,
        ),
        (
            b"kernel32.dll",
            b"DuplicateHandle",
            &[
                Signed(64),
                Signed(64),
                Signed(64),
                IntegerReference(IntegerSign::Signed, 64, MutableBorrow),
                Unsigned(32),
                Signed(32),
                Unsigned(32),
            ],
            Signed(32),
            Effect::Duplicate,
        ),
        (
            b"kernel32.dll",
            b"WriteFile",
            &[
                Signed(64),
                ByteView(SharedBorrow),
                IntegerReference(IntegerSign::Unsigned, 32, MutableBorrow),
                Unsigned(64),
            ],
            Signed(32),
            Effect::Use,
        ),
        (
            b"ntdll.dll",
            b"NtQueryInformationFile",
            &[
                Signed(64),
                IntegerRecord {
                    fields: &[
                        IntegerRecordField::Scalar(IntegerSign::Signed, 64),
                        IntegerRecordField::Scalar(IntegerSign::Unsigned, 64),
                    ],
                    access: MutableBorrow,
                },
                IntegerReference(IntegerSign::Unsigned, 32, MutableBorrow),
                Unsigned(32),
                Unsigned(32),
            ],
            Signed(32),
            Effect::Use,
        ),
        (
            b"kernel32.dll",
            b"SetFileTime",
            &[
                Signed(64),
                Unsigned(64),
                UnsignedArray(2, 32, SharedBorrow),
                UnsignedArray(2, 32, SharedBorrow),
            ],
            Signed(32),
            Effect::Use,
        ),
        (
            b"kernel32.dll",
            b"GetFileTime",
            &[
                Signed(64),
                Unsigned(64),
                IntegerReference(IntegerSign::Unsigned, 64, MutableBorrow),
                IntegerReference(IntegerSign::Unsigned, 64, MutableBorrow),
            ],
            Signed(32),
            Effect::Use,
        ),
        (
            b"kernel32.dll",
            b"DeleteFileW",
            &[path],
            Signed(32),
            Effect::Path { count: 1 },
        ),
        (
            b"kernel32.dll",
            b"CreatePipe",
            &[
                IntegerReference(U, 64, MutableBorrow),
                IntegerReference(U, 64, MutableBorrow),
                Unsigned(64),
                Unsigned(32),
            ],
            Signed(32),
            Effect::Pipe,
        ),
        (
            b"kernel32.dll",
            b"GetFileType",
            &[Signed(64)],
            Unsigned(32),
            Effect::Use,
        ),
        (
            b"kernel32.dll",
            b"GetFileInformationByHandleEx",
            &[
                Signed(64),
                Unsigned(32),
                IntegerRecord {
                    fields: &[
                        Scalar(U, 64),
                        Scalar(U, 64),
                        Scalar(U, 64),
                        Scalar(U, 64),
                        Scalar(U, 32),
                        Scalar(U, 32),
                    ],
                    access: MutableBorrow,
                },
                Unsigned(32),
            ],
            Signed(32),
            Effect::Query {
                class: 0,
                bytes: 40,
            },
        ),
        (
            b"kernel32.dll",
            b"GetFileInformationByHandleEx",
            &[
                Signed(64),
                Unsigned(32),
                IntegerRecord {
                    fields: &[
                        Scalar(I, 64),
                        Scalar(I, 64),
                        Scalar(U, 32),
                        Scalar(U, 8),
                        Scalar(U, 8),
                        Array(U, 8, 2),
                    ],
                    access: MutableBorrow,
                },
                Unsigned(32),
            ],
            Signed(32),
            Effect::Query {
                class: 1,
                bytes: 24,
            },
        ),
        (
            b"kernel32.dll",
            b"GetFileInformationByHandleEx",
            &[
                Signed(64),
                Unsigned(32),
                IntegerRecord {
                    fields: &[Scalar(U, 32), Scalar(U, 32)],
                    access: MutableBorrow,
                },
                Unsigned(32),
            ],
            Signed(32),
            Effect::Query { class: 9, bytes: 8 },
        ),
        (
            b"kernel32.dll",
            b"ReadFile",
            &[
                Signed(64),
                ByteView(MutableBorrow),
                IntegerReference(U, 32, MutableBorrow),
                Unsigned(64),
            ],
            Signed(32),
            Effect::Use,
        ),
        (
            b"kernel32.dll",
            b"SetFileInformationByHandle",
            &[
                Signed(64),
                Unsigned(32),
                IntegerRecord {
                    fields: &[
                        Scalar(U, 64),
                        Scalar(U, 64),
                        Scalar(U, 64),
                        Scalar(U, 64),
                        Scalar(U, 32),
                        Scalar(U, 32),
                    ],
                    access: SharedBorrow,
                },
                Unsigned(32),
            ],
            Signed(32),
            Effect::SetInformation {
                class: 0,
                bytes: 40,
            },
        ),
        (
            b"kernel32.dll",
            b"SetFileInformationByHandle",
            &[
                Signed(64),
                Unsigned(32),
                IntegerRecord {
                    fields: &[Scalar(I, 64)],
                    access: SharedBorrow,
                },
                Unsigned(32),
            ],
            Signed(32),
            Effect::SetInformation { class: 6, bytes: 8 },
        ),
        (
            b"kernel32.dll",
            b"SetFilePointerEx",
            &[
                Signed(64),
                Signed(64),
                IntegerReference(I, 64, MutableBorrow),
                Unsigned(32),
            ],
            Signed(32),
            Effect::Use,
        ),
        (
            b"kernel32.dll",
            b"CreateDirectoryW",
            &[path, Unsigned(64)],
            Signed(32),
            Effect::Path { count: 1 },
        ),
        (
            b"kernel32.dll",
            b"RemoveDirectoryW",
            &[path],
            Signed(32),
            Effect::Path { count: 1 },
        ),
        (
            b"kernel32.dll",
            b"FlushFileBuffers",
            &[Signed(64)],
            Signed(32),
            Effect::Use,
        ),
        (
            b"kernel32.dll",
            b"GetFinalPathNameByHandleW",
            &[
                Signed(64),
                UnsignedArray(4096, 16, MutableBorrow),
                Unsigned(32),
                Unsigned(32),
            ],
            Unsigned(32),
            Effect::Use,
        ),
        (
            b"kernel32.dll",
            b"SetFileAttributesW",
            &[path, Unsigned(32)],
            Signed(32),
            Effect::Path { count: 1 },
        ),
        (
            b"kernel32.dll",
            b"MoveFileExW",
            &[path, path, Unsigned(32)],
            Signed(32),
            Effect::Path { count: 2 },
        ),
        (
            b"ntdll.dll",
            b"NtQueryDirectoryFile",
            &[
                Signed(64),
                Unsigned(64),
                Unsigned(64),
                Unsigned(64),
                IntegerRecord {
                    fields: &[Scalar(I, 64), Scalar(U, 64)],
                    access: MutableBorrow,
                },
                IntegerRecord {
                    fields: &[Scalar(U, 64), Scalar(U, 32), Array(U, 16, 4096)],
                    access: MutableBorrow,
                },
                Unsigned(32),
                Unsigned(32),
                Unsigned(8),
                Unsigned(64),
                Unsigned(8),
            ],
            Signed(32),
            Effect::Directory {
                class: 12,
                bytes: 8208,
            },
        ),
        (
            b"ntdll.dll",
            b"NtQueryDirectoryFile",
            &[
                Signed(64),
                Unsigned(64),
                Unsigned(64),
                Unsigned(64),
                IntegerRecord {
                    fields: &[Scalar(I, 64), Scalar(U, 64)],
                    access: MutableBorrow,
                },
                IntegerRecord {
                    fields: &[
                        Scalar(U, 64),
                        Scalar(U, 64),
                        Scalar(U, 64),
                        Scalar(U, 64),
                        Scalar(U, 64),
                        Scalar(I, 64),
                        Scalar(I, 64),
                        Scalar(U, 32),
                        Scalar(U, 32),
                        Array(U, 16, 4096),
                    ],
                    access: MutableBorrow,
                },
                Unsigned(32),
                Unsigned(32),
                Unsigned(8),
                Unsigned(64),
                Unsigned(8),
            ],
            Signed(32),
            Effect::Directory {
                class: 1,
                bytes: 8256,
            },
        ),
        (
            b"ntdll.dll",
            b"NtOpenFile",
            &[
                IntegerReference(I, 64, MutableBorrow),
                Unsigned(32),
                IntegerRecord {
                    fields: &[
                        Scalar(U, 32),
                        Scalar(U, 32),
                        Scalar(I, 64),
                        ReferenceRecord(
                            &[
                                Scalar(U, 16),
                                Scalar(U, 16),
                                Scalar(U, 32),
                                ReferenceArray(U, 16, 4096, SharedBorrow),
                            ],
                            SharedBorrow,
                        ),
                        Scalar(U, 32),
                        Scalar(U, 32),
                        Scalar(U, 64),
                        Scalar(U, 64),
                    ],
                    access: SharedBorrow,
                },
                IntegerRecord {
                    fields: &[Scalar(I, 64), Scalar(U, 64)],
                    access: MutableBorrow,
                },
                Unsigned(32),
                Unsigned(32),
            ],
            Signed(32),
            Effect::OpenRelative,
        ),
        (
            b"kernel32.dll",
            b"SetFileInformationByHandle",
            &[
                Signed(64),
                Unsigned(32),
                IntegerRecord {
                    fields: &[Scalar(U, 8)],
                    access: SharedBorrow,
                },
                Unsigned(32),
            ],
            Signed(32),
            Effect::Disposition,
        ),
    ];
    let mut candidates = Vec::new();
    for &(library, symbol, parameters, result, effect) in rows {
        if !matches_platform_c_symbol(binding, symbol, &[], Some(library))? {
            continue;
        }
        candidates.push((symbol, parameters));
        if !matches_signature(program, declaration, parameters, result, layouts) {
            continue;
        }
        validate_entry(declaration, binding, parameters, result, symbol)?;
        return Ok(Some(Contract { effect, symbol }));
    }
    if let Some((symbol, _)) = candidates.first() {
        return Err(TerminalRuntimeError::missing(
            "runtime-filesystem",
            format!(
                "Windows {} differs from its complete semantic/storage contracts {:?}",
                String::from_utf8_lossy(symbol),
                candidates
                    .iter()
                    .map(|(_, parameters)| parameters)
                    .collect::<Vec<_>>()
            ),
        ));
    }
    Ok(None)
}

fn validate_entry(
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    parameters: &[Formal],
    result: Formal,
    symbol: &[u8],
) -> Result<(), TerminalRuntimeError> {
    let shape = |formal: Formal| match formal {
        Formal::Signed(bits) | Formal::Unsigned(bits) => {
            ValueShape::integer(u32::from(bits / 8), bits / 8)
        }
        Formal::ByteView(_) => ValueShape::integer(16, 8),
        _ => ValueShape::integer(8, 8),
    };
    let signature = CallSignature {
        parameters: parameters.iter().copied().map(shape).collect(),
        result: Some(shape(result)),
    };
    let mut expected = if matches!(symbol, b"ReadFile" | b"WriteFile") {
        let physical = CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 5],
            result: signature.result,
        };
        let mut plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, &physical);
        if let Ok(plan) = &mut plan {
            let count = plan.parameters.remove(2);
            plan.parameters[1].shape = ValueShape::integer(16, 8);
            plan.parameters[1]
                .locations
                .extend(count.locations.into_iter().map(|mut location| {
                    match &mut location {
                        calling_conventions::ValueLocation::Register {
                            value_byte_offset, ..
                        } => *value_byte_offset += 8,
                        _ => unreachable!("Microsoft second and third words are registers"),
                    }
                    location
                }));
        }
        plan
    } else {
        evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature)
    }
    .map_err(|error| {
        TerminalRuntimeError::missing(
            "runtime-filesystem",
            format!("Windows entry geometry: {error}"),
        )
    })?;
    if matches!(symbol, b"ReadFile" | b"WriteFile") {
        expected.parameters[2].shape = ValueShape::integer(8, 8);
    }
    if binding.boundary_entry_plan.call != expected {
        return Err(TerminalRuntimeError::missing(
            "runtime-filesystem",
            format!(
                "Windows {} selected calling plan differs from its exact C word placement",
                String::from_utf8_lossy(symbol)
            ),
        ));
    }
    admit_exact_foreign_call(
        &TerminalImportSignature::from(declaration),
        declaration,
        binding,
        &signature,
    )
}

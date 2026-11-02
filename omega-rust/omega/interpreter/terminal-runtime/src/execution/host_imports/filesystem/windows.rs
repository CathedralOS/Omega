//! Exact synchronous Windows file operations in the explicit host namespace.
//! Opaque HANDLEs belong to the private native worker. Guest identities select
//! only resources acquired by this instance; process pseudo-handles, asynchronous
//! requests and pointer-valued optional parameters cannot borrow its internals.

use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{IntegerSign, IntegerValue};
use std::collections::BTreeMap;
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalEffectRejection, TerminalImportSignature, TerminalProgram, TerminalScalarValue,
};
use terminal_psi::{BoundaryMachineDeclaration, StructuralAccess};

use super::{FilesystemContext, Formal, TerminalRuntimeError, matches_signature, rejection};

#[derive(Default)]
pub(super) struct Handles {
    entries: BTreeMap<i64, State>,
}
enum State {
    Open(i64),
    Uncertain,
}

impl Handles {
    fn translate(&self, guest: i64) -> Result<i64, TerminalEffectRejection> {
        match self.entries.get(&guest) {
            Some(State::Open(native)) => Ok(*native),
            Some(State::Uncertain) => {
                Err(rejection("Windows handle is uncertain after failed close"))
            }
            // NULL is a known invalid file handle, not a private resource. The
            // actual API produces its error. Unknown nonzero integers refuse.
            None if guest == 0 => Ok(0),
            None => Err(rejection(
                "Windows file handle is outside the supplied guest namespace",
            )),
        }
    }
    fn acquire(&mut self, native: i64) -> Result<i64, TerminalEffectRejection> {
        if native == 0 || native == -1 {
            return Ok(native);
        }
        let guest = (3..=i64::MAX)
            .find(|guest| !self.entries.contains_key(guest))
            .ok_or_else(|| rejection("guest Windows handle namespace is exhausted"))?;
        self.entries.insert(guest, State::Open(native));
        Ok(guest)
    }
}

#[derive(Clone, Copy, Debug)]
enum Effect {
    Acquire,
    Use,
    Close,
    Duplicate,
    Path,
}

pub(in super::super) struct Call {
    context: FilesystemContext,
    effect: Effect,
    symbol: &'static [u8],
    namespace: Result<(), &'static str>,
}

pub(super) fn admit(
    context: &FilesystemContext,
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    layouts: &[NativeBorrowedStorageLayout],
) -> Result<Option<Call>, TerminalRuntimeError> {
    if !binding
        .import()
        .is_some_and(|(locator, _)| locator.target() == target::TargetProfile::WindowsX64)
        || !cfg!(all(target_os = "windows", target_arch = "x86_64"))
    {
        return Ok(None);
    }
    use Formal::{ByteView, IntegerRecord, IntegerReference, Signed, Unsigned, UnsignedArray};
    use StructuralAccess::{MutableBorrow, SharedBorrow};
    let path = UnsignedArray(4096, 16, SharedBorrow);
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
                    fields: &[(IntegerSign::Signed, 64), (IntegerSign::Unsigned, 64)],
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
            Effect::Path,
        ),
    ];
    for &(library, symbol, parameters, result, effect) in rows {
        if !super::super::matches_platform_c_symbol(binding, symbol, &[], Some(library))? {
            continue;
        }
        if !matches_signature(program, declaration, parameters, result, layouts) {
            return Err(TerminalRuntimeError::missing(
                "runtime-filesystem",
                format!(
                    "Windows {} differs from its exact semantic/storage contract {:?}",
                    String::from_utf8_lossy(symbol),
                    parameters
                ),
            ));
        }
        validate_entry(declaration, binding, parameters, result, symbol)?;
        let namespace = if context.supplied {
            context.admit_services(program, declaration)
        } else {
            Err("no explicit host filesystem namespace was supplied")
        };
        return Ok(Some(Call {
            context: context.clone(),
            effect,
            symbol,
            namespace,
        }));
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
    // One semantic view carries two complete words. The real C API consumes
    // the low DWORD of the count register; its source requirement proves that
    // the full live length fits. Other C operands retain their ordinary slots.
    let mut expected = if symbol == b"WriteFile" {
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
    // The source's final OVERLAPPED word is eight bytes; the count-output
    // pointer occupies the fourth register after the expanded view.
    if symbol == b"WriteFile" {
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
    super::super::admit_exact_foreign_call(
        &TerminalImportSignature::from(declaration),
        declaration,
        binding,
        &signature,
    )
}

fn signed(value: TerminalScalarValue) -> Result<i64, TerminalEffectRejection> {
    match value {
        TerminalScalarValue::Integer {
            value: IntegerValue::Signed(value),
            ..
        } => {
            i64::try_from(value).map_err(|_| rejection("Windows handle exceeds its signed carrier"))
        }
        _ => Err(rejection("Windows handle lost its signed scalar carrier")),
    }
}
fn scalar(value: TerminalScalarValue, expected: i128) -> bool {
    match value {
        TerminalScalarValue::Integer {
            value: IntegerValue::Signed(value),
            ..
        } => value == expected,
        TerminalScalarValue::Integer {
            value: IntegerValue::Unsigned(value),
            ..
        } => u128::try_from(expected).ok() == Some(value),
        _ => false,
    }
}
fn replace_signed(
    mut value: TerminalScalarValue,
    replacement: i64,
) -> Result<TerminalScalarValue, TerminalEffectRejection> {
    let TerminalScalarValue::Integer {
        scalar_type,
        value: contents,
    } = &mut value
    else {
        return Err(rejection("Windows result is not a scalar handle"));
    };
    let replacement = IntegerValue::Signed(i128::from(replacement));
    if !scalar_type.admits(replacement) {
        return Err(rejection(
            "guest Windows handle exceeds its declared carrier",
        ));
    }
    *contents = replacement;
    Ok(value)
}

impl Call {
    pub(super) fn path_argument(&self, index: usize) -> bool {
        index == 0 && matches!(self.effect, Effect::Acquire | Effect::Path)
    }
    pub(super) fn validate_path(
        &self,
        index: usize,
        bytes: &[u8],
    ) -> Result<(), TerminalEffectRejection> {
        if self.path_argument(index)
            && (bytes.len() != 8192 || !bytes.chunks_exact(2).any(|word| word == [0, 0]))
        {
            return Err(rejection(
                "Windows path has no WCHAR terminator inside its complete captured array",
            ));
        }
        Ok(())
    }
    pub(super) fn arguments(
        &self,
        arguments: &[TerminalScalarValue],
    ) -> Result<Vec<TerminalScalarValue>, TerminalEffectRejection> {
        self.namespace.map_err(rejection)?;
        let invalid = || {
            rejection(
                "Windows file operation requires its exact synchronous/null/current-process inputs",
            )
        };
        let safe = match self.symbol {
            b"CreateFileW" => {
                arguments.len() == 6
                    && scalar(arguments[2], 0)
                    && scalar(arguments[5], 0)
                    && matches!(arguments[4], TerminalScalarValue::Integer { value: IntegerValue::Unsigned(flags), .. } if flags & 0x40000000 == 0)
            }
            b"DuplicateHandle" => {
                arguments.len() == 6
                    && scalar(arguments[0], -1)
                    && scalar(arguments[2], -1)
                    && scalar(arguments[3], 0)
                    && scalar(arguments[4], 0)
                    && scalar(arguments[5], 2)
            }
            b"WriteFile" | b"SetFileTime" | b"GetFileTime" => {
                arguments.len() == 2 && scalar(arguments[1], 0)
            }
            b"NtQueryInformationFile" => {
                arguments.len() == 3 && scalar(arguments[1], 4) && scalar(arguments[2], 16)
            }
            b"CloseHandle" => arguments.len() == 1,
            b"DeleteFileW" => arguments.is_empty(),
            _ => false,
        };
        if !safe {
            return Err(invalid());
        }
        let index = match self.effect {
            Effect::Acquire | Effect::Path => return Ok(arguments.to_vec()),
            Effect::Duplicate => 1,
            _ => 0,
        };
        let mut result = arguments.to_vec();
        let native = self
            .context
            .windows
            .borrow()
            .translate(signed(result[index])?)?;
        result[index] = replace_signed(result[index], native)?;
        Ok(result)
    }
    pub(super) fn result(
        &self,
        arguments: &[TerminalScalarValue],
        value: TerminalScalarValue,
    ) -> Result<TerminalScalarValue, TerminalEffectRejection> {
        match self.effect {
            Effect::Acquire => replace_signed(
                value,
                self.context.windows.borrow_mut().acquire(signed(value)?)?,
            ),
            Effect::Close => {
                let guest =
                    signed(*arguments.first().ok_or_else(|| {
                        rejection("closed Windows handle lost its guest identity")
                    })?)?;
                if guest != 0 {
                    if signed(value)? != 0 {
                        self.context.windows.borrow_mut().entries.remove(&guest);
                    } else {
                        self.context
                            .windows
                            .borrow_mut()
                            .entries
                            .insert(guest, State::Uncertain);
                    }
                }
                Ok(value)
            }
            _ => Ok(value),
        }
    }
    pub(super) fn output_scalar(
        &self,
        argument: usize,
        value: TerminalScalarValue,
        result: Option<TerminalScalarValue>,
    ) -> Result<Option<TerminalScalarValue>, TerminalEffectRejection> {
        if matches!(self.effect, Effect::Duplicate) && argument == 0 {
            let status =
                signed(result.ok_or_else(|| {
                    rejection("duplicate HANDLE lost its native completion status")
                })?)?;
            if status != 0 {
                let native = signed(value)?;
                if native == 0 || native == -1 {
                    return Err(rejection(
                        "successful duplicate returned no owned native HANDLE",
                    ));
                }
                return replace_signed(value, self.context.windows.borrow_mut().acquire(native)?)
                    .map(Some);
            }
        }
        Ok(None)
    }
}

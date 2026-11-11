// We manage synchronous Windows file operations in the explicit host namespace.
// Opaque HANDLEs belong to the private native worker and are translated through
// a per-invocation guest handle table, while process pseudo-handles,
// asynchronous requests, and pointer-valued optional parameters are rejected.

use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::IntegerValue;
use std::collections::BTreeMap;
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{TerminalEffectRejection, TerminalProgram, TerminalScalarValue};
use terminal_psi::BoundaryMachineDeclaration;

use super::{FilesystemContext, Formal, TerminalRuntimeError, rejection};

mod contracts;

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
    Path { count: usize },
    Pipe,
    Query { class: u32, bytes: u32 },
    Directory { class: u32, bytes: u32 },
    OpenRelative,
    Disposition,
    SetInformation { class: u32, bytes: u32 },
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
    let Some(contract) = contracts::select(program, declaration, binding, layouts)? else {
        return Ok(None);
    };
    let namespace = context
        .process_namespace()
        .and_then(|()| context.admit_services(program, declaration));
    Ok(Some(Call {
        context: context.clone(),
        effect: contract.effect,
        symbol: contract.symbol,
        namespace,
    }))
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

fn handle_bits(value: TerminalScalarValue) -> Result<i64, TerminalEffectRejection> {
    match value {
        TerminalScalarValue::Integer {
            value: IntegerValue::Unsigned(value),
            ..
        } => u64::try_from(value)
            .map(|value| value as i64)
            .map_err(|_| rejection("Windows output HANDLE exceeds its 64-bit carrier")),
        _ => signed(value),
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

fn directory_arguments(effect: Effect, arguments: &[TerminalScalarValue]) -> bool {
    let Effect::Directory { class, bytes } = effect else {
        return false;
    };
    arguments.len() == 9
        && scalar(arguments[1], 0)
        && scalar(arguments[2], 0)
        && scalar(arguments[3], 0)
        && scalar(arguments[4], i128::from(bytes))
        && scalar(arguments[5], i128::from(class))
        && scalar(arguments[6], 1)
        && scalar(arguments[7], 0)
        && (scalar(arguments[8], 0) || class == 1 && scalar(arguments[8], 1))
}

fn translate_attributes(
    record: &super::super::native_provider::ReadonlyRecord<'_>,
    translate: impl FnOnce(i64) -> Result<i64, TerminalEffectRejection>,
) -> Result<Option<Vec<u8>>, TerminalEffectRejection> {
    let invalid = || rejection("relative NT open changed its complete object-attributes contract");
    let name = record.sole_reference()?;
    let characters = name.sole_reference()?;
    if name.bytes().len() != 16 || characters.bytes().len() != 8192 {
        return Err(invalid());
    }
    let length = u16::from_le_bytes(name.bytes()[0..2].try_into().unwrap());
    let capacity = u16::from_le_bytes(name.bytes()[2..4].try_into().unwrap());
    if length == 0 || length > 8190 || length % 2 != 0 || capacity != 8192 {
        return Err(invalid());
    }
    let mut bytes = record.bytes().to_vec();
    if bytes.len() != 48 {
        return Err(invalid());
    }
    let u32_at = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let u64_at = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    if u32_at(0) != 48 || u32_at(24) != 4096 || u64_at(32) != 0 || u64_at(40) != 0 {
        return Err(invalid());
    }
    let guest = i64::from_le_bytes(bytes[8..16].try_into().unwrap());
    if guest == 0 || guest == -1 {
        return Err(invalid());
    }
    bytes[8..16].copy_from_slice(&translate(guest)?.to_le_bytes());
    Ok(Some(bytes))
}

fn replace_handle(
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
    let replacement = match contents {
        IntegerValue::Signed(_) => IntegerValue::Signed(i128::from(replacement)),
        IntegerValue::Unsigned(_) => IntegerValue::Unsigned(u128::from(replacement as u64)),
    };
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
        match self.effect {
            Effect::Acquire => index == 0,
            Effect::Path { count } => index < count,
            _ => false,
        }
    }

    pub(super) fn validate_path(
        &self,
        index: usize,
        bytes: &[u8],
    ) -> Result<(), TerminalEffectRejection> {
        // The captured bytes are the whole declared array, whatever its length.
        // A path without a terminator inside it would let the OS read past the
        // borrowed storage, so it is refused here rather than trusted.
        let (elements, partial) = bytes.as_chunks::<2>();
        if self.path_argument(index) && (!partial.is_empty() || !elements.contains(&[0, 0])) {
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
            rejection(format!(
                "Windows {} requires its exact synchronous/null/current-process inputs",
                String::from_utf8_lossy(self.symbol)
            ))
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
            b"ReadFile" | b"WriteFile" | b"SetFileTime" | b"GetFileTime" => {
                arguments.len() == 2 && scalar(arguments[1], 0)
            }
            b"NtQueryInformationFile" => {
                arguments.len() == 3 && scalar(arguments[1], 4) && scalar(arguments[2], 16)
            }
            b"CreatePipe" => {
                arguments.len() == 2 && scalar(arguments[0], 0) && scalar(arguments[1], 0)
            }
            b"GetFileInformationByHandleEx" => matches!(self.effect, Effect::Query { class, bytes }
                if arguments.len() == 3 && scalar(arguments[1], i128::from(class)) && scalar(arguments[2], i128::from(bytes))),
            b"NtQueryDirectoryFile" => directory_arguments(self.effect, arguments),
            b"NtOpenFile" => {
                arguments.len() == 3
                    && (scalar(arguments[0], 1114240) || scalar(arguments[0], 1114241))
                    && scalar(arguments[1], 7)
                    && scalar(arguments[2], 2113568)
            }
            b"SetFileInformationByHandle" => match self.effect {
                Effect::Disposition => {
                    arguments.len() == 3 && scalar(arguments[1], 4) && scalar(arguments[2], 1)
                }
                Effect::SetInformation { class, bytes } => {
                    arguments.len() == 3
                        && scalar(arguments[1], i128::from(class))
                        && scalar(arguments[2], i128::from(bytes))
                }
                _ => false,
            },
            b"SetFilePointerEx" => {
                arguments.len() == 3 && (0..=2).any(|method| scalar(arguments[2], method))
            }
            b"GetFinalPathNameByHandleW" => {
                arguments.len() == 3 && scalar(arguments[1], 4096) && scalar(arguments[2], 0)
            }
            b"CloseHandle" | b"GetFileType" | b"FlushFileBuffers" => arguments.len() == 1,
            b"CreateDirectoryW" => arguments.len() == 1 && scalar(arguments[0], 0),
            b"SetFileAttributesW" => arguments.len() == 1,
            b"MoveFileExW" => {
                arguments.len() == 1
                    && matches!(arguments[0], TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(flags), ..
                } if flags & !0x2b == 0)
            }
            b"DeleteFileW" | b"RemoveDirectoryW" => arguments.is_empty(),
            _ => false,
        };
        if !safe {
            return Err(invalid());
        }
        let index = match self.effect {
            Effect::Acquire | Effect::Path { .. } | Effect::Pipe | Effect::OpenRelative => {
                return Ok(arguments.to_vec());
            }
            Effect::Duplicate => 1,
            _ => 0,
        };
        let mut result = arguments.to_vec();
        let native = self
            .context
            .windows
            .borrow()
            .translate(signed(result[index])?)?;
        result[index] = replace_handle(result[index], native)?;
        Ok(result)
    }

    pub(super) fn result(
        &self,
        arguments: &[TerminalScalarValue],
        value: TerminalScalarValue,
    ) -> Result<TerminalScalarValue, TerminalEffectRejection> {
        match self.effect {
            Effect::Acquire => replace_handle(
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
        if matches!(self.effect, Effect::OpenRelative)
            && argument == 0
            && signed(
                result.ok_or_else(|| rejection("NT acquisition lost its completion status"))?,
            )? == 0
        {
            let native = handle_bits(value)?;
            if native == 0 || native == -1 {
                return Err(rejection(
                    "successful NT acquisition returned no owned HANDLE",
                ));
            }
            return replace_handle(value, self.context.windows.borrow_mut().acquire(native)?)
                .map(Some);
        }
        if (matches!(self.effect, Effect::Duplicate) && argument == 0)
            || (matches!(self.effect, Effect::Pipe) && argument < 2)
        {
            let status = signed(result.ok_or_else(|| {
                rejection("HANDLE acquisition lost its native completion status")
            })?)?;
            if status != 0 {
                let native = handle_bits(value)?;
                if native == 0 || native == -1 {
                    return Err(rejection(
                        "successful acquisition returned no owned native HANDLE",
                    ));
                }
                return replace_handle(value, self.context.windows.borrow_mut().acquire(native)?)
                    .map(Some);
            }
        }
        Ok(None)
    }

    pub(super) fn readonly_record_argument(&self, index: usize) -> bool {
        matches!(
            (self.effect, index),
            (Effect::OpenRelative, 1) | (Effect::Disposition, 0)
        )
    }

    pub(super) fn prepare_readonly_record(
        &self,
        index: usize,
        record: &super::super::native_provider::ReadonlyRecord<'_>,
    ) -> Result<Option<Vec<u8>>, TerminalEffectRejection> {
        self.namespace.map_err(rejection)?;
        match (self.effect, index) {
            (Effect::OpenRelative, 1) => translate_attributes(record, |guest| {
                self.context.windows.borrow().translate(guest)
            }),
            (Effect::Disposition, 0) if record.bytes() == [1] => Ok(None),
            _ => Err(rejection(
                "Windows readonly file request differs from its exact input record",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_call() -> Call {
        Call {
            context: FilesystemContext::new(
                crate::execution::TerminalFilesystemDelegation::HostNamespace,
            ),
            effect: Effect::Acquire,
            symbol: b"CreateFileW",
            namespace: Ok(()),
        }
    }

    #[test]
    fn path_of_any_length_needs_its_terminator_inside_the_array() {
        let call = open_call();
        // Four WCHARs "ab\0?" in a caller-sized array: terminated inside it.
        assert!(call.validate_path(0, &[97, 0, 98, 0, 0, 0, 7, 0]).is_ok());
        // No WCHAR is zero, although zero bytes straddle element boundaries.
        assert!(call.validate_path(0, &[97, 0, 0, 98, 7, 0, 0, 7]).is_err());
        // A capture that is not a whole number of WCHARs is not a path array.
        assert!(call.validate_path(0, &[97, 0, 0, 0, 0]).is_err());
        assert!(call.validate_path(0, &[]).is_err());
        // Only the path argument is checked.
        assert!(call.validate_path(1, &[97, 0]).is_ok());
    }
}

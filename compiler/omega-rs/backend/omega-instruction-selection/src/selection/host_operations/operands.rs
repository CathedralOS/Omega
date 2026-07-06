use crate::InstructionSelectionInput;
use crate::selection::bindings::RuntimeAliasResolutionContext;
use omega_calling_conventions::{HostCapability, HostOperation};
use omega_platform_interface::{
    HostCall, HostCallArgument, HostCallArgumentKind, LoweredHostOperation,
};

use super::runtime_text::{
    find_runtime_text_input_buffer_data_object, runtime_string_descriptor_place,
    runtime_text_literal_for_host_call,
};
use crate::selection::storage_places::resolve_runtime_storage_place_in_table;
use omega_abstract_operations::{
    AbstractDataObject, AbstractDataObjectHandle, InstructionOperand, InstructionOperandKind,
};
use omega_core::arena::{Arena, Handle, HandleSpan};

pub(super) fn select_host_operation_operands(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
    operation: &LoweredHostOperation,
    operands: &mut Arena<InstructionOperand>,
) -> HandleSpan<InstructionOperand> {
    if let Some(value) = operation.fixed_leading_immediate {
        return operands.insert_many([operand(InstructionOperandKind::ImmediateInteger(value))]);
    }

    match (
        operation.operation_key.capability,
        operation.operation_key.operation,
    ) {
        (HostCapability::Stdin, HostOperation::Read | HostOperation::ReadFile) => {
            let data_object_handle = find_data_object(input, host_call);
            if !data_object_handle.is_valid() {
                return HandleSpan::empty();
            };
            let byte_count = data_object_byte_count(input, data_object_handle);

            if operation.operation_key.operation == HostOperation::Read {
                return operands.insert_many([
                    operand(InstructionOperandKind::ImmediateInteger(0)),
                    operand(InstructionOperandKind::DataAddress {
                        data: data_object_handle,
                    }),
                    operand(InstructionOperandKind::ByteLength(byte_count)),
                ]);
            }

            operands.insert_many([
                operand(InstructionOperandKind::DataAddress {
                    data: data_object_handle,
                }),
                operand(InstructionOperandKind::ByteLength(byte_count)),
            ])
        }
        (
            HostCapability::Stdout | HostCapability::Stderr,
            HostOperation::Write | HostOperation::WriteFile,
        ) => {
            if let Some(place) =
                runtime_string_descriptor_place(input, host_call, dispatch_index, alias_context)
            {
                let pointer = if place.through_pointee {
                    InstructionOperandKind::RuntimePointeeStringPointer {
                        region: place.place.region,
                        byte_offset: place.place.byte_offset,
                    }
                } else {
                    InstructionOperandKind::RuntimeStringPointer {
                        region: place.place.region,
                        byte_offset: place.place.byte_offset,
                        is_bounded_buffer: place.is_bounded_buffer,
                    }
                };
                let length = if place.through_pointee {
                    InstructionOperandKind::RuntimePointeeStringLength {
                        region: place.place.region,
                        byte_offset: place.place.byte_offset,
                    }
                } else {
                    InstructionOperandKind::RuntimeStringLength {
                        region: place.place.region,
                        byte_offset: place.place.byte_offset,
                        is_bounded_buffer: place.is_bounded_buffer,
                    }
                };
                return console_write_operands(
                    operands,
                    operation.operation_key.capability,
                    operation.operation_key.operation,
                    pointer,
                    length,
                );
            }

            let direct_data_object = find_data_object(input, host_call);
            if direct_data_object.is_valid() {
                let byte_count = data_object_byte_count(input, direct_data_object);
                return console_write_operands(
                    operands,
                    operation.operation_key.capability,
                    operation.operation_key.operation,
                    InstructionOperandKind::DataAddress {
                        data: direct_data_object,
                    },
                    InstructionOperandKind::ByteLength(byte_count),
                );
            }

            if let Some(data_object) = find_runtime_text_input_buffer_data_object(input, host_call)
                && let Some(literal) = runtime_text_literal_for_host_call(input, host_call)
            {
                return console_write_operands(
                    operands,
                    operation.operation_key.capability,
                    operation.operation_key.operation,
                    InstructionOperandKind::DataAddress {
                        data: data_object_handle(input, data_object),
                    },
                    InstructionOperandKind::ByteLength(literal.len()),
                );
            }

            HandleSpan::empty()
        }
        (HostCapability::Input, HostOperation::KeyState) => {
            // operands = [result place, vk argument]: both must resolve or the
            // encoder hard-errors (no silent zero result / zero vk).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let argument = scalar_argument_operand_at(input, host_call, dispatch_index, 1);
            match (result, argument) {
                (Some(result), Some(argument)) => {
                    operands.insert_many([operand(result), operand(argument)])
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Gui, operation) => {
            select_gui_operation_operands(input, host_call, dispatch_index, operation, operands)
        }
        // A provides-sourced call (VtableSlot etc.): the operation key falls
        // OUTSIDE the closed catalog (Unknown), so there is no bespoke arm --
        // marshal the DECLARED arguments in order (each a scalar value or an
        // address-of), exactly as written. `this` lands in RCX, the rest
        // follow MS-x64. Void: no result operand.
        (HostCapability::Unknown, _) => {
            let arity = input
                .host_calls
                .arguments
                .span(host_call.arguments)
                .map_or(0, |arguments| arguments.len());
            let kinds: Option<Vec<InstructionOperandKind>> = (0..arity)
                .map(|index| {
                    scalar_argument_operand_at(input, host_call, dispatch_index, index)
                        .or_else(|| address_argument_operand_at(input, host_call, dispatch_index, index))
                })
                .collect();
            match kinds {
                Some(kinds) => operands.insert_many(kinds.into_iter().map(operand)),
                None => HandleSpan::empty(),
            }
        }
        (
            HostCapability::Process,
            HostOperation::Exit | HostOperation::ExitGroup | HostOperation::ExitProcess,
        )
        | (HostCapability::Clock, HostOperation::Sleep)
        | (HostCapability::Clock, HostOperation::TickCount) => {
            // Both take a single scalar first argument (exit code / sleep
            // milliseconds). A resolvable constant or runtime scalar lowers to a
            // marshallable operand; an unresolvable runtime argument lowers to NO
            // operand, so the architecture encoder hard-errors instead of silently
            // exiting `0` / sleeping `0`.
            match first_scalar_argument_operand(input, host_call, dispatch_index) {
                Some(kind) => operands.insert_many([operand(kind)]),
                None => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::Close | HostOperation::Dup) => {
            // Value-returning `rc = close(fd) -> _close(fd)` and
            // `new_fd = duplicate(fd) -> _dup(fd)` (identical one-fd shape; dup
            // returns the new fd instead of a status). operand[0] is the
            // result place (the assignment target, prepended by the
            // assignment-result collection); operand[1] is the fd. Either
            // unresolvable => no operands, so the encoder hard-errors rather
            // than storing a garbage rc / closing a garbage descriptor.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, 1);
            match (result, fd) {
                (Some(result), Some(fd)) => operands.insert_many([operand(result), operand(fd)]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::ReadErrno) => {
            // `errno = read_errno() -> ___error()` then deref. NO call args:
            // operand[0] is the result place, and that is the whole operand
            // list. Unresolvable result => no operands so the encoder errors.
            match first_scalar_argument_operand(input, host_call, dispatch_index) {
                Some(result) => operands.insert_many([operand(result)]),
                None => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::Sync) => {
            // Value-returning `rc = sync(fd) -> _fsync(fd)`. Same shape as
            // `close`: operand[0]=result place, [1]=fd; either unresolvable =>
            // no operands so the encoder hard-errors rather than syncing garbage.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, 1);
            match (result, fd) {
                (Some(result), Some(fd)) => operands.insert_many([operand(result), operand(fd)]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::Write) => {
            // Value-returning `n = write_bytes(fd, bytes) -> _write(fd, buf, len)`.
            // operand[0]=result, [1]=fd, then the buffer POINTER + LENGTH. The
            // first cut marshals a literal byte payload (its data object +
            // static length); a runtime buffer is a follow-up.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, 1);
            let data = find_data_object(input, host_call);
            match (result, fd) {
                (Some(result), Some(fd)) if data.is_valid() => {
                    // Literal payload: static data object + its length.
                    let length = data_object_byte_count(input, data);
                    operands.insert_many([
                        operand(result),
                        operand(fd),
                        operand(InstructionOperandKind::DataAddress { data }),
                        operand(InstructionOperandKind::ByteLength(length)),
                    ])
                }
                (Some(result), Some(fd)) => {
                    // Runtime slice payload (a `&[u8]` parameter/field): load the
                    // data pointer + length out of its descriptor.
                    match slice_argument_operands(input, host_call, dispatch_index, 2) {
                        Some((pointer, length)) => operands.insert_many([
                            operand(result),
                            operand(fd),
                            operand(pointer),
                            operand(length),
                        ]),
                        None => HandleSpan::empty(),
                    }
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::Read) => {
            // Value-returning `n = read_bytes(fd, buffer, count) -> _read(fd,
            // buf, count)`. operand[0]=result, [1]=fd, [2]=buffer POINTER (the
            // kernel writes through it), [3]=count. The caller passes the
            // buffer capacity as `count` (keeps the backend from deriving it).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, 1);
            let buffer = address_argument_operand_at(input, host_call, dispatch_index, 2);
            let count = scalar_argument_operand_at(input, host_call, dispatch_index, 3);
            match (result, fd, buffer, count) {
                (Some(result), Some(fd), Some(buffer), Some(count)) => operands.insert_many([
                    operand(result),
                    operand(fd),
                    operand(buffer),
                    operand(count),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::PRead) => {
            // Value-returning `n = read_at(fd, buf, count, offset) -> _pread(fd,
            // buf, count, offset)`. Same as `read` plus a trailing offset scalar:
            // operand[0]=result, [1]=fd, [2]=buffer POINTER, [3]=count, [4]=offset.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, 1);
            let buffer = address_argument_operand_at(input, host_call, dispatch_index, 2);
            let count = scalar_argument_operand_at(input, host_call, dispatch_index, 3);
            let offset = scalar_argument_operand_at(input, host_call, dispatch_index, 4);
            match (result, fd, buffer, count, offset) {
                (Some(result), Some(fd), Some(buffer), Some(count), Some(offset)) => operands
                    .insert_many([
                        operand(result),
                        operand(fd),
                        operand(buffer),
                        operand(count),
                        operand(offset),
                    ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::PWrite) => {
            // Value-returning `n = write_at(fd, bytes, offset) -> _pwrite(fd, buf,
            // len, offset)`. Same as `write` (literal or runtime slice payload)
            // plus a trailing offset scalar: operand[0]=result, [1]=fd,
            // [2]=buffer POINTER, [3]=length, [4]=offset. `bytes` is arg 2, so the
            // offset is arg 3.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, 1);
            let offset = scalar_argument_operand_at(input, host_call, dispatch_index, 3);
            let data = find_data_object(input, host_call);
            match (result, fd, offset) {
                (Some(result), Some(fd), Some(offset)) if data.is_valid() => {
                    let length = data_object_byte_count(input, data);
                    operands.insert_many([
                        operand(result),
                        operand(fd),
                        operand(InstructionOperandKind::DataAddress { data }),
                        operand(InstructionOperandKind::ByteLength(length)),
                        operand(offset),
                    ])
                }
                (Some(result), Some(fd), Some(offset)) => {
                    match slice_argument_operands(input, host_call, dispatch_index, 2) {
                        Some((pointer, length)) => operands.insert_many([
                            operand(result),
                            operand(fd),
                            operand(pointer),
                            operand(length),
                            operand(offset),
                        ]),
                        None => HandleSpan::empty(),
                    }
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::FStat | HostOperation::SetFileTimes) => {
            // `rc = read_file_metadata(fd, buf) -> _fstat(fd, buf)` and
            // `rc = set_file_times(fd, times) -> _futimens(fd, times)`: both are
            // `[result, fd scalar, buffer pointer]` (fstat's kernel WRITES the stat
            // record through the buffer; futimens READS two timespecs from it).
            // Same as `read` without the count -- keyed by an open descriptor.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, 1);
            let buffer = address_argument_operand_at(input, host_call, dispatch_index, 2);
            match (result, fd, buffer) {
                (Some(result), Some(fd), Some(buffer)) => {
                    operands.insert_many([operand(result), operand(fd), operand(buffer)])
                }
                _ => HandleSpan::empty(),
            }
        }
        (
            HostCapability::Filesystem,
            HostOperation::Open
            | HostOperation::Creat
            | HostOperation::MakeDir
            | HostOperation::Chmod,
        ) => {
            // Value-returning `fd = open_read(path, flags) -> _open(path,
            // flags)`, `fd = creat(path, mode) -> _creat(path, mode)`, and
            // `rc = set_permissions(path, mode) -> _chmod(path, mode)`. All are
            // `[result, path POINTER (NUL-terminated), second scalar]` -> args
            // `[path, flags-or-mode]`. The second args are NAMED (register)
            // params; creation uses `creat` precisely because `open`'s mode is
            // variadic (stack-passed on arm64) and would be dropped.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let path = path_pointer_operand(input, host_call, dispatch_index, 1);
            let second = scalar_argument_operand_at(input, host_call, dispatch_index, 2);
            match (result, path, second) {
                (Some(result), Some(path), Some(second)) => {
                    operands.insert_many([operand(result), operand(path), operand(second)])
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::ReadDir) => {
            // Value-returning `n = read_dir(fd, buf, count, position) ->
            // ___getdirentries64(fd, buf, count, &position)`. operand[0]=result,
            // [1]=fd, [2]=buffer POINTER (kernel writes dirent records), [3]=count
            // (buffer capacity), [4]=position POINTER (in/out i64 cursor).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, 1);
            let buffer = address_argument_operand_at(input, host_call, dispatch_index, 2);
            let count = scalar_argument_operand_at(input, host_call, dispatch_index, 3);
            let position = address_argument_operand_at(input, host_call, dispatch_index, 4);
            match (result, fd, buffer, count, position) {
                (Some(result), Some(fd), Some(buffer), Some(count), Some(position)) => operands
                    .insert_many([
                        operand(result),
                        operand(fd),
                        operand(buffer),
                        operand(count),
                        operand(position),
                    ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::ReadLink) => {
            // Value-returning `n = read_link(path, buf, count) -> _readlink(path,
            // buf, count)`. operand[0]=result, [1]=path POINTER (NUL-terminated),
            // [2]=buffer POINTER (kernel writes the target there), [3]=count.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let path = path_pointer_operand(input, host_call, dispatch_index, 1);
            let buffer = address_argument_operand_at(input, host_call, dispatch_index, 2);
            let count = scalar_argument_operand_at(input, host_call, dispatch_index, 3);
            match (result, path, buffer, count) {
                (Some(result), Some(path), Some(buffer), Some(count)) => operands.insert_many([
                    operand(result),
                    operand(path),
                    operand(buffer),
                    operand(count),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        (
            HostCapability::Filesystem,
            HostOperation::Stat | HostOperation::LStat | HostOperation::Realpath,
        ) => {
            // Value-returning `rc = read_metadata(path, buf) -> _stat(path, buf)`,
            // `rc = read_symlink_metadata(path, buf) -> _lstat(path, buf)`, and
            // `ptr = canonicalize(path, buf) -> _realpath(path, buf)` -- all share
            // the [result, path pointer, buffer pointer] shape (realpath's result
            // is the resolved-buffer pointer, used only as a non-NULL success flag).
            // operand[0]=result, [1]=path POINTER (NUL-terminated C string),
            // [2]=buffer POINTER (the kernel writes the 144-byte stat record).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let path = path_pointer_operand(input, host_call, dispatch_index, 1);
            let buffer = address_argument_operand_at(input, host_call, dispatch_index, 2);
            match (result, path, buffer) {
                (Some(result), Some(path), Some(buffer)) => {
                    operands.insert_many([operand(result), operand(path), operand(buffer)])
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::Unlink | HostOperation::RemoveDir) => {
            // Value-returning `rc = unlink(path) / rmdir(path)`.
            // operand[0]=result, [1]=path POINTER (NUL-terminated C string).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let path = path_pointer_operand(input, host_call, dispatch_index, 1);
            match (result, path) {
                (Some(result), Some(path)) => {
                    operands.insert_many([operand(result), operand(path)])
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::SetLen | HostOperation::Fchmod) => {
            // Value-returning `rc = set_len(fd, length) -> _ftruncate(fd, length)`
            // and `rc = set_file_permissions(fd, mode) -> _fchmod(fd, mode)`.
            // operand[0]=result, then the two scalar args (fd + length/mode).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, 1);
            let length = scalar_argument_operand_at(input, host_call, dispatch_index, 2);
            match (result, fd, length) {
                (Some(result), Some(fd), Some(length)) => {
                    operands.insert_many([operand(result), operand(fd), operand(length)])
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::Seek) => {
            // Value-returning `pos = seek(fd, offset, whence) -> _lseek(fd,
            // offset, whence)`. operand[0]=result, then the three scalar args.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, 1);
            let offset = scalar_argument_operand_at(input, host_call, dispatch_index, 2);
            let whence = scalar_argument_operand_at(input, host_call, dispatch_index, 3);
            match (result, fd, offset, whence) {
                (Some(result), Some(fd), Some(offset), Some(whence)) => operands.insert_many([
                    operand(result),
                    operand(fd),
                    operand(offset),
                    operand(whence),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        (
            HostCapability::Filesystem,
            HostOperation::Rename | HostOperation::Link | HostOperation::Symlink,
        ) => {
            // Value-returning `rc = rename(from, to) -> _rename(from, to)`,
            // `rc = hard_link(original, link) -> _link(original, link)`, and
            // `rc = symlink(target, linkpath) -> _symlink(target, linkpath)`.
            // operand[0]=result, [1]=first path POINTER, [2]=second path POINTER.
            // Two path LITERALS in one statement, resolved by creation order.
            // (Runtime-path forms are a future extension.)
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let from = find_nth_data_object(input, host_call, 0);
            let to = find_nth_data_object(input, host_call, 1);
            match result {
                Some(result) if from.is_valid() && to.is_valid() => operands.insert_many([
                    operand(result),
                    operand(InstructionOperandKind::DataAddress { data: from }),
                    operand(InstructionOperandKind::DataAddress { data: to }),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        _ => HandleSpan::empty(),
    }
}

pub(super) fn operand(kind: InstructionOperandKind) -> InstructionOperand {
    InstructionOperand { kind }
}

/// The SRCCOPY raster op (StretchDIBits' rop argument).
const SRCCOPY: i64 = 0x00CC_0020;

/// Build the FULL Win64 ABI operand list for a Gui import: `operands[0]` is the
/// RESULT place (every Gui op is value-returning and must be used in the
/// assignment form `self.h = self.gui.op(..)` -- the result-as-argument shape
/// the host-call collection produces), the rest are the callee's arguments in
/// call order, with the constant parameters the Omega surface hard-wires
/// (styles/origins/rops) interleaved as immediates. Wrong-arity calls (e.g. a
/// statement-position use, which has no result place) lower to NO operands, so
/// the encoder hard-errors instead of silently mis-marshalling (#40).
fn select_gui_operation_operands(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    operation: HostOperation,
    operands: &mut Arena<InstructionOperand>,
) -> HandleSpan<InstructionOperand> {
    let arity = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .map_or(0, |arguments| arguments.len());
    let scalar =
        |index: usize| scalar_argument_operand_at(input, host_call, dispatch_index, index);
    let address =
        |index: usize| address_argument_operand_at(input, host_call, dispatch_index, index);
    let imm = |value: i64| Some(InstructionOperandKind::ImmediateInteger(value));

    let kinds: Option<Vec<InstructionOperandKind>> = match operation {
        // dc_create() -> CreateCompatibleDC(NULL): [result].
        HostOperation::DcCreate if arity == 1 => {
            [scalar(0), imm(0)].into_iter().collect()
        }
        // get_dc(hwnd) -> GetDC(hwnd): [result, hwnd].
        HostOperation::GetDc if arity == 2 => [scalar(0), scalar(1)].into_iter().collect(),
        // is_window(hwnd) / window_destroy(hwnd): the same [result, hwnd] shape.
        HostOperation::IsWindow | HostOperation::WindowDestroy if arity == 2 => {
            [scalar(0), scalar(1)].into_iter().collect()
        }
        // msg_peek(msg) -> PeekMessageW(&msg, 0, 0, 0, PM_REMOVE): poll one
        // queued message into the caller's [u64; 6] MSG buffer.
        HostOperation::MsgPeek if arity == 2 => {
            [scalar(0), address(1), imm(0), imm(0), imm(0), imm(1)]
                .into_iter()
                .collect()
        }
        // msg_translate(msg) / msg_dispatch(msg): one MSG-buffer address arg.
        HostOperation::MsgTranslate | HostOperation::MsgDispatch if arity == 2 => {
            [scalar(0), address(1)].into_iter().collect()
        }
        // window_create(class, title, style, x, y, w, h) ->
        // CreateWindowExA(0, class, title, style, x, y, w, h, 0, 0, 0, 0).
        HostOperation::WindowCreate if arity == 8 => [
            scalar(0),
            imm(0), // dwExStyle
            address(1),
            address(2),
            scalar(3), // style
            scalar(4), // x
            scalar(5), // y
            scalar(6), // width
            scalar(7), // height
            imm(0), // hWndParent
            imm(0), // hMenu
            imm(0), // hInstance (NULL works for the system STATIC class)
            imm(0), // lpParam
        ]
        .into_iter()
        .collect(),
        // blit(hdc, dest_w, dest_h, src_w, src_h, pixels, info) ->
        // StretchDIBits(hdc, 0, 0, dest_w, dest_h, 0, 0, src_w, src_h, pixels,
        // info, DIB_RGB_COLORS, SRCCOPY). Separate dest/src sizes let a small
        // framebuffer stretch into a larger window; the return value is the
        // SOURCE scanline count (probed natively).
        HostOperation::Blit if arity == 8 => [
            scalar(0),
            scalar(1),  // hdc
            imm(0),     // xDest
            imm(0),     // yDest
            scalar(2),  // DestWidth
            scalar(3),  // DestHeight
            imm(0),     // xSrc
            imm(0),     // ySrc
            scalar(4),  // SrcWidth
            scalar(5),  // SrcHeight
            address(6), // bits
            address(7), // BITMAPINFO
            imm(0),     // DIB_RGB_COLORS
            imm(SRCCOPY),
        ]
        .into_iter()
        .collect(),
        _ => None,
    };
    match kinds {
        Some(kinds) => operands.insert_many(kinds.into_iter().map(operand)),
        None => HandleSpan::empty(),
    }
}

/// Resolve the host-call argument at `index` to the ADDRESS of its
/// runtime-storage place -- the pointer-argument shape (a `[u32; N]`
/// framebuffer, an OS-struct array, a byte-array C string). Unlike the scalar
/// resolution there is no width filter: the place IS the whole array; its
/// address is what marshals.
fn address_argument_operand_at(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    index: usize,
) -> Option<InstructionOperandKind> {
    let argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.get(index))?;
    let HostCallArgumentKind::Expression(expression) = &argument.kind else {
        return None;
    };
    resolve_runtime_storage_place_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        *expression,
    )
    .map(|place| InstructionOperandKind::RuntimeStorageAddress {
        region: place.region,
        byte_offset: place.byte_offset,
    })
}

/// Resolve a runtime `&[u8]` argument (a fat-pointer descriptor: `{ptr, len}`)
/// at `index` to its DATA pointer + length operands — the shape `_write` needs
/// when the payload is a slice value (a parameter/field), not a string literal.
/// The `RuntimeString{Pointer,Length}` operands load the ptr and len out of the
/// descriptor place; unlike `RuntimeStorageAddress` (a raw buffer address, used
/// by `read`) this dereferences one level.
fn slice_argument_operands(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    index: usize,
) -> Option<(InstructionOperandKind, InstructionOperandKind)> {
    let argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.get(index))?;
    let HostCallArgumentKind::Expression(expression) = &argument.kind else {
        return None;
    };
    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        *expression,
    )?;
    Some((
        InstructionOperandKind::RuntimeStringPointer {
            region: place.region,
            byte_offset: place.byte_offset,
            is_bounded_buffer: false,
        },
        InstructionOperandKind::RuntimeStringLength {
            region: place.region,
            byte_offset: place.byte_offset,
            is_bounded_buffer: false,
        },
    ))
}

/// The path POINTER operand for `creat`/`open`/`unlink` at `index`: a static
/// data object when the path is a literal, else the data pointer of a runtime
/// `&[u8] in Path` slice (which points at a NUL-terminated literal underneath,
/// so it is a valid C string). The length is irrelevant — these are C strings.
fn path_pointer_operand(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    index: usize,
) -> Option<InstructionOperandKind> {
    let data = find_data_object(input, host_call);
    if data.is_valid() {
        return Some(InstructionOperandKind::DataAddress { data });
    }
    slice_argument_operands(input, host_call, dispatch_index, index).map(|(pointer, _)| pointer)
}

/// File descriptor marshalled as the first `write` argument on the
/// syscall-style platforms (Linux syscall, darwin libSystem `_write`).
pub(super) fn write_file_descriptor(capability: HostCapability) -> i64 {
    if capability == HostCapability::Stderr {
        2
    } else {
        1
    }
}

fn console_write_operands(
    operands: &mut Arena<InstructionOperand>,
    capability: HostCapability,
    operation: HostOperation,
    first: InstructionOperandKind,
    second: InstructionOperandKind,
) -> HandleSpan<InstructionOperand> {
    if operation == HostOperation::Write {
        return operands.insert_many([
            operand(InstructionOperandKind::ImmediateInteger(
                write_file_descriptor(capability),
            )),
            operand(first),
            operand(second),
        ]);
    }

    operands.insert_many([operand(first), operand(second)])
}

fn find_data_object(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
) -> AbstractDataObjectHandle {
    input
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == host_call.source_key
                && data_object.source_statement == host_call.statement_index
        })
        .map(|(handle, _)| handle)
        .unwrap_or_else(AbstractDataObjectHandle::invalid)
}

/// The `n`-th static data object (string literal) of a host-call statement, in
/// creation/offset order — for calls with MORE THAN ONE literal argument (e.g.
/// `rename(from, to)`), where `find_data_object` (which returns the first match)
/// is ambiguous. Literals are emitted in argument order, so index 0 is the
/// first path literal, 1 the second.
fn find_nth_data_object(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    n: usize,
) -> AbstractDataObjectHandle {
    let mut matches: Vec<(AbstractDataObjectHandle, usize)> = input
        .data
        .objects
        .iter()
        .filter(|(_, data_object)| {
            data_object.source_key == host_call.source_key
                && data_object.source_statement == host_call.statement_index
        })
        .map(|(handle, data_object)| (handle, data_object.offset))
        .collect();
    matches.sort_by_key(|(_, offset)| *offset);
    matches
        .get(n)
        .map(|(handle, _)| *handle)
        .unwrap_or_else(AbstractDataObjectHandle::invalid)
}

fn data_object_byte_count(
    input: &InstructionSelectionInput<'_>,
    data_object: AbstractDataObjectHandle,
) -> usize {
    input
        .data
        .bytes
        .span(input.data.objects.get(data_object).bytes)
        .map_or(0, |bytes| bytes.len())
}

pub(super) fn data_object_handle(
    input: &InstructionSelectionInput<'_>,
    target: &AbstractDataObject,
) -> AbstractDataObjectHandle {
    input
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == target.source_key
                && data_object.source_statement == target.source_statement
                && data_object.offset == target.offset
        })
        .map(|(handle, _)| handle)
        .unwrap_or_else(Handle::invalid)
}

/// Resolve the operand for a host call's single scalar first argument (an
/// `exit_process`/`exit_group` exit code, or a `sleep` millisecond count).
///
/// A compile-time-constant argument (`exit_process(70)`, `sleep(33)`) lowers to an
/// `ImmediateInteger`. A runtime argument (`exit_process(self.v)`, a field/local
/// resolvable to a runtime-storage scalar slot) lowers to a `RuntimeScalarInteger`,
/// which the encoders load from the relocated region into the argument register.
///
/// Returns `None` for a runtime argument we cannot resolve to a marshallable scalar
/// slot; the caller then emits no operand at all, so the architecture encoder
/// hard-errors with a Diagnostic rather than silently exiting/sleeping `0`.
fn first_scalar_argument_operand(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
) -> Option<InstructionOperandKind> {
    let Some(argument) = first_argument(host_call, input) else {
        return Some(InstructionOperandKind::ImmediateInteger(0));
    };
    match &argument.kind {
        HostCallArgumentKind::Integer(value) => {
            Some(InstructionOperandKind::ImmediateInteger(*value))
        }
        HostCallArgumentKind::Expression(expression) => resolve_runtime_storage_place_in_table(
            input,
            dispatch_index.unwrap_or(0),
            host_call.source_key,
            &input.host_calls.expressions,
            *expression,
        )
        .filter(|place| matches!(place.byte_count, 1 | 2 | 4 | 8))
        .map(|place| InstructionOperandKind::RuntimeScalarInteger {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_count: place.byte_count,
        }),
        HostCallArgumentKind::Text(_) => Some(InstructionOperandKind::ImmediateInteger(0)),
    }
}

/// Scalar operand resolution for the host-call argument at `index` -- the same
/// shapes `first_scalar_argument_operand` accepts (constant integer or a
/// resolvable runtime-storage scalar place). `None` when absent/unresolvable.
fn scalar_argument_operand_at(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    index: usize,
) -> Option<InstructionOperandKind> {
    let argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.get(index))?;
    match &argument.kind {
        HostCallArgumentKind::Integer(value) => {
            Some(InstructionOperandKind::ImmediateInteger(*value))
        }
        HostCallArgumentKind::Expression(expression) => resolve_runtime_storage_place_in_table(
            input,
            dispatch_index.unwrap_or(0),
            host_call.source_key,
            &input.host_calls.expressions,
            *expression,
        )
        .filter(|place| matches!(place.byte_count, 1 | 2 | 4 | 8))
        .map(|place| InstructionOperandKind::RuntimeScalarInteger {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_count: place.byte_count,
        }),
        _ => None,
    }
}

fn first_argument<'plan>(
    host_call: &HostCall,
    input: &'plan InstructionSelectionInput<'plan>,
) -> Option<&'plan HostCallArgument> {
    input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.first())
}

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

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

use crate::InstructionSelectionInput;
use crate::selection::bindings::RuntimeAliasResolutionContext;
use omega_calling_conventions::{CallingPolicy, HostCapability, HostOperation, PlatformCallData};
use omega_platform_interface::{
    HostCall, HostCallArgument, HostCallArgumentKind, LoweredHostOperation,
};

use super::runtime_text::{
    find_runtime_text_input_buffer_data_object, runtime_string_descriptor_place,
    runtime_text_literal_for_host_call,
};
use crate::selection::storage_places::{
    RuntimeStoragePlace, resolve_fixed_array_length_in_table,
    resolve_runtime_storage_leaf_descriptor_in_table, resolve_runtime_storage_place_in_table,
};
use omega_abstract_operations::{
    AbstractDataObject, AbstractDataObjectHandle, InstructionOperand, InstructionOperandKind,
};
use omega_layout::{DataShape, TypeLayoutDescriptor};
use psi_arena::{Arena, Handle, HandleSpan};
use psi_checked_trees::expression::{ExpressionNode, ExpressionTable};
use psi_checked_trees::types::PrimitiveType;

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

            // A checked adapter can forward a String parameter whose caller
            // supplied a literal. At the host leaf the argument is still the
            // parameter name, while the read-only data object belongs to the
            // caller's statement. Resolve through the ordinary alias chain so
            // literal and field-backed String arguments share the same adapter
            // surface without requiring a synthetic runtime descriptor.
            if let Some((data, byte_count)) =
                aliased_literal_data_object(input, host_call, alias_context, 0)
            {
                return console_write_operands(
                    operands,
                    operation.operation_key.capability,
                    operation.operation_key.operation,
                    InstructionOperandKind::DataAddress { data },
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
            let argument =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
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
        // A source external call (VtableSlot / authored DllImport): the
        // operation key falls OUTSIDE the closed catalog (Unknown), so there
        // is no bespoke arm -- marshal the DECLARED arguments in order (each
        // a scalar value, preserved native small aggregate, or address-of),
        // exactly as written. The native call plan owns their eventual ABI
        // locations. When the collector prepended a
        // RESULT place (`let m = self.b.beep(v)`), argument[0] is a WRITABLE
        // place and must lower as the result-storage operand, not be read as
        // a value; the declared arguments then start at index 1. Any
        // unresolvable operand => NO operands, so the encoder hard-errors
        // rather than calling with garbage.
        (HostCapability::Unknown | HostCapability::Custom(_), _) => {
            let arity = input
                .host_calls
                .arguments
                .span(host_call.arguments)
                .map_or(0, |arguments| arguments.len());
            let result = if host_call.has_result {
                native_hfa_argument_operand_at(input, host_call, dispatch_index, alias_context, 0)
                    .or_else(|| {
                        system_v_classified_aggregate_operand_at(
                            input,
                            host_call,
                            dispatch_index,
                            alias_context,
                            0,
                        )
                    })
                    .or_else(|| {
                        native_large_aggregate_argument_operand_at(
                            input,
                            host_call,
                            dispatch_index,
                            alias_context,
                            0,
                        )
                    })
                    .or_else(|| {
                        native_small_aggregate_argument_operand_at(
                            input,
                            host_call,
                            dispatch_index,
                            alias_context,
                            0,
                        )
                    })
                    .or_else(|| {
                        authored_float_argument_operand_at(
                            input,
                            host_call,
                            dispatch_index,
                            alias_context,
                            0,
                        )
                    })
                    .or_else(|| first_scalar_argument_operand(input, host_call, dispatch_index))
            } else {
                None
            };
            if host_call.has_result && result.is_none() {
                return HandleSpan::empty();
            }
            let first_declared = usize::from(host_call.has_result);
            let kinds: Option<Vec<InstructionOperandKind>> = (first_declared..arity)
                .map(|index| {
                    // A BORROW argument (`&mut self.map_size`, `&self.msg`)
                    // marshals its ADDRESS -- the out-param shape
                    // (GetMemoryMap's five): the callee writes through the
                    // pointer. Scalar-first here read the POINTEE value and
                    // handed firmware garbage integers as write targets
                    // (caught cross-compiling the M2 out-param canary,
                    // 2026-07-17). Non-borrow arguments keep scalar-first
                    // (an aggregate still falls through to its address).
                    if host_call_argument_is_borrow(input, host_call, dispatch_index, index) {
                        address_argument_operand_at(
                            input,
                            host_call,
                            dispatch_index,
                            alias_context,
                            index,
                        )
                        .or_else(|| {
                            scalar_argument_operand_at(
                                input,
                                host_call,
                                dispatch_index,
                                alias_context,
                                index,
                            )
                        })
                    } else {
                        native_hfa_argument_operand_at(
                            input,
                            host_call,
                            dispatch_index,
                            alias_context,
                            index,
                        )
                        .or_else(|| {
                            system_v_classified_aggregate_operand_at(
                                input,
                                host_call,
                                dispatch_index,
                                alias_context,
                                index,
                            )
                        })
                        .or_else(|| {
                            native_large_aggregate_argument_operand_at(
                                input,
                                host_call,
                                dispatch_index,
                                alias_context,
                                index,
                            )
                        })
                        .or_else(|| {
                            native_small_aggregate_argument_operand_at(
                                input,
                                host_call,
                                dispatch_index,
                                alias_context,
                                index,
                            )
                        })
                        .or_else(|| {
                            authored_float_argument_operand_at(
                                input,
                                host_call,
                                dispatch_index,
                                alias_context,
                                index,
                            )
                        })
                        .or_else(|| {
                            scalar_argument_operand_at(
                                input,
                                host_call,
                                dispatch_index,
                                alias_context,
                                index,
                            )
                        })
                    }
                })
                .collect();
            match kinds {
                Some(kinds) => operands.insert_many(result.into_iter().chain(kinds).map(operand)),
                None => HandleSpan::empty(),
            }
        }
        (
            HostCapability::Process,
            HostOperation::Exit | HostOperation::ExitGroup | HostOperation::ExitProcess,
        )
        | (HostCapability::Clock, HostOperation::Sleep) => {
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
        (HostCapability::Clock, HostOperation::SleepPoll) => {
            // `sleep(ms) -> poll(NULL, 0, ms)`: darwin's poll-based millisecond
            // sleep. Two synthesized constant args (fds = NULL → x0, nfds = 0 → x1)
            // precede the boundary `milliseconds` arg (→ x2, poll's `timeout`, which
            // is already in milliseconds). An unresolvable `ms` lowers to NO
            // operands so the encoder hard-errors rather than sleeping garbage.
            match first_scalar_argument_operand(input, host_call, dispatch_index) {
                Some(ms) => operands.insert_many([
                    operand(InstructionOperandKind::ImmediateInteger(0)),
                    operand(InstructionOperandKind::ImmediateInteger(0)),
                    operand(ms),
                ]),
                None => HandleSpan::empty(),
            }
        }
        (
            HostCapability::Clock,
            HostOperation::MonotonicTicks
            | HostOperation::MonotonicTicksPerSecond
            | HostOperation::WallClockRaw
            | HostOperation::TickCount,
        ) => {
            // Value-returning, surface-ARGUMENT-FREE time reads (std::time
            // rungs 5/10). The SHAPE is per-target row DATA, so the arm is
            // data-driven: windows QPC/QPF/FILETIME are OUT-PARAM imports
            // (data None -> [result] only; the x86_64 encoder brackets the
            // call with the out-param stack slot); darwin's
            // `clock_gettime_nsec_np` takes an injected clockid
            // (ConstantArgument -> [result, imm]); darwin's frequency is the
            // POSIX constant (ConstantResult -> [result, imm], no call).
            // Unresolvable result => no operands so the encoder hard-errors
            // rather than storing garbage.
            match first_scalar_argument_operand(input, host_call, dispatch_index) {
                Some(result) => match host_call.data {
                    PlatformCallData::ConstantArgument { value }
                    | PlatformCallData::ConstantResult { value } => operands.insert_many([
                        operand(result),
                        operand(InstructionOperandKind::ImmediateInteger(value)),
                    ]),
                    PlatformCallData::TimespecResult { clock_id } => operands.insert_many([
                        operand(result),
                        operand(InstructionOperandKind::ImmediateInteger(clock_id)),
                    ]),
                    _ => operands.insert_many([operand(result)]),
                },
                None => HandleSpan::empty(),
            }
        }
        (
            HostCapability::Clock,
            HostOperation::WallClockUnitsPerSecond | HostOperation::WallClockEpochOffsetSeconds,
        ) => {
            // Per-target calibration CONSTANTS (D11: the lowering layer never
            // does arithmetic — it only publishes the two constants the proven
            // wrapper math divides by). No call at all: operand[0] is the
            // result place, operand[1] the constant from the platform-lowering
            // row's `ConstantResult` data. A row without that data or an
            // unresolvable result => no operands so the encoder hard-errors.
            let PlatformCallData::ConstantResult { value } = host_call.data else {
                return HandleSpan::empty();
            };
            match first_scalar_argument_operand(input, host_call, dispatch_index) {
                Some(result) => operands.insert_many([
                    operand(result),
                    operand(InstructionOperandKind::ImmediateInteger(value)),
                ]),
                None => HandleSpan::empty(),
            }
        }
        (
            HostCapability::Filesystem,
            HostOperation::Close
            | HostOperation::CloseHandle
            | HostOperation::Dup
            | HostOperation::FindClose
            | HostOperation::GetOsfHandle,
        ) => {
            // `handle = get_osfhandle(fd) -> _get_osfhandle(fd)` (session
            // slice 4a) rides the same one-scalar shape below.
            // Value-returning `rc = close(fd) -> _close(fd)` and
            // `new_fd = duplicate(fd) -> _dup(fd)` (identical one-fd shape; dup
            // returns the new fd instead of a status). `rc = find_close(handle)
            // -> FindClose(handle)` (fs rung 3a) is the same shape with the find
            // HANDLE as the scalar. operand[0] is the
            // result place (the assignment target, prepended by the
            // assignment-result collection); operand[1] is the fd. Either
            // unresolvable => no operands, so the encoder hard-errors rather
            // than storing a garbage rc / closing a garbage descriptor.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            match (result, fd) {
                (Some(result), Some(fd)) => operands.insert_many([operand(result), operand(fd)]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::ReadErrno | HostOperation::GetLastError) => {
            // `errno = read_errno() -> ___error()` then deref, or
            // `error = get_last_error() -> GetLastError()` directly. NO call args:
            // operand[0] is the result place, and that is the whole operand
            // list. Unresolvable result => no operands so the encoder errors.
            match first_scalar_argument_operand(input, host_call, dispatch_index) {
                Some(result) => operands.insert_many([operand(result)]),
                None => HandleSpan::empty(),
            }
        }
        (HostCapability::Math, HostOperation::RoundNearest) => {
            // Value-returning `n = round_nearest(x: f64) -> _lround(x)`. operand[0]
            // is the i64 result place (returned in x0, same as the scalar fs ops);
            // operand[1] is the f64 ARGUMENT, marshalled into v0 via a
            // `RuntimeScalarFloat` operand. Either unresolvable => no operands so
            // the encoder hard-errors rather than rounding garbage.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let arg = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            match (result, arg) {
                (Some(result), Some(arg)) => operands.insert_many([operand(result), operand(arg)]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Math, HostOperation::SquareRoot) => {
            // Value-returning `r = square_root(x: f64) -> _sqrt(x)`. operand[0] is
            // the f64 result place (returned in d0 → moved to x0 by `fmov x0,d0`,
            // then stored as raw 8 bytes — bit-identical to an i64 store, so it is
            // built as a plain scalar result operand); operand[1] is the f64
            // ARGUMENT in v0. Either unresolvable => no operands so the encoder
            // hard-errors rather than storing garbage.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let arg = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            match (result, arg) {
                (Some(result), Some(arg)) => operands.insert_many([operand(result), operand(arg)]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Math, HostOperation::Hypotenuse) => {
            // Value-returning `r = hypotenuse(x, y) -> _hypot(x, y)`. operand[0] is
            // the f64 result place; operand[1]/[2] are the two f64 ARGUMENTS,
            // marshalled into v0 and v1 by consecutive `RuntimeScalarFloat`
            // operands (the vreg counter in `append_call_operands` sequences them
            // independently of the x-register file). Any unresolvable => no
            // operands so the encoder hard-errors.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let x = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let y = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            match (result, x, y) {
                (Some(result), Some(x), Some(y)) => {
                    operands.insert_many([operand(result), operand(x), operand(y)])
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Math, HostOperation::FusedMultiplyAdd) => {
            // Value-returning `r = fused_multiply_add(x, y, z) -> _fma(x, y, z)`.
            // operand[0] the f64 result place; operand[1]/[2]/[3] the three f64 args
            // in v0, v1, v2 (consecutive `RuntimeScalarFloat` operands sequenced by
            // the vreg counter). Any unresolvable => no operands (encoder errors).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let x = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let y = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let z = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            match (result, x, y, z) {
                (Some(result), Some(x), Some(y), Some(z)) => {
                    operands.insert_many([operand(result), operand(x), operand(y), operand(z)])
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::ObjectiveC, HostOperation::GetClass | HostOperation::RegisterSelector) => {
            // Value-returning `p = get_class(name) -> _objc_getClass(name)` /
            // `sel = register_selector(name) -> _sel_registerName(name)`. operand[0]
            // the u64 result place (Class/SEL pointer in x0), operand[1] the
            // NUL-terminated name string POINTER (materialized like an fs path).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let name = path_pointer_operand(input, host_call, dispatch_index, alias_context, 1);
            match (result, name) {
                (Some(result), Some(name)) => {
                    operands.insert_many([operand(result), operand(name)])
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::ObjectiveC, HostOperation::MsgSend) => {
            // Value-returning `r = send(recv, sel) -> _objc_msgSend(recv, sel)`.
            // operand[0] result (id/scalar in x0); [1] recv → x0; [2] sel → x1.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let recv =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let sel =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            match (result, recv, sel) {
                (Some(result), Some(recv), Some(sel)) => {
                    operands.insert_many([operand(result), operand(recv), operand(sel)])
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::ObjectiveC, HostOperation::MsgSendScalar) => {
            // `r = send_scalar(recv, sel, arg) -> _objc_msgSend(recv, sel, arg)`.
            // operand[0] result; [1] recv → x0; [2] sel → x1; [3] the scalar
            // int/ptr/BOOL argument → x2. All three args are plain scalar values.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let recv =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let sel =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let arg =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            match (result, recv, sel, arg) {
                (Some(result), Some(recv), Some(sel), Some(arg)) => operands.insert_many([
                    operand(result),
                    operand(recv),
                    operand(sel),
                    operand(arg),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::ObjectiveC, HostOperation::MsgSendString) => {
            // `r = send_string(recv, sel, text) -> _objc_msgSend(recv, sel, char*)`.
            // operand[0] result; [1] recv → x0; [2] sel → x1; [3] the NUL-terminated
            // C-string arg pointer → x2 (materialized like an fs path).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let recv =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let sel =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let text = path_pointer_operand(input, host_call, dispatch_index, alias_context, 3);
            match (result, recv, sel, text) {
                (Some(result), Some(recv), Some(sel), Some(text)) => operands.insert_many([
                    operand(result),
                    operand(recv),
                    operand(sel),
                    operand(text),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::ObjectiveC, HostOperation::MsgSendRect) => {
            // `r = send_rect(recv, sel, x, y, w, h, a, b, c) -> _objc_msgSend(recv,
            // sel, NSRect{x,y,w,h}, a, b, c)`. The MIXED call: recv/sel/a/b/c are
            // SCALARS (→ x0,x1,x2,x3,x4 in list order) and x/y/w/h are FLOATS (→
            // v0,v1,v2,v3). The two register counters advance independently, so the
            // interleaving in this operand list does not affect placement.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let recv =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let sel =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let x = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            let y = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 4);
            let w = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 5);
            let h = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 6);
            let a = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 7);
            let b = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 8);
            let c = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 9);
            match (result, recv, sel, x, y, w, h, a, b, c) {
                (
                    Some(result),
                    Some(recv),
                    Some(sel),
                    Some(x),
                    Some(y),
                    Some(w),
                    Some(h),
                    Some(a),
                    Some(b),
                    Some(c),
                ) => operands.insert_many([
                    operand(result),
                    operand(recv),
                    operand(sel),
                    operand(x),
                    operand(y),
                    operand(w),
                    operand(h),
                    operand(a),
                    operand(b),
                    operand(c),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::ObjectiveC, HostOperation::MsgSendScalar4) => {
            // `r = send_scalar4(recv, sel, a, b, c, d) -> _objc_msgSend(recv, sel, a,
            // b, c, d)`. Six scalar args in list order → x0,x1,x2,x3,x4,x5. For the
            // event pump's nextEventMatchingMask:untilDate:inMode:dequeue:.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let recv =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let sel =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let a = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            let b = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 4);
            let c = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 5);
            let d = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 6);
            match (result, recv, sel, a, b, c, d) {
                (Some(result), Some(recv), Some(sel), Some(a), Some(b), Some(c), Some(d)) => {
                    operands.insert_many([
                        operand(result),
                        operand(recv),
                        operand(sel),
                        operand(a),
                        operand(b),
                        operand(c),
                        operand(d),
                    ])
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::ObjectiveC, HostOperation::MsgSendImageSize) => {
            // `r = send_image_size(recv, sel, image, w, h) -> _objc_msgSend(recv,
            // sel, image, NSSize{w,h})`. recv/sel/image are SCALARS (→ x0,x1,x2) and
            // w/h are FLOATS (the NSSize → v0,v1) — the two register counters are
            // independent. operand[0]=result, then [recv, sel, image, w, h].
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let recv =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let sel =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let image =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            let w = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 4);
            let h = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 5);
            match (result, recv, sel, image, w, h) {
                (Some(result), Some(recv), Some(sel), Some(image), Some(w), Some(h)) => operands
                    .insert_many([
                        operand(result),
                        operand(recv),
                        operand(sel),
                        operand(image),
                        operand(w),
                        operand(h),
                    ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::ObjectiveC, HostOperation::MsgSendByteString) => {
            // `r = send_byte_string(recv, sel, text) -> _objc_msgSend(recv, sel,
            // byte*)`. operand[0] result; [1] recv → x0; [2] sel → x1; [3] the
            // ADDRESS of a runtime byte buffer → x2 (unlike `send_string`, whose
            // C-string is a compile-time literal materialized like an fs path).
            // The callee reads to the first NUL; the buffer is NUL-terminated by
            // construction at the call sites.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let recv =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let sel =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let text =
                address_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            match (result, recv, sel, text) {
                (Some(result), Some(recv), Some(sel), Some(text)) => operands.insert_many([
                    operand(result),
                    operand(recv),
                    operand(sel),
                    operand(text),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::ObjectiveC, HostOperation::PoolPush) => {
            // `pool = pool_push() -> _objc_autoreleasePoolPush()`. NO args; just the
            // result place (the pool token in x0), like `color_space_rgb`.
            match first_scalar_argument_operand(input, host_call, dispatch_index) {
                Some(result) => operands.insert_many([operand(result)]),
                None => HandleSpan::empty(),
            }
        }
        (HostCapability::ObjectiveC, HostOperation::PoolPop) => {
            // `_ = pool_pop(pool) -> _objc_autoreleasePoolPop(pool)`: one scalar arg
            // (the push token → x0). A void C call; the result place is scratch.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let pool =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            match (result, pool) {
                (Some(result), Some(pool)) => {
                    operands.insert_many([operand(result), operand(pool)])
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::CoreGraphics, HostOperation::RectMaxX | HostOperation::RectMaxY) => {
            // `r = rect_max_x(x, y, w, h) -> _CGRectGetMaxX({x,y,w,h})`. The CGRect's
            // 4 doubles marshal as an HFA into v0–v3 (four consecutive
            // `RuntimeScalarFloat` operands sequenced by the vreg counter); the f64
            // result comes back in d0 (`returns_float`). operand[0] is the f64 result
            // place. Any unresolvable float => no operands (encoder hard-errors).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let x = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let y = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let w = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            let h = float_argument_operand_at(input, host_call, dispatch_index, alias_context, 4);
            match (result, x, y, w, h) {
                (Some(result), Some(x), Some(y), Some(w), Some(h)) => operands.insert_many([
                    operand(result),
                    operand(x),
                    operand(y),
                    operand(w),
                    operand(h),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::CoreGraphics, HostOperation::ColorSpaceRgb) => {
            // `space = color_space_rgb() -> _CGColorSpaceCreateDeviceRGB()`. NO args;
            // just the result place (ptr in x0). operand[0] alone.
            match first_scalar_argument_operand(input, host_call, dispatch_index) {
                Some(result) => operands.insert_many([operand(result)]),
                None => HandleSpan::empty(),
            }
        }
        (HostCapability::CoreGraphics, HostOperation::BitmapContext) => {
            // `ctx = bitmap_context(data, w, h, bpc, stride, space, info) ->
            // _CGBitmapContextCreate(...)`. SEVEN args → x0–x6: operand[1] is the
            // framebuffer POINTER (address of the `[i32;N]` field), the rest are
            // integer/pointer scalars. Result (CGContextRef) in x0.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let data =
                address_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let w = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let h = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            let bpc =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 4);
            let stride =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 5);
            let space =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 6);
            let info =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 7);
            match (result, data, w, h, bpc, stride, space, info) {
                (
                    Some(result),
                    Some(data),
                    Some(w),
                    Some(h),
                    Some(bpc),
                    Some(stride),
                    Some(space),
                    Some(info),
                ) => operands.insert_many([
                    operand(result),
                    operand(data),
                    operand(w),
                    operand(h),
                    operand(bpc),
                    operand(stride),
                    operand(space),
                    operand(info),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        (
            HostCapability::CoreGraphics,
            HostOperation::BitmapContextImage
            | HostOperation::ImageWidth
            | HostOperation::ContextRelease
            | HostOperation::ImageRelease,
        ) => {
            // `img = bitmap_context_image(ctx)` / `w = image_width(img)` /
            // `_ = context_release(ctx)` / `_ = image_release(img)`: one
            // pointer arg (→ x0), result in x0 (scratch for the void releases).
            // operand[0]=result, [1]=the ptr.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let arg =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            match (result, arg) {
                (Some(result), Some(arg)) => operands.insert_many([operand(result), operand(arg)]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::CoreGraphics, HostOperation::EventSourceKeyState) => {
            // `down = event_source_key_state(state_id, keycode) ->
            // CGEventSourceKeyState(state_id, keycode)`: two scalar args (state_id →
            // x0, keycode → x1) in list order, BOOL result (0/1) in x0.
            // operand[0]=result, [1]=state_id, [2]=keycode.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let state_id =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let keycode =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            match (result, state_id, keycode) {
                (Some(result), Some(state_id), Some(keycode)) => {
                    operands.insert_many([operand(result), operand(state_id), operand(keycode)])
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::Sync) => {
            // Value-returning `rc = sync(fd) -> _fsync(fd)`. Same shape as
            // `close`: operand[0]=result place, [1]=fd; either unresolvable =>
            // no operands so the encoder hard-errors rather than syncing garbage.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
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
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            // A slice-literal byte payload forwarded through a VALUE-CALL param
            // (`fs.write_all(path, "hi")` -> wrapper `write(fd, bytes)`) arrives as
            // the callee's `bytes` param aliased to the caller's literal. That
            // literal's data object is keyed to the CALLER's statement, so
            // `find_data_object` (keyed to THIS host-call's statement) misses it.
            // Resolve the arg through the alias chain to its literal data object.
            let data = {
                let direct = find_data_object(input, host_call);
                if direct.is_valid() {
                    direct
                } else {
                    aliased_literal_data_object(input, host_call, alias_context, 2)
                        .map_or_else(AbstractDataObjectHandle::invalid, |(handle, _)| handle)
                }
            };
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
                    // A FIXED-ARRAY `[u8; N]` payload (a plain byte buffer, e.g. the
                    // read-then-write buffer in `copy`) is NOT a `{ptr, len}`
                    // descriptor: marshal its raw ADDRESS + the static length N,
                    // exactly as `read` marshals its buffer. (`resolve_fixed_array_
                    // length_in_table` returns None for a `&[u8]` slice, which then
                    // falls through to the descriptor path below.)
                    let fixed_array = host_call_argument_expression(input, host_call, 2)
                        .and_then(|expression| {
                            resolve_fixed_array_length_in_table(
                                input,
                                dispatch_index.unwrap_or(0),
                                host_call.source_key,
                                &input.host_calls.expressions,
                                expression,
                            )
                        })
                        .zip(address_argument_operand_at(
                            input,
                            host_call,
                            dispatch_index,
                            alias_context,
                            2,
                        ));
                    if let Some((length, address)) = fixed_array {
                        operands.insert_many([
                            operand(result),
                            operand(fd),
                            operand(address),
                            operand(InstructionOperandKind::ByteLength(length)),
                        ])
                    } else if let Some((pointer, length)) =
                        subslice_argument_operands(input, host_call, dispatch_index, 2)
                    {
                        // A RUNTIME-length subslice `buffer[0..n]` (the faithful
                        // copy: write exactly the `n` bytes just read). Not a
                        // `{ptr, len}` descriptor -- marshal the fixed array's base
                        // ADDRESS + the range end loaded as a runtime scalar length.
                        operands.insert_many([
                            operand(result),
                            operand(fd),
                            operand(pointer),
                            operand(length),
                        ])
                    } else if let Some((pointer, length)) =
                        slice_argument_operands(input, host_call, dispatch_index, 2)
                    {
                        // Runtime slice payload (a `&[u8]` parameter/field): load the
                        // data pointer + length out of its descriptor.
                        operands.insert_many([
                            operand(result),
                            operand(fd),
                            operand(pointer),
                            operand(length),
                        ])
                    } else if let Some((length, address)) = alias_resolved_fixed_array_length_at(
                        input,
                        host_call,
                        dispatch_index,
                        alias_context,
                        2,
                    )
                    .zip(address_argument_operand_at(
                        input,
                        host_call,
                        dispatch_index,
                        alias_context,
                        2,
                    )) {
                        // LAST RESORT -- a fixed-array FIELD forwarded through a
                        // value-call param (`fs.write_all(path, self.bin_src)` ->
                        // the wrapper's `write(fd, bytes)`): a fixed array only in
                        // the CALLER's scope, no descriptor, no subslice. Kept
                        // last so a live descriptor route (copy's `&mut buffer`
                        // param) always wins with its proven address.
                        operands.insert_many([
                            operand(result),
                            operand(fd),
                            operand(address),
                            operand(InstructionOperandKind::ByteLength(length)),
                        ])
                    } else {
                        HandleSpan::empty()
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
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let buffer =
                address_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let count =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
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
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let buffer =
                address_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let count =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            let offset =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 4);
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
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let offset =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
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
        (
            HostCapability::Filesystem,
            HostOperation::FStat | HostOperation::SetFileTimes | HostOperation::FindNext,
        ) => {
            // `rc = read_file_metadata(fd, buf) -> _fstat(fd, buf)` and
            // `rc = set_file_times(fd, times) -> _futimens(fd, times)`: both are
            // `[result, fd scalar, buffer pointer]` (fstat's kernel WRITES the stat
            // record through the buffer; futimens READS two timespecs from it).
            // Same as `read` without the count -- keyed by an open descriptor.
            // `rc = find_next(handle, data) -> FindNextFileA(handle, &data)` (fs
            // rung 3a) is the same shape with the find HANDLE as the scalar.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let buffer =
                address_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
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
            let path = path_pointer_operand(input, host_call, dispatch_index, alias_context, 1);
            let second =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            match (result, path, second) {
                (Some(result), Some(path), Some(second)) => match host_call.data {
                    PlatformCallData::ConstantArgument { value } => operands.insert_many([
                        operand(result),
                        operand(InstructionOperandKind::ImmediateInteger(value)),
                        operand(path),
                        operand(second),
                    ]),
                    _ => operands.insert_many([operand(result), operand(path), operand(second)]),
                },
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::OpenAt | HostOperation::UnlinkAt) => {
            // Value-returning dirfd-relative `*at` ops: `fd = open_at(dirfd, name,
            // flags) -> _openat(dirfd, name, flags)` and `rc = unlink_at(dirfd,
            // name, flags) -> _unlinkat(dirfd, name, flags)`. Shape: [result, dirfd
            // SCALAR, name POINTER (NUL-terminated), flags SCALAR] -> C args
            // (dirfd, name, flags). A LITERAL name is NUL-terminated in rodata; a
            // runtime SUBSLICE name is not (that is the pending NUL-termination
            // seam, so native `remove_dir_all` awaits it -- the interpreter runs it).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let dirfd =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let name = path_pointer_operand(input, host_call, dispatch_index, alias_context, 2);
            let flags =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            match (result, dirfd, name, flags) {
                (Some(result), Some(dirfd), Some(name), Some(flags)) => operands.insert_many([
                    operand(result),
                    operand(dirfd),
                    operand(name),
                    operand(flags),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::ReadDir) => {
            // Value-returning `n = read_dir(fd, buf, count, position) ->
            // ___getdirentries64(fd, buf, count, &position)`. operand[0]=result,
            // [1]=fd, [2]=buffer POINTER (kernel writes dirent records), [3]=count
            // (buffer capacity), [4]=position POINTER (in/out i64 cursor).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let buffer =
                address_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let count =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            let position =
                address_argument_operand_at(input, host_call, dispatch_index, alias_context, 4);
            match (result, fd, buffer, count) {
                (Some(result), Some(fd), Some(buffer), Some(count))
                    if matches!(host_call.data, PlatformCallData::OmitTrailingArgument) =>
                {
                    operands.insert_many([
                        operand(result),
                        operand(fd),
                        operand(buffer),
                        operand(count),
                    ])
                }
                (Some(result), Some(fd), Some(buffer), Some(count)) => match position {
                    Some(position) => operands.insert_many([
                        operand(result),
                        operand(fd),
                        operand(buffer),
                        operand(count),
                        operand(position),
                    ]),
                    None => HandleSpan::empty(),
                },
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::ReadLink) => {
            // Value-returning `n = read_link(path, buf, count) -> _readlink(path,
            // buf, count)`. operand[0]=result, [1]=path POINTER (NUL-terminated),
            // [2]=buffer POINTER (kernel writes the target there), [3]=count.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let path = path_pointer_operand(input, host_call, dispatch_index, alias_context, 1);
            let buffer =
                address_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let count =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            match (result, path, buffer, count) {
                (Some(result), Some(path), Some(buffer), Some(count)) => match host_call.data {
                    PlatformCallData::ConstantArgument { value } => operands.insert_many([
                        operand(result),
                        operand(InstructionOperandKind::ImmediateInteger(value)),
                        operand(path),
                        operand(buffer),
                        operand(count),
                    ]),
                    _ => operands.insert_many([
                        operand(result),
                        operand(path),
                        operand(buffer),
                        operand(count),
                    ]),
                },
                _ => HandleSpan::empty(),
            }
        }
        (
            HostCapability::Filesystem,
            HostOperation::Stat
            | HostOperation::LStat
            | HostOperation::Realpath
            | HostOperation::FindFirst,
        ) => {
            // Value-returning `rc = read_metadata(path, buf) -> _stat(path, buf)`,
            // `rc = read_symlink_metadata(path, buf) -> _lstat(path, buf)`, and
            // `ptr = canonicalize(path, buf) -> _realpath(path, buf)` -- all share
            // the [result, path pointer, buffer pointer] shape (realpath's result
            // is the resolved-buffer pointer, used only as a non-NULL success flag).
            // `handle = find_first(pattern, data) -> FindFirstFileA(pattern, &data)`
            // (fs rung 3a) is the same shape: pattern pointer + the 320-byte
            // WIN32_FIND_DATAA buffer the system writes through; the result is
            // the find HANDLE (-1 = INVALID_HANDLE_VALUE).
            // operand[0]=result, [1]=path POINTER (NUL-terminated C string),
            // [2]=buffer POINTER (the kernel writes the 144-byte stat record).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let path = path_pointer_operand(input, host_call, dispatch_index, alias_context, 1);
            let buffer =
                address_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            match (result, path, buffer) {
                (Some(result), Some(path), Some(buffer)) => match host_call.data {
                    PlatformCallData::ConstantArguments { leading, trailing } => operands
                        .insert_many([
                            operand(result),
                            operand(InstructionOperandKind::ImmediateInteger(leading)),
                            operand(path),
                            operand(buffer),
                            operand(InstructionOperandKind::ImmediateInteger(trailing)),
                        ]),
                    _ => operands.insert_many([operand(result), operand(path), operand(buffer)]),
                },
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::Unlink | HostOperation::RemoveDir) => {
            // Value-returning `rc = unlink(path) / rmdir(path)`.
            // operand[0]=result, [1]=path POINTER (NUL-terminated C string).
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let path = path_pointer_operand(input, host_call, dispatch_index, alias_context, 1);
            match (result, path) {
                (Some(result), Some(path)) => match host_call.data {
                    PlatformCallData::ConstantArguments { leading, trailing } => operands
                        .insert_many([
                            operand(result),
                            operand(InstructionOperandKind::ImmediateInteger(leading)),
                            operand(path),
                            operand(InstructionOperandKind::ImmediateInteger(trailing)),
                        ]),
                    _ => operands.insert_many([operand(result), operand(path)]),
                },
                _ => HandleSpan::empty(),
            }
        }
        (
            HostCapability::Filesystem,
            HostOperation::SetLen | HostOperation::Fchmod | HostOperation::Flock,
        ) => {
            // Value-returning `rc = set_len(fd, length) -> _ftruncate(fd, length)`,
            // `rc = set_file_permissions(fd, mode) -> _fchmod(fd, mode)`, and
            // `rc = lock_file(fd, operation) -> _flock(fd, operation)` -- all the
            // same fd + one-scalar shape. operand[0]=result, then the two scalars.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let length =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            match (result, fd, length) {
                (Some(result), Some(fd), Some(length)) => {
                    operands.insert_many([operand(result), operand(fd), operand(length)])
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::Seek | HostOperation::Fchown) => {
            // Value-returning `pos = seek(fd, offset, whence) -> _lseek(fd,
            // offset, whence)` and `rc = change_file_owner(fd, uid, gid) ->
            // _fchown(fd, uid, gid)` -- both fd + two scalars. operand[0]=result,
            // then the three scalar args.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let fd = scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let offset =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let whence =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
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
            HostOperation::Chown | HostOperation::LChown | HostOperation::OpenCreate,
        ) => {
            // Value-returning `rc = change_owner(path, uid, gid) -> _chown(path,
            // uid, gid)` (and `_lchown`), plus `fd = open_create(path, flags, mode)
            // -> _open(path, flags, mode)` -- ALL `[result, path POINTER, scalar,
            // scalar]`. `open_create` differs in its concrete adapter subcall:
            // the normalized Darwin variadic plan marshals trailing `mode` on
            // the stack. operand[0]=result, [1]=path,
            // [2]=uid/flags, [3]=gid/mode.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let path = path_pointer_operand(input, host_call, dispatch_index, alias_context, 1);
            let uid =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let gid =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            match (result, path, uid, gid) {
                (Some(result), Some(path), Some(uid), Some(gid)) => match host_call.data {
                    PlatformCallData::ConstantArgument { value } => operands.insert_many([
                        operand(result),
                        operand(InstructionOperandKind::ImmediateInteger(value)),
                        operand(path),
                        operand(uid),
                        operand(gid),
                    ]),
                    _ => operands.insert_many([
                        operand(result),
                        operand(path),
                        operand(uid),
                        operand(gid),
                    ]),
                },
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
            // Each path resolves PER ARGUMENT through the alias chain to its
            // literal data object (`fs.rename(a, b)` forwards the wrapper's
            // params, whose literals live at the CALLER's statement -- the old
            // creation-order scan only saw THIS statement's literals, so the
            // wrapper form had no encodable sequence; per-argument resolution
            // also removes the swap hazard when only one side is direct).
            // Creation-order remains the fallback for both-direct forms.
            // (Runtime/computed path forms are still a future extension.)
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let from = aliased_literal_data_object(input, host_call, alias_context, 1)
                .map(|(handle, _)| handle)
                .unwrap_or_else(|| find_nth_data_object(input, host_call, 0));
            let to = aliased_literal_data_object(input, host_call, alias_context, 2)
                .map(|(handle, _)| handle)
                .unwrap_or_else(|| find_nth_data_object(input, host_call, 1));
            match result {
                Some(result) if from.is_valid() && to.is_valid() => {
                    let from = operand(InstructionOperandKind::DataAddress { data: from });
                    let to = operand(InstructionOperandKind::DataAddress { data: to });
                    match host_call.data {
                        PlatformCallData::DirectoryRelativePathPair {
                            first_dirfd: Some(first_dirfd),
                            second_dirfd,
                            trailing_flags: Some(trailing_flags),
                        } => operands.insert_many([
                            operand(result),
                            operand(InstructionOperandKind::ImmediateInteger(first_dirfd)),
                            from,
                            operand(InstructionOperandKind::ImmediateInteger(second_dirfd)),
                            to,
                            operand(InstructionOperandKind::ImmediateInteger(trailing_flags)),
                        ]),
                        PlatformCallData::DirectoryRelativePathPair {
                            first_dirfd: Some(first_dirfd),
                            second_dirfd,
                            trailing_flags: None,
                        } => operands.insert_many([
                            operand(result),
                            operand(InstructionOperandKind::ImmediateInteger(first_dirfd)),
                            from,
                            operand(InstructionOperandKind::ImmediateInteger(second_dirfd)),
                            to,
                        ]),
                        PlatformCallData::DirectoryRelativePathPair {
                            first_dirfd: None,
                            second_dirfd,
                            trailing_flags: Some(trailing_flags),
                        } => operands.insert_many([
                            operand(result),
                            from,
                            operand(InstructionOperandKind::ImmediateInteger(second_dirfd)),
                            to,
                            operand(InstructionOperandKind::ImmediateInteger(trailing_flags)),
                        ]),
                        PlatformCallData::DirectoryRelativePathPair {
                            first_dirfd: None,
                            second_dirfd,
                            trailing_flags: None,
                        } => operands.insert_many([
                            operand(result),
                            from,
                            operand(InstructionOperandKind::ImmediateInteger(second_dirfd)),
                            to,
                        ]),
                        _ => operands.insert_many([operand(result), from, to]),
                    }
                }
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::FinalPathNameByHandle) => {
            // `len = final_path_name_by_handle(handle, buffer, capacity, flags)
            // -> GetFinalPathNameByHandleA(handle, &buffer, capacity, flags)`
            // (session slice 4a): the FStat [result, scalar, buffer] shape plus
            // the two trailing scalars. operand[0]=result, [1]=the HANDLE,
            // [2]=buffer POINTER (the system writes the NUL-terminated path
            // through it), [3]=capacity, [4]=flags.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let handle =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let buffer =
                address_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let capacity =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            let flags =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 4);
            match (result, handle, buffer, capacity, flags) {
                (Some(result), Some(handle), Some(buffer), Some(capacity), Some(flags)) => operands
                    .insert_many([
                        operand(result),
                        operand(handle),
                        operand(buffer),
                        operand(capacity),
                        operand(flags),
                    ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::SetFileTime) => {
            // `rc = set_file_time(handle, creation, access_ft, write_ft) ->
            // SetFileTime(handle, NULL, &access, &write)` (session slice 4b):
            // operand[0]=result, [1]=the HANDLE, [2]=the NULL-able creation
            // scalar (0), [3]/[4]=the two 8-byte FILETIME buffer POINTERS the
            // API reads through.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let handle =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let creation =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let access =
                address_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            let write =
                address_argument_operand_at(input, host_call, dispatch_index, alias_context, 4);
            match (result, handle, creation, access, write) {
                (Some(result), Some(handle), Some(creation), Some(access), Some(write)) => operands
                    .insert_many([
                        operand(result),
                        operand(handle),
                        operand(creation),
                        operand(access),
                        operand(write),
                    ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::CreateFile) => {
            // `handle = open_path_handle(path, access, share, security,
            // disposition, flags, template) -> CreateFileA(...)`. The path is
            // a NUL-terminated pointer; the remaining six arguments are exact
            // Win32 scalars. Win64 marshals arguments beyond the fourth in the
            // outgoing stack area through the ordinary import-call encoder.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let path = path_pointer_operand(input, host_call, dispatch_index, alias_context, 1);
            let access =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let share =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            let security =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 4);
            let disposition =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 5);
            let flags =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 6);
            let template =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 7);
            match (
                result,
                path,
                access,
                share,
                security,
                disposition,
                flags,
                template,
            ) {
                (
                    Some(result),
                    Some(path),
                    Some(access),
                    Some(share),
                    Some(security),
                    Some(disposition),
                    Some(flags),
                    Some(template),
                ) => operands.insert_many([
                    operand(result),
                    operand(path),
                    operand(access),
                    operand(share),
                    operand(security),
                    operand(disposition),
                    operand(flags),
                    operand(template),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::LockFileEx) => {
            // `rc = lock_file_ex(handle, flags, reserved, low, high, overlapped)`
            // mirrors LockFileEx exactly. The last two scalar arguments ride
            // Win64's outgoing stack area; the pointer is the sixth argument.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let handle =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let flags =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let reserved =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            let low =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 4);
            let high =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 5);
            let overlapped =
                address_argument_operand_at(input, host_call, dispatch_index, alias_context, 6);
            match (result, handle, flags, reserved, low, high, overlapped) {
                (
                    Some(result),
                    Some(handle),
                    Some(flags),
                    Some(reserved),
                    Some(low),
                    Some(high),
                    Some(overlapped),
                ) => operands.insert_many([
                    operand(result),
                    operand(handle),
                    operand(flags),
                    operand(reserved),
                    operand(low),
                    operand(high),
                    operand(overlapped),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::UnlockFile) => {
            // `rc = unlock_file(handle, offset_low, offset_high, length_low,
            // length_high) -> UnlockFile(...)` -- five scalar call arguments.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let handle =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 1);
            let offset_low =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 2);
            let offset_high =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            let length_low =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 4);
            let length_high =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 5);
            match (
                result,
                handle,
                offset_low,
                offset_high,
                length_low,
                length_high,
            ) {
                (
                    Some(result),
                    Some(handle),
                    Some(offset_low),
                    Some(offset_high),
                    Some(length_low),
                    Some(length_high),
                ) => operands.insert_many([
                    operand(result),
                    operand(handle),
                    operand(offset_low),
                    operand(offset_high),
                    operand(length_low),
                    operand(length_high),
                ]),
                _ => HandleSpan::empty(),
            }
        }
        (HostCapability::Filesystem, HostOperation::CreateHardLink) => {
            // `rc = create_hard_link(link, existing, 0) -> CreateHardLinkA(link,
            // existing, NULL)` (windows session slice 3): the two-path shape of
            // Rename/Link above PLUS the trailing security-attributes scalar the
            // API requires as NULL. operand[0]=result, [1]=link path POINTER,
            // [2]=existing path POINTER, [3]=the 0 scalar. Paths resolve per
            // argument through the alias chain like the Rename arm.
            let result = first_scalar_argument_operand(input, host_call, dispatch_index);
            let link = aliased_literal_data_object(input, host_call, alias_context, 1)
                .map(|(handle, _)| handle)
                .unwrap_or_else(|| find_nth_data_object(input, host_call, 0));
            let existing = aliased_literal_data_object(input, host_call, alias_context, 2)
                .map(|(handle, _)| handle)
                .unwrap_or_else(|| find_nth_data_object(input, host_call, 1));
            let security =
                scalar_argument_operand_at(input, host_call, dispatch_index, alias_context, 3);
            match (result, security) {
                (Some(result), Some(security)) if link.is_valid() && existing.is_valid() => {
                    operands.insert_many([
                        operand(result),
                        operand(InstructionOperandKind::DataAddress { data: link }),
                        operand(InstructionOperandKind::DataAddress { data: existing }),
                        operand(security),
                    ])
                }
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
    // Gui host calls never forward a value-call's params (they are not reached
    // through the ergonomic wrapper shape), so no alias context is needed here.
    let scalar =
        |index: usize| scalar_argument_operand_at(input, host_call, dispatch_index, None, index);
    let address =
        |index: usize| address_argument_operand_at(input, host_call, dispatch_index, None, index);
    let imm = |value: i64| Some(InstructionOperandKind::ImmediateInteger(value));

    let kinds: Option<Vec<InstructionOperandKind>> = match operation {
        // dc_create() -> CreateCompatibleDC(NULL): [result].
        HostOperation::DcCreate if arity == 1 => [scalar(0), imm(0)].into_iter().collect(),
        // foreground_window() -> GetForegroundWindow(): [result], no args.
        HostOperation::ForegroundWindow if arity == 1 => [scalar(0)].into_iter().collect(),
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
            imm(0),    // hWndParent
            imm(0),    // hMenu
            imm(0),    // hInstance (NULL works for the system STATIC class)
            imm(0),    // lpParam
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
/// The expression of the host-call argument at `index`, if it is an expression
/// (not a synthesized literal). Used to inspect an argument's TYPE (e.g. whether a
/// `write` payload is a fixed `[u8; N]` array vs a `&[u8]` slice).
fn host_call_argument_expression(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    index: usize,
) -> Option<psi_checked_trees::expression::ExpressionHandle> {
    let argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.get(index))?;
    match &argument.kind {
        HostCallArgumentKind::Expression(expression) => Some(*expression),
        _ => None,
    }
}

fn address_argument_operand_at(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
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
    // ALIAS REWRITE FIRST: a value-call-forwarded `&mut` buffer param is
    // BY-REF -- when an alias binding rewrites it to the caller's place, that
    // place is the semantic truth. The direct callee-scope resolution below
    // falls through to MACHINE-OWNED NAME matching, which silently captured a
    // CALLER FIELD that happened to share the param's name (`buffer` param vs
    // a caller `buffer: [u8; 64]` field -- the wrapper_read_buffer_decoy
    // repro: the read filled the caller's same-named field, the spelled
    // buffer stayed ZII). `alias_resolved_place_at` returns None unless an
    // alias actually rewrote the expression, so non-forwarded arguments keep
    // the direct path unchanged.
    alias_resolved_place_at(input, host_call, dispatch_index, alias_context, index)
        .or_else(|| {
            resolve_runtime_storage_place_in_table(
                input,
                dispatch_index.unwrap_or(0),
                host_call.source_key,
                &input.host_calls.expressions,
                *expression,
            )
        })
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

/// Marshal a RUNTIME-length subslice `collection[0..end]` write payload to its
/// {POINTER, LENGTH} operands: the pointer is the collection's raw base ADDRESS
/// (a fixed array, exactly as `read` marshals its buffer), the length is the
/// range END loaded as a runtime scalar -- i.e. `_write(fd, &buf[0], end)`.
/// Restricted to a literal-`0` start (the base needs no offset) and an EXCLUSIVE
/// end, matching the shape the checker's runtime-subslice bounds proof admits
/// (`known_length_range_via_index_bounds_is_proven`). Constant-bound subslices
/// keep flowing through the existing literal descriptor paths; this adds the
/// RUNTIME end (`buffer[0..n]`, `n` a proven runtime value -- the faithful copy).
fn subslice_argument_operands(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    index: usize,
) -> Option<(InstructionOperandKind, InstructionOperandKind)> {
    let expression = host_call_argument_expression(input, host_call, index)?;
    let ExpressionNode::Indexed(indexed) = input.host_calls.expressions.expression(expression)
    else {
        return None;
    };
    let ExpressionNode::Range(range) = input.host_calls.expressions.expression(indexed.index)
    else {
        return None;
    };
    // Start must be the literal 0 (base address is the collection base, no offset)
    // and the end an exclusive runtime bound -- the shape the checker proves.
    let start_is_zero = if range.start.is_valid() {
        matches!(
            input.host_calls.expressions.expression(range.start),
            ExpressionNode::Integer(value) if value.value_i64() == Some(0)
        )
    } else {
        true
    };
    if !start_is_zero || !range.end.is_valid() || range.end_inclusive {
        return None;
    }
    // Pointer: the collection's raw base address (a fixed array `[u8; N]`).
    let base = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        indexed.collection,
    )?;
    // Length: the range end loaded as a runtime scalar value.
    let length = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        range.end,
    )
    .filter(|place| matches!(place.byte_count, 1 | 2 | 4 | 8))?;
    Some((
        InstructionOperandKind::RuntimeStorageAddress {
            region: base.region,
            byte_offset: base.byte_offset,
        },
        InstructionOperandKind::RuntimeScalarInteger {
            region: length.region,
            byte_offset: length.byte_offset,
            byte_count: length.byte_count,
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
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
    index: usize,
) -> Option<InstructionOperandKind> {
    let data = find_data_object(input, host_call);
    if data.is_valid() {
        return Some(InstructionOperandKind::DataAddress { data });
    }
    if let Some(address) = subslice_path_pointer(input, host_call, dispatch_index, index) {
        return Some(address);
    }
    // A path LITERAL forwarded through a value-call param (`fs.open(path)` -> the
    // wrapper's `host.open(path, ..)`) arrives as the callee's `path` param ALIASED
    // to the caller's literal, whose NUL-terminated data object is keyed to the
    // CALLER's statement. Resolve it the same way the write byte payload does
    // (symmetric to fix #1) -- its base pointer is a valid C string.
    if let Some((data, _)) = aliased_literal_data_object(input, host_call, alias_context, index) {
        return Some(InstructionOperandKind::DataAddress { data });
    }
    slice_argument_operands(input, host_call, dispatch_index, index).map(|(pointer, _)| pointer)
}

/// A LITERAL-start subslice of a fixed-array BUFFER (`namebuf[start..end]`, e.g. a
/// runtime-built directory-entry name) used as a C-string path/name arg: the
/// native `char*` pointer is simply the buffer's base + `start * element_size`.
/// The C call reads until the NUL the Omega code writes into the buffer, so ONLY
/// the start pointer matters (the length is irrelevant) -- no scratch copy, no
/// descriptor deref. This is what lets a RUNTIME name (not a rodata literal) flow
/// into `open_at`/`unlink_at` natively. Declines anything that is not a
/// literal-start subslice of an actual fixed array (so a subslice of a LITERAL
/// still needs the pending NUL-termination scratch seam).
fn subslice_path_pointer(
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
    let ExpressionNode::Indexed(subslice) = input.host_calls.expressions.expression(*expression)
    else {
        return None;
    };
    let ExpressionNode::Range(range) = input.host_calls.expressions.expression(subslice.index)
    else {
        return None;
    };
    let start = if range.start.is_valid() {
        let ExpressionNode::Integer(start) = input.host_calls.expressions.expression(range.start)
        else {
            return None;
        };
        usize::try_from(start.value_i64()?).ok()?
    } else {
        0
    };
    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        subslice.collection,
    )?;
    if std::env::var_os("OMEGA_DEBUG_SUBSLICE").is_some() {
        eprintln!(
            "subslice ptr: state m{} s{} seg{} dispatch {:?} -> region {:?} offset {} count {}",
            host_call.source_key.machine.arena_index(),
            host_call.source_key.state.arena_index(),
            host_call.source_key.segment_index,
            dispatch_index,
            place.region,
            place.byte_offset,
            place.byte_count,
        );
    }
    let length = resolve_fixed_array_length_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        subslice.collection,
    )?;
    if length == 0 || place.byte_count % length != 0 {
        return None;
    }
    let element_byte_size = place.byte_count / length;
    let byte_offset = place
        .byte_offset
        .checked_add(start.checked_mul(element_byte_size)?)?;
    Some(InstructionOperandKind::RuntimeStorageAddress {
        region: place.region,
        byte_offset,
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

/// A string LITERAL forwarded through a value-call param (`fs.write_all(path,
/// "hi")` -> the wrapper's `write(fd, bytes)`, or `open(path)` -> `host.open(path)`)
/// reaches the callee host-call as its param NAME (`bytes`/`path`) ALIASED to the
/// caller's literal. `find_data_object` keys on THIS host-call's own statement, so
/// it misses the literal's data object (keyed to the CALLER's statement). This
/// resolves the arg at `index` through the alias chain; if it lands on a string
/// literal, it returns that literal's data object (matched by resolved source
/// state + byte content) and length. Returns None when there is no alias context,
/// the arg is not an aliased literal, or no matching data object exists — so a
/// non-forwarded call is unaffected (it takes the `find_data_object` path).
fn aliased_literal_data_object(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
    index: usize,
) -> Option<(AbstractDataObjectHandle, usize)> {
    use crate::selection::bindings::{RuntimeAliasBuffer, resolve_runtime_alias_binding_handle};
    let alias_context = alias_context?;
    let argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.get(index))?;
    let HostCallArgumentKind::Expression(expression) = &argument.kind else {
        return None;
    };
    let mut expressions =
        ExpressionTable::with_expression_capacity(alias_context.aliases.len().saturating_add(4));
    let copied_aliases = RuntimeAliasBuffer::copy_from_bindings(
        alias_context.alias_expressions,
        alias_context.aliases,
        &mut expressions,
    );
    let expression_handle = expressions.copy_from(&input.host_calls.expressions, *expression);
    let resolved = resolve_runtime_alias_binding_handle(
        expression_handle,
        host_call.source_key,
        copied_aliases.bindings(),
        &mut expressions,
    );
    let value = expressions.string_literal_value(resolved.expression)?;
    let bytes = value.as_ref();
    let object_bytes_match = |object: &AbstractDataObject| -> bool {
        input
            .data
            .bytes
            .span(object.bytes)
            .is_some_and(|object_bytes| object_bytes == bytes)
    };
    // Prefer the object keyed to the resolved binding's state -- this is the
    // CONTAINED-receiver value call, whose alias binding resolves the param to
    // the CALLER's key, matching where the static-string collector keyed the
    // literal's data object.
    if let Some((handle, _)) =
        input.data.objects.iter().find(|(_, object)| {
            object.source_key == resolved.source_key && object_bytes_match(object)
        })
    {
        return Some((handle, bytes.len()));
    }
    // Fallback for a SELF value call (`self.doit("lit")` -> `self.raw.open(path)`):
    // the alias binding resolves the param to the CALLEE's key, but the literal's
    // data object is keyed to the CALLER's statement, so the state-keyed lookup
    // above misses. Match by BYTES alone -- every data object with identical bytes
    // is an identical read-only C string, so pointing the arg at any of them is
    // correctness-equivalent (at worst a missed dedup, never a wrong pointer).
    input
        .data
        .objects
        .iter()
        .find(|(_, object)| object_bytes_match(object))
        .map(|(handle, _)| (handle, bytes.len()))
}

/// Resolve the host-call argument at `index` to its runtime storage PLACE, first
/// directly and then -- when it is a value-call-forwarded PARAM aliased to the
/// caller's argument (`self.fs.read_all(.., count)` -> the wrapper's
/// `self.host.read(fd, buffer, count)`, where `count`/`buffer` alias the caller's
/// `self.cap`/`&mut self.buffer`) -- through the alias chain. The scalar/address
/// analog of `aliased_literal_data_object` (fix #1, which does the same for a
/// forwarded LITERAL). Returns None with no alias context (a non-forwarded call is
/// unaffected; it took the direct path already).
fn alias_resolved_place_at(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
    index: usize,
) -> Option<crate::selection::storage_places::RuntimeStoragePlace> {
    use crate::selection::bindings::{RuntimeAliasBuffer, resolve_runtime_alias_binding_handle};
    let alias_context = alias_context?;
    let argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.get(index))?;
    let HostCallArgumentKind::Expression(expression) = &argument.kind else {
        return None;
    };
    let mut expressions =
        ExpressionTable::with_expression_capacity(alias_context.aliases.len().saturating_add(4));
    let copied_aliases = RuntimeAliasBuffer::copy_from_bindings(
        alias_context.alias_expressions,
        alias_context.aliases,
        &mut expressions,
    );
    let expression_handle = expressions.copy_from(&input.host_calls.expressions, *expression);
    let resolved = resolve_runtime_alias_binding_handle(
        expression_handle,
        host_call.source_key,
        copied_aliases.bindings(),
        &mut expressions,
    );
    // Only follow an alias that actually rewrote the expression to the caller's
    // place (a different source key OR a different expression node); otherwise the
    // direct resolution already covered it.
    if resolved.source_key == host_call.source_key && resolved.expression == expression_handle {
        return None;
    }
    resolve_runtime_storage_place_in_table(
        input,
        dispatch_index.unwrap_or(0),
        resolved.source_key,
        &expressions,
        resolved.expression,
    )
}

/// Like `alias_resolved_place_at`, but resolving the argument to a FIXED-ARRAY
/// LENGTH in the caller's scope: `fs.write_all(path, self.bin_src)` binds the
/// wrapper's `bytes` param to the caller's `[u8; N]` field, which is a fixed
/// array only under the RESOLVED source key -- the callee-scope probe returns
/// None for the bare param name.
fn alias_resolved_fixed_array_length_at(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
    index: usize,
) -> Option<usize> {
    use crate::selection::bindings::{RuntimeAliasBuffer, resolve_runtime_alias_binding_handle};
    let alias_context = alias_context?;
    let argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.get(index))?;
    let HostCallArgumentKind::Expression(expression) = &argument.kind else {
        return None;
    };
    let mut expressions =
        ExpressionTable::with_expression_capacity(alias_context.aliases.len().saturating_add(4));
    let copied_aliases = RuntimeAliasBuffer::copy_from_bindings(
        alias_context.alias_expressions,
        alias_context.aliases,
        &mut expressions,
    );
    let expression_handle = expressions.copy_from(&input.host_calls.expressions, *expression);
    let resolved = resolve_runtime_alias_binding_handle(
        expression_handle,
        host_call.source_key,
        copied_aliases.bindings(),
        &mut expressions,
    );
    // Only follow an alias that actually rewrote the expression (same contract
    // as `alias_resolved_place_at`) -- the direct probe covered the rest.
    if resolved.source_key == host_call.source_key && resolved.expression == expression_handle {
        return None;
    }
    resolve_fixed_array_length_in_table(
        input,
        dispatch_index.unwrap_or(0),
        resolved.source_key,
        &expressions,
        resolved.expression,
    )
}

/// Like `alias_resolved_place_at`, but for an alias that rewrites the argument
/// to the CALLER'S INTEGER LITERAL: the wrapper shape `fs.read(file, &mut buf,
/// 32)` binds the callee's `count` param to `32`, which has no storage place --
/// the scalar operand is the literal itself.
fn alias_resolved_integer_at(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
    index: usize,
) -> Option<i64> {
    use crate::selection::bindings::{RuntimeAliasBuffer, resolve_runtime_alias_binding_handle};
    let alias_context = alias_context?;
    let argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.get(index))?;
    let HostCallArgumentKind::Expression(expression) = &argument.kind else {
        return None;
    };
    let mut expressions =
        ExpressionTable::with_expression_capacity(alias_context.aliases.len().saturating_add(4));
    let copied_aliases = RuntimeAliasBuffer::copy_from_bindings(
        alias_context.alias_expressions,
        alias_context.aliases,
        &mut expressions,
    );
    let expression_handle = expressions.copy_from(&input.host_calls.expressions, *expression);
    let resolved = resolve_runtime_alias_binding_handle(
        expression_handle,
        host_call.source_key,
        copied_aliases.bindings(),
        &mut expressions,
    );
    if resolved.source_key == host_call.source_key && resolved.expression == expression_handle {
        return None;
    }
    match expressions.expression(resolved.expression) {
        psi_checked_trees::expression::ExpressionNode::Integer(literal) => literal.value_i64(),
        _ => None,
    }
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
        HostCallArgumentKind::Expression(expression) => {
            computed_scalar_argument_operand(input, host_call, dispatch_index, 0)
                .or_else(|| {
                    resolve_runtime_storage_place_in_table(
                        input,
                        dispatch_index.unwrap_or(0),
                        host_call.source_key,
                        &input.host_calls.expressions,
                        *expression,
                    )
                    .filter(|place| matches!(place.byte_count, 1 | 2 | 4 | 8))
                    .map(|place| {
                        InstructionOperandKind::RuntimeScalarInteger {
                            region: place.region,
                            byte_offset: place.byte_offset,
                            byte_count: place.byte_count,
                        }
                    })
                })
                .or_else(|| {
                    machine_value_call_argument_result_place(input, host_call, dispatch_index, 0)
                        .map(|place| InstructionOperandKind::RuntimeScalarInteger {
                            region: place.region,
                            byte_offset: place.byte_offset,
                            byte_count: place.byte_count,
                        })
                })
        }
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
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
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
        // ALIAS REWRITE FIRST (same ordering + rationale as
        // `address_argument_operand_at`): a forwarded param's alias binding is
        // the semantic truth; the direct callee-scope resolution falls through
        // to machine-owned NAME matching, which silently captured a caller
        // field sharing the param's name (a caller `count` field shadowing the
        // wrapper's `count` param made the read request the field's ZII 0
        // bytes). `alias_resolved_place_at` is None unless an alias actually
        // rewrote the expression, so non-forwarded arguments are unchanged.
        HostCallArgumentKind::Expression(expression) => {
            let scalar_place = |place: crate::selection::storage_places::RuntimeStoragePlace| {
                matches!(place.byte_count, 1 | 2 | 4 | 8).then_some(
                    InstructionOperandKind::RuntimeScalarInteger {
                        region: place.region,
                        byte_offset: place.byte_offset,
                        byte_count: place.byte_count,
                    },
                )
            };
            // The alias rewrite may land on the caller's PLACE or the caller's
            // INTEGER LITERAL (`fs.read(file, &mut buf, 32)` binds `count` to
            // `32`, which has no storage place -- it marshals as an immediate).
            // BOTH precede the direct fallback: the direct path's machine-owned
            // NAME matching silently captured a caller field sharing the
            // param's name (a caller `count` field shadowing the wrapper's
            // `count` param made the read request the field's ZII 0 bytes).
            // Both alias probes are None unless an alias actually rewrote the
            // expression, so non-forwarded arguments are unchanged.
            alias_resolved_place_at(input, host_call, dispatch_index, alias_context, index)
                .and_then(scalar_place)
                .or_else(|| {
                    alias_resolved_integer_at(input, host_call, alias_context, index)
                        .map(InstructionOperandKind::ImmediateInteger)
                })
                .or_else(|| {
                    computed_scalar_argument_operand(input, host_call, dispatch_index, index)
                })
                .or_else(|| {
                    resolve_runtime_storage_place_in_table(
                        input,
                        dispatch_index.unwrap_or(0),
                        host_call.source_key,
                        &input.host_calls.expressions,
                        *expression,
                    )
                    .and_then(scalar_place)
                })
                .or_else(|| {
                    machine_value_call_argument_result_place(
                        input,
                        host_call,
                        dispatch_index,
                        index,
                    )
                    .and_then(scalar_place)
                })
        }
        _ => None,
    }
}

fn computed_scalar_argument_operand(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    index: usize,
) -> Option<InstructionOperandKind> {
    if input.runtime_storage.host_argument_scratch_size < (index + 1) * 8 {
        return None;
    }
    let expression = host_call_argument_expression(input, host_call, index)?;
    let byte_count = crate::selection::runtime_dispatch::computed_host_argument_byte_size(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        expression,
    )?;
    let region = omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame;
    let byte_offset = input.runtime_storage.host_argument_scratch_base + index * 8;
    if crate::selection::runtime_dispatch::computed_host_argument_is_float(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        expression,
    ) {
        Some(InstructionOperandKind::RuntimeScalarFloat {
            region,
            byte_offset,
            byte_count,
        })
    } else {
        Some(InstructionOperandKind::RuntimeScalarInteger {
            region,
            byte_offset,
            byte_count,
        })
    }
}

/// Like `scalar_argument_operand_at`, but for a FLOAT (`f32`/`f64`) argument: emits
/// a `RuntimeScalarFloat` operand so the encoder marshals it into a v-register.
/// Only a place-backed float (a field/local holding the value) is supported — a
/// bare float literal has no storage slot, so pass it through a field. `byte_count`
/// must be 4 or 8.
fn float_argument_operand_at(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
    index: usize,
) -> Option<InstructionOperandKind> {
    let argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.get(index))?;
    match &argument.kind {
        HostCallArgumentKind::Expression(expression) => resolve_runtime_storage_place_in_table(
            input,
            dispatch_index.unwrap_or(0),
            host_call.source_key,
            &input.host_calls.expressions,
            *expression,
        )
        .or_else(|| alias_resolved_place_at(input, host_call, dispatch_index, alias_context, index))
        .filter(|place| matches!(place.byte_count, 4 | 8))
        .map(|place| InstructionOperandKind::RuntimeScalarFloat {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_count: place.byte_count,
        })
        .or_else(|| {
            machine_value_call_argument_result_place(input, host_call, dispatch_index, index)
                .filter(|place| matches!(place.byte_count, 4 | 8))
                .map(|place| InstructionOperandKind::RuntimeScalarFloat {
                    region: place.region,
                    byte_offset: place.byte_offset,
                    byte_count: place.byte_count,
                })
        })
        .or_else(|| {
            computed_scalar_argument_operand(input, host_call, dispatch_index, index).filter(
                |operand| matches!(operand, InstructionOperandKind::RuntimeScalarFloat { .. }),
            )
        }),
        _ => None,
    }
}

fn machine_value_call_argument_result_place(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    index: usize,
) -> Option<RuntimeStoragePlace> {
    let arguments = input.host_calls.arguments.span(host_call.arguments)?;
    let HostCallArgumentKind::Expression(expression) = arguments.get(index)?.kind else {
        return None;
    };
    let ExpressionNode::Call(call) = input.host_calls.expressions.expression(expression) else {
        return None;
    };

    let target_matches = |state_call: &omega_state_calls::StateCall| {
        let target_name = input
            .control_flow
            .state_by_key(state_call.target_key)
            .map(|state| state.name.as_str());
        state_call.target_key.state == call.target_symbol
            || target_name == Some(call.target.as_str())
    };
    let prior_same_target = arguments[..index]
        .iter()
        .filter(|argument| {
            let HostCallArgumentKind::Expression(expression) = argument.kind else {
                return false;
            };
            let ExpressionNode::Call(prior) = input.host_calls.expressions.expression(expression)
            else {
                return false;
            };
            prior.target_symbol == call.target_symbol && prior.target == call.target
        })
        .count();
    let state_call = input
        .state_calls
        .calls_for_statement(host_call.source_key, host_call.statement_index)
        .filter(|state_call| state_call.role == omega_state_calls::StateCallRole::CallArgument)
        .filter(|state_call| target_matches(state_call))
        .nth(prior_same_target)?;
    let slot = input.runtime_storage.call_result_slot_by_ordinal(
        dispatch_index.unwrap_or(0),
        state_call.source_key,
        state_call.statement_index,
        state_call.role,
        state_call.call_ordinal,
    )?;
    matches!(slot.byte_size, 1 | 2 | 4 | 8).then_some(RuntimeStoragePlace {
        region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
        byte_offset: slot.byte_offset,
        byte_count: slot.byte_size,
    })
}

/// Preserve scalar float identity for authored imports. Catalog operations
/// know their float slots from the operation key; authored calls instead need
/// to recover it from the selected expression's layout descriptor before the
/// generic scalar fallback erases the distinction.
fn authored_float_argument_operand_at(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
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
    let place = alias_resolved_place_at(input, host_call, dispatch_index, alias_context, index)
        .or_else(|| {
            resolve_runtime_storage_place_in_table(
                input,
                dispatch_index.unwrap_or(0),
                host_call.source_key,
                &input.host_calls.expressions,
                *expression,
            )
        })?;
    let descriptor = resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        *expression,
    )?;
    let byte_count = scalar_float_descriptor_byte_count(&descriptor)?;
    (place.byte_count == byte_count).then_some(InstructionOperandKind::RuntimeScalarFloat {
        region: place.region,
        byte_offset: place.byte_offset,
        byte_count,
    })
}

fn scalar_float_descriptor_byte_count(descriptor: &TypeLayoutDescriptor) -> Option<usize> {
    let descriptor = match descriptor {
        TypeLayoutDescriptor::Constrained { base_type, .. } => {
            return scalar_float_descriptor_byte_count(base_type);
        }
        descriptor => descriptor,
    };
    let TypeLayoutDescriptor::Named { name, .. } = descriptor else {
        return None;
    };
    match PrimitiveType::from_name(name.as_ref())? {
        PrimitiveType::F32 => Some(4),
        PrimitiveType::F64 => Some(8),
        _ => None,
    }
}

/// Preserve one supported flat by-value HFA as one selected operand. AAPCS64
/// accepts its one-to-four-member family; SysV accepts flat f32/f64 records
/// totaling at most two eightbytes and groups their bytes into SSE fragments.
fn native_hfa_argument_operand_at(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
    index: usize,
) -> Option<InstructionOperandKind> {
    let policy = CallingPolicy::native_for_target(input.target);
    if !matches!(policy, CallingPolicy::Aapcs64 | CallingPolicy::SystemVAMD64) {
        return None;
    }
    let argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.get(index))?;
    let HostCallArgumentKind::Expression(expression) = &argument.kind else {
        return None;
    };
    let place = alias_resolved_place_at(input, host_call, dispatch_index, alias_context, index)
        .or_else(|| {
            resolve_runtime_storage_place_in_table(
                input,
                dispatch_index.unwrap_or(0),
                host_call.source_key,
                &input.host_calls.expressions,
                *expression,
            )
        })?;
    let descriptor = resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        *expression,
    )?;
    let (member_byte_count, members) = hfa_descriptor_shape(input, &descriptor)?;
    // SysV has no distinct one-member HFA ABI class: a record that recursively
    // contains one f32/f64 occupies one SSE eightbyte and follows the ordinary
    // classified-record path below.  Preserving it as
    // `RuntimeHomogeneousFloatAggregate { members: 1 }` makes selection produce
    // a shape the SysV plan deliberately rejects (its HFA carrier represents
    // only the multi-member fragmented case), even though the record is a
    // perfectly legal native aggregate.  AAPCS64 does classify one-to-four
    // member HFAs, so keep the one-member carrier there.
    if policy == CallingPolicy::SystemVAMD64 && members == 1 {
        return None;
    }
    if policy == CallingPolicy::SystemVAMD64 && member_byte_count * usize::from(members) > 16 {
        return None;
    }
    (place.byte_count == member_byte_count * usize::from(members)).then_some(
        InstructionOperandKind::RuntimeHomogeneousFloatAggregate {
            region: place.region,
            byte_offset: place.byte_offset,
            member_byte_count,
            members,
        },
    )
}

fn hfa_descriptor_shape(
    input: &InstructionSelectionInput<'_>,
    descriptor: &TypeLayoutDescriptor,
) -> Option<(usize, u8)> {
    if !aggregate_descriptor_is_public_fixed_shape(input, descriptor) {
        return None;
    }
    let layout = boundary_descriptor_layout(input, descriptor, 0)?;
    let mut leaves = Vec::new();
    collect_homogeneous_float_leaves(input, descriptor, layout, 0, &mut leaves, 0)?;
    let (first_offset, member_byte_count) = *leaves.first()?;
    let members = u8::try_from(leaves.len()).ok()?;
    if first_offset != 0 || !(1..=4).contains(&members) {
        return None;
    }
    let contiguous = leaves.iter().enumerate().all(|(index, (offset, size))| {
        *size == member_byte_count && *offset == index * member_byte_count
    });
    (contiguous
        && matches!(member_byte_count, 4 | 8)
        && layout.size == member_byte_count * usize::from(members)
        && layout.alignment == member_byte_count)
        .then_some((member_byte_count, members))
}

fn collect_homogeneous_float_leaves(
    input: &InstructionSelectionInput<'_>,
    descriptor: &TypeLayoutDescriptor,
    layout: omega_layout::TypeLayout,
    base_offset: usize,
    leaves: &mut Vec<(usize, usize)>,
    depth: usize,
) -> Option<()> {
    if depth > 8 || layout.size == 0 {
        return None;
    }
    match descriptor {
        TypeLayoutDescriptor::Constrained { base_type, .. } => {
            collect_homogeneous_float_leaves(input, base_type, layout, base_offset, leaves, depth)
        }
        TypeLayoutDescriptor::Named { symbol, name } => {
            if let Some(primitive) = PrimitiveType::from_name(name.as_ref()) {
                let expected = omega_layout::primitive_layout(
                    input.target.pointer_size,
                    input.target.pointer_alignment,
                    primitive,
                );
                if !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
                    || layout != expected
                {
                    return None;
                }
                leaves.push((base_offset, layout.size));
                return Some(());
            }
            let data_layout = input
                .layouts
                .data_layouts
                .iter()
                .find(|(_, candidate)| {
                    candidate.symbol == *symbol || candidate.name.as_str() == name.as_str()
                })
                .map(|(_, candidate)| candidate)?;
            if data_layout.layout != layout {
                return None;
            }
            let DataShape::Record { fields } = data_layout.shape else {
                return None;
            };
            for field in input.layouts.fields.span(fields)? {
                collect_homogeneous_float_leaves(
                    input,
                    &field.type_descriptor,
                    field.layout,
                    base_offset.checked_add(field.offset)?,
                    leaves,
                    depth + 1,
                )?;
            }
            Some(())
        }
        TypeLayoutDescriptor::FixedArray {
            element_type,
            length,
        } => {
            if *length == 0 {
                return None;
            }
            let element_layout = boundary_descriptor_layout(input, element_type, depth + 1)?;
            if layout.size != element_layout.size.checked_mul(*length)?
                || layout.alignment != element_layout.alignment
            {
                return None;
            }
            for index in 0..*length {
                collect_homogeneous_float_leaves(
                    input,
                    element_type,
                    element_layout,
                    base_offset.checked_add(element_layout.size.checked_mul(index)?)?,
                    leaves,
                    depth + 1,
                )?;
            }
            Some(())
        }
        TypeLayoutDescriptor::Reference { .. }
        | TypeLayoutDescriptor::BoundedByteBuffer { .. }
        | TypeLayoutDescriptor::Slice { .. }
        | TypeLayoutDescriptor::DynamicTrait { .. }
        | TypeLayoutDescriptor::Unit => None,
    }
}

/// Preserve the currently normalized classified SysV record family. Each
/// scalar leaf contributes INTEGER or SSE to its containing eightbyte;
/// INTEGER dominates within an eightbyte. All-INTEGER records keep using the
/// native small-aggregate operand; HFA selection runs before this classifier.
fn system_v_classified_aggregate_operand_at(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
    index: usize,
) -> Option<InstructionOperandKind> {
    if CallingPolicy::native_for_target(input.target) != CallingPolicy::SystemVAMD64 {
        return None;
    }
    let argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.get(index))?;
    let HostCallArgumentKind::Expression(expression) = &argument.kind else {
        return None;
    };
    let place = alias_resolved_place_at(input, host_call, dispatch_index, alias_context, index)
        .or_else(|| {
            resolve_runtime_storage_place_in_table(
                input,
                dispatch_index.unwrap_or(0),
                host_call.source_key,
                &input.host_calls.expressions,
                *expression,
            )
        })?;
    let descriptor = resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        *expression,
    )?;
    let (byte_count, alignment, sse_eightbytes) =
        system_v_classified_aggregate_descriptor_shape(input, &descriptor)?;
    if place.byte_count != byte_count {
        return None;
    }
    if byte_count <= 8 {
        return (sse_eightbytes == 0b01).then_some(InstructionOperandKind::RuntimeScalarFloat {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_count,
        });
    }
    Some(InstructionOperandKind::RuntimeSystemVAggregate {
        region: place.region,
        byte_offset: place.byte_offset,
        byte_count,
        alignment,
        sse_eightbytes,
    })
}

fn system_v_classified_aggregate_descriptor_shape(
    input: &InstructionSelectionInput<'_>,
    descriptor: &TypeLayoutDescriptor,
) -> Option<(usize, usize, u8)> {
    system_v_record_descriptor_shape(input, descriptor)
        .filter(|(_, _, sse_eightbytes)| *sse_eightbytes != 0)
}

/// Preserve a fixed pure-integer record of at most two ABI words as one
/// selected operand for native C ABIs. AAPCS64 and SysV AMD64 may split the
/// value into architecture-specific fragments or move it wholly to stack;
/// Microsoft x64 passes widths above eight bytes through a caller copy.
fn native_small_aggregate_argument_operand_at(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
    index: usize,
) -> Option<InstructionOperandKind> {
    if !matches!(
        CallingPolicy::native_for_target(input.target),
        CallingPolicy::Aapcs64 | CallingPolicy::MicrosoftX64 | CallingPolicy::SystemVAMD64
    ) {
        return None;
    }
    let argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.get(index))?;
    let HostCallArgumentKind::Expression(expression) = &argument.kind else {
        return None;
    };
    let place = alias_resolved_place_at(input, host_call, dispatch_index, alias_context, index)
        .or_else(|| {
            resolve_runtime_storage_place_in_table(
                input,
                dispatch_index.unwrap_or(0),
                host_call.source_key,
                &input.host_calls.expressions,
                *expression,
            )
        })?;
    let descriptor = resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        *expression,
    )?;
    let (byte_count, alignment) = match CallingPolicy::native_for_target(input.target) {
        CallingPolicy::SystemVAMD64 => {
            system_v_pure_integer_aggregate_descriptor_shape(input, &descriptor)?
        }
        CallingPolicy::MicrosoftX64 => aggregate_descriptor_shape(input, &descriptor)
            .filter(|(byte_count, _)| *byte_count <= 16)?,
        _ => small_aggregate_descriptor_shape(input, &descriptor)?,
    };
    (place.byte_count == byte_count).then_some(InstructionOperandKind::RuntimeSmallAggregate {
        region: place.region,
        byte_offset: place.byte_offset,
        byte_count,
        alignment,
    })
}

/// Preserve a fixed pure-integer record above two ABI words for the native C
/// plan. AAPCS64 and Microsoft x64 pass a pointer to a caller copy; SysV places
/// the MEMORY-class value directly in the outgoing stack area.
fn native_large_aggregate_argument_operand_at(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
    index: usize,
) -> Option<InstructionOperandKind> {
    if !matches!(
        CallingPolicy::native_for_target(input.target),
        CallingPolicy::Aapcs64 | CallingPolicy::MicrosoftX64 | CallingPolicy::SystemVAMD64
    ) {
        return None;
    }
    let argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.get(index))?;
    let HostCallArgumentKind::Expression(expression) = &argument.kind else {
        return None;
    };
    let place = alias_resolved_place_at(input, host_call, dispatch_index, alias_context, index)
        .or_else(|| {
            resolve_runtime_storage_place_in_table(
                input,
                dispatch_index.unwrap_or(0),
                host_call.source_key,
                &input.host_calls.expressions,
                *expression,
            )
        })?;
    let descriptor = resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        *expression,
    )?;
    let (byte_count, alignment) = aggregate_descriptor_shape(input, &descriptor)?;
    (byte_count > 16 && place.byte_count == byte_count).then_some(
        InstructionOperandKind::RuntimeLargeAggregate {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_count,
            alignment,
        },
    )
}

fn small_aggregate_descriptor_shape(
    input: &InstructionSelectionInput<'_>,
    descriptor: &TypeLayoutDescriptor,
) -> Option<(usize, usize)> {
    aggregate_descriptor_shape(input, descriptor)
        .filter(|(byte_count, _)| (1..=16).contains(byte_count))
}

fn system_v_pure_integer_aggregate_descriptor_shape(
    input: &InstructionSelectionInput<'_>,
    descriptor: &TypeLayoutDescriptor,
) -> Option<(usize, usize)> {
    system_v_record_descriptor_shape(input, descriptor).and_then(
        |(byte_count, alignment, sse_eightbytes)| {
            (sse_eightbytes == 0).then_some((byte_count, alignment))
        },
    )
}

pub(in crate::selection) fn system_v_record_descriptor_shape(
    input: &InstructionSelectionInput<'_>,
    descriptor: &TypeLayoutDescriptor,
) -> Option<(usize, usize, u8)> {
    if !aggregate_descriptor_is_public_fixed_shape(input, descriptor) {
        return None;
    }
    let layout = boundary_descriptor_layout(input, descriptor, 0)?;
    if !(1..=16).contains(&layout.size)
        || !layout.alignment.is_power_of_two()
        || layout.alignment > 8
    {
        return None;
    }
    let mut classes = [None, None];
    classify_system_v_field(input, descriptor, layout, 0, layout.size, &mut classes, 0)?;
    let Some(first_is_sse) = classes[0] else {
        return None;
    };
    let second_is_sse = if layout.size > 8 { classes[1]? } else { false };
    let sse_eightbytes = u8::from(first_is_sse) | (u8::from(second_is_sse) << 1);
    Some((layout.size, layout.alignment, sse_eightbytes))
}

fn classify_system_v_record(
    input: &InstructionSelectionInput<'_>,
    data_layout: &omega_layout::DataLayout,
    base_offset: usize,
    outer_size: usize,
    classes: &mut [Option<bool>; 2],
    depth: usize,
) -> Option<()> {
    if depth > 8 || data_layout.layout.size == 0 {
        return None;
    }
    let DataShape::Record { fields } = data_layout.shape else {
        return None;
    };
    for field in input.layouts.fields.span(fields)? {
        let absolute_offset = base_offset.checked_add(field.offset)?;
        let field_end = absolute_offset.checked_add(field.layout.size)?;
        let relative_end = field.offset.checked_add(field.layout.size)?;
        if field.layout.size == 0
            || field.layout.alignment == 0
            || !field.layout.alignment.is_power_of_two()
            || absolute_offset % field.layout.alignment != 0
            || field_end > outer_size
            || relative_end > data_layout.layout.size
        {
            return None;
        }
        classify_system_v_field(
            input,
            &field.type_descriptor,
            field.layout,
            absolute_offset,
            outer_size,
            classes,
            depth,
        )?;
    }
    Some(())
}

fn classify_system_v_field(
    input: &InstructionSelectionInput<'_>,
    descriptor: &TypeLayoutDescriptor,
    layout: omega_layout::TypeLayout,
    absolute_offset: usize,
    outer_size: usize,
    classes: &mut [Option<bool>; 2],
    depth: usize,
) -> Option<()> {
    if depth > 8 {
        return None;
    }
    match descriptor {
        TypeLayoutDescriptor::Constrained { base_type, .. } => classify_system_v_field(
            input,
            base_type,
            layout,
            absolute_offset,
            outer_size,
            classes,
            depth,
        ),
        TypeLayoutDescriptor::Reference { .. } => {
            (layout.size == 8).then_some(())?;
            merge_system_v_scalar_class(classes, absolute_offset, 8, false)
        }
        TypeLayoutDescriptor::Named { symbol, name } => {
            if let Some(primitive) = PrimitiveType::from_name(name.as_ref()) {
                let scalar_size = primitive.scalar_byte_size()?;
                if scalar_size != layout.size || scalar_size > 8 {
                    return None;
                }
                return merge_system_v_scalar_class(
                    classes,
                    absolute_offset,
                    scalar_size,
                    matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64),
                );
            }
            let nested = input
                .layouts
                .data_layouts
                .iter()
                .find(|(_, candidate)| {
                    candidate.symbol == *symbol || candidate.name.as_str() == name.as_str()
                })
                .map(|(_, candidate)| candidate)?;
            if nested.layout != layout || absolute_offset.checked_add(layout.size)? > outer_size {
                return None;
            }
            classify_system_v_record(
                input,
                nested,
                absolute_offset,
                outer_size,
                classes,
                depth + 1,
            )
        }
        TypeLayoutDescriptor::FixedArray {
            element_type,
            length,
        } => {
            if *length == 0 || layout.size % length != 0 {
                return None;
            }
            let element_layout = omega_layout::TypeLayout {
                size: layout.size / length,
                alignment: layout.alignment,
            };
            if element_layout.size == 0 {
                return None;
            }
            for index in 0..*length {
                let element_offset = element_layout.size.checked_mul(index)?;
                classify_system_v_field(
                    input,
                    element_type,
                    element_layout,
                    absolute_offset.checked_add(element_offset)?,
                    outer_size,
                    classes,
                    depth + 1,
                )?;
            }
            Some(())
        }
        TypeLayoutDescriptor::BoundedByteBuffer { .. }
        | TypeLayoutDescriptor::Slice { .. }
        | TypeLayoutDescriptor::DynamicTrait { .. }
        | TypeLayoutDescriptor::Unit => None,
    }
}

fn merge_system_v_scalar_class(
    classes: &mut [Option<bool>; 2],
    absolute_offset: usize,
    byte_size: usize,
    is_sse: bool,
) -> Option<()> {
    let eightbyte = absolute_offset / 8;
    let last_byte = absolute_offset.checked_add(byte_size)?.checked_sub(1)?;
    if byte_size == 0 || eightbyte > 1 || eightbyte != last_byte / 8 {
        return None;
    }
    classes[eightbyte] = Some(match classes[eightbyte] {
        Some(existing_is_sse) => existing_is_sse && is_sse,
        None => is_sse,
    });
    Some(())
}

fn aggregate_descriptor_shape(
    input: &InstructionSelectionInput<'_>,
    descriptor: &TypeLayoutDescriptor,
) -> Option<(usize, usize)> {
    if !aggregate_descriptor_is_public_fixed_shape(input, descriptor) {
        return None;
    }
    let layout = boundary_descriptor_layout(input, descriptor, 0)?;
    (layout.size > 0 && layout.alignment.is_power_of_two())
        .then_some((layout.size, layout.alignment))
}

fn aggregate_descriptor_is_public_fixed_shape(
    input: &InstructionSelectionInput<'_>,
    descriptor: &TypeLayoutDescriptor,
) -> bool {
    match descriptor {
        TypeLayoutDescriptor::Constrained { base_type, .. } => {
            aggregate_descriptor_is_public_fixed_shape(input, base_type)
        }
        TypeLayoutDescriptor::FixedArray { .. } => true,
        TypeLayoutDescriptor::Named { symbol, name } => {
            PrimitiveType::from_name(name.as_ref()).is_none()
                && input.layouts.data_layouts.iter().any(|(_, layout)| {
                    (layout.symbol == *symbol || layout.name.as_str() == name.as_str())
                        && matches!(layout.shape, DataShape::Record { .. })
                })
        }
        TypeLayoutDescriptor::Reference { .. }
        | TypeLayoutDescriptor::BoundedByteBuffer { .. }
        | TypeLayoutDescriptor::Slice { .. }
        | TypeLayoutDescriptor::DynamicTrait { .. }
        | TypeLayoutDescriptor::Unit => false,
    }
}

fn boundary_descriptor_layout(
    input: &InstructionSelectionInput<'_>,
    descriptor: &TypeLayoutDescriptor,
    depth: usize,
) -> Option<omega_layout::TypeLayout> {
    if depth > 8 {
        return None;
    }
    match descriptor {
        TypeLayoutDescriptor::Constrained { base_type, .. } => {
            boundary_descriptor_layout(input, base_type, depth)
        }
        TypeLayoutDescriptor::Reference { .. } => Some(omega_layout::TypeLayout {
            size: input.target.pointer_size,
            alignment: input.target.pointer_alignment,
        }),
        TypeLayoutDescriptor::FixedArray {
            element_type,
            length,
        } => {
            let element = boundary_descriptor_layout(input, element_type, depth + 1)?;
            Some(omega_layout::TypeLayout {
                size: element.size.checked_mul(*length)?,
                alignment: element.alignment,
            })
        }
        TypeLayoutDescriptor::Named { symbol, name } => {
            if let Some(primitive) = PrimitiveType::from_name(name.as_ref()) {
                return Some(omega_layout::primitive_layout(
                    input.target.pointer_size,
                    input.target.pointer_alignment,
                    primitive,
                ));
            }
            input
                .layouts
                .data_layouts
                .iter()
                .find(|(_, layout)| {
                    layout.symbol == *symbol || layout.name.as_str() == name.as_str()
                })
                .map(|(_, layout)| layout.layout)
        }
        TypeLayoutDescriptor::BoundedByteBuffer { .. }
        | TypeLayoutDescriptor::Slice { .. }
        | TypeLayoutDescriptor::DynamicTrait { .. }
        | TypeLayoutDescriptor::Unit => None,
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

/// Whether the host-call argument at `index` denotes a borrowed place. Mutable
/// borrows retain a checked-expression marker; immutable `&place` syntax is
/// erased by parsing, so the formal reference type supplies the missing fact.
/// A reference-typed actual already contains the pointer value and therefore
/// must not acquire another address layer.
fn host_call_argument_is_borrow(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    index: usize,
) -> bool {
    let Some(arguments) = input.host_calls.arguments.span(host_call.arguments) else {
        return false;
    };
    let Some(argument) = arguments.get(index) else {
        return false;
    };
    if argument.is_borrowed {
        return true;
    }
    if !argument.expects_reference && !argument.expects_address {
        return false;
    }
    let omega_platform_interface::HostCallArgumentKind::Expression(expression) = argument.kind
    else {
        return false;
    };
    let descriptor = resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index.unwrap_or(0),
        host_call.source_key,
        &input.host_calls.expressions,
        expression,
    );
    if argument.expects_reference {
        return !descriptor.is_some_and(|descriptor| descriptor_is_reference(&descriptor));
    }
    !descriptor.is_some_and(|descriptor| descriptor_is_raw_address(&descriptor))
}

fn descriptor_is_reference(descriptor: &TypeLayoutDescriptor) -> bool {
    match descriptor {
        TypeLayoutDescriptor::Reference { .. } => true,
        TypeLayoutDescriptor::Constrained { base_type, .. } => descriptor_is_reference(base_type),
        _ => false,
    }
}

fn descriptor_is_raw_address(descriptor: &TypeLayoutDescriptor) -> bool {
    match descriptor {
        TypeLayoutDescriptor::Named { name, .. } => name.as_str() == "addr",
        TypeLayoutDescriptor::Constrained { base_type, .. } => descriptor_is_raw_address(base_type),
        _ => false,
    }
}

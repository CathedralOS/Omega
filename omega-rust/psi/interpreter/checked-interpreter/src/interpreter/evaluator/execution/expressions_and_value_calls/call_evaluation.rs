use super::byte_sequence_operand_bytes;
use crate::interpreter::evaluator::{
    ArithmeticDomain, EvalResult, ExpressionHandle, ExpressionNode, FloatMeaning, FloatSemantics,
    Frame, Halt, PrimitiveType, SemanticFloatClass, SemanticFloatFormat, SymbolHandle, Value, trap,
    unsupported,
};
use language_semantics::declaration_selection::CollectionMeasure;
use symbols::BuiltinFunction;

impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
    pub(super) fn eval_call_expression(
        &mut self,
        handle: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        // A transition's guard subject evaluates ONCE per transition evaluation: the
        // parser lowers `transition self.f(x) { true -> a false -> b }` into one guard
        // per arm, each holding a COPY of the subject call (distinct handles, identical
        // structure). A later arm reuses the earlier arm's result instead of re-running
        // the callee's side effects -- matching the native lowering's shared prelude.
        if self.guard_depth > 0
            && let Some(value) = frame.guard_call_result(self.program, handle)
        {
            return Ok(value);
        }

        let receiver_symbol = match self.program.expression_table.expression(call.receiver) {
            ExpressionNode::Member(member) => member.member_symbol,
            ExpressionNode::Name(path) => path.symbol,
            _ => SymbolHandle::invalid(),
        };
        if let Some(dispatch) = self.selected_boundary_adapter(
            receiver_symbol,
            call.target_symbol,
            &call.machine_arguments,
        ) {
            let value = self.run_boundary_adapter(
                dispatch,
                self.program
                    .expression_table
                    .expression_handles(call.arguments),
                frame,
            )?;
            if self.guard_depth > 0 {
                frame.record_guard_call_result(handle, &value);
            }
            return Ok(value);
        }

        // An explicitly bound bodyless wait is a host call even when its
        // top-level requirement has a resolvable entry state. Checked adapters
        // above still execute before their external leaf reaches this route.
        if let Some(value) = self.try_virtual_clock_operation(
            call.target_symbol,
            self.program
                .expression_table
                .expression_handles(call.arguments),
            frame,
        )? {
            if self.guard_depth > 0 {
                frame.record_guard_call_result(handle, &value);
            }
            return Ok(value);
        }

        // A boundary operator application (`CheckedMath::offset_zero(70)`,
        // `I8::from_f32(x)`, a policy cast bound to its conversion) or a
        // spelled float operation bound to its bodyless `Float` requirement
        // runs the realization checked Psi selects for that exact
        // requirement. The requirement owns no executable state, so it
        // resolves here by identity before any spelling-keyed route below
        // could claim its leaf.
        if let Some(value) = self.try_operator_application(
            call.target_symbol,
            handle,
            self.program
                .expression_table
                .expression_handles(call.arguments),
            frame,
        )? {
            if self.guard_depth > 0 {
                frame.record_guard_call_result(handle, &value);
            }
            return Ok(value);
        }

        // A byte-sequence predicate (`valid_utf8`, `no_nul`, `ascii_only`,
        // `non_empty`) owns no executable callee: its contract evaluates the
        // argument's live bytes directly.
        if let Some((predicate, argument)) =
            typed_trees::byte_predicates::byte_predicate_call(self.program, handle)
        {
            let value = self.eval_expression(argument, frame)?;
            let bytes = byte_sequence_operand_bytes(&value)?;
            let value = Value::Bool(predicate.holds_for(&bytes));
            if self.guard_depth > 0 {
                frame.record_guard_call_result(handle, &value);
            }
            return Ok(value);
        }

        // Build paths are selected by exact toolchain identity; the explicit
        // facet operand, not a declaration namespace, carries runtime authority.
        if let Some(value) = self.try_build_path_call(
            call.target_symbol,
            self.program
                .expression_table
                .expression_handles(call.arguments),
            frame,
        )? {
            if self.guard_depth > 0 {
                frame.record_guard_call_result(handle, &value);
            }
            return Ok(value);
        }

        // Product selection takes its authority as an ordinary explicit
        // argument, before namespace classification of the selected call.
        if let Some(value) = self.try_build_product_entry_call(
            call.target_symbol,
            self.program
                .expression_table
                .expression_handles(call.arguments),
            self.program.expression_table.source_span(handle),
            frame,
        )? {
            return Ok(value);
        }
        if let Some(value) = self.try_build_product_schema_call(
            call.target_symbol,
            self.program
                .expression_table
                .expression_handles(call.arguments),
            self.program.expression_table.source_span(handle),
            frame,
        )? {
            return Ok(value);
        }
        if let Some(value) = self.try_build_product_provider_call(
            call.target_symbol,
            self.program
                .expression_table
                .expression_handles(call.arguments),
            self.program.expression_table.source_span(handle),
            frame,
        )? {
            return Ok(value);
        }
        if let Some(value) = self.try_product_description_path_call(
            call.target_symbol,
            self.program
                .expression_table
                .expression_handles(call.arguments),
            frame,
        )? {
            return Ok(value);
        }
        // Required-output custody takes the activation facet and the issued
        // obligation as ordinary explicit operands, so it is selected here by
        // resolved toolchain identity rather than below by receiver shape.
        if let Some(value) = self.try_build_output_obligation_call(
            call.target_symbol,
            self.program
                .expression_table
                .expression_handles(call.arguments),
            frame,
        )? {
            return Ok(value);
        }
        if let Some(value) = self.try_required_output_path_call(
            call.target_symbol,
            self.program
                .expression_table
                .expression_handles(call.arguments),
            frame,
        )? {
            return Ok(value);
        }
        if let Some(value) = self.try_build_log_write_line_call(
            call.target_symbol,
            self.program
                .expression_table
                .expression_handles(call.arguments),
            frame,
        )? {
            return Ok(value);
        }
        let target = call.target.as_str();
        // Collection builtins and the host fallback below still classify the
        // qualifier: probing storage for a qualified user `read`/`resolve`
        // would trap before its exact ordinary target can execute.
        let qualified = self.expression_receiver_is_namespace(call.receiver);
        if let Some(request) = &call.private_layout_operation {
            let arguments = self
                .program
                .expression_table
                .expression_handles(call.arguments);
            let [plan, offset] = arguments else {
                return trap(
                    "compiler-known `Plan::place_private` requires one Plan and one u64 offset",
                );
            };
            let plan = self.eval_expression(*plan, frame)?;
            let offset = match self.eval_expression(*offset, frame)? {
                Value::Int(offset) => u64::try_from(offset).map_err(|_| {
                    Halt::Trap(format!(
                        "compiler-known `Plan::place_private` offset `{offset}` is outside the u64 range"
                    ))
                })?,
                other => {
                    return trap(format!(
                        "compiler-known `Plan::place_private` offset is not an integer: {other:?}"
                    ));
                }
            };
            self.private_layout_placements
                .push(crate::PrivateLayoutPlacementReceipt {
                    operation_expression: handle,
                    selected_slot: request.selected_slot.clone(),
                    offset,
                });
            return Ok(plan);
        }
        // Product declarations are harvested for the product, but their
        // explicit argument still has ordinary evaluation and Build custody.
        // A resolved same-named user declaration remains an ordinary call.
        if !call.target_symbol.is_valid()
            && matches!(
                language_semantics::declaration_selection::BuildOperation::from_call_target(target),
                Some(language_semantics::declaration_selection::BuildOperation::RepresentationSelection
                    | language_semantics::declaration_selection::BuildOperation::BoundaryAcceptance
                    | language_semantics::declaration_selection::BuildOperation::WireCompatibilityRequest)
            )
        {
            let [builder] = self.program.expression_table.expression_handles(call.arguments) else {
                return trap("build declaration requires one explicit mutable Build argument");
            };
            let builder = self.eval_argument(*builder, frame)?;
            self.require_root_build_argument(self.deref_cell(builder), target)?;
            if language_semantics::declaration_selection::BuildOperation::from_call_target(target)
                == Some(language_semantics::declaration_selection::BuildOperation::WireCompatibilityRequest)
            {
                self.executed_wire_demands.push(handle);
            }
            return Ok(Value::Unit);
        }
        if let Some(value) = self.try_provider_selection_value_call(handle, call, frame)? {
            if self.guard_depth > 0 {
                frame.record_guard_call_result(handle, &value);
            }
            return Ok(value);
        }
        if let Some(value) = self.try_component_assumption_acceptance(
            call.target_symbol,
            self.program
                .expression_table
                .expression_handles(call.arguments),
            frame,
        )? {
            if self.guard_depth > 0 {
                frame.record_guard_call_result(handle, &value);
            }
            return Ok(value);
        }
        // Evaluated behavior exclusions (see the statement-call twin): the
        // call records the selection only when it actually runs against the
        // activation's root Build.
        if let Some(value) = self.try_behavior_exclusion_value_call(handle, call, frame)? {
            if self.guard_depth > 0 {
                frame.record_guard_call_result(handle, &value);
            }
            return Ok(value);
        }
        // The tree walker has no architectural flags register. Preserve the
        // value-flow shape with the architecturally fixed RFLAGS bit 1 set;
        // the matching restore statement is a no-op above.
        let builtin = self
            .program
            .symbols
            .builtin_function_for_symbol(call.target_symbol);
        // Authored numeric names retain ordinary machine dispatch below;
        // only the selected intrinsic owns builtin evaluation and bounds.
        if builtin == Some(BuiltinFunction::AsmSnapshotFlags) && !call.receiver.is_valid() {
            return Ok(Value::Int(2));
        }
        // Only admitted float requirements select these unnameable operations.
        if let Some(builtin @ (BuiltinFunction::FloatMaximum | BuiltinFunction::FloatMinimum)) =
            builtin
        {
            let arguments = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if arguments.len() == 2 {
                let left = self.eval_expression(arguments[0], frame)?;
                let right = self.eval_expression(arguments[1], frame)?;
                return self.eval_float_extremum(builtin, left, right);
            }
        }
        if matches!(
            target,
            "float#multiply_then_add_f32"
                | "float#multiply_then_add_f64"
                | "float#fused_multiply_add_f32"
                | "float#fused_multiply_add_f64"
                | "float#fused_multiply_add_toward_zero_f32"
                | "float#fused_multiply_add_toward_zero_f64"
                | "float#fused_multiply_add_toward_positive_f32"
                | "float#fused_multiply_add_toward_positive_f64"
                | "float#fused_multiply_add_toward_negative_f32"
                | "float#fused_multiply_add_toward_negative_f64"
        ) && call.receiver == ExpressionHandle::invalid()
        {
            let arguments = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if arguments.len() == 3 {
                let format = if target.ends_with("_f32") {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                return self.eval_rewritten_ternary_float(&arguments, format, target, frame);
            }
        }
        if matches!(
            target,
            "float#add_toward_zero_f32"
                | "float#add_toward_zero_f64"
                | "float#add_toward_positive_f32"
                | "float#add_toward_positive_f64"
                | "float#add_toward_negative_f32"
                | "float#add_toward_negative_f64"
                | "float#subtract_toward_zero_f32"
                | "float#subtract_toward_zero_f64"
                | "float#subtract_toward_positive_f32"
                | "float#subtract_toward_positive_f64"
                | "float#subtract_toward_negative_f32"
                | "float#subtract_toward_negative_f64"
                | "float#multiply_toward_zero_f32"
                | "float#multiply_toward_zero_f64"
                | "float#multiply_toward_positive_f32"
                | "float#multiply_toward_positive_f64"
                | "float#multiply_toward_negative_f32"
                | "float#multiply_toward_negative_f64"
                | "float#divide_toward_zero_f32"
                | "float#divide_toward_zero_f64"
                | "float#divide_toward_positive_f32"
                | "float#divide_toward_positive_f64"
                | "float#divide_toward_negative_f32"
                | "float#divide_toward_negative_f64"
        ) && call.receiver == ExpressionHandle::invalid()
        {
            let arguments = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if arguments.len() == 2 {
                let format = if target.ends_with("_f32") {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                return self.eval_rewritten_directed_binary(&arguments, format, target, frame);
            }
        }
        if matches!(
            target,
            "float#sqrt_toward_zero_f32"
                | "float#sqrt_toward_zero_f64"
                | "float#sqrt_toward_positive_f32"
                | "float#sqrt_toward_positive_f64"
                | "float#sqrt_toward_negative_f32"
                | "float#sqrt_toward_negative_f64"
        ) && call.receiver == ExpressionHandle::invalid()
        {
            let arguments = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if arguments.len() == 1 {
                let format = if target.ends_with("_f32") {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                return self.eval_rewritten_directed_square_root(&arguments, format, target, frame);
            }
        }
        // Builtin: sqrt over a single float operand. The interpreter consumes
        // the same exact semantic function used as the native
        // sqrtsd/sqrtss contract oracle.
        if builtin == Some(BuiltinFunction::FloatSquareRoot)
            && call.receiver == ExpressionHandle::invalid()
        {
            if let Some((value, format)) = self.unary_float_argument(call, frame)? {
                return match value {
                    value if value.is_float() => {
                        let meaning = self.float_operand(&value, format)?;
                        Ok(self.semantic_float_result(
                            FloatSemantics::square_root(format, &meaning),
                            format,
                        ))
                    }
                    other => Err(Halt::Trap(format!(
                        "sqrt expects a float argument, got {other:?}"
                    ))),
                };
            }
        }
        // Internal enum-valued classifiers selected only by exact named-float
        // plans. The format is encoded in the unnameable builtin identity so
        // state-local expression copying cannot erase it.
        if matches!(target, "float#classify_f32" | "float#classify_f64")
            && call.receiver == ExpressionHandle::invalid()
        {
            let arguments = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if arguments.len() == 1 {
                let format = if target == "float#classify_f32" {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                return match self.eval_expression(arguments[0], frame)? {
                    value if value.is_float() => {
                        let meaning = self.float_operand(&value, format)?;
                        let (variant_name, negative) =
                            match FloatSemantics::classify(format, &meaning) {
                                SemanticFloatClass::NaN => ("NaN", None),
                                SemanticFloatClass::Infinity { negative } => {
                                    ("Infinity", Some(negative))
                                }
                                SemanticFloatClass::Normal { negative } => {
                                    ("Normal", Some(negative))
                                }
                                SemanticFloatClass::Subnormal { negative } => {
                                    ("Subnormal", Some(negative))
                                }
                                SemanticFloatClass::Zero { negative } => ("Zero", Some(negative)),
                            };
                        Ok(Value::Enum {
                            type_symbol: self
                                .find_data_by_name("FloatClass")
                                .map(|data| data.symbol)
                                .unwrap_or_else(SymbolHandle::invalid),
                            variant_name: variant_name.into(),
                            payload: match negative {
                                Some(negative) => vec![(
                                    "negative".into(),
                                    self.allocate_cell(Value::Bool(negative))?,
                                )],
                                None => Vec::new(),
                            },
                        })
                    }
                    _ => unsupported("float classification argument is not a float"),
                };
            }
        }

        // Internal unary predicates selected only by exact named-float plans.
        // Evaluate the argument once; lowering a predicate to a duplicated
        // expression would repeat nontrivial argument evaluation.
        if matches!(
            target,
            "float#is_nan"
                | "float#is_finite"
                | "float#is_infinite"
                | "float#is_normal"
                | "float#is_subnormal"
        ) && call.receiver == ExpressionHandle::invalid()
        {
            if let Some((value, format)) = self.unary_float_argument(call, frame)? {
                return match value {
                    value if value.is_float() => {
                        let meaning = self.float_operand(&value, format)?;
                        let result = match target {
                            "float#is_nan" => FloatSemantics::is_nan(&meaning),
                            "float#is_finite" => FloatSemantics::is_finite(&meaning),
                            "float#is_infinite" => FloatSemantics::is_infinite(&meaning),
                            "float#is_normal" => FloatSemantics::is_normal(format, &meaning),
                            "float#is_subnormal" => FloatSemantics::is_subnormal(format, &meaning),
                            _ => unreachable!(),
                        };
                        Ok(Value::Bool(result))
                    }
                    _ => unsupported("float classification argument is not a float"),
                };
            }
        }

        // `.len()` on an array-valued receiver returns the element count.
        if CollectionMeasure::from_authored_spelling(target) == Some(CollectionMeasure::Length)
            && call.receiver.is_valid()
            && !qualified
            && let Ok(cell) = self.resolve_place(call.receiver, frame)
        {
            let cell = self.deref_cell(cell);
            if let Value::Array(elements) = &*cell.borrow() {
                return Ok(Value::Int(elements.len() as i64));
            }
        }

        // A selected concrete input leaf has no authored body to execute.
        // Resolve its exact satisfaction before ordinary machine execution,
        // but after guard memoization so one guard subject observes one byte.
        if self.exact_console_intrinsic_host_method(call.target_symbol) == Some("read_byte") {
            self.host_boundary_touched = true;
            self.non_fs_host_boundary_touched = true;
            let value = self.read_stdin_byte_value(call.target_symbol)?;
            if self.guard_depth > 0 {
                frame.record_guard_call_result(handle, &value);
            }
            return Ok(value);
        }

        // Ordinary calls consume the checked target, exactly as statement
        // calls do. A boundary requirement may instead have a host realization.
        let (machine, entry_state) = match self.resolve_static_call(call.target_symbol, target) {
            Ok(resolution) => resolution,
            Err(halt) => {
                // A host-boundary VALUE call (`self.clock.tick_count()`,
                // `self.fs.create(..)`): driven directly, like the
                // statement-position host calls in try_host_call. User machines
                // take precedence -- the host fallback only fires when nothing
                // else resolves, mirroring the native collection (which keys on
                // boundary-trait signature symbols).

                // Compatibility Math boundary calls are still imported through
                // the host tables natively. The interpreter must nevertheless
                // consume the same executable semantic definition as build-time
                // folding and ordinary landed arithmetic; otherwise FMA acquires
                // a third, host-language meaning (or remains unsupported).
                if let Some(value) = self.try_float_boundary_value_call(target, call, frame)? {
                    return Ok(value);
                }

                // Value-returning canonical FilesystemHost ops
                // (assignment-position calls like
                // `self.fd = self.fs.create(path, mode)`). Exact toolchain
                // requirement identity selects authority before the readable
                // leaf routes inside the provider.
                if let Some(filesystem_operation) =
                    self.exact_filesystem_host_operation(call.target_symbol)?
                {
                    let fs_args = self
                        .program
                        .expression_table
                        .expression_handles(call.arguments)
                        .to_vec();
                    let value = self.try_filesystem_call(filesystem_operation, &fs_args, frame)?;
                    self.host_boundary_touched = true;
                    return Ok(value);
                }

                // A resolved boundary requirement the host serves (the
                // bounded `Console::read_line(&mut line)`) runs by that
                // identity, exactly like its statement-position twin. The
                // requirement is bodyless, so declaration resolution above
                // never selects it; ordinary calls resolve before this
                // lookup is paid.
                if let Some(value) = self.try_host_value_call(call, frame)? {
                    if self.guard_depth > 0 {
                        frame.record_guard_call_result(handle, &value);
                    }
                    return Ok(value);
                }

                // Identity-backed intrinsic/requirement routes above remain
                // valid for namespace calls. A missing ordinary declaration
                // cannot inherit a host operation from its leaf spelling.
                if qualified {
                    return Err(halt);
                }

                if matches!(
                    target,
                    "tick_count"
                        | "key_state"
                        | "dc_create"
                        | "get_dc"
                        | "window_create"
                        | "is_window"
                        | "window_destroy"
                        | "foreground_window"
                        | "msg_peek"
                        | "msg_translate"
                        | "msg_dispatch"
                ) {
                    // Value-position host fallbacks are host-boundary calls too
                    // (the build-time purity backstop must see them); none of
                    // these are filesystem ops, so the granted-build backstop
                    // sees them as well.
                    self.host_boundary_touched = true;
                    self.non_fs_host_boundary_touched = true;
                }
                if target == "read_byte" {
                    // The next raw stdin byte as `ByteRead::Byte { value }`,
                    // or `ByteRead::Eof` at end-of-input (the ZII zero case;
                    // sentinel spellings are vetoed); the byte path does no CRLF
                    // normalization. Mirrors the statement-position arm in
                    // try_host_call, but read_byte is value-position by
                    // nature (`let r = self.console.read_byte()`).
                    self.host_boundary_touched = true;
                    self.non_fs_host_boundary_touched = true;
                    let value = self.read_stdin_byte_value(call.target_symbol)?;
                    if self.guard_depth > 0 {
                        frame.record_guard_call_result(handle, &value);
                    }
                    return Ok(value);
                }
                if let Some(value) = self.virtual_time_host_value(target) {
                    self.host_boundary_touched = true;
                    self.non_fs_host_boundary_touched = true;
                    return Ok(value);
                }
                if target == "tick_count" {
                    self.advance_virtual_clock(1)?;
                    return Ok(Value::Int(self.virtual_ticks));
                }
                if target == "key_state" {
                    // The virtual host has no keyboard: no key is ever down.
                    return Ok(Value::Int(0));
                }
                if target == "dc_create" || target == "get_dc" {
                    // Virtual device contexts are the opaque non-zero token 1
                    // (programs must branch on handle != 0, never on a concrete
                    // handle value -- native handles are real pointers).
                    return Ok(Value::Int(1));
                }
                if target == "window_create" {
                    // Mint a live virtual window handle token.
                    self.virtual_window_next += 1;
                    self.virtual_live_windows.insert(self.virtual_window_next);
                    return Ok(Value::Int(self.virtual_window_next));
                }
                if target == "foreground_window" {
                    // The virtual desktop has one app: the most recently
                    // created window is foreground while it lives, 0 after.
                    let foreground = if self
                        .virtual_live_windows
                        .contains(&self.virtual_window_next)
                    {
                        self.virtual_window_next
                    } else {
                        0
                    };
                    return Ok(Value::Int(foreground));
                }
                if target == "is_window" || target == "window_destroy" {
                    // Liveness mirrors native IsWindow/DestroyWindow: 1 for a
                    // live handle, 0 otherwise; destroy removes it.
                    let Some(handle_argument) = self
                        .program
                        .expression_table
                        .expression_handles(call.arguments)
                        .first()
                        .copied()
                    else {
                        return Err(halt);
                    };
                    let handle = match &*self
                        .eval_call_expression_argument(handle_argument, frame)?
                        .borrow()
                    {
                        Value::Int(handle) => *handle,
                        _ => return Ok(Value::Int(0)),
                    };
                    let live = if target == "window_destroy" {
                        self.virtual_live_windows.remove(&handle)
                    } else {
                        self.virtual_live_windows.contains(&handle)
                    };
                    return Ok(Value::Int(i64::from(live)));
                }
                if target == "msg_peek" || target == "msg_translate" || target == "msg_dispatch" {
                    // The virtual host posts no messages: the queue is always
                    // empty (peek = 0) and translate/dispatch have nothing to do.
                    return Ok(Value::Int(0));
                }
                if target == "blit" {
                    // Virtual GDI blit(hdc, dest_w, dest_h, src_w, src_h, pixels,
                    // info): StretchDIBits reports the copied SOURCE scanline
                    // count (probed natively: the source height even when
                    // stretching, even into the memory DC's default 1x1 bitmap).
                    let Some(height) = self
                        .program
                        .expression_table
                        .expression_handles(call.arguments)
                        .get(4)
                        .copied()
                    else {
                        return Err(halt);
                    };
                    return self
                        .eval_call_expression_argument(height, frame)
                        .map(|value| value.borrow().clone());
                }
                return Err(halt);
            }
        };
        let arguments = self.eval_state_arguments(
            entry_state,
            self.program
                .expression_table
                .expression_handles(call.arguments),
            frame,
        )?;
        // Suspend the guard flag while the callee RUNS: distinct same-shaped calls
        // inside its body are genuine repeat calls, not copies of one source
        // expression, and must not memoize against each other.
        let entered_guard_depth = self.guard_depth;
        self.guard_depth = 0;
        let value = self
            .run_state_collect(machine, entry_state, arguments)
            .map(|value| value.unwrap_or(Value::Unit));
        self.guard_depth = entered_guard_depth;
        let value = value?;
        if entered_guard_depth > 0 {
            frame.record_guard_call_result(handle, &value);
        }
        Ok(value)
    }

    pub(super) fn try_float_boundary_value_call(
        &mut self,
        target: &str,
        call: &typed_trees::expression::TableCallExpression,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        if !call.receiver.is_valid() {
            return Ok(None);
        }

        let core_format = typed_trees::operator::resolve_named_expression_call(self.program, call)
            .and_then(|operator| {
                self.program
                    .operator_path_members(operator.name)
                    .first()
                    .and_then(|namespace| match namespace.as_str() {
                        "F32" => Some(SemanticFloatFormat::BINARY32),
                        "F64" => Some(SemanticFloatFormat::BINARY64),
                        _ => None,
                    })
            });
        let compatibility_call = core_format.is_none()
            && !self.expression_receiver_is_namespace(call.receiver)
            && matches!(target, "square_root" | "fused_multiply_add");
        if core_format.is_none() && !compatibility_call {
            return Ok(None);
        }

        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec();
        let Some(expected_arity) =
            numerics::float_semantics_catalog::FloatSemanticOperation::named_call_arity(target)
        else {
            return Ok(None);
        };
        if arguments.len() != expected_arity {
            return Ok(None);
        }
        let format = core_format.unwrap_or_else(|| {
            if matches!(
                self.expression_scalar_type(arguments[0], frame),
                Some((PrimitiveType::F32, _))
            ) {
                SemanticFloatFormat::BINARY32
            } else {
                SemanticFloatFormat::BINARY64
            }
        });
        let policy_domain = if core_format.is_some() {
            let mut selected = ArithmeticDomain::Exact;
            for argument in &arguments {
                let Some((_, domain)) = self.expression_scalar_type(*argument, frame) else {
                    continue;
                };
                if domain == ArithmeticDomain::Exact {
                    continue;
                }
                if selected != ArithmeticDomain::Exact && selected != domain {
                    return trap(format!(
                        "mixed arithmetic domains in named float operation `{target}`"
                    ));
                }
                selected = domain;
            }
            selected
        } else {
            // Compatibility Math calls are separate host-boundary operations,
            // not the normalized F32/F64 requirement surface.
            ArithmeticDomain::Exact
        };
        let mut operands = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let value = self.eval_expression(argument, frame)?;
            if !value.is_float() {
                return Err(Halt::Trap(format!(
                    "{target} expects float arguments, got {value:?}"
                )));
            }
            operands.push(self.float_operand(&value, format)?);
        }

        let float_value = |meaning: FloatMeaning, division: bool| -> EvalResult<Value> {
            let meaning = match policy_domain {
                ArithmeticDomain::Saturating if division => {
                    FloatSemantics::apply_saturating_divide_policy(
                        format,
                        &operands[0],
                        &operands[1],
                        meaning,
                    )
                }
                ArithmeticDomain::Saturating => {
                    let operand_refs = operands.iter().collect::<Vec<_>>();
                    FloatSemantics::apply_saturating_policy(format, &operand_refs, meaning)
                }
                ArithmeticDomain::Trapping => FloatSemantics::apply_trapping_policy(meaning)
                    .map_err(|trap_class| {
                        Halt::Trap(format!(
                            "named float operation `{target}` produced {} in Trapping domain",
                            trap_class.label()
                        ))
                    })?,
                ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => meaning,
            };
            Ok(self.semantic_float_result(meaning, format))
        };
        let value = match target {
            "negate" => float_value(FloatSemantics::negate(format, &operands[0]), false)?,
            "square_root" => float_value(FloatSemantics::square_root(format, &operands[0]), false)?,
            "multiply_then_add" => float_value(
                FloatSemantics::multiply_then_add(format, &operands[0], &operands[1], &operands[2]),
                false,
            )?,
            "fused_multiply_add" => float_value(
                FloatSemantics::fused_multiply_add(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                ),
                false,
            )?,
            "minimum" => float_value(FloatSemantics::minimum(&operands[0], &operands[1]), false)?,
            "maximum" => float_value(FloatSemantics::maximum(&operands[0], &operands[1]), false)?,
            "classify" => {
                let (variant_name, negative) = match FloatSemantics::classify(format, &operands[0])
                {
                    SemanticFloatClass::NaN => ("NaN", None),
                    SemanticFloatClass::Infinity { negative } => ("Infinity", Some(negative)),
                    SemanticFloatClass::Normal { negative } => ("Normal", Some(negative)),
                    SemanticFloatClass::Subnormal { negative } => ("Subnormal", Some(negative)),
                    SemanticFloatClass::Zero { negative } => ("Zero", Some(negative)),
                };
                Value::Enum {
                    type_symbol: self
                        .find_data_by_name("FloatClass")
                        .map(|data| data.symbol)
                        .unwrap_or_else(SymbolHandle::invalid),
                    variant_name: variant_name.into(),
                    payload: match negative {
                        Some(negative) => vec![(
                            "negative".into(),
                            self.allocate_cell(Value::Bool(negative))?,
                        )],
                        None => Vec::new(),
                    },
                }
            }
            "is_finite" => Value::Bool(FloatSemantics::is_finite(&operands[0])),
            "is_nan" => Value::Bool(FloatSemantics::is_nan(&operands[0])),
            "is_infinite" => Value::Bool(FloatSemantics::is_infinite(&operands[0])),
            "is_normal" => Value::Bool(FloatSemantics::is_normal(format, &operands[0])),
            "is_subnormal" => Value::Bool(FloatSemantics::is_subnormal(format, &operands[0])),
            "add_toward_zero" => float_value(
                FloatSemantics::add_toward_zero(format, &operands[0], &operands[1]),
                false,
            )?,
            "add_toward_positive" => float_value(
                FloatSemantics::add_toward_positive(format, &operands[0], &operands[1]),
                false,
            )?,
            "add_toward_negative" => float_value(
                FloatSemantics::add_toward_negative(format, &operands[0], &operands[1]),
                false,
            )?,
            "subtract_toward_zero" => float_value(
                FloatSemantics::subtract_toward_zero(format, &operands[0], &operands[1]),
                false,
            )?,
            "subtract_toward_positive" => float_value(
                FloatSemantics::subtract_toward_positive(format, &operands[0], &operands[1]),
                false,
            )?,
            "subtract_toward_negative" => float_value(
                FloatSemantics::subtract_toward_negative(format, &operands[0], &operands[1]),
                false,
            )?,
            "multiply_toward_zero" => float_value(
                FloatSemantics::multiply_toward_zero(format, &operands[0], &operands[1]),
                false,
            )?,
            "multiply_toward_positive" => float_value(
                FloatSemantics::multiply_toward_positive(format, &operands[0], &operands[1]),
                false,
            )?,
            "multiply_toward_negative" => float_value(
                FloatSemantics::multiply_toward_negative(format, &operands[0], &operands[1]),
                false,
            )?,
            "divide_toward_zero" => float_value(
                FloatSemantics::divide_toward_zero(format, &operands[0], &operands[1]),
                true,
            )?,
            "divide_toward_positive" => float_value(
                FloatSemantics::divide_toward_positive(format, &operands[0], &operands[1]),
                true,
            )?,
            "divide_toward_negative" => float_value(
                FloatSemantics::divide_toward_negative(format, &operands[0], &operands[1]),
                true,
            )?,
            "square_root_toward_zero" => float_value(
                FloatSemantics::square_root_toward_zero(format, &operands[0]),
                false,
            )?,
            "square_root_toward_positive" => float_value(
                FloatSemantics::square_root_toward_positive(format, &operands[0]),
                false,
            )?,
            "square_root_toward_negative" => float_value(
                FloatSemantics::square_root_toward_negative(format, &operands[0]),
                false,
            )?,
            "fused_multiply_add_toward_zero" => float_value(
                FloatSemantics::fused_multiply_add_toward_zero(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                ),
                false,
            )?,
            "fused_multiply_add_toward_positive" => float_value(
                FloatSemantics::fused_multiply_add_toward_positive(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                ),
                false,
            )?,
            "fused_multiply_add_toward_negative" => float_value(
                FloatSemantics::fused_multiply_add_toward_negative(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                ),
                false,
            )?,
            _ => return Ok(None),
        };

        // The named F32/F64 requirements are checked boundary contracts with a
        // hermetic intrinsic provider, so semantic evaluation may consume them.
        // Compatibility imports still represent an actual host boundary and
        // must trip the dynamic build-time purity backstop.
        if compatibility_call {
            self.host_boundary_touched = true;
            self.non_fs_host_boundary_touched = true;
        }
        Ok(Some(value))
    }
    /// A unary float builtin's operand: the argument list must name
    /// exactly one expression, whose scalar type picks the semantic
    /// format. `None` defers the call to the next builtin candidate.
    fn unary_float_argument(
        &mut self,
        call: &typed_trees::expression::TableCallExpression,
        frame: &mut Frame,
    ) -> Result<Option<(Value, SemanticFloatFormat)>, Halt> {
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec();
        if arguments.len() != 1 {
            return Ok(None);
        }
        let format = if matches!(
            self.expression_scalar_type(arguments[0], frame),
            Some((PrimitiveType::F32, _))
        ) {
            SemanticFloatFormat::BINARY32
        } else {
            SemanticFloatFormat::BINARY64
        };
        Ok(Some((self.eval_expression(arguments[0], frame)?, format)))
    }
}

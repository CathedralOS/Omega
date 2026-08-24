use super::*;

impl<'program> Evaluator<'program> {
    pub(super) fn eval_expression(
        &mut self,
        handle: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Value> {
        self.tick()?;
        let node = self.program.expression_table.expression(handle).clone();
        if matches!(
            &node,
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        ) && let Some(value) = self.read_mutable_record_recast_target(handle, frame)?
        {
            return Ok(value);
        }
        match node {
            ExpressionNode::Atomic(atomic) => self.eval_expression(atomic.value, frame),
            ExpressionNode::Integer(value) => match value.bits_u64() {
                // Value::Int carries the 8-byte two's-complement pattern; u64
                // semantics ride the bits. The literal-width gate guarantees an
                // oversize literal only reaches u64-classed positions, so the
                // bit-cast is the value there -- refuse anything wider.
                Some(bits) => Ok(Value::Int(bits as i64)),
                None => unsupported(format!(
                    "integer literal `{value}` exceeds the interpreter's 8-byte value width"
                )),
            },
            ExpressionNode::Boolean(value) => Ok(Value::Bool(value)),
            // The LANDED read (F2a): an f32-suffixed literal means its
            // correctly-rounded f32 value everywhere -- widened exactly to the
            // carrier f64. Keyed on the landing, identically to the native
            // literal reads (f32_bits), so the engines stay bit-for-bit.
            ExpressionNode::Float(value) => Ok(Value::Float(value.landed_f64())),
            ExpressionNode::String(value) => Ok(Value::bytes(value.to_vec())),
            ExpressionNode::Name(path) => self.eval_name(&path, frame),
            ExpressionNode::Member(member) => {
                // A member on a PLACE receiver reads through its storage cell,
                // preserving aliasing. An inline NON-place receiver -- e.g. `.len`
                // on a subslice literal `(arr[a..b]).len`, whose receiver is a VIEW,
                // not a storage location -- has no place; evaluate the receiver to a
                // value and read the field off it. (A subslice BOUND to a local is a
                // place and takes the fast path.) Without this fallback the
                // receiver's range index reached the `Range` arm below and tripped
                // "range expression outside index position", diverging from the
                // native fold of `(arr[a..b]).len`.
                match self.resolve_place(handle, frame) {
                    Ok(cell) => Ok(cell.borrow().clone()),
                    Err(_) => {
                        let receiver = self.eval_expression(member.receiver, frame)?;
                        let field = self.field_cell(&receiver.cell(), member.member.as_str())?;
                        Ok(self.deref_cell(field).borrow().clone())
                    }
                }
            }
            ExpressionNode::Borrow(inner) => {
                // `&mut place as &mut View` can survive typed-tree rewriting as
                // `Mutable(Cast(RecastMutable))` in a guard or forwarded value.
                // In value position evaluate the recast view itself; argument
                // and assignment seams separately preserve its mutable alias.
                if matches!(
                    self.program.expression_table.expression(inner.target),
                    ExpressionNode::Cast(cast) if cast.form.is_recast()
                ) {
                    return self.eval_expression(inner.target, frame);
                }
                let cell = self.resolve_place(inner.target, frame)?;
                // Re-borrow collapse: see eval_argument's Mutable arm.
                let target = match &*cell.borrow() {
                    Value::Ref(target) => Rc::clone(target),
                    _ => Rc::clone(&cell),
                };
                Ok(Value::Ref(target))
            }
            ExpressionNode::Unary(unary) => {
                let operand = self.eval_expression(unary.operand, frame)?;
                self.eval_unary(unary.operator, operand)
            }
            ExpressionNode::Binary(binary) => {
                if let Some(value) = self.eval_selected_trait_operator(handle, &binary, frame)? {
                    return Ok(value);
                }
                let left = self.eval_expression(binary.left, frame)?;
                // `&&`/`||` SHORT-CIRCUIT: synthesized structural equality
                // (Equatable) guards each sum arm's payload reads behind tag
                // compares, so the right operand must not evaluate when the
                // left already decides -- a cross-case payload read would
                // trap here while the native backend's eager read of
                // in-allocation payload bytes is masked by the false tag.
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    let decided = left
                        .as_bool()
                        .ok_or_else(|| Halt::Trap("logical operand not boolean".to_owned()))?;
                    if (binary.operator == BinaryOperator::And) != decided {
                        // false && _  /  true || _
                        return Ok(Value::Bool(decided));
                    }
                    let right = self.eval_expression(binary.right, frame)?;
                    return right
                        .as_bool()
                        .map(Value::Bool)
                        .ok_or_else(|| Halt::Trap("logical operand not boolean".to_owned()));
                }
                let right = self.eval_expression(binary.right, frame)?;
                let unsigned_operands = matches!(
                    binary.operator,
                    BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                        | BinaryOperator::Divide
                        | BinaryOperator::Modulo
                        | BinaryOperator::ShiftRight
                ) && (self.expression_is_unsigned64(binary.left, frame)
                    || self.expression_is_unsigned64(binary.right, frame));
                // Non-Exact ADD/SUB/MUL apply their domain at the OPERATION
                // node (native emits the clamping/trapping/wrapping-width
                // sequence itself), signed DIV/MOD resolve the MIN/-1
                // corner there, and Wrapping SHIFTS need the type WIDTH for
                // their at/above-width count semantics (modular zero /
                // sign-fill), so resolve the expression's declared scalar
                // type for the operators the domains cover.
                let scalar_type = if matches!(
                    binary.operator,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                        | BinaryOperator::Modulo
                        | BinaryOperator::ShiftLeft
                        | BinaryOperator::ShiftRight
                ) {
                    self.expression_scalar_type(handle, frame)
                } else {
                    None
                };
                let named_float_operation = self.named_float_operation_name(handle);
                self.eval_binary(
                    binary.operator,
                    left,
                    right,
                    unsigned_operands,
                    scalar_type,
                    named_float_operation,
                )
            }
            ExpressionNode::Call(call) => self.eval_call_expression(handle, &call, frame),
            ExpressionNode::Cast(cast) => {
                let target = self.cast_target_primitive(cast.target_type);
                // RUNG B interior recast (`&self.buf[4] as &u32`): assemble the
                // target's bytes LITTLE-ENDIAN from the byte region starting at
                // the indexed element (the judged class guarantees a literal
                // in-bounds offset over a `[u8; N]` place).
                if cast.form.is_recast()
                    && let Some(assembled) = self.eval_interior_recast(&cast, target, frame)?
                {
                    return Ok(assembled);
                }
                let value = self.eval_expression(cast.value, frame)?;
                if cast.form.is_recast() {
                    if target.is_none()
                        && let Some(source_type) = self.expression_type_reference(cast.value, frame)
                        && self
                            .record_view_type_layout(source_type, &mut HashSet::new())
                            .is_some()
                    {
                        let source = value.clone().cell();
                        let cells = self.snapshot_typed_value_bytes(&source, source_type)?;
                        if let Some(value) =
                            self.assemble_record_view_type(cast.target_type, &cells, 0)?
                        {
                            return Ok(value);
                        }
                    }
                    return self.eval_recast_to_type(value, cast.target_type);
                }
                let source = self
                    .expression_type_reference(cast.value, frame)
                    .and_then(|source| self.program.primitive_type_reference(source));
                self.eval_cast(value, source, target, cast.domain)
            }
            ExpressionNode::Indexed(indexed) => {
                // A range index `arr[start..end]` produces a SUBSLICE view sharing the
                // collection's element cells; a scalar index reads one element.
                if let ExpressionNode::Range(range) = self
                    .program
                    .expression_table
                    .expression(indexed.index)
                    .clone()
                {
                    return self.eval_subslice(indexed.collection, &range, frame);
                }
                // A scalar index into a string VIEW (`Value::Str`) reads the i-th BYTE as an Int
                // -- this is how the oracle cross-checks byte-string canaries (hashing,
                // comparison, byte walks) instead of skipping them as "cannot index Str". A
                // carrier `[u8; N]` is a `Value::Array` and takes the element path below. READ
                // ONLY: a write `s[i] = x` still traps via element_cell (string views are
                // immutable), so there is no silent no-op.
                if let Ok(collection_cell) = self.resolve_place(indexed.collection, frame) {
                    let collection_cell = self.deref_cell(collection_cell);
                    let indexes_str = matches!(&*collection_cell.borrow(), Value::Str(_));
                    if indexes_str {
                        let index = self.eval_index(indexed.index, frame)?;
                        if let Value::Str(text) = &*collection_cell.borrow() {
                            return text
                                .borrow()
                                .get(index)
                                .map(|byte| Value::Int(i64::from(*byte)))
                                .ok_or_else(|| {
                                    Halt::Trap(format!("string index {index} out of bounds"))
                                });
                        }
                    }
                }
                let cell = self.resolve_place(handle, frame)?;
                let value = self.deref_cell(cell).borrow().clone();
                Ok(value)
            }
            ExpressionNode::ArrayLiteral(values) => {
                let mut elements = Vec::new();
                for value in self.program.expression_table.expression_handles(values) {
                    elements.push(self.eval_expression(*value, frame)?.cell());
                }
                Ok(Value::Array(elements))
            }
            // The frontend only produces a Range under an index expression (handled in the
            // Indexed arm above as a subslice); general/open ranges in value or argument
            // position are parse errors (probed in tests/coverage.rs). Decline defensively
            // in case a future frontend starts emitting them elsewhere.
            ExpressionNode::Range(_) => unsupported("range expression outside index position"),
            ExpressionNode::StructLiteral(literal) => self.eval_struct_literal(&literal, frame),
            ExpressionNode::ZeroValue(_) => {
                unsupported("proof-only zero_value<T>() reached runtime evaluation")
            }
        }
    }

    /// Execute a fixed token through the exact conformance row already
    /// selected in checked facts. No name or visible-conformance lookup is
    /// available here: the retained realization machine/state symbols are the
    /// complete dispatch authority.
    fn eval_selected_trait_operator(
        &mut self,
        expression: ExpressionHandle,
        binary: &psi_typed_trees::expression::TableBinaryExpression,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        let Some(candidate) = self
            .operator_facts
            .and_then(|facts| {
                facts.selected_trait_candidate_in_machine(expression, frame.machine_symbol)
            })
            .copied()
        else {
            return Ok(None);
        };
        let machine = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == candidate.realization_machine_symbol)
            .cloned()
            .ok_or_else(|| {
                Halt::Trap("selected trait operator lost its realization machine".to_owned())
            })?;
        let state = self
            .program
            .machine_states(&machine)
            .iter()
            .find(|state| state.symbol == candidate.realization_state_symbol)
            .cloned()
            .ok_or_else(|| {
                Halt::Trap("selected trait operator lost its realization state".to_owned())
            })?;
        let operands = [binary.left, binary.right];
        let has_self = self
            .program
            .state_parameters(&state)
            .iter()
            .any(|parameter| parameter.is_self);
        let instance = if has_self {
            let receiver = self.eval_argument(operands[0], frame)?;
            self.deref_cell(receiver)
        } else {
            Rc::clone(&frame.self_cell)
        };
        let mut arguments = Vec::with_capacity(operands.len() - usize::from(has_self));
        for operand in operands.into_iter().skip(usize::from(has_self)) {
            arguments.push(self.eval_state_argument(operand, frame)?);
        }

        let entered_guard_depth = self.guard_depth;
        self.guard_depth = 0;
        let value = self.run_state_collect(&machine, state.name.as_str(), instance, arguments);
        self.guard_depth = entered_guard_depth;
        value?
            .ok_or_else(|| {
                Halt::Trap("selected trait operator realization returned no value".to_owned())
            })
            .map(Some)
    }

    fn eval_call_expression(
        &mut self,
        handle: ExpressionHandle,
        call: &psi_typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Value> {
        // Builtins: max / min over two integer/float operands.
        let target = call.target.as_str();
        // CH10 root grant marker (see the statement-call twin): a no-op.
        if target.starts_with("accept_boundary#")
            || target == "select_provider"
            || target.starts_with("wire_compatibility#")
            || target.starts_with("bind_root#")
        {
            return Ok(Value::Unit);
        }
        // The tree walker has no architectural flags register. Preserve the
        // value-flow shape with the architecturally fixed RFLAGS bit 1 set;
        // the matching restore statement is a no-op above.
        if target == "asm#pushfq" && !call.receiver.is_valid() {
            return Ok(Value::Int(2));
        }
        if matches!(target, "max" | "min") {
            let args = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if args.len() == 2 {
                let left = self.eval_expression(args[0], frame)?;
                let right = self.eval_expression(args[1], frame)?;
                // A u64-classed operand selects the UNSIGNED min/max witness (the
                // same test the binary div/mod/shr path uses): `max(u64::MAX, 5)`
                // must pick u64::MAX, not the signed -1. Native lowers these to
                // MaxUnsigned/MinUnsigned for unsigned targets.
                let unsigned = self.expression_is_unsigned64(args[0], frame)
                    || self.expression_is_unsigned64(args[1], frame);
                return self.eval_min_max(target, left, right, unsigned);
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
            let args = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if args.len() == 3 {
                let format = if target.ends_with("_f32") {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                return self.eval_rewritten_ternary_float(&args, format, target, frame);
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
            let args = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if args.len() == 2 {
                let format = if target.ends_with("_f32") {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                return self.eval_rewritten_directed_binary(&args, format, target, frame);
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
            let args = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if args.len() == 1 {
                let format = if target.ends_with("_f32") {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                return self.eval_rewritten_directed_square_root(&args, format, target, frame);
            }
        }
        // Builtin: sqrt over a single float operand. The interpreter consumes
        // the same exact semantic function used as the native
        // sqrtsd/sqrtss contract oracle.
        if target == "sqrt" && call.receiver == ExpressionHandle::invalid() {
            let args = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if args.len() == 1 {
                let format = if matches!(
                    self.expression_scalar_type(args[0], frame),
                    Some((PrimitiveType::F32, _))
                ) {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                return match self.eval_expression(args[0], frame)? {
                    Value::Float(value) => {
                        let meaning = if format == SemanticFloatFormat::BINARY32 {
                            FloatMeaning::from_f32(value as f32)
                        } else {
                            FloatMeaning::from_f64(value)
                        };
                        Ok(Value::Float(
                            FloatSemantics::square_root(format, &meaning)
                                .to_interpreter_value(format),
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
            let args = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if args.len() == 1 {
                let format = if target == "float#classify_f32" {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                return match self.eval_expression(args[0], frame)? {
                    Value::Float(value) => {
                        let meaning = if format == SemanticFloatFormat::BINARY32 {
                            FloatMeaning::from_f32(value as f32)
                        } else {
                            FloatMeaning::from_f64(value)
                        };
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
                            variant_name: variant_name.to_owned(),
                            payload: negative
                                .map(|negative| {
                                    vec![("negative".to_owned(), Value::Bool(negative).cell())]
                                })
                                .unwrap_or_default(),
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
            let args = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            if args.len() == 1 {
                let format = if matches!(
                    self.expression_scalar_type(args[0], frame),
                    Some((PrimitiveType::F32, _))
                ) {
                    SemanticFloatFormat::BINARY32
                } else {
                    SemanticFloatFormat::BINARY64
                };
                return match self.eval_expression(args[0], frame)? {
                    Value::Float(value) => {
                        let meaning = if format == SemanticFloatFormat::BINARY32 {
                            FloatMeaning::from_f32(value as f32)
                        } else {
                            FloatMeaning::from_f64(value)
                        };
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

        // Slice/array view builtins on an array-valued receiver. `.as_slice()` /
        // `.as_mut_slice()` produce a slice that SHARES the array's element cells (so a
        // write through the slice aliases the array); `.len()` returns the element count.
        if matches!(target, "as_slice" | "as_mut_slice" | "len") && call.receiver.is_valid() {
            if let Ok(cell) = self.resolve_place(call.receiver, frame) {
                let cell = self.deref_cell(cell);
                let elements = match &*cell.borrow() {
                    Value::Array(elements) => Some(elements.clone()),
                    _ => None,
                };
                if let Some(elements) = elements {
                    return Ok(match target {
                        "len" => Value::Int(elements.len() as i64),
                        // A slice view shares the same element `Rc`s.
                        _ => Value::Array(elements),
                    });
                }
            }
        }

        // Borrowed text-view builtins are descriptor-preserving views. The
        // interpreter represents literal-backed and borrowed byte views with
        // the same shared `Value::Str` cell, mirroring the native `{ptr, len}`
        // descriptor copy. Returning a clone shares the bytes.
        if matches!(target, "as_view" | "bytes") && call.receiver.is_valid() {
            if let Ok(cell) = self.resolve_place(call.receiver, frame) {
                let cell = self.deref_cell(cell);
                let value = cell.borrow().clone();
                if matches!(value, Value::Str(_)) {
                    return Ok(value);
                }
            }
        }

        // A transition's guard subject evaluates ONCE per transition evaluation: the
        // parser lowers `transition self.f(x) { true -> a false -> b }` into one guard
        // per arm, each holding a COPY of the subject call (distinct handles, identical
        // structure). A later arm reuses the earlier arm's result instead of re-running
        // the callee's side effects -- matching the native lowering's shared prelude.
        if self.guard_depth > 0 {
            let memo = frame.guard_call_results.borrow();
            for (seen, value) in memo.iter() {
                if self
                    .program
                    .expression_table
                    .expressions_structurally_equal(*seen, handle)
                {
                    return Ok(value.clone());
                }
            }
        }

        // Resolve the value-call. A bare-self receiver naming a SIBLING state of the
        // current machine runs that state; a receiver expression resolving to a contained
        // sub-machine instance runs on that instance; otherwise a free helper machine.
        let (machine, entry_state, instance) = match self
            .resolve_value_call_target(call, target, frame)
        {
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
                    return Ok(self.read_stdin_byte_value());
                }
                if let Some(value) = self.virtual_time_host_value(target) {
                    self.host_boundary_touched = true;
                    self.non_fs_host_boundary_touched = true;
                    return Ok(value);
                }
                if target == "tick_count" {
                    self.virtual_ticks += 1;
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
        let mut args = Vec::new();
        for argument in self
            .program
            .expression_table
            .expression_handles(call.arguments)
        {
            args.push(self.eval_state_argument(*argument, frame)?);
        }
        // Suspend the guard flag while the callee RUNS: distinct same-shaped calls
        // inside its body are genuine repeat calls, not copies of one source
        // expression, and must not memoize against each other.
        let entered_guard_depth = self.guard_depth;
        self.guard_depth = 0;
        let value = self
            .run_state_collect(&machine, &entry_state, instance, args)
            .map(|value| value.unwrap_or(Value::Unit));
        self.guard_depth = entered_guard_depth;
        let value = value?;
        if entered_guard_depth > 0 {
            frame
                .guard_call_results
                .borrow_mut()
                .push((handle, value.clone()));
        }
        Ok(value)
    }

    fn try_float_boundary_value_call(
        &mut self,
        target: &str,
        call: &psi_typed_trees::expression::TableCallExpression,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        if !call.receiver.is_valid() {
            return Ok(None);
        }

        let core_format =
            psi_typed_trees::operator::resolve_named_expression_call(self.program, call).and_then(
                |operator| {
                    self.program
                        .operator_path_members(operator.name)
                        .first()
                        .and_then(|namespace| match namespace.as_str() {
                            "F32" => Some(SemanticFloatFormat::BINARY32),
                            "F64" => Some(SemanticFloatFormat::BINARY64),
                            _ => None,
                        })
                },
            );
        let compatibility_call =
            core_format.is_none() && matches!(target, "square_root" | "fused_multiply_add");
        if core_format.is_none() && !compatibility_call {
            return Ok(None);
        }

        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec();
        let expected_arity = match target {
            "negate"
            | "square_root"
            | "classify"
            | "is_finite"
            | "is_nan"
            | "is_infinite"
            | "is_normal"
            | "is_subnormal"
            | "square_root_toward_zero"
            | "square_root_toward_positive"
            | "square_root_toward_negative" => 1,
            "minimum"
            | "maximum"
            | "add_toward_zero"
            | "add_toward_positive"
            | "add_toward_negative"
            | "subtract_toward_zero"
            | "subtract_toward_positive"
            | "subtract_toward_negative"
            | "multiply_toward_zero"
            | "multiply_toward_positive"
            | "multiply_toward_negative"
            | "divide_toward_zero"
            | "divide_toward_positive"
            | "divide_toward_negative" => 2,
            "multiply_then_add"
            | "fused_multiply_add"
            | "fused_multiply_add_toward_zero"
            | "fused_multiply_add_toward_positive"
            | "fused_multiply_add_toward_negative" => 3,
            _ => return Ok(None),
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
            let Value::Float(value) = value else {
                return Err(Halt::Trap(format!(
                    "{target} expects float arguments, got {value:?}"
                )));
            };
            operands.push(if format == SemanticFloatFormat::BINARY32 {
                FloatSemantics::convert(format, &FloatMeaning::from_f64(value))
            } else {
                FloatMeaning::from_f64(value)
            });
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
                            match trap_class {
                                FloatPolicyTrap::NaNResult => "NaN",
                                FloatPolicyTrap::InfinityResult => "infinity",
                            }
                        ))
                    })?,
                ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => meaning,
            };
            Ok(Value::Float(meaning.to_interpreter_value(format)))
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
                    variant_name: variant_name.to_owned(),
                    payload: negative
                        .map(|negative| vec![("negative".to_owned(), Value::Bool(negative).cell())])
                        .unwrap_or_default(),
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

    fn resolve_value_call_target(
        &mut self,
        call: &psi_typed_trees::expression::TableCallExpression,
        target: &str,
        frame: &Frame,
    ) -> EvalResult<(Machine, String, Cell)> {
        // Static-machine specialization rewrites a parameter call to the exact
        // selected ENTRY symbol.  Its human-facing target remains the authored
        // leaf (`TotalOrder`, not `F64::TotalOrder`), so name lookup alone is
        // insufficient for attached or plural satisfiers.  Receiverless calls
        // can dispatch directly through that resolved symbol; receiver calls
        // still need the runtime instance logic below.
        if !call.receiver.is_valid()
            && let Some(resolved) = self.resolve_entry_state_symbol(call.target_symbol, frame)
        {
            return Ok(resolved);
        }

        // Whether this call is on `self` (or receiverless). A NON-self receiver
        // (`self.host.create(..)`) must resolve on the RECEIVER's type, never on
        // a same-named sibling state of the current machine -- else a wrapper
        // machine `Filesystem::create` calling `self.host.create` would recurse
        // into itself. Mirrors the validator's receiver-typed resolution fix.
        let receiver_is_self = if !call.receiver.is_valid() {
            true
        } else {
            match self.program.expression_table.expression(call.receiver) {
                psi_typed_trees::expression::ExpressionNode::Name(path) => {
                    let members = self
                        .program
                        .expression_table
                        .name_path_members(path.members);
                    members.is_empty() || (members.len() == 1 && members[0].as_str() == "self")
                }
                _ => false,
            }
        };

        // (1) Receiver expression resolving to a contained sub-machine / data instance
        // (e.g. `s.code()` where `s: &mut Circle`): run on that instance's machine.
        if call.receiver.is_valid() {
            if let Ok(cell) = self.resolve_place(call.receiver, frame) {
                let cell = self.deref_cell(cell);
                let is_self = Rc::ptr_eq(&cell, &frame.self_cell);
                if !is_self {
                    if let Some(machine) = self.machine_for_instance_state(&cell, target) {
                        return Ok((machine, target.to_owned(), cell));
                    }
                }
            }
        }

        // (1b) TYPE-qualified receiverless call (`Duration::from_milliseconds(x)`):
        // the "receiver" names a type GROUP, not a place, and the callee is the
        // group-qualified machine `<Type>::<target>`. Such a machine takes no
        // receiver (a pure constructor/helper), so the caller's self cell rides
        // along untouched -- same as the free-machine arm (3).
        if call.receiver.is_valid() {
            if let psi_typed_trees::expression::ExpressionNode::Name(path) =
                self.program.expression_table.expression(call.receiver)
            {
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
                if members.len() == 1
                    && members[0].as_str() != "self"
                    && frame.get(members[0].as_str()).is_none()
                {
                    let group = members[0].as_str();
                    if let Some(machine) = self
                        .program
                        .machines()
                        .iter()
                        .find(|machine| {
                            let mut segments = machine.name.as_str().split("::");
                            segments.next() == Some(group)
                                && machine.name.as_str().ends_with(target)
                                && self.find_state(machine, target).is_some()
                        })
                        .cloned()
                    {
                        return Ok((machine, target.to_owned(), Rc::clone(&frame.self_cell)));
                    }
                }
            }
        }

        // (2) Sibling state of the current machine -- ONLY for self/receiverless
        // calls (a non-self receiver was handled by (1) or falls to the host
        // fallback below).
        if receiver_is_self {
            if let Some(machine) = self.current_machine(frame) {
                if self.find_state(machine, target).is_some() {
                    return Ok((
                        machine.clone(),
                        target.to_owned(),
                        Rc::clone(&frame.self_cell),
                    ));
                }
            }
        }

        // (3) A free helper machine (self/receiverless calls only).
        let machine = self
            .find_machine_for_call(target, frame)
            .filter(|_| receiver_is_self)
            .ok_or_else(|| Halt::Unsupported(format!("unknown value-call target `{target}`")))?;
        let entry_state = self
            .machine_entry_state_name(&machine)
            .ok_or_else(|| Halt::Unsupported(format!("value-call `{target}` has no state")))?;
        Ok((machine, entry_state, Rc::clone(&frame.self_cell)))
    }

    /// Construct a `data` value from a struct literal `Type { field: value, .. }`. Fields
    /// not named take the type's default; named fields override. A case literal
    /// (`Command::Say { text: ... }`) constructs an Enum value instead: the case name is
    /// the tag and the named payload fields fill the case's declared payload.
    fn eval_struct_literal(
        &mut self,
        literal: &psi_typed_trees::expression::TableStructLiteral,
        frame: &Frame,
    ) -> EvalResult<Value> {
        if let Some(case_name) = &literal.case_name {
            return self.eval_case_literal(literal, case_name.as_str(), frame);
        }
        let type_name = literal.type_name.as_str().to_owned();
        let data = self.find_data_by_name(&type_name);
        let (type_symbol, mut fields) = if let Some(data) = data {
            let mut fields = BTreeMap::new();
            self.populate_data_fields(data, &mut fields)?;
            (data.symbol, fields)
        } else {
            (SymbolHandle::invalid(), BTreeMap::new())
        };
        for field in self.program.expression_table.struct_fields(literal.fields) {
            let value = self.eval_expression(field.value, frame)?;
            // Coerce the field value to the field's declared width/domain, matching
            // the native store into the field slot (`Point { x: a+b }` with `a+b`
            // = 300 into a u8 field reads 44). The field type carries its own
            // domain, so resolve it directly.
            let value = match self.field_type_reference(type_symbol, field.name.as_str()) {
                Some(type_reference) => self.coerce_scalar_value(value, type_reference)?,
                None => value,
            };
            fields.insert(field.name.as_str().to_owned(), value.cell());
        }
        Ok(Value::Struct {
            type_symbol,
            type_name,
            fields,
        })
    }

    /// Construct a payload-carrying case value `Type::Case { field: value, .. }`. Payload
    /// cells follow the case's DECLARED field order; unnamed payload fields default, named
    /// literal fields override, and a literal field that is not part of the case's payload
    /// traps.
    fn eval_case_literal(
        &mut self,
        literal: &psi_typed_trees::expression::TableStructLiteral,
        case_name: &str,
        frame: &Frame,
    ) -> EvalResult<Value> {
        let type_name = literal.type_name.as_str();
        let Some(data) = self.find_data_by_name(type_name) else {
            return trap(format!("unknown data type `{type_name}` in case literal"));
        };
        let Some(variant) =
            self.program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name => {
                        Some(variant)
                    }
                    _ => None,
                })
        else {
            return trap(format!("`{type_name}` has no case `{case_name}`"));
        };

        let mut payload = Vec::new();
        // MIXED shapes: the COMMON fields exist in every case and come first.
        // Case construction ZERO-initializes them (frozen decision 7's rule;
        // never the declared default -- validation rejects defaults on mixed
        // common fields), unless the literal names them below.
        for member in self.program.data_members(data) {
            let DataMember::Field(common_field) = member else {
                continue;
            };
            let name = common_field.name.as_str().to_owned();
            let value = self.default_value_for_type(common_field.type_reference)?;
            payload.push((name, value.cell()));
        }
        for field in self.program.data_payload_fields(variant) {
            let name = field.name.as_str().to_owned();
            let value = self.default_value_for_type(field.type_reference)?;
            payload.push((name, value.cell()));
        }
        for field in self.program.expression_table.struct_fields(literal.fields) {
            let value = self.eval_expression(field.value, frame)?;
            let Some(slot) = payload
                .iter_mut()
                .find(|(name, _)| name == field.name.as_str())
            else {
                return trap(format!(
                    "case `{type_name}::{case_name}` has no payload field `{}`",
                    field.name.as_str()
                ));
            };
            slot.1 = value.cell();
        }

        Ok(Value::Enum {
            type_symbol: data.symbol,
            variant_name: case_name.to_owned(),
            payload,
        })
    }

    fn eval_call_expression_argument(
        &mut self,
        argument: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Cell> {
        // Share the same argument-evaluation rules as state calls (incl. reference
        // forwarding for bare-place args that already hold a `&mut`).
        self.eval_argument(argument, frame)
    }
}

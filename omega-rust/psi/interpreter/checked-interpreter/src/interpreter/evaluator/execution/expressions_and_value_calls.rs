use crate::interpreter::evaluator::{
    ArithmeticDomain, BinaryOperator, EvalResult, Evaluator, ExpressionHandle, ExpressionNode,
    Frame, Halt, HashSet, PrimitiveType, Reference, Value, trap, unsupported,
};
mod call_evaluation;
mod literal_evaluation;
mod match_dispatch;

impl<'program> Evaluator<'program> {
    pub(in crate::interpreter::evaluator) fn eval_expression(
        &mut self,
        handle: ExpressionHandle,
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        let outermost = frame.scalar_expression_depth == 0;
        if outermost {
            frame.scalar_expression_values.clear();
        }
        if let Some((_, value)) = frame
            .scalar_expression_values
            .iter()
            .find(|(expression, _)| *expression == handle)
        {
            return Ok(value.clone());
        }
        frame.scalar_expression_depth += 1;
        let result = self.eval_expression_node(handle, frame);
        frame.scalar_expression_depth -= 1;
        if let Ok(
            value @ (Value::Int(_) | Value::Bool(_) | Value::Float(_) | Value::UndeterminedNaN(_)),
        ) = &result
        {
            frame.scalar_expression_values.push((handle, value.clone()));
        }
        if outermost {
            frame.scalar_expression_values.clear();
        }
        result
    }

    fn eval_expression_node(
        &mut self,
        handle: ExpressionHandle,
        frame: &mut Frame,
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
        // Checking normalizes a token use into its exact requirement call.
        // Both representations consume the already-selected sealed meaning;
        // no requirement body or provider name is searched at execution time.
        if let Some(selected) = self
            .selected_build_time_operators
            .iter()
            .find(|selected| {
                selected.expression == handle
                    && selected.origin.machine_symbol() == Some(frame.machine_symbol)
            })
            .copied()
        {
            let operands = match &node {
                ExpressionNode::Binary(binary) if binary.operator == selected.operation => {
                    [binary.left, binary.right]
                }
                ExpressionNode::Call(call) => {
                    let exact_target = call.target_symbol == selected.requirement
                        || self
                            .program
                            .machine_by_symbol(selected.requirement)
                            .is_some_and(|machine| {
                                let states = self.program.machine_states(machine);
                                states.len() == 1 && states[0].symbol == call.target_symbol
                            });
                    let [left, right] = self
                        .program
                        .expression_table
                        .expression_handles(call.arguments)
                    else {
                        return trap("selected binary requirement lost its exact arity");
                    };
                    if !exact_target
                        || call.receiver.is_valid()
                        || call.static_machine_parameter.is_valid()
                        || !call.machine_arguments.is_empty()
                        || !call.carries_only_positional_arguments()
                    {
                        return trap(
                            "selected binary requirement lost its exact target or arguments",
                        );
                    }
                    [*left, *right]
                }
                _ => return trap("selected binary execution lost its current operation"),
            };
            let unique = self
                .selected_build_time_operators
                .iter()
                .filter(|row| {
                    row.expression == handle
                        && row.origin.machine_symbol() == Some(frame.machine_symbol)
                })
                .count()
                == 1;
            let checked_selection = self.operator_facts.is_none_or(|facts| {
                let mut count = 0usize;
                facts.for_each_resolved_expression_selection(
                    handle,
                    |origin, requirement, policy, _| {
                        if origin == selected.origin
                            && requirement == selected.requirement
                            && policy == selected.policy
                        {
                            count += 1;
                        }
                    },
                );
                count == 1
            });
            if operands != selected.operands
                || !unique
                || selected.provider.is_empty()
                || !checked_selection
            {
                return trap(
                    "selected build-time execution lost its exact occurrence or checked selection",
                );
            }
            let primitive = match selected.format {
                numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
            };
            let left =
                self.eval_expression_with_destination(operands[0], Some(primitive), frame)?;
            let right =
                self.eval_expression_with_destination(operands[1], Some(primitive), frame)?;
            if !left.is_float() || !right.is_float() {
                return trap(format!(
                    "selected float execution at {handle:?} received nonfloat operands: {left:?}, {right:?}"
                ));
            }
            let domain = match selected.policy {
                checked_trees::CheckedArithmeticPolicyAdapter::None => ArithmeticDomain::Exact,
                checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
                    ..
                } => ArithmeticDomain::Saturating,
                checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
                    ..
                } => ArithmeticDomain::Trapping,
            };
            return self.eval_float_binary(
                selected.operation,
                &left,
                &right,
                Some((primitive, domain)),
                None,
            );
        }
        match node {
            ExpressionNode::Match(dispatch) => {
                self.with_selected_match(handle, &dispatch, frame, |evaluator, selected, frame| {
                    evaluator.eval_expression(selected, frame)
                })
            }
            ExpressionNode::SavedSubject(saved) => {
                let cell = frame
                    .match_subjects
                    .iter()
                    .rev()
                    .find(|(capture, _)| *capture == saved.capture)
                    .map(|(_, cell)| cell.clone())
                    .ok_or_else(|| {
                        Halt::Unsupported(
                            "saved subject has no active evaluated capture".to_owned(),
                        )
                    })?;
                let value = cell.borrow().clone();
                self.observe_reference_value(value)
            }
            ExpressionNode::Atomic(atomic) => {
                if atomic.result_custody.has_structural_result() {
                    return unsupported(
                        "single-attempt outcome requires its exact structural receiving type",
                    );
                }
                self.eval_expression(atomic.value, frame)
            }
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
            ExpressionNode::UndeterminedNaN(format) => Ok(Value::UndeterminedNaN(format)),
            ExpressionNode::String(value) => self.allocate_text(value.to_vec()),
            ExpressionNode::Name(path) => self.eval_name(&path, frame),
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                let field = self.eval_read_cell(handle, frame)?;
                let value = field.borrow().clone();
                self.observe_reference_value(value)
            }
            ExpressionNode::Borrow(inner) => {
                // `&mut place as &mut View` can survive typed-tree rewriting as
                // `Borrow(Cast(Recast(access)))` in a guard or forwarded value.
                // In value position evaluate the recast view itself; argument
                // and assignment seams separately preserve its mutable alias.
                if matches!(
                    self.program.expression_table.expression(inner.target),
                    ExpressionNode::Cast(cast) if cast.form.is_recast()
                ) {
                    return self.eval_expression(inner.target, frame);
                }
                let cell = self.eval_read_cell(inner.target, frame)?;
                // Re-borrow collapse: see eval_argument's Mutable arm.
                let target = match &*cell.borrow() {
                    Value::Ref(target) => target.clone(),
                    _ => Reference::new(cell.clone()),
                };
                Ok(Value::Ref(target))
            }
            ExpressionNode::Unary(unary) => {
                let operand = self.eval_expression(unary.operand, frame)?;
                self.eval_unary(unary.operator, operand)
            }
            ExpressionNode::Binary(binary) => {
                if binary.operator == BinaryOperator::CaseMembership {
                    return self.eval_case_membership(handle, &binary, frame);
                }

                if let Some(value) = self.eval_selected_trait_operator(handle, &binary, frame)? {
                    return Ok(value);
                }
                let left_type = self
                    .expression_scalar_type(binary.left, frame)
                    .map(|(primitive, _)| primitive);
                let right_type = self
                    .expression_scalar_type(binary.right, frame)
                    .map(|(primitive, _)| primitive);
                let comparison = matches!(
                    binary.operator,
                    BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                        | BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                );
                let destination = left_type.or(right_type);
                let left =
                    self.eval_expression_with_destination(binary.left, destination, frame)?;
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
                let right = self.eval_expression_with_destination(
                    binary.right,
                    if comparison {
                        right_type.or(left_type)
                    } else {
                        destination
                    },
                    frame,
                )?;
                // Membership observes the already evaluated borrow's tag.
                // Ordinary equality keeps reference operands for its selected
                // operator; do not turn this into general implicit dereferencing.
                let left = if matches!(left, Value::Ref(_))
                    && self.current_machine(frame).is_some_and(|machine| {
                        self.program
                            .state_in_machine(machine, frame.state_symbol)
                            .is_some_and(|state| {
                                validation::has_exact_case_membership_meaning(
                                    self.program,
                                    machine,
                                    Some(state),
                                    handle,
                                    &binary,
                                )
                            })
                    }) {
                    match left {
                        Value::Ref(reference) => self.read_reference(&reference)?,
                        value => value,
                    }
                } else {
                    left
                };
                if comparison && let (Value::Int(left), Value::Int(right)) = (&left, &right) {
                    return self.eval_integer_comparison(
                        binary.operator,
                        *left,
                        left_type.or(right_type).unwrap_or(PrimitiveType::I64),
                        *right,
                        right_type.or(left_type).unwrap_or(PrimitiveType::I64),
                    );
                }
                // A shift count has its own carrier. Its signedness cannot
                // choose logical versus arithmetic shifting of the value.
                let unsigned_operands = if binary.operator == BinaryOperator::ShiftRight {
                    self.expression_is_unsigned64(binary.left, frame)
                } else {
                    matches!(
                        binary.operator,
                        BinaryOperator::Less
                            | BinaryOperator::LessOrEqual
                            | BinaryOperator::Greater
                            | BinaryOperator::GreaterOrEqual
                            | BinaryOperator::Divide
                            | BinaryOperator::Modulo
                    ) && (self.expression_is_unsigned64(binary.left, frame)
                        || self.expression_is_unsigned64(binary.right, frame))
                };
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
                let (value, source) = if cast.form.is_recast() {
                    (self.eval_expression(cast.value, frame)?, None)
                } else {
                    self.eval_expression_with_destination_and_carrier(cast.value, target, frame)?
                };
                if cast.form.is_recast() {
                    if target.is_none()
                        && let Some(source_type) = self.expression_type_reference(cast.value, frame)
                        && self
                            .record_view_type_layout(source_type, &mut HashSet::new())
                            .is_some()
                    {
                        let source = self.allocate_cell(value.clone())?;
                        let cells = self.snapshot_typed_value_bytes(&source, source_type)?;
                        if let Some(value) =
                            self.assemble_record_view_type(cast.target_type, &cells, 0)?
                        {
                            return Ok(value);
                        }
                    }
                    return self.eval_recast_to_type(value, cast.target_type);
                }
                self.eval_cast(value, source, target)
            }
            ExpressionNode::ArrayLiteral(values) => {
                let mut elements = Vec::new();
                for value in self.program.expression_table.expression_handles(values) {
                    let element_type = self
                        .expression_type_reference(*value, frame)
                        .unwrap_or_default();
                    let value = self.eval_expression(*value, frame)?;
                    let value = self.capture_value_operand(value, element_type)?;
                    elements.push(self.allocate_cell(value)?);
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

    /// Observe a nominal tag without constructing the classifier as a value.
    fn eval_case_membership(
        &mut self,
        expression: ExpressionHandle,
        binary: &typed_trees::expression::TableBinaryExpression,
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        if self
            .selected_build_time_operators
            .iter()
            .any(|selected| selected.expression == expression)
            || self.operator_facts.is_some_and(|facts| {
                facts
                    .uses
                    .iter()
                    .any(|(_, selected)| selected.expression == expression)
            })
        {
            return trap("case membership cannot carry selected value-operator execution");
        }
        let table = &self.program.expression_table;
        if !table.expression_is_valid(binary.left) || !table.expression_is_valid(binary.right) {
            return trap("case membership has an invalid operand");
        }
        let Some(classifier) = table.name_path_node(binary.right) else {
            return trap("case membership requires an exact case classifier");
        };
        let case_symbol = classifier.symbol;
        let Some(owner) = validation::exact_case_reference_owner(self.program, binary.right) else {
            return trap("case membership requires an exact case classifier");
        };
        let owner_symbol = owner.symbol;
        let subject = self.eval_expression(binary.left, frame)?;
        let subject = match subject {
            Value::Ref(reference) => self.read_reference(&reference)?,
            value => value,
        };
        let Value::Enum {
            type_symbol,
            variant_name,
            ..
        } = subject
        else {
            return trap("case membership subject is not a nominal sum value");
        };
        if type_symbol != owner_symbol {
            return trap("case membership subject has a different nominal owner");
        }
        let Some(runtime_case) =
            self.program
                .data_members(owner)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant)
                        if *variant.name.as_str() == *variant_name =>
                    {
                        Some(variant.symbol)
                    }
                    _ => None,
                })
        else {
            return trap("case membership subject has no declared runtime case");
        };
        Ok(Value::Bool(runtime_case == case_symbol))
    }

    /// Execute a fixed token through the exact conformance row already
    /// selected in checked facts. No name or visible-conformance lookup is
    /// available here: the retained realization machine/state symbols are the
    /// complete dispatch authority.
    fn eval_selected_trait_operator(
        &mut self,
        expression: ExpressionHandle,
        binary: &typed_trees::expression::TableBinaryExpression,
        frame: &mut Frame,
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
            .machine(candidate.realization_machine_symbol)
            .ok_or_else(|| {
                Halt::Trap("selected trait operator lost its realization machine".to_owned())
            })?;
        let state = self
            .program
            .state_in_machine(machine, candidate.realization_state_symbol)
            .ok_or_else(|| {
                Halt::Trap("selected trait operator lost its realization state".to_owned())
            })?;
        let operands = [binary.left, binary.right];
        let arguments = self.eval_state_arguments(state, &operands, frame)?;

        let entered_guard_depth = self.guard_depth;
        self.guard_depth = 0;
        let value = self.run_state_collect(machine, state, arguments);
        self.guard_depth = entered_guard_depth;
        value?
            .ok_or_else(|| {
                Halt::Trap("selected trait operator realization returned no value".to_owned())
            })
            .map(Some)
    }
}

/// Flatten an evaluated byte-sequence operand to its live bytes: a text
/// buffer, a byte-array window, or a reference to either.
pub(super) fn byte_sequence_operand_bytes(value: &Value) -> EvalResult<Vec<u8>> {
    match value {
        Value::Str(text) => Ok(text.borrow().to_vec()),
        Value::Array(elements) => elements
            .iter()
            .map(|cell| {
                let value = cell.borrow();
                value
                    .as_int()
                    .and_then(|byte| u8::try_from(byte).ok())
                    .ok_or_else(|| {
                        Halt::Trap(
                            "byte-sequence predicate operand holds a non-byte element".to_owned(),
                        )
                    })
            })
            .collect(),
        Value::Ref(reference) if reference.storage.is_none() => {
            byte_sequence_operand_bytes(&reference.cell.borrow())
        }
        _ => Err(Halt::Trap(
            "byte-sequence predicate operand is not a byte sequence".to_owned(),
        )),
    }
}

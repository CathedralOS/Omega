/*
    Expression evaluation, value-call invocation, match subject bindings,
    literals, and layout target queries for the checked-tree interpreter.

    We evaluate expression DAGs, memoize transition-guard subject calls per
    state pass, and dispatch value-returning machine and trait calls.
*/

use crate::interpreter::UnavailableStaticInput::TypePredicate;
use crate::interpreter::evaluator::{
    ArithmeticDomain, BinaryOperator, EvalResult, Evaluator, ExpressionHandle, ExpressionNode,
    Frame, Halt, HashSet, PrimitiveType, Reference, Value, expression_table, trap,
    type_reference_table, unsupported,
};
use checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly;
use checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite;
use typed_trees::references::collect_type_symbol_dependencies;

impl<'program> Evaluator<'program> {
    pub(crate) fn eval_expression(
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
        if let Some((left, right, _)) = self.program.static_type_predicate_operands(handle) {
            return self
                .program
                .static_type_predicate_value(handle)
                .map(Value::Bool)
                .ok_or_else(|| {
                    let input = [left, right].into_iter().find_map(|reference| {
                        collect_type_symbol_dependencies(self.program, reference)
                            .into_iter()
                            .find(|symbol| {
                                symbol.is_valid()
                                    && self.program.symbols.get(*symbol).kind
                                        == symbols::SymbolKind::TypeParameter
                            })
                    });
                    match input {
                        Some(input) => Halt::StaticInputUnavailable(TypePredicate {
                            expression: handle,
                            input,
                        }),
                        None => Halt::Unsupported(
                            "closed static type predicate cannot be evaluated".to_owned(),
                        ),
                    }
                });
        }
        let node = self.program.expression_table.expression(handle).clone();
        if matches!(
            &node,
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        ) && let Some(value) = self.read_mutable_record_recast_target(handle, frame)?
        {
            return Ok(value);
        }
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
                    let [left, right] =
                        expression_table(self.program).expression_handles(call.arguments)
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
                || selected.provider.is_zero()
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
                FloatSaturatingOverflowOnly { .. } => ArithmeticDomain::Saturating,
                FloatTrappingNonFinite { .. } => ArithmeticDomain::Trapping,
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
                Some(bits) => Ok(Value::Int(bits as i64)),
                None => unsupported(format!(
                    "integer literal `{value}` exceeds the interpreter's 8-byte value width"
                )),
            },
            ExpressionNode::Boolean(value) => Ok(Value::Bool(value)),
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
                if matches!(
                    self.program.expression_table.expression(inner.target),
                    ExpressionNode::Cast(cast) if cast.form.is_recast()
                ) {
                    return self.eval_expression(inner.target, frame);
                }
                let cell = self.eval_read_cell(inner.target, frame)?;
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
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    let decided = left
                        .as_bool()
                        .ok_or_else(|| Halt::Trap("logical operand not boolean".to_owned()))?;
                    if (binary.operator == BinaryOperator::And) != decided {
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
                if let Some(selector) = self.try_dynamic_selector_cast(&cast, frame)? {
                    return Ok(selector);
                }
                let target = self.cast_target_primitive(cast.target_type);
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
            ExpressionNode::Range(_) => unsupported("range expression outside index position"),
            ExpressionNode::TypeExpression(_) => {
                unsupported("unresolved static type operand reached runtime evaluation")
            }
            ExpressionNode::StructLiteral(literal) => {
                self.eval_struct_literal(handle, &literal, frame)
            }
            ExpressionNode::ZeroValue(_) => {
                unsupported("proof-only zero_value<T>() reached runtime evaluation")
            }
        }
    }

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
        let Some(subject_type) = self.expression_type_reference(binary.left, frame) else {
            return trap("case membership subject has no exact receiving type");
        };
        let subject_type = type_reference_table(self.program).peeled_type_reference(subject_type);
        let Some(selected_owner) = self
            .program
            .closed_data_constructor_definition(subject_type, owner.symbol)
        else {
            return trap("case membership classifier has no exact subject application");
        };
        let Some(selected_case) = self
            .program
            .closed_data_constructor_case(subject_type, case_symbol)
        else {
            return trap("case membership classifier has no case in its subject application");
        };
        let owner_symbol = selected_owner.symbol;
        let case_symbol = selected_case.symbol;
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
                .data_members(selected_owner)
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
        let arguments = self.eval_state_arguments(state, &operands, None, frame)?;

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

pub(crate) fn byte_sequence_operand_bytes(value: &Value) -> EvalResult<Vec<u8>> {
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

pub(crate) mod call_evaluation {
    use super::byte_sequence_operand_bytes;
    use crate::interpreter::evaluator::{
        ArithmeticDomain, EvalResult, ExpressionHandle, FloatMeaning, FloatSemantics, Frame, Halt,
        PrimitiveType, SemanticFloatClass, SemanticFloatFormat, SymbolHandle, Value,
        expression_table, trap, unsupported,
    };
    use language_semantics::declaration_selection::CollectionMeasure;
    use numerics::float_semantics_catalog::FloatSemanticOperation;
    use symbols::BuiltinFunction;
    use typed_trees::byte_predicates::byte_predicate_call;

    impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
        fn guard_subject_value(
            &mut self,
            handle: ExpressionHandle,
            frame: &mut Frame,
            value: Value,
        ) -> Value {
            if self.guard_depth > 0 {
                frame.record_guard_call_result(handle, &value);
            }
            value
        }

        pub(crate) fn eval_call_expression(
            &mut self,
            handle: ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            if let Some(value) = self.try_source_selection_call(call, frame)? {
                return Ok(value);
            }
            if let Some(value) = self.try_layout_target_facts(call, frame)? {
                return Ok(self.guard_subject_value(handle, frame, value));
            }
            if call.native_code_layout.is_some() {
                return unsupported(
                    "late native code-layout metadata is unavailable to interpreted or same-product semantic constant, type, layout, or optimization evaluation",
                );
            }

            if self.guard_depth > 0
                && let Some(value) = frame.guard_call_result(self.program, handle)
            {
                return Ok(value);
            }

            if call.static_requirement_dispatch.is_some() {
                let value = self.run_selected_realization(
                    call.target_symbol,
                    expression_table(self.program).expression_handles(call.arguments),
                    Some(validation::CallSite::Expression {
                        expression: handle,
                        call,
                    }),
                    frame,
                )?;
                return Ok(self.guard_subject_value(handle, frame, value));
            }
            if let Some(slot) = self.program.dynamic_slot_by_symbol(call.target_symbol) {
                let value = self.run_dynamic_slot(
                    slot,
                    expression_table(self.program).expression_handles(call.arguments),
                    frame,
                )?;
                return Ok(self.guard_subject_value(handle, frame, value));
            }
            if let Some(value) = self.try_operator_application(
                call.target_symbol,
                handle,
                expression_table(self.program).expression_handles(call.arguments),
                frame,
            )? {
                return Ok(self.guard_subject_value(handle, frame, value));
            }

            if let Some((predicate, argument)) = byte_predicate_call(self.program, handle) {
                let value = self.eval_expression(argument, frame)?;
                let bytes = byte_sequence_operand_bytes(&value)?;
                let value = Value::Bool(predicate.holds_for(&bytes));
                return Ok(self.guard_subject_value(handle, frame, value));
            }

            if let Some(value) = self.try_build_facet_routes(
                call.target_symbol,
                expression_table(self.program).expression_handles(call.arguments),
                self.program.expression_table.source_span(handle),
                frame,
            )? {
                return Ok(self.guard_subject_value(handle, frame, value));
            }
            let target = call.target.as_str();
            let qualified = self.expression_receiver_is_namespace(call.receiver);
            if let Some(request) = &call.private_layout_operation {
                let arguments = expression_table(self.program).expression_handles(call.arguments);
                let [plan, placement] = arguments else {
                    return trap(
                        "compiler-known `LayoutPlan::place_private` requires one plan and one placement",
                    );
                };
                let plan = self.eval_expression(*plan, frame)?;
                let placement = self.eval_expression(*placement, frame)?;
                let placement =
                    crate::build_time::observe_build_time_value(&placement, &|reference| {
                        self.read_reference(reference)
                    })?;
                self.private_layout_placements
                    .push(crate::PrivateLayoutPlacementReceipt {
                        operation_expression: handle,
                        selected_slot: request.selected_slot.clone(),
                        placement,
                    });
                return Ok(plan);
            }
            if !call.target_symbol.is_valid()
            && matches!(
                language_semantics::declaration_selection::BuildOperation::from_call_target(target),
                Some(language_semantics::declaration_selection::BuildOperation::RepresentationSelection
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
            if let Some(value) = self.try_build_selection_value_call(handle, call, frame)? {
                return Ok(self.guard_subject_value(handle, frame, value));
            }
            if let Some(value) = self.try_behavior_exclusion_value_call(handle, call, frame)? {
                return Ok(self.guard_subject_value(handle, frame, value));
            }
            let builtin = self
                .program
                .symbols
                .builtin_function_for_symbol(call.target_symbol);
            if builtin == Some(BuiltinFunction::AsmSnapshotFlags) && !call.receiver.is_valid() {
                return unsupported(
                    "pushfq requires an explicitly supplied target flags environment in Terminal execution",
                );
            }
            if let Some(builtin @ (BuiltinFunction::FloatMaximum | BuiltinFunction::FloatMinimum)) =
                builtin
            {
                let arguments = expression_table(self.program)
                    .expression_handles(call.arguments)
                    .to_vec();
                if arguments.len() == 2 {
                    let left = self.eval_expression(arguments[0], frame)?;
                    let right = self.eval_expression(arguments[1], frame)?;
                    return self.eval_float_extremum(builtin, left, right);
                }
            }
            if call.receiver == ExpressionHandle::invalid()
                && let Some(value) = self.eval_named_float_intrinsic(
                    target,
                    expression_table(self.program).expression_handles(call.arguments),
                    frame,
                )?
            {
                return Ok(value);
            }
            if builtin == Some(BuiltinFunction::FloatSquareRoot)
                && call.receiver == ExpressionHandle::invalid()
                && let Some((value, format)) = self.unary_float_argument(call, frame)?
            {
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
            if matches!(target, "float#classify_f32" | "float#classify_f64")
                && call.receiver == ExpressionHandle::invalid()
            {
                let arguments = expression_table(self.program)
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
                                    SemanticFloatClass::Zero { negative } => {
                                        ("Zero", Some(negative))
                                    }
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

            if matches!(
                target,
                "float#is_nan"
                    | "float#is_finite"
                    | "float#is_infinite"
                    | "float#is_normal"
                    | "float#is_subnormal"
            ) && call.receiver == ExpressionHandle::invalid()
                && let Some((value, format)) = self.unary_float_argument(call, frame)?
            {
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

            if self.exact_console_intrinsic_host_method(call.target_symbol) == Some("read_byte") {
                self.host_boundary_touched = true;
                let value = self.read_stdin_byte_value(call.target_symbol)?;
                return Ok(self.guard_subject_value(handle, frame, value));
            }

            let (machine, entry_state) = match self.resolve_static_call(call.target_symbol, target)
            {
                Ok(resolution) => resolution,
                Err(halt) => {
                    if let Some(value) = self.try_float_boundary_value_call(target, call, frame)? {
                        return Ok(value);
                    }

                    if let Some(value) = self.try_host_value_call(call, frame)? {
                        return Ok(self.guard_subject_value(handle, frame, value));
                    }

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
                        self.host_boundary_touched = true;
                    }
                    if target == "read_byte" {
                        self.host_boundary_touched = true;
                        let value = self.read_stdin_byte_value(call.target_symbol)?;
                        return Ok(self.guard_subject_value(handle, frame, value));
                    }
                    if let Some(value) = self.virtual_time_host_value(target) {
                        self.host_boundary_touched = true;
                        return Ok(value);
                    }
                    if target == "tick_count" {
                        self.advance_virtual_clock(1)?;
                        return Ok(Value::Int(self.virtual_ticks));
                    }
                    if target == "key_state" {
                        return Ok(Value::Int(0));
                    }
                    if target == "dc_create" || target == "get_dc" {
                        return Ok(Value::Int(1));
                    }
                    if target == "window_create" {
                        self.virtual_window_next += 1;
                        self.virtual_live_windows.insert(self.virtual_window_next);
                        return Ok(Value::Int(self.virtual_window_next));
                    }
                    if target == "foreground_window" {
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
                        let Some(handle_argument) = expression_table(self.program)
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
                    if target == "msg_peek" || target == "msg_translate" || target == "msg_dispatch"
                    {
                        return Ok(Value::Int(0));
                    }
                    if target == "blit" {
                        let Some(height) = expression_table(self.program)
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
                expression_table(self.program).expression_handles(call.arguments),
                Some(validation::CallSite::Expression {
                    expression: handle,
                    call,
                }),
                frame,
            )?;
            let entered_guard_depth = self.guard_depth;
            self.guard_depth = 0;
            let value = self
                .run_state_collect(machine, entry_state, arguments)
                .map(|value| value.unwrap_or(Value::Unit));
            self.guard_depth = entered_guard_depth;
            let value = value?;
            Ok(self.guard_subject_value(handle, frame, value))
        }

        pub(crate) fn try_float_boundary_value_call(
            &mut self,
            target: &str,
            call: &typed_trees::expression::TableCallExpression,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            if !call.receiver.is_valid() {
                return Ok(None);
            }

            let core_format =
                typed_trees::operator::resolve_named_expression_call(self.program, call).and_then(
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
            let compatibility_call = core_format.is_none()
                && !self.expression_receiver_is_namespace(call.receiver)
                && matches!(target, "square_root" | "fused_multiply_add");
            if core_format.is_none() && !compatibility_call {
                return Ok(None);
            }

            let arguments = expression_table(self.program)
                .expression_handles(call.arguments)
                .to_vec();
            let Some(expected_arity) = FloatSemanticOperation::named_call_arity(target) else {
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
                "square_root" => {
                    float_value(FloatSemantics::square_root(format, &operands[0]), false)?
                }
                "multiply_then_add" => float_value(
                    FloatSemantics::multiply_then_add(
                        format,
                        &operands[0],
                        &operands[1],
                        &operands[2],
                    ),
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
                "minimum" => {
                    float_value(FloatSemantics::minimum(&operands[0], &operands[1]), false)?
                }
                "maximum" => {
                    float_value(FloatSemantics::maximum(&operands[0], &operands[1]), false)?
                }
                "classify" => {
                    let (variant_name, negative) =
                        match FloatSemantics::classify(format, &operands[0]) {
                            SemanticFloatClass::NaN => ("NaN", None),
                            SemanticFloatClass::Infinity { negative } => {
                                ("Infinity", Some(negative))
                            }
                            SemanticFloatClass::Normal { negative } => ("Normal", Some(negative)),
                            SemanticFloatClass::Subnormal { negative } => {
                                ("Subnormal", Some(negative))
                            }
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

            if compatibility_call {
                self.host_boundary_touched = true;
            }
            Ok(Some(value))
        }
        fn unary_float_argument(
            &mut self,
            call: &typed_trees::expression::TableCallExpression,
            frame: &mut Frame,
        ) -> Result<Option<(Value, SemanticFloatFormat)>, Halt> {
            let arguments = expression_table(self.program)
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
}

pub(crate) mod layout_target_facts {
    use crate::interpreter::evaluator::{
        EvalResult, Evaluator, Frame, Halt, Value, expression_table, trap,
    };

    impl Evaluator<'_> {
        pub(crate) fn try_layout_target_facts(
            &mut self,
            call: &typed_trees::expression::TableCallExpression,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let Some(result) = self
                .program
                .layout_target_facts_result(call.target_symbol)
                .map_err(|reason| Halt::Trap(reason.into()))?
            else {
                return Ok(None);
            };
            let [argument] = expression_table(self.program).expression_handles(call.arguments)
            else {
                return trap("layout target snapshot requires its single shared context argument");
            };
            let value = self.eval_expression(*argument, frame)?;
            let value = match value {
                Value::Ref(reference) => self.read_reference(&reference)?,
                value => value,
            };
            let Value::Struct {
                type_name, fields, ..
            } = value
            else {
                return trap("layout target snapshot did not receive an issued context");
            };
            if type_name.as_ref() != "@compiler-layout-target"
                || fields.len() != 4
                || ![
                    "profile",
                    "pointer_size",
                    "pointer_alignment",
                    "little_endian",
                ]
                .iter()
                .all(|name| fields.contains_key(*name))
            {
                return trap("layout target snapshot requires the exact compiler-issued context");
            }
            let fields = fields
                .into_iter()
                .map(|(name, cell)| {
                    let value = cell.borrow().clone();
                    self.allocate_cell(value).map(|cell| (name, cell))
                })
                .collect::<EvalResult<_>>()?;
            Ok(Some(Value::Struct {
                type_symbol: result,
                type_name: self.program.symbols.name(result).into(),
                fields,
            }))
        }
    }
}

pub(crate) mod literal_evaluation {
    use crate::interpreter::evaluator::{
        BTreeMap, Cell, EvalResult, Evaluator, ExpressionHandle, Frame, Value, trap,
    };
    use validation::{selected_construction_case, selected_construction_definition};

    impl<'program> Evaluator<'program> {
        pub(crate) fn expression_receiver_is_namespace(&self, receiver: ExpressionHandle) -> bool {
            if !receiver.is_valid() {
                return false;
            }
            let Some(path) = self.program.expression_table.name_path_node(receiver) else {
                return false;
            };
            self.is_declaration_namespace(path.symbol)
        }

        pub(crate) fn eval_struct_literal(
            &mut self,
            expression: ExpressionHandle,
            literal: &typed_trees::expression::TableStructLiteral,
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            if let Some(case_name) = &literal.case_name {
                return self.eval_case_literal(expression, literal, case_name.as_str(), frame);
            }
            let Some(data) = selected_construction_definition(self.program, expression, literal)
            else {
                return trap("record construction has no exact receiving data shape");
            };
            let type_symbol = data.symbol;
            let type_name = data.name.as_str().to_owned();
            let mut fields = BTreeMap::new();
            self.populate_data_fields(data, &mut fields)?;
            for field in self.program.expression_table.struct_fields(literal.fields) {
                let Some(field_type) = validation::selected_construction_field_type(
                    self.program,
                    expression,
                    literal,
                    field.field_symbol,
                ) else {
                    return trap(format!(
                        "record field `{}` has no checked receiving type",
                        field.name
                    ));
                };
                let value = self.eval_expression_at_type(field.value, field_type, frame)?;
                let value = self.coerce_scalar_value(value, field_type)?;
                let value = self.capture_value_operand(value, field_type)?;
                fields.insert(field.name.as_str().into(), self.allocate_cell(value)?);
            }
            Ok(Value::Struct {
                type_symbol,
                type_name: type_name.into(),
                fields,
            })
        }

        pub(crate) fn eval_case_literal(
            &mut self,
            expression: ExpressionHandle,
            literal: &typed_trees::expression::TableStructLiteral,
            case_name: &str,
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            let type_name = literal.type_name.as_str();
            let Some(data) = selected_construction_definition(self.program, expression, literal)
            else {
                return trap(format!(
                    "case `{type_name}::{case_name}` has no exact receiving data shape"
                ));
            };
            let Some(variant) = selected_construction_case(self.program, expression, literal, data)
            else {
                return trap(format!("`{type_name}` has no case `{case_name}`"));
            };

            let mut payload = Vec::new();
            for member in self.program.data_members(data) {
                let Some(common_field) = member.as_field() else {
                    continue;
                };
                let name: crate::value::ValueName = common_field.name.as_str().into();
                let value = self.default_value_for_type(common_field.type_reference)?;
                payload.push((name, self.allocate_cell(value)?));
            }
            for field in self.program.data_payload_fields(variant) {
                let name: crate::value::ValueName = field.name.as_str().into();
                let value = self.default_value_for_type(field.type_reference)?;
                payload.push((name, self.allocate_cell(value)?));
            }
            for field in self.program.expression_table.struct_fields(literal.fields) {
                let Some(field_type) = validation::selected_construction_field_type(
                    self.program,
                    expression,
                    literal,
                    field.field_symbol,
                ) else {
                    return trap(format!(
                        "case field `{}` has no checked receiving type",
                        field.name
                    ));
                };
                let value = self.eval_expression_at_type(field.value, field_type, frame)?;
                let value = self.capture_value_operand(value, field_type)?;
                let Some(slot) = payload
                    .iter_mut()
                    .find(|(name, _)| **name == *field.name.as_str())
                else {
                    return trap(format!(
                        "case `{type_name}::{case_name}` has no payload field `{}`",
                        field.name.as_str()
                    ));
                };
                slot.1 = self.allocate_cell(value)?;
            }

            Ok(Value::Enum {
                type_symbol: data.symbol,
                variant_name: case_name.into(),
                payload,
            })
        }

        pub(crate) fn eval_call_expression_argument(
            &mut self,
            argument: ExpressionHandle,
            frame: &mut Frame,
        ) -> EvalResult<Cell> {
            self.eval_argument(argument, frame)
        }
    }
}

pub(crate) mod match_dispatch {
    use crate::interpreter::evaluator::{
        ArithmeticDomain, BinaryOperator, EvalResult, Evaluator, ExpressionHandle, Frame, Halt,
        PrimitiveType, Value, expression_table, unsupported,
    };
    use arena::Handle;
    use checked_trees::CheckedOperatorOccurrence;
    use typed_trees::expression::{MatchPattern, TableMatchArm, TableMatchExpression};

    impl Evaluator<'_> {
        pub(crate) fn with_selected_match<ResultValue>(
            &mut self,
            expression: ExpressionHandle,
            dispatch: &TableMatchExpression,
            frame: &mut Frame,
            evaluate: impl FnOnce(&mut Self, ExpressionHandle, &mut Frame) -> EvalResult<ResultValue>,
        ) -> EvalResult<ResultValue> {
            let outer_captures = frame.match_subjects.len();
            let result = self
                .select_match_arm(expression, dispatch, frame)
                .and_then(|selected| evaluate(self, selected, frame));
            frame.match_subjects.truncate(outer_captures);
            result
        }

        pub(crate) fn select_match_arm(
            &mut self,
            expression: ExpressionHandle,
            dispatch: &TableMatchExpression,
            frame: &mut Frame,
        ) -> EvalResult<ExpressionHandle> {
            let destination = self
                .expression_scalar_type(dispatch.subject, frame)
                .map(|(primitive, _)| primitive);
            let subject = if dispatch.capture.is_valid() {
                let cell = self.eval_read_cell(dispatch.subject, frame)?;
                let reference = self
                    .program
                    .machine_by_symbol(frame.machine_symbol)
                    .and_then(|machine| {
                        self.program
                            .state_in_machine(machine, frame.state_symbol)
                            .and_then(|state| {
                                validation::expression_result_type_reference(
                                    self.program,
                                    machine,
                                    state,
                                    dispatch.subject,
                                )
                            })
                    });
                let cell = if let Some(reference) = reference
                    && self.program.type_multiplicity(reference)
                        == language_semantics::Multiplicity::Unrestricted
                {
                    let value = cell.borrow().clone();
                    let captured = self.capture_value_operand(value, reference)?;
                    self.allocate_cell(captured)?
                } else {
                    cell
                };
                frame.match_subjects.push((dispatch.capture, cell.clone()));
                let value = cell.borrow().clone();
                self.observe_reference_value(value)?
            } else {
                self.eval_expression_with_destination(dispatch.subject, destination, frame)?
            };
            for (ordinal, arm) in expression_table(self.program)
                .match_arms(dispatch.arms)
                .iter()
                .enumerate()
            {
                let matches = match arm.pattern {
                    MatchPattern::Destructure(pattern) => self
                        .eval_expression(pattern.condition, frame)?
                        .as_bool()
                        .ok_or_else(|| {
                            Halt::Unsupported(
                                "structural pattern predicate is not Boolean".to_owned(),
                            )
                        })?,
                    MatchPattern::Wildcard => true,
                    MatchPattern::Value(pattern) => {
                        let pattern =
                            self.eval_expression_with_destination(pattern, destination, frame)?;
                        let arm_index = u32::try_from(ordinal)
                            .ok()
                            .and_then(|ordinal| {
                                dispatch.arms.start().arena_index().checked_add(ordinal)
                            })
                            .ok_or_else(|| {
                                Halt::Unsupported("Match arm identity overflowed".to_owned())
                            })?;
                        let source_arm =
                            Handle::from_parts(arm_index, dispatch.arms.start().generation());
                        let selected_float = self.operator_facts.is_some_and(|facts| {
                            facts.uses.iter().any(|(handle, selected)| {
                                selected.expression == expression
                                    && selected.origin.machine_symbol()
                                        == Some(frame.machine_symbol)
                                    && selected.occurrence
                                        == CheckedOperatorOccurrence::MatchEquality { source_arm }
                                    && facts
                                        .selected_float_comparison(self.program, handle)
                                        .is_some()
                            })
                        });
                        let value = if selected_float
                            || matches!(destination, Some(PrimitiveType::F32 | PrimitiveType::F64))
                            || subject.is_float()
                            || pattern.is_float()
                        {
                            self.eval_selected_match_equality(
                                expression,
                                source_arm,
                                &subject,
                                &pattern,
                                destination,
                                frame,
                            )?
                        } else {
                            self.eval_binary(
                                BinaryOperator::Equal,
                                subject.clone(),
                                pattern,
                                false,
                                None,
                                None,
                            )?
                        };
                        value.as_bool().ok_or_else(|| {
                            Halt::Unsupported("match equality did not produce a Boolean".to_owned())
                        })?
                    }
                };
                if matches {
                    return Ok(arm.value);
                }
            }
            unsupported(
                "checked match has no selected arm; exhaustive dispatch evidence is required",
            )
        }

        fn eval_selected_match_equality(
            &self,
            expression: ExpressionHandle,
            source_arm: Handle<TableMatchArm>,
            subject: &Value,
            pattern: &Value,
            destination: Option<PrimitiveType>,
            frame: &Frame,
        ) -> EvalResult<Value> {
            let missing = || {
                Halt::Unsupported(
                    "selected floating Match equality has no interpreter execution custody"
                        .to_owned(),
                )
            };
            let facts = self.operator_facts.ok_or_else(missing)?;
            let mut uses = facts.uses.iter().filter(|(_, selected)| {
                selected.expression == expression
                    && selected.origin.machine_symbol() == Some(frame.machine_symbol)
                    && selected.occurrence
                        == CheckedOperatorOccurrence::MatchEquality { source_arm }
            });
            let (operator_use, _) = uses.next().ok_or_else(missing)?;
            if uses.next().is_some() {
                return Err(missing());
            }
            let mut executions = facts
                .selected_float_comparisons
                .iter()
                .map(|(_, execution)| execution)
                .filter(|execution| execution.operator_use() == operator_use);
            let execution = executions.next().ok_or_else(missing)?;
            if executions.next().is_some() {
                return Err(missing());
            }
            let primitive = execution
                .validated_primitive(self.program, facts)
                .ok_or_else(missing)?;
            if destination.is_some_and(|destination| destination != primitive) {
                return Err(missing());
            }
            if !subject.is_float() || !pattern.is_float() {
                return Err(missing());
            }
            self.eval_float_binary(
                BinaryOperator::Equal,
                subject,
                pattern,
                Some((primitive, ArithmeticDomain::Exact)),
                None,
            )
        }
    }
}

/*
    Scalar binary and unary operations, casts and recasts, numeric landing, and
    stateless operator applications for the checked-tree interpreter.

    We evaluate width- and domain-aware integer and IEEE floating-point
    operations and enforce declared landing-slot arithmetic domains.
*/

pub(crate) mod casts_and_recasts {
    use crate::interpreter::evaluator::{
        ArithmeticDomain, BigInt, EvalResult, Evaluator, ExpressionNode, FloatMeaning,
        FloatSemantics, Frame, Halt, PrimitiveType, SemanticFloatFormat, Value,
        big_integer_runtime_value, float_to_integer_trap_message, is_unsigned_integer_primitive,
        semantic_integer_format, trap, type_reference_table, wrap_to_width,
    };
    impl<'program> Evaluator<'program> {
        pub(crate) fn try_dynamic_selector_cast(
            &mut self,
            cast: &typed_trees::expression::TableCastExpression,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            if !matches!(
                type_reference_table(self.program).type_reference(cast.target_type),
                typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
            ) {
                return Ok(None);
            }
            if let ExpressionNode::Name(path) =
                self.program.expression_table.expression(cast.value).clone()
            {
                let implementation = self
                    .program
                    .conformance_by_symbol(path.symbol)
                    .map(|conformance| (conformance.symbol, conformance.trait_name.as_str()))
                    .or_else(|| {
                        self.find_machine_by_symbol(path.symbol)
                            .map(|machine| (machine.symbol, machine.name.as_str()))
                    });
                if let Some((symbol, name)) = implementation {
                    return Ok(Some(Value::Struct {
                        type_symbol: symbol,
                        type_name: name.into(),
                        fields: Default::default(),
                    }));
                }
            }
            self.eval_expression(cast.value, frame).map(Some)
        }

        pub(crate) fn cast_target_primitive(
            &self,
            target_type: typed_trees::types::TypeReferenceHandle,
        ) -> Option<PrimitiveType> {
            self.program.primitive_type_reference(target_type)
        }

        pub(crate) fn eval_cast(
            &self,
            value: Value,
            source: Option<PrimitiveType>,
            target: Option<PrimitiveType>,
        ) -> EvalResult<Value> {
            if target.is_some_and(|target| semantic_integer_format(target).is_some())
                && matches!(value, Value::Float(_) | Value::UndeterminedNaN(_))
            {
                let integral = match value.float_meaning() {
                    Some(FloatMeaning::FiniteNonZero(value)) => value.to_integer_exact().is_some(),
                    Some(FloatMeaning::Zero { .. }) => true,
                    _ => false,
                };
                if !integral {
                    return trap("exact authored float-to-integer coercion changes denotation");
                }
            }
            let original_float = matches!(value, Value::Float(_) | Value::UndeterminedNaN(_))
                .then(|| value.float_meaning())
                .flatten();
            let original_integer = match &value {
                Value::Int(value) if source.is_some_and(is_unsigned_integer_primitive) => {
                    Some(BigInt::from_u64(*value as u64))
                }
                Value::Int(value) => Some(BigInt::from_i64(*value)),
                _ => None,
            };
            let converted =
                self.eval_named_conversion(value, source, target, ArithmeticDomain::Exact)?;
            if target
                .is_some_and(|target| matches!(target, PrimitiveType::F32 | PrimitiveType::F64))
            {
                let result = converted.float_meaning();
                if let Some(original) = original_float {
                    if result.as_ref() != Some(&original) {
                        return trap("exact authored float coercion changes denotation");
                    }
                } else if let Some(original) = original_integer {
                    let represented = match result {
                        Some(FloatMeaning::FiniteNonZero(value)) => value.to_integer_exact(),
                        Some(FloatMeaning::Zero { .. }) => Some(BigInt::zero()),
                        _ => None,
                    };
                    if represented != Some(original) {
                        return trap("exact authored integer-to-float coercion changes denotation");
                    }
                }
            }
            Ok(converted)
        }

        pub(crate) fn eval_named_conversion(
            &self,
            value: Value,
            source: Option<PrimitiveType>,
            target: Option<PrimitiveType>,
            domain: ArithmeticDomain,
        ) -> EvalResult<Value> {
            let Some(target) = target else {
                return Ok(value);
            };
            match target {
                PrimitiveType::F32 | PrimitiveType::F64 => {
                    let format = if target == PrimitiveType::F32 {
                        SemanticFloatFormat::BINARY32
                    } else {
                        SemanticFloatFormat::BINARY64
                    };
                    let meaning = match value {
                        Value::UndeterminedNaN(_) => FloatMeaning::NaN,
                        Value::Float(source) => {
                            FloatSemantics::convert(format, &FloatMeaning::from_f64(source))
                        }
                        Value::Int(value) => {
                            let value = if source.is_some_and(is_unsigned_integer_primitive) {
                                BigInt::from_u64(value as u64)
                            } else {
                                BigInt::from_i64(value)
                            };
                            FloatSemantics::from_integer(format, &value)
                        }
                        _ => return trap(format!("cast to {target:?} of non-numeric")),
                    };
                    Ok(self.semantic_float_result(meaning, format))
                }
                PrimitiveType::Bool => Ok(Value::Bool(value.as_bool().unwrap_or(false))),
                integer => {
                    let raw = match value {
                        value @ (Value::Float(_) | Value::UndeterminedNaN(_)) => {
                            let format = semantic_integer_format(integer)
                                .expect("a primitive integer has a semantic format");
                            let meaning = value.float_meaning().ok_or_else(|| {
                                Halt::Trap("float conversion lost its meaning".to_owned())
                            })?;
                            let converted = match domain {
                                ArithmeticDomain::Saturating => {
                                    FloatSemantics::to_integer_saturating(&meaning, format)
                                }
                                ArithmeticDomain::Trapping => FloatSemantics::to_integer_trapping(
                                    &meaning, format,
                                )
                                .map_err(|reason| {
                                    Halt::Trap(float_to_integer_trap_message(integer, reason, true))
                                })?,
                                ArithmeticDomain::Exact => FloatSemantics::to_integer_exact(
                                    &meaning, format,
                                )
                                .map_err(|reason| {
                                    Halt::Trap(float_to_integer_trap_message(
                                        integer, reason, false,
                                    ))
                                })?,
                                ArithmeticDomain::Wrapping => {
                                    return trap(
                                    "Wrapping float-to-integer conversion has no semantic reading"
                                        .to_owned(),
                                );
                                }
                            };
                            return Ok(Value::Int(big_integer_runtime_value(&converted, integer)));
                        }
                        other => other.as_int().ok_or_else(|| {
                            Halt::Trap("cast to integer of non-numeric".to_owned())
                        })?,
                    };
                    if domain == ArithmeticDomain::Wrapping {
                        return Ok(Value::Int(wrap_to_width(raw, integer)));
                    }
                    let mathematical = if source.is_some_and(is_unsigned_integer_primitive) {
                        i128::from(raw as u64)
                    } else {
                        i128::from(raw)
                    };
                    let format = semantic_integer_format(integer).ok_or_else(|| {
                        Halt::Trap("integer conversion has no target format".to_owned())
                    })?;
                    let (minimum, maximum) = if format.signed {
                        let half_range = 1_i128 << (format.bits - 1);
                        (-half_range, half_range - 1)
                    } else {
                        (0, (1_i128 << format.bits) - 1)
                    };
                    let converted = match domain {
                        ArithmeticDomain::Saturating => mathematical.clamp(minimum, maximum),
                        ArithmeticDomain::Exact | ArithmeticDomain::Trapping
                            if mathematical < minimum || mathematical > maximum =>
                        {
                            return trap(format!(
                                "integer conversion out of range in {domain:?} domain: {mathematical} for {integer:?}"
                            ));
                        }
                        _ => mathematical,
                    };
                    Ok(Value::Int(wrap_to_width(converted as i64, integer)))
                }
            }
        }

        pub(crate) fn eval_interior_recast(
            &mut self,
            cast: &typed_trees::expression::TableCastExpression,
            target: Option<PrimitiveType>,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let ExpressionNode::Indexed(indexed) =
                self.program.expression_table.expression(cast.value).clone()
            else {
                return Ok(None);
            };
            let offset_value = match self.program.expression_table.expression(indexed.index) {
                ExpressionNode::Integer(literal) => literal.value_i64(),
                _ => self.eval_expression(indexed.index, frame)?.as_int(),
            };
            let Some(offset) = offset_value.and_then(|value| usize::try_from(value).ok()) else {
                return Ok(None);
            };
            let collection = self.eval_expression(indexed.collection, frame)?;
            let Value::Array(cells) = collection else {
                return Ok(None);
            };
            let Some(target) = target else {
                return self.assemble_record_view_type(cast.target_type, &cells, offset);
            };
            self.assemble_scalar_byte_region(&cells, offset, target)
                .map(Some)
        }
    }
}

pub(crate) mod numeric_landing {
    use crate::interpreter::evaluator::{
        EvalResult, EvaluatedArgument, Evaluator, ExpressionHandle, ExpressionNode, Frame, Halt,
        PrimitiveType, Reference, State, TypeReferenceHandle, TypeReferenceNode, Value,
        type_reference_table, unsupported,
    };
    use checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback;
    impl Evaluator<'_> {
        pub(crate) fn eval_expression_at_type(
            &mut self,
            expression: ExpressionHandle,
            destination: TypeReferenceHandle,
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            if let Some(carrier) = validation::atomic_once_carrier(self.program, expression) {
                self.tick()?;
                let declared = self
                    .expression_type_reference(carrier.place, frame)
                    .ok_or_else(|| {
                        Halt::Unsupported("atomic attempt lost its cell type".to_owned())
                    })?;
                let TypeReferenceNode::Named { symbol, .. } =
                    self.program.type_reference_table.type_reference(declared)
                else {
                    return unsupported("atomic attempt needs a dedicated atomic cell");
                };
                if !self
                    .program
                    .symbols
                    .builtin_type_atom(*symbol)
                    .is_some_and(|atom| atom.is_atomic())
                {
                    return unsupported("atomic attempt needs a dedicated atomic cell");
                }
                let Some(primitive) = self.program.primitive_type_reference(declared) else {
                    return unsupported("atomic attempt lost its scalar resident");
                };
                let Some(custody) = carrier.operation.single_attempt_custody() else {
                    return unsupported("atomic attempt lost its selected outcome contract");
                };
                if !validation::atomic_once_result_type(
                    self.program,
                    destination,
                    primitive,
                    custody,
                ) {
                    return unsupported("atomic attempt has no exact canonical outcome type");
                }
                let Some(data) = self.program.closed_data_definition(destination) else {
                    return unsupported("atomic outcome type is absent");
                };
                let type_symbol = data.symbol;
                let cell = self.resolve_place(carrier.place, frame)?;
                let cell = self.deref_cell(cell);
                let expected =
                    self.eval_expression_at_type(carrier.operands[0], declared, frame)?;
                let replacement =
                    self.eval_expression_at_type(carrier.operands[1], declared, frame)?;
                let prior = cell.borrow().clone();
                let (Value::Int(observed), Value::Int(expected), Value::Int(_)) =
                    (&prior, &expected, &replacement)
                else {
                    return unsupported("atomic attempt needs integer resident and operands");
                };
                let matched = observed == expected;
                let case = custody.outcome_identity.cases()[usize::from(matched)];
                let payload = match case.payload {
                    None => Vec::new(),
                    Some(role) => {
                        use language_core::atomic::AtomicCompareExchangeOutcomePayload;
                        let value = match role {
                            AtomicCompareExchangeOutcomePayload::Observed
                            | AtomicCompareExchangeOutcomePayload::Displaced => prior,
                            AtomicCompareExchangeOutcomePayload::Proposed => replacement.clone(),
                        };
                        vec![(role.field_name().into(), self.allocate_cell(value)?)]
                    }
                };
                if matched {
                    *cell.borrow_mut() = replacement;
                }
                return Ok(Value::Enum {
                    type_symbol,
                    variant_name: case.case.name().into(),
                    payload,
                });
            }
            if let ExpressionNode::Match(dispatch) =
                self.program.expression_table.expression(expression).clone()
            {
                self.tick()?;
                return self.with_selected_match(
                    expression,
                    &dispatch,
                    frame,
                    |evaluator, selected, frame| {
                        evaluator.eval_expression_at_type(selected, destination, frame)
                    },
                );
            }
            if self.declared_type_is_reference(destination) {
                if let Some((source, storage)) =
                    self.capture_reference_view(expression, destination, frame)?
                {
                    return Ok(Value::Ref(Reference::view(source, storage)));
                }
                let cell = self.eval_read_cell(expression, frame)?;
                return Ok(cell.borrow().clone());
            }
            if let ExpressionNode::ArrayLiteral(_) =
                self.program.expression_table.expression(expression)
                && let TypeReferenceNode::FixedArray { element_type, .. } =
                    type_reference_table(self.program).type_reference(destination)
            {
                return self.eval_array_literal_at_element_type(expression, *element_type, frame);
            }
            let carrier = type_reference_table(self.program).peeled_type_reference(destination);
            if let Some(bytes) = self.program.expression_table.string_node(expression)
                && let TypeReferenceNode::FixedArray { element_type, .. } =
                    self.program.type_reference_table.type_reference(carrier)
                && self.program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
            {
                let Some(length) = self.program.closed_fixed_array_length(carrier) else {
                    return unsupported("byte-array literal has no checked closed extent");
                };
                if bytes.len() != length {
                    return unsupported(
                        "byte-array literal width differs from its checked destination",
                    );
                }
                self.tick()?;
                let values = bytes
                    .iter()
                    .map(|byte| self.allocate_cell(Value::Int(i64::from(*byte))))
                    .collect::<EvalResult<Vec<_>>>()?;
                return Ok(Value::Array(values));
            }
            let primitive = self.program.primitive_type_reference(destination);
            self.eval_expression_with_destination(expression, primitive, frame)
        }

        pub(crate) fn eval_array_literal_at_element_type(
            &mut self,
            expression: ExpressionHandle,
            element_type: TypeReferenceHandle,
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            let Some(elements) = self.program.expression_table.array_literal_node(expression)
            else {
                return unsupported("array element destination requires an array literal");
            };
            let elements = *elements;
            self.tick()?;
            let mut values = Vec::new();
            for element in self.program.expression_table.expression_handles(elements) {
                let value = self.eval_expression_at_type(*element, element_type, frame)?;
                let value = self.capture_value_operand(value, element_type)?;
                values.push(self.allocate_cell(value)?);
            }
            Ok(Value::Array(values))
        }

        pub(crate) fn capture_value_operand(
            &self,
            value: Value,
            destination: TypeReferenceHandle,
        ) -> EvalResult<Value> {
            match type_reference_table(self.program).type_reference(destination) {
                TypeReferenceNode::Constrained { base_type, .. } => {
                    return self.capture_value_operand(value, *base_type);
                }
                TypeReferenceNode::Reference { .. } => return Ok(value),
                _ => {}
            }
            let owned_aggregate = matches!(value, Value::Struct { .. } | Value::Enum { .. })
                || (matches!(value, Value::Array(_))
                    && self.declared_type_is_fixed_array(destination));
            if owned_aggregate {
                value.deep_clone_with(&|value| self.allocate_cell(value))
            } else {
                Ok(value)
            }
        }

        pub(crate) fn eval_expression_with_destination(
            &mut self,
            expression: ExpressionHandle,
            destination: Option<PrimitiveType>,
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            self.eval_expression_with_destination_and_carrier(expression, destination, frame)
                .map(|(value, _)| value)
        }

        pub(crate) fn eval_expression_with_destination_and_carrier(
            &mut self,
            expression: ExpressionHandle,
            destination: Option<PrimitiveType>,
            frame: &mut Frame,
        ) -> EvalResult<(Value, Option<PrimitiveType>)> {
            if let ExpressionNode::Match(dispatch) =
                self.program.expression_table.expression(expression).clone()
            {
                self.tick()?;
                return self.with_selected_match(
                    expression,
                    &dispatch,
                    frame,
                    |evaluator, selected, frame| {
                        evaluator.eval_expression_with_destination_and_carrier(
                            selected,
                            destination,
                            frame,
                        )
                    },
                );
            }
            if let Some(value) = self.anonymous_integer_landing_value(expression, destination)? {
                return Ok((value, destination));
            }
            let source = self
                .expression_scalar_type(expression, frame)
                .map(|(primitive, _)| primitive)
                .or_else(|| {
                    self.expression_type_reference(expression, frame)
                        .and_then(|reference| self.program.primitive_type_reference(reference))
                });
            self.eval_expression(expression, frame)
                .map(|value| (value, source))
        }

        pub(crate) fn anonymous_integer_landing_value(
            &mut self,
            expression: ExpressionHandle,
            destination: Option<PrimitiveType>,
        ) -> EvalResult<Option<Value>> {
            if let Some(destination) = destination.filter(|primitive| primitive.is_fixed_integer())
                && let Some((literal, _)) = validation::land_anonymous_integer_expression(
                    self.program,
                    expression,
                    destination,
                    &[],
                    |expression| match self
                        .operator_facts
                        .and_then(|facts| facts.expression_use(expression))
                    {
                        Some(operator) => operator.status == BuiltinFallback,
                        None => {
                            validation::has_anonymous_operator_meaning(self.program, expression)
                        }
                    },
                )
                && let Some(bits) = literal.bits_u64()
            {
                let mut pending = vec![expression];
                while let Some(expression) = pending.pop() {
                    self.tick()?;
                    if let Some(binary) = self.program.expression_table.binary_node(expression) {
                        pending.push(binary.right);
                        pending.push(binary.left);
                    }
                }
                return Ok(Some(Value::Int(bits as i64)));
            }
            Ok(None)
        }

        pub(crate) fn eval_state_arguments(
            &mut self,
            state: &State,
            arguments: &[ExpressionHandle],
            source: Option<validation::CallSite<'_>>,
            frame: &mut Frame,
        ) -> EvalResult<Vec<EvaluatedArgument>> {
            self.check_state_argument_count(state, arguments.len())?;
            let order = match source {
                Some(source) => source.argument_evaluation_order(self.program),
                None => self.program.call_argument_evaluation_order(
                    Default::default(),
                    state.symbol,
                    arguments.len(),
                ),
            }
            .map_err(|reason| Halt::Unsupported(reason.to_owned()))?;
            let mut aliases = Vec::new();
            if let Some(source) = source {
                let application = source.authored_application();
                if application.is_valid() {
                    for binding in self
                        .program
                        .expression_table
                        .static_machine_application_bindings(application)
                    {
                        let typed_trees::static_applications::StaticMachineBindingValue::RuntimeValue {
                        mut expression,
                        ..
                    } = binding.value
                    else {
                        continue;
                    };
                        while expression.is_valid() {
                            match self.program.expression_table.expression(expression) {
                                ExpressionNode::Member(member) => expression = member.receiver,
                                ExpressionNode::SavedSubject(saved) => {
                                    let position = source.captured_argument_position(self.program, saved)
                                    .ok_or_else(|| Halt::Unsupported("runtime descriptor alias belongs to another invocation".to_owned()))?;
                                    aliases.push((position, saved.capture));
                                    break;
                                }
                                _ => break,
                            }
                        }
                    }
                }
            }
            let prefix = frame.match_subjects.len();
            let result = (|| {
                let mut evaluated = vec![None; arguments.len()];
                for ordinal in order {
                    let destination = self.program.state_parameters(state)[ordinal].type_reference;
                    let argument =
                        self.eval_state_argument(arguments[ordinal], destination, frame)?;
                    for (_, capture) in aliases.iter().filter(|(position, _)| *position == ordinal)
                    {
                        frame.match_subjects.push((*capture, argument.cell.clone()));
                    }
                    evaluated[ordinal] = Some(argument);
                }
                evaluated
                    .into_iter()
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        Halt::Unsupported("call argument has no completed observation".to_owned())
                    })
            })();
            frame.match_subjects.truncate(prefix);
            result
        }

        pub(crate) fn check_state_argument_count(
            &self,
            state: &State,
            argument_count: usize,
        ) -> EvalResult<()> {
            let parameters = self.program.state_parameters(state);
            if argument_count == parameters.len() {
                return Ok(());
            }
            unsupported(format!(
                "call to `{}` supplies {argument_count} arguments for {} declared parameters",
                state.name.as_str(),
                parameters.len(),
            ))
        }
    }
}

pub(crate) mod operator_applications {
    use crate::interpreter::evaluator::{
        ArithmeticDomain, BinaryOperator, EvalResult, Evaluator, ExpressionHandle, Frame, Machine,
        PrimitiveType, SemanticFloatFormat, State, SymbolHandle, TypeReferenceHandle, Value, trap,
        type_reference_table,
    };
    use checked_trees::CheckedArithmeticPolicyAdapter;
    use language_semantics::{
        ExternalBindingIdentity, ExternalBindingMechanism, MachineSupplyMode,
    };
    use typed_trees::operator::OperatorDefinition;

    #[derive(Clone, Copy)]
    struct RequirementApplication<'program> {
        symbol: SymbolHandle,
        signature: &'program OperatorDefinition,
    }

    #[derive(Debug, Clone, Copy)]
    enum SelectedRealization<'program> {
        CheckedBody {
            machine: &'program Machine,
            state: &'program State,
        },
        Conversion(DestinationConversion),
        PrimitiveFloatBinary {
            operation: BinaryOperator,
            primitive: PrimitiveType,
            domain: ArithmeticDomain,
            parameters: [TypeReferenceHandle; 2],
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct DestinationConversion {
        parameter: TypeReferenceHandle,
        source: PrimitiveType,
        destination: PrimitiveType,
        domain: ArithmeticDomain,
    }

    enum RealizationKind<'program> {
        CheckedBody(&'program Machine),
        CompilerIntrinsic,
        Other,
    }

    impl<'program> Evaluator<'program> {
        pub(crate) fn try_operator_application(
            &mut self,
            target_symbol: SymbolHandle,
            application: ExpressionHandle,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let Some(realization) = self.selected_realization(target_symbol, application)? else {
                return Ok(None);
            };
            match realization {
                SelectedRealization::CheckedBody { machine, state } => {
                    let arguments = self.eval_state_arguments(state, arguments, None, frame)?;
                    let entered_guard_depth = self.guard_depth;
                    self.guard_depth = 0;
                    let value = self.run_state_collect(machine, state, arguments);
                    self.guard_depth = entered_guard_depth;
                    Ok(Some(value?.unwrap_or(Value::Unit)))
                }
                SelectedRealization::Conversion(conversion) => {
                    let [argument] = arguments else {
                        return trap("a destination-owned conversion takes exactly one argument");
                    };
                    let value =
                        self.eval_expression_at_type(*argument, conversion.parameter, frame)?;
                    self.eval_named_conversion(
                        value,
                        Some(conversion.source),
                        Some(conversion.destination),
                        conversion.domain,
                    )
                    .map(Some)
                }
                SelectedRealization::PrimitiveFloatBinary {
                    operation,
                    primitive,
                    domain,
                    parameters,
                } => {
                    let [left, right] = arguments else {
                        return trap("a primitive float operation takes exactly two operands");
                    };
                    let left = self.eval_expression_at_type(*left, parameters[0], frame)?;
                    let right = self.eval_expression_at_type(*right, parameters[1], frame)?;
                    self.eval_float_binary(
                        operation,
                        &left,
                        &right,
                        Some((primitive, domain)),
                        None,
                    )
                    .map(Some)
                }
            }
        }

        fn selected_realization(
            &self,
            target_symbol: SymbolHandle,
            application: ExpressionHandle,
        ) -> EvalResult<Option<SelectedRealization<'program>>> {
            let Some(facts) = self.operator_facts else {
                return Ok(None);
            };
            let Some(requirement) = self.requirement_application(target_symbol) else {
                return Ok(None);
            };
            let mut adapter = CheckedArithmeticPolicyAdapter::None;
            if application.is_valid() {
                let mut first_selection = None;
                let mut multiple_selections = false;
                facts.for_each_resolved_expression_selection(
                    application,
                    |_, selected, policy, _| {
                        if first_selection.is_none() {
                            first_selection = Some((selected, policy));
                        } else {
                            multiple_selections = true;
                        }
                    },
                );
                let Some((selected, policy)) = first_selection else {
                    return trap("operator application has no checked expression selection");
                };
                if selected != requirement.symbol {
                    return trap("operator application differs from its checked named selection");
                }
                if multiple_selections {
                    return trap("operator application has conflicting checked selections");
                }
                adapter = policy;
            }

            let mut checked_bodies = Vec::new();
            let mut intrinsic_realizations = 0usize;
            let mut other_realizations = 0usize;
            for contract in facts
                .operator_realization_contracts
                .iter()
                .filter(|contract| contract.operator_symbol() == requirement.symbol)
            {
                match self.realization_kind(contract.machine_symbol()) {
                    RealizationKind::CheckedBody(machine) => checked_bodies.push(machine),
                    RealizationKind::CompilerIntrinsic => intrinsic_realizations += 1,
                    RealizationKind::Other => other_realizations += 1,
                }
            }

            match (
                checked_bodies.as_slice(),
                intrinsic_realizations,
                other_realizations,
            ) {
                ([machine], 0, 0) => {
                    let Some(state) = self.program.machine_states(machine).first() else {
                        return trap("selected operator realization has no entry state");
                    };
                    Ok(Some(SelectedRealization::CheckedBody { machine, state }))
                }
                ([], 1.., 0) => Ok(self.compiler_known_realization(requirement.signature, adapter)),
                _ => Ok(None),
            }
        }

        fn requirement_application(
            &self,
            target_symbol: SymbolHandle,
        ) -> Option<RequirementApplication<'program>> {
            if !target_symbol.is_valid() {
                return None;
            }
            let symbol = self.program.symbols.get(target_symbol);
            let application = match symbol.kind {
                symbols::SymbolKind::Operator => {
                    let operator =
                        typed_trees::operator::declaration_by_symbol(self.program, target_symbol)?;
                    operator.is_boundary.then_some(RequirementApplication {
                        symbol: target_symbol,
                        signature: operator,
                    })?
                }
                symbols::SymbolKind::State => {
                    let machine = self.program.machine_by_symbol(symbol.parent)?;
                    if machine.body_is_present
                        || machine.supply_mode != MachineSupplyMode::TopLevelRequirement
                        || !self.program.machine_type_parameters(machine).is_empty()
                        || self.program.machine_states(machine).first()?.symbol != target_symbol
                    {
                        return None;
                    }
                    let signature = self
                        .program
                        .machine_token_binding_by_symbol(machine.symbol)?;
                    RequirementApplication {
                        symbol: machine.symbol,
                        signature,
                    }
                }
                _ => return None,
            };
            (application.signature.lifetime_parameters.is_empty()
                && self
                    .program
                    .operator_type_parameters(application.signature)
                    .is_empty())
            .then_some(application)
        }

        fn realization_kind(&self, machine_symbol: SymbolHandle) -> RealizationKind<'program> {
            let Some(machine) = self.program.machine_by_symbol(machine_symbol) else {
                return RealizationKind::Other;
            };
            if !machine.lifetime_parameters.is_empty()
                || !self.program.machine_type_parameters(machine).is_empty()
            {
                return RealizationKind::Other;
            }
            if machine.body_is_present && machine.supply_mode.is_checked_body() {
                return RealizationKind::CheckedBody(machine);
            }
            match machine.supply_mode {
                MachineSupplyMode::ExternalRealization {
                    binding: Some(binding),
                    mechanism: Some(ExternalBindingMechanism::CompilerIntrinsic),
                } if !machine.body_is_present
                    && self.program.external_bindings.identity(binding)
                        == Some(&ExternalBindingIdentity::CompilerIntrinsic) =>
                {
                    RealizationKind::CompilerIntrinsic
                }
                _ => RealizationKind::Other,
            }
        }

        fn compiler_known_realization(
            &self,
            signature: &OperatorDefinition,
            adapter: CheckedArithmeticPolicyAdapter,
        ) -> Option<SelectedRealization<'program>> {
            if let Some((operation, format)) =
                typed_trees::operator::primitive_float_binary_semantics(self.program, signature)
            {
                let primitive = match format {
                    numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                    numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                };
                let domain = match adapter {
                    CheckedArithmeticPolicyAdapter::None => ArithmeticDomain::Exact,
                    CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { format }
                        if format == semantic_format(primitive) =>
                    {
                        ArithmeticDomain::Saturating
                    }
                    CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { format }
                        if format == semantic_format(primitive) =>
                    {
                        ArithmeticDomain::Trapping
                    }
                    _ => return None,
                };
                let [left, right] = self.program.operator_parameters(signature) else {
                    return None;
                };
                return Some(SelectedRealization::PrimitiveFloatBinary {
                    operation,
                    primitive,
                    domain,
                    parameters: [left.type_reference, right.type_reference],
                });
            }
            if adapter != CheckedArithmeticPolicyAdapter::None {
                return None;
            }
            self.destination_conversion(signature)
                .map(SelectedRealization::Conversion)
        }

        fn destination_conversion(
            &self,
            signature: &OperatorDefinition,
        ) -> Option<DestinationConversion> {
            let [parameter] = self.program.operator_parameters(signature) else {
                return None;
            };
            if parameter.is_const || parameter.is_mutable {
                return None;
            }
            let source = self
                .program
                .primitive_type_reference(parameter.type_reference)?;
            let destination = self
                .program
                .primitive_type_reference(signature.return_type)?;
            let is_float = |primitive| matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64);
            let is_numeric = |primitive| is_float(primitive) || primitive.is_fixed_integer();
            (source != destination
                && is_numeric(source)
                && is_numeric(destination)
                && (is_float(source) || is_float(destination)))
            .then(|| DestinationConversion {
                parameter: parameter.type_reference,
                source,
                destination,
                domain: type_reference_table(self.program).arithmetic_domain(signature.return_type),
            })
        }
    }

    fn semantic_format(primitive: PrimitiveType) -> SemanticFloatFormat {
        if primitive == PrimitiveType::F32 {
            SemanticFloatFormat::BINARY32
        } else {
            SemanticFloatFormat::BINARY64
        }
    }
}

pub(crate) mod scalar_operations {
    use crate::interpreter::evaluator::{
        ArithmeticDomain, BinaryOperator, Cell, DataDefinition, DataMember, EvalResult, Evaluator,
        ExpressionHandle, FloatMeaning, FloatPolicyTrap, FloatSemantics, Frame, Halt,
        PrimitiveType, SemanticFloatFormat, SymbolHandle, UnaryOperator, Value, integer_bounds,
        primitive_bit_width, primitive_is_unsigned64, trap, unsupported, wrap_to_width,
    };
    use numerics::float_semantics::RoundingDirection;
    use symbols::BuiltinFunction;

    fn byte_sequence_element(element: &Cell) -> EvalResult<u8> {
        let element = element.borrow();
        let Value::Int(value) = &*element else {
            return unsupported("byte-sequence equality has a non-byte element");
        };
        u8::try_from(*value).map_err(|_| {
            Halt::Unsupported("byte-sequence equality has an out-of-range element".to_owned())
        })
    }

    #[derive(Clone, Copy)]
    pub(crate) enum NamedFloatIntrinsic {
        MultiplyThenAdd,
        FusedMultiplyAdd,
        FusedMultiplyAddDirected(RoundingDirection),
        DirectedBinary(BinaryOperator, RoundingDirection),
        DirectedSquareRoot(RoundingDirection),
    }

    impl NamedFloatIntrinsic {
        fn from_target(target: &str) -> Option<(Self, SemanticFloatFormat)> {
            let spelling = target.strip_prefix("float#")?;
            let (stem, format) = match spelling.strip_suffix("_f32") {
                Some(stem) => (stem, SemanticFloatFormat::BINARY32),
                None => (
                    spelling.strip_suffix("_f64")?,
                    SemanticFloatFormat::BINARY64,
                ),
            };
            let Some((operation, direction)) = stem.rsplit_once("_toward_") else {
                let intrinsic = match stem {
                    "multiply_then_add" => Self::MultiplyThenAdd,
                    "fused_multiply_add" => Self::FusedMultiplyAdd,
                    _ => return None,
                };
                return Some((intrinsic, format));
            };
            let direction = match direction {
                "zero" => RoundingDirection::TowardZero,
                "positive" => RoundingDirection::TowardPositive,
                "negative" => RoundingDirection::TowardNegative,
                _ => return None,
            };
            let intrinsic = match operation {
                "fused_multiply_add" => Self::FusedMultiplyAddDirected(direction),
                "sqrt" => Self::DirectedSquareRoot(direction),
                "add" => Self::DirectedBinary(BinaryOperator::Add, direction),
                "subtract" => Self::DirectedBinary(BinaryOperator::Subtract, direction),
                "multiply" => Self::DirectedBinary(BinaryOperator::Multiply, direction),
                "divide" => Self::DirectedBinary(BinaryOperator::Divide, direction),
                _ => return None,
            };
            Some((intrinsic, format))
        }

        const fn arity(self) -> usize {
            match self {
                Self::MultiplyThenAdd
                | Self::FusedMultiplyAdd
                | Self::FusedMultiplyAddDirected(_) => 3,
                Self::DirectedBinary(..) => 2,
                Self::DirectedSquareRoot(_) => 1,
            }
        }

        const fn operand_refusal(self) -> &'static str {
            match self {
                Self::MultiplyThenAdd
                | Self::FusedMultiplyAdd
                | Self::FusedMultiplyAddDirected(_) => {
                    "selected ternary float operand is not a float"
                }
                Self::DirectedBinary(..) => "selected directed-float operand is not a float",
                Self::DirectedSquareRoot(_) => {
                    "selected directed square-root operand is not a float"
                }
            }
        }

        fn trapping_operation(self, target: &str) -> String {
            match self {
                Self::MultiplyThenAdd
                | Self::FusedMultiplyAdd
                | Self::FusedMultiplyAddDirected(_) => {
                    format!(
                        "named float operation `{}`",
                        target
                            .strip_prefix("float#")
                            .and_then(|name| name.rsplit_once('_').map(|(operation, _)| operation))
                            .unwrap_or("ternary_float")
                    )
                }
                Self::DirectedBinary(..) => "directed float operation".to_owned(),
                Self::DirectedSquareRoot(_) => "directed square root".to_owned(),
            }
        }
    }

    fn directed_binary_kernel(
        operation: BinaryOperator,
        direction: RoundingDirection,
    ) -> Option<fn(SemanticFloatFormat, &FloatMeaning, &FloatMeaning) -> FloatMeaning> {
        use RoundingDirection::{TowardNegative, TowardPositive, TowardZero};
        Some(match (operation, direction) {
            (BinaryOperator::Add, TowardZero) => FloatSemantics::add_toward_zero,
            (BinaryOperator::Add, TowardPositive) => FloatSemantics::add_toward_positive,
            (BinaryOperator::Add, TowardNegative) => FloatSemantics::add_toward_negative,
            (BinaryOperator::Subtract, TowardZero) => FloatSemantics::subtract_toward_zero,
            (BinaryOperator::Subtract, TowardPositive) => FloatSemantics::subtract_toward_positive,
            (BinaryOperator::Subtract, TowardNegative) => FloatSemantics::subtract_toward_negative,
            (BinaryOperator::Multiply, TowardZero) => FloatSemantics::multiply_toward_zero,
            (BinaryOperator::Multiply, TowardPositive) => FloatSemantics::multiply_toward_positive,
            (BinaryOperator::Multiply, TowardNegative) => FloatSemantics::multiply_toward_negative,
            (BinaryOperator::Divide, TowardZero) => FloatSemantics::divide_toward_zero,
            (BinaryOperator::Divide, TowardPositive) => FloatSemantics::divide_toward_positive,
            (BinaryOperator::Divide, TowardNegative) => FloatSemantics::divide_toward_negative,
            _ => return None,
        })
    }

    fn fused_multiply_add_kernel(
        direction: RoundingDirection,
    ) -> fn(SemanticFloatFormat, &FloatMeaning, &FloatMeaning, &FloatMeaning) -> FloatMeaning {
        match direction {
            RoundingDirection::TowardZero => FloatSemantics::fused_multiply_add_toward_zero,
            RoundingDirection::TowardPositive => FloatSemantics::fused_multiply_add_toward_positive,
            RoundingDirection::TowardNegative => FloatSemantics::fused_multiply_add_toward_negative,
            RoundingDirection::NearestTiesToEven => FloatSemantics::fused_multiply_add,
        }
    }

    fn directed_square_root_kernel(
        direction: RoundingDirection,
    ) -> fn(SemanticFloatFormat, &FloatMeaning) -> FloatMeaning {
        match direction {
            RoundingDirection::TowardZero => FloatSemantics::square_root_toward_zero,
            RoundingDirection::TowardPositive => FloatSemantics::square_root_toward_positive,
            RoundingDirection::TowardNegative => FloatSemantics::square_root_toward_negative,
            RoundingDirection::NearestTiesToEven => FloatSemantics::square_root,
        }
    }

    impl<'program> Evaluator<'program> {
        pub(crate) fn eval_unary(
            &self,
            operator: UnaryOperator,
            operand: Value,
        ) -> EvalResult<Value> {
            match operator {
                UnaryOperator::BitwiseNot => match operand {
                    Value::Int(value) => Ok(Value::Int(!value)),
                    _ => Err(Halt::Trap("bitwise-not of non-integer".to_owned())),
                },
                UnaryOperator::LogicalNot => operand
                    .as_bool()
                    .map(|value| Value::Bool(!value))
                    .ok_or_else(|| Halt::Trap("logical-not of non-boolean".to_owned())),
            }
        }

        pub(crate) fn named_float_operation_name(
            &self,
            expression: ExpressionHandle,
        ) -> Option<&str> {
            let operator_use = self
                .operator_facts?
                .named_uses()
                .find(|operator_use| operator_use.expression == expression)?;
            let operator = self
                .program
                .operator_definition_by_symbol(operator_use.selected_operator_symbol)?;
            let members = self.program.operator_path_members(operator.name);
            match members.first()?.as_str() {
                "F32" | "F64" => members.last().map(|member| member.as_str()),
                _ => None,
            }
        }

        pub(crate) fn eval_named_float_intrinsic(
            &mut self,
            target: &str,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let Some((intrinsic, format)) = NamedFloatIntrinsic::from_target(target) else {
                return Ok(None);
            };
            if arguments.len() != intrinsic.arity() {
                return Ok(None);
            }
            let domain = arguments
                .iter()
                .filter_map(|argument| self.expression_scalar_type(*argument, frame))
                .reduce(|left, right| {
                    if left.1 != ArithmeticDomain::Exact {
                        left
                    } else {
                        right
                    }
                })
                .map(|(_, domain)| domain);
            let mut operands = Vec::with_capacity(arguments.len());
            for argument in arguments {
                let value = self.eval_expression(*argument, frame)?;
                if !value.is_float() {
                    return unsupported(intrinsic.operand_refusal());
                }
                operands.push(self.float_operand(&value, format)?);
            }
            let meaning = match intrinsic {
                NamedFloatIntrinsic::MultiplyThenAdd => FloatSemantics::multiply_then_add(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                ),
                NamedFloatIntrinsic::FusedMultiplyAdd => FloatSemantics::fused_multiply_add(
                    format,
                    &operands[0],
                    &operands[1],
                    &operands[2],
                ),
                NamedFloatIntrinsic::FusedMultiplyAddDirected(direction) => {
                    fused_multiply_add_kernel(direction)(
                        format,
                        &operands[0],
                        &operands[1],
                        &operands[2],
                    )
                }
                NamedFloatIntrinsic::DirectedBinary(operation, direction) => {
                    let Some(kernel) = directed_binary_kernel(operation, direction) else {
                        return unsupported("unknown selected directed-float intrinsic");
                    };
                    kernel(format, &operands[0], &operands[1])
                }
                NamedFloatIntrinsic::DirectedSquareRoot(direction) => {
                    directed_square_root_kernel(direction)(format, &operands[0])
                }
            };
            let meaning = match domain {
                Some(ArithmeticDomain::Saturating) => {
                    let operand_refs = operands.iter().collect::<Vec<_>>();
                    FloatSemantics::apply_saturating_policy(format, &operand_refs, meaning)
                }
                Some(ArithmeticDomain::Trapping) => FloatSemantics::apply_trapping_policy(meaning)
                    .map_err(|trap_class| {
                        Halt::Trap(format!(
                            "{} produced {} in Trapping domain",
                            intrinsic.trapping_operation(target),
                            trap_class.label()
                        ))
                    })?,
                Some(ArithmeticDomain::Exact | ArithmeticDomain::Wrapping) | None => meaning,
            };
            Ok(Some(self.semantic_float_result(meaning, format)))
        }

        pub(crate) fn eval_integer_comparison(
            &self,
            operator: BinaryOperator,
            left: i64,
            left_type: PrimitiveType,
            right: i64,
            right_type: PrimitiveType,
        ) -> EvalResult<Value> {
            use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
            let decode = |value: i64, primitive| -> EvalResult<(IntegerType, IntegerValue)> {
                let (sign, width) = match primitive {
                    PrimitiveType::I8 => (IntegerSign::Signed, 8),
                    PrimitiveType::I16 => (IntegerSign::Signed, 16),
                    PrimitiveType::I32 => (IntegerSign::Signed, 32),
                    PrimitiveType::I64 => (IntegerSign::Signed, 64),
                    PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
                    PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
                    PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
                    PrimitiveType::U64 | PrimitiveType::Addr => (IntegerSign::Unsigned, 64),
                    _ => return unsupported("integer comparison requires integer carriers"),
                };
                let integer = if primitive == PrimitiveType::Addr {
                    IntegerType::address(width)
                } else {
                    IntegerType::new(sign, width)
                }
                .map_err(|_| Halt::Unsupported("invalid integer comparison carrier".into()))?;
                let value = if sign == IntegerSign::Signed {
                    IntegerValue::Signed(i128::from(value))
                } else {
                    IntegerValue::Unsigned(u128::from(value as u64))
                };
                Ok((integer, value))
            };
            let (left_type, left) = decode(left, left_type)?;
            let (right_type, right) = decode(right, right_type)?;
            let ordering = left_type
                .compare_with(left, right_type, right)
                .ok_or_else(|| {
                    Halt::Unsupported("integer comparison operand outside its carrier".into())
                })?;
            Ok(Value::Bool(match operator {
                BinaryOperator::Less => ordering.is_lt(),
                BinaryOperator::LessOrEqual => ordering.is_le(),
                BinaryOperator::Greater => ordering.is_gt(),
                BinaryOperator::GreaterOrEqual => ordering.is_ge(),
                BinaryOperator::Equal => ordering.is_eq(),
                BinaryOperator::NotEqual => ordering.is_ne(),
                _ => return unsupported("integer comparison requires a comparison operator"),
            }))
        }

        pub(crate) fn eval_binary(
            &self,
            operator: BinaryOperator,
            left: Value,
            right: Value,
            unsigned_operands: bool,
            scalar_type: Option<(PrimitiveType, ArithmeticDomain)>,
            named_float_operation: Option<&str>,
        ) -> EvalResult<Value> {
            use BinaryOperator::*;

            if matches!(operator, And | Or) {
                let l = left
                    .as_bool()
                    .ok_or_else(|| Halt::Trap("logical operand not boolean".to_owned()))?;
                let r = right
                    .as_bool()
                    .ok_or_else(|| Halt::Trap("logical operand not boolean".to_owned()))?;
                return Ok(Value::Bool(match operator {
                    And => l && r,
                    Or => l || r,
                    _ => unreachable!(),
                }));
            }

            if matches!(operator, Equal | NotEqual) {
                let equal = self.values_equal(&left, &right)?;
                return Ok(Value::Bool(if operator == Equal { equal } else { !equal }));
            }

            if let (Value::Str(a), Value::Str(b)) = (&left, &right)
                && operator == Add
            {
                let mut joined = a.borrow().to_vec();
                joined.extend_from_slice(&b.borrow());
                return self.allocate_text(joined);
            }

            if left.is_float() || right.is_float() {
                return self.eval_float_binary(
                    operator,
                    &left,
                    &right,
                    scalar_type,
                    named_float_operation,
                );
            }

            let l = self.arithmetic_operand_int(&left)?;
            let r = self.arithmetic_operand_int(&right)?;
            if matches!(operator, Divide | Modulo)
                && let Some((
                    ty @ (PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64),
                    domain @ (ArithmeticDomain::Wrapping
                    | ArithmeticDomain::Saturating
                    | ArithmeticDomain::Trapping),
                )) = scalar_type
            {
                if r == 0 {
                    return if operator == Divide {
                        trap("integer division by zero")
                    } else {
                        trap("integer modulo by zero")
                    };
                }
                let wide = if operator == Divide {
                    l as i128 / r as i128
                } else {
                    l as i128 % r as i128
                };
                let (min, max) = integer_bounds(ty).unwrap_or((i64::MIN, i64::MAX));
                return match domain {
                    ArithmeticDomain::Wrapping => Ok(Value::Int(wrap_to_width(wide as i64, ty))),
                    ArithmeticDomain::Saturating => {
                        Ok(Value::Int(wide.clamp(min as i128, max as i128) as i64))
                    }
                    ArithmeticDomain::Trapping if wide < min as i128 || wide > max as i128 => {
                        trap(format!(
                            "arithmetic overflow in Trapping domain: {wide} is out of range for {ty:?}"
                        ))
                    }
                    _ => Ok(Value::Int(wide as i64)),
                };
            }
            if let Some((ty, ArithmeticDomain::Wrapping)) = scalar_type
                && matches!(operator, Add | Subtract | Multiply | ShiftLeft | ShiftRight)
            {
                let wide = match operator {
                    Add => l.wrapping_add(r),
                    Subtract => l.wrapping_sub(r),
                    Multiply => l.wrapping_mul(r),
                    ShiftLeft => {
                        let masked = ((r as u64) & (primitive_bit_width(ty) - 1)) as u32;
                        l.wrapping_shl(masked)
                    }
                    ShiftRight => {
                        let masked = ((r as u64) & (primitive_bit_width(ty) - 1)) as u32;
                        if unsigned_operands {
                            ((l as u64).wrapping_shr(masked)) as i64
                        } else {
                            l.wrapping_shr(masked)
                        }
                    }
                    _ => unreachable!(),
                };
                return Ok(Value::Int(wrap_to_width(wide, ty)));
            }
            if let Some((
                ty,
                domain @ (ArithmeticDomain::Saturating | ArithmeticDomain::Trapping),
            )) = scalar_type
            {
                if domain == ArithmeticDomain::Trapping
                    && matches!(operator, ShiftLeft | ShiftRight)
                    && (r as u64) >= primitive_bit_width(ty)
                {
                    return trap(format!(
                        "shift count out of range in Trapping domain: the count is not below \
                     the operand width for {ty:?}"
                    ));
                }
                if operator == ShiftRight {
                    return Ok(Value::Int(wrap_to_width(
                        if (r as u64) >= primitive_bit_width(ty) {
                            if unsigned_operands || l >= 0 { 0 } else { -1 }
                        } else if unsigned_operands {
                            ((l as u64).wrapping_shr(r as u32)) as i64
                        } else {
                            l.wrapping_shr(r as u32)
                        },
                        ty,
                    )));
                }
                if operator == ShiftLeft {
                    let (minimum, maximum, value) = if primitive_is_unsigned64(Some(ty)) {
                        (0i128, u64::MAX as i128, l as u64 as i128)
                    } else {
                        let (minimum, maximum) = integer_bounds(ty).unwrap_or((i64::MIN, i64::MAX));
                        (minimum as i128, maximum as i128, l as i128)
                    };
                    let wide = if (r as u64) >= primitive_bit_width(ty) {
                        match value.signum() {
                            0 => 0,
                            1 => maximum + 1,
                            _ => minimum - 1,
                        }
                    } else {
                        value << (r as u32)
                    };
                    return match domain {
                        ArithmeticDomain::Saturating => {
                            Ok(Value::Int(wide.clamp(minimum, maximum) as i64))
                        }
                        ArithmeticDomain::Trapping if wide < minimum || wide > maximum => {
                            trap(format!(
                                "arithmetic overflow in Trapping domain: shifted value is out of range for {ty:?}"
                            ))
                        }
                        _ => Ok(Value::Int(wide as i64)),
                    };
                }
                if matches!(operator, Add | Subtract | Multiply) {
                    let bounds_and_wide = if primitive_is_unsigned64(Some(ty)) {
                        let (lu, ru) = (l as u64 as i128, r as u64 as i128);
                        let wide = match operator {
                            Add => lu + ru,
                            Subtract => lu - ru,
                            Multiply => lu * ru,
                            _ => unreachable!(),
                        };
                        Some((0i128, u64::MAX as i128, wide))
                    } else if let Some((min, max)) = integer_bounds(ty) {
                        let wide = match operator {
                            Add => l as i128 + r as i128,
                            Subtract => l as i128 - r as i128,
                            Multiply => l as i128 * r as i128,
                            _ => unreachable!(),
                        };
                        Some((min as i128, max as i128, wide))
                    } else {
                        None
                    };
                    if let Some((min, max, wide)) = bounds_and_wide {
                        return match domain {
                            ArithmeticDomain::Saturating => {
                                Ok(Value::Int(wide.clamp(min, max) as i64))
                            }
                            ArithmeticDomain::Trapping if wide < min || wide > max => {
                                trap(format!(
                                    "arithmetic overflow in Trapping domain: {wide} is out of range for {ty:?}"
                                ))
                            }
                            _ => Ok(Value::Int(wide as i64)),
                        };
                    }
                }
            }
            self.eval_int_binary(operator, l, r, unsigned_operands)
        }

        fn eval_int_binary(
            &self,
            operator: BinaryOperator,
            l: i64,
            r: i64,
            unsigned_operands: bool,
        ) -> EvalResult<Value> {
            use BinaryOperator::*;
            Ok(match operator {
                Add => Value::Int(l.wrapping_add(r)),
                Subtract => Value::Int(l.wrapping_sub(r)),
                Multiply => Value::Int(l.wrapping_mul(r)),
                Divide => {
                    if r == 0 {
                        return trap("integer division by zero");
                    }
                    if unsigned_operands {
                        Value::Int(((l as u64).wrapping_div(r as u64)) as i64)
                    } else {
                        Value::Int(l.wrapping_div(r))
                    }
                }
                Modulo => {
                    if r == 0 {
                        return trap("integer modulo by zero");
                    }
                    if unsigned_operands {
                        Value::Int(((l as u64).wrapping_rem(r as u64)) as i64)
                    } else {
                        Value::Int(l.wrapping_rem(r))
                    }
                }
                ShiftLeft => Value::Int(l.wrapping_shl(r as u32)),
                ShiftRight if unsigned_operands => {
                    Value::Int(((l as u64).wrapping_shr(r as u32)) as i64)
                }
                ShiftRight => Value::Int(l.wrapping_shr(r as u32)),
                BitwiseAnd => Value::Int(l & r),
                BitwiseOr => Value::Int(l | r),
                BitwiseXor => Value::Int(l ^ r),
                Less if unsigned_operands => Value::Bool((l as u64) < (r as u64)),
                LessOrEqual if unsigned_operands => Value::Bool((l as u64) <= (r as u64)),
                Greater if unsigned_operands => Value::Bool((l as u64) > (r as u64)),
                GreaterOrEqual if unsigned_operands => Value::Bool((l as u64) >= (r as u64)),
                Less => Value::Bool(l < r),
                LessOrEqual => Value::Bool(l <= r),
                Greater => Value::Bool(l > r),
                GreaterOrEqual => Value::Bool(l >= r),
                CaseMembership => return trap("case membership requires its retained classifier"),
                Equal | NotEqual | And | Or => unreachable!("handled earlier"),
            })
        }

        pub(crate) fn eval_float_binary(
            &self,
            operator: BinaryOperator,
            left: &Value,
            right: &Value,
            scalar_type: Option<(PrimitiveType, ArithmeticDomain)>,
            named_float_operation: Option<&str>,
        ) -> EvalResult<Value> {
            use BinaryOperator::*;
            let format = if matches!(scalar_type, Some((PrimitiveType::F32, _))) {
                SemanticFloatFormat::BINARY32
            } else {
                SemanticFloatFormat::BINARY64
            };
            let left = self.float_operand(left, format)?;
            let right = self.float_operand(right, format)?;
            let domain = scalar_type.map(|(_, domain)| domain);
            let arith = |meaning: FloatMeaning, division: bool| -> EvalResult<Value> {
                let meaning = match domain {
                    Some(ArithmeticDomain::Saturating) if division => {
                        FloatSemantics::apply_saturating_divide_policy(
                            format, &left, &right, meaning,
                        )
                    }
                    Some(ArithmeticDomain::Saturating) => {
                        FloatSemantics::apply_saturating_policy(format, &[&left, &right], meaning)
                    }
                    Some(ArithmeticDomain::Trapping) => {
                        match FloatSemantics::apply_trapping_policy(meaning) {
                            Ok(finite) => finite,
                            Err(FloatPolicyTrap::NaNResult) => {
                                if let Some(operation) = named_float_operation {
                                    return trap(format!(
                                        "named float operation `{operation}` produced NaN in Trapping domain"
                                    ));
                                }
                                let reason = if left.is_finite() && right.is_finite() {
                                    "invalid float operation in Trapping domain"
                                } else {
                                    "non-finite NaN result in Trapping domain"
                                };
                                return trap(reason.to_owned());
                            }
                            Err(FloatPolicyTrap::InfinityResult) => {
                                if let Some(operation) = named_float_operation {
                                    return trap(format!(
                                        "named float operation `{operation}` produced infinity in Trapping domain"
                                    ));
                                }
                                let reason = if division && right.is_zero() {
                                    "float division by zero in Trapping domain"
                                } else if left.is_finite() && right.is_finite() {
                                    "float overflow in Trapping domain"
                                } else {
                                    "non-finite infinity result in Trapping domain"
                                };
                                return trap(reason.to_owned());
                            }
                        }
                    }
                    _ => meaning,
                };
                Ok(self.semantic_float_result(meaning, format))
            };
            Ok(match operator {
                Add => return arith(FloatSemantics::add(format, &left, &right), false),
                Subtract => return arith(FloatSemantics::subtract(format, &left, &right), false),
                Multiply => return arith(FloatSemantics::multiply(format, &left, &right), false),
                Divide => return arith(FloatSemantics::divide(format, &left, &right), true),
                Less => Value::Bool(FloatSemantics::less(&left, &right)),
                LessOrEqual => Value::Bool(FloatSemantics::less_or_equal(&left, &right)),
                Greater => Value::Bool(FloatSemantics::greater(&left, &right)),
                GreaterOrEqual => Value::Bool(FloatSemantics::greater_or_equal(&left, &right)),
                Modulo | ShiftLeft | ShiftRight | BitwiseAnd | BitwiseOr | BitwiseXor => {
                    return unsupported("float modulo/shift/bitwise not supported");
                }
                Equal => Value::Bool(FloatSemantics::equal(&left, &right)),
                NotEqual => Value::Bool(!FloatSemantics::equal(&left, &right)),
                CaseMembership => return trap("case membership requires a nominal sum subject"),
                And | Or => unreachable!("logical operations have Boolean operands"),
            })
        }

        pub(crate) fn eval_float_extremum(
            &self,
            builtin: BuiltinFunction,
            left: Value,
            right: Value,
        ) -> EvalResult<Value> {
            let left_meaning = left.float_meaning().ok_or_else(|| {
                Halt::Trap("floating extremum requires a float operand".to_owned())
            })?;
            let right_meaning = right.float_meaning().ok_or_else(|| {
                Halt::Trap("floating extremum requires a float operand".to_owned())
            })?;
            let pick_left = match builtin {
                BuiltinFunction::FloatMaximum => {
                    FloatSemantics::greater(&left_meaning, &right_meaning)
                }
                BuiltinFunction::FloatMinimum => {
                    FloatSemantics::less(&left_meaning, &right_meaning)
                }
                _ => return unsupported("not an admitted floating extremum"),
            };
            Ok(if pick_left { left } else { right })
        }

        fn values_equal(&self, left: &Value, right: &Value) -> EvalResult<bool> {
            Ok(match (left, right) {
                (
                    Value::Enum {
                        variant_name: a, ..
                    },
                    Value::Enum {
                        variant_name: b, ..
                    },
                ) => a == b,
                (
                    Value::Int(tag),
                    Value::Enum {
                        type_symbol,
                        variant_name,
                        ..
                    },
                )
                | (
                    Value::Enum {
                        type_symbol,
                        variant_name,
                        ..
                    },
                    Value::Int(tag),
                ) => self.enum_variant_tag(*type_symbol, variant_name) == Some(*tag),
                (Value::Str(a), Value::Str(b)) => *a.borrow() == *b.borrow(),
                (Value::Array(elements), Value::Str(text))
                | (Value::Str(text), Value::Array(elements)) => {
                    let bytes = text.borrow();
                    if elements.len() != bytes.len() {
                        return Ok(false);
                    }
                    for (element, byte) in elements.iter().zip(bytes.iter()) {
                        let value = byte_sequence_element(element)?;
                        if value != *byte {
                            return Ok(false);
                        }
                    }
                    true
                }
                (Value::Array(left), Value::Array(right)) => {
                    if left.len() != right.len() {
                        return Ok(false);
                    }
                    for (left, right) in left.iter().zip(right) {
                        if byte_sequence_element(left)? != byte_sequence_element(right)? {
                            return Ok(false);
                        }
                    }
                    true
                }
                (Value::Bool(a), Value::Bool(b)) => a == b,
                _ => {
                    if left.is_float() || right.is_float() {
                        let (Some(left), Some(right)) =
                            (left.float_meaning(), right.float_meaning())
                        else {
                            return unsupported(
                                "equality has a non-numeric floating-point operand",
                            );
                        };
                        FloatSemantics::equal(&left, &right)
                    } else {
                        let (Some(left), Some(right)) = (left.as_int(), right.as_int()) else {
                            return unsupported("equality operands have no supported comparison");
                        };
                        left == right
                    }
                }
            })
        }

        fn arithmetic_operand_int(&self, value: &Value) -> EvalResult<i64> {
            if let Some(int) = value.as_int() {
                return Ok(int);
            }
            if let Value::Enum {
                type_symbol,
                variant_name,
                payload,
            } = value
                && payload.is_empty()
                && let Some(tag) = self.enum_variant_tag(*type_symbol, variant_name)
            {
                return Ok(tag);
            }
            Err(Halt::Trap("non-integer operand".to_owned()))
        }

        fn enum_variant_tag(&self, type_symbol: SymbolHandle, variant_name: &str) -> Option<i64> {
            let ordinal_in = |data: &DataDefinition| {
                let mut ordinal: i64 = 0;
                for member in self.program.data_members(data) {
                    if let DataMember::Variant(variant) = member {
                        if variant.name.as_str() == variant_name {
                            return Some(ordinal);
                        }
                        ordinal += 1;
                    }
                }
                None
            };
            if type_symbol.is_valid() {
                let data = self.program.data_definition_by_symbol(type_symbol)?;
                return ordinal_in(data);
            }
            self.program.data_definitions().iter().find_map(ordinal_in)
        }
    }
}

/*
    Pure scalar plans describe payload computation; computation graphs additionally
    retain call, selection, and semantic-cast occurrences. A same-carrier cast can
    still change declared meaning, so its source binding belongs to the graph,
    even when the payload tree could otherwise fold it away. Independent lowering
    replays that occurrence before emitting the qualification-change edge.
    Call operands are collected independently of the enclosing result category:
    scalar arguments still need exact occurrences when the callee returns custody.
    Recording operands does not admit the call or establish its result frontier.

    `expression_plans.rs` drives the plan walk; `scalar_lowering.rs`,
    `boolean_lowering.rs`, `call_lowering.rs` and
    `machine_parameter_booleans.rs` lower each expression family, and
    `subslice_endpoints.rs` lowers range endpoints for every site that narrows
    a view, and
    `expression_facts.rs` answers the questions they share. The remaining
    modules own array constructions, call arguments, case membership,
    constant array projection, contract entries, primitive
    reference reads, result contracts, selected-operator operands, semantic
    casts and structural fields. The computation graph builder itself lives
    in `execution::scalar_computations`, the one plan builder producing
    scalar and Unit computations; these members stay `pub(crate)` so that
    builder consumes the same lowering utilities the pure plans use.
*/

pub(crate) mod array_constructions {
    /*
        Array actuals use the captured call occurrence and authored formal position.
        Discovery does not schedule evaluation or create a synthetic call. Pure and
        computed leaves share this roster after flow capture, while statement-owned
        arrays retain their existing pre-flow value production.
    */
    use checked_trees::{CheckedArrayConstructionSource, FlowFacts, FlowStateFact};
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::statement::StatementNode;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Clone, Copy)]
    pub(crate) struct CallArrayConstruction {
        pub source: CheckedArrayConstructionSource,
        pub expression: ExpressionHandle,
        pub type_reference: TypeReferenceHandle,
    }

    /*
        The one flow fact captured for `state`; `None` when there is none or more
        than one. A per-statement walk resolves it once per state rather than
        rescanning every state fact at each statement.
    */
    pub(crate) fn unique_state_flow<'flow>(
        flow: &'flow FlowFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> Option<&'flow FlowStateFact> {
        flow.control.unique_state(machine.symbol, state.symbol)
    }

    /*
        Array arguments discovered within the unique flow fact already resolved
        by the caller.
    */
    pub(crate) fn call_array_constructions_in(
        program: &dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        captured: &FlowStateFact,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
    ) -> Vec<CallArrayConstruction> {
        let Some(calls) = flow.control.calls.span(captured.calls) else {
            return Vec::new();
        };
        let mut output = Vec::new();
        for call in calls
            .iter()
            .filter(|call| call.statement_index == statement_index)
        {
            if calls
                .iter()
                .filter(|other| {
                    other.statement_index == statement_index
                        && other.call_ordinal == call.call_ordinal
                })
                .count()
                != 1
            {
                continue;
            }
            let Some(site) = validation::find_call_site(
                program,
                machine.symbol,
                state.symbol,
                statement_index,
                call.call_ordinal,
            ) else {
                continue;
            };
            let target_symbol = match &site {
                validation::CallSite::Expression { call, .. } => {
                    validation::call_target_symbol(program, call)
                }
                validation::CallSite::Statement(call) => call.target_symbol,
                validation::CallSite::TransitionNamed { path, .. } => path.symbol,
            };
            if validation::call_target_entry_symbol(program, target_symbol) != call.target_symbol {
                continue;
            }
            let Some(parameters) = validation::call_target_parameters(program, target_symbol)
            else {
                continue;
            };
            let return_type = match site {
                validation::CallSite::TransitionNamed { .. } => {
                    validation::terminal_callable_target(program, machine, target_symbol)
                        .map(|(_, return_type)| return_type)
                }
                _ => validation::call_target_return_type(program, target_symbol),
            };
            let Some(return_type) = return_type else {
                continue;
            };
            let direct_root = match program.statement_table.statement_at(state, statement_index) {
                Some(StatementNode::Call(_)) => {
                    matches!(site, validation::CallSite::Statement(_))
                }
                Some(StatementNode::LocalData(local)) => {
                    call.authored_expression == local.initial_value
                }
                Some(StatementNode::Assignment(assignment)) => {
                    super::call_lowering::call_is_boundary(program, target_symbol)
                        && call.authored_expression == assignment.value
                }
                Some(StatementNode::Expression(expression)) => {
                    call.authored_expression == *expression
                }
                Some(StatementNode::Transition(_)) => {
                    matches!(site, validation::CallSite::TransitionNamed { .. })
                }
                _ => false,
            };
            if !direct_root && program.primitive_type_reference(return_type).is_some() {
                continue;
            }
            let arguments = validation::call_site_argument_expressions(program, &site);
            let explicit_parameters = parameters.iter().enumerate();
            if parameters.len() != arguments.len() {
                continue;
            }
            for ((position, parameter), expression) in explicit_parameters.zip(arguments) {
                if (parameter.is_const)
                    || parameter.is_mutable
                    || !validation::is_closed_primitive_array_type(
                        program,
                        parameter.type_reference,
                    )
                    || validation::scalar_array_elements(
                        program,
                        machine.symbol,
                        *expression,
                        parameter.type_reference,
                    )
                    .is_none()
                {
                    continue;
                }
                let (Ok(call_ordinal), Ok(parameter_position)) =
                    (u32::try_from(call.call_ordinal), u32::try_from(position))
                else {
                    continue;
                };
                output.push(CallArrayConstruction {
                    source: CheckedArrayConstructionSource::CallArgument {
                        call_ordinal,
                        parameter_position,
                    },
                    expression: *expression,
                    type_reference: parameter.type_reference,
                });
            }
        }
        output
    }
}
pub(crate) mod atomic_operands {
    /*
        Scalar rows for the authored operands of one atomic carrier assignment.

        A writing atomic is spelled `place = Atomic { model }`, where the model is
        arithmetic over the observed prior (see `validation::atomic_assignment_carrier`).
        The model itself is not a value any execution evaluates: the event's
        operands are. Each authored operand therefore gets its own scalar row at the
        carrier statement under `AtomicOperand { operand_ordinal }`, lowered at the
        atomic place's own carrier, and the ordinary `AssignmentValue` row the model
        would otherwise produce is never retained. Lowering rejoins each row to the
        operand expression the same decoder names.
    */
    use crate::values::scalar::expression_plans::ScalarLocal;
    use crate::values::scalar::scalar_lowering::lower_return_expression;
    use checked_trees::{
        CheckedOperatorFacts, CheckedScalarExpression, CheckedScalarExpressionRole,
    };
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::StateParameter;
    use typed_trees::types::PrimitiveType;

    /* Both writing statements and structural-result locals share operand rows. */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_carrier(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        carrier: validation::AtomicCarrierAccess,
        scalar_parameters: &[StateParameter],
        parameters: &[StateParameter],
        parameter_types: &[PrimitiveType],
        locals: &[ScalarLocal],
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> Option<
        Vec<(
            ExpressionHandle,
            CheckedScalarExpressionRole,
            CheckedScalarExpression,
        )>,
    > {
        let primitive_type =
            validation::declared_place_type_raw(program, machine, Some(state), carrier.place)
                .and_then(|declared| program.primitive_type_reference(declared));
        let mut rows = Vec::new();
        for (ordinal, operand) in carrier.operands.iter().copied().enumerate() {
            let (Some(primitive_type), Ok(operand_ordinal)) =
                (primitive_type, u32::try_from(ordinal))
            else {
                break;
            };
            if let Some(value) = lower_return_expression(
                program,
                operators,
                operand,
                scalar_parameters,
                parameters,
                parameter_types,
                locals,
                primitive_type,
                exact_integer_casts,
            ) {
                rows.push((
                    operand,
                    CheckedScalarExpressionRole::AtomicOperand { operand_ordinal },
                    value,
                ));
            }
        }
        Some(rows)
    }
}
pub(crate) mod boolean_lowering {
    /*
        Lowering Boolean expressions and guards, including closed evaluation and
        integer comparisons.
    */
    use super::scalar_lowering::scalar_storage_read;
    use crate::values::scalar::case_membership;
    use crate::values::scalar::constant_array_projection;
    use crate::values::scalar::expression_facts::{
        is_integer, local_position, operator_is_builtin, parameter_position, scalar_expression_type,
    };
    use crate::values::scalar::expression_plans::{
        ScalarLocal, lower_closed_integer_literal_guard,
    };
    use crate::values::scalar::primitive_reference_read;
    use crate::values::scalar::scalar_lowering::{
        land_contextual_integer_literal, lower_scalar_operands,
    };
    use crate::values::scalar::structural_fields;
    use checked_trees::{
        CheckedBooleanExpression as BooleanExpression,
        CheckedIntegerComparisonKind as ComparisonKind, CheckedOperatorFacts,
        CheckedScalarExpression as ScalarExpression,
    };
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
    };
    use typed_trees::signature::StateParameter;
    use typed_trees::types::PrimitiveType;

    /*
        Recover a normal-return Boolean using the same selected operations as
        scalar computation folding, without assumptions about any runtime binding.
        Callers still evaluate the expression's children: a known result (such as a
        fixed extent) does not establish that evaluating its source is effect-free.
    */
    pub(crate) fn evaluate_closed_boolean_expression(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        expression: ExpressionHandle,
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> Option<bool> {
        let selected = lower_boolean_expression(
            program,
            operators,
            expression,
            &[],
            &[],
            &[],
            &[],
            exact_integer_casts,
        )?;
        let facts::ScalarValue::Boolean(value) = crate::values::evaluate_checked_scalar(
            &ScalarExpression::Boolean(Box::new(selected)),
            &mut |_| None,
        )?
        else {
            return None;
        };
        Some(value)
    }

    macro_rules! lower_boolean_entry {
        ($name:ident, $walk:ident) => {
            pub(crate) fn $name(
                program: &dyn ::validation::CheckingInput,
                operators: &CheckedOperatorFacts,
                expression: ExpressionHandle,
                parameters: &[StateParameter],
                authored_parameters: &[StateParameter],
                parameter_types: &[PrimitiveType],
                locals: &[ScalarLocal],
                exact_integer_casts: &[validation::ExactIntegerCastFact],
            ) -> Option<BooleanExpression> {
                BooleanLowering::new(
                    program,
                    operators,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )
                .$walk(expression)
            }
        };
    }

    lower_boolean_entry!(lower_boolean_expression, expression);

    /*
        The recursive Boolean walk keeps the same invocation checking input and
        borrowed scalar namespace. Authored coordinates remain separate from dense
        scalar positions; guard normalization and admission keep their existing order.
    */
    struct BooleanLowering<'a> {
        program: &'a dyn ::validation::CheckingInput,
        operators: &'a CheckedOperatorFacts,
        parameters: &'a [StateParameter],
        authored_parameters: &'a [StateParameter],
        parameter_types: &'a [PrimitiveType],
        locals: &'a [ScalarLocal],
        exact_integer_casts: &'a [validation::ExactIntegerCastFact],
    }

    impl<'a> BooleanLowering<'a> {
        fn new(
            program: &'a dyn ::validation::CheckingInput,
            operators: &'a CheckedOperatorFacts,
            parameters: &'a [StateParameter],
            authored_parameters: &'a [StateParameter],
            parameter_types: &'a [PrimitiveType],
            locals: &'a [ScalarLocal],
            exact_integer_casts: &'a [validation::ExactIntegerCastFact],
        ) -> Self {
            Self {
                program,
                operators,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            }
        }
    }

    impl BooleanLowering<'_> {
        fn expression(&self, expression: ExpressionHandle) -> Option<BooleanExpression> {
            if self
                .program
                .static_type_predicate_operands(expression)
                .is_some()
            {
                return self
                    .program
                    .static_type_predicate_value(expression)
                    .map(BooleanExpression::Constant);
            }
            if let Some(ScalarExpression::Boolean(read)) = primitive_reference_read::lower(
                self.program,
                self.authored_parameters,
                expression,
                PrimitiveType::Bool,
            ) {
                return Some(*read);
            }
            if let Some(read) = crate::values::scalar::scalar_lowering::lower_recast_boolean_operand(
                self.program,
                self.operators,
                expression,
                self.parameters,
                self.authored_parameters,
                self.parameter_types,
                self.locals,
                self.exact_integer_casts,
            ) {
                return Some(read);
            }
            if let Some(membership) =
                case_membership::lower(self.program, self.authored_parameters, expression)
            {
                return Some(membership);
            }
            if let Some((leaf, PrimitiveType::Bool)) =
                validation::closed_record_scalar_projection(self.program, expression)
            {
                let value = self.program.expression_table.boolean_node(leaf)?;
                return Some(BooleanExpression::Constant(*value));
            }
            if let Some(leaf) = constant_array_projection::selected_leaf(
                self.program,
                self.operators,
                self.authored_parameters,
                expression,
            ) {
                return self.expression(leaf);
            }
            if let Some((ScalarExpression::Boolean(field), _)) = scalar_storage_read(
                self.program,
                self.operators,
                expression,
                self.parameters,
                self.authored_parameters,
                self.parameter_types,
                self.locals,
                self.exact_integer_casts,
            ) {
                return Some(*field);
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Boolean(value) => Some(BooleanExpression::Constant(*value)),
                ExpressionNode::Name(path) => {
                    if let Some(position) = parameter_position(self.program, path, self.parameters)
                    {
                        let parameter = &self.parameters[position];
                        if parameter.is_mutable {
                            return (crate::values::mutable_scalar_parameter_type(
                                self.program,
                                parameter,
                            ) == Some(PrimitiveType::Bool)
                                && path.symbol == parameter.symbol
                                && path.head_symbol == parameter.symbol)
                                .then_some(BooleanExpression::StorageRead {
                                    symbol: parameter.symbol,
                                });
                        }
                        return (self.parameter_types[position] == PrimitiveType::Bool)
                            .then_some(BooleanExpression::Parameter { position });
                    }
                    let local_position =
                        local_position(self.program, expression, path, self.locals)?;
                    let local = &self.locals[local_position];
                    if local.is_mutable || local.is_reference {
                        return (local.primitive_type == PrimitiveType::Bool).then_some(
                            BooleanExpression::StorageRead {
                                symbol: local.symbol,
                            },
                        );
                    }
                    let position = self.parameters.len().checked_add(
                        self.locals[..local_position]
                            .iter()
                            .filter(|local| local.is_value_binding())
                            .count(),
                    )?;
                    (self.locals[local_position].primitive_type == PrimitiveType::Bool)
                        .then_some(BooleanExpression::Local { position })
                }
                ExpressionNode::Unary(unary)
                    if unary.operator == UnaryOperator::LogicalNot
                        && operator_is_builtin(self.operators, expression) =>
                {
                    Some(BooleanExpression::Not(Box::new(
                        self.expression(unary.operand)?,
                    )))
                }
                ExpressionNode::Binary(binary)
                    if binary.operator.is_comparison()
                        && operator_is_builtin(self.operators, expression) =>
                {
                    let integer_comparison = (|| {
                        let ((left, _), (right, _)) = lower_scalar_operands(
                            self.program,
                            self.operators,
                            binary,
                            self.parameters,
                            self.authored_parameters,
                            self.parameter_types,
                            self.locals,
                            self.exact_integer_casts,
                        )?;
                        construct_integer_comparison(binary.operator, left, right)
                    })();
                    if !matches!(
                        binary.operator,
                        BinaryOperator::Equal | BinaryOperator::NotEqual
                    ) || integer_comparison.is_some()
                    {
                        return integer_comparison;
                    }
                    if let Some(equality) = byte_carrier_literal_equality(
                        self.program,
                        binary,
                        self.authored_parameters,
                    )
                    .or_else(|| {
                        byte_carrier_pair_equality(self.program, binary, self.authored_parameters)
                    }) {
                        return Some(if binary.operator == BinaryOperator::NotEqual {
                            BooleanExpression::Not(Box::new(equality))
                        } else {
                            equality
                        });
                    }
                    let equality = BooleanExpression::Equal {
                        left: Box::new(self.expression(binary.left)?),
                        right: Box::new(self.expression(binary.right)?),
                    };
                    Some(if binary.operator == BinaryOperator::NotEqual {
                        BooleanExpression::Not(Box::new(equality))
                    } else {
                        equality
                    })
                }
                ExpressionNode::Binary(binary)
                    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                        && operator_is_builtin(self.operators, expression) =>
                {
                    let left = Box::new(self.expression(binary.left)?);
                    let right = Box::new(self.expression(binary.right)?);
                    Some(if binary.operator == BinaryOperator::And {
                        BooleanExpression::And { left, right }
                    } else {
                        BooleanExpression::Or { left, right }
                    })
                }
                _ => None,
            }
        }

        fn guard(&self, expression: ExpressionHandle) -> Option<BooleanExpression> {
            if let Some(value) =
                lower_closed_integer_literal_guard(self.program, self.operators, expression)
            {
                return Some(value);
            }
            let Some(binary) = self.program.expression_table.binary_node(expression) else {
                return self.expression(expression);
            };
            if binary.operator == BinaryOperator::Equal
                && operator_is_builtin(self.operators, expression)
            {
                match (
                    self.program.expression_table.expression(binary.left),
                    self.program.expression_table.expression(binary.right),
                ) {
                    (ExpressionNode::Boolean(true), _) => {
                        return self.expression(binary.right);
                    }
                    (_, ExpressionNode::Boolean(true)) => {
                        return self.expression(binary.left);
                    }
                    _ => {}
                }
            }
            self.expression(expression)
        }
    }

    /*
        `carrier == "literal"` admits no scalar operand pair: content equality is
        the carrier's live length plus each literal byte at its fixed index. The
        fold keeps the length comparison leftmost so short-circuit evaluation
        dominates every indexed read, and each read's `index < length` obligation
        is the true-path fact of that comparison.

        Fixed arrays observe their declared extent; byte views observe the extent
        in their descriptor. Qualification does not change either geometry. The
        same observations apply to roots, fields and established immutable locals.
        Write-only views grant no byte observations. Non-byte element views do not
        have the byte geometry required by this comparison.
    */
    fn byte_carrier_literal_equality(
        program: &dyn ::validation::CheckingInput,
        binary: &typed_trees::expression::TableBinaryExpression,
        authored_parameters: &[StateParameter],
    ) -> Option<BooleanExpression> {
        for (carrier, literal) in [(binary.left, binary.right), (binary.right, binary.left)] {
            let Some(bytes) = typed_trees::byte_predicates::literal_bytes(program, literal) else {
                continue;
            };
            let Some((root, path, geometry)) =
                comparable_byte_carrier(program, authored_parameters, carrier)
            else {
                continue;
            };
            let maximum_length = match geometry {
                validation::ByteObservationCarrier::FixedArray { length } => Some(length),
                validation::ByteObservationCarrier::Sequence(
                    checked_trees::CheckedByteSequenceCarrier::BoundedView { capacity, .. }
                    | checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity },
                ) => Some(capacity),
                _ => None,
            };
            return BooleanExpression::byte_literal_equality_in_extent(
                root,
                path,
                &bytes,
                maximum_length,
            );
        }
        None
    }

    /*
        `left == right` over two live byte carriers is content equality. Two
        fixed arrays compare their exact extents and bytes. Other byte carriers
        retain their own explicit length observations. Read permissions and typed
        paths select the actual stored values, including below record fields.
    */
    fn byte_carrier_pair_equality(
        program: &dyn ::validation::CheckingInput,
        binary: &typed_trees::expression::TableBinaryExpression,
        authored_parameters: &[StateParameter],
    ) -> Option<BooleanExpression> {
        let left = comparable_byte_carrier(program, authored_parameters, binary.left)?;
        let right = comparable_byte_carrier(program, authored_parameters, binary.right)?;
        let ((left_root, left_path, left_carrier), (right_root, right_path, right_carrier)) =
            (left, right);
        let capacities = match (left_carrier, right_carrier) {
            (
                validation::ByteObservationCarrier::FixedArray { length: left },
                validation::ByteObservationCarrier::FixedArray { length: right },
            ) => Some((left, right)),
            (
                validation::ByteObservationCarrier::Sequence(
                    checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity: left },
                ),
                validation::ByteObservationCarrier::Sequence(
                    checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity: right },
                ),
            ) => Some((left, right)),
            _ => None,
        };
        let Some((left_capacity, right_capacity)) = capacities else {
            return Some(BooleanExpression::ByteSequenceEqual {
                left: checked_trees::CheckedByteSequenceOperand {
                    root: left_root,
                    path: left_path,
                },
                right: checked_trees::CheckedByteSequenceOperand {
                    root: right_root,
                    path: right_path,
                },
            });
        };
        let length = |root, path: &Vec<_>| {
            Box::new(ScalarExpression::StructuralSequenceLength {
                root,
                path: path
                    .iter()
                    .cloned()
                    .map(checked_trees::CheckedScalarReadProjection::Static)
                    .collect(),
            })
        };
        let read = |root, path: &Vec<_>, index| {
            Some(Box::new(ScalarExpression::StructuralParameterIndexedRead {
                observation: ExpressionHandle::invalid(),
                root,
                path: path.clone(),
                index: Box::new(checked_trees::CheckedScalarSelector {
                    expression: integer_literal(index, PrimitiveType::U64)?,
                    occurrence: ExpressionHandle::invalid(),
                    entry_bound: None,
                }),
                element_path: Vec::new(),
                primitive_type: PrimitiveType::U8,
            }))
        };
        let mut remaining = None;
        for index in (0..left_capacity.min(right_capacity)).rev() {
            let agreeing = BooleanExpression::And {
                left: Box::new(BooleanExpression::IntegerComparison {
                    kind: ComparisonKind::LessThan,
                    left: Box::new(integer_literal(index, PrimitiveType::U64)?),
                    right: length(right_root, &right_path),
                }),
                right: Box::new(BooleanExpression::IntegerComparison {
                    kind: ComparisonKind::Equal,
                    left: read(left_root, &left_path, index)?,
                    right: read(right_root, &right_path, index)?,
                }),
            };
            let continued = match remaining {
                Some(remaining) => BooleanExpression::And {
                    left: Box::new(agreeing),
                    right: Box::new(remaining),
                },
                None => agreeing,
            };
            remaining = Some(BooleanExpression::Or {
                left: Box::new(BooleanExpression::IntegerComparison {
                    kind: ComparisonKind::LessOrEqual,
                    left: length(left_root, &left_path),
                    right: Box::new(integer_literal(index, PrimitiveType::U64)?),
                }),
                right: Box::new(continued),
            });
        }
        let lengths = BooleanExpression::IntegerComparison {
            kind: ComparisonKind::Equal,
            left: length(left_root, &left_path),
            right: length(right_root, &right_path),
        };
        Some(match remaining {
            Some(remaining) => BooleanExpression::And {
                left: Box::new(lengths),
                right: Box::new(remaining),
            },
            None => lengths,
        })
    }

    /*
        The storage root, path and carrier of a byte carrier whose live length and
        bytes a content comparison may observe, or `None` when `carrier` names no
        such place.
    */
    pub(crate) fn comparable_byte_carrier(
        program: &dyn ::validation::CheckingInput,
        authored_parameters: &[StateParameter],
        carrier: ExpressionHandle,
    ) -> Option<(
        checked_trees::CheckedStorageRoot,
        Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
        validation::ByteObservationCarrier,
    )> {
        let (root, path, type_reference) =
            structural_fields::structural_storage_place(program, authored_parameters, carrier)?;
        let carrier = validation::comparable_byte_carrier_type(program, type_reference)?;
        Some((root, path, carrier))
    }

    fn integer_literal(value: u64, primitive_type: PrimitiveType) -> Option<ScalarExpression> {
        facts::u64_integer_literal(value, primitive_type)
    }

    /*
        Comparison normalization may swap completed values for `>` and `>=`; it
        does not change the source operand evaluation order retained by the caller.
    */
    pub(crate) fn construct_integer_comparison(
        operator: BinaryOperator,
        mut left: ScalarExpression,
        mut right: ScalarExpression,
    ) -> Option<BooleanExpression> {
        match (
            scalar_expression_type(&left),
            scalar_expression_type(&right),
        ) {
            (Some(primitive_type), None) => {
                right = land_contextual_integer_literal(right, primitive_type)?;
            }
            (None, Some(primitive_type)) => {
                left = land_contextual_integer_literal(left, primitive_type)?;
            }
            (Some(_), Some(_)) => {}
            (None, None) => return None,
        }
        let left_type = scalar_expression_type(&left)?;
        let right_type = scalar_expression_type(&right)?;
        if !is_integer(left_type)
            || !is_integer(right_type)
            || (left_type != right_type
                && (left_type == PrimitiveType::Addr || right_type == PrimitiveType::Addr))
        {
            return None;
        }
        let (kind, negated) = match operator {
            BinaryOperator::Equal => (ComparisonKind::Equal, false),
            BinaryOperator::NotEqual => (ComparisonKind::Equal, true),
            BinaryOperator::Less => (ComparisonKind::LessThan, false),
            BinaryOperator::LessOrEqual => (ComparisonKind::LessOrEqual, false),
            BinaryOperator::Greater => {
                std::mem::swap(&mut left, &mut right);
                (ComparisonKind::LessThan, false)
            }
            BinaryOperator::GreaterOrEqual => {
                std::mem::swap(&mut left, &mut right);
                (ComparisonKind::LessOrEqual, false)
            }
            _ => return None,
        };
        let comparison = BooleanExpression::IntegerComparison {
            kind,
            left: Box::new(left),
            right: Box::new(right),
        };
        Some(if negated {
            BooleanExpression::Not(Box::new(comparison))
        } else {
            comparison
        })
    }

    lower_boolean_entry!(lower_boolean_guard, guard);
}
pub(crate) mod call_arguments {
    /*
        Nested call operands retain captured coordinates and source scope independently
        of result shape. The consuming planner owns result eligibility and custody.
    */
    use super::call_lowering::value_call_expression;
    use crate::execution::terminal_unit::control::checked_structural_result_type;
    use crate::execution::terminal_unit::types::ShapeCollector;
    use crate::flow::expressions;
    use crate::flow::indexed_statements;
    use crate::values::scalar::array_constructions::call_array_constructions_in;
    use crate::values::scalar::array_constructions::unique_state_flow;
    use crate::values::scalar::call_lowering::lower_call_arguments;
    use crate::values::scalar::call_lowering::retain_call_arguments;
    use crate::values::scalar::expression_plans::ScalarLocal;
    use crate::values::scalar::scalar_lowering::lower_return_expression;
    use crate::values::scalar::scalar_parameter_roster;
    use checked_trees::CheckedLocatedScalarExpression;
    use checked_trees::CheckedOperatorFacts;
    use checked_trees::CheckedScalarExpressionBindings;
    use checked_trees::CheckedScalarExpressionPlans;
    use checked_trees::CheckedScalarExpressionRole;
    use checked_trees::FlowFacts;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::statement::StatementNode;
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) fn is_scalar_return_call(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(primitive_type) = program.primitive_type_reference(state.return_type) else {
            return false;
        };
        if !matches!(program.statement_table.statements(state.statement_nodes).last(),
            Some(StatementNode::Expression(root)) if *root == expression)
        {
            return false;
        }
        let Some(call) = program.expression_table.call_node(expression) else {
            return false;
        };
        let target = validation::call_target_symbol(program, call);
        validation::call_target_return_type(program, target).is_some_and(|return_type| {
            program.primitive_type_reference(return_type) == Some(primitive_type)
        })
    }
    pub(crate) fn retain_nested_call_arguments(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        flow: &FlowFacts,
        plans: &mut CheckedScalarExpressionPlans,
        proof_terms: &mut Vec<checked_trees::CheckedLocatedProofTerm>,
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) {
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let (parameters, parameter_types) =
                    scalar_parameter_roster(program, program.state_parameters(state))
                        .expect("filtered scalar parameters retain primitive carriers");
                let mut locals = Vec::new();
                let captured = unique_state_flow(flow, machine, state);
                for (statement_index, statement) in
                    indexed_statements(program, state.statement_nodes)
                {
                    let Ok(statement_ordinal) = u32::try_from(statement_index) else {
                        continue;
                    };
                    for construction in captured
                        .map(|captured| {
                            call_array_constructions_in(
                                program,
                                flow,
                                captured,
                                machine,
                                state,
                                statement_index,
                            )
                        })
                        .unwrap_or_default()
                    {
                        let Some(array) = validation::scalar_array_elements(
                            program,
                            machine.symbol,
                            construction.expression,
                            construction.type_reference,
                        ) else {
                            continue;
                        };
                        if !array.all_authored() {
                            continue;
                        }
                        for (element_index, element) in array.elements.into_iter().enumerate() {
                            let validation::ScalarArrayElement::Authored {
                                expression: element,
                                primitive_type,
                            } = element
                            else {
                                continue;
                            };
                            let Ok(element_ordinal) = u32::try_from(element_index) else {
                                break;
                            };
                            let Some(value) = lower_return_expression(
                                program,
                                operators,
                                element,
                                &parameters,
                                program.state_parameters(state),
                                &parameter_types,
                                &locals,
                                primitive_type,
                                exact_integer_casts,
                            ) else {
                                continue;
                            };
                            let role = CheckedScalarExpressionRole::ArrayElement {
                                source: construction.source,
                                element_ordinal,
                            };
                            plans
                                .source_bindings
                                .append(CheckedScalarExpressionBindings {
                                    destination: symbols::SymbolHandle::invalid(),
                                    state: state.symbol,
                                    statement_ordinal,
                                    role,
                                    expression: element,
                                    symbols: plans.binding_symbols.insert_many(
                                        parameters.iter().map(|parameter| parameter.symbol).chain(
                                            locals
                                                .iter()
                                                .filter(|local| local.is_value_binding())
                                                .map(|local| local.symbol),
                                        ),
                                    ),
                                });
                            plans.expressions.push(CheckedLocatedScalarExpression {
                                state: state.symbol,
                                statement_ordinal,
                                role,
                                expression: value,
                            });
                        }
                    }
                    for (call_ordinal, site) in captured
                        .map(|captured| {
                            nested_call_sites(program, flow, captured, statement, statement_index)
                        })
                        .unwrap_or_default()
                    {
                        if let Some(arguments) = lower_call_arguments(
                            program,
                            operators,
                            state,
                            statement_ordinal,
                            call_ordinal,
                            &site,
                            &parameters,
                            program.state_parameters(state),
                            &parameter_types,
                            &locals,
                            exact_integer_casts,
                        ) {
                            retain_call_arguments(
                                arguments,
                                &parameters,
                                &locals,
                                &mut plans.expressions,
                                proof_terms,
                                &mut plans.source_bindings,
                                &mut plans.binding_symbols,
                            );
                        }
                    }
                    if let Some(local) = statement.as_local_data() {
                        if let Some(primitive_type) =
                            program.primitive_type_reference(local.type_reference)
                        {
                            locals.push(ScalarLocal::from_local_data(
                                program,
                                local,
                                primitive_type,
                            ));
                        } else if let Some(binding) =
                            super::primitive_reference_read::local(program, local)
                        {
                            locals.push(binding);
                        }
                    }
                }
            }
        }
        plans.seal_expressions();
    }

    /*
        Read captured occurrences directly. In particular, skipped syntax does not
        acquire a flow call, and execution order never becomes occurrence identity.
        `source` is the state's unique flow fact, which the caller resolves once
        per state.
    */
    pub(crate) fn nested_call_sites<'program>(
        program: &'program dyn ::validation::CheckingInput,
        flow: &FlowFacts,
        source: &checked_trees::FlowStateFact,
        statement: &StatementNode,
        statement_index: usize,
    ) -> Vec<(usize, validation::CallSite<'program>)> {
        let Some(calls) = flow.control.calls.span(source.calls) else {
            return Vec::new();
        };
        let statement_root = match statement {
            StatementNode::Expression(expression) => Some(*expression),
            StatementNode::LocalData(local) => value_call_expression(program, local.initial_value),
            StatementNode::Assignment(assignment) => {
                value_call_expression(program, assignment.value)
            }
            _ => None,
        };
        calls
            .iter()
            .filter_map(|source| {
                if source.statement_index != statement_index
                    || (source.call_ordinal == 0
                        && statement_root == Some(source.authored_expression))
                    || !expressions(program).expression_is_valid(source.authored_expression)
                    || calls.iter().any(|other| {
                        !std::ptr::eq(source, other)
                            && other.statement_index == statement_index
                            && (other.call_ordinal == source.call_ordinal
                                || other.authored_expression == source.authored_expression)
                    })
                {
                    return None;
                }
                let ExpressionNode::Call(call) =
                    expressions(program).expression(source.authored_expression)
                else {
                    return None;
                };
                if validation::call_target_entry_symbol(
                    program,
                    validation::call_target_symbol(program, call),
                ) != source.target_symbol
                    || !call.selects_only_nominal_route()
                    || !call.evidence_arguments.is_empty()
                {
                    return None;
                }
                if source.has_receiver != call.receiver.is_valid()
                    || source.receiver_symbol
                        != if call.receiver.is_valid() {
                            let name = program.expression_table.name_path_node(call.receiver)?;
                            name.symbol
                        } else {
                            symbols::SymbolHandle::invalid()
                        }
                {
                    return None;
                }
                Some((
                    source.call_ordinal,
                    validation::CallSite::Expression {
                        expression: source.authored_expression,
                        call,
                    },
                ))
            })
            .collect()
    }

    /*
        One source eligibility judgment feeds pure/computed operands and structural
        scheduling. A checked producer plan and its exact result shape are joined
        later; this early selector creates no result or ownership evidence.
        Stored loans and nominal cleanup belong to the completed carrier and its
        consuming continuation, rather than a dynamic-only contents restriction.
    */
    pub(crate) fn nested_structural_call_return_type(
        program: &dyn ::validation::CheckingInput,
        caller: symbols::SymbolHandle,
        call: &typed_trees::expression::TableCallExpression,
    ) -> Option<TypeReferenceHandle> {
        if !call.target_symbol.is_valid()
            || !call.evidence_arguments.is_empty()
            || !call.selects_only_nominal_route()
        {
            return None;
        }
        let return_type = if let Some(slot) = program.dynamic_slot_by_symbol(call.target_symbol) {
            slot.return_type
        } else if let Some((owner, target)) =
            validation::find_machine_head(program, call.target_symbol)
        {
            let receiver_free_head =
                call.target_symbol == owner.symbol && !call.receiver.is_valid();
            if !call.carries_only_positional_arguments()
                || !(owner.supplies_checked_body() || owner.has_bodyless_signature())
                || !(receiver_free_head
                    || program.call_has_no_runtime_receiver(call, owner, target))
            {
                return None;
            }
            target.return_type
        } else {
            let signature = if let Some((owner, signature)) =
                program.machine_parameter_signature(call.target_symbol)
            {
                if owner.symbol != caller || !call.carries_only_positional_arguments() {
                    return None;
                }
                signature
            } else {
                program.requirement_call_signature(caller, call)?
            };
            let (declaration, _) = program.trait_machine_signature(signature.symbol)?;
            if !declaration.is_boundary {
                return None;
            }
            signature.return_type
        };
        checked_structural_result_type(
            program,
            &mut ShapeCollector::new(program),
            return_type,
            &[],
        )?;
        Some(return_type)
    }
}
pub(crate) mod call_lowering {
    /*
        Lowering call arguments into scalar plans: retained arguments, boundary
        calls, direct call bindings and qualified call expressions.
    */
    use crate::execution::terminal_unit::strips_erased_parameter;
    use crate::flow::expressions;
    use crate::values::mutable_scalar_parameter_type;
    use crate::values::scalar::expression_facts::is_integer;
    use crate::values::scalar::expression_facts::parameter_position;
    use crate::values::scalar::expression_plans::{ScalarLocal, located_binding_symbols};
    use crate::values::scalar::scalar_lowering::lower_return_expression;
    use checked_trees::{
        CheckedLocatedProofTerm, CheckedLocatedScalarExpression, CheckedMathematicalArgument,
        CheckedOperatorFacts, CheckedProofTerm, CheckedProofTermField, CheckedProofTermRole,
        CheckedScalarExpression, CheckedScalarExpressionBindings, CheckedScalarExpressionRole,
    };
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::literals::IntegerLanding;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::signature::StateParameter;
    use typed_trees::types::PrimitiveType;

    /*
        The arguments a call site retains: scalar-lane expressions plus the
        proof terms erased proof-only formals contribute to the contract lane.
    */
    pub(crate) struct LoweredCallArguments {
        pub(crate) scalar_arguments: Vec<(ExpressionHandle, CheckedLocatedScalarExpression)>,
        pub(crate) proof_terms: Vec<CheckedLocatedProofTerm>,
    }

    pub(crate) fn retain_call_arguments(
        arguments: LoweredCallArguments,
        parameters: &[StateParameter],
        locals: &[ScalarLocal],
        expressions: &mut Vec<CheckedLocatedScalarExpression>,
        proof_terms: &mut Vec<CheckedLocatedProofTerm>,
        source_bindings: &mut arena::Arena<CheckedScalarExpressionBindings>,
        binding_symbols: &mut arena::Arena<symbols::SymbolHandle>,
    ) {
        proof_terms.extend(arguments.proof_terms);
        for (authored_argument, argument) in arguments.scalar_arguments {
            source_bindings.append(CheckedScalarExpressionBindings {
                destination: symbols::SymbolHandle::invalid(),
                state: argument.state,
                statement_ordinal: argument.statement_ordinal,
                role: argument.role,
                expression: authored_argument,
                symbols: binding_symbols.insert_many(located_binding_symbols(parameters, locals)),
            });
            expressions.push(argument);
        }
    }

    pub(crate) fn call_is_boundary(
        program: &dyn ::validation::CheckingInput,
        target_symbol: symbols::SymbolHandle,
    ) -> bool {
        let requirement_symbol = program
            .machine_parameter_signature(target_symbol)
            .map(|(_, signature)| signature.symbol)
            .unwrap_or(target_symbol);
        let requirement_symbol = program
            .closed_requirement_application(requirement_symbol)
            .map_or(requirement_symbol, |application| application.template);
        program.machines().iter().any(|machine| {
            machine.has_bodyless_signature()
                && program
                    .machine_states(machine)
                    .iter()
                    .any(|candidate| candidate.symbol == target_symbol)
        }) || program.traits().iter().any(|definition| {
            definition.is_boundary
                && program
                    .trait_machine_signature_by_symbol(definition, requirement_symbol)
                    .is_some()
        }) || program
            .operators()
            .iter()
            .any(|operator| operator.is_boundary && operator.symbol == target_symbol)
            || validation::exact_external_leaf_boundary_requirement(program, target_symbol)
                .is_some()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_call_arguments(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        state: &typed_trees::state::State,
        statement_ordinal: u32,
        call_ordinal: usize,
        call_site: &validation::CallSite<'_>,
        parameters: &[StateParameter],
        authored_parameters: &[StateParameter],
        parameter_types: &[PrimitiveType],
        locals: &[ScalarLocal],
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> Option<LoweredCallArguments> {
        let target_symbol = match call_site {
            validation::CallSite::Statement(call) => call.target_symbol,
            validation::CallSite::Expression { call, .. } => {
                validation::call_target_symbol(program, call)
            }
            validation::CallSite::TransitionNamed { .. } => return None,
        };
        let is_boundary = call_is_boundary(program, target_symbol);

        if let Some(operation) = program
            .symbols
            .builtin_function_for_symbol(target_symbol)
            .and_then(|builtin| {
                language_core::inline_assembly::AsmRegisterOperation::from_intrinsic_name(
                    builtin.name(),
                )
            })
        {
            let arguments = validation::call_site_argument_expressions(program, call_site);
            if arguments.len() != operation.input_carriers().len() {
                return None;
            }
            let mut scalar_arguments = Vec::new();
            for (ordinal, (argument, carrier)) in
                arguments.iter().zip(operation.input_carriers()).enumerate()
            {
                if let Some(expression) = lower_return_expression(
                    program,
                    operators,
                    *argument,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    PrimitiveType::from_name(carrier.name())?,
                    exact_integer_casts,
                ) {
                    scalar_arguments.push((
                        *argument,
                        CheckedLocatedScalarExpression {
                            state: state.symbol,
                            statement_ordinal,
                            role: CheckedScalarExpressionRole::UnitCallArgument {
                                call_ordinal: u32::try_from(call_ordinal).ok()?,
                                argument_ordinal: u32::try_from(ordinal).ok()?,
                            },
                            expression,
                        },
                    ));
                }
            }
            return Some(LoweredCallArguments {
                scalar_arguments,
                proof_terms: Vec::new(),
            });
        }

        let target_parameters = validation::call_target_parameters(program, target_symbol)?;
        let explicit_arguments = validation::call_site_argument_expressions(program, call_site);
        let mut explicit_index = 0usize;
        let mut scalar_index = 0usize;
        let mut scalar_erased_index = 0usize;
        let mut proof_erased_index = 0usize;
        let proof_only = validation::proof_only_classification(program);
        let mut proof_terms = Vec::new();
        let mut output = Vec::new();
        for target in target_parameters {
            let argument = *explicit_arguments.get(explicit_index)?;
            explicit_index = explicit_index.checked_add(1)?;
            if strips_erased_parameter(target)? {
                if !is_boundary {
                    let lanes = program.erased_scalar_lanes(target.type_reference);
                    if !lanes.is_empty() {
                        for lane in &lanes {
                            if let Some(lowered) = lower_erased_lane_argument(
                                state.symbol,
                                statement_ordinal,
                                program,
                                operators,
                                argument,
                                parameters,
                                authored_parameters,
                                parameter_types,
                                locals,
                                target.type_reference,
                                lane,
                                exact_integer_casts,
                            ) {
                                output.push((
                                    argument,
                                    CheckedLocatedScalarExpression {
                                        state: state.symbol,
                                        statement_ordinal,
                                        role: CheckedScalarExpressionRole::ErasedUnitCallArgument {
                                            call_ordinal: u32::try_from(call_ordinal).ok()?,
                                            erased_ordinal: u32::try_from(scalar_erased_index)
                                                .ok()?,
                                        },
                                        expression: lowered,
                                    },
                                ));
                            }
                            scalar_erased_index = scalar_erased_index.checked_add(1)?;
                        }
                    } else if proof_only.contract_term_carrier(program, target.type_reference)
                        || validation::mathematical_applications::is_source_evidence_carrier(
                            program,
                            target.type_reference,
                        )
                    {
                        if let Some(term) = lower_proof_term(
                            program,
                            state.symbol,
                            operators,
                            argument,
                            target.type_reference,
                            parameters,
                            authored_parameters,
                            parameter_types,
                            locals,
                            exact_integer_casts,
                            &proof_only,
                        ) {
                            proof_terms.push(CheckedLocatedProofTerm {
                                state: state.symbol,
                                statement_ordinal,
                                role: CheckedProofTermRole::ErasedUnitCallArgument {
                                    call_ordinal: u32::try_from(call_ordinal).ok()?,
                                    erased_ordinal: u32::try_from(proof_erased_index).ok()?,
                                },
                                expression: argument,
                                term,
                            });
                        }
                        proof_erased_index = proof_erased_index.checked_add(1)?;
                    } else {
                        return None;
                    }
                }
                continue;
            }
            let Some(expected_type) = program.primitive_type_reference(target.type_reference)
            else {
                continue;
            };
            if (target.is_const)
                || (target.is_mutable && mutable_scalar_parameter_type(program, target).is_none())
            {
                return None;
            }
            let lowered = lower_return_expression(
                program,
                operators,
                argument,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                expected_type,
                exact_integer_casts,
            );
            if let Some(lowered) = lowered {
                output.push((
                    argument,
                    CheckedLocatedScalarExpression {
                        state: state.symbol,
                        statement_ordinal,
                        role: if is_boundary {
                            CheckedScalarExpressionRole::BoundaryCallArgument {
                                call_ordinal: u32::try_from(call_ordinal).ok()?,
                                argument_ordinal: u32::try_from(scalar_index).ok()?,
                            }
                        } else {
                            CheckedScalarExpressionRole::UnitCallArgument {
                                call_ordinal: u32::try_from(call_ordinal).ok()?,
                                argument_ordinal: u32::try_from(scalar_index).ok()?,
                            }
                        },
                        expression: lowered,
                    },
                ));
            }
            scalar_index = scalar_index.checked_add(1)?;
        }
        if explicit_index != explicit_arguments.len() {
            return None;
        }
        let machine_arguments: &[typed_trees::expression::StaticMachineArgument] = match call_site {
            validation::CallSite::Statement(call) => call.machine_arguments.as_ref(),
            validation::CallSite::Expression { call, .. } => call.machine_arguments.as_ref(),
            validation::CallSite::TransitionNamed { .. } => &[],
        };
        let slot = program.dynamic_slot_by_symbol(target_symbol);
        if machine_arguments.is_empty() || slot.is_none() {
            return Some(LoweredCallArguments {
                scalar_arguments: output,
                proof_terms,
            });
        }
        let slot = slot?;
        let requirement = program.dynamic_slot_requirement(slot)?;
        let typed_trees::finite_family::FamilyProbe::Finite { arity, .. } =
            program.finite_signature_family(requirement)
        else {
            return None;
        };
        let binders = program
            .state_signature_type_parameters(requirement)
            .iter()
            .filter(|parameter| parameter.kind.is_const_or_value())
            .collect::<Vec<_>>();
        if arity != binders.len() || machine_arguments.len() != arity {
            return None;
        }
        for (family_ordinal, (argument, binder)) in
            machine_arguments.iter().zip(binders.iter()).enumerate()
        {
            let expected_type = program.primitive_type_reference(match binder.kind {
                typed_trees::data::TypeParameterKind::Const { type_reference }
                | typed_trees::data::TypeParameterKind::Value { type_reference } => type_reference,
                _ => return None,
            })?;
            let expression =
                family_argument_expression(program, argument, parameters, locals, expected_type)?;
            output.push((
                typed_trees::expression::ExpressionHandle::default(),
                CheckedLocatedScalarExpression {
                    state: state.symbol,
                    statement_ordinal,
                    role: CheckedScalarExpressionRole::DynamicFamilyArgument {
                        call_ordinal: u32::try_from(call_ordinal).ok()?,
                        family_ordinal: u32::try_from(family_ordinal).ok()?,
                    },
                    expression,
                },
            ));
        }
        Some(LoweredCallArguments {
            scalar_arguments: output,
            proof_terms,
        })
    }

    /*
        The evaluated operand one finite-family actual contributes: a closed
        const lands at the binder's declared primitive type, and an ordinary
        runtime subject reads its dense scalar parameter, value-binding, or
        storage position -- the same reading the subject's name would produce as
        an explicit argument. Anything else owns no row-selection operand.
    */
    fn family_argument_expression(
        program: &dyn ::validation::CheckingInput,
        argument: &typed_trees::expression::StaticMachineArgument,
        scalar_parameters: &[StateParameter],
        locals: &[ScalarLocal],
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarExpression> {
        if let Some(literal) = &argument.const_literal {
            let landed_type = expected_type.landed_landing()?;
            return Some(CheckedScalarExpression::IntegerLiteral {
                literal: literal.clone().with_landing(IntegerLanding {
                    landed_type,
                    domain: ArithmeticDomain::Exact,
                }),
            });
        }
        if !validation::static_arguments::is_runtime_value_subject(program, argument)
            || !argument.symbol.is_valid()
        {
            return None;
        }
        if let Some(position) = scalar_parameters
            .iter()
            .position(|parameter| parameter.symbol == argument.symbol)
        {
            return Some(CheckedScalarExpression::Parameter {
                position,
                primitive_type: expected_type,
            });
        }
        let local = locals
            .iter()
            .find(|local| local.symbol == argument.symbol)?;
        if !local.is_value_binding() {
            return Some(CheckedScalarExpression::StorageRead {
                symbol: local.symbol,
                primitive_type: expected_type,
            });
        }
        Some(CheckedScalarExpression::Local {
            position: located_binding_symbols(scalar_parameters, locals)
                .position(|symbol| symbol == argument.symbol)?,
            primitive_type: expected_type,
        })
    }

    /*
        Form a proof-side observation contingent on final source admission of the
        actual's type, scope, borrow and callee precondition. A borrow names its
        exact referent expression here; it never forms an executable descriptor or
        load. Located argument rows retain the original authored expression.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_erased_scalar_argument(
        state_symbol: symbols::SymbolHandle,
        before_statement: u32,
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        expression: ExpressionHandle,
        parameters: &[StateParameter],
        authored_parameters: &[StateParameter],
        parameter_types: &[PrimitiveType],
        locals: &[ScalarLocal],
        expected_type: PrimitiveType,
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> Option<CheckedScalarExpression> {
        let observation = match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Shared =>
            {
                borrow.target
            }
            _ => expression,
        };
        let observation = super::erased_observations::resolve(
            program,
            state_symbol,
            before_statement,
            observation,
        )?;
        lower_return_expression(
            program,
            operators,
            observation,
            parameters,
            authored_parameters,
            parameter_types,
            locals,
            expected_type,
            exact_integer_casts,
        )
    }

    /*
        Lower the actual of one erased scalar lane. A primitive lane keeps the
        ordinary erased scalar observation. A record leaf is the matching lane
        of the caller's own erased formal, the scalar the caller wrote at that
        field of an immutable closed construction, or that field observed at
        the place of a record local or runtime record formal. Nothing here
        reads the record at run time. `validation::erased_actuals` owns which
        of those a leaf is and refuses every other actual at source checking.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_erased_lane_argument(
        state_symbol: symbols::SymbolHandle,
        before_statement: u32,
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        expression: ExpressionHandle,
        parameters: &[StateParameter],
        authored_parameters: &[StateParameter],
        parameter_types: &[PrimitiveType],
        locals: &[ScalarLocal],
        formal_type: typed_trees::types::TypeReferenceHandle,
        lane: &typed_trees::proof_only::ErasedScalarLane,
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> Option<CheckedScalarExpression> {
        if lane.fields.is_empty() {
            return lower_erased_scalar_argument(
                state_symbol,
                before_statement,
                program,
                operators,
                expression,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                lane.primitive_type,
                exact_integer_casts,
            );
        }
        // Source checking has already refused an actual with no leaf source;
        // the same owner names the source lowered here.
        let leaf = validation::erased_actuals::resolve_leaf(
            program,
            state_symbol,
            before_statement,
            expression,
            formal_type,
            &lane.fields,
            lane.primitive_type,
            authored_parameters,
        )
        .ok()?;
        match leaf {
            validation::erased_actuals::ErasedLeaf::Formal { position } => {
                Some(CheckedScalarExpression::ErasedParameter {
                    position,
                    primitive_type: lane.primitive_type,
                })
            }
            validation::erased_actuals::ErasedLeaf::ParameterField { position, segments } => {
                Some(CheckedScalarExpression::StructuralParameterField {
                    observation: expression,
                    parameter_position: position,
                    path: super::structural_fields::structural_projection_path(program, &segments)?,
                    primitive_type: lane.primitive_type,
                })
            }
            validation::erased_actuals::ErasedLeaf::LocalField { symbol, segments } => {
                Some(CheckedScalarExpression::StructuralLocalField {
                    symbol,
                    path: super::structural_fields::structural_projection_path(program, &segments)?,
                    primitive_type: lane.primitive_type,
                })
            }
            validation::erased_actuals::ErasedLeaf::Value(leaf) => lower_return_expression(
                program,
                operators,
                leaf,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                lane.primitive_type,
                exact_integer_casts,
            ),
        }
    }

    /*
        Lower one erased contract-term actual: a construction (or a bare
        reference to the caller's own erased proof-lane formal) becomes the
        checked proof term the callee's contract lane carries. A construction
        field whose declared type is primitive lowers through the scalar lane
        into a `Scalar` leaf — the erased record carrier's runtime payload is
        retained as a term, not a runtime operand.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_proof_term(
        program: &dyn ::validation::CheckingInput,
        caller_state: symbols::SymbolHandle,
        operators: &CheckedOperatorFacts,
        expression: ExpressionHandle,
        expected_type: typed_trees::types::TypeReferenceHandle,
        parameters: &[StateParameter],
        authored_parameters: &[StateParameter],
        parameter_types: &[PrimitiveType],
        locals: &[ScalarLocal],
        exact_integer_casts: &[validation::ExactIntegerCastFact],
        proof_only: &typed_trees::proof_only::ProofOnlyClassification,
    ) -> Option<CheckedProofTerm> {
        if let Some(view) = program.prepared_mathematics() {
            if let Some(constructor) = view.source_constructor(expression)
                && constructor.type_identity == program.normalized_type_identity(expected_type)
                && constructor.constructor != expression
            {
                return lower_proof_term(
                    program,
                    caller_state,
                    operators,
                    constructor.constructor,
                    expected_type,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                    proof_only,
                );
            }
            if let Some(constructor) = view.source_constructor(expression)
                && constructor.type_identity == program.normalized_type_identity(expected_type)
            {
                // Both publication forms carry closed mathematical judgments.
                // A quantified source schema is useful during source checking,
                // but must be specialized before entering either receiving lane.
                if constructor.level_arity != 0 {
                    return None;
                }
                // Abstraction can produce a closed value under an unrelated
                // lexical scalar scope. Its empty dependency roster is already
                // kernel checked; retain that exact value rather than rebuilding
                // its source fields or treating it as an open function actual.
                if constructor.parameters.is_empty() {
                    return Some(CheckedProofTerm::Mathematical {
                        type_identity: constructor.type_identity,
                        term: constructor.schema,
                        ty: constructor.ty,
                    });
                }
                let arguments = constructor
                    .parameters
                    .iter()
                    .map(|symbol| {
                        let parameter = authored_parameters.iter().find(|parameter| {
                            parameter.symbol == *symbol && !parameter.is_mutable
                        })?;
                        if parameter.relevance == language_core::BindingRelevance::Erased {
                            return Some(CheckedMathematicalArgument::Formal {
                                parameter_symbol: *symbol,
                            });
                        }
                        let primitive_type =
                            program.primitive_type_reference(parameter.type_reference)?;
                        if primitive_type != PrimitiveType::Bool
                            && primitive_type.integer_bounds().is_none()
                        {
                            return None;
                        }
                        let position = parameters
                            .iter()
                            .position(|parameter| parameter.symbol == *symbol)?;
                        Some(CheckedMathematicalArgument::Scalar {
                            type_identity: program
                                .normalized_type_identity(parameter.type_reference),
                            expression: CheckedScalarExpression::Parameter {
                                position,
                                primitive_type,
                            },
                        })
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(arguments) = arguments {
                    return Some(CheckedProofTerm::MathematicalApplication {
                        type_identity: constructor.type_identity,
                        term: constructor.schema,
                        ty: constructor.ty,
                        arguments,
                    });
                }
            }
            if let Some(evidence) = view.source_evidence(expression) {
                let expected_identity = program.normalized_type_identity(expected_type);
                if evidence.type_identity == expected_identity {
                    return Some(CheckedProofTerm::Mathematical {
                        type_identity: evidence.type_identity,
                        term: evidence.term,
                        ty: evidence.ty,
                    });
                }
            }
        }
        if let Some(primitive) = program.primitive_type_reference(expected_type) {
            return Some(CheckedProofTerm::Scalar(lower_return_expression(
                program,
                operators,
                expression,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                primitive,
                exact_integer_casts,
            )?));
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::ArrayLiteral(elements) => {
                let typed_trees::types::TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(expected_type)
                else {
                    return None;
                };
                let elements = program.expression_table.expression_handles(*elements);
                if elements.len() != *length {
                    return None;
                }
                Some(CheckedProofTerm::Array {
                    type_identity: program.type_identity(
                        typed_trees::type_identity::TypeIdentityRequest::ordinary(expected_type),
                    ),
                    elements: elements
                        .iter()
                        .map(|element| {
                            lower_proof_term(
                                program,
                                caller_state,
                                operators,
                                *element,
                                *element_type,
                                parameters,
                                authored_parameters,
                                parameter_types,
                                locals,
                                exact_integer_casts,
                                proof_only,
                            )
                        })
                        .collect::<Option<Vec<_>>>()?,
                })
            }
            ExpressionNode::StructLiteral(literal) => {
                if !literal.type_symbol.is_valid() {
                    return None;
                }
                let mut fields = Vec::new();
                for field in expressions(program).struct_fields(literal.fields).iter() {
                    let declared = program
                        .data_constructor_field(
                            expected_type,
                            literal.type_symbol,
                            field.field_symbol,
                        )?
                        .type_reference;
                    let term = lower_proof_term(
                        program,
                        caller_state,
                        operators,
                        field.value,
                        declared,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        exact_integer_casts,
                        proof_only,
                    )?;
                    fields.push(CheckedProofTermField {
                        field_symbol: field.field_symbol,
                        term,
                    });
                }
                let type_identity = program.normalized_type_identity(expected_type);
                Some(CheckedProofTerm::Construction {
                    data_symbol: literal.type_symbol,
                    type_identity,
                    case_symbol: literal.case_symbol,
                    fields,
                })
            }
            ExpressionNode::Borrow(borrow) => lower_proof_term(
                program,
                caller_state,
                operators,
                borrow.target,
                expected_type,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
                proof_only,
            ),
            ExpressionNode::Name(path) => {
                if let Some(position) = parameter_position(program, path, authored_parameters) {
                    let parameter = authored_parameters.get(position)?;
                    return (parameter.relevance.is_erased()
                        && parameter_is_erased_proof(
                            program,
                            proof_only,
                            parameter.type_reference,
                        ))
                    .then_some(CheckedProofTerm::Formal {
                        parameter_symbol: parameter.symbol,
                    });
                }
                let symbol = if path.symbol.is_valid() {
                    path.symbol
                } else {
                    path.head_symbol
                };
                if !symbol.is_valid() {
                    return None;
                }
                let state = program.state_by_symbol(caller_state)?;
                let (_, local) = program.state_local_declaration(state, symbol)?;
                if !local.relevance.is_erased()
                    || !local.initial_value.is_valid()
                    || !parameter_is_erased_proof(program, proof_only, local.type_reference)
                {
                    return None;
                }
                lower_proof_term(
                    program,
                    caller_state,
                    operators,
                    local.initial_value,
                    expected_type,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                    proof_only,
                )
            }
            ExpressionNode::Member(_) | ExpressionNode::SavedSubject(_) => {
                erased_payload_projection(
                    program,
                    expression,
                    expected_type,
                    authored_parameters,
                    proof_only,
                )
            }
            ExpressionNode::Call(call) => {
                let definition = program
                    .mathematical_definitions()
                    .iter()
                    .find(|definition| definition.symbol == call.target_symbol)?;
                let typed_trees::mathematical::MathematicalBody::Definition(body) = definition.body
                else {
                    return None;
                };
                let ExpressionNode::Name(path) = program.expression_table.expression(body) else {
                    return None;
                };
                let definition_parameters = program.mathematical_parameters(definition.parameters);
                let index = definition_parameters.iter().position(|parameter| {
                    parameter.symbol == path.symbol
                        || program.symbols.get(path.symbol).generated_from == parameter.symbol
                })?;
                let argument = program
                    .expression_table
                    .expression_handles(call.arguments)
                    .get(index)
                    .copied()?;
                lower_proof_term(
                    program,
                    caller_state,
                    operators,
                    argument,
                    expected_type,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                    proof_only,
                )
            }
            _ => None,
        }
    }

    fn parameter_is_erased_proof(
        program: &dyn ::validation::CheckingInput,
        proof_only: &typed_trees::proof_only::ProofOnlyClassification,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        proof_only.contract_term_carrier(program, type_reference)
            || validation::mathematical_applications::is_source_evidence_carrier(
                program,
                type_reference,
            )
    }

    /*
        An erased payload read (`evidence.left`) is the enclosing formal plus the
        field path. Recording only the formal would name the whole carrier. The
        projection keeps the formal and every erased evidence field, and its type
        is the callee formal only when that carrier corresponds.
    */
    fn erased_payload_projection(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        expected_type: typed_trees::types::TypeReferenceHandle,
        authored_parameters: &[StateParameter],
        proof_only: &typed_trees::proof_only::ProofOnlyClassification,
    ) -> Option<CheckedProofTerm> {
        let mut cursor = expression;
        let mut members = Vec::new();
        for _ in 0..8 {
            match program.expression_table.expression(cursor) {
                ExpressionNode::Borrow(borrow) => cursor = borrow.target,
                ExpressionNode::Member(member) => {
                    members.push(cursor);
                    cursor = member.receiver;
                }
                ExpressionNode::SavedSubject(saved) => {
                    cursor = program.saved_value_source(saved)?;
                }
                ExpressionNode::Name(_) => break,
                _ => return None,
            }
        }
        let ExpressionNode::Name(path) = program.expression_table.expression(cursor) else {
            return None;
        };
        if members.is_empty() {
            return None;
        }
        let parameter = authored_parameters
            .iter()
            .find(|parameter| parameter.symbol == path.symbol && parameter.relevance.is_erased())?;
        if !(proof_only.contract_term_carrier(program, parameter.type_reference)
            || validation::mathematical_applications::is_source_evidence_carrier(
                program,
                parameter.type_reference,
            ))
        {
            return None;
        }
        let mut carrier = parameter.type_reference;
        let mut field_symbols = Vec::new();
        for member_expression in members.iter().rev() {
            let ExpressionNode::Member(member) =
                program.expression_table.expression(*member_expression)
            else {
                return None;
            };
            let field = validation::applied_data_member_field(program, carrier, member)?;
            if !field.relevance.is_erased() {
                return None;
            }
            if !(proof_only.contract_term_carrier(program, field.type_reference)
                || validation::mathematical_applications::is_source_evidence_carrier(
                    program,
                    field.type_reference,
                ))
            {
                return None;
            }
            field_symbols.push(field.symbol);
            carrier = field.type_reference;
        }
        let expected_identity = program.normalized_type_identity(expected_type);
        let carrier_identity = program.normalized_type_identity(carrier);
        let corresponds = carrier_identity == expected_identity
            || erased_generic_carriers_correspond(program, carrier, expected_type)
            || projection_result_matches(program, parameter, expression, expected_identity)
            || projection_instantiated_by_call(
                program,
                parameter,
                expression,
                carrier,
                expected_type,
            );
        corresponds.then_some(CheckedProofTerm::Projection {
            parameter_symbol: parameter.symbol,
            field_symbols,
            type_identity: expected_identity,
        })
    }

    /*
        The payload's indices are the constructor fields. The callee formal's
        indices are its own parameters. They name one carrier when the call
        passes each field value in that formal parameter's position.
    */
    fn projection_instantiated_by_call(
        program: &dyn ::validation::CheckingInput,
        parameter: &StateParameter,
        expression: ExpressionHandle,
        carrier: typed_trees::types::TypeReferenceHandle,
        expected: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        let Some((carrier_base, carrier_indices)) = generic_index_symbols(program, carrier) else {
            return false;
        };
        let Some((expected_base, expected_indices)) = generic_index_symbols(program, expected)
        else {
            return false;
        };
        if carrier_indices.len() != expected_indices.len()
            || !proof_symbols_correspond(program, carrier_base, expected_base)
        {
            return false;
        }
        program.machines().iter().any(|machine| {
            program.machine_states(machine).iter().any(|state| {
                program
                    .state_parameters(state)
                    .iter()
                    .any(|candidate| candidate.symbol == parameter.symbol)
                    && program
                        .statement_table
                        .statements(state.statement_nodes)
                        .iter()
                        .any(|statement| {
                            projection_statement_instantiates(
                                program,
                                expression,
                                statement,
                                &carrier_indices,
                                &expected_indices,
                            )
                        })
            })
        })
    }

    fn projection_statement_instantiates(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        statement: &typed_trees::statement::StatementNode,
        carrier_indices: &[symbols::SymbolHandle],
        expected_indices: &[symbols::SymbolHandle],
    ) -> bool {
        match statement {
            typed_trees::statement::StatementNode::Transition(transition) => {
                [transition.target, transition.continuation]
                    .into_iter()
                    .filter(|handle| handle.is_valid())
                    .any(|handle| {
                        let Some(transfer) =
                            validation::completing_transition_transfer(program, handle)
                        else {
                            return false;
                        };
                        transfer.arguments.contains(&expression)
                            && call_arguments_instantiate(
                                program,
                                transfer.target,
                                transfer.arguments,
                                carrier_indices,
                                expected_indices,
                            )
                    })
            }
            typed_trees::statement::StatementNode::Call(call) => {
                let arguments = program.statement_table.expression_handles(call.arguments);
                arguments.contains(&expression)
                    && call_arguments_instantiate(
                        program,
                        call.target_symbol,
                        arguments,
                        carrier_indices,
                        expected_indices,
                    )
            }
            typed_trees::statement::StatementNode::Expression(call_expression) => program
                .expression_table
                .call_node(*call_expression)
                .is_some_and(|call| {
                    let arguments = program.expression_table.expression_handles(call.arguments);
                    arguments.contains(&expression)
                        && call_arguments_instantiate(
                            program,
                            call.target_symbol,
                            arguments,
                            carrier_indices,
                            expected_indices,
                        )
                }),
            _ => false,
        }
    }

    fn call_arguments_instantiate(
        program: &dyn ::validation::CheckingInput,
        target: symbols::SymbolHandle,
        arguments: &[ExpressionHandle],
        carrier_indices: &[symbols::SymbolHandle],
        expected_indices: &[symbols::SymbolHandle],
    ) -> bool {
        let Some(parameters) = validation::call_target_parameters(program, target) else {
            return false;
        };
        carrier_indices
            .iter()
            .zip(expected_indices)
            .all(|(carrier_index, formal_index)| {
                let Some(position) = parameters.iter().position(|parameter| {
                    proof_symbols_correspond(program, parameter.symbol, *formal_index)
                }) else {
                    return false;
                };
                arguments.get(position).is_some_and(|argument| {
                    expression_symbol(program, *argument).is_some_and(|symbol| {
                        proof_symbols_correspond(program, symbol, *carrier_index)
                    })
                })
            })
    }

    fn generic_index_symbols(
        program: &dyn ::validation::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<(symbols::SymbolHandle, Vec<symbols::SymbolHandle>)> {
        let typed_trees::types::TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = program
            .type_reference_table
            .type_reference(peel_type_shell(program, reference))
        else {
            return None;
        };
        if !base_symbol.is_valid() {
            return None;
        }
        let indices = program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .map(|argument| proof_index_symbol(program, *argument))
            .collect::<Option<Vec<_>>>()?;
        Some((*base_symbol, indices))
    }

    fn expression_symbol(
        program: &dyn ::validation::CheckingInput,
        mut expression: ExpressionHandle,
    ) -> Option<symbols::SymbolHandle> {
        for _ in 0..8 {
            match program.expression_table.expression(expression) {
                ExpressionNode::Borrow(borrow) => expression = borrow.target,
                ExpressionNode::SavedSubject(saved) => {
                    expression = program.saved_value_source(saved)?;
                }
                ExpressionNode::Name(path) if path.symbol.is_valid() => return Some(path.symbol),
                ExpressionNode::Member(member) if member.member_symbol.is_valid() => {
                    return Some(member.member_symbol);
                }
                _ => return None,
            }
        }
        None
    }

    fn proof_symbols_correspond(
        program: &dyn ::validation::CheckingInput,
        left: symbols::SymbolHandle,
        right: symbols::SymbolHandle,
    ) -> bool {
        left == right
            || proof_symbol_derives_from(program, left, right)
            || proof_symbol_derives_from(program, right, left)
    }

    fn projection_result_matches(
        program: &dyn ::validation::CheckingInput,
        parameter: &StateParameter,
        expression: ExpressionHandle,
        expected_identity: typed_trees::type_identity::TypeIdentityId,
    ) -> bool {
        program.machines().iter().any(|machine| {
            program.machine_states(machine).iter().any(|state| {
                program
                    .state_parameters(state)
                    .iter()
                    .any(|candidate| candidate.symbol == parameter.symbol)
                    && validation::expression_result_type_reference(
                        program, machine, state, expression,
                    )
                    .is_some_and(|result| {
                        program.normalized_type_identity(result) == expected_identity
                    })
            })
        })
    }

    fn erased_generic_carriers_correspond(
        program: &dyn ::validation::CheckingInput,
        actual: typed_trees::types::TypeReferenceHandle,
        expected: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        let actual = peel_type_shell(program, actual);
        let expected = peel_type_shell(program, expected);
        let (
            typed_trees::types::TypeReferenceNode::Generic {
                base_symbol: actual_base,
                arguments: actual_arguments,
                ..
            },
            typed_trees::types::TypeReferenceNode::Generic {
                base_symbol: expected_base,
                arguments: expected_arguments,
                ..
            },
        ) = (
            program.type_reference_table.type_reference(actual),
            program.type_reference_table.type_reference(expected),
        )
        else {
            return false;
        };
        if *actual_base != *expected_base
            && !proof_symbol_derives_from(program, *actual_base, *expected_base)
            && !proof_symbol_derives_from(program, *expected_base, *actual_base)
        {
            return false;
        }
        let actual_arguments = program
            .type_reference_table
            .type_reference_handles(*actual_arguments);
        let expected_arguments = program
            .type_reference_table
            .type_reference_handles(*expected_arguments);
        actual_arguments.len() == expected_arguments.len()
            && actual_arguments.iter().zip(expected_arguments.iter()).all(
                |(actual_argument, expected_argument)| {
                    program.normalized_type_identity(*actual_argument)
                        == program.normalized_type_identity(*expected_argument)
                        || proof_index_symbols_correspond(
                            program,
                            *actual_argument,
                            *expected_argument,
                        )
                },
            )
    }

    fn proof_index_symbols_correspond(
        program: &dyn ::validation::CheckingInput,
        actual: typed_trees::types::TypeReferenceHandle,
        expected: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        let (Some(actual), Some(expected)) = (
            proof_index_symbol(program, actual),
            proof_index_symbol(program, expected),
        ) else {
            return false;
        };
        actual == expected
            || proof_symbol_derives_from(program, actual, expected)
            || proof_symbol_derives_from(program, expected, actual)
    }

    fn proof_index_symbol(
        program: &dyn ::validation::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<symbols::SymbolHandle> {
        match program
            .type_reference_table
            .type_reference(peel_type_shell(program, reference))
        {
            typed_trees::types::TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                Some(*symbol)
            }
            typed_trees::types::TypeReferenceNode::ConstExpression(expression) => {
                match program.expression_table.expression(*expression) {
                    ExpressionNode::Name(path) if path.symbol.is_valid() => Some(path.symbol),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn peel_type_shell(
        program: &dyn ::validation::CheckingInput,
        mut reference: typed_trees::types::TypeReferenceHandle,
    ) -> typed_trees::types::TypeReferenceHandle {
        for _ in 0..8 {
            match program.type_reference_table.type_reference(reference) {
                typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                    reference = *referee;
                }
                typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                    reference = *base_type;
                }
                _ => break,
            }
        }
        reference
    }

    fn proof_symbol_derives_from(
        program: &dyn ::validation::CheckingInput,
        mut symbol: symbols::SymbolHandle,
        origin: symbols::SymbolHandle,
    ) -> bool {
        if !symbol.is_valid() || !origin.is_valid() {
            return false;
        }
        if symbol == origin {
            return true;
        }
        for _ in 0..8 {
            let next = program.symbols.get(symbol).generated_from;
            if !next.is_valid() {
                return false;
            }
            if next == origin {
                return true;
            }
            symbol = next;
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_direct_call_binding_arguments(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        state: symbols::SymbolHandle,
        statement_ordinal: u32,
        binding_ordinal: u32,
        expression: ExpressionHandle,
        parameters: &[StateParameter],
        authored_parameters: &[StateParameter],
        parameter_types: &[PrimitiveType],
        locals: &[ScalarLocal],
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> Option<LoweredCallArguments> {
        let call = program.expression_table.call_node(expression)?;
        if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
            return None;
        }
        validation::find_machine_by_entry_state(program, call.target_symbol)?;
        let target_parameters = validation::call_target_parameters(program, call.target_symbol)?;
        if target_parameters.iter().any(|parameter| {
            (parameter.is_const)
                || (parameter.is_mutable
                    && mutable_scalar_parameter_type(program, parameter).is_none())
        }) {
            return None;
        }
        let arguments = program.expression_table.expression_handles(call.arguments);
        if arguments.len() != target_parameters.len() {
            return None;
        }
        if target_parameters
            .iter()
            .any(|target_parameter| strips_erased_parameter(target_parameter).is_none())
        {
            return None;
        }
        let mut argument_ordinal = 0u32;
        let mut erased_ordinal = 0u32;
        let mut proof_erased_ordinal = 0u32;
        let proof_only = validation::proof_only_classification(program);
        let mut scalar_arguments = Vec::new();
        let mut proof_terms = Vec::new();
        for (argument, target_parameter) in arguments.iter().zip(target_parameters) {
            let lanes = if target_parameter.relevance.is_erased() {
                program.erased_scalar_lanes(target_parameter.type_reference)
            } else {
                Vec::new()
            };
            if !lanes.is_empty() {
                for lane in &lanes {
                    let ordinal = erased_ordinal;
                    erased_ordinal = erased_ordinal.checked_add(1)?;
                    scalar_arguments.push((
                        *argument,
                        CheckedLocatedScalarExpression {
                            state,
                            statement_ordinal,
                            role: CheckedScalarExpressionRole::ErasedCallArgument {
                                binding_ordinal,
                                erased_ordinal: ordinal,
                            },
                            expression: lower_erased_lane_argument(
                                state,
                                statement_ordinal,
                                program,
                                operators,
                                *argument,
                                parameters,
                                authored_parameters,
                                parameter_types,
                                locals,
                                target_parameter.type_reference,
                                lane,
                                exact_integer_casts,
                            )?,
                        },
                    ));
                }
                continue;
            }
            // Every erased formal with lanes was lowered above, so an erased
            // formal reaching here can only be a contract-term carrier.
            let Some(expected_type) = (if target_parameter.relevance.is_erased() {
                None
            } else {
                program.primitive_type_reference(target_parameter.type_reference)
            }) else {
                if !target_parameter.relevance.is_erased()
                    || !(proof_only.contract_term_carrier(program, target_parameter.type_reference)
                        || validation::mathematical_applications::is_source_evidence_carrier(
                            program,
                            target_parameter.type_reference,
                        ))
                {
                    return None;
                }
                let ordinal = proof_erased_ordinal;
                proof_erased_ordinal = proof_erased_ordinal.checked_add(1)?;
                proof_terms.push(CheckedLocatedProofTerm {
                    state,
                    statement_ordinal,
                    role: CheckedProofTermRole::ErasedCallArgument {
                        binding_ordinal,
                        erased_ordinal: ordinal,
                    },
                    expression: *argument,
                    term: lower_proof_term(
                        program,
                        state,
                        operators,
                        *argument,
                        target_parameter.type_reference,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        exact_integer_casts,
                        &proof_only,
                    )?,
                });
                continue;
            };
            let ordinal = argument_ordinal;
            argument_ordinal = argument_ordinal.checked_add(1)?;
            scalar_arguments.push((
                *argument,
                CheckedLocatedScalarExpression {
                    state,
                    statement_ordinal,
                    role: CheckedScalarExpressionRole::CallArgument {
                        binding_ordinal,
                        argument_ordinal: ordinal,
                    },
                    expression: lower_return_expression(
                        program,
                        operators,
                        *argument,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        expected_type,
                        exact_integer_casts,
                    )?,
                },
            ));
        }
        Some(LoweredCallArguments {
            scalar_arguments,
            proof_terms,
        })
    }

    /*
        The call a value expression evaluates, whether spelled directly or reached
        beneath same-carrier integer casts — the two custody spellings a statement
        can write for `CallSite::Expression` argument rows.
    */
    pub(crate) fn value_call_expression(
        program: &dyn ::validation::CheckingInput,
        value: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        scalar_qualified_call_expression(program, value).or_else(|| {
            matches!(
                program.expression_table.expression(value),
                ExpressionNode::Call(_)
            )
            .then_some(value)
        })
    }

    /*
        Locate a call beneath only same-carrier integer qualifications. The actual
        callee declaration supplies that carrier; no conversion result is guessed.
    */
    pub(crate) fn scalar_qualified_call_expression(
        program: &dyn ::validation::CheckingInput,
        mut expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let mut targets = Vec::new();
        loop {
            if !program.expression_table.expression_is_valid(expression) {
                return None;
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Cast(cast)
                    if !cast.form.is_recast() && cast.semantic_domain.is_empty() =>
                {
                    targets.push(program.primitive_type_reference(cast.target_type)?);
                    expression = cast.value;
                }
                ExpressionNode::Call(call) => {
                    let state = program.state_by_symbol(call.target_symbol)?;
                    let primitive = program.primitive_type_reference(state.return_type)?;
                    return (is_integer(primitive)
                        && primitive != PrimitiveType::Addr
                        && targets.iter().all(|target| *target == primitive))
                    .then_some(expression);
                }
                _ => return None,
            }
        }
    }
}
pub(crate) mod case_membership {
    /*
        Runtime membership uses the same exact authored selection judgment as
        checking. It is a tag observation, never equality against a fabricated
        payload value. The current scalar namespace names parameter referents;
        constructor and local referents must arrive through ordinary value sequencing.
        Normal-return contracts additionally name their authenticated result place,
        never a synthetic parameter slot or a constructor-derived assumption.
    */
    use crate::flow::expressions;
    use crate::values::scalar::structural_fields::structural_data;
    use checked_trees::CheckedBooleanExpression;
    use checked_trees::CheckedStructuralParameterField;
    use typed_trees::expression::BinaryOperator;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::signature::StateParameter;

    pub(crate) fn lower(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<CheckedBooleanExpression> {
        let (binary, parameter_position, path) =
            parameter_case_subject(program, parameters, expression)?;
        let root = parameters.get(parameter_position as usize)?;
        let (machine, state) = program.machines().iter().find_map(|machine| {
            program.machine_states(machine).iter().find_map(|state| {
                let authored = program.state_parameters(state);
                (authored
                    .get(parameter_position as usize)
                    .is_some_and(|parameter| parameter.symbol == root.symbol)
                    && authored == parameters)
                    .then_some((machine, state))
            })
        })?;
        let subject = peeled_subject(program, binary);
        let case = case_identity(program, machine, Some(state), expression, binary, subject)?;
        Some(CheckedBooleanExpression::StructuralCaseMembership {
            subject: CheckedStructuralParameterField {
                parameter_position,
                path,
            }
            .into(),
            case,
        })
    }

    /*
        A requirement signature's own `requires subject in T::Case` has no machine
        state to rejoin: the published telescope of `owner` is the observation
        scope, as source checking judged it. The test stays a tag observation of
        the actual parameter referent.
    */
    pub(crate) fn lower_in_declaration(
        program: &dyn ::validation::CheckingInput,
        owner: symbols::SymbolHandle,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<CheckedBooleanExpression> {
        let (binary, parameter_position, path) =
            parameter_case_subject(program, parameters, expression)?;
        let selected = validation::applied_declaration_case_membership_variant(
            program, owner, parameters, expression, binary,
        )?;
        Some(CheckedBooleanExpression::StructuralCaseMembership {
            subject: CheckedStructuralParameterField {
                parameter_position,
                path,
            }
            .into(),
            case: structural_case_name(program, selected)?,
        })
    }

    fn peeled_subject(
        program: &dyn ::validation::CheckingInput,
        binary: &typed_trees::expression::TableBinaryExpression,
    ) -> ExpressionHandle {
        match program.expression_table.expression(binary.left) {
            ExpressionNode::Borrow(borrow) => borrow.target,
            _ => binary.left,
        }
    }

    fn parameter_case_subject<'program>(
        program: &'program dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<(
        &'program typed_trees::expression::TableBinaryExpression,
        u32,
        Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    )> {
        let binary = program.expression_table.binary_node(expression)?;
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::CaseMembership
        ) {
            return None;
        }
        let mut path = Vec::new();
        let parameter_position = super::structural_fields::structural_parameter_case_path(
            program,
            parameters,
            peeled_subject(program, binary),
            &mut path,
        )?;
        Some((binary, parameter_position, path))
    }

    pub(crate) fn lower_result(
        program: &dyn ::validation::CheckingInput,
        owner: symbols::SymbolHandle,
        result_type: typed_trees::types::TypeReferenceHandle,
        expression: ExpressionHandle,
    ) -> Option<CheckedBooleanExpression> {
        let binary = program.expression_table.binary_node(expression)?;
        let result = validation::reserved_result_place(program, binary.left)?;
        if result.machine_symbol != owner
            || validation::reserved_result_owner(program, result.root)? != (owner, result_type)
        {
            return None;
        }
        let machine = program.machine_by_symbol(owner)?;
        let case = case_identity(program, machine, None, expression, binary, binary.left)?;
        let path = super::structural_fields::structural_projection_path(program, &result.segments)?;
        let (path, _) =
            super::structural_fields::explicit_observation_path(program, result_type, &path)?;
        Some(CheckedBooleanExpression::StructuralCaseMembership {
            subject: checked_trees::CheckedStructuralCaseSubject {
                root: checked_trees::CheckedStructuralCaseRoot::Result,
                path,
            },
            case,
        })
    }

    fn case_identity(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: Option<&typed_trees::state::State>,
        expression: ExpressionHandle,
        binary: &typed_trees::expression::TableBinaryExpression,
        subject: ExpressionHandle,
    ) -> Option<checked_trees::CheckedMemberIdentity> {
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::CaseMembership
        ) {
            return None;
        }
        match program.expression_table.expression(binary.right) {
            ExpressionNode::Name(_) => {
                let selected = validation::applied_case_membership_variant(
                    program, machine, state, expression, binary,
                )?;
                structural_case_name(program, selected)
            }
            ExpressionNode::StructLiteral(literal)
                if literal.case_symbol.is_some()
                    && expressions(program)
                        .struct_fields(literal.fields)
                        .is_empty() =>
            {
                if !validation::has_builtin_binary_expression_meaning(
                    program, machine, state, expression,
                ) {
                    return None;
                }
                let case_symbol = literal.case_symbol?;
                let owner = program.data_definitions().iter().find(|data| {
                    data.symbol == literal.type_symbol
                        && program.data_members(data).iter().any(|member| {
                            matches!(
                                member,
                                typed_trees::data::DataMember::Variant(variant)
                                    if variant.symbol == case_symbol
                                        && program.data_payload_fields(variant).is_empty()
                            )
                        })
                })?;
                let subject_type = validation::declared_place_type_raw(
                    program, machine, state, subject,
                )
                .or_else(|| {
                    validation::reserved_result_place(program, subject)
                        .filter(|place| place.machine_symbol == machine.symbol)
                        .map(|place| place.type_reference)
                })?;
                if structural_data(program, subject_type)?.symbol != owner.symbol {
                    return None;
                }
                structural_case_name(program, case_symbol)
            }
            _ => None,
        }
    }

    /*
        The discriminant key a checked case-membership test stores: the declared
        `#<decimal>` identity an authored discriminant value writes, or the case's
        declared name when no identity exists.
    */
    pub(crate) fn structural_case_name(
        program: &dyn ::validation::CheckingInput,
        selected: symbols::SymbolHandle,
    ) -> Option<checked_trees::CheckedMemberIdentity> {
        let case = program
            .data_definitions()
            .iter()
            .flat_map(|data| program.data_members(data))
            .find_map(|member| match member {
                typed_trees::data::DataMember::Variant(case) if case.symbol == selected => {
                    Some(case)
                }
                _ => None,
            })?;
        Some(checked_trees::CheckedMemberIdentity::of_member(
            case.identity,
            case.name.clone(),
        ))
    }
}
pub(crate) mod constant_array_projection {
    /*
        A fixed projection of a closed constant array produces its existing scalar
        leaf. The original typed indexing and declaration selections remain intact
        for checking and source custody; this creates neither a place nor a borrow.
        Every leaf must be a literal, since selecting one element cannot erase the
        evaluation of an effectful or failing sibling constructor expression.
    */
    use super::structural_fields;
    use crate::flow::expressions;
    use checked_trees::CheckedOperatorFacts;
    use numerics::arithmetic::ArithmeticDomain;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::signature::StateParameter;
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

    pub(crate) fn selected_leaf(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let mut projections = Vec::new();
        let mut root = expression;
        while let Some(indexed) = program.expression_table.indexed_node(root) {
            projections.push((root, indexed.index));
            root = indexed.collection;
        }
        if projections.is_empty() {
            return None;
        }
        let mut reference = validation::declared_constant_array_type(program, root)?;
        validation::closed_literal_array_elements(program, root, reference)?;
        let mut selected = root;
        for (projection, index) in projections.into_iter().rev() {
            let TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } = program.type_reference_table.type_reference(reference)
            else {
                return None;
            };
            let elements = program.expression_table.array_literal_node(selected)?;
            let literal = program.expression_table.integer_node(index)?;
            if literal.landing().is_some_and(|landing| {
                landing.landed_type == numerics::literals::LandedIntegerType::Addr
                    || landing.domain != ArithmeticDomain::Exact
            }) || !structural_fields::indexed_read_is_builtin(
                program, operators, parameters, projection, reference, index,
            ) {
                return None;
            }
            let element_index = usize::try_from(literal.value_u64()?).ok()?;
            if element_index >= *length {
                return None;
            }
            selected = *expressions(program)
                .expression_handles(*elements)
                .get(element_index)?;
            reference = *element_type;
        }
        matches!(
            program.expression_table.expression(selected),
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::Float(_)
        )
        .then_some(selected)
    }
}
pub(crate) mod contract_entry {
    /*
        Declared Requires and crash ceilings describe invocation-entry operands.
        Body reads continue to use the independent current-storage namespace.
        Namespace qualification contributes no operand or data-owner obligation:
        resolution's nominal attachment, when present, is the owner identity.
    */
    use crate::values::lower_machine_parameter_boolean_expression;
    use checked_trees::CheckedBooleanExpression;
    use checked_trees::CheckedOperatorFacts;
    use checked_trees::CheckedScalarExpression;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::StateParameter;
    use typed_trees::types::PrimitiveType;

    mod crash_entry {
        /*
            Read crash conditions over call arguments and their fields. Integer
            comparisons may also use completed constants; float comparisons retain
            their exact literals and IEEE rules. Resolve each operand once so its type
            classification and the expression used for checking cannot disagree.
        */
        use crate::published::places::place_at;
        use crate::values::operator_is_builtin;

        mod logical_matches {
            /*
                Proof-local record subject bindings, distinct from runtime completed homes.

                A supported match observes one pure readable path at this query's frontier.
                Its arm contains only the ordinary predicate algebra, so nothing can write
                between that observation and an alias read. Ambient producers are not replayed.
            */
            use super::super::super::structural_fields::readable_entry_parameter_path;
            use super::super::super::structural_fields::structural_projection_path;
            use super::{ComparisonOperand, FieldPath, LogicalMatchBinding, Reader};
            use checked_trees::CheckedBooleanExpression;
            use typed_trees::expression::{ExpressionHandle, ExpressionNode};
            use validation::logical_record_match_capture;
            use validation::logical_saved_subject_projection;

            impl Reader<'_> {
                fn pure_match_subject(
                    &mut self,
                    expression: ExpressionHandle,
                    depth: usize,
                ) -> Option<()> {
                    self.charge(depth)?;
                    match self.program.expression_table.expression(expression) {
                        ExpressionNode::Name(_) => Some(()),
                        ExpressionNode::Member(member) => {
                            self.pure_match_subject(member.receiver, depth + 1)
                        }
                        ExpressionNode::SavedSubject(_) if !self.logical_captures.is_empty() => {
                            self.logical_saved_path(expression, depth + 1).map(|_| ())
                        }
                        _ => None,
                    }
                }

                fn record_binding(
                    &mut self,
                    expression: ExpressionHandle,
                    depth: usize,
                ) -> Option<LogicalMatchBinding> {
                    self.charge(depth)?;
                    let program = self.program;
                    let ExpressionNode::Match(dispatch) =
                        program.expression_table.expression(expression)
                    else {
                        return None;
                    };
                    self.pure_match_subject(dispatch.subject, depth + 1)?;
                    let observed = self.subject(dispatch.subject, depth + 1)?;
                    let capture =
                        logical_record_match_capture(program, expression, observed.type_reference)?;
                    Some(LogicalMatchBinding { capture, observed })
                }

                pub(super) fn logical_match_boolean(
                    &mut self,
                    expression: ExpressionHandle,
                    depth: usize,
                ) -> Option<CheckedBooleanExpression> {
                    let binding = self.record_binding(expression, depth + 1)?;
                    let arm = binding.capture.arm_value();
                    let previous = self.logical_captures.len();
                    self.logical_captures.push(binding);
                    let result = self.boolean(arm, depth + 1);
                    self.logical_captures.truncate(previous);
                    result
                }

                pub(super) fn logical_match_operand(
                    &mut self,
                    expression: ExpressionHandle,
                    depth: usize,
                ) -> Option<ComparisonOperand> {
                    let binding = self.record_binding(expression, depth + 1)?;
                    let arm = binding.capture.arm_value();
                    let previous = self.logical_captures.len();
                    self.logical_captures.push(binding);
                    let result = self.comparison_operand(arm, depth + 1);
                    self.logical_captures.truncate(previous);
                    result
                }

                fn saved_projection_leaf(
                    &self,
                    mut expression: ExpressionHandle,
                ) -> Option<ExpressionHandle> {
                    for _ in 0..64 {
                        match self.program.expression_table.expression(expression) {
                            ExpressionNode::SavedSubject(_) => return Some(expression),
                            ExpressionNode::Member(member) => expression = member.receiver,
                            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
                            ExpressionNode::Borrow(borrow) => expression = borrow.target,
                            _ => return None,
                        }
                    }
                    None
                }

                pub(super) fn is_saved_projection(&self, expression: ExpressionHandle) -> bool {
                    self.saved_projection_leaf(expression).is_some()
                }

                pub(super) fn logical_saved_path(
                    &mut self,
                    expression: ExpressionHandle,
                    depth: usize,
                ) -> Option<FieldPath> {
                    self.charge(depth)?;
                    let (mut observed, segments) = logical_saved_subject_projection(
                        self.program,
                        expression,
                        &self.logical_captures,
                    )?;
                    observed
                        .path
                        .extend(structural_projection_path(self.program, &segments)?);
                    observed.type_reference = readable_entry_parameter_path(
                        self.program,
                        self.parameters,
                        u32::try_from(observed.parameter_position).ok()?,
                        &observed.path,
                        &mut self.remaining,
                    )?;
                    Some(observed)
                }
            }
        }

        use checked_trees::{
            CheckedBooleanExpression, CheckedOperatorFacts, CheckedScalarExpression,
            CheckedStructuralPredicatePathSegment as PathSegment,
        };
        use symbols::BuiltinTypeAtom;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
        };
        use typed_trees::machine::Machine;
        use typed_trees::signature::StateParameter;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn lower_machine_entry_crash_contract_expression(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            machine: &Machine,
            expression: ExpressionHandle,
            _exact_integer_casts: &[validation::ExactIntegerCastFact],
        ) -> Option<CheckedBooleanExpression> {
            let parameters = super::authored_entry_parameters(program, machine)?;
            let predicate = Reader {
                program,
                operators,
                machine: Some(machine),
                state: None,
                owner: machine.symbol,
                parameters,
                remaining: 4096,
                subjects: None,
                logical_captures: Vec::new(),
            }
            .boolean(expression, 0)?;
            Some(predicate)
        }

        /*
            Read pure logical predicates against the current state's declared frame.
            This supplies no entry-value equation and never rereads local initializers;
            the caller binds observations at its actual statement and live contexts.
        */
        pub(crate) fn lower_current_state_predicate(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            machine: &Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            expression: ExpressionHandle,
        ) -> Option<CheckedBooleanExpression> {
            if statement_index > crate::flow::state_statements(program, state.statement_nodes).len()
            {
                return None;
            }
            Reader {
                program,
                operators,
                machine: Some(machine),
                state: Some(state),
                owner: state.symbol,
                parameters: program.state_parameters(state),
                remaining: 4096,
                subjects: None,
                logical_captures: Vec::new(),
            }
            .boolean(expression, 0)
        }

        type EntrySubjectResolver<'program> =
            dyn FnMut(ExpressionHandle) -> Option<(u32, Vec<PathSegment>)> + 'program;

        /*
            Read the same predicate algebra while the caller supplies authenticated
            invocation-entry subjects for observations made in a body state.
        */
        pub(crate) fn lower_entry_predicate(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            machine: &Machine,
            state: &typed_trees::state::State,
            expression: ExpressionHandle,
            subjects: &mut EntrySubjectResolver<'_>,
        ) -> Option<CheckedBooleanExpression> {
            let parameters = super::authored_entry_parameters(program, machine)?;
            let predicate = Reader {
                program,
                operators,
                machine: Some(machine),
                state: Some(state),
                owner: machine.symbol,
                parameters,
                remaining: 4096,
                subjects: Some(subjects),
                logical_captures: Vec::new(),
            }
            .boolean(expression, 0)?;
            entry_predicate_has_meaning(program, machine, state, expression, 0).then_some(predicate)
        }

        /*
            Every node of a captured entry guard needs its builtin meaning, except an
            IEEE comparison: that selects the sealed Float operator, so it needs the
            sealed declaration for its operands instead, and its operands still need
            their own builtin meaning. `x.value < 0.0 && ready` is therefore read, while
            a user-supplied comparison stays unread.
        */
        fn entry_predicate_has_meaning(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &typed_trees::state::State,
            expression: ExpressionHandle,
            depth: usize,
        ) -> bool {
            if validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) {
                return true;
            }
            if depth >= 64 {
                return false;
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Binary(binary)
                    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
                {
                    validation::has_builtin_binary_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        expression,
                    ) && [binary.left, binary.right].into_iter().all(|operand| {
                        entry_predicate_has_meaning(program, machine, state, operand, depth + 1)
                    })
                }
                ExpressionNode::Binary(binary) => {
                    validation::sealed_float_comparison_declaration(
                        program,
                        machine,
                        Some(state),
                        expression,
                        binary,
                    )
                    .is_some()
                        && [binary.left, binary.right].into_iter().all(|operand| {
                            validation::has_builtin_bound_expression_meaning(
                                program,
                                machine,
                                Some(state),
                                operand,
                            )
                        })
                }
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                    entry_predicate_has_meaning(program, machine, state, unary.operand, depth + 1)
                }
                _ => false,
            }
        }

        /*
            Scalar actuals use the ordinary checked algebra with observations rebound
            to authenticated invocation-entry subjects. This does not replay stored
            initializer expressions or establish arithmetic safety by itself.
        */
        pub(crate) fn lower_entry_scalar(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            machine: &Machine,
            state: &typed_trees::state::State,
            before_statement: usize,
            expression: ExpressionHandle,
            primitive_type: PrimitiveType,
            subjects: &impl Fn(&validation::CanonicalPlace) -> Option<(u32, Vec<PathSegment>)>,
        ) -> Option<CheckedScalarExpression> {
            if primitive_type == PrimitiveType::Bool {
                return lower_entry_predicate(
                    program,
                    operators,
                    machine,
                    state,
                    expression,
                    &mut |leaf| {
                        let place = place_at(program, state.symbol, before_statement, leaf)?;
                        subjects(&place)
                    },
                )
                .map(|value| CheckedScalarExpression::Boolean(Box::new(value)));
            }
            if !validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) {
                return None;
            }
            let mut reader = Reader {
                program,
                operators,
                machine: Some(machine),
                state: Some(state),
                owner: machine.symbol,
                parameters: super::authored_entry_parameters(program, machine)?,
                remaining: 4096,
                subjects: None,
                logical_captures: Vec::new(),
            };
            crate::values::lower_unit_scalar_argument_for_places(
                program,
                operators,
                state,
                before_statement,
                expression,
                primitive_type,
                |place, primitive| {
                    let (position, path) = subjects(place)?;
                    let parameter_position = usize::try_from(position).ok()?;
                    let parameter = reader.parameters.get(parameter_position)?;
                    if parameter.is_const || parameter.relevance.is_erased() {
                        return None;
                    }
                    let type_reference =
                        super::super::structural_fields::readable_entry_parameter_path(
                            program,
                            reader.parameters,
                            position,
                            &path,
                            &mut reader.remaining,
                        )?;
                    if reader.primitive(type_reference, 0)??.0 != primitive {
                        return None;
                    }
                    Some(
                        if path.is_empty()
                            && crate::values::scalar::occupies_scalar_position(program, parameter)
                        {
                            CheckedScalarExpression::Parameter {
                                position: crate::values::scalar::scalar_position_count(
                                    program,
                                    &reader.parameters[..parameter_position],
                                ),
                                primitive_type: primitive,
                            }
                        } else {
                            CheckedScalarExpression::StructuralParameterField {
                                observation: Default::default(),
                                parameter_position: position,
                                path,
                                primitive_type: primitive,
                            }
                        },
                    )
                },
            )
        }

        struct Reader<'program> {
            program: &'program dyn ::validation::CheckingInput,
            operators: &'program CheckedOperatorFacts,
            machine: Option<&'program Machine>,
            state: Option<&'program typed_trees::state::State>,
            owner: symbols::SymbolHandle,
            parameters: &'program [StateParameter],
            remaining: usize,
            subjects: Option<&'program mut EntrySubjectResolver<'program>>,
            logical_captures: Vec<LogicalMatchBinding>,
        }

        /*
            Check a requirement's crash condition against its own declared parameters.
            Bodyless requirements do not need a synthetic machine body.
        */
        pub(crate) fn lower_signature_crash_contract_expression(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            signature: &typed_trees::signature::StateSignature,
            expression: ExpressionHandle,
        ) -> Option<CheckedBooleanExpression> {
            if !signature.symbol.is_valid() {
                return None;
            }
            lower_declaration_crash_contract_expression(
                program,
                operators,
                signature.symbol,
                program.state_signature_parameters(signature),
                expression,
            )
        }

        /*
            Read the declaration's parameters. Unused generic parameters do not prevent
            checking a concrete condition; reading an unsupported type still rejects.
        */
        fn lower_declaration_crash_contract_expression(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            owner: symbols::SymbolHandle,
            parameters: &[StateParameter],
            expression: ExpressionHandle,
        ) -> Option<CheckedBooleanExpression> {
            for (position, parameter) in parameters.iter().enumerate() {
                if !parameter.symbol.is_valid()
                    || parameters[..position]
                        .iter()
                        .any(|prior| prior.symbol == parameter.symbol)
                {
                    return None;
                }
            }
            Reader {
                program,
                operators,
                machine: None,
                state: None,
                owner,
                parameters,
                remaining: 4096,
                subjects: None,
                logical_captures: Vec::new(),
            }
            .boolean(expression, 0)
        }

        /*
            An operator declaration publishes its crash routes over its own formal
            parameters, exactly as a bodyless signature does: the structured form
            binds scalar formals by dense scalar position, which the Terminal
            operation-contract telescope reads as formal `k + 1`. Structural leaves
            retain authored formal positions. A generic operator has no closed
            telescope yet, so it keeps identity without inventing executable operands.
        */
        pub(crate) fn lower_operator_crash_contract_expression(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            operator: &typed_trees::operator::OperatorDefinition,
            expression: ExpressionHandle,
        ) -> Option<CheckedBooleanExpression> {
            if !operator.symbol.is_valid()
                || !program.operator_type_parameters(operator).is_empty()
                || !operator.lifetime_parameters.is_empty()
            {
                return None;
            }
            lower_declaration_crash_contract_expression(
                program,
                operators,
                operator.symbol,
                program.operator_parameters(operator),
                expression,
            )
        }

        #[derive(Clone)]
        struct FieldPath {
            parameter_position: usize,
            path: Vec<PathSegment>,
            type_reference: TypeReferenceHandle,
        }

        type LogicalMatchBinding = validation::LogicalMatchBinding<FieldPath>;

        /*
            One comparison operand, resolved once: its carrier selects the comparison
            family and the same resolution lowers it.
        */
        enum ComparisonOperand {
            Constant {
                value: CheckedScalarExpression,
                reference: TypeReferenceHandle,
                integer: bool,
                ieee: bool,
            },
            /*
                A subject read; `static_subject` is present when the read needs no
                runtime selector, the only read form IEEE comparisons admit.
            */
            Subject {
                static_subject: Option<FieldPath>,
                value: CheckedScalarExpression,
                reference: TypeReferenceHandle,
                integer: bool,
                ieee: bool,
            },
            IntegerLiteral,
            Unsupported,
        }

        impl ComparisonOperand {
            fn is_ieee(&self) -> bool {
                matches!(self, ComparisonOperand::Constant { ieee: true, .. })
                    || matches!(self, ComparisonOperand::Subject { ieee: true, .. })
            }

            fn is_integer(&self) -> bool {
                matches!(self, ComparisonOperand::IntegerLiteral)
                    || matches!(self, ComparisonOperand::Constant { integer: true, .. })
                    || matches!(self, ComparisonOperand::Subject { integer: true, .. })
            }

            /* The IEEE operand this resolution supplies. */
            fn ieee_operand(self) -> Option<(CheckedScalarExpression, TypeReferenceHandle)> {
                match self {
                    ComparisonOperand::Constant {
                        value,
                        reference,
                        ieee: true,
                        ..
                    } => Some((value, reference)),
                    ComparisonOperand::Subject {
                        static_subject: Some(_),
                        ieee: true,
                        value,
                        reference,
                        ..
                    } => Some((value, reference)),
                    _ => None,
                }
            }
        }

        impl<'program> Reader<'program> {
            fn charge(&mut self, depth: usize) -> Option<()> {
                if depth >= 64 || self.remaining == 0 {
                    self.remaining = 0;
                    return None;
                }
                self.remaining -= 1;
                Some(())
            }

            fn boolean(
                &mut self,
                expression: ExpressionHandle,
                depth: usize,
            ) -> Option<CheckedBooleanExpression> {
                self.charge(depth)?;
                if !self
                    .program
                    .expression_table
                    .expression_is_valid(expression)
                {
                    return None;
                }
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::Boolean(value) => {
                        Some(CheckedBooleanExpression::Constant(*value))
                    }
                    ExpressionNode::Match(_) => self.logical_match_boolean(expression, depth + 1),
                    ExpressionNode::Name(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::SavedSubject(_) => {
                        let subject = self.subject(expression, depth + 1)?;
                        if self.primitive(subject.type_reference, depth + 1)?
                            != Some((PrimitiveType::Bool, BuiltinTypeAtom::Bool))
                        {
                            return None;
                        }
                        if subject.path.is_empty()
                            && super::super::occupies_scalar_position(
                                self.program,
                                &self.parameters[subject.parameter_position],
                            )
                        {
                            Some(CheckedBooleanExpression::Parameter {
                                position: self
                                    .scalar_position(subject.parameter_position, depth + 1)?,
                            })
                        } else {
                            Some(CheckedBooleanExpression::StructuralParameterField {
                                observation: Default::default(),
                                parameter_position: u32::try_from(subject.parameter_position)
                                    .ok()?,
                                path: subject.path,
                            })
                        }
                    }
                    ExpressionNode::Unary(unary)
                        if unary.operator == UnaryOperator::LogicalNot
                            && operator_is_builtin(self.operators, expression) =>
                    {
                        Some(CheckedBooleanExpression::Not(Box::new(
                            self.boolean(unary.operand, depth + 1)?,
                        )))
                    }
                    ExpressionNode::Binary(binary)
                        if matches!(
                            binary.operator,
                            BinaryOperator::And
                                | BinaryOperator::Or
                                | BinaryOperator::Equal
                                | BinaryOperator::NotEqual
                                | BinaryOperator::Less
                                | BinaryOperator::LessOrEqual
                                | BinaryOperator::Greater
                                | BinaryOperator::GreaterOrEqual
                        ) =>
                    {
                        /*
                            An IEEE comparison selects the sealed Float operator, which
                            is not a builtin one; `float_comparison` authenticates that
                            builtin-or-sealed meaning itself. Every other form still
                            needs the builtin operator and its builtin meaning here.
                        */
                        let builtin = operator_is_builtin(self.operators, expression)
                            && self.machine.is_none_or(|machine| {
                                validation::has_builtin_binary_expression_meaning(
                                    self.program,
                                    machine,
                                    self.state
                                        .or_else(|| self.program.machine_states(machine).first()),
                                    expression,
                                )
                            });
                        if !matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                            let operands = [
                                self.comparison_operand(binary.left, depth + 1)?,
                                self.comparison_operand(binary.right, depth + 1)?,
                            ];
                            let ieee = operands.iter().any(ComparisonOperand::is_ieee);
                            if !ieee && !builtin {
                                return None;
                            }
                            if ieee
                                || operands.iter().any(ComparisonOperand::is_integer)
                                || matches!(
                                    binary.operator,
                                    BinaryOperator::Less
                                        | BinaryOperator::LessOrEqual
                                        | BinaryOperator::Greater
                                        | BinaryOperator::GreaterOrEqual
                                )
                            {
                                self.comparison_telescope(depth + 1)?;
                                return if ieee {
                                    self.float_comparison(expression, operands)
                                } else {
                                    self.integer_comparison(expression, operands)
                                };
                            }
                        }
                        if !builtin {
                            return None;
                        }
                        let left = Box::new(self.boolean(binary.left, depth + 1)?);
                        let right = Box::new(self.boolean(binary.right, depth + 1)?);
                        if self.machine.is_none()
                            && matches!(
                                binary.operator,
                                BinaryOperator::Equal | BinaryOperator::NotEqual
                            )
                            && !typed_trees::operator::has_builtin_spelled_expression_meaning(
                                self.program,
                                self.owner,
                                expression,
                                if binary.operator == BinaryOperator::Equal {
                                    language_core::OperatorSpelling::Equal
                                } else {
                                    language_core::OperatorSpelling::NotEqual
                                },
                                &[None, None],
                            )
                        {
                            return None;
                        }
                        Some(match binary.operator {
                            BinaryOperator::And => CheckedBooleanExpression::And { left, right },
                            BinaryOperator::Or => CheckedBooleanExpression::Or { left, right },
                            BinaryOperator::Equal => {
                                CheckedBooleanExpression::Equal { left, right }
                            }
                            BinaryOperator::NotEqual => CheckedBooleanExpression::Not(Box::new(
                                CheckedBooleanExpression::Equal { left, right },
                            )),
                            _ => return None,
                        })
                    }
                    _ => None,
                }
            }

            /// One comparison operand resolved once; the family its carrier selects
            /// also lowers it.
            fn comparison_operand(
                &mut self,
                expression: ExpressionHandle,
                depth: usize,
            ) -> Option<ComparisonOperand> {
                self.charge(depth)?;
                if !self
                    .program
                    .expression_table
                    .expression_is_valid(expression)
                {
                    return None;
                }
                if let Some(constant) =
                    validation::completed_integer_constant(self.program, expression)
                {
                    let reference = constant.type_reference?;
                    let (primitive, atom) = self.primitive(reference, depth + 1)??;
                    if constant.primitive != Some(primitive) || !fixed_integer_atom(atom) {
                        return None;
                    }
                    return Some(ComparisonOperand::Constant {
                        value: CheckedScalarExpression::IntegerLiteral {
                            literal: validation::land_integer_value(&constant.value, primitive)?,
                        },
                        reference,
                        integer: true,
                        ieee: false,
                    });
                }
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::Match(_) => self.logical_match_operand(expression, depth + 1),
                    ExpressionNode::Float(literal) => {
                        let reference = validation::parameter_expression_result_type_reference(
                            self.program,
                            self.owner,
                            self.parameters,
                            expression,
                        )?;
                        let value = CheckedScalarExpression::IeeeFloatLiteral {
                            value: typed_trees::expression::ieee_float_value(
                                literal,
                                literal.landing()?,
                            ),
                        };
                        Some(ComparisonOperand::Constant {
                            value,
                            reference,
                            integer: false,
                            ieee: true,
                        })
                    }
                    ExpressionNode::Integer(_) => Some(ComparisonOperand::IntegerLiteral),
                    ExpressionNode::Name(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::SavedSubject(_) => {
                        let static_subject = self.subject(expression, depth + 2);
                        let (value, reference, atom) =
                            self.resolved_subject(expression, static_subject.as_ref(), depth + 1)?;
                        Some(ComparisonOperand::Subject {
                            static_subject,
                            value,
                            reference,
                            integer: atom.is_some_and(fixed_integer_atom),
                            ieee: atom.is_some_and(ieee_float_atom),
                        })
                    }
                    _ => Some(ComparisonOperand::Unsupported),
                }
            }

            /*
                Dense scalar positions depend on the entire entry telescope, including
                unread formals; both families share this validation.
            */
            fn comparison_telescope(&mut self, depth: usize) -> Option<()> {
                for parameter in self.parameters {
                    if parameter.is_const
                        || self.program.symbols.name(parameter.symbol) != parameter.name.as_str()
                    {
                        return None;
                    }
                    self.primitive(parameter.type_reference, depth + 1)?;
                }
                Some(())
            }

            fn integer_comparison(
                &self,
                expression: ExpressionHandle,
                operands: [ComparisonOperand; 2],
            ) -> Option<CheckedBooleanExpression> {
                let mut subjects = [None, None];
                for (position, operand) in operands.into_iter().enumerate() {
                    match operand {
                        ComparisonOperand::Subject {
                            value,
                            reference,
                            integer: true,
                            ..
                        }
                        | ComparisonOperand::Constant {
                            value,
                            reference,
                            integer: true,
                            ..
                        } => subjects[position] = Some((value, reference)),
                        ComparisonOperand::IntegerLiteral => {}
                        /*
                            This slice establishes no totality for arithmetic, calls,
                            casts, result values, or current body storage.
                        */
                        _ => return None,
                    }
                }
                /*
                    Literal landing, same-carrier checks, and selected operator meaning
                    stay with the existing numeric contract owner. Trapping-qualified
                    inputs are legal: the comparison itself is a total operation.
                */
                super::super::result_contract::lower_integer_contract_comparison(
                    self.program,
                    self.operators,
                    self.owner,
                    expression,
                    subjects,
                )
            }

            /*
                IEEE relations retain their own meaning, including unordered inputs.
                Read exact formal identities and already-landed literal bits; never
                reinterpret them as integer order or round them through a host float.
                The shared contract owner checks both carriers and the meaning.
            */
            fn float_comparison(
                &self,
                expression: ExpressionHandle,
                operands: [ComparisonOperand; 2],
            ) -> Option<CheckedBooleanExpression> {
                let [left, right] = operands;
                super::super::result_contract::lower_float_contract_comparison(
                    self.program,
                    self.owner,
                    expression,
                    [left.ieee_operand(), right.ieee_operand()],
                )
            }

            /*
                Read ordered selections with the ordinary scalar projection owner.
                Authenticate the collection and every selector separately: neither a
                stable collection nor a saved selector alone authenticates their read.
                This translation supplies meaning, never an indexing bound; the
                caller's already-resolved static subject keeps the authored walk to
                one pass per operand.
            */
            fn resolved_subject(
                &mut self,
                expression: ExpressionHandle,
                subject: Option<&FieldPath>,
                depth: usize,
            ) -> Option<(
                CheckedScalarExpression,
                TypeReferenceHandle,
                Option<BuiltinTypeAtom>,
            )> {
                self.charge(depth)?;
                if let Some(subject) = subject {
                    let (primitive, atom) = self.primitive(subject.type_reference, depth + 1)??;
                    let value = self.subject_scalar(subject, primitive, depth + 1)?;
                    return Some((value, subject.type_reference, Some(atom)));
                }
                let source_parameters = self
                    .state
                    .map(|state| self.program.state_parameters(state))
                    .unwrap_or(self.parameters);
                let program = self.program;
                let operators = self.operators;
                let (mut value, _) =
                    super::super::scalar_lowering::scalar_storage_read_with_selectors(
                        program,
                        operators,
                        expression,
                        source_parameters,
                        |_, selector, _| {
                            let selector_subject = self.subject(selector, depth + 2);
                            self.resolved_subject(selector, selector_subject.as_ref(), depth + 1)
                                .map(|(value, _, _)| value)
                                .or_else(|| {
                                    super::super::scalar_lowering::land_anonymous_scalar_expression(
                                        program,
                                        operators,
                                        selector,
                                        PrimitiveType::U64,
                                    )
                                })
                        },
                    )?;
                let CheckedScalarExpression::StructuralParameterIndexedRead {
                    root,
                    path,
                    primitive_type,
                    ..
                } = &mut value
                else {
                    return None;
                };
                let mut cursor = expression;
                loop {
                    self.charge(depth)?;
                    match program.expression_table.expression(cursor) {
                        ExpressionNode::Indexed(indexed) => cursor = indexed.collection,
                        ExpressionNode::Member(member) => cursor = member.receiver,
                        ExpressionNode::Name(_) => break,
                        _ => return None,
                    }
                }
                let subject = self.subject(cursor, depth + 1)?;
                let mut entry_path = subject.path;
                entry_path.append(path);
                *path = entry_path;
                *root = checked_trees::CheckedStorageRoot::Parameter {
                    index: u32::try_from(subject.parameter_position).ok()?,
                };
                let reference = if let (Some(machine), Some(state)) = (self.machine, self.state) {
                    validation::declared_place_type_raw(program, machine, Some(state), expression)?
                } else {
                    validation::parameter_expression_result_type_reference(
                        program,
                        self.owner,
                        source_parameters,
                        expression,
                    )?
                };
                let reference =
                    validation::field_domains::readable_type_reference(program, reference)?;
                let (primitive, atom) = self.primitive(reference, depth + 1)??;
                (primitive == *primitive_type).then_some((value, reference, Some(atom)))
            }

            /*
                Scalar positions are dense; structural roots retain the authored formal
                position. Both integer and IEEE leaves use this same entry namespace.
            */
            fn subject_scalar(
                &mut self,
                subject: &FieldPath,
                primitive_type: PrimitiveType,
                depth: usize,
            ) -> Option<CheckedScalarExpression> {
                Some(
                    if subject.path.is_empty()
                        && super::super::occupies_scalar_position(
                            self.program,
                            &self.parameters[subject.parameter_position],
                        )
                    {
                        CheckedScalarExpression::Parameter {
                            position: self
                                .scalar_position(subject.parameter_position, depth + 1)?,
                            primitive_type,
                        }
                    } else {
                        CheckedScalarExpression::StructuralParameterField {
                            observation: Default::default(),
                            parameter_position: u32::try_from(subject.parameter_position).ok()?,
                            path: subject.path.clone(),
                            primitive_type,
                        }
                    },
                )
            }

            fn scalar_position(&mut self, position: usize, depth: usize) -> Option<usize> {
                let mut scalar_position = 0;
                for parameter in &self.parameters[..position] {
                    self.primitive(parameter.type_reference, depth + 1)?;
                    if super::super::occupies_scalar_position(self.program, parameter) {
                        scalar_position += 1;
                    }
                }
                Some(scalar_position)
            }

            fn subject(&mut self, expression: ExpressionHandle, depth: usize) -> Option<FieldPath> {
                if !self.logical_captures.is_empty() && self.is_saved_projection(expression) {
                    return self.logical_saved_path(expression, depth + 1);
                }
                if let Some(resolve) = self.subjects.as_deref_mut() {
                    let (position, path) = resolve(expression)?;
                    let parameter = self.parameters.get(usize::try_from(position).ok()?)?;
                    if parameter.is_const {
                        return None;
                    }
                    let type_reference =
                        super::super::structural_fields::readable_entry_parameter_path(
                            self.program,
                            self.parameters,
                            position,
                            &path,
                            &mut self.remaining,
                        )?;
                    return Some(FieldPath {
                        parameter_position: usize::try_from(position).ok()?,
                        path,
                        type_reference,
                    });
                }
                if matches!(
                    self.program.expression_table.expression(expression),
                    ExpressionNode::Name(_)
                ) {
                    let position = self.parameter(expression)?;
                    Some(FieldPath {
                        parameter_position: position,
                        path: Vec::new(),
                        type_reference: validation::field_domains::readable_type_reference(
                            self.program,
                            super::super::structural_fields::readable_entry_parameter_path(
                                self.program,
                                self.parameters,
                                u32::try_from(position).ok()?,
                                &[],
                                &mut self.remaining,
                            )?,
                        )?,
                    })
                } else {
                    self.field_path(expression, depth)
                }
            }

            fn parameter(&self, expression: ExpressionHandle) -> Option<usize> {
                let name = self.program.expression_table.name_path_node(expression)?;
                let [spelling] = self
                    .program
                    .expression_table
                    .name_path_members(name.members)
                else {
                    return None;
                };
                self.parameters.iter().position(|parameter| {
                    !parameter.is_const
                        && parameter.name.as_str() == spelling.as_str()
                        && self.program.symbols.name(parameter.symbol) == parameter.name.as_str()
                        && {
                            name.symbol == parameter.symbol && name.head_symbol == parameter.symbol
                        }
                })
            }

            fn field_path(
                &mut self,
                expression: ExpressionHandle,
                depth: usize,
            ) -> Option<FieldPath> {
                let mut current = expression;
                let mut depth = depth;
                loop {
                    self.charge(depth)?;
                    if !self.program.expression_table.expression_is_valid(current) {
                        return None;
                    }
                    match self.program.expression_table.expression(current) {
                        ExpressionNode::Member(member) => current = member.receiver,
                        ExpressionNode::Indexed(indexed) => current = indexed.collection,
                        ExpressionNode::Name(_) => break,
                        _ => return None,
                    }
                    depth += 1;
                }
                let (position, path, selected_type) =
                    super::super::structural_fields::structural_parameter_place(
                        self.program,
                        self.parameters,
                        expression,
                    )?;
                let type_reference =
                    super::super::structural_fields::readable_entry_parameter_path(
                        self.program,
                        self.parameters,
                        position,
                        &path,
                        &mut self.remaining,
                    )?;
                (type_reference == selected_type).then_some(FieldPath {
                    parameter_position: usize::try_from(position).ok()?,
                    path,
                    type_reference,
                })
            }

            fn primitive(
                &mut self,
                mut type_reference: TypeReferenceHandle,
                mut depth: usize,
            ) -> Option<Option<(PrimitiveType, BuiltinTypeAtom)>> {
                loop {
                    self.charge(depth)?;
                    if !self
                        .program
                        .type_reference_table
                        .contains_type_reference(type_reference)
                    {
                        return None;
                    }
                    match self
                        .program
                        .type_reference_table
                        .type_reference(type_reference)
                    {
                        TypeReferenceNode::Constrained { base_type, .. } => {
                            type_reference = *base_type
                        }
                        TypeReferenceNode::Named { symbol, name } => {
                            let Some(primitive) = PrimitiveType::from_name(name) else {
                                return Some(None);
                            };
                            let atom = self.program.symbols.builtin_type_atom(*symbol)?;
                            if self.program.symbols.name(*symbol) != name.as_str()
                                || PrimitiveType::from_name(self.program.symbols.name(*symbol))
                                    != Some(primitive)
                            {
                                return None;
                            }
                            let expected = match atom {
                                BuiltinTypeAtom::Bool | BuiltinTypeAtom::AtomicBool => {
                                    PrimitiveType::Bool
                                }
                                BuiltinTypeAtom::I8 => PrimitiveType::I8,
                                BuiltinTypeAtom::I16 => PrimitiveType::I16,
                                BuiltinTypeAtom::I32 => PrimitiveType::I32,
                                BuiltinTypeAtom::I64 => PrimitiveType::I64,
                                BuiltinTypeAtom::U8 => PrimitiveType::U8,
                                BuiltinTypeAtom::U16 => PrimitiveType::U16,
                                BuiltinTypeAtom::U32 | BuiltinTypeAtom::AtomicU32 => {
                                    PrimitiveType::U32
                                }
                                BuiltinTypeAtom::U64 | BuiltinTypeAtom::AtomicU64 => {
                                    PrimitiveType::U64
                                }
                                BuiltinTypeAtom::F32 => PrimitiveType::F32,
                                BuiltinTypeAtom::F64 => PrimitiveType::F64,
                                BuiltinTypeAtom::Address => PrimitiveType::Addr,
                                _ => return None,
                            };
                            return (primitive == expected).then_some(Some((primitive, atom)));
                        }
                        _ => return Some(None),
                    }
                    depth += 1;
                }
            }
        }

        fn fixed_integer_atom(atom: BuiltinTypeAtom) -> bool {
            matches!(
                atom,
                BuiltinTypeAtom::I8
                    | BuiltinTypeAtom::I16
                    | BuiltinTypeAtom::I32
                    | BuiltinTypeAtom::I64
                    | BuiltinTypeAtom::U8
                    | BuiltinTypeAtom::U16
                    | BuiltinTypeAtom::U32
                    | BuiltinTypeAtom::U64
            )
        }

        fn ieee_float_atom(atom: BuiltinTypeAtom) -> bool {
            matches!(atom, BuiltinTypeAtom::F32 | BuiltinTypeAtom::F64)
        }
    }
    pub(crate) use crash_entry::lower_operator_crash_contract_expression;
    pub(crate) use crash_entry::lower_signature_crash_contract_expression;
    pub(crate) use crash_entry::{
        lower_current_state_predicate, lower_entry_predicate, lower_entry_scalar,
        lower_machine_entry_crash_contract_expression,
    };

    /*
        Scalar contracts and structural crash predicates share exact authored
        machine/state/formal identity, without borrowing each other's leaf vocabulary
        or eligibility policy. Unread type parameters do not erase concrete clauses.
    */
    pub(crate) fn authored_entry_parameters<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Option<&'program [StateParameter]> {
        if !machine.symbol.is_valid()
            || program.symbols.get(machine.symbol).kind != symbols::SymbolKind::Machine
            || program
                .machines()
                .iter()
                .filter(|candidate| candidate.symbol == machine.symbol)
                .count()
                != 1
        {
            return None;
        }
        match &machine.attached_data {
            None if machine.attached_data_symbol.is_valid() => return None,
            None => {}
            Some(_) if !machine.attached_data_symbol.is_valid() => {}
            Some(name) => {
                let mut owners = program
                    .data_definitions()
                    .iter()
                    .filter(|data| data.symbol == machine.attached_data_symbol);
                let owner = owners.next()?;
                if owners.next().is_some()
                    || program.symbols.get(owner.symbol).kind != symbols::SymbolKind::Data
                    || owner.name.as_str() != name.as_str()
                {
                    return None;
                }
            }
        }
        let entry = program.machine_states(machine).first()?;
        let entry_symbol = program.symbols.get(entry.symbol);
        if entry_symbol.kind != symbols::SymbolKind::State || entry_symbol.parent != machine.symbol
        {
            return None;
        }
        let parameters = program.state_parameters(entry);
        for (position, parameter) in parameters.iter().enumerate() {
            let symbol = program.symbols.get(parameter.symbol);
            if symbol.kind != symbols::SymbolKind::Parameter
                || symbol.parent != entry.symbol
                || parameters[..position]
                    .iter()
                    .any(|prior| prior.symbol == parameter.symbol || prior.name == parameter.name)
            {
                return None;
            }
        }
        Some(parameters)
    }

    pub(crate) fn lower_machine_entry_boolean_expression(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        machine: &typed_trees::machine::Machine,
        expression: ExpressionHandle,
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> Option<CheckedBooleanExpression> {
        let mut predicate = lower_machine_parameter_boolean_expression(
            program,
            operators,
            machine,
            expression,
            exact_integer_casts,
        )?;
        let entry = program.machine_states(machine).first()?;
        EntryOperands {
            program,
            parameters: program.state_parameters(entry),
        }
        .boolean(&mut predicate)?;
        Some(predicate)
    }

    /*
        Lower a scalar membership subject as the caller supplies it at invocation.
        The entry state's statement-0 namespace reads a `mut` scalar formal through
        its body storage; before any statement runs that storage still holds the
        argument, so the read is rebound to the formal's dense entry position. A
        requirement row published on the call boundary cannot name body storage.
    */
    pub(crate) fn lower_machine_entry_scalar_argument(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        machine: &typed_trees::machine::Machine,
        expression: ExpressionHandle,
        primitive_type: PrimitiveType,
    ) -> Option<CheckedScalarExpression> {
        let entry = program.machine_states(machine).first()?;
        let mut subject = crate::values::lower_unit_scalar_argument(
            program,
            operators,
            entry,
            0,
            expression,
            primitive_type,
        )?;
        EntryOperands {
            program,
            parameters: program.state_parameters(entry),
        }
        .scalar(&mut subject)?;
        Some(subject)
    }

    struct EntryOperands<'program> {
        program: &'program dyn ::validation::CheckingInput,
        parameters: &'program [StateParameter],
    }

    impl EntryOperands<'_> {
        fn position(
            &self,
            symbol: symbols::SymbolHandle,
            primitive: PrimitiveType,
        ) -> Option<usize> {
            let mut matches = self
                .parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| symbol.is_valid() && parameter.symbol == symbol);
            let (position, parameter) = matches.next()?;
            if matches.next().is_some()
                || parameter.relevance.is_erased()
                || crate::values::mutable_scalar_parameter_type(self.program, parameter)
                    != Some(primitive)
            {
                return None;
            }
            Some(crate::values::scalar::scalar_position_count(
                self.program,
                &self.parameters[..position],
            ))
        }

        /*
            Dense erased-scalar index of the parameter carrying `symbol`, or `None`
            when no uniquely named erased scalar formal exists. Erased formals share
            the authored parameter roster but index their own proof-only namespace.
        */
        fn erased_position(
            &self,
            symbol: symbols::SymbolHandle,
            primitive: PrimitiveType,
        ) -> Option<usize> {
            let mut matches = self
                .parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| symbol.is_valid() && parameter.symbol == symbol);
            let (position, parameter) = matches.next()?;
            if matches.next().is_some()
                || !parameter.relevance.is_erased()
                || self
                    .program
                    .erased_scalar_observation_type(parameter.type_reference)
                    != Some(primitive)
            {
                return None;
            }
            Some(
                self.program
                    .erased_scalar_lane_base(self.parameters, position),
            )
        }

        fn scalar(&self, expression: &mut CheckedScalarExpression) -> Option<()> {
            match expression {
                CheckedScalarExpression::StorageRead {
                    symbol,
                    primitive_type,
                } => {
                    *expression = if let Some(position) = self.position(*symbol, *primitive_type) {
                        CheckedScalarExpression::Parameter {
                            position,
                            primitive_type: *primitive_type,
                        }
                    } else {
                        CheckedScalarExpression::ErasedParameter {
                            position: self.erased_position(*symbol, *primitive_type)?,
                            primitive_type: *primitive_type,
                        }
                    };
                }
                CheckedScalarExpression::StructuralSequenceLength {
                    root: checked_trees::CheckedStorageRoot::Parameter { .. },
                    path,
                } => {
                    for step in path {
                        if let checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) =
                            step
                        {
                            self.scalar(&mut selector.expression)?;
                        }
                    }
                }
                CheckedScalarExpression::StructuralParameterIndexedRead {
                    root: checked_trees::CheckedStorageRoot::Parameter { .. },
                    index,
                    element_path,
                    ..
                } => {
                    self.scalar(&mut index.expression)?;
                    for step in element_path {
                        if let checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) =
                            step
                        {
                            self.scalar(&mut selector.expression)?;
                        }
                    }
                }
                CheckedScalarExpression::Local { .. }
                | CheckedScalarExpression::StructuralLocalField { .. }
                | CheckedScalarExpression::StructuralSequenceLength { .. }
                | CheckedScalarExpression::StructuralParameterIndexedRead { .. } => return None,
                CheckedScalarExpression::RepresentationView { operand, .. } => {
                    self.scalar(operand)?
                }
                CheckedScalarExpression::IntegerBinary { left, right, .. } => {
                    self.scalar(left)?;
                    self.scalar(right)?;
                }
                checked_trees::integer_widen_triple_pattern!(operand, ..) => {
                    self.scalar(operand)?
                }
                CheckedScalarExpression::Boolean(expression) => self.boolean(expression)?,
                CheckedScalarExpression::ConstantApplication { .. }
                | CheckedScalarExpression::Parameter { .. }
                | CheckedScalarExpression::ErasedParameter { .. }
                | CheckedScalarExpression::StructuralParameterField { .. }
                | CheckedScalarExpression::IntegerLiteral { .. }
                | CheckedScalarExpression::IeeeFloatLiteral { .. }
                | CheckedScalarExpression::ImplementationSelection { .. } => {}
            }
            Some(())
        }

        fn boolean(&self, expression: &mut CheckedBooleanExpression) -> Option<()> {
            match expression {
                CheckedBooleanExpression::StorageRead { symbol } => {
                    *expression =
                        if let Some(position) = self.position(*symbol, PrimitiveType::Bool) {
                            CheckedBooleanExpression::Parameter { position }
                        } else {
                            CheckedBooleanExpression::ErasedParameter {
                                position: self.erased_position(*symbol, PrimitiveType::Bool)?,
                            }
                        };
                }
                CheckedBooleanExpression::Local { .. }
                | CheckedBooleanExpression::IntegerMathComparison { .. } => return None,
                CheckedBooleanExpression::Not(operand) => self.boolean(operand)?,
                facts::checked_boolean_triple_pattern!(left, right, ..) => {
                    self.boolean(left)?;
                    self.boolean(right)?;
                }
                CheckedBooleanExpression::IntegerComparison { left, right, .. }
                | CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
                    self.scalar(left)?;
                    self.scalar(right)?;
                }
                CheckedBooleanExpression::Parameter { .. }
                | CheckedBooleanExpression::ErasedParameter { .. }
                | CheckedBooleanExpression::Constant(_)
                | CheckedBooleanExpression::StructuralParameterField { .. }
                | CheckedBooleanExpression::IeeeFloatComparison { .. }
                | CheckedBooleanExpression::StructuralIdentity { .. }
                | CheckedBooleanExpression::ByteSequenceEqual { .. }
                | CheckedBooleanExpression::ByteSequencePredicate { .. }
                | CheckedBooleanExpression::StructuralCaseMembership { .. } => {}
            }
            Some(())
        }
    }
}
pub(crate) mod expression_facts {
    /*
        Facts about scalar expressions the lowerings share: parameter and local
        positions, expression types, builtin operators and arithmetic domains.
    */
    use crate::flow::expressions;
    use crate::values::scalar::expression_plans::ScalarLocal;
    use checked_trees::{
        CheckedOperatorFacts, CheckedOperatorResolutionStatus,
        CheckedScalarExpression as ScalarExpression,
    };
    use numerics::arithmetic::ArithmeticDomain;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::StateParameter;
    use typed_trees::types::PrimitiveType;

    /*
        Whether an authored parameter occupies a position in the dense scalar
        namespace that `ScalarExpression::Parameter` indexes: a primitive
        carrier that is not an `[erased]` binding occurrence. An erased primitive
        parameter owns no position (contracts.md#explicit-erased-bindings), so the
        retained parameters after it shift down, matching the stripped signature
        plan in `execution::unit::calls::signatures`; a runtime read of it then
        finds no position and fails closed here as well as in validation.
    */
    pub(crate) fn occupies_scalar_position(
        program: &dyn ::validation::CheckingInput,
        parameter: &StateParameter,
    ) -> bool {
        !parameter.relevance.is_erased()
            && program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
    }

    /*
        The scalar-parameter roster and each retained parameter's primitive
        carrier, in authored order. `None` when a retained parameter has no
        primitive carrier. Every lowerer that reads the dense scalar namespace
        takes its roster from here rather than rebuilding the filter.
    */
    pub(crate) fn scalar_parameter_roster(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
    ) -> Option<(Vec<StateParameter>, Vec<PrimitiveType>)> {
        let scalar_parameters = parameters
            .iter()
            .filter(|parameter| occupies_scalar_position(program, parameter))
            .cloned()
            .collect::<Vec<_>>();
        let parameter_types = scalar_parameters
            .iter()
            .map(|parameter| program.primitive_type_reference(parameter.type_reference))
            .collect::<Option<Vec<_>>>()?;
        Some((scalar_parameters, parameter_types))
    }

    /*
        The dense scalar position a parameter at `parameters.len()` would take:
        the number of retained scalar parameters in the slice.
    */
    pub(crate) fn scalar_position_count(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
    ) -> usize {
        parameters
            .iter()
            .filter(|parameter| occupies_scalar_position(program, parameter))
            .count()
    }

    pub(crate) fn parameter_position(
        program: &dyn ::validation::CheckingInput,
        path: &typed_trees::expression::TableNamePath,
        parameters: &[StateParameter],
    ) -> Option<usize> {
        let members = program.expression_table.name_path_members(path.members);
        if path.symbol.is_valid() && path.head_symbol.is_valid() && path.symbol != path.head_symbol
        {
            return None;
        }
        (members.len() == 1)
            .then(|| {
                parameters.iter().position(|parameter| {
                    if path.symbol.is_valid() {
                        parameter.symbol == path.symbol
                    } else if path.head_symbol.is_valid() {
                        parameter.symbol == path.head_symbol
                    } else {
                        parameter.name.as_str() == members[0].as_str()
                    }
                })
            })
            .flatten()
    }

    pub(crate) fn local_position(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        path: &typed_trees::expression::TableNamePath,
        locals: &[ScalarLocal],
    ) -> Option<usize> {
        if path.symbol.is_valid() && path.head_symbol.is_valid() && path.symbol != path.head_symbol
        {
            return None;
        }
        (expressions(program).name_path_members(path.members).len() == 1)
            .then(|| {
                locals.iter().rposition(|local| {
                    if local.is_mutable {
                        local.symbol.is_valid()
                            && local.symbol == path.symbol
                            && local.symbol == path.head_symbol
                    } else if path.symbol.is_valid() {
                        local.symbol == path.symbol
                    } else if path.head_symbol.is_valid() {
                        local.symbol == path.head_symbol
                    } else {
                        local.name == program.expression_table.display_name(expression)
                    }
                })
            })
            .flatten()
    }

    pub(crate) fn scalar_expression_type(expression: &ScalarExpression) -> Option<PrimitiveType> {
        expression.primitive_type()
    }

    /*
        Plain primitive declarations establish zero without an authored
        initializer. Keep this tied to the declaration: a missing expression at
        another site or a reference has no such meaning. Arithmetic policy alone
        permits zero; other qualifications still need their establishment evidence.
    */
    pub(crate) fn zeroed_scalar_local_initializer(
        program: &dyn ::validation::CheckingInput,
        local: &typed_trees::statement::TableLocalData,
    ) -> Option<ScalarExpression> {
        use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix};
        if local.initial_value.is_valid()
            || !validation::is_plain_scalar_storage(program, local.type_reference)
        {
            return None;
        }
        let primitive = program.primitive_type_reference(local.type_reference)?;
        let landed_type = match primitive {
            PrimitiveType::Bool => {
                return Some(ScalarExpression::Boolean(Box::new(
                    checked_trees::CheckedBooleanExpression::Constant(false),
                )));
            }
            PrimitiveType::F32 => {
                return Some(ScalarExpression::IeeeFloatLiteral {
                    value: semantic_vocabulary::IeeeFloatValue::Binary32(0),
                });
            }
            PrimitiveType::F64 => {
                return Some(ScalarExpression::IeeeFloatLiteral {
                    value: semantic_vocabulary::IeeeFloatValue::Binary64(0),
                });
            }
            primitive => primitive.landed_landing()?,
        };
        Some(ScalarExpression::IntegerLiteral {
            literal: IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "0")
                .ok()?
                .with_landing(IntegerLanding {
                    landed_type,
                    domain: ArithmeticDomain::Exact,
                }),
        })
    }

    pub(crate) fn is_integer(primitive: PrimitiveType) -> bool {
        !matches!(
            primitive,
            PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
        )
    }

    pub(crate) fn operator_is_builtin(
        operators: &CheckedOperatorFacts,
        expression: ExpressionHandle,
    ) -> bool {
        operators
            .expression_use(expression)
            .is_none_or(|operator_use| {
                operator_use.status == CheckedOperatorResolutionStatus::BuiltinFallback
            })
    }

    pub(crate) fn combine_arithmetic_domains(
        left: ArithmeticDomain,
        right: ArithmeticDomain,
    ) -> Option<ArithmeticDomain> {
        match (left, right) {
            (ArithmeticDomain::Exact, domain) | (domain, ArithmeticDomain::Exact) => Some(domain),
            (left, right) if left == right => Some(left),
            _ => None,
        }
    }

    pub(crate) use validation::checked_integer_binary_kind;
}
pub(crate) mod expression_plans {
    /*
        Building the checked scalar expression plans of one program: the scalar
        locals, the plan walk and the closed literal guards it lowers.

        The walk visits every statement of every state once. A
        `StatementPlanner` carries the state's scalar signature and the rosters
        the statement extends; `array_elements` plans a closed array literal's
        elements first, and the statement's kind selects its own planner beside
        this file.
    */
    use crate::flow::indexed_statements;
    use crate::values::scalar::boolean_lowering::lower_boolean_guard;
    use crate::values::scalar::call_lowering::{
        LoweredCallArguments, lower_call_arguments, lower_direct_call_binding_arguments,
        retain_call_arguments, value_call_expression,
    };
    use crate::values::scalar::expression_facts::operator_is_builtin;
    use crate::values::scalar::scalar_lowering::{lower_index_expression, lower_return_expression};
    use crate::values::scalar::selected_operator_operands::lower_selected_operator_operands;
    use crate::values::scalar::{scalar_parameter_roster, semantic_casts};
    use checked_trees::{
        CheckedBooleanExpression, CheckedLocatedScalarExpression, CheckedOperatorFacts,
        CheckedScalarExpression, CheckedScalarExpressionBindings, CheckedScalarExpressionPlans,
        CheckedScalarExpressionRole,
    };
    use numerics::arithmetic::ArithmeticDomain;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    mod array_elements {
        /*
            Array literal elements: an immutable local, a returned value or an
            assignment whose destination is a closed primitive array plans each
            element as a scalar expression under its `ArrayElement` role.
        */
        use super::{
            CheckedScalarExpressionRole, StatementNode, StatementPlanner, lower_return_expression,
        };

        pub(crate) fn plan(planner: &mut StatementPlanner<'_, '_>, statement: &StatementNode) {
            let array_destination = match statement {
                StatementNode::LocalData(local) if !local.is_mutable => {
                    Some((local.initial_value, local.type_reference, local.symbol))
                }
                StatementNode::Expression(expression) => Some((
                    *expression,
                    planner.state.return_type,
                    symbols::SymbolHandle::invalid(),
                )),
                StatementNode::Assignment(assignment) => validation::assignment_stored_value_type(
                    planner.program,
                    planner.machine,
                    planner.state,
                    assignment,
                )
                .and_then(|declared| validation::closed_array_store_type(planner.program, declared))
                .map(|expected| (assignment.value, expected, symbols::SymbolHandle::invalid())),
                _ => None,
            };
            if let Some((expression, expected, destination)) = array_destination
                && let Some(elements) = validation::scalar_array_elements(
                    planner.program,
                    planner.machine.symbol,
                    expression,
                    expected,
                )
                && elements.all_authored()
            {
                for (element_index, element) in elements.elements.into_iter().enumerate() {
                    let validation::ScalarArrayElement::Authored {
                        expression: element,
                        primitive_type,
                    } = element
                    else {
                        continue;
                    };
                    let Ok(element_ordinal) = u32::try_from(element_index) else {
                        break;
                    };
                    let Some(value) = lower_return_expression(
                        planner.program,
                        planner.operators,
                        element,
                        planner.scalar_parameters,
                        planner.parameters,
                        planner.parameter_types,
                        planner.locals,
                        primitive_type,
                        planner.exact_integer_casts,
                    ) else {
                        continue;
                    };
                    let role = CheckedScalarExpressionRole::ArrayElement {
                        source: checked_trees::CheckedArrayConstructionSource::Statement,
                        element_ordinal,
                    };
                    planner.record_scalar_value_pair(role, destination, element, value);
                }
            }
        }
    }
    mod assignment {
        /*
            An assignment: an atomic carrier's authored operands, each index of the
            target place (`AssignmentIndex`) and the stored value (`AssignmentValue`).
        */
        use super::{
            CheckedScalarExpressionRole, ExpressionNode, StatementPlanner,
            assignment_target_primitive_type, lower_index_expression, lower_return_expression,
            retain_subslice_endpoints,
        };

        pub(crate) fn plan(
            planner: &mut StatementPlanner<'_, '_>,
            assignment: &typed_trees::statement::TableAssignment,
        ) {
            if let Some(carrier) =
                validation::atomic_assignment_carrier(planner.program, assignment)
                && let Some(operands) = super::super::atomic_operands::lower_carrier(
                    planner.program,
                    planner.operators,
                    planner.machine,
                    planner.state,
                    carrier,
                    planner.scalar_parameters,
                    planner.parameters,
                    planner.parameter_types,
                    planner.locals,
                    planner.exact_integer_casts,
                )
            {
                retain_subslice_endpoints(
                    operands,
                    planner.state.symbol,
                    planner.statement_ordinal,
                    planner.scalar_parameters,
                    planner.locals,
                    planner.expressions,
                    planner.source_bindings,
                    planner.binding_symbols,
                );
                return;
            }
            for (depth, index) in
                validation::assignment_target_selectors(planner.program, assignment.target)
                    .into_iter()
                    .enumerate()
            {
                let Ok(depth) = u32::try_from(depth) else {
                    break;
                };
                let Some(expression) = lower_index_expression(
                    planner.program,
                    planner.operators,
                    index,
                    planner.scalar_parameters,
                    planner.parameters,
                    planner.parameter_types,
                    planner.locals,
                    planner.exact_integer_casts,
                ) else {
                    continue;
                };
                planner.record_scalar_value_pair(
                    CheckedScalarExpressionRole::AssignmentIndex { depth },
                    symbols::SymbolHandle::invalid(),
                    index,
                    expression,
                );
            }
            if let Some(expression) =
                super::value_call_expression(planner.program, assignment.value)
            {
                planner.retain_expression_call_arguments(expression);
            }
            let Some(target_type_reference) = validation::expression_type_reference_in_state(
                planner.program,
                planner.state.symbol,
                planner.statement_index,
                assignment.target,
            ) else {
                return;
            };
            let Some(target_type) =
                assignment_target_primitive_type(planner.program, target_type_reference)
            else {
                return;
            };
            let Some(expression) = lower_return_expression(
                planner.program,
                planner.operators,
                assignment.value,
                planner.scalar_parameters,
                planner.parameters,
                planner.parameter_types,
                planner.locals,
                target_type,
                planner.exact_integer_casts,
            ) else {
                return;
            };
            planner.record_scalar_value_pair(
                CheckedScalarExpressionRole::AssignmentValue,
                match planner
                    .program
                    .expression_table
                    .expression(assignment.target)
                {
                    ExpressionNode::Name(path) => path.symbol,
                    _ => symbols::SymbolHandle::invalid(),
                },
                assignment.value,
                expression,
            );
        }
    }
    mod call_statement {
        /*
            A statement call's scalar arguments.
        */
        use super::{StatementPlanner, lower_call_arguments, retain_call_arguments};
        use typed_trees::statement::TableCall;
        use validation::CallSite;

        pub(crate) fn plan(planner: &mut StatementPlanner<'_, '_>, call: &TableCall) {
            if let Some(arguments) = lower_call_arguments(
                planner.program,
                planner.operators,
                planner.state,
                planner.statement_ordinal,
                0,
                &CallSite::Statement(call),
                planner.scalar_parameters,
                planner.parameters,
                planner.parameter_types,
                planner.locals,
                planner.exact_integer_casts,
            ) {
                retain_call_arguments(
                    arguments,
                    planner.scalar_parameters,
                    planner.locals,
                    planner.expressions,
                    planner.proof_terms,
                    planner.source_bindings,
                    planner.binding_symbols,
                );
            }
        }
    }
    mod expression_statement {
        /*
            A state's final expression: a returned value call's scalar arguments,
            or the returned scalar itself (`Return`). Structural constructions retain
            their operands through the shared value producer, not a return-only role.
        */
        use super::{
            CheckedScalarExpressionRole, ExpressionHandle, StatementPlanner,
            lower_return_expression,
        };

        pub(crate) fn plan(planner: &mut StatementPlanner<'_, '_>, expression: ExpressionHandle) {
            let unit_statement = validation::unit_statement_call_is_admitted(
                planner.program,
                planner.machine,
                planner.state,
                expression,
            );
            planner.retain_expression_call_arguments(expression);
            if !unit_statement
                && let Some(result_type) = planner.result_type
                && let Some(return_expression) = lower_return_expression(
                    planner.program,
                    planner.operators,
                    expression,
                    planner.scalar_parameters,
                    planner.parameters,
                    planner.parameter_types,
                    planner.locals,
                    result_type,
                    planner.exact_integer_casts,
                )
            {
                planner.record_scalar_value_pair(
                    CheckedScalarExpressionRole::Return,
                    symbols::SymbolHandle::invalid(),
                    expression,
                    return_expression,
                );
            }
        }
    }
    pub(crate) mod local_data {
        /*
            A `let` initializer: a value call's scalar arguments, a view local's
            subslice endpoints, or the initializer itself under its
            `LocalInitializer` or `StorageInitializer` role. Every primitive local
            then joins the state's local roster, which later statements read.
            Only executable expressions belong here. Contract predicates can contain
            proof-only equality vocabulary; accepting those as pure initializers would
            suppress the ordinary computation builder that owns runtime evaluation.
        */
        use super::{
            CheckedScalarExpressionRole, LoweredCallArguments, ScalarLocal, StatementPlanner,
            lower_direct_call_binding_arguments, lower_return_expression,
            lower_selected_operator_operands, retain_call_arguments,
        };
        use symbols::SymbolHandle;
        use typed_trees::statement::TableLocalData;
        use validation::atomic_once_carrier;
        use validation::expression_result_type_reference;
        use validation::slice_view_shell;

        pub(super) fn plan(planner: &mut StatementPlanner<'_, '_>, local: &TableLocalData) {
            if let Some(carrier) = atomic_once_carrier(planner.program, local.initial_value) {
                if let Some(operands) = super::super::atomic_operands::lower_carrier(
                    planner.program,
                    planner.operators,
                    planner.machine,
                    planner.state,
                    carrier,
                    planner.scalar_parameters,
                    planner.parameters,
                    planner.parameter_types,
                    planner.locals,
                    planner.exact_integer_casts,
                ) {
                    for (source, role, value) in operands {
                        planner.record_scalar_value_pair(
                            role,
                            SymbolHandle::invalid(),
                            source,
                            value,
                        );
                    }
                }
                return;
            }
            retain_call_arguments(
                LoweredCallArguments {
                    scalar_arguments: lower_selected_operator_operands(
                        planner.program,
                        planner.operators,
                        planner.machine,
                        planner.state,
                        planner.statement_ordinal,
                        local,
                        planner.scalar_parameters,
                        planner.parameters,
                        planner.parameter_types,
                        planner.locals,
                        planner.exact_integer_casts,
                    ),
                    proof_terms: Vec::new(),
                },
                planner.scalar_parameters,
                planner.locals,
                planner.expressions,
                planner.proof_terms,
                planner.source_bindings,
                planner.binding_symbols,
            );
            if (!local.is_mutable
                || planner
                    .program
                    .primitive_type_reference(local.type_reference)
                    .is_none())
                && let Some(expression) =
                    super::value_call_expression(planner.program, local.initial_value)
            {
                planner.retain_expression_call_arguments(expression);
            }
            let Some(primitive_type) = planner
                .program
                .primitive_type_reference(local.type_reference)
                .filter(|_| {
                    local.is_mutable
                        || expression_result_type_reference(
                            planner.program,
                            planner.machine,
                            planner.state,
                            local.initial_value,
                        )
                        .and_then(|result| slice_view_shell(planner.program, result, &[]))
                        .is_none()
                })
            else {
                if let Some(binding) =
                    super::super::primitive_reference_read::local(planner.program, local)
                {
                    planner.locals.push(binding);
                }
                return;
            };
            let binding_ordinal = u32::try_from(
                planner
                    .locals
                    .iter()
                    .filter(|local: &&ScalarLocal| local.is_value_binding())
                    .count(),
            )
            .ok();
            if let Some(binding_ordinal) = binding_ordinal {
                let role = if local.is_mutable {
                    CheckedScalarExpressionRole::StorageInitializer
                } else {
                    CheckedScalarExpressionRole::LocalInitializer { binding_ordinal }
                };
                if !local.is_mutable
                    && let Some(arguments) = lower_direct_call_binding_arguments(
                        planner.program,
                        planner.operators,
                        planner.state.symbol,
                        planner.statement_ordinal,
                        binding_ordinal,
                        local.initial_value,
                        planner.scalar_parameters,
                        planner.parameters,
                        planner.parameter_types,
                        planner.locals,
                        planner.exact_integer_casts,
                    )
                {
                    retain_call_arguments(
                        arguments,
                        planner.scalar_parameters,
                        planner.locals,
                        planner.expressions,
                        planner.proof_terms,
                        planner.source_bindings,
                        planner.binding_symbols,
                    );
                } else if let Some(initializer) =
                    crate::values::zeroed_scalar_local_initializer(planner.program, local).or_else(
                        || {
                            lower_return_expression(
                                planner.program,
                                planner.operators,
                                local.initial_value,
                                planner.scalar_parameters,
                                planner.parameters,
                                planner.parameter_types,
                                planner.locals,
                                primitive_type,
                                planner.exact_integer_casts,
                            )
                        },
                    )
                {
                    planner.record_scalar_value_pair(
                        role,
                        local.symbol,
                        local.initial_value,
                        initializer,
                    );
                }
            }
            planner.locals.push(ScalarLocal::from_local_data(
                planner.program,
                local,
                primitive_type,
            ));
        }
    }
    mod range_operands {
        /*
            Range operands have one occurrence inventory for pure and computed values.
            Planning records coordinates; the consumer owns selective evaluation.
        */
        use super::{
            StatementNode, StatementPlanner, lower_index_expression, retain_subslice_endpoints,
        };

        pub(crate) fn plan(planner: &mut StatementPlanner<'_, '_>, statement: &StatementNode) {
            for (expression, site) in super::super::subslice_endpoints::sites(
                planner.program,
                planner.machine,
                planner.state,
                planner.statement_index,
                statement,
            ) {
                let endpoints = super::super::subslice_endpoints::subslice_endpoints(
                    planner.program,
                    planner.operators,
                    expression,
                    site,
                    |endpoint| {
                        lower_index_expression(
                            planner.program,
                            planner.operators,
                            endpoint,
                            planner.scalar_parameters,
                            planner.parameters,
                            planner.parameter_types,
                            planner.locals,
                            planner.exact_integer_casts,
                        )
                    },
                );
                retain_subslice_endpoints(
                    endpoints,
                    planner.state.symbol,
                    planner.statement_ordinal,
                    planner.scalar_parameters,
                    planner.locals,
                    planner.expressions,
                    planner.source_bindings,
                    planner.binding_symbols,
                );
            }
        }
    }
    mod transition {
        /*
            A transition: its guard (`Guard`), a returned arm value (`Return` or
            `ContinuationReturn`), and each successor argument as a scalar
            expression, a subslice endpoint or an erased proof term.
        */
        use super::{
            CheckedScalarExpression, CheckedScalarExpressionRole, StatementPlanner,
            TransitionGuardNode, TransitionTargetNode, lower_boolean_guard,
            lower_return_expression,
        };
        use checked_trees::CheckedLocatedProofTerm;
        use checked_trees::CheckedProofTermRole;
        use symbols::SymbolHandle;
        use typed_trees::statement::TableTransition;
        use typed_trees::statement::TransitionExit;
        use validation::activation_target_state_index;
        use validation::completing_transition_transfer;
        use validation::find_machine_head;
        use validation::mathematical_applications::is_source_evidence_carrier;

        pub(crate) fn plan(planner: &mut StatementPlanner<'_, '_>, transition: &TableTransition) {
            if let TransitionGuardNode::When(authored_guard) = transition.guard
                && let Some(guard) = lower_boolean_guard(
                    planner.program,
                    planner.operators,
                    authored_guard,
                    planner.scalar_parameters,
                    planner.parameters,
                    planner.parameter_types,
                    planner.locals,
                    planner.exact_integer_casts,
                )
            {
                planner.record_scalar_value_pair(
                    CheckedScalarExpressionRole::Guard,
                    SymbolHandle::invalid(),
                    authored_guard,
                    CheckedScalarExpression::Boolean(Box::new(guard)),
                );
            }
            for (target, continuation) in
                [(transition.target, false), (transition.continuation, true)]
            {
                if !target.is_valid() {
                    continue;
                }
                if transition.exit == TransitionExit::Ordinary
                    && let TransitionTargetNode::Value(expression) =
                        planner.program.statement_table.transition_target(target)
                    && let Some(result_type) = planner.result_type
                    && let Some(return_expression) = lower_return_expression(
                        planner.program,
                        planner.operators,
                        *expression,
                        planner.scalar_parameters,
                        planner.parameters,
                        planner.parameter_types,
                        planner.locals,
                        result_type,
                        planner.exact_integer_casts,
                    )
                {
                    let role = if continuation {
                        CheckedScalarExpressionRole::ContinuationReturn
                    } else {
                        CheckedScalarExpressionRole::Return
                    };
                    planner.record_scalar_value_pair(
                        role,
                        SymbolHandle::invalid(),
                        *expression,
                        return_expression,
                    );
                }
                let Some(transfer) = completing_transition_transfer(planner.program, target) else {
                    continue;
                };
                let Some(target_state) = activation_target_state_index(
                    planner.program,
                    planner.machine,
                    transfer.target,
                )
                .and_then(|target_index| planner.states.get(target_index))
                .or_else(|| {
                    find_machine_head(planner.program, transfer.target).map(|(_, entry)| entry)
                }) else {
                    continue;
                };
                let target_parameters = planner
                    .program
                    .state_parameters(target_state)
                    .iter()
                    .enumerate();
                for (argument, (target_position, target_parameter)) in
                    transfer.arguments.iter().zip(target_parameters)
                {
                    let Ok(argument_ordinal) = u32::try_from(target_position) else {
                        continue;
                    };
                    // A transition argument row is keyed by its authored
                    // position, so it carries one erased scalar lane. Source
                    // checking refuses a nested machine whose erased record
                    // formal has several.
                    let lanes = if target_parameter.relevance.is_erased() {
                        planner
                            .program
                            .erased_scalar_lanes(target_parameter.type_reference)
                    } else {
                        Vec::new()
                    };
                    let lane = match lanes.as_slice() {
                        [lane] => Some(lane),
                        _ => None,
                    };
                    let Some(target_type) = (if target_parameter.relevance.is_erased() {
                        lane.map(|lane| lane.primitive_type)
                    } else {
                        planner
                            .program
                            .primitive_type_reference(target_parameter.type_reference)
                    }) else {
                        if target_parameter.relevance.is_erased()
                            && (planner.proof_only.contract_term_carrier(
                                planner.program,
                                target_parameter.type_reference,
                            ) || is_source_evidence_carrier(
                                planner.program,
                                target_parameter.type_reference,
                            ))
                        {
                            if let Some(term) =
                                crate::values::scalar::call_lowering::lower_proof_term(
                                    planner.program,
                                    planner.state.symbol,
                                    planner.operators,
                                    *argument,
                                    target_parameter.type_reference,
                                    planner.scalar_parameters,
                                    planner.parameters,
                                    planner.parameter_types,
                                    planner.locals,
                                    planner.exact_integer_casts,
                                    planner.proof_only,
                                )
                            {
                                planner.proof_terms.push(CheckedLocatedProofTerm {
                                    state: planner.state.symbol,
                                    statement_ordinal: planner.statement_ordinal,
                                    role: if continuation {
                                        CheckedProofTermRole::TransitionContinuationArgument {
                                            argument_ordinal,
                                        }
                                    } else {
                                        CheckedProofTermRole::TransitionArgument {
                                            argument_ordinal,
                                        }
                                    },
                                    expression: *argument,
                                    term,
                                });
                            }
                            continue;
                        }
                        if continuation {
                            continue;
                        }
                        continue;
                    };
                    let lowered = match lane {
                        Some(lane) => {
                            crate::values::scalar::call_lowering::lower_erased_lane_argument(
                                planner.state.symbol,
                                planner.statement_ordinal,
                                planner.program,
                                planner.operators,
                                *argument,
                                planner.scalar_parameters,
                                planner.parameters,
                                planner.parameter_types,
                                planner.locals,
                                target_parameter.type_reference,
                                lane,
                                planner.exact_integer_casts,
                            )
                        }
                        None => crate::values::scalar::scalar_lowering::lower_return_expression(
                            planner.program,
                            planner.operators,
                            *argument,
                            planner.scalar_parameters,
                            planner.parameters,
                            planner.parameter_types,
                            planner.locals,
                            target_type,
                            planner.exact_integer_casts,
                        ),
                    };
                    let Some(expression) = lowered else {
                        continue;
                    };
                    let role = if continuation {
                        CheckedScalarExpressionRole::TransitionContinuationArgument {
                            argument_ordinal,
                        }
                    } else {
                        CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
                    };
                    planner.record_scalar_value_pair(
                        role,
                        target_parameter.symbol,
                        *argument,
                        expression,
                    );
                }
            }
        }
    }

    #[derive(Debug, Clone)]
    pub(crate) struct ScalarLocal {
        pub(crate) is_mutable: bool,
        pub(crate) symbol: symbols::SymbolHandle,
        pub(crate) name: String,
        pub(crate) primitive_type: PrimitiveType,
        pub(crate) arithmetic_domain: ArithmeticDomain,
        /*
            A reference binding is observed through its established carrier. It owns
            no scalar value ordinal, regardless of the expression that produced it.
        */
        pub(crate) is_reference: bool,
    }

    impl ScalarLocal {
        /*
            The scalar reading of an ordinary `LocalData` declaration: a named,
            non-reference local carrying its declared primitive shape and domain.
        */
        pub(crate) fn from_local_data(
            program: &dyn ::validation::CheckingInput,
            local: &typed_trees::statement::TableLocalData,
            primitive_type: PrimitiveType,
        ) -> Self {
            Self {
                is_mutable: local.is_mutable,
                symbol: local.symbol,
                name: local.name.as_str().to_owned(),
                primitive_type,
                arithmetic_domain: program
                    .arithmetic_domain_for_type_reference(local.type_reference),
                is_reference: false,
            }
        }
    }

    impl ScalarLocal {
        /*
            Immutable value bindings own a `Local` position and a binding
            ordinal; reference carriers stay out of the
            value-binding namespace.
        */
        pub(crate) fn is_value_binding(&self) -> bool {
            !self.is_mutable && !self.is_reference
        }
    }

    /*
        The scalar-parameter plus value-binding-local symbol roster every
        located expression binding carries.
    */
    pub(crate) fn located_binding_symbols<'a>(
        scalar_parameters: &'a [typed_trees::signature::StateParameter],
        locals: &'a [ScalarLocal],
    ) -> impl Iterator<Item = symbols::SymbolHandle> + 'a {
        scalar_parameters
            .iter()
            .map(|parameter| parameter.symbol)
            .chain(
                locals
                    .iter()
                    .filter(|local| local.is_value_binding())
                    .map(|local| local.symbol),
            )
    }

    /*
        One statement's place in the plan walk: the program facts every
        statement kind reads, the state's scalar signature and the rosters each
        statement's plans extend.
    */
    struct StatementPlanner<'p, 's> {
        program: &'p dyn ::validation::CheckingInput,
        operators: &'p CheckedOperatorFacts,
        exact_integer_casts: &'p [validation::ExactIntegerCastFact],
        proof_only: &'s typed_trees::proof_only::ProofOnlyClassification,
        machine: &'p typed_trees::machine::Machine,
        state: &'p typed_trees::state::State,
        states: &'p [typed_trees::state::State],
        parameters: &'p [typed_trees::signature::StateParameter],
        scalar_parameters: &'s [typed_trees::signature::StateParameter],
        parameter_types: &'s [PrimitiveType],
        result_type: Option<PrimitiveType>,
        statement_index: usize,
        statement_ordinal: u32,
        locals: &'s mut Vec<ScalarLocal>,
        expressions: &'s mut Vec<CheckedLocatedScalarExpression>,
        proof_terms: &'s mut Vec<checked_trees::CheckedLocatedProofTerm>,
        source_bindings: &'s mut arena::Arena<CheckedScalarExpressionBindings>,
        binding_symbols: &'s mut arena::Arena<symbols::SymbolHandle>,
    }

    impl<'p> StatementPlanner<'p, '_> {
        /*
            Lower one value-call expression's scalar arguments and retain them. The
            call-producing statements each guard the same lowering with their own
            source test (`value_call_expression` for assignment and local data, the
            statement's own expression for an expression statement).
        */
        pub(super) fn retain_expression_call_arguments(&mut self, expression: ExpressionHandle) {
            let Some(call) = self.program.expression_table.call_node(expression) else {
                return;
            };
            let Some(arguments) = lower_call_arguments(
                self.program,
                self.operators,
                self.state,
                self.statement_ordinal,
                0,
                &validation::CallSite::Expression { expression, call },
                self.scalar_parameters,
                self.parameters,
                self.parameter_types,
                self.locals,
                self.exact_integer_casts,
            ) else {
                return;
            };
            retain_call_arguments(
                arguments,
                self.scalar_parameters,
                self.locals,
                self.expressions,
                self.proof_terms,
                self.source_bindings,
                self.binding_symbols,
            );
        }

        /*
            Records the source-binding and located-expression rows one bound scalar
            value contributes to its plan.
        */
        fn record_scalar_value_pair(
            &mut self,
            role: CheckedScalarExpressionRole,
            destination: symbols::SymbolHandle,
            bound_expression: ExpressionHandle,
            located_expression: CheckedScalarExpression,
        ) {
            self.source_bindings
                .append(CheckedScalarExpressionBindings {
                    destination,
                    state: self.state.symbol,
                    statement_ordinal: self.statement_ordinal,
                    role,
                    expression: bound_expression,
                    symbols: self
                        .binding_symbols
                        .insert_many(located_binding_symbols(self.scalar_parameters, self.locals)),
                });
            self.expressions.push(CheckedLocatedScalarExpression {
                state: self.state.symbol,
                statement_ordinal: self.statement_ordinal,
                role,
                expression: located_expression,
            });
        }
    }

    pub(crate) fn build_checked_scalar_expression_plans(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        exact_integer_casts: &[validation::ExactIntegerCastFact],
        proof_terms: &mut Vec<checked_trees::CheckedLocatedProofTerm>,
    ) -> CheckedScalarExpressionPlans {
        let mut expressions = Vec::new();
        let mut source_bindings = arena::Arena::default();
        let mut binding_symbols = arena::Arena::default();
        let proof_only = validation::proof_only_classification(program);
        for machine in program.machines() {
            let states = program.machine_states(machine);
            for state in states {
                let mut locals = Vec::new();
                let parameters = program.state_parameters(state);
                let (scalar_parameters, parameter_types) =
                    scalar_parameter_roster(program, parameters)
                        .expect("filtered scalar parameters retain primitive carriers");
                let result_type = program.primitive_type_reference(state.return_type);
                for (statement_index, statement) in
                    indexed_statements(program, state.statement_nodes)
                {
                    let Ok(statement_ordinal) = u32::try_from(statement_index) else {
                        continue;
                    };
                    let mut planner = StatementPlanner {
                        program,
                        operators,
                        exact_integer_casts,
                        proof_only: &proof_only,
                        machine,
                        state,
                        states,
                        parameters,
                        scalar_parameters: &scalar_parameters,
                        parameter_types: &parameter_types,
                        result_type,
                        statement_index,
                        statement_ordinal,
                        locals: &mut locals,
                        expressions: &mut expressions,
                        proof_terms,
                        source_bindings: &mut source_bindings,
                        binding_symbols: &mut binding_symbols,
                    };
                    array_elements::plan(&mut planner, statement);
                    range_operands::plan(&mut planner, statement);
                    match statement {
                        StatementNode::LocalData(local) => local_data::plan(&mut planner, local),
                        StatementNode::Expression(expression) => {
                            expression_statement::plan(&mut planner, *expression)
                        }
                        StatementNode::Assignment(assignment) => {
                            assignment::plan(&mut planner, assignment)
                        }
                        StatementNode::Call(call) => call_statement::plan(&mut planner, call),
                        StatementNode::Transition(transition) => {
                            transition::plan(&mut planner, transition)
                        }
                        _ => {}
                    }
                }
            }
        }
        let mut retained_bindings = arena::Arena::default();
        for (_, binding) in source_bindings.iter() {
            if semantic_casts::requires_custody(program, binding.state, binding.expression) {
                expressions.retain(|expression| {
                    expression.state != binding.state
                        || expression.statement_ordinal != binding.statement_ordinal
                        || expression.role != binding.role
                });
            } else {
                retained_bindings.append(binding.clone());
            }
        }
        CheckedScalarExpressionPlans {
            expressions,
            source_bindings: retained_bindings,
            binding_symbols,
            ..Default::default()
        }
    }

    /*
        Retain one range's lowered endpoints as source-bound pure plans. Endpoints
        read the site's scalar namespace but bind no destination: the range, not
        a local or formal, consumes them.
    */
    fn retain_subslice_endpoints(
        endpoints: Vec<(
            ExpressionHandle,
            CheckedScalarExpressionRole,
            CheckedScalarExpression,
        )>,
        state: symbols::SymbolHandle,
        statement_ordinal: u32,
        scalar_parameters: &[typed_trees::signature::StateParameter],
        locals: &[ScalarLocal],
        expressions: &mut Vec<CheckedLocatedScalarExpression>,
        source_bindings: &mut arena::Arena<CheckedScalarExpressionBindings>,
        binding_symbols: &mut arena::Arena<symbols::SymbolHandle>,
    ) {
        for (endpoint, role, expression) in endpoints {
            source_bindings.append(CheckedScalarExpressionBindings {
                destination: symbols::SymbolHandle::invalid(),
                state,
                statement_ordinal,
                role,
                expression: endpoint,
                symbols: binding_symbols
                    .insert_many(located_binding_symbols(scalar_parameters, locals)),
            });
            expressions.push(CheckedLocatedScalarExpression {
                state,
                statement_ordinal,
                role,
                expression,
            });
        }
    }

    pub(crate) fn lower_closed_integer_literal_guard(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        mut expression: ExpressionHandle,
    ) -> Option<CheckedBooleanExpression> {
        if let Some(binary) = program.expression_table.binary_node(expression)
            && binary.operator == BinaryOperator::Equal
            && operator_is_builtin(operators, expression)
        {
            match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(true), _) => expression = binary.right,
                (_, ExpressionNode::Boolean(true)) => expression = binary.left,
                _ => {}
            }
        }
        let binary = program.expression_table.binary_node(expression)?;
        if !operator_is_builtin(operators, expression) {
            return None;
        }
        let left = program.expression_table.integer_node(binary.left)?;
        let right = program.expression_table.integer_node(binary.right)?;
        let left = left.value_bignum()?;
        let right = right.value_bignum()?;
        let value = binary.operator.evaluate_comparison(&left, &right)?;
        Some(CheckedBooleanExpression::Constant(value))
    }

    pub(crate) fn assignment_target_primitive_type(
        program: &dyn ::validation::CheckingInput,
        mut type_reference: TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        let mut crossed_reference = false;
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::Reference { referee, .. } if !crossed_reference => {
                    crossed_reference = true;
                    type_reference = *referee;
                }
                TypeReferenceNode::Reference { .. } => return None,
                _ => return program.primitive_type_reference(type_reference),
            }
        }
    }
}
pub(crate) mod machine_parameter_booleans {
    /*
        Lowering the Boolean expressions that machine parameters admit.

        A predicate that reads a structural parameter lowers through `structural`
        in the entry's authored parameter namespace. Every other predicate lowers
        through the shared scalar `lower_boolean_expression` over the entry's
        dense scalar parameters.
    */
    mod structural {
        /*
            Entry-parameter predicates that read structural parameters: Boolean
            fields, case membership, structural equality, and numeric comparisons
            whose operands read structural fields. Structural
            places keep authored parameter positions; scalar parameters use the dense
            scalar namespace.
        */

        use super::structural_paths::{path_primitive_type, path_type_reference};
        use crate::flow::expressions;
        use crate::values::scalar::boolean_lowering::construct_integer_comparison;
        use crate::values::scalar::expression_facts::{
            operator_is_builtin, scalar_expression_type,
        };
        use crate::values::scalar::scalar_lowering::lower_scalar_operands;
        use crate::values::scalar::structural_fields::{
            structural_data, structural_parameter_case_path, structural_parameter_field_path,
        };
        use checked_trees::{
            CheckedBooleanExpression, CheckedOperatorFacts, CheckedStructuralParameterField,
        };
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
        };
        use typed_trees::signature::StateParameter;
        use typed_trees::types::PrimitiveType;

        /*
            Lower `expression` in the entry parameters' namespace, or `None` when no
            structural form describes it.
        */
        pub(crate) fn lower(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            operators: &CheckedOperatorFacts,
            parameters: &[StateParameter],
            expression: ExpressionHandle,
        ) -> Option<CheckedBooleanExpression> {
            if let Some(membership) =
                lower_structural_case_membership(program, machine, parameters, expression)
            {
                return Some(membership);
            }

            let mut path = Vec::new();
            if let Some(parameter_position) =
                structural_parameter_field_path(program, parameters, expression, &mut path)
                && !path.is_empty()
                && path_primitive_type(program, parameters, parameter_position, &path)
                    == Some(PrimitiveType::Bool)
            {
                return Some(CheckedBooleanExpression::StructuralParameterField {
                    observation: expression,
                    parameter_position,
                    path,
                });
            }

            match program.expression_table.expression(expression) {
                ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
                ExpressionNode::Name(name) => {
                    let name_symbol = name.symbol.is_valid().then_some(name.symbol).or_else(|| {
                        expressions(program)
                            .name_path_member_symbols(name.member_symbols)
                            .iter()
                            .copied()
                            .find(|symbol| symbol.is_valid())
                    });
                    let name_text = expressions(program).name_path_members(name.members).last();
                    let source_position = parameters.iter().position(|parameter| {
                        name_symbol.is_some_and(|symbol| parameter.symbol == symbol)
                            || name_text.is_some_and(|text| parameter.name == *text)
                    })?;
                    let parameter = parameters.get(source_position)?;
                    if parameter.relevance.is_erased() {
                        return None;
                    }
                    if parameter.is_mutable {
                        return (crate::values::mutable_scalar_parameter_type(program, parameter)
                            == Some(PrimitiveType::Bool)
                            && name.symbol == parameter.symbol
                            && name.head_symbol == parameter.symbol)
                            .then_some(CheckedBooleanExpression::StorageRead {
                                symbol: parameter.symbol,
                            });
                    }
                    (program.primitive_type_reference(parameter.type_reference)
                        == Some(PrimitiveType::Bool))
                    .then(|| CheckedBooleanExpression::Parameter {
                        position: crate::values::scalar::scalar_position_count(
                            program,
                            &parameters[..source_position],
                        ),
                    })
                }
                ExpressionNode::Unary(unary)
                    if unary.operator == UnaryOperator::LogicalNot
                        && operator_is_builtin(operators, expression) =>
                {
                    Some(CheckedBooleanExpression::Not(Box::new(lower(
                        program,
                        machine,
                        operators,
                        parameters,
                        unary.operand,
                    )?)))
                }
                ExpressionNode::Binary(binary)
                    if binary.operator.is_comparison()
                        && operator_is_builtin(operators, expression) =>
                {
                    let numeric_comparison = (|| {
                        let (scalar_parameters, parameter_types) =
                            crate::values::scalar::scalar_parameter_roster(program, parameters)?;
                        let ((left, _), (right, _)) = lower_scalar_operands(
                            program,
                            operators,
                            binary,
                            &scalar_parameters,
                            parameters,
                            &parameter_types,
                            &[],
                            &[],
                        )?;
                        if scalar_expression_type(&left) == Some(PrimitiveType::Addr)
                            || scalar_expression_type(&right) == Some(PrimitiveType::Addr)
                        {
                            return None;
                        }
                        if matches!(
                            scalar_expression_type(&left),
                            Some(PrimitiveType::F32 | PrimitiveType::F64)
                        ) {
                            let references = [binary.left, binary.right].map(|operand| {
                                validation::parameter_expression_result_type_reference(
                                    program,
                                    machine.symbol,
                                    parameters,
                                    operand,
                                )
                            });
                            return crate::values::scalar::result_contract::lower_float_contract_comparison(
                                program,
                                machine.symbol,
                                expression,
                                [Some((left, references[0]?)), Some((right, references[1]?))],
                            );
                        }
                        construct_integer_comparison(binary.operator, left, right)
                    })();
                    if !matches!(
                        binary.operator,
                        BinaryOperator::Equal | BinaryOperator::NotEqual
                    ) || numeric_comparison.is_some()
                    {
                        return numeric_comparison;
                    }
                    let equality = CheckedBooleanExpression::Equal {
                        left: Box::new(lower(
                            program,
                            machine,
                            operators,
                            parameters,
                            binary.left,
                        )?),
                        right: Box::new(lower(
                            program,
                            machine,
                            operators,
                            parameters,
                            binary.right,
                        )?),
                    };
                    Some(if binary.operator == BinaryOperator::NotEqual {
                        CheckedBooleanExpression::Not(Box::new(equality))
                    } else {
                        equality
                    })
                }
                ExpressionNode::Binary(binary)
                    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                        && operator_is_builtin(operators, expression) =>
                {
                    let left =
                        Box::new(lower(program, machine, operators, parameters, binary.left)?);
                    let right = Box::new(lower(
                        program,
                        machine,
                        operators,
                        parameters,
                        binary.right,
                    )?);
                    Some(if binary.operator == BinaryOperator::And {
                        CheckedBooleanExpression::And { left, right }
                    } else {
                        CheckedBooleanExpression::Or { left, right }
                    })
                }
                _ => None,
            }
        }

        fn lower_structural_case_membership(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            parameters: &[StateParameter],
            expression: ExpressionHandle,
        ) -> Option<CheckedBooleanExpression> {
            let binary = program.expression_table.binary_node(expression)?;
            if !matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::CaseMembership
            ) {
                return None;
            }
            let state = program.machine_states(machine).first()?;
            if !validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) {
                return None;
            }
            let membership = validation::has_exact_case_membership_meaning(
                program,
                machine,
                Some(state),
                expression,
                binary,
            );
            let classifier = |candidate: ExpressionHandle| {
                let (case_symbol, literal_owner) =
                    match program.expression_table.expression(candidate) {
                        ExpressionNode::Name(name) => (name.symbol, None),
                        ExpressionNode::StructLiteral(literal)
                            if expressions(program)
                                .struct_fields(literal.fields)
                                .is_empty() =>
                        {
                            (literal.case_symbol?, Some(literal.type_symbol))
                        }
                        _ => return None,
                    };
                program.data_definitions().iter().find_map(|data| {
                    if literal_owner.is_some_and(|owner| owner != data.symbol)
                        || (!membership
                            && program.data_members(data).iter().any(|member| {
                                matches!(member, typed_trees::data::DataMember::Field(_))
                            }))
                    {
                        return None;
                    }
                    program.data_members(data).iter().find_map(|member| {
                        let variant = member.as_variant()?;
                        (variant.symbol == case_symbol
                            && (membership || program.data_payload_fields(variant).is_empty()))
                        .then(|| {
                            (
                                data.symbol,
                                checked_trees::CheckedMemberIdentity::of_member(
                                    variant.identity,
                                    variant.name.clone(),
                                ),
                            )
                        })
                    })
                })
            };
            let (subject_expression, data_symbol, case) =
                if let Some((data, case)) = classifier(binary.right) {
                    (binary.left, data, case)
                } else if let Some((data, case)) = classifier(binary.left) {
                    (binary.right, data, case)
                } else {
                    return None;
                };
            let mut path = Vec::new();
            let parameter_position =
                structural_parameter_case_path(program, parameters, subject_expression, &mut path)?;
            let subject_type = path_type_reference(program, parameters, parameter_position, &path)?;
            (structural_data(program, subject_type)?.symbol == data_symbol).then_some(
                CheckedBooleanExpression::StructuralCaseMembership {
                    subject: CheckedStructuralParameterField {
                        parameter_position,
                        path,
                    }
                    .into(),
                    case,
                },
            )
        }
    }
    mod structural_paths {
        /*
            Types and primitives at a structural parameter path: the field or payload
            each `Field`, `Case`, and `FixedIndex` segment selects.
        */
        use crate::values::scalar::structural_fields;
        use crate::values::scalar::structural_fields::structural_data;
        use checked_trees::{CheckedMemberIdentity, CheckedStructuralPredicatePathSegment};
        use typed_trees::signature::StateParameter;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

        fn field_type(
            program: &dyn ::validation::CheckingInput,
            receiver: TypeReferenceHandle,
            identity: &CheckedMemberIdentity,
        ) -> Option<TypeReferenceHandle> {
            let declaration = structural_data(program, receiver)?;
            program.data_members(declaration).iter().find_map(|member| {
                let field = member.as_field()?;
                let matches_identity = *identity
                    == CheckedMemberIdentity::of_member(field.identity, field.name.clone());
                matches_identity.then_some(field.type_reference)
            })
        }

        pub(crate) fn path_type_reference(
            program: &dyn ::validation::CheckingInput,
            parameters: &[StateParameter],
            parameter_position: u32,
            path: &[CheckedStructuralPredicatePathSegment],
        ) -> Option<TypeReferenceHandle> {
            let parameter = usize::try_from(parameter_position)
                .ok()
                .and_then(|position| parameters.get(position))?;
            let mut receiver =
                structural_fields::predicate_parameter_type(program, parameter.type_reference)?;
            let mut selected_case = None;
            for segment in path {
                match segment {
                    CheckedStructuralPredicatePathSegment::Referent => {
                        if selected_case.is_some() {
                            return None;
                        }
                        let mut selected = receiver;
                        while let TypeReferenceNode::Constrained { base_type, .. } =
                            program.type_reference_table.type_reference(selected)
                        {
                            selected = *base_type;
                        }
                        let TypeReferenceNode::Reference {
                            referee, access, ..
                        } = program.type_reference_table.type_reference(selected)
                        else {
                            return None;
                        };
                        if *access == language_core::ReferenceAccess::WriteOnly {
                            return None;
                        }
                        receiver = *referee;
                    }
                    CheckedStructuralPredicatePathSegment::FixedIndex(element_index) => {
                        if selected_case.is_some() {
                            return None;
                        }
                        receiver = structural_fields::fixed_index_element_type(
                            program,
                            receiver,
                            *element_index,
                        )?;
                    }
                    CheckedStructuralPredicatePathSegment::RuntimeIndex(_) => {
                        if selected_case.is_some() {
                            return None;
                        }
                        receiver =
                            structural_fields::runtime_index_element_type(program, receiver)?;
                    }
                    CheckedStructuralPredicatePathSegment::Case(case) => {
                        if selected_case.is_some() {
                            return None;
                        }
                        let data = structural_data(program, receiver)?;
                        let variant = program.data_members(data).iter().find_map(|member| {
                            let variant = member.as_variant()?;
                            let identity = CheckedMemberIdentity::of_member(
                                variant.identity,
                                variant.name.clone(),
                            );
                            (identity == *case).then_some(variant)
                        })?;
                        selected_case = Some(variant);
                    }
                    CheckedStructuralPredicatePathSegment::Field(field) => {
                        receiver = if let Some(variant) = selected_case.take() {
                            program
                                .data_payload_fields(variant)
                                .iter()
                                .find_map(|candidate| {
                                    let identity = CheckedMemberIdentity::of_member(
                                        candidate.identity,
                                        candidate.name.clone(),
                                    );
                                    (identity == *field).then_some(candidate.type_reference)
                                })?
                        } else {
                            field_type(program, receiver, field)?
                        };
                    }
                }
            }
            selected_case.is_none().then_some(receiver)
        }

        pub(crate) fn path_primitive_type(
            program: &dyn ::validation::CheckingInput,
            parameters: &[StateParameter],
            parameter_position: u32,
            path: &[CheckedStructuralPredicatePathSegment],
        ) -> Option<PrimitiveType> {
            program.primitive_type_reference(path_type_reference(
                program,
                parameters,
                parameter_position,
                path,
            )?)
        }
    }

    use crate::values::scalar::boolean_lowering::lower_boolean_expression;
    use crate::values::scalar::scalar_parameter_roster;
    use checked_trees::{CheckedBooleanExpression, CheckedOperatorFacts};
    use typed_trees::expression::ExpressionHandle;

    /*
        Lower a contract predicate in the selected machine's entry-parameter
        namespace. Crash contracts use this to retain the same checked scalar
        meaning as executable guards without carrying typed-tree handles into the
        terminal producer.
    */
    pub(crate) fn lower_machine_parameter_boolean_expression(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        machine: &typed_trees::machine::Machine,
        expression: ExpressionHandle,
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> Option<CheckedBooleanExpression> {
        let entry = program.machine_states(machine).first()?;
        let parameters = program.state_parameters(entry);

        if let Some(structural) =
            structural::lower(program, machine, operators, parameters, expression)
        {
            return Some(structural);
        }
        parameters
            .iter()
            .map(|parameter| program.primitive_type_reference(parameter.type_reference))
            .collect::<Option<Vec<_>>>()?;
        let (scalar_parameters, parameter_types) = scalar_parameter_roster(program, parameters)?;
        lower_boolean_expression(
            program,
            operators,
            expression,
            &scalar_parameters,
            parameters,
            &parameter_types,
            &[],
            exact_integer_casts,
        )
    }

    /*
        Lower one predicate in its authored parameter namespace. Call binding may
        instantiate the parameter types, but never replaces authored symbols or
        manufactures an executable state for a bodyless requirement.
    */
    pub(crate) fn lower_parameter_boolean_expression(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        owner: symbols::SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        expression: ExpressionHandle,
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> Option<CheckedBooleanExpression> {
        if !validation::has_builtin_parameter_bound_expression_meaning(
            program, owner, parameters, expression,
        ) {
            return None;
        }
        let (scalar_parameters, parameter_types) = scalar_parameter_roster(program, parameters)?;
        lower_boolean_expression(
            program,
            operators,
            expression,
            &scalar_parameters,
            parameters,
            &parameter_types,
            &[],
            exact_integer_casts,
        )
    }

    /*
        Lower a scalar in an authored parameter telescope without manufacturing an
        executable state. Proof aliases and abstract callable contracts share this
        checked arithmetic and operator-selection path.
    */
    pub(crate) fn lower_parameter_scalar_expression(
        program: &dyn validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        owner: symbols::SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        expression: ExpressionHandle,
        primitive: typed_trees::types::PrimitiveType,
    ) -> Option<checked_trees::CheckedScalarExpression> {
        if !validation::has_builtin_parameter_bound_expression_meaning(
            program, owner, parameters, expression,
        ) {
            return None;
        }
        let (scalar_parameters, parameter_types) = scalar_parameter_roster(program, parameters)?;
        super::scalar_lowering::lower_return_expression(
            program,
            operators,
            expression,
            &scalar_parameters,
            parameters,
            &parameter_types,
            &[],
            primitive,
            &[],
        )
    }
}
pub(crate) mod primitive_reference_read {
    /*
        Whole readable primitive referents are storage reads, not scalar formals.
    */
    use crate::flow::expressions;
    use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression as ScalarExpression};
    use typed_trees::{
        expression::ExpressionHandle,
        signature::StateParameter,
        types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode},
    };

    pub(crate) fn lower(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
        expected: PrimitiveType,
    ) -> Option<ScalarExpression> {
        let (value, _) = declared(program, parameters, expression)?;
        (super::scalar_expression_type(&value) == Some(expected)).then_some(value)
    }

    /*
        Primitive reference reads are ordinary leaves, including below operators.
        Their declared referee supplies both carrier and arithmetic semantics; an
        enclosing expression's expected result must not invent either property.
    */
    pub(crate) fn declared(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<(ScalarExpression, numerics::arithmetic::ArithmeticDomain)> {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        let name = program.expression_table.name_path_node(expression)?;
        if !program.expression_table.name_path_is_single_member(name)
            || expressions(program)
                .name_path_member_symbols(name.member_symbols)
                .first()
                .is_some_and(|member| *member != name.symbol)
        {
            return None;
        }
        parameter(program, parameters, name.symbol)
    }

    /*
        Read a formal through the same storage or erased observation namespace as
        a named source operand. Entry adapters have a symbol, not a Name node.
    */
    pub(crate) fn parameter(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        symbol: symbols::SymbolHandle,
    ) -> Option<(ScalarExpression, numerics::arithmetic::ArithmeticDomain)> {
        if !symbol.is_valid() {
            return None;
        }
        let mut matching = parameters
            .iter()
            .filter(|parameter| parameter.symbol == symbol);
        let parameter = matching.next()?;
        if (matching.next().is_some()) || parameter.is_const {
            return None;
        }
        let (primitive_type, arithmetic_domain) = readable_type(program, parameter.type_reference)?;
        if parameter.relevance.is_erased() {
            return erased_observation(program, parameters, parameter.symbol, primitive_type);
        }
        let value = if primitive_type == PrimitiveType::Bool {
            ScalarExpression::Boolean(Box::new(CheckedBooleanExpression::StorageRead {
                symbol: parameter.symbol,
            }))
        } else {
            ScalarExpression::StorageRead {
                symbol: parameter.symbol,
                primitive_type,
            }
        };
        Some((value, arithmetic_domain))
    }

    /*
        Exact erased formal observation, shared by body-shaped and contract-shaped
        readers. Relevance selects a proof namespace, not an executable read or a
        structural slot. Its presence supplies no proof: final source admission must
        still establish type, scope, access, assumptions and every precondition.
    */
    pub(crate) fn erased_observation(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        symbol: symbols::SymbolHandle,
        primitive_type: PrimitiveType,
    ) -> Option<(ScalarExpression, numerics::arithmetic::ArithmeticDomain)> {
        let mut matching = parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| symbol.is_valid() && parameter.symbol == symbol);
        let (authored_position, parameter) = matching.next()?;
        if matching.next().is_some()
            || !parameter.relevance.is_erased()
            || program.erased_scalar_observation_type(parameter.type_reference)
                != Some(primitive_type)
        {
            return None;
        }
        let position = program.erased_scalar_lane_base(parameters, authored_position);
        Some((
            ScalarExpression::ErasedParameter {
                position,
                primitive_type,
            },
            program.arithmetic_domain_for_type_reference(parameter.type_reference),
        ))
    }

    /*
        A field read below an erased record formal names that leaf's erased
        scalar lane. The formal owns no structural slot, so the read never
        becomes a structural observation; a path that is not exactly one lane
        of the formal has no observation at all.
    */
    pub(crate) fn erased_leaf_observation(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        authored_position: usize,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        primitive_type: PrimitiveType,
        leaf_type: typed_trees::types::TypeReferenceHandle,
    ) -> Option<(ScalarExpression, numerics::arithmetic::ArithmeticDomain)> {
        let parameter = parameters.get(authored_position)?;
        if !parameter.relevance.is_erased() || parameter.is_const || parameter.is_mutable {
            return None;
        }
        let names = path
            .iter()
            .filter_map(|segment| match segment {
                checked_trees::CheckedStructuralPredicatePathSegment::Referent => None,
                checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                    Some(Some(identity.name.as_str()))
                }
                _ => Some(None),
            })
            .collect::<Option<Vec<_>>>()?;
        let lanes = program.erased_scalar_lanes(parameter.type_reference);
        let lane = lanes.iter().position(|lane| {
            lane.primitive_type == primitive_type
                && lane.fields.len() == names.len()
                && lane
                    .fields
                    .iter()
                    .zip(&names)
                    .all(|(field, name)| program.symbols.name(*field) == *name)
        })?;
        Some((
            ScalarExpression::ErasedParameter {
                position: program.erased_scalar_lane_base(parameters, authored_position) + lane,
                primitive_type,
            },
            program.arithmetic_domain_for_type_reference(leaf_type),
        ))
    }

    /*
        Observation depends on the declared referee and current carrier, never on
        whether its producer was a borrow, recast, call, or transferred reference.
        Access and live custody are independently checked when that read is lowered.
    */
    pub(crate) fn local(
        program: &dyn ::validation::CheckingInput,
        local: &typed_trees::statement::TableLocalData,
    ) -> Option<super::expression_plans::ScalarLocal> {
        let (primitive_type, arithmetic_domain) = readable_type(program, local.type_reference)?;
        Some(super::expression_plans::ScalarLocal {
            is_mutable: local.is_mutable,
            symbol: local.symbol,
            name: local.name.as_str().to_owned(),
            primitive_type,
            arithmetic_domain,
            is_reference: true,
        })
    }

    fn readable_type(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<(PrimitiveType, numerics::arithmetic::ArithmeticDomain)> {
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        if !matches!(
            access,
            language_semantics::ReferenceAccess::Shared
                | language_semantics::ReferenceAccess::Mutable
        ) {
            return None;
        }
        let primitive_type = program.primitive_type_reference(*referee)?;
        (primitive_type != PrimitiveType::Addr).then(|| {
            (
                primitive_type,
                program.arithmetic_domain_for_type_reference(*referee),
            )
        })
    }
}
pub(crate) mod result_contract {
    /*
        Scalar contract predicates share logical structure and exact entry/result
        identities. They are not body expressions: mutable entry snapshots are
        available to requires, while normal guarantees cannot reread them as old
        values. Integer leaves keep their contextual landing and operator owner.
    */
    mod logical_matches {
        /*
            Logical capture scoping under the callable's existing formal/result policy.
        */
        use super::super::structural_fields::readable_entry_parameter_path;
        use super::super::structural_fields::structural_parameter_place;
        use super::super::structural_fields::structural_projection_path;
        use super::*;
        use checked_trees::CheckedStructuralPredicatePathSegment;

        #[derive(Clone)]
        pub(super) struct ContractSubject {
            position: u32,
            path: Vec<CheckedStructuralPredicatePathSegment>,
            reference: TypeReferenceHandle,
            result: bool,
        }

        impl ContractPredicates<'_, '_> {
            /*
                Exact phase-scoped storage actual for a declared predicate telescope. Shared
                presentation unwraps only this authored borrow; no initializer is read.
            */
            pub(super) fn predicate_actual_storage(
                &self,
                expression: ExpressionHandle,
            ) -> Option<(
                u32,
                Vec<CheckedStructuralPredicatePathSegment>,
                TypeReferenceHandle,
            )> {
                let expression = match self.program.expression_table.expression(expression) {
                    ExpressionNode::Borrow(borrow)
                        if borrow.access == language_core::ReferenceAccess::Shared =>
                    {
                        borrow.target
                    }
                    _ => expression,
                };
                let subject = self.logical_subject(expression, 0)?;
                Some((subject.position, subject.path, subject.reference))
            }

            fn logical_readable_path(
                &self,
                position: u32,
                path: &[CheckedStructuralPredicatePathSegment],
            ) -> Option<TypeReferenceHandle> {
                let mut remaining = self.remaining.get();
                let result = readable_entry_parameter_path(
                    self.program,
                    self.parameters,
                    position,
                    path,
                    &mut remaining,
                );
                self.remaining.set(remaining);
                result
            }

            fn logical_subject(
                &self,
                expression: ExpressionHandle,
                depth: usize,
            ) -> Option<ContractSubject> {
                if depth >= 64 {
                    return None;
                }
                let program = self.program;
                if let Some(observed) = self.logical_saved_subject(expression) {
                    return Some(observed);
                }
                match program.expression_table.expression(expression) {
                    ExpressionNode::Name(_) => {}
                    ExpressionNode::Member(member) => {
                        self.logical_subject(member.receiver, depth + 1)?;
                    }
                    ExpressionNode::SavedSubject(_) => {
                        return self.logical_saved_subject(expression);
                    }
                    _ => return None,
                }
                if self.allow_result
                    && let Some(result) = validation::reserved_result_place(program, expression)
                {
                    if result.machine_symbol != self.owner
                        || validation::reserved_result_owner(program, result.root)
                            != Some((self.owner, self.result_type))
                    {
                        return None;
                    }
                    let mut reference = self.result_type;
                    for segment in &result.segments {
                        if validation::reference_shell(program, reference).is_some() {
                            return None;
                        }
                        reference = validation::project_type_reference_from_segments(
                            program,
                            reference,
                            std::slice::from_ref(segment),
                        )?;
                    }
                    if validation::reference_shell(program, reference).is_some() {
                        return None;
                    }
                    return Some(ContractSubject {
                        position: u32::try_from(self.parameters.len()).ok()?,
                        path: structural_projection_path(program, &result.segments)?,
                        reference: validation::field_domains::readable_type_reference(
                            program, reference,
                        )?,
                        result: true,
                    });
                }
                let (position, path, _) =
                    structural_parameter_place(program, self.parameters, expression)?;
                let formal = self.parameters.get(position as usize)?;
                if formal.is_const
                    || !super::contract_storage_root(program, formal, self.allow_result)
                {
                    return None;
                }
                let reference = self.logical_readable_path(position, &path)?;
                Some(ContractSubject {
                    position,
                    path,
                    reference,
                    result: false,
                })
            }

            fn logical_saved_subject(
                &self,
                expression: ExpressionHandle,
            ) -> Option<ContractSubject> {
                let (mut observed, segments) = validation::logical_saved_subject_projection(
                    self.program,
                    expression,
                    &self.logical_captures,
                )?;
                observed
                    .path
                    .extend(structural_projection_path(self.program, &segments)?);
                if observed.result {
                    let mut reference = observed.reference;
                    for segment in &segments {
                        if validation::reference_shell(self.program, reference).is_some() {
                            return None;
                        }
                        reference = validation::project_type_reference_from_segments(
                            self.program,
                            reference,
                            std::slice::from_ref(segment),
                        )?;
                    }
                    if validation::reference_shell(self.program, reference).is_some() {
                        return None;
                    }
                    observed.reference = validation::field_domains::readable_type_reference(
                        self.program,
                        reference,
                    )?;
                } else {
                    observed.reference =
                        self.logical_readable_path(observed.position, &observed.path)?;
                }
                Some(observed)
            }

            pub(super) fn logical_saved_observation(
                &self,
                expression: ExpressionHandle,
            ) -> Option<(CheckedScalarExpression, TypeReferenceHandle)> {
                let observed = self.logical_saved_subject(expression)?;
                let primitive_type = self.program.primitive_type_reference(observed.reference)?;
                let term = if primitive_type == PrimitiveType::Bool {
                    CheckedScalarExpression::Boolean(Box::new(
                        CheckedBooleanExpression::StructuralParameterField {
                            observation: Default::default(),
                            parameter_position: observed.position,
                            path: observed.path,
                        },
                    ))
                } else {
                    CheckedScalarExpression::StructuralParameterField {
                        observation: Default::default(),
                        parameter_position: observed.position,
                        path: observed.path,
                        primitive_type,
                    }
                };
                Some((term, observed.reference))
            }

            fn logical_binding(
                &self,
                expression: ExpressionHandle,
            ) -> Option<validation::LogicalMatchBinding<ContractSubject>> {
                let ExpressionNode::Match(dispatch) =
                    self.program.expression_table.expression(expression)
                else {
                    return None;
                };
                let observed = self.logical_subject(dispatch.subject, 0)?;
                let capture = validation::logical_record_match_capture(
                    self.program,
                    expression,
                    observed.reference,
                )?;
                Some(validation::LogicalMatchBinding { capture, observed })
            }

            pub(super) fn logical_match_boolean(
                &mut self,
                expression: ExpressionHandle,
                depth: usize,
            ) -> Option<CheckedBooleanExpression> {
                let binding = self.logical_binding(expression)?;
                let arm = binding.capture.arm_value();
                let previous = self.logical_captures.len();
                self.logical_captures.push(binding);
                let result = self.boolean(arm, depth + 1);
                self.logical_captures.truncate(previous);
                result
            }

            pub(super) fn logical_match_numeric(
                &self,
                expression: ExpressionHandle,
                depth: usize,
            ) -> Option<(CheckedScalarExpression, TypeReferenceHandle)> {
                let binding = self.logical_binding(expression)?;
                let arm = binding.capture.arm_value();
                let mut bindings = self.logical_captures.clone();
                bindings.push(binding);
                let scoped = ContractPredicates {
                    program: self.program,
                    operators: self.operators,
                    owner: self.owner,
                    result_type: self.result_type,
                    parameters: self.parameters,
                    allow_result: self.allow_result,
                    remaining: self.remaining,
                    logical_captures: bindings,
                };
                scoped.numeric_term(arm, depth + 1)
            }
        }
    }
    mod predicate_conditions {
        /*
            Complete predicate conditions use the ordinary typed scalar read owner.
            The proof query owns equivalence and guard-before-read formation; this
            adapter owns exact actual storage, declared types and selector substitution.
        */
        use super::super::scalar_lowering::scalar_storage_read_projection;
        use super::super::scalar_lowering::storage_projection;
        use super::*;
        use checked_trees::{CheckedScalarReadProjection, CheckedStorageRoot};

        impl ContractPredicates<'_, '_> {
            pub(super) fn exact_predicate_condition(
                &mut self,
                expression: ExpressionHandle,
                depth: usize,
            ) -> Option<CheckedBooleanExpression> {
                if depth >= 64 {
                    return None;
                }
                let conditions =
                    validation::exact_predicate_conjunction(self.program, self.owner, expression)?;
                self.remaining
                    .set(self.remaining.get().checked_sub(conditions.len())?);
                let mut predicates = Vec::new();
                for condition in conditions {
                    let predicate = match condition {
                        validation::PredicateCondition::Boolean { subject, holds } => {
                            let value = if subject.projection.is_empty() {
                                self.boolean(subject.expression, depth + 1)?
                            } else {
                                let (CheckedScalarExpression::Boolean(value), _) = self
                                    .projected_scalar_subject(
                                        subject.expression,
                                        &subject.projection,
                                        depth + 1,
                                    )?
                                else {
                                    return None;
                                };
                                *value
                            };
                            if holds {
                                value
                            } else {
                                CheckedBooleanExpression::Not(Box::new(value))
                            }
                        }
                        validation::PredicateCondition::Ordering(condition) => {
                            let left = self.condition_operand(&condition.left, None, depth + 1);
                            let right = self.condition_operand(
                                &condition.right,
                                left.as_ref().and_then(scalar_expression_type),
                                depth + 1,
                            )?;
                            let left = left.or_else(|| {
                                self.condition_operand(
                                    &condition.left,
                                    scalar_expression_type(&right),
                                    depth + 1,
                                )
                            })?;
                            construct_integer_comparison(condition.operator, left, right)?
                        }
                    };
                    predicates.push(predicate);
                }
                let mut predicates = predicates.into_iter();
                let first = predicates.next()?;
                Some(
                    predicates.fold(first, |left, right| CheckedBooleanExpression::And {
                        left: Box::new(left),
                        right: Box::new(right),
                    }),
                )
            }

            fn condition_operand(
                &self,
                operand: &validation::PredicateIntegerOperand,
                context: Option<PrimitiveType>,
                depth: usize,
            ) -> Option<CheckedScalarExpression> {
                if depth >= 64 || self.remaining.get() == 0 {
                    return None;
                }
                self.remaining.set(self.remaining.get() - 1);
                match operand {
                    validation::PredicateIntegerOperand::Argument(subject) => self
                        .numeric_subject(subject.clone(), depth + 1)
                        .map(|value| value.0),
                    validation::PredicateIntegerOperand::Integer(value) => {
                        Some(CheckedScalarExpression::IntegerLiteral {
                            literal: validation::land_integer_value(
                                &numerics::bignum::BigInt::from_i128(*value),
                                context?,
                            )?,
                        })
                    }
                    validation::PredicateIntegerOperand::Read(read) => {
                        self.predicate_read(read, depth + 1)
                    }
                }
            }

            fn predicate_read(
                &self,
                read: &validation::PredicateIntegerReadSubject,
                depth: usize,
            ) -> Option<CheckedScalarExpression> {
                if depth >= 64 {
                    return None;
                }
                let machine = self.program.machine(read.predicate)?;
                let entry = self.program.machine_states(machine).first()?;
                let parameters = self.program.state_parameters(entry);
                if parameters.len() != read.arguments.len() {
                    return None;
                }
                let actuals = parameters
                    .iter()
                    .zip(&read.arguments)
                    .map(|(parameter, actual)| {
                        let actual = self.predicate_actual_storage(*actual)?;
                        let declared = validation::field_domains::readable_type_reference(
                            self.program,
                            parameter.type_reference,
                        )?;
                        let supplied = validation::field_domains::readable_type_reference(
                            self.program,
                            actual.2,
                        )?;
                        validation::declared_carriers_match(self.program, declared, supplied)
                            .then_some(actual)
                    })
                    .collect::<Option<Vec<_>>>()?;
                let mut projection = storage_projection(
                    self.program,
                    self.operators,
                    read.expression,
                    parameters,
                    |_, selector, _| {
                        let operand = validation::predicate_integer_operand(
                            self.program,
                            read.predicate,
                            &read.arguments,
                            selector,
                        )?;
                        self.condition_operand(&operand, Some(PrimitiveType::U64), depth + 1)
                    },
                )?;
                let position = usize::try_from(projection.root.parameter()?).ok()?;
                let (actual_position, prefix, _) = actuals.get(position)?;
                projection.root = CheckedStorageRoot::Parameter {
                    index: *actual_position,
                };
                let mut path = prefix
                    .iter()
                    .cloned()
                    .map(CheckedScalarReadProjection::Static)
                    .collect::<Vec<_>>();
                path.extend(projection.path);
                projection.path = path;
                scalar_storage_read_projection(
                    self.program,
                    read.expression,
                    self.parameters,
                    projection,
                )
                .map(|value| value.0)
            }
        }
    }

    use super::boolean_lowering::comparable_byte_carrier;
    use super::case_membership::lower_result;
    use super::scalar_lowering::scalar_storage_read_with_selectors;
    use super::structural_fields;
    use crate::flow::expressions;
    use crate::values::operator_is_builtin;
    use crate::values::scalar::boolean_lowering::construct_integer_comparison;
    use crate::values::scalar::expression_facts::is_integer;
    use crate::values::scalar::scalar_lowering::{
        construct_integer_binary, lower_return_expression,
    };
    use crate::values::scalar::{contract_entry, scalar_position_count};
    use crate::values::scalar_expression_type;
    use checked_trees::CheckedOperatorFacts;
    use checked_trees::CheckedOperatorResolutionStatus;
    use checked_trees::CheckedScalarExpression;
    use checked_trees::{
        CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedIntegerComparisonKind,
        CheckedIntegerMathExpression, CheckedIntegerMathTerm,
    };
    use numerics::arithmetic::ArithmeticDomain;
    use typed_trees::expression::BinaryOperator;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::expression::UnaryOperator;
    use typed_trees::signature::StateParameter;
    use typed_trees::types::PrimitiveType;
    use typed_trees::types::TypeReferenceHandle;
    use typed_trees::types::TypeReferenceNode;

    /*
        The caller supplies a predicate from this machine's exact contract clause.
        Entry scalar parameters precede the reserved ensures-only result position.
        The caller owns the shared work budget, including unsuccessful reads.
    */
    pub(crate) fn lower_scalar_contract_predicate(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        machine: &typed_trees::machine::Machine,
        expression: ExpressionHandle,
        allow_result: bool,
        remaining: &mut usize,
    ) -> Option<CheckedBooleanExpression> {
        let parameters = contract_entry::authored_entry_parameters(program, machine)?;
        let result_type = program.machine_states(machine).first()?.return_type;
        lower_callable_scalar_contract_predicate(
            program,
            operators,
            machine.symbol,
            parameters,
            result_type,
            expression,
            allow_result,
            remaining,
        )
    }

    /*
        One authored `requires` predicate on a non-entry state, lowered into the
        closed scalar contract namespace. The state's authored parameter roster
        indexes the subjects; no result position exists on an arrival contract.
        The machine still owns the builtin-meaning checks inside the predicate.
    */
    pub(crate) fn lower_state_scalar_contract_predicate(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
        remaining: &mut usize,
    ) -> Option<CheckedBooleanExpression> {
        let budget = ContractBudget::new(remaining);
        ContractPredicates {
            program,
            operators,
            owner: machine.symbol,
            result_type: TypeReferenceHandle::invalid(),
            parameters: program.state_parameters(state),
            allow_result: false,
            logical_captures: Vec::new(),
            remaining: &budget.remaining,
        }
        .boolean(expression, 0)
    }

    /*
        Executable and bodyless declarations use the same formal observation
        algebra. A result belongs to the supplied declaration, never a synthetic
        machine created to lower a requirement signature.
    */
    pub(crate) fn lower_callable_scalar_contract_predicate(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        owner: symbols::SymbolHandle,
        parameters: &[StateParameter],
        result_type: TypeReferenceHandle,
        expression: ExpressionHandle,
        allow_result: bool,
        remaining: &mut usize,
    ) -> Option<CheckedBooleanExpression> {
        let budget = ContractBudget::new(remaining);
        ContractPredicates {
            program,
            operators,
            owner,
            result_type,
            parameters,
            allow_result,
            logical_captures: Vec::new(),
            remaining: &budget.remaining,
        }
        .boolean(expression, 0)
    }

    /*
        Query-owned copy counter. Even an unsuccessful read commits consumption to
        the caller when the scope exits; child logical readers borrow this counter.
    */
    struct ContractBudget<'budget> {
        remaining: std::cell::Cell<usize>,
        caller: &'budget mut usize,
    }

    impl<'budget> ContractBudget<'budget> {
        fn new(caller: &'budget mut usize) -> Self {
            Self {
                remaining: std::cell::Cell::new(*caller),
                caller,
            }
        }
    }

    impl Drop for ContractBudget<'_> {
        fn drop(&mut self) {
            *self.caller = self.remaining.get();
        }
    }

    struct ContractPredicates<'program, 'budget> {
        program: &'program dyn ::validation::CheckingInput,
        operators: &'program CheckedOperatorFacts,
        owner: symbols::SymbolHandle,
        result_type: TypeReferenceHandle,
        parameters: &'program [StateParameter],
        allow_result: bool,
        remaining: &'budget std::cell::Cell<usize>,
        logical_captures: Vec<validation::LogicalMatchBinding<logical_matches::ContractSubject>>,
    }

    /*
        Project a scalar membership under the callable's existing formal namespace.
        The domain owner supplies the complete theory; this does not reinterpret
        routed membership as a predicate or infer a subject from its spelling.
    */
    pub(crate) fn lower_callable_scalar_membership(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        owner: symbols::SymbolHandle,
        parameters: &[StateParameter],
        membership: &typed_trees::domain::ProofMembershipFact,
        remaining: &mut usize,
    ) -> Option<CheckedBooleanExpression> {
        validation::scalar_membership_carrier(program, membership)?;
        lower_callable_scalar_membership_value(
            program, operators, owner, parameters, membership, remaining,
        )
    }

    /*
        Numeric projection for an owner that independently retained exact membership.
        This must never stand in for the nominal admission obligation itself.
    */
    pub(crate) fn lower_callable_scalar_membership_value(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        owner: symbols::SymbolHandle,
        parameters: &[StateParameter],
        membership: &typed_trees::domain::ProofMembershipFact,
        remaining: &mut usize,
    ) -> Option<CheckedBooleanExpression> {
        let primitive = validation::scalar_membership_value_carrier(program, membership)?;
        let budget = ContractBudget::new(remaining);
        let mut context = ContractPredicates {
            program,
            operators,
            owner,
            result_type: TypeReferenceHandle::invalid(),
            parameters,
            allow_result: false,
            logical_captures: Vec::new(),
            remaining: &budget.remaining,
        };
        let subject = if primitive == PrimitiveType::Bool {
            CheckedScalarExpression::Boolean(Box::new(context.boolean(membership.value, 0)?))
        } else if let Some((subject, _)) = context.numeric_term(membership.value, 0) {
            (scalar_expression_type(&subject) == Some(primitive)).then_some(subject)?
        } else {
            lower_contract_integer_literal(program, operators, membership.value, primitive)?
        };
        let mut predicates =
            validation::scalar_membership_value_predicates(program, membership, &subject)?
                .into_iter();
        let first = predicates
            .next()
            .unwrap_or(CheckedBooleanExpression::Constant(true));
        Some(
            predicates.fold(first, |left, right| CheckedBooleanExpression::And {
                left: Box::new(left),
                right: Box::new(right),
            }),
        )
    }

    impl ContractPredicates<'_, '_> {
        /*
            A selector has the collection's count context even when an anonymous
            literal has no independently inferred comparison carrier. Keep this
            namespace common to scalar reads and descriptor metadata contracts.
        */
        fn selector_term(
            &self,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Option<CheckedScalarExpression> {
            if depth >= 64 {
                return None;
            }
            self.numeric_term(expression, depth + 1)
                .map(|(term, _)| term)
                .or_else(|| {
                    lower_contract_integer_literal(
                        self.program,
                        self.operators,
                        expression,
                        PrimitiveType::U64,
                    )
                })
        }

        /*
            Scalar storage observations share the formal-place and access check.
            Selectors use this contract's exact scalar namespace; the shared read
            planner retains builtin meaning, declaration geometry and occurrences.
        */
        fn structural_observation(
            &self,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Option<(CheckedScalarExpression, TypeReferenceHandle)> {
            if depth >= 64 {
                return None;
            }
            if let Some(observation) = self.logical_saved_observation(expression) {
                return Some(observation);
            }
            if self.allow_result
                && let Some(result) = validation::reserved_result_place(self.program, expression)
            {
                if result.machine_symbol != self.owner
                    || validation::reserved_result_owner(self.program, result.root)
                        != Some((self.owner, self.result_type))
                {
                    return None;
                }
                let mut receiver = self.result_type;
                for segment in &result.segments {
                    if validation::reference_shell(self.program, receiver).is_some() {
                        return None;
                    }
                    receiver = validation::project_type_reference_from_segments(
                        self.program,
                        receiver,
                        std::slice::from_ref(segment),
                    )?;
                }
                if validation::reference_shell(self.program, receiver).is_some() {
                    return None;
                }
                let reference = validation::field_domains::readable_type_reference(
                    self.program,
                    result.type_reference,
                )?;
                let primitive_type = self.program.primitive_type_reference(reference)?;
                let path =
                    structural_fields::structural_projection_path(self.program, &result.segments)?;
                let parameter_position = u32::try_from(self.parameters.len()).ok()?;
                let term = if primitive_type == PrimitiveType::Bool {
                    CheckedScalarExpression::Boolean(Box::new(
                        CheckedBooleanExpression::StructuralParameterField {
                            observation: Default::default(),
                            parameter_position,
                            path,
                        },
                    ))
                } else {
                    CheckedScalarExpression::StructuralParameterField {
                        observation: Default::default(),
                        parameter_position,
                        path,
                        primitive_type,
                    }
                };
                return Some((term, reference));
            }
            let (term, _) = scalar_storage_read_with_selectors(
                self.program,
                self.operators,
                expression,
                self.parameters,
                |_, selector, _| self.selector_term(selector, depth + 1),
            )?;
            let position = match &term {
                CheckedScalarExpression::StructuralParameterIndexedRead { root, .. } => {
                    root.parameter()?
                }
                CheckedScalarExpression::StructuralParameterField {
                    parameter_position, ..
                } => *parameter_position,
                CheckedScalarExpression::Boolean(value) => {
                    let CheckedBooleanExpression::StructuralParameterField {
                        parameter_position,
                        ..
                    } = value.as_ref()
                    else {
                        return None;
                    };
                    *parameter_position
                }
                _ => return None,
            };
            let reference = validation::parameter_expression_result_type_reference(
                self.program,
                self.owner,
                self.parameters,
                expression,
            )?;
            let reference =
                validation::field_domains::readable_type_reference(self.program, reference)?;
            if scalar_expression_type(&term) != self.program.primitive_type_reference(reference) {
                return None;
            }
            let parameter = self.parameters.get(position as usize)?;
            if !contract_storage_root(self.program, parameter, self.allow_result) {
                return None;
            }
            let term = if matches!(
                &term,
                CheckedScalarExpression::StructuralParameterIndexedRead {
                    primitive_type: PrimitiveType::Bool,
                    ..
                }
            ) {
                let selected = term.parameter_observation_projection()?;
                if selected.path.iter().any(|segment| {
                    matches!(
                        segment,
                        checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(_)
                    )
                }) {
                    return None;
                }
                CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::StructuralParameterField {
                        observation: Default::default(),
                        parameter_position: selected.parameter_position,
                        path: selected.path,
                    },
                ))
            } else {
                term
            };
            Some((term, reference))
        }

        fn subject(
            &self,
            expression: ExpressionHandle,
        ) -> Option<(usize, TypeReferenceHandle, bool)> {
            let program = self.program;
            let path = program.expression_table.name_path_node(expression)?;
            if let Some((position, parameter)) =
                self.parameters.iter().enumerate().find(|(_, parameter)| {
                    parameter.symbol == path.symbol && path.head_symbol == parameter.symbol
                })
            {
                if (parameter.is_const)
                    || expressions(program).name_path_members(path.members).len() != 1
                {
                    return None;
                }
                if self.allow_result && parameter.is_mutable {
                    return None;
                }
                program.primitive_type_reference(parameter.type_reference)?;
                if parameter.relevance.is_erased() {
                    let erased_position =
                        program.erased_scalar_lane_base(self.parameters, position);
                    return Some((erased_position, parameter.type_reference, true));
                }
                let scalar_position = scalar_position_count(program, &self.parameters[..position]);
                return Some((scalar_position, parameter.type_reference, false));
            }
            (self.allow_result
                && validation::reserved_result_owner(program, expression)
                    == Some((self.owner, self.result_type)))
            .then(|| {
                (
                    scalar_position_count(program, self.parameters),
                    self.result_type,
                    false,
                )
            })
        }

        /*
            One comparison operand as a typed scalar term: an exact entry/result
            subject or landed IEEE literal, total builtin integer arithmetic over
            subjects and contextual integer literals, or a value-preserving
            Exact widening of a term. The operands' arithmetic policies select the
            operation as they do in a body, so an Exact, Wrapping or Saturating
            carrier keeps its own meaning; Trapping arithmetic, which can crash,
            or any other spelling stays outside the closed language and lowers to
            `None`.
        */
        fn numeric_term(
            &self,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Option<(CheckedScalarExpression, TypeReferenceHandle)> {
            let program = self.program;
            let recipe = program.constant_recipe_for_contract_expression(expression);
            if recipe.is_valid() {
                let reference = validation::parameter_expression_result_type_reference(
                    program,
                    self.owner,
                    self.parameters,
                    expression,
                )?;
                let primitive = program.primitive_type_reference(reference)?;
                let Some(value) =
                    program.closed_integer_expression_value_in(expression, self.owner)
                else {
                    return Some((
                        CheckedScalarExpression::ConstantApplication {
                            recipe,
                            owner: self.owner,
                            primitive_type: primitive,
                        },
                        reference,
                    ));
                };
                typed_trees::closed_numeric::land_integer(&value, primitive)?;
                let spelling = value.to_string();
                let (negative, digits) = spelling
                    .strip_prefix('-')
                    .map_or((false, spelling.as_str()), |digits| (true, digits));
                let literal = numerics::literals::IntegerLiteral::from_parts(
                    negative,
                    numerics::literals::IntegerRadix::Decimal,
                    digits,
                )
                .ok()?
                .with_landing(numerics::literals::IntegerLanding {
                    landed_type: primitive.landed_landing()?,
                    domain: ArithmeticDomain::Exact,
                });
                return Some((
                    CheckedScalarExpression::IntegerLiteral { literal },
                    reference,
                ));
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Match(_) if depth < 64 => {
                    self.logical_match_numeric(expression, depth + 1)
                }
                ExpressionNode::Float(literal) => {
                    let reference = validation::parameter_expression_result_type_reference(
                        program,
                        self.owner,
                        self.parameters,
                        expression,
                    )?;
                    let value =
                        typed_trees::expression::ieee_float_value(literal, literal.landing()?);
                    Some((
                        CheckedScalarExpression::IeeeFloatLiteral { value },
                        reference,
                    ))
                }
                ExpressionNode::Name(_) => {
                    let Some((position, reference, erased)) = self.subject(expression) else {
                        return self.structural_observation(expression, depth + 1);
                    };
                    let primitive_type = program.primitive_type_reference(reference)?;
                    Some((
                        if erased {
                            CheckedScalarExpression::ErasedParameter {
                                position,
                                primitive_type,
                            }
                        } else {
                            CheckedScalarExpression::Parameter {
                                position,
                                primitive_type,
                            }
                        },
                        reference,
                    ))
                }
                ExpressionNode::Binary(binary)
                    if depth < 64 && operator_is_builtin(self.operators, expression) =>
                {
                    let spelling = match binary.operator {
                        BinaryOperator::Add => language_core::OperatorSpelling::Add,
                        BinaryOperator::Subtract => language_core::OperatorSpelling::Subtract,
                        BinaryOperator::Multiply => language_core::OperatorSpelling::Multiply,
                        _ => return None,
                    };
                    let terms = [binary.left, binary.right]
                        .map(|operand| self.numeric_term(operand, depth + 1));
                    let (carrier_type, carrier_reference) =
                        terms.iter().flatten().find_map(|(term, reference)| {
                            Some((scalar_expression_type(term)?, *reference))
                        })?;
                    if !is_integer(carrier_type) {
                        return None;
                    }
                    let operand_types = [Some(carrier_reference), Some(carrier_reference)];
                    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        self.owner,
                        expression,
                        spelling,
                        &operand_types,
                    ) {
                        return None;
                    }
                    let [left, right] = [
                        (binary.left, terms[0].clone()),
                        (binary.right, terms[1].clone()),
                    ]
                    .map(|(operand, term)| match term {
                        Some((term, reference)) => {
                            (scalar_expression_type(&term) == Some(carrier_type)).then(|| {
                                (
                                    term,
                                    program.arithmetic_domain_for_type_reference(reference),
                                    reference,
                                )
                            })
                        }
                        None => lower_contract_integer_literal(
                            program,
                            self.operators,
                            operand,
                            carrier_type,
                        )
                        .map(|literal| (literal, ArithmeticDomain::Exact, carrier_reference)),
                    });
                    let (left, left_domain, left_reference) = left?;
                    let (right, right_domain, right_reference) = right?;
                    let (term, domain) = construct_integer_binary(
                        binary.operator,
                        left,
                        left_domain,
                        right,
                        right_domain,
                        false,
                        false,
                    )?;
                    if matches!(
                        &term,
                        CheckedScalarExpression::IntegerBinary { kind, .. } if kind.is_trapping()
                    ) {
                        return None;
                    }
                    let reference =
                        if program.arithmetic_domain_for_type_reference(left_reference) == domain {
                            left_reference
                        } else {
                            right_reference
                        };
                    Some((term, reference))
                }
                ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                    if let Some(term) = structural_fields::structural_sequence_length(
                        program,
                        self.operators,
                        self.parameters,
                        expression,
                        |_, operand, _| self.selector_term(operand, depth + 1),
                    ) {
                        if let CheckedScalarExpression::StructuralSequenceLength { root, .. } =
                            &term
                            && !matches!(root, checked_trees::CheckedStorageRoot::Parameter { .. })
                        {
                            return None;
                        }
                        if let CheckedScalarExpression::StructuralSequenceLength { path, .. } =
                            &term
                        {
                            checked_trees::CheckedScalarReadProjection::static_path(path)?;
                        }
                        let reference = program.type_reference_table.named_references().find_map(
                            |(reference, symbol, _)| {
                                (program.symbols.builtin_type_atom(symbol)
                                    == Some(symbols::BuiltinTypeAtom::U64))
                                .then_some(reference)
                            },
                        )?;
                        return Some((term, reference));
                    }
                    self.structural_observation(expression, depth + 1)
                }
                ExpressionNode::Cast(cast)
                    if depth < 64
                        && !cast.form.is_recast()
                        && cast.semantic_domain.is_empty()
                        && cast.domain == ArithmeticDomain::Exact =>
                {
                    let target_type = program.primitive_type_reference(cast.target_type)?;
                    let (operand, _) = self.numeric_term(cast.value, depth + 1)?;
                    let source_type = scalar_expression_type(&operand)?;
                    if !validation::integer_widen_is_total(source_type, target_type) {
                        return None;
                    }
                    Some((
                        CheckedScalarExpression::IntegerWiden {
                            primitive_type: target_type,
                            operand: Box::new(operand),
                        },
                        cast.target_type,
                    ))
                }
                _ => None,
            }
        }

        fn integer_math_expression(
            &mut self,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Option<CheckedIntegerMathExpression> {
            let mut terms = arena::Arena::new();
            let root = self.integer_math_term(expression, depth, &mut terms)?;
            Some(CheckedIntegerMathExpression { terms, root })
        }

        fn integer_math_term(
            &mut self,
            expression: ExpressionHandle,
            depth: usize,
            terms: &mut arena::Arena<CheckedIntegerMathTerm>,
        ) -> Option<arena::Handle<CheckedIntegerMathTerm>> {
            if depth >= 64
                || self.remaining.get() == 0
                || !self
                    .program
                    .expression_table
                    .expression_is_valid(expression)
            {
                return None;
            }
            self.remaining.set(self.remaining.get() - 1);
            let program = self.program;
            let term = if let Some((primitive_type, argument)) =
                validation::integer_embedding_argument(program, expression)
            {
                let operand = self
                    .numeric_term(argument, depth + 1)
                    .map(|(term, _)| term)
                    .or_else(|| {
                        lower_contract_integer_literal(
                            program,
                            self.operators,
                            argument,
                            primitive_type,
                        )
                    })?;
                if scalar_expression_type(&operand) != Some(primitive_type)
                    || !is_integer(primitive_type)
                {
                    return None;
                }
                CheckedIntegerMathTerm::Embed {
                    primitive_type,
                    operand,
                }
            } else if let Some(member) = expressions(program)
                .name_path_node(expression)
                .and_then(|path| program.symbols.builtin_type_member(path.symbol))
            {
                CheckedIntegerMathTerm::BuiltinTypeMember(member)
            } else if let Some(binary) = program.expression_table.binary_node(expression)
                && validation::proof_integer_expression(program, expression)
                && operator_is_builtin(self.operators, expression)
            {
                let operator = binary.operator;
                if !matches!(
                    operator,
                    BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
                ) {
                    return None;
                }
                let left = self.integer_math_term(binary.left, depth + 1, terms)?;
                let right = self.integer_math_term(binary.right, depth + 1, terms)?;
                match operator {
                    BinaryOperator::Add => CheckedIntegerMathTerm::Add { left, right },
                    BinaryOperator::Subtract => CheckedIntegerMathTerm::Subtract { left, right },
                    BinaryOperator::Multiply => CheckedIntegerMathTerm::Multiply { left, right },
                    _ => return None,
                }
            } else {
                CheckedIntegerMathTerm::Literal(validation::anonymous_integer_math_literal(
                    program, expression,
                )?)
            };
            Some(terms.insert(term))
        }

        fn boolean_type(&self, expression: ExpressionHandle) -> Option<TypeReferenceHandle> {
            if matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::Name(_)
            ) {
                return self.subject(expression).map(|(_, reference, _)| reference);
            }
            self.program
                .type_reference_table
                .named_references()
                .find_map(|(reference, symbol, _)| {
                    (self.program.symbols.builtin_type_atom(symbol)
                        == Some(symbols::BuiltinTypeAtom::Bool))
                    .then_some(reference)
                })
        }

        /*
            Rejoin a callee-relative observation with this contract's exact formal
            or result root. Stored-reference crossings need a distinct live epoch;
            they cannot be encoded as another field below the descriptor storage.
        */
        fn numeric_subject(
            &self,
            subject: validation::PredicateIntegerSubject,
            depth: usize,
        ) -> Option<(CheckedScalarExpression, TypeReferenceHandle)> {
            if subject.projection.is_empty() {
                return self.numeric_term(subject.expression, depth);
            }
            let value =
                self.projected_scalar_subject(subject.expression, &subject.projection, depth)?;
            is_integer(self.program.primitive_type_reference(value.1)?).then_some(value)
        }

        fn projected_scalar_subject(
            &self,
            expression: ExpressionHandle,
            projection: &[facts::PlaceSegment],
            depth: usize,
        ) -> Option<(CheckedScalarExpression, TypeReferenceHandle)> {
            if depth >= 64 || projection.is_empty() {
                return None;
            }
            let (position, mut path, reference) = if self.allow_result
                && let Some(result) = validation::reserved_result_place(self.program, expression)
            {
                if result.machine_symbol != self.owner
                    || validation::reserved_result_owner(self.program, result.root)
                        != Some((self.owner, self.result_type))
                {
                    return None;
                }
                let reference = validation::owned_projection_type_reference(
                    self.program,
                    self.result_type,
                    &result.segments,
                )?;
                (
                    u32::try_from(self.parameters.len()).ok()?,
                    structural_fields::structural_projection_path(self.program, &result.segments)?,
                    reference,
                )
            } else {
                let (root, path, reference) = structural_fields::structural_storage_place(
                    self.program,
                    self.parameters,
                    expression,
                )?;
                let position = root.parameter()?;
                if !contract_storage_root(
                    self.program,
                    self.parameters.get(position as usize)?,
                    self.allow_result,
                ) {
                    return None;
                }
                (position, path, reference)
            };
            let segments = structural_fields::applied_projection_segments(
                self.program,
                reference,
                projection,
            )?;
            let reference =
                validation::owned_projection_type_reference(self.program, reference, &segments)?;
            let primitive_type = self.program.primitive_type_reference(reference)?;
            if !is_integer(primitive_type) && primitive_type != PrimitiveType::Bool {
                return None;
            }
            path.extend(structural_fields::structural_projection_path(
                self.program,
                &segments,
            )?);
            let value = if primitive_type == PrimitiveType::Bool {
                CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::StructuralParameterField {
                        observation: Default::default(),
                        parameter_position: position,
                        path,
                    },
                ))
            } else {
                CheckedScalarExpression::StructuralParameterField {
                    observation: Default::default(),
                    parameter_position: position,
                    path,
                    primitive_type,
                }
            };
            Some((value, reference))
        }

        /*
            Publish a whole predicate only after checked bidirectional equivalence.
            Actual observations still pass the ordinary closed formal namespace and
            readable descriptor gates; the explorer supplies no executable access.
        */
        fn exact_predicate(
            &mut self,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Option<CheckedBooleanExpression> {
            if let Some(relation) =
                validation::exact_predicate_ordering(self.program, self.owner, expression)
            {
                let remaining = self.remaining.get().checked_sub(1)?;
                self.remaining.set(remaining);
                let (
                    validation::PredicateIntegerOperand::Argument(left),
                    validation::PredicateIntegerOperand::Argument(right),
                ) = (relation.left, relation.right)
                else {
                    return None;
                };
                return construct_integer_comparison(
                    relation.operator,
                    self.numeric_subject(left, depth + 1)?.0,
                    self.numeric_subject(right, depth + 1)?.0,
                );
            }
            if let Some(predicate) = self.exact_predicate_condition(expression, depth + 1) {
                return Some(predicate);
            }
            let rows = validation::exact_predicate_intervals(self.program, self.owner, expression)?;
            let remaining = self.remaining.get().checked_sub(rows.len())?;
            self.remaining.set(remaining);
            let mut predicate = CheckedBooleanExpression::Constant(true);
            for row in rows {
                let subject = if row.extent {
                    let term = structural_fields::structural_sequence_receiver_length(
                        self.program,
                        self.operators,
                        self.parameters,
                        row.argument,
                        |_, operand, _| self.selector_term(operand, depth + 1),
                    )?;
                    if let CheckedScalarExpression::StructuralSequenceLength { root, path } = &term
                    {
                        root.parameter()?;
                        checked_trees::CheckedScalarReadProjection::static_path(path)?;
                    }
                    term
                } else {
                    self.numeric_term(row.argument, depth + 1)?.0
                };
                let primitive = scalar_expression_type(&subject)?;
                if !is_integer(primitive) {
                    return None;
                }
                for (value, lower) in [(row.low, true), (row.high, false)] {
                    let literal = numerics::bignum::BigInt::from_i128(value);
                    let bound = CheckedScalarExpression::IntegerLiteral {
                        literal: validation::land_integer_value(&literal, primitive)?,
                    };
                    let comparison = if lower {
                        construct_integer_comparison(
                            BinaryOperator::LessOrEqual,
                            bound,
                            subject.clone(),
                        )?
                    } else {
                        construct_integer_comparison(
                            BinaryOperator::LessOrEqual,
                            subject.clone(),
                            bound,
                        )?
                    };
                    predicate = CheckedBooleanExpression::And {
                        left: Box::new(predicate),
                        right: Box::new(comparison),
                    };
                }
            }
            Some(predicate)
        }

        fn boolean(
            &mut self,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Option<CheckedBooleanExpression> {
            let expression = self.program.static_boolean_normal_form(expression);
            if self
                .program
                .static_type_predicate_operands(expression)
                .is_some()
            {
                return self
                    .program
                    .static_type_predicate_value(expression)
                    .map(CheckedBooleanExpression::Constant);
            }
            if depth >= 64
                || self.remaining.get() == 0
                || !self
                    .program
                    .expression_table
                    .expression_is_valid(expression)
            {
                return None;
            }
            self.remaining.set(self.remaining.get() - 1);
            let program = self.program;
            if program
                .prepared_mathematics()
                .is_some_and(|view| view.application(expression).is_some())
            {
                return validation::mathematical_applications::scalar_predicate(
                    program,
                    expression,
                    |capture, _| {
                        self.numeric_term(capture, depth + 1)
                            .map(|(value, _)| value)
                    },
                );
            }
            if let Some(membership) =
                super::case_membership::lower(program, self.parameters, expression)
                    .or_else(|| {
                        super::case_membership::lower_in_declaration(
                            program,
                            self.owner,
                            self.parameters,
                            expression,
                        )
                    })
                    .or_else(|| {
                        self.allow_result
                            .then(|| {
                                lower_result(program, self.owner, self.result_type, expression)
                            })
                            .flatten()
                    })
            {
                return Some(membership);
            }
            if let Some(predicate) = self.exact_predicate(expression, depth + 1) {
                return Some(predicate);
            }
            if matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
            ) && self.subject(expression).is_none()
                && let Some((CheckedScalarExpression::Boolean(predicate), _)) =
                    self.structural_observation(expression, depth + 1)
            {
                return Some(*predicate);
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Match(_) => self.logical_match_boolean(expression, depth + 1),
                ExpressionNode::SavedSubject(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_)
                    if self.logical_saved_observation(expression).is_some() =>
                {
                    let (CheckedScalarExpression::Boolean(value), _) =
                        self.logical_saved_observation(expression)?
                    else {
                        return None;
                    };
                    Some(*value)
                }
                ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
                ExpressionNode::Call(_) => {
                    let (predicate, operand) =
                        typed_trees::byte_predicates::byte_predicate_call(program, expression)?;
                    let (root, path, _) =
                        comparable_byte_carrier(program, self.parameters, operand)?;
                    root.parameter()?;
                    Some(CheckedBooleanExpression::ByteSequencePredicate {
                        predicate,
                        root,
                        path,
                    })
                }
                ExpressionNode::Name(_) => {
                    let (position, reference, erased) = self.subject(expression)?;
                    (program.primitive_type_reference(reference) == Some(PrimitiveType::Bool))
                        .then_some(if erased {
                            CheckedBooleanExpression::ErasedParameter { position }
                        } else {
                            CheckedBooleanExpression::Parameter { position }
                        })
                }
                ExpressionNode::Unary(unary)
                    if unary.operator == UnaryOperator::LogicalNot
                        && operator_is_builtin(self.operators, expression) =>
                {
                    Some(CheckedBooleanExpression::Not(Box::new(
                        self.boolean(unary.operand, depth + 1)?,
                    )))
                }
                ExpressionNode::Binary(binary) => {
                    let subjects =
                        [binary.left, binary.right].map(|operand| self.numeric_term(operand, 0));
                    if let Some(comparison) = lower_float_contract_comparison(
                        program,
                        self.owner,
                        expression,
                        subjects.clone(),
                    ) {
                        return Some(comparison);
                    }
                    if !operator_is_builtin(self.operators, expression) {
                        return None;
                    }
                    if validation::proof_integer_expression(program, binary.left)
                        || validation::proof_integer_expression(program, binary.right)
                    {
                        let (kind, swap, negate) = match binary.operator {
                            BinaryOperator::Equal => {
                                (CheckedIntegerComparisonKind::Equal, false, false)
                            }
                            BinaryOperator::NotEqual => {
                                (CheckedIntegerComparisonKind::Equal, false, true)
                            }
                            BinaryOperator::Less => {
                                (CheckedIntegerComparisonKind::LessThan, false, false)
                            }
                            BinaryOperator::LessOrEqual => {
                                (CheckedIntegerComparisonKind::LessOrEqual, false, false)
                            }
                            BinaryOperator::Greater => {
                                (CheckedIntegerComparisonKind::LessThan, true, false)
                            }
                            BinaryOperator::GreaterOrEqual => {
                                (CheckedIntegerComparisonKind::LessOrEqual, true, false)
                            }
                            _ => return None,
                        };
                        let left = self.integer_math_expression(
                            if swap { binary.right } else { binary.left },
                            depth + 1,
                        )?;
                        let right = self.integer_math_expression(
                            if swap { binary.left } else { binary.right },
                            depth + 1,
                        )?;
                        let comparison =
                            CheckedBooleanExpression::IntegerMathComparison { kind, left, right };
                        return Some(if negate {
                            CheckedBooleanExpression::Not(Box::new(comparison))
                        } else {
                            comparison
                        });
                    }
                    if let Some(comparison) = lower_integer_contract_comparison(
                        program,
                        self.operators,
                        self.owner,
                        expression,
                        subjects,
                    ) {
                        return Some(comparison);
                    }
                    if !matches!(
                        binary.operator,
                        BinaryOperator::And
                            | BinaryOperator::Or
                            | BinaryOperator::Equal
                            | BinaryOperator::NotEqual
                    ) {
                        return None;
                    }
                    let left = Box::new(self.boolean(binary.left, depth + 1)?);
                    let right = Box::new(self.boolean(binary.right, depth + 1)?);
                    Some(match binary.operator {
                        BinaryOperator::And => CheckedBooleanExpression::And { left, right },
                        BinaryOperator::Or => CheckedBooleanExpression::Or { left, right },
                        BinaryOperator::Equal | BinaryOperator::NotEqual => {
                            let spelling = if binary.operator == BinaryOperator::Equal {
                                language_core::OperatorSpelling::Equal
                            } else {
                                language_core::OperatorSpelling::NotEqual
                            };
                            let types = [
                                Some(self.boolean_type(binary.left)?),
                                Some(self.boolean_type(binary.right)?),
                            ];
                            if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                                program, self.owner, expression, spelling, &types,
                            ) {
                                return None;
                            }
                            let equality = CheckedBooleanExpression::Equal { left, right };
                            if binary.operator == BinaryOperator::Equal {
                                equality
                            } else {
                                CheckedBooleanExpression::Not(Box::new(equality))
                            }
                        }
                        _ => return None,
                    })
                }
                _ => None,
            }
        }
    }

    /*
        Entry requirements observe every readable argument's completed value,
        including an owned structural copy. Normal guarantees use the post-state;
        this namespace can transport that storage only through a readable reference.
        Keeping these phases distinct avoids excluding owned fields from requires
        merely because their callee-local post-state is not a caller observation.
    */
    fn contract_storage_root(
        program: &dyn ::validation::CheckingInput,
        parameter: &StateParameter,
        allow_result: bool,
    ) -> bool {
        let mut type_reference = parameter.type_reference;
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::Reference { access, .. } => {
                    return match access {
                        language_core::ReferenceAccess::Shared => {
                            !allow_result || !parameter.is_mutable
                        }
                        language_core::ReferenceAccess::Mutable => true,
                        _ => false,
                    };
                }
                _ => return !allow_result,
            }
        }
    }

    /*
        Subject readers retain their own namespace custody. Comparison selection,
        contextual literal landing, and same-carrier construction have one owner.
    */
    pub(crate) fn lower_integer_contract_comparison(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        owner: symbols::SymbolHandle,
        expression: ExpressionHandle,
        subjects: [Option<(CheckedScalarExpression, TypeReferenceHandle)>; 2],
    ) -> Option<CheckedBooleanExpression> {
        let binary = program.expression_table.binary_node(expression)?;
        if operators.uses.iter().any(|(_, operator)| {
            operator.expression == expression
                && operator.status != CheckedOperatorResolutionStatus::BuiltinFallback
        }) {
            return None;
        }
        let spelling = validation::comparison_operator_spelling(binary.operator)?;
        let operand_types = [(binary.left, &subjects[0]), (binary.right, &subjects[1])].map(
            |(expression, subject)| {
                subject
                    .as_ref()
                    .map(|(_, type_reference)| *type_reference)
                    .or_else(|| {
                        validation::landed_integer_literal_type_reference(program, expression)
                    })
            },
        );
        /*
            A typed subject or already-landed literal supplies contextual landing.
            Keep literal comparisons as predicates too: folding their value here
            would lose the selected operator and carrier needed by source replay.
        */
        let contextual_type = operand_types.into_iter().flatten().next()?;
        let contextual_primitive = program.primitive_type_reference(contextual_type)?;
        if !is_integer(contextual_primitive) {
            return None;
        }
        if !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            owner,
            expression,
            spelling,
            &operand_types,
        ) {
            return None;
        }
        let operand =
            |expression, subject: Option<(CheckedScalarExpression, TypeReferenceHandle)>| {
                if let Some((value, type_reference)) = subject {
                    let primitive_type = program.primitive_type_reference(type_reference)?;
                    if !is_integer(primitive_type)
                        || scalar_expression_type(&value) != Some(primitive_type)
                    {
                        return None;
                    }
                    return Some(value);
                }
                lower_contract_integer_literal(program, operators, expression, contextual_primitive)
            };
        let [left_subject, right_subject] = subjects;
        let left = operand(binary.left, left_subject)?;
        let right = operand(binary.right, right_subject)?;
        construct_integer_comparison(binary.operator, left, right)
    }

    /*
        One IEEE relation over authenticated contract subjects. Subject readers
        own entry/result/state namespaces; this owner preserves format, literal bits,
        selected comparison meaning and unordered behavior for all those readers.
        Comparison spelling alone never grants builtin or sealed Float semantics.
    */
    pub(crate) fn lower_float_contract_comparison(
        program: &dyn ::validation::CheckingInput,
        owner: symbols::SymbolHandle,
        expression: ExpressionHandle,
        subjects: [Option<(CheckedScalarExpression, TypeReferenceHandle)>; 2],
    ) -> Option<CheckedBooleanExpression> {
        let binary = program.expression_table.binary_node(expression)?;
        let [
            Some((mut left, left_reference)),
            Some((mut right, right_reference)),
        ] = subjects
        else {
            return None;
        };
        let primitive = program.primitive_type_reference(left_reference)?;
        if !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
            || program.primitive_type_reference(right_reference) != Some(primitive)
            || scalar_expression_type(&left) != Some(primitive)
            || scalar_expression_type(&right) != Some(primitive)
            || !validation::has_builtin_or_sealed_float_comparison_meaning(
                program,
                owner,
                expression,
                binary.operator,
                &[Some(left_reference), Some(right_reference)],
            )
        {
            return None;
        }
        let kind = match binary.operator {
            BinaryOperator::Equal => CheckedIeeeFloatComparisonKind::Equal,
            BinaryOperator::NotEqual => CheckedIeeeFloatComparisonKind::NotEqual,
            BinaryOperator::Less => CheckedIeeeFloatComparisonKind::LessThan,
            BinaryOperator::LessOrEqual => CheckedIeeeFloatComparisonKind::LessOrEqual,
            BinaryOperator::Greater | BinaryOperator::GreaterOrEqual => {
                std::mem::swap(&mut left, &mut right);
                if binary.operator == BinaryOperator::Greater {
                    CheckedIeeeFloatComparisonKind::LessThan
                } else {
                    CheckedIeeeFloatComparisonKind::LessOrEqual
                }
            }
            _ => return None,
        };
        Some(CheckedBooleanExpression::ScalarIeeeFloatComparison {
            kind,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    /*
        Anonymous arithmetic lands at the comparison or typed peer's carrier through
        the ordinary scalar lowerer. With no runtime namespace, only a completed
        literal can supply this operand: typed operations retain their own policy
        and remain with `integer_term`, rather than being reinterpreted as constants.
    */
    fn lower_contract_integer_literal(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        expression: ExpressionHandle,
        primitive: PrimitiveType,
    ) -> Option<CheckedScalarExpression> {
        let value = lower_return_expression(
            program,
            operators,
            expression,
            &[],
            &[],
            &[],
            &[],
            primitive,
            &[],
        )?;
        matches!(value, CheckedScalarExpression::IntegerLiteral { .. }).then_some(value)
    }

    /*
        One authored parameter range constraint observed by the scalar contract
        family. The three projections share a single constraint walk so a
        requires-tail clause row and its retained roster evidence can never
        disagree about which source range they describe.
    */
    pub(crate) struct ParameterRangeRequirements {
        /*
            Scalar refinements as closed predicates, one slot per authored
            constraint. The crash requirement capsule and boundary replay read
            this projection. Legacy floating ranges retain `None` because their
            separate endpoint roster has not joined this predicate owner.
        */
        pub(crate) scalar_predicates: Vec<Option<CheckedBooleanExpression>>,
        /*
            Requires-tail rows in authored constraint order: supported integer
            ranges as `Predicate` clauses and retained floating ranges as
            `FloatRange` clauses carrying their IEEE endpoints verbatim. `None`
            marks a present range that could not be retained at all.
        */
        pub(crate) scalar_clauses: Vec<Option<checked_trees::ClosedScalarContractValue>>,
        /*
            The retained floating roster in dense scalar-parameter order. `None`
            records an incomplete roster — an authored floating range whose
            endpoints could not be retained exactly — so consumers fail closed
            rather than read a partial roster as complete.
        */
        pub(crate) float_entry_ranges: Option<Vec<checked_trees::ClosedFloatRangeRequirement>>,
        /*
            The retained integer roster in dense scalar-parameter order. `None`
            records an incomplete roster — a present integer range whose
            normalized endpoints could not be retained exactly — so consumers
            fail closed rather than read a partial roster as complete. Each row
            shares the exact landed literals its `Predicate` clause carries.
        */
        pub(crate) integer_entry_ranges: Option<Vec<checked_trees::ClosedIntegerRangeRequirement>>,
    }

    /*
        Bracket constraints are native numeric requires sugar. Keep every present
        range: integer intervals land as closed `<=` conjunctions while floating
        windows retain their authored IEEE endpoints and boundary kind verbatim —
        the exclusive end stays authored, never an integer predecessor.
    */
    pub(crate) fn lower_scalar_parameter_range_requirements(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> ParameterRangeRequirements {
        let Some(entry) = program.machine_states(machine).first() else {
            return ParameterRangeRequirements {
                scalar_predicates: Vec::new(),
                scalar_clauses: Vec::new(),
                float_entry_ranges: None,
                integer_entry_ranges: None,
            };
        };
        lower_callable_parameter_range_requirements(program, program.state_parameters(entry))
    }

    /*
        Parameter ranges belong to the declared callable signature, whether its
        owner has a machine body or is a bodyless operator requirement.
    */
    pub(crate) fn lower_callable_parameter_range_requirements(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
    ) -> ParameterRangeRequirements {
        let mut ranges = ParameterRangeRequirements {
            scalar_predicates: Vec::new(),
            scalar_clauses: Vec::new(),
            float_entry_ranges: Some(Vec::new()),
            integer_entry_ranges: Some(Vec::new()),
        };
        let mut scalar_position = 0;
        for parameter in parameters {
            let primitive_type = program.primitive_type_reference(parameter.type_reference);
            let position = scalar_position;
            if primitive_type.is_some() {
                scalar_position += 1;
            }
            let mut type_reference = parameter.type_reference;
            loop {
                match program.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        for constraint in program.type_reference_table.constraints(*constraints) {
                            if let typed_trees::types::TypeConstraintNode::Domain(domain) =
                                constraint
                                && let typed_trees::types::DomainConstraintSubject::Value(domain) =
                                    domain.subject
                            {
                                let predicate = primitive_type.and_then(|primitive_type| {
                                    validation::scalar_value_domain_predicate(
                                        domain,
                                        CheckedScalarExpression::Parameter {
                                            position,
                                            primitive_type,
                                        },
                                    )
                                });
                                ranges.scalar_clauses.push(
                                    predicate
                                        .clone()
                                        .map(checked_trees::ClosedScalarContractValue::Predicate),
                                );
                                ranges.scalar_predicates.push(predicate);
                                continue;
                            }
                            if let typed_trees::types::TypeConstraintNode::Domain(domain) =
                                constraint
                            {
                                let Some((low, high)) = primitive_type
                                    .filter(|primitive_type| is_integer(*primitive_type))
                                    .and_then(|primitive_type| {
                                        validation::exact_declared_domain_carrier_interval(
                                            program,
                                            primitive_type,
                                            domain,
                                        )
                                        .filter(|(minimum, maximum)| minimum <= maximum)
                                    })
                                else {
                                    continue;
                                };
                                ranges.push_integer(closed_integer_entry_range(
                                    program, parameter, position, low, high,
                                ));
                                continue;
                            }
                        }
                        type_reference = *base_type;
                    }
                    _ => break,
                }
            }
        }
        ranges
    }

    impl ParameterRangeRequirements {
        /*
            One integer entry range retains its normalized bounds on the roster
            while the requires tail keeps the predicate clause. One failed range
            voids the whole roster so no consumer reads a partial one.
        */
        fn push_integer(
            &mut self,
            range: Option<(
                CheckedBooleanExpression,
                checked_trees::ClosedIntegerRangeRequirement,
            )>,
        ) {
            match range {
                Some((predicate, requirement)) => {
                    if let Some(roster) = &mut self.integer_entry_ranges {
                        roster.push(requirement);
                    }
                    self.scalar_clauses.push(Some(
                        checked_trees::ClosedScalarContractValue::Predicate(predicate.clone()),
                    ));
                    self.scalar_predicates.push(Some(predicate));
                }
                None => {
                    self.integer_entry_ranges = None;
                    self.scalar_clauses.push(None);
                    self.scalar_predicates.push(None);
                }
            }
        }
    }

    /*
        A state's complete scalar parameter domain, projected into the dense scalar
        parameter namespace. Projection does not prove any incoming value satisfies it.
    */
    pub(crate) fn lower_state_domain_predicate(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        parameter: symbols::SymbolHandle,
        constraint: &typed_trees::types::DomainConstraint,
    ) -> Option<CheckedBooleanExpression> {
        let parameters = program.state_parameters(state);
        let index = parameters
            .iter()
            .position(|candidate| candidate.symbol == parameter)?;
        let authored = &parameters[index];
        if authored.relevance.is_erased() || authored.is_const {
            return None;
        }
        let position = scalar_position_count(program, &parameters[..index]);
        validation::scalar_domain_value_predicate(
            program,
            constraint,
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: program.primitive_type_reference(authored.type_reference)?,
            },
        )
    }

    /*
        The closed `low <= parameter && parameter <= high` clause and its roster
        row for one integer parameter's interval, whether a bracketed range or an
        exactly-interval domain stated it.
    */
    fn closed_integer_entry_range(
        program: &dyn ::validation::CheckingInput,
        parameter: &StateParameter,
        position: usize,
        low: numerics::bignum::BigInt,
        high: numerics::bignum::BigInt,
    ) -> Option<(
        CheckedBooleanExpression,
        checked_trees::ClosedIntegerRangeRequirement,
    )> {
        if (parameter.is_const)
            || program.arithmetic_domain_for_type_reference(parameter.type_reference)
                != ArithmeticDomain::Exact
        {
            return None;
        }
        let primitive_type = program.primitive_type_reference(parameter.type_reference)?;
        if !is_integer(primitive_type) || low > high {
            return None;
        }
        let minimum_literal = validation::land_integer_value(&low, primitive_type)?;
        let maximum_literal = validation::land_integer_value(&high, primitive_type)?;
        let minimum = CheckedScalarExpression::IntegerLiteral {
            literal: minimum_literal.clone(),
        };
        let maximum = CheckedScalarExpression::IntegerLiteral {
            literal: maximum_literal.clone(),
        };
        let subject = CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        };
        let predicate = CheckedBooleanExpression::And {
            left: Box::new(construct_integer_comparison(
                BinaryOperator::LessOrEqual,
                minimum,
                subject.clone(),
            )?),
            right: Box::new(construct_integer_comparison(
                BinaryOperator::LessOrEqual,
                subject,
                maximum,
            )?),
        };
        let requirement = checked_trees::ClosedIntegerRangeRequirement {
            position,
            primitive_type,
            minimum: minimum_literal,
            maximum: maximum_literal,
        };
        Some((predicate, requirement))
    }

    /*
        Predicate obligations from authored parameter refinements. Legacy floating
        interval rows keep an unsupported slot until their endpoint owner joins
        the predicate representation; core value domains already use predicates.
    */
    pub(crate) fn lower_scalar_parameter_refinement_predicates(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Vec<Option<CheckedBooleanExpression>> {
        lower_scalar_parameter_range_requirements(program, machine).scalar_predicates
    }
}
pub(crate) mod scalar_lowering {
    /*
        Lowering one scalar expression: state and unit scalars, returns, operand
        landing, integer binary, bitwise and cast construction, and literal
        landing and retagging.

        Indexed observations use one ordered projection plan for members and
        selectors. A canonical place describes geometry but cannot retain an
        executable selector's occurrence; read plans therefore keep the full
        scalar operand and its exact indexing handle, independently of root location.

        Integer operators select their result from operand roles, not storage
        context: address offsets return addresses, while differences return counts.
        Anonymous offsets land as u64; named operands retain their source carriers.
        Later Terminal reconstruction independently checks those signatures and
        proves the mathematical result fits its carrier before native lowering.
    */
    pub(crate) use validation::integer_widen_is_total;

    pub(crate) use element_selection::*;
    pub(crate) use entry_lowering::*;
    pub(crate) use integers::*;
    pub(crate) use recast_reads::*;
    mod element_selection {
        /*
            Resolve storage observations before classifying their scalar or sequence
            endpoint. Authored indexes retain occurrences even when their values are
            literals: a runtime view still owes a live bound, and representation views
            need the selected element rather than an already folded scalar field.
        */
        use super::entry_lowering::*;
        use crate::values::scalar::expression_facts::{is_integer, scalar_expression_type};
        use crate::values::scalar::expression_plans::ScalarLocal;
        use crate::values::scalar::structural_fields;
        use checked_trees::{
            CheckedOperatorFacts, CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
        };
        use numerics::arithmetic::ArithmeticDomain;
        use numerics::literals::{IntegerLanding, LandedIntegerType};
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::signature::StateParameter;
        use typed_trees::types::PrimitiveType;

        /*
            A storage observation retains ordered selections before its endpoint is
            classified as a scalar or a sequence. Selectors carry both their evaluated
            value and the source occurrence that owns the bound.
        */
        pub(crate) struct ScalarStorageProjection {
            pub root: checked_trees::CheckedStorageRoot,
            pub path: Vec<checked_trees::CheckedScalarReadProjection>,
            pub selected_type: typed_trees::types::TypeReferenceHandle,
        }

        /*
            Prepare one scalar indexed observation independently of the arrangement of
            indexes and fields after its first collection selection. Every runtime
            selector retains its full scalar value and exact indexing occurrence.
            Storage roots and declared carriers, not source nesting families, determine
            how the observation is emitted. Static literal geometry remains explicit.
        */
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn scalar_storage_read(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            expression: ExpressionHandle,
            parameters: &[StateParameter],
            authored_parameters: &[StateParameter],
            parameter_types: &[PrimitiveType],
            locals: &[ScalarLocal],
            exact_integer_casts: &[validation::ExactIntegerCastFact],
        ) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
            scalar_storage_read_with_selectors(
                program,
                operators,
                expression,
                authored_parameters,
                |_, operand, _| {
                    pure_selector(
                        program,
                        operators,
                        operand,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        exact_integer_casts,
                    )
                },
            )
        }

        /*
            Prepare the same observation with selectors supplied by the consuming
            scalar namespace. The reader owns indexing occurrences and declaration
            geometry; supplying a value never supplies an indexing proof.
        */
        pub(crate) fn scalar_storage_read_with_selectors(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            expression: ExpressionHandle,
            authored_parameters: &[StateParameter],
            selector: impl FnMut(
                ExpressionHandle,
                ExpressionHandle,
                typed_trees::types::TypeReferenceHandle,
            ) -> Option<CheckedScalarExpression>,
        ) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
            let projection = storage_projection(
                program,
                operators,
                expression,
                authored_parameters,
                selector,
            )?;
            scalar_storage_read_projection(program, expression, authored_parameters, projection)
        }

        /*
            Classify a typed read after its owner has substituted the exact formal root
            and selector namespace. The original occurrence and ordered path survive.
        */
        pub(crate) fn scalar_storage_read_projection(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            authored_parameters: &[StateParameter],
            projection: ScalarStorageProjection,
        ) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
            let first_index = projection.path.iter().position(|segment| {
                matches!(
                    segment,
                    checked_trees::CheckedScalarReadProjection::RuntimeIndex(_)
                )
            });
            let Some(first_index) = first_index else {
                let parameter_position = projection.root.parameter()?;
                let path = projection
                    .path
                    .into_iter()
                    .map(|segment| match segment {
                        checked_trees::CheckedScalarReadProjection::Static(segment) => {
                            Some(segment)
                        }
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                let mut reference = projection.selected_type;
                if path.is_empty() {
                    let parameter =
                        authored_parameters.get(usize::try_from(parameter_position).ok()?)?;
                    reference = parameter.type_reference;
                    let shell = validation::reference_shell(program, reference)?;
                    if !matches!(
                        shell.access,
                        language_core::ReferenceAccess::Shared
                            | language_core::ReferenceAccess::Mutable
                    ) {
                        return None;
                    }
                    reference =
                        validation::field_domains::readable_type_reference(program, reference)?;
                }
                let primitive_type = program.primitive_type_reference(reference)?;
                if path.is_empty() {
                    let parameter =
                        authored_parameters.get(usize::try_from(parameter_position).ok()?)?;
                    if parameter.relevance.is_erased() {
                        return crate::values::scalar::primitive_reference_read::erased_observation(
                            program,
                            authored_parameters,
                            parameter.symbol,
                            primitive_type,
                        );
                    }
                } else if authored_parameters
                    .get(usize::try_from(parameter_position).ok()?)
                    .is_some_and(|parameter| parameter.relevance.is_erased())
                {
                    return crate::values::scalar::primitive_reference_read::erased_leaf_observation(
                        program,
                        authored_parameters,
                        usize::try_from(parameter_position).ok()?,
                        &path,
                        primitive_type,
                        reference,
                    );
                }
                let expression = if primitive_type == PrimitiveType::Bool {
                    CheckedScalarExpression::Boolean(Box::new(
                        checked_trees::CheckedBooleanExpression::StructuralParameterField {
                            observation: expression,
                            parameter_position,
                            path,
                        },
                    ))
                } else {
                    if !is_integer(primitive_type)
                        && !matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64)
                    {
                        return None;
                    }
                    CheckedScalarExpression::StructuralParameterField {
                        observation: expression,
                        parameter_position,
                        path,
                        primitive_type,
                    }
                };
                return Some((
                    expression,
                    program.arithmetic_domain_for_type_reference(reference),
                ));
            };
            let mut segments = projection.path.into_iter();
            let path = segments
                .by_ref()
                .take(first_index)
                .map(|segment| match segment {
                    checked_trees::CheckedScalarReadProjection::Static(segment) => Some(segment),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            let checked_trees::CheckedScalarReadProjection::RuntimeIndex(index) =
                segments.next()?
            else {
                return None;
            };
            let readable_type = validation::field_domains::readable_type_reference(
                program,
                projection.selected_type,
            )?;
            let primitive_type = program.primitive_type_reference(readable_type)?;
            if !is_integer(primitive_type)
                && !matches!(
                    primitive_type,
                    PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
                )
            {
                return None;
            }
            Some((
                CheckedScalarExpression::StructuralParameterIndexedRead {
                    observation: expression,
                    root: projection.root,
                    path,
                    index: Box::new(index),
                    element_path: segments.collect(),
                    primitive_type,
                },
                program.arithmetic_domain_for_type_reference(projection.selected_type),
            ))
        }

        pub(crate) fn storage_projection(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            expression: ExpressionHandle,
            authored_parameters: &[StateParameter],
            mut selector: impl FnMut(
                ExpressionHandle,
                ExpressionHandle,
                typed_trees::types::TypeReferenceHandle,
            ) -> Option<CheckedScalarExpression>,
        ) -> Option<ScalarStorageProjection> {
            enum Projection {
                Field(ExpressionHandle),
                Index {
                    occurrence: ExpressionHandle,
                    operand: ExpressionHandle,
                },
            }
            let mut projections = Vec::new();
            let mut cursor = expression;
            let (root, path, mut element_type) = loop {
                if let Some(storage) = structural_fields::structural_storage_place(
                    program,
                    authored_parameters,
                    cursor,
                ) && !storage.1.iter().any(|segment| {
                    matches!(
                        segment,
                        CheckedStructuralPredicatePathSegment::FixedIndex(_)
                            | CheckedStructuralPredicatePathSegment::RuntimeIndex(_)
                    )
                }) {
                    break storage;
                }
                match program.expression_table.expression(cursor) {
                    ExpressionNode::Indexed(indexed) => {
                        if matches!(
                            program.expression_table.expression(indexed.index),
                            ExpressionNode::Range(_)
                        ) {
                            return None;
                        }
                        projections.push(Projection::Index {
                            occurrence: cursor,
                            operand: indexed.index,
                        });
                        cursor = indexed.collection;
                    }
                    ExpressionNode::Member(member) => {
                        projections.push(Projection::Field(cursor));
                        cursor = member.receiver;
                    }
                    _ => return None,
                }
            };
            let mut element_path = path
                .into_iter()
                .map(checked_trees::CheckedScalarReadProjection::Static)
                .collect::<Vec<_>>();
            for projection in projections.into_iter().rev() {
                match projection {
                    Projection::Field(occurrence) => {
                        let (path, selected_type) = structural_fields::member_projection(
                            program,
                            element_type,
                            program.expression_table.member_node(occurrence)?,
                        )?;
                        element_path.extend(
                            path.into_iter()
                                .map(checked_trees::CheckedScalarReadProjection::Static),
                        );
                        element_type = selected_type;
                    }
                    Projection::Index {
                        occurrence,
                        operand,
                    } => {
                        if !structural_fields::indexed_read_is_builtin(
                            program,
                            operators,
                            authored_parameters,
                            occurrence,
                            element_type,
                            operand,
                        ) {
                            return None;
                        }
                        let value = selector(occurrence, operand, element_type)?;
                        let carrier = scalar_expression_type(&value)?;
                        if !is_integer(carrier) || carrier == PrimitiveType::Addr {
                            return None;
                        }
                        element_type =
                            validation::collection_element_type_reference(program, element_type)?;
                        element_path.push(
                            checked_trees::CheckedScalarReadProjection::RuntimeIndex(
                                checked_trees::CheckedScalarSelector {
                                    expression: value,
                                    occurrence,
                                    entry_bound: None,
                                },
                            ),
                        );
                    }
                }
            }
            Some(ScalarStorageProjection {
                root,
                path: element_path,
                selected_type: element_type,
            })
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn pure_selector(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            operand: ExpressionHandle,
            parameters: &[StateParameter],
            authored_parameters: &[StateParameter],
            parameter_types: &[PrimitiveType],
            locals: &[ScalarLocal],
            exact_integer_casts: &[validation::ExactIntegerCastFact],
        ) -> Option<CheckedScalarExpression> {
            land_anonymous_scalar_expression(program, operators, operand, PrimitiveType::U64)
                .or_else(|| {
                    lower_scalar_expression(
                        program,
                        operators,
                        operand,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        exact_integer_casts,
                    )
                    .map(|(value, _)| value)
                })
        }

        /*
            `literal`'s bit-width admission against `landed_type` plus the landed
            expression it mints — both integer-literal lowering paths share the
            signed/unsigned bound test and the `Exact` domain landing.
        */
        pub(crate) fn landed_integer_literal_fits(
            literal: &numerics::literals::IntegerLiteral,
            landed_type: LandedIntegerType,
        ) -> Option<CheckedScalarExpression> {
            let fits = if landed_type.is_signed() {
                let value = literal.value_i64()?;
                let bits = landed_type.bit_width();
                let minimum = -(1_i128 << (bits - 1));
                let maximum = (1_i128 << (bits - 1)) - 1;
                let value = i128::from(value);
                minimum <= value && value <= maximum
            } else {
                let value = literal.value_u64()?;
                let bits = landed_type.bit_width();
                let maximum = if bits == 64 {
                    u64::MAX
                } else {
                    (1_u64 << bits) - 1
                };
                value <= maximum
            };
            fits.then(|| CheckedScalarExpression::IntegerLiteral {
                literal: literal.with_landing(IntegerLanding {
                    landed_type,
                    domain: ArithmeticDomain::Exact,
                }),
            })
        }
    }
    mod entry_lowering {
        /*
            entry lowering slice of scalar_lowering.
        */
        use super::{element_selection::*, integers::*, recast_reads::*};
        use crate::values::scalar::boolean_lowering::lower_boolean_expression;
        use crate::values::scalar::constant_array_projection;
        use crate::values::scalar::expression_facts::{
            is_integer, local_position, operator_is_builtin, parameter_position,
            scalar_expression_type,
        };
        use crate::values::scalar::expression_plans::ScalarLocal;
        use crate::values::scalar::primitive_reference_read;
        use crate::values::scalar::structural_fields;
        use checked_trees::{
            CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedScalarExpression,
        };
        use facts::PlaceRoot;
        use numerics::arithmetic::ArithmeticDomain;
        use numerics::literals::FloatFormat;
        use symbols::SymbolHandle;
        use typed_trees::expression::{
            ExpressionHandle, ExpressionNode, TableBinaryExpression, UnaryOperator,
            ieee_float_value,
        };
        use typed_trees::operator::integer_peer_operand_carrier;
        use typed_trees::signature::StateParameter;
        use typed_trees::state::State;
        use typed_trees::types::PrimitiveType;
        use validation::CanonicalPlace;
        use validation::ExactIntegerCastFact;
        use validation::closed_record_scalar_projection;
        use validation::has_anonymous_operator_meaning;
        use validation::land_anonymous_integer_expression;

        /*
            Lower one call argument in the caller state's checked scalar namespace.
            Only the immutable scalar-prefix shape accepted by terminal scalar lowering
            is represented; any wider state shape stays explicit as `None` so crash
            refinement cannot claim a portable predicate it cannot later materialize.
        */
        pub(crate) fn lower_state_scalar_expression(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            state: &State,
            before_statement: usize,
            expression: ExpressionHandle,
            expected_type: PrimitiveType,
            exact_integer_casts: &[ExactIntegerCastFact],
        ) -> Option<CheckedScalarExpression> {
            let parameters = program.state_parameters(state);
            let parameter_types = parameters
                .iter()
                .map(|parameter| program.primitive_type_reference(parameter.type_reference))
                .collect::<Option<Vec<_>>>()?;
            let statements = program.statement_table.statements(state.statement_nodes);
            let prefix = statements.get(..before_statement)?;
            let mut locals = Vec::new();
            for statement in prefix {
                let local = statement.as_local_data()?;
                if local.is_mutable
                    || (!local.initial_value.is_valid()
                        && crate::values::zeroed_scalar_local_initializer(program, local).is_none())
                {
                    return None;
                }
                let primitive_type = program.primitive_type_reference(local.type_reference)?;
                locals.push(ScalarLocal::from_local_data(program, local, primitive_type));
            }
            lower_return_expression(
                program,
                operators,
                expression,
                parameters,
                program.state_parameters(state),
                &parameter_types,
                &locals,
                expected_type,
                exact_integer_casts,
            )
        }

        /*
            Lower one scalar argument inside a structural/Unit state. Structural
            parameters retain their separate custody namespace; only primitive
            parameters and earlier immutable primitive locals occupy scalar positions.
        */
        pub(crate) fn lower_unit_scalar_argument(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            state: &State,
            before_statement: usize,
            expression: ExpressionHandle,
            expected_type: PrimitiveType,
        ) -> Option<CheckedScalarExpression> {
            let (parameters, parameter_types, locals) =
                unit_scalar_context(program, state, before_statement)?;

            lower_return_expression(
                program,
                operators,
                expression,
                &parameters,
                program.state_parameters(state),
                &parameter_types,
                &locals,
                expected_type,
                &operators.exact_integer_casts,
            )
        }

        /*
            Lower the ordinary scalar algebra, then bind each storage observation in
            that exact state namespace. The binder supplies its own custody policy;
            local reads stay reads of the local value rather than its initializer.
        */
        pub(crate) fn lower_unit_scalar_argument_for_places(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            state: &State,
            before_statement: usize,
            expression: ExpressionHandle,
            expected_type: PrimitiveType,
            mut bind: impl FnMut(&CanonicalPlace, PrimitiveType) -> Option<CheckedScalarExpression>,
        ) -> Option<CheckedScalarExpression> {
            let mut value = lower_unit_scalar_argument(
                program,
                operators,
                state,
                before_statement,
                expression,
                expected_type,
            )?;
            let symbols = unit_scalar_binding_symbols(program, state, before_statement)?;
            crate::published::facts::boolean_predicates::map_scalar(&mut value, &mut |operand| {
                let primitive = operand.primitive_type()?;
                let place = match operand.scalar()? {
                    CheckedScalarExpression::Parameter { position, .. }
                    | CheckedScalarExpression::Local { position, .. } => {
                        crate::flow::canonical_place_from_symbol(*symbols.get(*position)?)?
                    }
                    CheckedScalarExpression::StorageRead { symbol, .. } => {
                        crate::flow::canonical_place_from_symbol(*symbol)?
                    }
                    CheckedScalarExpression::StructuralParameterField {
                        parameter_position,
                        path,
                        ..
                    } => {
                        let (root, segments, _, _) =
                            crate::values::resolve_structural_parameter_path(
                                program,
                                program.state_parameters(state),
                                *parameter_position,
                                path,
                            )?;
                        CanonicalPlace {
                            root: PlaceRoot::Symbol(root),
                            segments,
                        }
                    }
                    _ => return None,
                };
                let bound = bind(&place, primitive)?;
                if bound.primitive_type()? != primitive {
                    return None;
                }
                *operand = bound.into();
                Some(())
            })?;
            Some(value)
        }

        pub(crate) fn unit_scalar_context(
            program: &dyn ::validation::CheckingInput,
            state: &State,
            before_statement: usize,
        ) -> Option<(Vec<StateParameter>, Vec<PrimitiveType>, Vec<ScalarLocal>)> {
            let (parameters, parameter_types) = crate::values::scalar::scalar_parameter_roster(
                program,
                program.state_parameters(state),
            )?;
            let statements = program.statement_table.statements(state.statement_nodes);
            let prefix = statements.get(..before_statement)?;
            let mut locals = Vec::new();
            for statement in prefix {
                let Some(local) = statement.as_local_data() else {
                    continue;
                };
                let Some(primitive_type) = program.primitive_type_reference(local.type_reference)
                else {
                    if let Some(binding) = primitive_reference_read::local(program, local) {
                        locals.push(binding);
                    }
                    continue;
                };
                if !local.initial_value.is_valid()
                    && crate::values::zeroed_scalar_local_initializer(program, local).is_none()
                {
                    return None;
                }
                locals.push(ScalarLocal::from_local_data(program, local, primitive_type));
            }
            Some((parameters, parameter_types, locals))
        }

        /*
            The exact binding namespace used by `lower_unit_scalar_argument`. Stored
            local values retain their own symbols rather than replaying initializers.
        */
        pub(crate) fn unit_scalar_binding_symbols(
            program: &dyn ::validation::CheckingInput,
            state: &State,
            before_statement: usize,
        ) -> Option<Vec<SymbolHandle>> {
            let (parameters, _, locals) = unit_scalar_context(program, state, before_statement)?;
            Some(
                parameters
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .chain(
                        locals
                            .iter()
                            .filter(|local| local.is_value_binding())
                            .map(|local| local.symbol),
                    )
                    .collect(),
            )
        }

        /*
            Indexing consumes an integer value, not an expected u64 expression. Keep
            typed arithmetic in its authored carrier; only wholly anonymous arithmetic
            lands directly in the general count carrier. Terminal emission performs the
            exact coordinate conversion after evaluating this expression.
        */
        pub(crate) fn lower_index_expression(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            expression: ExpressionHandle,
            parameters: &[StateParameter],
            authored_parameters: &[StateParameter],
            parameter_types: &[PrimitiveType],
            locals: &[ScalarLocal],
            exact_integer_casts: &[ExactIntegerCastFact],
        ) -> Option<CheckedScalarExpression> {
            if let Some(value) =
                land_anonymous_scalar_expression(program, operators, expression, PrimitiveType::U64)
            {
                return Some(value);
            }
            let (expression, _) = lower_scalar_expression(
                program,
                operators,
                expression,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?;
            scalar_expression_type(&expression)
                .is_some_and(|primitive| primitive != PrimitiveType::Addr && is_integer(primitive))
                .then_some(expression)
        }

        pub(crate) fn lower_return_expression(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            expression: ExpressionHandle,
            parameters: &[StateParameter],
            authored_parameters: &[StateParameter],
            parameter_types: &[PrimitiveType],
            locals: &[ScalarLocal],
            result_type: PrimitiveType,
            exact_integer_casts: &[ExactIntegerCastFact],
        ) -> Option<CheckedScalarExpression> {
            if let Some(value) =
                land_anonymous_scalar_expression(program, operators, expression, result_type)
            {
                return Some(value);
            }
            if result_type == PrimitiveType::Bool
                && let Some(expression) = lower_boolean_expression(
                    program,
                    operators,
                    expression,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )
            {
                return Some(CheckedScalarExpression::Boolean(Box::new(expression)));
            }
            let (expression, _) = lower_scalar_expression(
                program,
                operators,
                expression,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?;
            match scalar_expression_type(&expression) {
                Some(actual_type) => (actual_type == result_type).then_some(expression),
                None => land_contextual_integer_literal(expression, result_type),
            }
        }

        pub(crate) fn land_anonymous_scalar_expression(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            expression: ExpressionHandle,
            destination: PrimitiveType,
        ) -> Option<CheckedScalarExpression> {
            if let ExpressionNode::Float(literal) = program.expression_table.expression(expression)
                && literal.landing().is_none()
                && matches!(destination, PrimitiveType::F32 | PrimitiveType::F64)
            {
                let format = match destination {
                    PrimitiveType::F32 => FloatFormat::F32,
                    PrimitiveType::F64 => FloatFormat::F64,
                    _ => return None,
                };
                return Some(CheckedScalarExpression::IeeeFloatLiteral {
                    value: ieee_float_value(literal, format),
                });
            }
            land_anonymous_integer_expression(program, expression, destination, &[], |expression| {
                match operators.expression_use(expression) {
                    Some(operator) => {
                        operator.status == CheckedOperatorResolutionStatus::BuiltinFallback
                    }
                    None => has_anonymous_operator_meaning(program, expression),
                }
            })
            .map(|(literal, _)| CheckedScalarExpression::IntegerLiteral { literal })
        }

        pub(crate) fn lower_scalar_operands(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            binary: &TableBinaryExpression,
            parameters: &[StateParameter],
            authored_parameters: &[StateParameter],
            parameter_types: &[PrimitiveType],
            locals: &[ScalarLocal],
            exact_integer_casts: &[ExactIntegerCastFact],
        ) -> Option<(
            (CheckedScalarExpression, ArithmeticDomain),
            (CheckedScalarExpression, ArithmeticDomain),
        )> {
            let lower = |expression| {
                lower_scalar_expression(
                    program,
                    operators,
                    expression,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    exact_integer_casts,
                )
            };
            let mut left = lower(binary.left);
            let mut right = lower(binary.right);
            if let Some(destination) = right
                .as_ref()
                .and_then(|(value, _)| scalar_expression_type(value))
                && let Some(value) = land_anonymous_scalar_expression(
                    program,
                    operators,
                    binary.left,
                    integer_peer_operand_carrier(binary.operator, destination, false),
                )
            {
                left = Some((value, ArithmeticDomain::Exact));
            }
            if let Some(destination) = left
                .as_ref()
                .and_then(|(value, _)| scalar_expression_type(value))
                && let Some(value) = land_anonymous_scalar_expression(
                    program,
                    operators,
                    binary.right,
                    integer_peer_operand_carrier(binary.operator, destination, true),
                )
            {
                right = Some((value, ArithmeticDomain::Exact));
            }
            Some((left?, right?))
        }

        pub(crate) fn lower_scalar_expression(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            expression: ExpressionHandle,
            parameters: &[StateParameter],
            authored_parameters: &[StateParameter],
            parameter_types: &[PrimitiveType],
            locals: &[ScalarLocal],
            exact_integer_casts: &[ExactIntegerCastFact],
        ) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
            macro_rules! lower_child {
                ($expression:expr $(,)?) => {
                    lower_scalar_expression(
                        program,
                        operators,
                        $expression,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        exact_integer_casts,
                    )
                };
            }
            if let Some((source, target_type)) = recast_view_operand(program, expression) {
                let primitive_type = program.primitive_type_reference(target_type)?;
                let element = program.expression_table.indexed_node(source).and_then(|_| {
                    scalar_storage_read(
                        program,
                        operators,
                        source,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        exact_integer_casts,
                    )
                });
                let (read, _) = match element {
                    Some(element) => element,
                    None => lower_child!(source)?,
                };
                return Some((
                    view_place_read(read, primitive_type)?,
                    program.arithmetic_domain_for_type_reference(target_type),
                ));
            }
            if let ExpressionNode::Float(literal) = program.expression_table.expression(expression)
            {
                let value = ieee_float_value(literal, literal.landing()?);
                return Some((
                    CheckedScalarExpression::IeeeFloatLiteral { value },
                    ArithmeticDomain::Exact,
                ));
            }
            if let Some(read) =
                primitive_reference_read::declared(program, authored_parameters, expression)
            {
                return Some(read);
            }
            if let Some((leaf, primitive)) = closed_record_scalar_projection(program, expression) {
                let lowered = if is_integer(primitive) {
                    let literal = program.expression_table.integer_node(leaf)?;
                    CheckedScalarExpression::IntegerLiteral {
                        literal: if literal.landing().is_some() {
                            literal.clone()
                        } else {
                            land_anonymous_integer_expression(
                                program,
                                leaf,
                                primitive,
                                &[],
                                |_| false,
                            )?
                            .0
                        },
                    }
                } else {
                    lower_child!(leaf)?.0
                };
                return (lowered.primitive_type() == Some(primitive))
                    .then_some((lowered, ArithmeticDomain::Exact));
            }
            if let Some(leaf) = constant_array_projection::selected_leaf(
                program,
                operators,
                authored_parameters,
                expression,
            ) {
                return lower_child!(leaf);
            }
            if let Some(length) = structural_fields::structural_sequence_length(
                program,
                operators,
                authored_parameters,
                expression,
                |_, operand, _| {
                    super::element_selection::pure_selector(
                        program,
                        operators,
                        operand,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        exact_integer_casts,
                    )
                },
            ) {
                return Some((length, ArithmeticDomain::Exact));
            }
            if let Some(length) = exact_inline_literal_subslice_length(program, expression) {
                return Some((length, ArithmeticDomain::Exact));
            }
            if let Some(read) = scalar_storage_read(
                program,
                operators,
                expression,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            ) {
                return Some(read);
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(path) => {
                    if let Some(position) = parameter_position(program, path, parameters) {
                        let parameter = &parameters[position];
                        if parameter.is_mutable {
                            let primitive_type =
                                crate::values::mutable_scalar_parameter_type(program, parameter)?;
                            if path.symbol != parameter.symbol
                                || path.head_symbol != parameter.symbol
                            {
                                return None;
                            }
                            return Some((
                                CheckedScalarExpression::StorageRead {
                                    symbol: parameter.symbol,
                                    primitive_type,
                                },
                                program
                                    .arithmetic_domain_for_type_reference(parameter.type_reference),
                            ));
                        }
                        return Some((
                            CheckedScalarExpression::Parameter {
                                position,
                                primitive_type: parameter_types[position],
                            },
                            program.arithmetic_domain_for_type_reference(
                                parameters[position].type_reference,
                            ),
                        ));
                    }
                    if let Some(authored) = parameter_position(program, path, authored_parameters) {
                        let parameter = &authored_parameters[authored];
                        if parameter.relevance.is_erased()
                            && let Some(primitive_type) =
                                program.erased_scalar_observation_type(parameter.type_reference)
                        {
                            return primitive_reference_read::erased_observation(
                                program,
                                authored_parameters,
                                parameter.symbol,
                                primitive_type,
                            );
                        }
                    }
                    let local_position = local_position(program, expression, path, locals)?;
                    let local = &locals[local_position];
                    if local.is_mutable || local.is_reference {
                        return Some((
                            CheckedScalarExpression::StorageRead {
                                symbol: local.symbol,
                                primitive_type: local.primitive_type,
                            },
                            local.arithmetic_domain,
                        ));
                    }
                    let position = parameters.len().checked_add(
                        locals[..local_position]
                            .iter()
                            .filter(|local| local.is_value_binding())
                            .count(),
                    )?;
                    Some((
                        CheckedScalarExpression::Local {
                            position,
                            primitive_type: locals[local_position].primitive_type,
                        },
                        locals[local_position].arithmetic_domain,
                    ))
                }
                ExpressionNode::Integer(literal) => Some((
                    CheckedScalarExpression::IntegerLiteral {
                        literal: literal.clone(),
                    },
                    literal
                        .landing()
                        .map(|landing| landing.domain)
                        .unwrap_or(ArithmeticDomain::Exact),
                )),
                ExpressionNode::Cast(cast)
                    if !cast.form.is_recast() && cast.semantic_domain.is_empty() =>
                {
                    let target_type = program.primitive_type_reference(cast.target_type)?;
                    if target_type == PrimitiveType::Selector {
                        let name = program.expression_table.name_path_node(cast.value)?;
                        return Some((
                            CheckedScalarExpression::ImplementationSelection {
                                implementation: name.symbol,
                                interface_identity: program
                                    .normalized_type_identity(cast.target_type),
                            },
                            ArithmeticDomain::Exact,
                        ));
                    }
                    if matches!(target_type, PrimitiveType::F32 | PrimitiveType::F64) {
                        let (operand, _) = lower_child!(cast.value)?;
                        return (scalar_expression_type(&operand) == Some(target_type))
                            .then_some((operand, cast.domain));
                    }
                    if !is_integer(target_type) {
                        return None;
                    }
                    let operand = land_anonymous_scalar_expression(
                        program,
                        operators,
                        cast.value,
                        target_type,
                    )
                    .or_else(|| lower_child!(cast.value).map(|(value, _)| value))?;
                    construct_integer_cast(program, expression, operand, exact_integer_casts)
                }
                ExpressionNode::Unary(unary)
                    if unary.operator == UnaryOperator::BitwiseNot
                        && operator_is_builtin(operators, expression) =>
                {
                    let (operand, domain) = lower_child!(unary.operand)?;
                    construct_integer_bitwise_not(operand, domain)
                }
                ExpressionNode::Binary(binary) if operator_is_builtin(operators, expression) => {
                    let ((left, left_domain), (right, right_domain)) = lower_scalar_operands(
                        program,
                        operators,
                        binary,
                        parameters,
                        authored_parameters,
                        parameter_types,
                        locals,
                        exact_integer_casts,
                    )?;
                    construct_integer_binary(
                        binary.operator,
                        left,
                        left_domain,
                        right,
                        right_domain,
                        operators.formation_is_decided(expression),
                        operators.is_trap_free(expression),
                    )
                }
                _ => None,
            }
        }
    }
    mod integers {
        /*
            integers slice of scalar_lowering.
        */
        use super::element_selection::*;
        use super::integer_widen_is_total;
        use crate::flow::expressions;
        use crate::values::scalar::expression_facts::{
            checked_integer_binary_kind, combine_arithmetic_domains, is_integer,
            scalar_expression_type,
        };
        use checked_trees::{CheckedIntegerRange, CheckedScalarExpression};
        use numerics::arithmetic::ArithmeticDomain;
        use numerics::literals::{IntegerLanding, LandedIntegerType};
        use typed_trees::expression::{BinaryOperator, ExpressionHandle};
        use typed_trees::operator::{builtin_integer_result_carrier, integer_peer_operand_carrier};
        use typed_trees::types::PrimitiveType;

        /*
            Construct one selected builtin operation from operands whose evaluation
            order has already been retained by either the pure tree or computation plan.
        */
        pub(crate) fn construct_integer_binary(
            operator: BinaryOperator,
            mut left: CheckedScalarExpression,
            left_domain: ArithmeticDomain,
            mut right: CheckedScalarExpression,
            right_domain: ArithmeticDomain,
            formation_decided: bool,
            trap_free: bool,
        ) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
            let shift = matches!(
                operator,
                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
            );
            let domain = if shift {
                left_domain
            } else {
                combine_arithmetic_domains(left_domain, right_domain)?
            };
            let kind = checked_integer_binary_kind(operator, domain)?;
            let mut left_type = scalar_expression_type(&left);
            let mut right_type = scalar_expression_type(&right);
            if operator == BinaryOperator::Subtract && left_type.is_none() {
                left = land_anonymous_zero(left, right_type?)?;
                left_type = scalar_expression_type(&left);
            }
            if left_type.is_none() {
                left = land_contextual_integer_literal(
                    left,
                    integer_peer_operand_carrier(operator, right_type?, false),
                )?;
                left_type = scalar_expression_type(&left);
            }
            if right_type.is_none() {
                right = land_contextual_integer_literal(
                    right,
                    integer_peer_operand_carrier(operator, left_type?, true),
                )?;
                right_type = scalar_expression_type(&right);
            }
            let left_type = left_type?;
            let right_type = right_type?;
            let primitive_type = builtin_integer_result_carrier(operator, [left_type, right_type])?;
            if (left_type == PrimitiveType::Addr || right_type == PrimitiveType::Addr)
                && domain != ArithmeticDomain::Exact
            {
                return None;
            }
            Some((
                CheckedScalarExpression::IntegerBinary {
                    kind,
                    primitive_type,
                    left: Box::new(left),
                    right: Box::new(right),
                    formation_decided,
                    trap_free,
                },
                domain,
            ))
        }

        pub(crate) fn construct_integer_bitwise_not(
            operand: CheckedScalarExpression,
            domain: ArithmeticDomain,
        ) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
            let primitive_type = scalar_expression_type(&operand)?;
            is_integer(primitive_type).then_some((
                CheckedScalarExpression::IntegerBitwiseNot {
                    primitive_type,
                    operand: Box::new(operand),
                },
                domain,
            ))
        }

        /*
            Retain the exact carrier conversion at its original source occurrence,
            independently of the policy qualifying later operations on the result.
            Completing the operand through a computation plan preserves that proof.
        */
        pub(crate) fn construct_integer_cast(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            operand: CheckedScalarExpression,
            exact_integer_casts: &[validation::ExactIntegerCastFact],
        ) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
            let cast = program.expression_table.cast_node(expression)?;
            if cast.form.is_recast() || !cast.semantic_domain.is_empty() {
                return None;
            }
            let target_type = program.primitive_type_reference(cast.target_type)?;
            if !is_integer(target_type) {
                return None;
            }
            let source_type = scalar_expression_type(&operand);
            if source_type.is_none() {
                return land_contextual_integer_literal(operand, target_type)
                    .map(|literal| (literal, cast.domain));
            }
            let source_type = source_type?;
            if source_type == target_type {
                return Some((operand, cast.domain));
            }
            if source_type != PrimitiveType::Addr
                && target_type != PrimitiveType::Addr
                && let Some(literal) = retag_exact_integer_literal(&operand, target_type)
            {
                return Some((literal, cast.domain));
            }
            if integer_widen_is_total(source_type, target_type) {
                return Some((
                    CheckedScalarExpression::IntegerWiden {
                        primitive_type: target_type,
                        operand: Box::new(operand),
                    },
                    cast.domain,
                ));
            }
            if let Some(fact) = exact_integer_casts
                .iter()
                .find(|fact| fact.expression == expression)
                && fact.source_type == source_type
                && fact.target_type == target_type
            {
                return Some((
                    CheckedScalarExpression::IntegerExactCast {
                        primitive_type: target_type,
                        operand: Box::new(operand),
                        range: CheckedIntegerRange {
                            minimum: fact.minimum.clone(),
                            maximum: fact.maximum.clone(),
                        },
                        formation_decided: true,
                    },
                    cast.domain,
                ));
            }
            None
        }

        /*
            Reclose the one source-level slice view whose length is already fixed by
            two literal bounds. This is a value fact, not a backend fold: retaining the
            landed `u64` here lets later checked control replay the exact initializer
            without consulting a target representation.
        */
        pub(crate) fn exact_inline_literal_subslice_length(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<CheckedScalarExpression> {
            let receiver = expressions(program).authored_length_receiver(expression)?;
            let indexed = program.expression_table.indexed_node(receiver)?;
            let range = program.expression_table.range_node(indexed.index)?;
            if !range.start.is_valid() || !range.end.is_valid() {
                return None;
            }
            let start = program.expression_table.integer_node(range.start)?;
            let end = program.expression_table.integer_node(range.end)?;
            let length = end
                .value_u64()?
                .checked_add(u64::from(range.end_inclusive))?
                .checked_sub(start.value_u64()?)?;
            let length = i64::try_from(length).ok()?;
            Some(CheckedScalarExpression::IntegerLiteral {
                literal: numerics::literals::IntegerLiteral::from_value(length).with_landing(
                    IntegerLanding {
                        landed_type: LandedIntegerType::U64,
                        domain: ArithmeticDomain::Exact,
                    },
                ),
            })
        }

        pub(crate) fn land_anonymous_zero(
            expression: CheckedScalarExpression,
            primitive_type: PrimitiveType,
        ) -> Option<CheckedScalarExpression> {
            let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
                return None;
            };
            if literal.landing().is_some() || literal.value_i64() != Some(0) {
                return None;
            }
            let landed_type = primitive_type.landed_landing()?;
            Some(CheckedScalarExpression::IntegerLiteral {
                literal: literal.with_landing(IntegerLanding {
                    landed_type,
                    domain: ArithmeticDomain::Exact,
                }),
            })
        }

        pub(crate) fn retag_exact_integer_literal(
            expression: &CheckedScalarExpression,
            primitive_type: PrimitiveType,
        ) -> Option<CheckedScalarExpression> {
            let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
                return None;
            };
            let landed_type = landed_for_primitive(primitive_type)?;
            landed_integer_literal_fits(literal, landed_type)
        }

        pub(crate) fn land_contextual_integer_literal(
            expression: CheckedScalarExpression,
            primitive_type: PrimitiveType,
        ) -> Option<CheckedScalarExpression> {
            let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
                return None;
            };
            if literal.landing().is_some() {
                return None;
            }
            let landed_type = match primitive_type {
                PrimitiveType::Addr => LandedIntegerType::Addr,
                primitive_type => landed_for_primitive(primitive_type)?,
            };
            landed_integer_literal_fits(&literal, landed_type)
        }

        pub(crate) fn landed_for_primitive(
            primitive_type: PrimitiveType,
        ) -> Option<LandedIntegerType> {
            primitive_type.landed_landing_fixed()
        }
    }
    mod recast_reads {
        /*
            recast reads slice of scalar_lowering.
        */
        use super::entry_lowering::*;
        use crate::flow::expressions;
        use crate::values::scalar::expression_plans::ScalarLocal;
        use checked_trees::{
            CheckedBooleanExpression as BooleanExpression, CheckedOperatorFacts,
            CheckedScalarExpression as ScalarExpression, CheckedStructuralPredicatePathSegment,
        };
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::signature::StateParameter;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

        /*
            Peel the authored `&`/`&mut` spellings around one recast view operand,
            returning the whole-place source and the cast's stated target type.
            `&x as &T` parses as `Borrow { Cast }` -- the unary `&` wraps the postfix
            chain with the authored access -- while parentheses can leave the borrow
            around the cast's SOURCE (`&(x) as &T`). Sources are name, member and
            indexed places; `view_place_read` admits an indexed source only as a whole
            element or as the first byte of a `u8` region, and deeper view chains have
            no scalar place read to re-view.
        */
        pub(crate) fn recast_view_operand(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<(ExpressionHandle, TypeReferenceHandle)> {
            let mut expression = expression;
            expression = expressions(program).peeled_expression_handle(expression);
            let cast = program.expression_table.cast_node(expression)?;
            if !cast.form.is_recast() {
                return None;
            }
            let mut source = cast.value;
            source = program.expression_table.peeled_expression_handle(source);
            matches!(
                program.expression_table.expression(source),
                ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
            )
            .then_some((source, cast.target_type))
        }

        /*
            One whole scalar place's storage site, recovered from its checked read.
            A recast view re-reads the same site through its target carrier.
        */
        pub(crate) enum PlaceReadSite {
            Storage(symbols::SymbolHandle),
            Parameter(usize),
            Local(usize),
            StructuralField {
                observation: ExpressionHandle,
                parameter_position: u32,
                path: Vec<CheckedStructuralPredicatePathSegment>,
            },
        }

        /*
            Recover the storage site of one checked read when it is a whole scalar
            place. Boolean place leaves unwrap to their site as well; any composed
            value (operations, literals, indexed reads) is not a place.
        */
        pub(crate) fn place_read_site(read: &ScalarExpression) -> Option<PlaceReadSite> {
            match read {
                ScalarExpression::StorageRead { symbol, .. } => {
                    Some(PlaceReadSite::Storage(*symbol))
                }
                ScalarExpression::Parameter { position, .. } => {
                    Some(PlaceReadSite::Parameter(*position))
                }
                ScalarExpression::Local { position, .. } => Some(PlaceReadSite::Local(*position)),
                ScalarExpression::StructuralParameterField {
                    observation,
                    parameter_position,
                    path,
                    ..
                } => Some(PlaceReadSite::StructuralField {
                    observation: *observation,
                    parameter_position: *parameter_position,
                    path: path.clone(),
                }),
                ScalarExpression::Boolean(inner) => match inner.as_ref() {
                    BooleanExpression::StorageRead { symbol } => {
                        Some(PlaceReadSite::Storage(*symbol))
                    }
                    BooleanExpression::Parameter { position } => {
                        Some(PlaceReadSite::Parameter(*position))
                    }
                    BooleanExpression::Local { position } => Some(PlaceReadSite::Local(*position)),
                    BooleanExpression::StructuralParameterField {
                        observation,
                        parameter_position,
                        path,
                        ..
                    } => Some(PlaceReadSite::StructuralField {
                        observation: *observation,
                        parameter_position: *parameter_position,
                        path: path.clone(),
                    }),
                    _ => None,
                },
                _ => None,
            }
        }

        /*
            The checked read of one whole place re-viewed as `primitive_type`. A
            same-carrier view changes facts, not representation, and is the place's
            own read. A carrier change retains the read at its declared carrier inside
            `RepresentationView`, so later stages replay the carrier relation instead
            of meeting a field read whose carrier its declaration does not name. A
            Boolean view keeps the retagged Boolean leaf its guard positions consume;
            its native replay is still owed a Boolean re-view form.

            A runtime-selected `u8` element is the first byte of a byte-region view
            (`&self.buf[offset] as &u32`): the recast judgment proved the whole
            footprint in bounds, and the retained view keeps that one authored element
            read so Psi 06 can replay the region byte by byte, each generated byte
            carrying the proven footprint bound as its selector decision. Any other
            width change is not a whole-place view and stays unlowered.
        */
        pub(crate) fn view_place_read(
            read: ScalarExpression,
            primitive_type: PrimitiveType,
        ) -> Option<ScalarExpression> {
            let byte_region = matches!(
                &read,
                ScalarExpression::StructuralParameterIndexedRead {
                    element_path,
                    primitive_type: PrimitiveType::U8,
                    ..
                } if element_path.is_empty()
            );
            let indexed_element = matches!(
                read,
                ScalarExpression::StructuralParameterIndexedRead { .. }
            );
            if let ScalarExpression::RepresentationView {
                primitive_type: view_type,
                ..
            } = &read
                && *view_type == primitive_type
            {
                return Some(read);
            }
            if primitive_type == PrimitiveType::Bool
                && let ScalarExpression::Boolean(_) = read
            {
                return Some(read);
            }
            if !indexed_element {
                place_read_site(&read)?;
            }
            let source = read.primitive_type()?;
            if source == primitive_type {
                return Some(read);
            }
            let region = byte_region
                && primitive_type != PrimitiveType::Bool
                && scalar_byte_width(primitive_type).is_some();
            if !region && scalar_byte_width(source)? != scalar_byte_width(primitive_type)? {
                return None;
            }
            Some(ScalarExpression::RepresentationView {
                primitive_type,
                operand: Box::new(read),
            })
        }

        /* The storage width of one primitive scalar carrier, in bytes. */
        pub(crate) fn scalar_byte_width(primitive_type: PrimitiveType) -> Option<u32> {
            Some(match primitive_type {
                PrimitiveType::Bool | PrimitiveType::U8 | PrimitiveType::I8 => 1,
                PrimitiveType::U16 | PrimitiveType::I16 => 2,
                PrimitiveType::U32 | PrimitiveType::I32 | PrimitiveType::F32 => 4,
                PrimitiveType::U64 | PrimitiveType::I64 | PrimitiveType::F64 => 8,
                PrimitiveType::Addr | PrimitiveType::Selector => return None,
            })
        }

        /*
            Re-read one place site through the bool carrier -- the checked form of
            `&place as &bool` in a Boolean position.
        */
        pub(crate) fn place_read_site_as_boolean(site: PlaceReadSite) -> BooleanExpression {
            match site {
                PlaceReadSite::Storage(symbol) => BooleanExpression::StorageRead { symbol },
                PlaceReadSite::Parameter(position) => BooleanExpression::Parameter { position },
                PlaceReadSite::Local(position) => BooleanExpression::Local { position },
                PlaceReadSite::StructuralField {
                    observation,
                    parameter_position,
                    path,
                } => BooleanExpression::StructuralParameterField {
                    observation,
                    parameter_position,
                    path,
                },
            }
        }

        /*
            A Boolean place keeps its ordinary Boolean read. A different source
            carrier remains a scalar representation view so its landing obligation
            reaches Terminal Psi through the ordinary scalar computation path.
        */
        pub(crate) fn lower_recast_boolean_operand(
            program: &dyn ::validation::CheckingInput,
            operators: &CheckedOperatorFacts,
            expression: ExpressionHandle,
            parameters: &[StateParameter],
            authored_parameters: &[StateParameter],
            parameter_types: &[PrimitiveType],
            locals: &[ScalarLocal],
            exact_integer_casts: &[validation::ExactIntegerCastFact],
        ) -> Option<BooleanExpression> {
            let (source, target_type) = recast_view_operand(program, expression)?;
            if program.primitive_type_reference(target_type) != Some(PrimitiveType::Bool) {
                return None;
            }
            let (read, _) = lower_scalar_expression(
                program,
                operators,
                source,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                exact_integer_casts,
            )?;
            if read.primitive_type()? != PrimitiveType::Bool {
                return None;
            }
            Some(place_read_site_as_boolean(place_read_site(&read)?))
        }
    }
}
pub(crate) mod selected_operator_operands {
    /*
        Source-bound scalar operands of a selected boundary-operator application.

        A `let` whose whole initializer applies a boundary operator becomes a call
        to the selected realization once providers settle, and Unit planning then
        passes each scalar operand as a call argument. An ordinary call argument
        reaches that plan through a values-stage binding
        (`CheckedScalarExpressionRole::UnitCallArgument`) that Checked-to-Lowered
        Psi replays against the authored argument. These rows give the operator's
        operands the same custody under `SelectedOperatorOperand`, keyed by the
        authored operand position. They are produced here, before settlement
        rewrites the operator node, from the operator facts that already name the
        selected boundary operator and its closed application; only which
        realization runs remains open until settlement.
    */
    use crate::values::scalar::expression_plans::ScalarLocal;
    use crate::values::scalar::scalar_lowering::lower_return_expression;
    use checked_trees::{
        CheckedLocatedScalarExpression, CheckedOperatorFacts, CheckedScalarExpressionRole,
        CheckedValueOrigin, CheckedValueStatementRole,
    };
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::StateParameter;
    use typed_trees::types::PrimitiveType;

    /*
        One lowered scalar expression per scalar operand, beside the authored
        operand it was lowered from. Structural operands and an initializer that
        applies no selected boundary operator yield nothing.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_selected_operator_operands(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_ordinal: u32,
        local: &typed_trees::statement::TableLocalData,
        parameters: &[StateParameter],
        authored_parameters: &[StateParameter],
        parameter_types: &[PrimitiveType],
        locals: &[ScalarLocal],
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> Vec<(ExpressionHandle, CheckedLocatedScalarExpression)> {
        if local.is_mutable {
            return Vec::new();
        }
        let Ok(statement_index) = usize::try_from(statement_ordinal) else {
            return Vec::new();
        };
        let origin = CheckedValueOrigin::StateStatement {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index,
            role: CheckedValueStatementRole::LocalInitializer,
        };
        let Some(application) =
            operators.boundary_application_operands(program, local.initial_value, origin)
        else {
            return Vec::new();
        };
        application
            .operands
            .iter()
            .enumerate()
            .filter_map(|(position, (operand, primitive_type))| {
                let operand_ordinal = u32::try_from(position).ok()?;
                let expression = lower_return_expression(
                    program,
                    operators,
                    *operand,
                    parameters,
                    authored_parameters,
                    parameter_types,
                    locals,
                    (*primitive_type)?,
                    exact_integer_casts,
                )?;
                Some((
                    *operand,
                    CheckedLocatedScalarExpression {
                        state: state.symbol,
                        statement_ordinal,
                        role: CheckedScalarExpressionRole::SelectedOperatorOperand {
                            operand_ordinal,
                        },
                        expression,
                    },
                ))
            })
            .collect()
    }
}
pub(crate) mod semantic_casts {
    /*
        Semantic cast custody is independent of the pure scalar payload grammar.
    */
    use crate::published::facts::domain_is_vacuous;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::types::TypeReferenceHandle;
    use typed_trees::types::TypeReferenceNode;
    use typed_trees::types::{DomainConstraintSubject, TypeConstraintNode};

    pub(crate) fn result_type(
        program: &dyn ::validation::CheckingInput,
        state_symbol: symbols::SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        validation::find_state(program, state_symbol).and_then(|(machine, state)| {
            validation::expression_result_type_reference(program, machine, state, expression)
        })
    }

    /*
        Whether `reference` carries a declared domain that is a semantic atom. A
        domain whose membership is exactly an integer interval is not one: like a
        bracketed range it only bounds the value, so reading or casting it needs
        no qualification custody.
    */
    pub(crate) fn has_declared_domains(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| {
                        matches!(constraint, TypeConstraintNode::Domain(domain)
                        if domain.subject == DomainConstraintSubject::Declared
                            && !is_interval_domain(program, reference, domain))
                    })
                    || has_declared_domains(program, *base_type)
            }
            _ => false,
        }
    }

    fn is_interval_domain(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
        domain: &typed_trees::types::DomainConstraint,
    ) -> bool {
        program
            .primitive_type_reference(reference)
            .and_then(|primitive| {
                validation::exact_declared_domain_carrier_interval(program, primitive, domain)
                    .filter(|(minimum, maximum)| minimum <= maximum)
            })
            .is_some()
    }

    pub(crate) fn has_only_vacuous_tags(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { .. } => {
                program.primitive_type_reference(reference).is_some()
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                has_only_vacuous_tags(program, *base_type)
                    && program
                        .type_reference_table
                        .constraints(*constraints)
                        .iter()
                        .all(|constraint| match constraint {
                            TypeConstraintNode::Domain(domain)
                                if is_interval_domain(program, reference, domain) =>
                            {
                                true
                            }
                            TypeConstraintNode::Domain(domain) => {
                                domain.subject == DomainConstraintSubject::Declared
                                    && domain.semantic_id.is_valid()
                                    && !domain.predicate_presence.is_present()
                                    && domain.establishment_routes.is_empty()
                                    && domain_is_vacuous(program, domain.symbol, &mut Vec::new())
                            }
                            TypeConstraintNode::ArithmeticDomain(_) => true,
                            TypeConstraintNode::Named(_) => false,
                        })
            }
            _ => false,
        }
    }

    pub(crate) fn requires_custody(
        program: &dyn ::validation::CheckingInput,
        state: symbols::SymbolHandle,
        expression: ExpressionHandle,
    ) -> bool {
        program.subtree_any(
            expression,
            typed_trees::SubtreeEdges::STRUCTURAL,
            |_, node| match node {
                ExpressionNode::Cast(cast)
                    if !cast.semantic_domain.is_empty()
                        || result_type(program, state, cast.value)
                            .is_some_and(|reference| has_declared_domains(program, reference)) =>
                {
                    typed_trees::SubtreeVisit::Found
                }
                _ => typed_trees::SubtreeVisit::Descend,
            },
        )
    }

    /*
        A checked value cast that changes arithmetic policy or interval qualification
        without changing the primitive carrier needs no payload conversion. Semantic
        domains and borrow recasts keep their separate custody and are never peeled.
    */
    pub(crate) fn same_carrier_operand(
        program: &dyn ::validation::CheckingInput,
        state_symbol: symbols::SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let cast = program.expression_table.cast_node(expression)?;
        if cast.form.is_recast()
            || !cast.semantic_domain.is_empty()
            || has_declared_domains(program, cast.target_type)
        {
            return None;
        }
        let target = program.primitive_type_reference(cast.target_type)?;
        let operand_type = result_type(program, state_symbol, cast.value)?;
        (!has_declared_domains(program, operand_type)
            && program.primitive_type_reference(operand_type) == Some(target))
        .then_some(cast.value)
    }
}
pub(crate) mod structural_fields {
    /*
        Resolve structural observations against their declared storage and exact projections.

        Root custody distinguishes parameters, owned locals and view locals. Selection
        does not: length, content and element observations must retain the same field
        path and selected type rather than rediscovering a whole-local source shape.
    */
    use super::scalar_lowering::storage_projection;
    use crate::execution::terminal_unit::types::borrowed_slice_view_element;
    use crate::execution::terminal_unit::types::parameter_is_root;
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use checked_trees::CheckedOperatorFacts;
    use checked_trees::CheckedOperatorResolutionStatus;
    use checked_trees::CheckedScalarExpression;
    use checked_trees::CheckedStructuralPredicatePathSegment;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::signature::StateParameter;
    use typed_trees::types::PrimitiveType;
    use typed_trees::types::TypeReferenceHandle;
    use typed_trees::types::TypeReferenceNode;

    fn declared_data_member(
        program: &dyn ::validation::CheckingInput,
        data_index: u32,
        member_index: u32,
    ) -> &typed_trees::data::DataMember {
        &program.data_members(&program.data_definitions()[data_index as usize])
            [member_index as usize]
    }

    fn declared_data_field(
        program: &dyn ::validation::CheckingInput,
        position: (u32, u32, u32),
    ) -> Option<&typed_trees::data::DataField> {
        let member = declared_data_member(program, position.0, position.1);
        if position.2 == u32::MAX {
            match member {
                typed_trees::data::DataMember::Field(field) => Some(field),
                _ => None,
            }
        } else {
            let variant = member.as_variant()?;
            program
                .data_payload_fields(variant)
                .get(position.2 as usize)
        }
    }

    pub(crate) fn structural_sequence_length(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
        selector: impl FnMut(
            ExpressionHandle,
            ExpressionHandle,
            TypeReferenceHandle,
        ) -> Option<CheckedScalarExpression>,
    ) -> Option<CheckedScalarExpression> {
        let receiver = expressions(program).authored_length_receiver(expression)?;
        structural_sequence_receiver_length(program, operators, parameters, receiver, selector)
    }

    /*
        Observe the exact descriptor actual of a checked predicate application.
        This shares all geometry, readability and selector gates with authored len.
    */
    pub(crate) fn structural_sequence_receiver_length(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        parameters: &[StateParameter],
        receiver: ExpressionHandle,
        selector: impl FnMut(
            ExpressionHandle,
            ExpressionHandle,
            TypeReferenceHandle,
        ) -> Option<CheckedScalarExpression>,
    ) -> Option<CheckedScalarExpression> {
        let projection = storage_projection(program, operators, receiver, parameters, selector)?;
        let selected_type = projection.selected_type;
        if let Some(length) = validation::fixed_array_type_length(program, selected_type) {
            if projection.path.iter().any(|step| match step {
                checked_trees::CheckedScalarReadProjection::Static(_) => false,
                checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) => {
                    selector.expression.contains_trapping_operation()
                }
            }) {
                return None;
            }
            return facts::u64_integer_literal(u64::try_from(length).ok()?, PrimitiveType::U64);
        }
        validation::field_domains::readable_type_reference(program, selected_type)?;
        let byte_carrier = crate::execution::terminal_unit::types::byte_sequence_carrier(
            program,
            selected_type,
            &[],
        );
        let element_view = borrowed_slice_view_element(program, selected_type, &[]);
        if !matches!(
            byte_carrier,
            Some(
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                    | checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }
            )
        ) && element_view.is_none()
        {
            return None;
        }
        Some(CheckedScalarExpression::StructuralSequenceLength {
            root: projection.root,
            path: projection.path,
        })
    }

    /*
        Read the current reference in a local slot, including a replaced slot.
        Replacing the slot does not change the reference type's read/write access.
        This records its declaration and current home, not an earlier pointer or
        frozen contents. Body sequencing establishes that home before each read;
        lowering resolves the symbol to its current place or refuses.
    */
    pub(crate) fn view_local_root(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
        let (symbol, local) = whole_local_binding(program, parameters, expression)?;
        validation::reference_shell(program, local.type_reference)
            .is_some_and(|shell| {
                matches!(
                    shell.access,
                    language_core::ReferenceAccess::Shared
                        | language_core::ReferenceAccess::Mutable
                )
            })
            .then_some((symbol, local.type_reference))
    }

    /*
        The `let` symbol and declared type of an owned structural local that
        `expression` names whole, when that name is not a state parameter or a
        view local. The plan that sequences the body decides whether the local's
        place was established before this use; lowering resolves the symbol to
        that place or refuses.
    */
    pub(crate) fn owned_local_root(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
        let (symbol, local) = whole_local_binding(program, parameters, expression)?;
        let mut carrier_type = local.type_reference;
        while let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(carrier_type)
        {
            carrier_type = *base_type;
        }
        let owned = match program.type_reference_table.type_reference(carrier_type) {
            TypeReferenceNode::FixedArray { .. } => true,
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => false,
            _ => program.closed_data_definition(carrier_type).is_some(),
        };
        owned.then_some((symbol, local.type_reference))
    }

    /*
        Empty spelling resolution is builtin only when the exact operand types
        independently carry the builtin index meaning.
    */
    pub(crate) fn indexed_read_is_builtin(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
        collection_type: TypeReferenceHandle,
        index: ExpressionHandle,
    ) -> bool {
        use language_core::OperatorSpelling;
        if let Some(selected) = operators.expression_use(expression)
            && (selected.spelling != OperatorSpelling::Index
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    CheckedOperatorResolutionStatus::Missing
                        | CheckedOperatorResolutionStatus::BuiltinFallback
                ))
        {
            return false;
        }
        indexed_read_has_builtin_meaning(program, parameters, expression, collection_type, index)
    }

    fn indexed_read_has_builtin_meaning(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
        collection_type: TypeReferenceHandle,
        index: ExpressionHandle,
    ) -> bool {
        let (owner, index_type) = if let Some((machine_index, state_index)) =
            program.state_position_for_parameter_symbols(parameters)
        {
            let machine = &program.machines()[machine_index as usize];
            let state = &program.machine_states(machine)[state_index as usize];
            (
                machine.symbol,
                validation::declared_place_type_raw(program, machine, Some(state), index),
            )
        } else {
            let Some(first) = parameters
                .first()
                .filter(|parameter| parameter.symbol.is_valid())
            else {
                return false;
            };
            let scope = program.symbols.get(first.symbol).parent;
            if !scope.is_valid()
                || parameters.iter().any(|parameter| {
                    !parameter.symbol.is_valid()
                        || program.symbols.get(parameter.symbol).parent != scope
                })
            {
                return false;
            }
            if program.symbols.get(scope).kind == symbols::SymbolKind::State
                && !program
                    .trait_machine_signature(scope)
                    .is_some_and(|(_, signature)| {
                        program.state_signature_parameters(signature) == parameters
                    })
            {
                return false;
            }
            (
                scope,
                validation::parameter_expression_result_type_reference(
                    program, scope, parameters, index,
                ),
            )
        };
        let operands = [Some(collection_type), index_type];
        typed_trees::operator::builtin_index_read_is_spelled(program, owner, expression, &operands)
    }

    /*
        Borrowed formal entry is represented by the published root access, whereas
        stored references below it remain physical descriptors in the path.
    */
    pub(crate) fn predicate_parameter_type(
        program: &dyn ::validation::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        let mut endpoint = reference;
        let mut visited = Vec::new();
        loop {
            if !program
                .type_reference_table
                .contains_type_reference(reference)
                || visited.contains(&reference)
            {
                return None;
            }
            visited.push(reference);
            if validation::stored_view_shell(program, reference, &[]).is_some() {
                return Some(endpoint);
            }
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Reference { referee, .. } => {
                    reference = *referee;
                    endpoint = reference;
                }
                _ => return Some(endpoint),
            }
        }
    }

    /*
        Retain each actual readable reference shell rather than asking a consumer
        to infer pointer loads from the next field or final sum shape.
    */
    pub(crate) fn predicate_referent_path(
        program: &dyn ::validation::CheckingInput,
        mut reference: TypeReferenceHandle,
        path: &mut Vec<CheckedStructuralPredicatePathSegment>,
    ) -> Option<TypeReferenceHandle> {
        let mut endpoint = reference;
        let mut visited = Vec::new();
        loop {
            if visited.contains(&reference) {
                return None;
            }
            visited.push(reference);
            if validation::stored_view_shell(program, reference, &[]).is_some() {
                return Some(endpoint);
            }
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Reference {
                    referee, access, ..
                } if *access != language_core::ReferenceAccess::WriteOnly => {
                    path.push(CheckedStructuralPredicatePathSegment::Referent);
                    reference = *referee;
                    endpoint = reference;
                }
                TypeReferenceNode::Reference { .. } => return None,
                _ => return Some(endpoint),
            }
        }
    }

    pub(crate) fn structural_parameter_field_path(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
        fields: &mut Vec<CheckedStructuralPredicatePathSegment>,
    ) -> Option<u32> {
        parameter_projection(program, parameters, expression, fields).map(|(position, _)| position)
    }

    pub(crate) fn structural_parameter_case_path(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
        path: &mut Vec<CheckedStructuralPredicatePathSegment>,
    ) -> Option<u32> {
        let (position, selected) = parameter_projection(program, parameters, expression, path)?;
        predicate_referent_path(program, selected, path)?;
        Some(position)
    }

    pub(crate) fn explicit_observation_path(
        program: &dyn validation::CheckingInput,
        reference: TypeReferenceHandle,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<(
        Vec<CheckedStructuralPredicatePathSegment>,
        TypeReferenceHandle,
    )> {
        let mut explicit = Vec::new();
        let mut case_payload = false;
        let (_, selected, _) =
            resolve_structural_path(program, reference, false, path, &mut |receiver, step| {
                if let Some(step) = step {
                    if !case_payload
                        && !matches!(step, CheckedStructuralPredicatePathSegment::Referent)
                    {
                        predicate_referent_path(program, receiver, &mut explicit)?;
                    }
                    explicit.push(step.clone());
                    case_payload = matches!(step, CheckedStructuralPredicatePathSegment::Case(_));
                }
                Some(())
            })?;
        let selected = predicate_referent_path(program, selected, &mut explicit)?;
        Some((explicit, selected))
    }

    /*
        Carry the applied receiver type through the authored walk instead of
        reconstructing each prefix or selecting fields from a global name roster.
    */
    fn parameter_projection(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
        fields: &mut Vec<CheckedStructuralPredicatePathSegment>,
    ) -> Option<(u32, TypeReferenceHandle)> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Indexed(indexed) => {
                let (position, collection_type) =
                    parameter_projection(program, parameters, indexed.collection, fields)?;
                if !indexed_read_has_builtin_meaning(
                    program,
                    parameters,
                    expression,
                    collection_type,
                    indexed.index,
                ) {
                    return None;
                }
                let element_index =
                    u64::try_from(expressions(program).constant_integer_value(indexed.index)?)
                        .ok()?;
                let collection_type = predicate_referent_path(program, collection_type, fields)?;
                let element = fixed_index_element_type(program, collection_type, element_index)?;
                fields.push(CheckedStructuralPredicatePathSegment::FixedIndex(
                    element_index,
                ));
                Some((position, element))
            }
            ExpressionNode::Name(name) => {
                if program.symbols.get(name.symbol).kind == symbols::SymbolKind::Machine {
                    return None;
                }
                let root = name.symbol.is_valid().then_some(name.symbol).or_else(|| {
                    expressions(program)
                        .name_path_member_symbols(name.member_symbols)
                        .iter()
                        .copied()
                        .find(|symbol| symbol.is_valid())
                })?;
                let position = parameters
                    .iter()
                    .position(|parameter| parameter_is_root(parameter, root))?;
                Some((
                    u32::try_from(position).ok()?,
                    predicate_parameter_type(program, parameters[position].type_reference)?,
                ))
            }
            ExpressionNode::Member(member) => {
                let (parameter, receiver) =
                    parameter_projection(program, parameters, member.receiver, fields)?;
                let (projection, selected) = member_projection(program, receiver, member)?;
                fields.extend(projection);
                Some((parameter, selected))
            }
            _ => None,
        }
    }

    /*
        The resolved path is `frozen` when the root parameter and every
        intermediate receiver are immutable: no `mut` formal and no exclusive
        reference. Declared constraints on frozen storage are read invariants;
        mutable storage can be reborrowed through a pointee type that drops them,
        so only the live snapshots speak for it.
    */
    pub(crate) fn resolve_structural_parameter_path(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<(
        symbols::SymbolHandle,
        Vec<facts::PlaceSegment>,
        TypeReferenceHandle,
        bool,
    )> {
        let parameter = parameters.get(usize::try_from(position).ok()?)?;
        if !parameter.symbol.is_valid() {
            return None;
        }
        let (segments, selected_type, frozen) = resolve_structural_path(
            program,
            parameter.type_reference,
            parameter.is_mutable,
            path,
            &mut |_, _| Some(()),
        )?;
        Some((parameter.symbol, segments, selected_type, frozen))
    }

    /*
        Admit total entry observations through the ordinary declaration walk.
        Geometry alone can peel references or select a view element; an entry
        predicate retains readable custody at every reference crossing; lowering
        records those crossings explicitly. Static selectors still require fixed
        arrays, so no dynamic bounds obligation hides inside this declaration walk.
        Case validity remains the ordinary contract checker's obligation.
    */
    pub(crate) fn readable_entry_parameter_path(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
        remaining: &mut usize,
    ) -> Option<TypeReferenceHandle> {
        let parameter = parameters.get(usize::try_from(position).ok()?)?;
        if parameter.is_const || parameter.relevance.is_erased() || path.len() >= 64 {
            return None;
        }
        let (_, selected_type, _) = resolve_structural_path(
            program,
            parameter.type_reference,
            parameter.is_mutable,
            path,
            &mut |mut receiver, segment| {
                for _ in 0..64 {
                    *remaining = remaining.checked_sub(1)?;
                    if !program
                        .type_reference_table
                        .contains_type_reference(receiver)
                    {
                        return None;
                    }
                    match program.type_reference_table.type_reference(receiver) {
                        TypeReferenceNode::Constrained { base_type, .. } => receiver = *base_type,
                        TypeReferenceNode::Reference {
                            referee, access, ..
                        } if *access != language_core::ReferenceAccess::WriteOnly => {
                            receiver = *referee
                        }
                        TypeReferenceNode::Reference { access, .. }
                            if matches!(
                                segment,
                                Some(CheckedStructuralPredicatePathSegment::Referent)
                            ) && *access != language_core::ReferenceAccess::WriteOnly =>
                        {
                            return Some(());
                        }
                        TypeReferenceNode::Reference { .. } => return None,
                        node => {
                            if matches!(
                                segment,
                                Some(CheckedStructuralPredicatePathSegment::FixedIndex(_))
                            ) && !matches!(
                                node,
                                TypeReferenceNode::FixedArray {
                                    length: typed_trees::types::FixedArrayLength::Literal(_),
                                    ..
                                }
                            ) {
                                return None;
                            }
                            if matches!(
                                segment,
                                Some(
                                    CheckedStructuralPredicatePathSegment::Field(_)
                                        | CheckedStructuralPredicatePathSegment::Case(_)
                                )
                            ) {
                                let definition = program.closed_data_definition(receiver)?;
                                if program.symbols.get(definition.symbol).kind
                                    != symbols::SymbolKind::Data
                                {
                                    return None;
                                }
                            }
                            return Some(());
                        }
                    }
                }
                None
            },
        )?;
        Some(selected_type)
    }

    /*
        Resolve each projection against the declaration reached by its predecessor,
        independent of the storage root's custody. Keeping this join common prevents
        local observations from losing fixed indexes or accepting a foreign field.
    */
    pub(crate) fn resolve_structural_path(
        program: &dyn ::validation::CheckingInput,
        declared_type: TypeReferenceHandle,
        is_mutable: bool,
        path: &[CheckedStructuralPredicatePathSegment],
        observe_receiver: &mut impl FnMut(
            TypeReferenceHandle,
            Option<&CheckedStructuralPredicatePathSegment>,
        ) -> Option<()>,
    ) -> Option<(Vec<facts::PlaceSegment>, TypeReferenceHandle, bool)> {
        let mut receiver = declared_type;
        let mut frozen = !is_mutable && !exclusive_reference(program, receiver);
        let mut segments = Vec::new();
        let mut selected_case = None;
        for segment in path {
            observe_receiver(receiver, Some(segment))?;
            match segment {
                CheckedStructuralPredicatePathSegment::Referent => {
                    if selected_case.is_some() {
                        return None;
                    }
                    let mut selected = receiver;
                    while let TypeReferenceNode::Constrained { base_type, .. } =
                        program.type_reference_table.type_reference(selected)
                    {
                        selected = *base_type;
                    }
                    let TypeReferenceNode::Reference {
                        referee, access, ..
                    } = program.type_reference_table.type_reference(selected)
                    else {
                        return None;
                    };
                    if *access == language_core::ReferenceAccess::WriteOnly {
                        return None;
                    }
                    receiver = *referee;
                }
                CheckedStructuralPredicatePathSegment::FixedIndex(element_index) => {
                    if selected_case.is_some() {
                        return None;
                    }
                    receiver = fixed_index_element_type(program, receiver, *element_index)?;
                    segments.push(facts::PlaceSegment::FixedIndex {
                        index: usize::try_from(*element_index).ok()?,
                    });
                }
                CheckedStructuralPredicatePathSegment::Case(identity) => {
                    if selected_case.is_some() {
                        return None;
                    }
                    let definition = structural_data(program, receiver)?;
                    let variant = program.data_members(definition).iter().find_map(|member| {
                        let variant = member.as_variant()?;
                        (*identity
                            == checked_trees::CheckedMemberIdentity::of_member(
                                variant.identity,
                                variant.name.clone(),
                            ))
                        .then_some(variant)
                    })?;
                    if !variant.symbol.is_valid() {
                        return None;
                    }
                    segments.push(facts::PlaceSegment::Case {
                        variant: variant.symbol,
                    });
                    selected_case = Some(variant);
                }
                CheckedStructuralPredicatePathSegment::Field(identity) => {
                    let matches = |field: &&typed_trees::data::DataField| {
                        checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        ) == *identity
                    };
                    let field = if let Some(variant) = selected_case.take() {
                        program.data_payload_fields(variant).iter().find(matches)?
                    } else {
                        let definition = structural_data(program, receiver)?;
                        program
                            .data_members(definition)
                            .iter()
                            .filter_map(|member| {
                                let field = member.as_field()?;
                                Some(field)
                            })
                            .find(matches)?
                    };
                    if !field.symbol.is_valid() || field.relevance.is_erased() {
                        return None;
                    }
                    segments.push(facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    receiver = field.type_reference;
                }
                CheckedStructuralPredicatePathSegment::RuntimeIndex(_) => return None,
            }
            if exclusive_reference(program, receiver) {
                frozen = false;
            }
        }
        observe_receiver(receiver, None)?;
        selected_case
            .is_none()
            .then_some((segments, receiver, frozen))
    }

    /*
        Whether this reference resolves through an exclusive borrow (`&mut`/`&w`).
        Shared references keep their referent frozen for the borrow's lifetime.
    */
    pub(crate) fn exclusive_reference(
        program: &dyn ::validation::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> bool {
        loop {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => reference = *referee,
                TypeReferenceNode::Reference { access, .. } => return access.is_exclusive(),
                _ => return false,
            }
        }
    }

    /*
        A declared predicate can describe a read only while the selected storage
        cannot change. Follow canonical type coordinates, including dynamic array
        selections, rather than requiring one particular scalar-expression shape.
        Mutable bindings, exclusive crossings and interior authority use live facts.
    */
    pub(crate) fn storage_observation_is_frozen(
        program: &dyn ::validation::CheckingInput,
        state: symbols::SymbolHandle,
        statement: usize,
        expression: ExpressionHandle,
    ) -> bool {
        fn shared_reference_chain(
            program: &dyn ::validation::CheckingInput,
            mut reference: TypeReferenceHandle,
        ) -> bool {
            loop {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    TypeReferenceNode::Reference {
                        referee, access, ..
                    } => {
                        if *access != language_core::ReferenceAccess::Shared {
                            return false;
                        }
                        reference = *referee;
                    }
                    _ => return true,
                }
            }
        }
        let Some(place) = place_at(program, state, statement, expression) else {
            return false;
        };
        let Some(root) = place.root.as_symbol() else {
            return false;
        };
        let Some(state) = program.state_by_symbol(state) else {
            return false;
        };
        let declaration = program
            .state_parameter_by_symbol(state, root)
            .map(|parameter| (parameter.type_reference, parameter.is_mutable))
            .or_else(|| {
                program
                    .state_local_declaration(state, root)
                    .filter(|(ordinal, _)| *ordinal < statement)
                    .map(|(_, local)| (local.type_reference, local.is_mutable))
            });
        let Some((mut reference, is_mutable)) = declaration else {
            return false;
        };
        if is_mutable {
            return false;
        }
        for segment in &place.segments {
            if !shared_reference_chain(program, reference) {
                return false;
            }
            let Some(projected) = validation::project_type_reference_from_segments(
                program,
                reference,
                std::slice::from_ref(segment),
            ) else {
                return false;
            };
            reference = projected;
        }
        shared_reference_chain(program, reference)
            && validation::has_contents(
                program,
                reference,
                &[],
                validation::ContentsRequirement::StableObservation,
            )
    }

    /*
        Peel reference and constraint shells down to the indexed carrier's node.
        A byte carrier (a borrowed `&[u8]` or `&[u8; N]`, or a domain-qualified
        bounded owner) declares no element row: its extent is observed through the
        carrier, so an index selects no static element
        (wiki/spec/language/domains.md, "Byte containers and encoding domains").
    */
    fn peeled_indexed_carrier(
        program: &dyn ::validation::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        loop {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference { .. } | TypeReferenceNode::Constrained { .. }
                    if validation::byte_sequence_carrier(program, reference, &[]).is_some() =>
                {
                    return None;
                }
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => reference = *referee,
                _ => return Some(reference),
            }
        }
    }

    pub(crate) fn fixed_index_element_type(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
        element_index: u64,
    ) -> Option<TypeReferenceHandle> {
        let carrier = peeled_indexed_carrier(program, reference)?;
        match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::FixedArray { length, .. } => match length {
                typed_trees::types::FixedArrayLength::Literal(length) => (element_index
                    < u64::try_from(*length).ok()?)
                .then(|| validation::collection_element_type_reference(program, reference))?,
                typed_trees::types::FixedArrayLength::ConstCall { .. }
                    if program.constant_recipe_for_array_length(carrier).is_valid() =>
                {
                    validation::collection_element_type_reference(program, reference)
                }
                _ => None,
            },
            TypeReferenceNode::Slice { .. } => {
                validation::collection_element_type_reference(program, reference)
            }
            _ => None,
        }
    }

    /*
        The element type a runtime selector reaches inside a collection: the
        same unwrap `fixed_index_element_type` walks, with no literal to prove
        against the extent — the read's own obligation checks the bound.
    */
    pub(crate) fn runtime_index_element_type(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        let carrier = peeled_indexed_carrier(program, reference)?;
        match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::FixedArray { length, .. } => {
                (matches!(length, typed_trees::types::FixedArrayLength::Literal(_))
                    || program.constant_recipe_for_array_length(carrier).is_valid())
                .then(|| validation::collection_element_type_reference(program, reference))?
            }
            TypeReferenceNode::Slice { .. } => {
                validation::collection_element_type_reference(program, reference)
            }
            _ => None,
        }
    }

    pub(crate) fn structural_data(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        let peeled = program
            .type_reference_table
            .peeled_type_reference(type_reference);
        let (symbol, name) = match program.type_reference_table.type_reference(peeled) {
            TypeReferenceNode::Named { symbol, name }
            | TypeReferenceNode::Generic {
                base_symbol: symbol,
                base_name: name,
                ..
            } => {
                if let Some(instance) = program.closed_data_definition(peeled) {
                    return Some(instance);
                }
                (*symbol, name)
            }
            _ => return None,
        };
        if symbol.is_valid() {
            program.data_definition_by_symbol(symbol)
        } else {
            program
                .data_definition_positions_for_name(name.as_str())
                .first()
                .map(|position| &program.data_definitions()[*position as usize])
        }
    }

    pub(crate) fn structural_parameter_place(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<(
        u32,
        Vec<CheckedStructuralPredicatePathSegment>,
        TypeReferenceHandle,
    )> {
        let (root, path, selected_type) =
            structural_storage_place(program, parameters, expression)?;
        let checked_trees::CheckedStorageRoot::Parameter { index } = root else {
            return None;
        };
        Some((index, path, selected_type))
    }

    /*
        Authenticate one authored storage occurrence and resolve its entire static
        projection. Root classification supplies custody only; all roots use the
        same canonical place and declaration traversal below.
    */
    pub(crate) fn structural_storage_place(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<(
        checked_trees::CheckedStorageRoot,
        Vec<CheckedStructuralPredicatePathSegment>,
        TypeReferenceHandle,
    )> {
        let place = validation::canonical_place_from_expression(program, expression)?;
        let mut authored = expression;
        let mut selected_fields = place
            .segments
            .iter()
            .rev()
            .filter_map(|segment| match segment {
                facts::PlaceSegment::Field { symbol } => Some(*symbol),
                _ => None,
            });
        loop {
            match program.expression_table.expression(authored) {
                ExpressionNode::Borrow(borrow) => {
                    authored = borrow.target;
                    continue;
                }
                ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                    authored = cast.value;
                    continue;
                }
                _ => {}
            }
            if let Some(indexed) = program.expression_table.indexed_node(authored) {
                let (_, _, collection_type) =
                    structural_storage_place(program, parameters, indexed.collection)?;
                if !indexed_read_has_builtin_meaning(
                    program,
                    parameters,
                    authored,
                    collection_type,
                    indexed.index,
                ) {
                    return None;
                }
                authored = indexed.collection;
                continue;
            }
            let Some(member) = program.expression_table.member_node(authored) else {
                break;
            };
            let selected = selected_fields.next()?;
            if member.member_symbol.is_valid() && selected != member.member_symbol {
                let closed_field =
                    validation::expression_place_type_reference(program, member.receiver, &[])
                        .and_then(|reference| {
                            validation::unwrapped_type_reference(program, reference)
                        })
                        .and_then(|reference| {
                            program.closed_data_member_field(
                                reference,
                                member.member_symbol,
                                member.member.as_str(),
                            )
                        });
                if closed_field.is_some_and(|field| field.symbol == selected) {
                    authored = member.receiver;
                    continue;
                }
                return None;
            }
            authored = member.receiver;
        }
        let root = place.root;
        let _ = root.as_symbol()?;
        let name = program.expression_table.name_path_node(authored)?;
        let names = program.expression_table.name_path_members(name.members);
        let (storage_root, declared_type, is_mutable) = if matches!(names, [spelling] if spelling.is_self_receiver())
            && !parameters
                .iter()
                .any(|parameter| parameter.symbol == name.head_symbol)
        {
            return None;
        } else {
            if !name.symbol.is_valid()
                || !name.head_symbol.is_valid()
                || (names.len() == 1 && name.symbol != name.head_symbol)
            {
                return None;
            }
            let mut candidates = parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| parameter.symbol == name.head_symbol);
            if let Some((position, parameter)) = candidates.next() {
                if candidates.next().is_some()
                    || program.symbols.get(name.head_symbol).kind != symbols::SymbolKind::Parameter
                    || root != facts::PlaceRoot::Symbol(parameter.symbol)
                {
                    return None;
                }
                (
                    checked_trees::CheckedStorageRoot::Parameter {
                        index: u32::try_from(position).ok()?,
                    },
                    parameter.type_reference,
                    parameter.is_mutable,
                )
            } else {
                let (symbol, local) = whole_local_binding(program, parameters, authored)?;
                if root != facts::PlaceRoot::Symbol(symbol) {
                    return None;
                }
                let storage_root = if view_local_root(program, parameters, authored).is_some() {
                    checked_trees::CheckedStorageRoot::ViewLocal { symbol }
                } else {
                    owned_local_root(program, parameters, authored)?;
                    checked_trees::CheckedStorageRoot::OwnedLocal { symbol }
                };
                (storage_root, local.type_reference, local.is_mutable)
            }
        };
        let path = structural_projection_path(program, &place.segments)?;
        let (segments, _, _) =
            resolve_structural_path(program, declared_type, is_mutable, &path, &mut |_, _| {
                Some(())
            })?;
        if segments != place.segments {
            return None;
        }
        let observed_root = if storage_root.parameter().is_some() {
            predicate_parameter_type(program, declared_type)?
        } else {
            declared_type
        };
        let (explicit, selected) = explicit_observation_path(program, observed_root, &path)?;
        Some((storage_root, explicit, selected))
    }

    /*
        Rebind a declaration-owned projection through the actual receiver's
        retained nominal identities. This uses the same path judgment as source
        parameter observations; template symbols are selectors, not applied slots.
    */
    pub(crate) fn applied_projection_segments(
        program: &dyn validation::CheckingInput,
        reference: TypeReferenceHandle,
        segments: &[facts::PlaceSegment],
    ) -> Option<Vec<facts::PlaceSegment>> {
        let path = structural_projection_path(program, segments)?;
        resolve_structural_path(program, reference, false, &path, &mut |_, _| Some(()))
            .map(|(segments, _, _)| segments)
    }
    /*
        Translate retained declaration coordinates without assigning a storage root.
        The receiving root must independently resolve this path against its type.
    */
    pub(crate) fn structural_projection_path(
        program: &dyn validation::CheckingInput,
        segments: &[facts::PlaceSegment],
    ) -> Option<Vec<CheckedStructuralPredicatePathSegment>> {
        let mut path = Vec::with_capacity(segments.len());
        for segment in segments {
            path.push(match segment {
                facts::PlaceSegment::Field { symbol } => {
                    let field = program
                        .data_field_positions_for_symbol(*symbol)
                        .iter()
                        .copied()
                        .find_map(|position| declared_data_field(program, position))?;
                    CheckedStructuralPredicatePathSegment::Field(
                        checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        ),
                    )
                }
                facts::PlaceSegment::Case { variant } => {
                    let case = program
                        .data_variant_position_for_symbol(*variant)
                        .and_then(|(data_index, member_index)| {
                            match declared_data_member(program, data_index, member_index) {
                                typed_trees::data::DataMember::Variant(candidate) => {
                                    Some(candidate)
                                }
                                _ => None,
                            }
                        })?;
                    CheckedStructuralPredicatePathSegment::Case(
                        checked_trees::CheckedMemberIdentity::of_member(
                            case.identity,
                            case.name.clone(),
                        ),
                    )
                }
                facts::PlaceSegment::FixedIndex { index } => {
                    CheckedStructuralPredicatePathSegment::FixedIndex(u64::try_from(*index).ok()?)
                }
                _ => return None,
            });
        }
        Some(path)
    }

    /*
        Resolve a member and its payload namespace under the exact applied receiver.
        Normalized observations may omit a member symbol or an authored case qualifier;
        the receiver declaration supplies a unique selection, while any retained symbol
        must agree. All parameter and indexed paths share this operation. Resolving the
        declaration grants neither case membership nor ownership of its siblings.
    */
    pub(crate) fn member_projection(
        program: &dyn ::validation::CheckingInput,
        receiver: TypeReferenceHandle,
        member: &typed_trees::expression::TableMemberExpression,
    ) -> Option<(
        Vec<CheckedStructuralPredicatePathSegment>,
        TypeReferenceHandle,
    )> {
        let mut path = Vec::new();
        let current = predicate_referent_path(program, receiver, &mut path)?;
        let field = validation::applied_data_member_field(program, current, member)?;
        if field.relevance.is_erased()
            || program.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
        {
            return None;
        }
        let mut segments = Vec::new();
        if let Some(variant) = facts::payload_variant_for_field(program, field.symbol) {
            let (data, member_position) = program.data_variant_position_for_symbol(variant)?;
            let declaration = declared_data_member(program, data, member_position).as_variant()?;
            path.push(CheckedStructuralPredicatePathSegment::Case(
                checked_trees::CheckedMemberIdentity::of_member(
                    declaration.identity,
                    declaration.name.clone(),
                ),
            ));
            segments.push(facts::PlaceSegment::Case { variant });
        }
        path.push(CheckedStructuralPredicatePathSegment::Field(
            checked_trees::CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
        ));
        segments.push(facts::PlaceSegment::Field {
            symbol: field.symbol,
        });
        let selected =
            validation::project_type_reference_from_segments(program, receiver, &segments)?;
        Some((path, selected))
    }

    /*
        The `let` local `expression` names whole when that name is not a state
        parameter: its symbol and declared local-data row. Both whole-name local
        root lookups peel the same authored-name shape before their kind gates.
    */
    fn whole_local_binding<'a>(
        program: &'a dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<(
        symbols::SymbolHandle,
        &'a typed_trees::statement::TableLocalData,
    )> {
        let path = program.expression_table.name_path_node(expression)?;
        if !program.expression_table.name_path_is_single_member(path)
            || parameters
                .iter()
                .any(|parameter| parameter.symbol == path.symbol)
        {
            return None;
        }
        let positions = program.local_data_positions_for_symbol(path.symbol);
        let [position] = positions.as_slice() else {
            return None;
        };
        let machine = &program.machines()[position.0 as usize];
        let state = &program.machine_states(machine)[position.1 as usize];
        let typed_trees::statement::StatementNode::LocalData(local) =
            &program.statement_table.statements(state.statement_nodes)[position.2 as usize]
        else {
            return None;
        };
        Some((path.symbol, local))
    }
}
pub(crate) mod subslice_endpoints {
    /*
        The present endpoints of one authored collection range.

        A call argument, a transition argument and a `let` initializer narrow a
        view with the same builtin range, so every site lowers its endpoints here
        under the shared `SubsliceStart`/`SubsliceEnd` roles keyed by its
        `CheckedSubsliceSite`. An omitted endpoint produces no row: zero and the
        source length are supplied by the range's consumer, not by a scalar fact.
        An inclusive end retains its authored value here too; its exact successor is
        descriptor normalization after evaluation, not a second scalar source site.
    */
    use validation::CallSite;

    use checked_trees::{
        CheckedOperatorFacts, CheckedOperatorResolutionStatus,
        CheckedScalarExpressionRole as ExpressionRole, CheckedSubsliceSite,
    };
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::statement::StatementNode;

    /*
        Lower the present endpoints of `expression` when it is a range
        whose operator use (if any) resolved to the builtin meaning. `lower`
        evaluates one endpoint in its authored integer carrier in the site's scalar
        namespace. Descriptor emission performs the exact count conversion later.
    */
    pub(crate) fn subslice_endpoints<Value>(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        expression: ExpressionHandle,
        site: CheckedSubsliceSite,
        mut lower: impl FnMut(ExpressionHandle) -> Option<Value>,
    ) -> Vec<(ExpressionHandle, ExpressionRole, Value)> {
        let Some(expression) = validation::subslice_expression(program, expression) else {
            return Vec::new();
        };
        let Some(indexed) = program.expression_table.indexed_node(expression) else {
            return Vec::new();
        };
        let Some(range) = program.expression_table.range_node(indexed.index) else {
            return Vec::new();
        };
        if operators
            .expression_use(expression)
            .is_some_and(|selected| {
                selected.spelling != language_core::OperatorSpelling::Range
                    || selected.selected_operator_symbol.is_valid()
                    || selected.candidate_count != 0
                    || !matches!(
                        selected.status,
                        CheckedOperatorResolutionStatus::Missing
                            | CheckedOperatorResolutionStatus::BuiltinFallback
                    )
            })
        {
            return Vec::new();
        }
        [
            (range.start, ExpressionRole::SubsliceStart { site }),
            (range.end, ExpressionRole::SubsliceEnd { site }),
        ]
        .into_iter()
        .filter(|(endpoint, _)| endpoint.is_valid())
        .filter_map(|(endpoint, role)| Some((endpoint, role, lower(endpoint)?)))
        .collect()
    }

    /*
        The source occurrence, not the result's destination or the endpoint's syntax,
        identifies a range operand. Both scalar plan producers consume this inventory.
    */
    pub(crate) fn sites(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        statement: &StatementNode,
    ) -> Vec<(ExpressionHandle, CheckedSubsliceSite)> {
        let mut result = Vec::new();
        if let Some(local) = statement.as_local_data() {
            result.push((local.initial_value, CheckedSubsliceSite::LocalBinding));
        }
        for (call_ordinal, site) in
            validation::collect_call_sites_in_statement(program, machine, state, statement_index)
                .into_iter()
                .enumerate()
        {
            let Ok(call_ordinal) = u32::try_from(call_ordinal) else {
                break;
            };
            if let validation::CallSite::TransitionNamed { path, .. } = &site
                && validation::terminal_callable_target(program, machine, path.symbol).is_none()
            {
                continue;
            }
            for (argument, argument_ordinal) in structural_call_arguments(program, &site) {
                result.push((
                    argument,
                    CheckedSubsliceSite::CallArgument {
                        call_ordinal,
                        argument_ordinal,
                    },
                ));
            }
        }
        if let Some(transition) = statement.as_transition()
            && let Some(transfer) =
                validation::completing_transition_transfer(program, transition.target)
            && let Some(parameters) = validation::call_target_parameters(program, transfer.target)
        {
            for (argument, (position, parameter)) in
                transfer.arguments.iter().zip(parameters.iter().enumerate())
            {
                if !parameter.relevance.is_erased()
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
                    && let Ok(argument_ordinal) = u32::try_from(position)
                {
                    result.push((
                        *argument,
                        CheckedSubsliceSite::TransitionArgument { argument_ordinal },
                    ));
                }
            }
        }
        let table = &program.expression_table;
        let mut pending: Vec<_> = program
            .tables
            .expression_roots(statement)
            .into_iter()
            .map(|(_, root)| root)
            .filter(|root| root.is_valid())
            .collect();
        let mut visited = Vec::new();
        while let Some(expression) = pending.pop() {
            if !table.expression_is_valid(expression) || visited.contains(&expression) {
                continue;
            }
            visited.push(expression);
            result.push((expression, CheckedSubsliceSite::Expression { expression }));
            validation::expression_traversal::children(
                program,
                table.expression(expression),
                |child| {
                    if child.is_valid() {
                        pending.push(child);
                    }
                },
            );
        }
        result
    }

    /*
        Structural formals have their own dense ordinal. Erased and scalar formals
        consume their authored actual but no structural position.
    */
    fn structural_call_arguments(
        program: &dyn ::validation::CheckingInput,
        site: &validation::CallSite<'_>,
    ) -> Vec<(typed_trees::expression::ExpressionHandle, u32)> {
        let target = match site {
            CallSite::Statement(call) => call.target_symbol,
            CallSite::Expression { call, .. } => validation::call_target_symbol(program, call),
            CallSite::TransitionNamed { path, .. } => path.symbol,
        };
        let Some(parameters) = validation::call_target_parameters(program, target) else {
            return Vec::new();
        };
        let arguments = validation::call_site_argument_expressions(program, site);
        let explicit_parameters = parameters.iter();
        if parameters.len() != arguments.len() {
            return Vec::new();
        }
        let mut structural = Vec::new();
        for (argument, parameter) in arguments.iter().zip(explicit_parameters) {
            let Some(erased) = crate::execution::terminal_unit::strips_erased_parameter(parameter)
            else {
                return Vec::new();
            };
            if erased
                || program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
            {
                continue;
            }
            let Ok(ordinal) = u32::try_from(structural.len()) else {
                return Vec::new();
            };
            structural.push((*argument, ordinal));
        }
        structural
    }
}
pub(crate) use subslice_endpoints::subslice_endpoints;

pub(crate) use boolean_lowering::evaluate_closed_boolean_expression;
pub(crate) use call_arguments::is_scalar_return_call;
pub(crate) use call_arguments::{nested_structural_call_return_type, retain_nested_call_arguments};
pub(crate) use contract_entry::{
    lower_current_state_predicate, lower_entry_predicate, lower_entry_scalar,
    lower_machine_entry_boolean_expression, lower_machine_entry_crash_contract_expression,
    lower_machine_entry_scalar_argument, lower_operator_crash_contract_expression,
    lower_signature_crash_contract_expression,
};
pub(crate) use expression_facts::{
    occupies_scalar_position, operator_is_builtin, parameter_position, scalar_expression_type,
    scalar_parameter_roster, scalar_position_count, zeroed_scalar_local_initializer,
};
pub(crate) use expression_plans::{
    assignment_target_primitive_type, build_checked_scalar_expression_plans,
};
pub(crate) use machine_parameter_booleans::{
    lower_machine_parameter_boolean_expression, lower_parameter_boolean_expression,
    lower_parameter_scalar_expression,
};
pub(crate) use result_contract::{
    lower_callable_parameter_range_requirements, lower_callable_scalar_contract_predicate,
    lower_scalar_contract_predicate, lower_scalar_parameter_refinement_predicates,
    lower_state_domain_predicate, lower_state_scalar_contract_predicate,
};
pub(crate) use scalar_lowering::{
    landed_for_primitive, lower_state_scalar_expression, lower_unit_scalar_argument,
    lower_unit_scalar_argument_for_places, unit_scalar_binding_symbols,
};
pub(crate) use structural_fields::{
    applied_projection_segments, exclusive_reference, resolve_structural_parameter_path,
    structural_data, structural_parameter_place,
};

mod erased_observations {
    /*
        Proof-side observations retain exact current places without executable reads.
        Immutable shared descriptors may name their original referent; immutable
        closed constructors may supply their stored scalar leaf. Neither operation
        copies a referent's mutable initializer, crosses a recast, changes the
        authored call actual, or creates an invocation from a captured call result.
        Source type/loan/scope/precondition validation remains the admission owner.
    */
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::types::TypeReferenceNode;

    pub(crate) fn resolve(
        program: &dyn ::validation::CheckingInput,
        state_symbol: symbols::SymbolHandle,
        before_statement: u32,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let state = program.state_by_symbol(state_symbol)?;
        let mut current = expression;
        let mut visited = Vec::new();
        loop {
            if visited.contains(&current)
                || visited.len() >= program.expression_table.expression_count()
            {
                return None;
            }
            visited.push(current);
            if !static_place_syntax(program, current) {
                return Some(current);
            }
            let Some(place) = validation::canonical_place_from_expression(program, current) else {
                return Some(current);
            };
            let Some(symbol) = place.root.as_symbol() else {
                return Some(current);
            };
            let Some((ordinal, local)) = program.state_local_declaration(state, symbol) else {
                return Some(current);
            };
            if ordinal >= before_statement as usize || !local.initial_value.is_valid() {
                return None;
            }
            if local.is_mutable {
                return Some(current);
            }
            if place.segments.is_empty() {
                if program
                    .primitive_type_reference(local.type_reference)
                    .is_some()
                {
                    return Some(
                        validation::closed_scalar_place_projection(
                            program,
                            local.initial_value,
                            local.type_reference,
                            &[],
                        )
                        .map_or(current, |(leaf, _)| leaf),
                    );
                }
                let TypeReferenceNode::Reference { access, .. } = program
                    .type_reference_table
                    .type_reference(local.type_reference)
                else {
                    return Some(current);
                };
                let ExpressionNode::Borrow(borrow) =
                    program.expression_table.expression(local.initial_value)
                else {
                    return Some(current);
                };
                if *access != language_core::ReferenceAccess::Shared || borrow.access != *access {
                    return None;
                }
                current = borrow.target;
                continue;
            }
            let (leaf, _) = validation::closed_scalar_place_projection(
                program,
                local.initial_value,
                local.type_reference,
                &place.segments,
            )?;
            return Some(leaf);
        }
    }

    fn static_place_syntax(
        program: &dyn ::validation::CheckingInput,
        mut expression: ExpressionHandle,
    ) -> bool {
        for _ in 0..program.expression_table.expression_count() {
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(_) => return true,
                ExpressionNode::Member(member) => expression = member.receiver,
                ExpressionNode::Indexed(indexed) => expression = indexed.collection,
                _ => return false,
            }
        }
        false
    }
}

/*
    Typed-program proof obligation collection, expression constraint extraction,
    canonical semantic obligation identity, and interval range reconstruction.

    We walk typed machines, states, statements, and expressions to build a
    `ProofPlan`, compute canonical `ProofObligationKey` fingerprints for caching,
    and fold expression constraints into integer and floating-point ranges.
*/

mod collection {
    use crate::obligations::constraints::{
        collect_constraints, derived_binary_constraints, expression_constraints,
    };
    use crate::obligations::plan::ConstraintBuffer;
    use crate::obligations::ranges::{constrained_type_reference, integer_range_from_constraints};
    use crate::obligations::{
        BinaryValueOperands, BoundedAssignmentObligation, BoundedCallArgumentObligation,
        BoundedInitializerObligation, BoundedStateReturnObligation,
        BoundedTransitionArgumentObligation, BoundedValueObligation, GuardedTransitionObligation,
        IntegerRange, ProofObligation, ProofObligationOwner, ProofPlan,
    };
    use crate::typed_expressions::{
        dehoisted_operand, expression_contains_call_node, expression_table,
        expression_type_reference, statement_table, type_reference_table,
    };
    use arena::HandleSpan;
    use symbols::SymbolHandle;
    use typed_trees::boundary::called_boundary_signature;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::signature::StateParameter;
    use typed_trees::state::State;
    use typed_trees::statement::{
        StatementNode, TableAssignment, TableLocalData, TransitionGuardNode,
        TransitionTargetHandle, TransitionTargetNode,
    };
    use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
    use typed_trees::{expression, statement};
    use validation::expression_traversal::visit_post_order;

    pub fn build_proof_plan(program: &dyn ::validation::CheckingInput) -> ProofPlan<'_> {
        let mut proof_plan = ProofPlan::new(program);
        let call_frames = validation::CallFrameResolver::new(program);

        for machine in program.machines() {
            for owned_data in program.machine_owned_data(machine) {
                let owner = ProofObligationOwner::MachineOwnedData {
                    machine_symbol: machine.symbol,
                    machine: machine.name.clone(),
                    data_symbol: owned_data.symbol,
                    data: owned_data.name.clone(),
                };
                collect_bounded_value_obligation(
                    program,
                    owner.clone(),
                    owned_data.type_reference,
                    &mut proof_plan,
                );
                if owned_data.initial_value.is_valid() {
                    collect_bounded_initializer_obligation(
                        program,
                        owner,
                        owned_data.type_reference,
                        owned_data.initial_value,
                        &mut proof_plan,
                    );
                }
            }

            for state in program.machine_states(machine) {
                for parameter in program.state_parameters(state) {
                    collect_bounded_value_obligation(
                        program,
                        ProofObligationOwner::StateParameter {
                            machine_symbol: machine.symbol,
                            machine: machine.name.clone(),
                            state_symbol: state.symbol,
                            state: state.name.clone(),
                            parameter_symbol: parameter.symbol,
                            parameter: parameter.name.clone(),
                        },
                        parameter.type_reference,
                        &mut proof_plan,
                    );
                }

                if state.return_type.is_valid() {
                    collect_bounded_value_obligation(
                        program,
                        ProofObligationOwner::StateReturn {
                            machine_symbol: machine.symbol,
                            machine: machine.name.clone(),
                            state_symbol: state.symbol,
                            state: state.name.clone(),
                        },
                        state.return_type,
                        &mut proof_plan,
                    );
                    collect_bounded_state_return_obligation(
                        program,
                        machine,
                        state,
                        state.return_type,
                        &mut proof_plan,
                    );
                }

                let table_statements = program.statement_table.statements(state.statement_nodes);
                let mut incoming_guard = None;
                for (statement_index, statement) in table_statements.iter().enumerate() {
                    match statement {
                        StatementNode::Assembly(_) => {}
                        StatementNode::Assignment(assignment) => {
                            collect_bounded_assignment_obligation(
                                program,
                                machine,
                                state,
                                assignment,
                                table_statements,
                                statement_index,
                                call_frames.as_ref(),
                                &mut incoming_guard,
                                &mut proof_plan,
                            );
                        }
                        StatementNode::Call(table_call) => {
                            let receiver =
                                statement_table(program).name_path_members(table_call.receiver);
                            collect_bounded_call_argument_obligations(
                                program,
                                machine,
                                state,
                                statement_index,
                                CallOccurrence {
                                    target_symbol: table_call.target_symbol,
                                    target: table_call.target.clone(),
                                    receiver: (!receiver.is_empty())
                                        .then(|| display_name_path(receiver)),
                                    arguments: statement_table(program)
                                        .expression_handles(table_call.arguments),
                                },
                                &mut proof_plan,
                            );
                        }
                        StatementNode::LocalData(local) => {
                            collect_bounded_local_initializer_obligation(
                                program,
                                machine,
                                state,
                                statement_index,
                                local,
                                &mut proof_plan,
                            );
                        }
                        StatementNode::Transition(transition) => {
                            let transition_guard = transition.guard;
                            if let TransitionGuardNode::When(_) = &transition_guard {
                                proof_plan.push_obligation(ProofObligation::GuardedTransition(
                                    GuardedTransitionObligation {
                                        machine_symbol: machine.symbol,
                                        machine: machine.name.clone(),
                                        state_symbol: state.symbol,
                                        state: state.name.clone(),
                                        guard: transition_guard,
                                    },
                                ));
                            }

                            collect_bounded_transition_argument_obligations(
                                program,
                                machine,
                                state,
                                transition_guard,
                                table_statements.get(statement_index),
                                &mut proof_plan,
                            );
                        }
                        StatementNode::AssemblyFact(_) | StatementNode::Expression(_) => {}
                    }
                    collect_nested_receiving_obligations(
                        program,
                        machine,
                        state,
                        statement_index,
                        statement,
                        &mut proof_plan,
                    );
                }
            }
        }

        proof_plan
    }

    pub(crate) fn estimated_proof_obligation_capacity(
        program: &dyn ::validation::CheckingInput,
    ) -> usize {
        let mut capacity = 0usize;

        for machine in program.machines() {
            capacity = capacity.saturating_add(program.machine_owned_data(machine).len() * 2);

            for state in program.machine_states(machine) {
                capacity = capacity.saturating_add(program.state_parameters(state).len());

                if state.return_type.is_valid() {
                    capacity = capacity.saturating_add(2);
                }

                for statement in program.statement_table.statements(state.statement_nodes) {
                    match statement {
                        StatementNode::Assignment(_) => capacity = capacity.saturating_add(1),
                        StatementNode::Call(call) => {
                            capacity = capacity.saturating_add(
                                statement_table(program)
                                    .expression_handles(call.arguments)
                                    .len(),
                            );
                        }
                        StatementNode::Transition(transition) => {
                            if matches!(transition.guard, TransitionGuardNode::When(_)) {
                                capacity = capacity.saturating_add(1);
                            }

                            let argument_count = table_transition_target_state_and_arguments(
                                program,
                                state,
                                transition.target,
                            )
                            .map_or(0, |(_, arguments)| arguments.len());
                            capacity = capacity.saturating_add(argument_count);
                        }
                        _ => {}
                    }
                }
            }
        }

        capacity
    }

    fn walk_bounded_type<Context: Clone>(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
        context: Context,
        on_constrained: &mut impl FnMut(Context, TypeReferenceHandle, HandleSpan<TypeConstraintNode>),
    ) {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                walk_bounded_type(program, *referee, context, on_constrained);
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => on_constrained(context, *base_type, *constraints),
            TypeReferenceNode::FixedArray { element_type, .. } => {
                walk_bounded_type(program, *element_type, context, on_constrained);
            }
            TypeReferenceNode::Slice { element_type } => {
                walk_bounded_type(program, *element_type, context, on_constrained);
            }
            TypeReferenceNode::Generic { arguments, .. } => {
                for argument in type_reference_table(program).type_reference_handles(*arguments) {
                    walk_bounded_type(program, *argument, context.clone(), on_constrained);
                }
            }
            TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Named { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => {}
        }
    }

    fn collect_bounded_value_obligation(
        program: &dyn ::validation::CheckingInput,
        owner: ProofObligationOwner,
        type_reference: TypeReferenceHandle,
        proof_plan: &mut ProofPlan<'_>,
    ) {
        walk_bounded_type(
            program,
            type_reference,
            owner,
            &mut |owner, base_type, constraints| {
                let constraints =
                    proof_plan.store_constraint_nodes(program, base_type, constraints);
                proof_plan.push_obligation(ProofObligation::BoundedValue(BoundedValueObligation {
                    owner,
                    base_type,
                    constraints,
                }));
            },
        );
    }

    fn collect_bounded_initializer_obligation(
        program: &dyn ::validation::CheckingInput,
        owner: ProofObligationOwner,
        type_reference: TypeReferenceHandle,
        value: ExpressionHandle,
        proof_plan: &mut ProofPlan<'_>,
    ) {
        walk_bounded_type(
            program,
            type_reference,
            owner,
            &mut |owner, base_type, constraints| {
                let constraints =
                    proof_plan.store_constraint_nodes(program, base_type, constraints);
                proof_plan.push_obligation(ProofObligation::BoundedInitializer(
                    BoundedInitializerObligation {
                        owner,
                        value,
                        value_constraints: HandleSpan::empty(),
                        base_type,
                        constraints,
                    },
                ));
            },
        );
    }

    fn collect_bounded_assignment_obligation(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        assignment: &TableAssignment,
        statements: &[StatementNode],
        statement_index: usize,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        incoming_guard: &mut Option<Option<(TransitionGuardNode, SymbolHandle)>>,
        proof_plan: &mut ProofPlan<'_>,
    ) {
        let ensures_witness_bounds = ensures_witness_bounds_at(
            program,
            machine,
            state,
            statements,
            statement_index,
            call_frames,
        );
        let target = assignment.target;
        let Some(target_type) = expression_type_reference(program, machine, state, target) else {
            return;
        };
        let collection_window = expression_table(program)
            .indexed_node(target)
            .filter(|indexed| program.expression_table.range_node(indexed.index).is_some());
        let construction_type = collection_window
            .and_then(|indexed| {
                expression_type_reference(program, machine, state, indexed.collection)
            })
            .unwrap_or(target_type);
        collect_array_element_obligations(
            program,
            machine,
            state,
            statement_index,
            assignment.value,
            construction_type,
            proof_plan,
        );
        if collection_window.is_some() {
            return;
        }
        let bounded_target = constrained_type_reference(program, target_type);
        let (base_type, constraints) = bounded_target
            .map(|(base_type, constraints)| {
                (
                    base_type,
                    proof_plan.store_constraint_nodes_with_domain_intervals(
                        program,
                        base_type,
                        constraints,
                    ),
                )
            })
            .unwrap_or((target_type, HandleSpan::empty()));

        let value = assignment.value;
        let value_constraints =
            proof_plan.store_constraints(expression_constraints(program, machine, state, value));
        let (state_guard, state_guard_source) = match *incoming_guard
            .get_or_insert_with(|| incoming_state_guard(program, machine, state))
        {
            Some((guard, source)) => (Some(guard), source),
            None => (None, SymbolHandle::invalid()),
        };
        let binary_operands = binary_value_operands(program, machine, state, value);

        let obligation = BoundedAssignmentObligation {
            machine_symbol: machine.symbol,
            machine: machine.name.clone(),
            state_symbol: state.symbol,
            state: state.name.clone(),
            statement_index,
            state_guard,
            state_guard_source,
            target,
            value,
            value_constraints,
            base_type,
            constraints,
            binary_operands,
            ensures_witness_bounds,
        };
        proof_plan
            .assignment_value_ranges
            .append(obligation.clone());
        if bounded_target.is_some() {
            proof_plan.push_obligation(ProofObligation::BoundedAssignment(obligation));
        }
    }

    fn binary_value_operands(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        value: ExpressionHandle,
    ) -> Option<BinaryValueOperands> {
        match program.expression_table.expression(value) {
            ExpressionNode::Binary(binary) => {
                let operand_range = |operand: ExpressionHandle| {
                    integer_range_from_constraints(&expression_constraints(
                        program, machine, state, operand,
                    ))
                };
                let left = dehoisted_operand(program, state, binary.left);
                let right = dehoisted_operand(program, state, binary.right);
                Some(BinaryValueOperands {
                    operator: binary.operator,
                    left,
                    left_range: operand_range(left),
                    right,
                    right_range: operand_range(right),
                })
            }
            _ => None,
        }
    }

    pub(crate) fn declared_integer_range(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<IntegerRange> {
        integer_range_from_constraints(&expression_constraints(program, machine, state, expression))
    }

    fn retire_written_witnesses(witnesses: &mut Vec<(String, i64)>, written: Option<&[String]>) {
        let Some(written) = written else {
            witnesses.clear();
            return;
        };
        witnesses.retain(|(place, _)| {
            written
                .iter()
                .all(|written| !validation::frame_paths_overlap(place, written))
        });
    }

    pub(crate) fn ensures_witness_bounds_at<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program Machine,
        state: &State,
        statements: &'program [StatementNode],
        upto: usize,
        call_frames: Option<&validation::CallFrameResolver<'program>>,
    ) -> Vec<(String, i64)> {
        let mut witnesses: Vec<(String, i64)> = Vec::new();
        for statement in &statements[..upto] {
            if !witnesses.is_empty() {
                let written = call_frames
                    .and_then(|frames| frames.statement_value_may_write_paths(machine, statement));
                retire_written_witnesses(&mut witnesses, written.as_deref());
            }
            match statement {
                StatementNode::Call(call) => {
                    let written =
                        call_frames.and_then(|frames| frames.may_write_paths(machine, call));
                    retire_written_witnesses(&mut witnesses, written.as_deref());
                    let Some(signature) = called_boundary_signature(program, call) else {
                        continue;
                    };
                    let Some(origins) =
                        call_frames.and_then(|frames| frames.call_write_origins(machine, call))
                    else {
                        continue;
                    };
                    for conjunct in validation::ensures_contract_expressions(program, signature) {
                        collect_witness_conjunct(
                            program,
                            state,
                            signature,
                            &origins,
                            conjunct,
                            &mut witnesses,
                        );
                    }
                }
                StatementNode::Assignment(_) if !witnesses.is_empty() => {
                    let written = call_frames.and_then(|frames| {
                        frames
                            .assignment_write_frame(machine, statement)
                            .into_complete_paths()
                    });
                    retire_written_witnesses(&mut witnesses, written.as_deref());
                }
                _ => {}
            }
        }
        witnesses
    }

    fn collect_witness_conjunct(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        signature: &typed_trees::signature::StateSignature,
        origins: &validation::CallWriteOrigins,
        conjunct: ExpressionHandle,
        witnesses: &mut Vec<(String, i64)>,
    ) {
        let mut comparisons = Vec::new();
        validation::collect_parameter_bound_comparisons(
            program,
            state,
            signature,
            origins,
            conjunct,
            &mut comparisons,
        );
        for (place, operator, literal) in comparisons {
            let inclusive_offset = match operator {
                BinaryOperator::LessOrEqual => 0,
                BinaryOperator::Less => -1,
                _ => continue,
            };
            let Some(bound) = literal.checked_add(inclusive_offset) else {
                continue;
            };
            witnesses.push((place, bound));
        }
    }

    fn collect_bounded_transition_argument_obligations(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        transition_guard: TransitionGuardNode,
        table_statement: Option<&StatementNode>,
        proof_plan: &mut ProofPlan<'_>,
    ) {
        let Some(StatementNode::Transition(table_transition)) = table_statement else {
            return;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        let Some(statement_index) = statements.iter().position(|statement| {
            table_statement.is_some_and(|current| std::ptr::eq(statement, current))
        }) else {
            return;
        };
        let refuted_exit_guards = prior_exit_guards(&statements[..statement_index]);
        let Some((target_state, arguments)) =
            table_transition_target_state_and_arguments(program, state, table_transition.target)
        else {
            return;
        };

        let arguments_are_call_free = arguments
            .iter()
            .all(|argument| !expression_contains_call_node(program, *argument));

        for (parameter, argument) in
            callable_parameters(program, target_state).zip(arguments.iter())
        {
            collect_array_element_obligations(
                program,
                machine,
                state,
                statement_index,
                *argument,
                parameter.type_reference,
                proof_plan,
            );
            let Some((base_type, constraints)) =
                constrained_type_reference(program, parameter.type_reference)
            else {
                continue;
            };

            let argument = *argument;
            let argument_constraint_buffer = if arguments_are_call_free {
                guarded_argument_constraints(program, machine, state, argument, &transition_guard)
            } else {
                expression_constraints(program, machine, state, argument)
            };
            let argument_constraints = proof_plan.store_constraints(argument_constraint_buffer);
            let constraints = proof_plan.store_constraint_nodes(program, base_type, constraints);

            proof_plan.push_obligation(ProofObligation::BoundedTransitionArgument(
                BoundedTransitionArgumentObligation {
                    machine_symbol: machine.symbol,
                    machine: machine.name.clone(),
                    state_symbol: state.symbol,
                    state: state.name.clone(),
                    statement_index,
                    parameter_symbol: parameter.symbol,
                    parameter: parameter.name.clone(),
                    argument,
                    argument_constraints,
                    base_type,
                    constraints,
                    guard: if arguments_are_call_free {
                        transition_guard
                    } else {
                        TransitionGuardNode::Always
                    },
                    refuted_exit_guards: if arguments_are_call_free {
                        refuted_exit_guards.clone()
                    } else {
                        Vec::new()
                    },
                },
            ));
        }
    }

    fn guarded_argument_constraints(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        argument: ExpressionHandle,
        guard: &TransitionGuardNode,
    ) -> ConstraintBuffer {
        match program.expression_table.expression(argument) {
            ExpressionNode::Member(_) => {
                payload_field_constraints_under_case_guard(program, argument, guard)
                    .unwrap_or_else(|| expression_constraints(program, machine, state, argument))
            }
            ExpressionNode::Binary(binary) => {
                let left =
                    guarded_argument_constraints(program, machine, state, binary.left, guard);
                let right =
                    guarded_argument_constraints(program, machine, state, binary.right, guard);
                derived_binary_constraints(binary.operator, &left, &right)
            }
            ExpressionNode::Cast(cast) => {
                guarded_argument_constraints(program, machine, state, cast.value, guard)
            }
            ExpressionNode::Unary(unary) => {
                guarded_argument_constraints(program, machine, state, unary.operand, guard)
            }
            _ => expression_constraints(program, machine, state, argument),
        }
    }

    fn payload_field_constraints_under_case_guard(
        program: &dyn ::validation::CheckingInput,
        argument: ExpressionHandle,
        guard: &TransitionGuardNode,
    ) -> Option<ConstraintBuffer> {
        let member = program.expression_table.member_node(argument)?;
        let TransitionGuardNode::When(condition) = guard else {
            return None;
        };
        let variant_symbol = case_guard_proven_variant(program, *condition, member.receiver)?;
        let payload_type_reference = variant_payload_field_type_reference(
            program,
            variant_symbol,
            member.member_symbol,
            &member.member,
        )?;
        let constraints = collect_constraints(program, payload_type_reference);
        (!constraints.is_empty()).then_some(constraints)
    }

    fn case_guard_proven_variant(
        program: &dyn ::validation::CheckingInput,
        condition: ExpressionHandle,
        receiver: ExpressionHandle,
    ) -> Option<SymbolHandle> {
        let binary = program.expression_table.binary_node(condition)?;
        match binary.operator {
            BinaryOperator::And => case_guard_proven_variant(program, binary.left, receiver)
                .or_else(|| case_guard_proven_variant(program, binary.right, receiver)),
            BinaryOperator::Equal => {
                if matches!(
                    program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(true)
                ) {
                    return case_guard_proven_variant(program, binary.left, receiver);
                }
                if matches!(
                    program.expression_table.expression(binary.left),
                    ExpressionNode::Boolean(true)
                ) {
                    return case_guard_proven_variant(program, binary.right, receiver);
                }
                let case_reference = program.expression_table.name_path_node(binary.right)?;
                let variant_symbol = case_reference.symbol;
                (variant_symbol.is_valid()
                    && expressions_equivalent_for_precondition(program, binary.left, receiver))
                .then_some(variant_symbol)
            }
            _ => None,
        }
    }

    fn variant_payload_field_type_reference(
        program: &dyn ::validation::CheckingInput,
        variant_symbol: SymbolHandle,
        member_symbol: SymbolHandle,
        member_name: &Identifier,
    ) -> Option<TypeReferenceHandle> {
        program
            .variant_owner(variant_symbol)
            .and_then(|(_, variant)| {
                program
                    .data_payload_fields(variant)
                    .iter()
                    .find_map(|field| {
                        ((member_symbol.is_valid() && field.symbol == member_symbol)
                            || field.name == *member_name)
                            .then_some(field.type_reference)
                    })
            })
    }

    struct CallOccurrence<'program> {
        target_symbol: SymbolHandle,
        target: Identifier,
        receiver: Option<Identifier>,
        arguments: &'program [ExpressionHandle],
    }

    fn collect_nested_receiving_obligations(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        statement: &StatementNode,
        proof_plan: &mut ProofPlan<'_>,
    ) {
        let mut calls = Vec::new();
        let mut fields = Vec::new();
        let mut visited = validation::expression_traversal::EnteredExpressions::default();
        for (_, root) in program.tables.expression_roots(statement) {
            visit_post_order(program, root, &mut visited, |expression, node| {
                if let ExpressionNode::Call(call) = node {
                    calls.push(CallOccurrence {
                        target_symbol: call.target_symbol,
                        target: call.target.clone(),
                        receiver: call.receiver.is_valid().then(|| {
                            Identifier::generated(
                                program.expression_table.display_name(call.receiver),
                            )
                        }),
                        arguments: program.expression_table.expression_handles(call.arguments),
                    });
                }
                if let ExpressionNode::StructLiteral(literal) = node {
                    for field in program.expression_table.struct_fields(literal.fields) {
                        let Some(reference) = validation::selected_construction_field_type(
                            program,
                            expression,
                            literal,
                            field.field_symbol,
                        ) else {
                            continue;
                        };
                        fields.push((field, reference));
                    }
                }
            });
        }
        for (field, reference) in fields {
            collect_receiving_initializer_obligation(
                program,
                machine,
                state,
                field.value,
                reference,
                ProofObligationOwner::ConstructorField {
                    machine_symbol: machine.symbol,
                    machine: machine.name.clone(),
                    state_symbol: state.symbol,
                    state: state.name.clone(),
                    field_symbol: field.field_symbol,
                    field: field.name.clone(),
                    statement_index,
                },
                proof_plan,
            );
        }
        for call in calls {
            collect_bounded_call_argument_obligations(
                program,
                machine,
                state,
                statement_index,
                call,
                proof_plan,
            );
        }
    }

    fn collect_bounded_call_argument_obligations(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        call: CallOccurrence<'_>,
        proof_plan: &mut ProofPlan<'_>,
    ) {
        let Some(parameters) = call_target_parameters(program, call.target_symbol) else {
            return;
        };

        for (parameter, argument) in parameters.iter().zip(call.arguments.iter()) {
            collect_array_element_obligations(
                program,
                machine,
                state,
                statement_index,
                *argument,
                parameter.type_reference,
                proof_plan,
            );
            let Some((base_type, constraints)) =
                constrained_type_reference(program, parameter.type_reference)
            else {
                continue;
            };

            let argument = *argument;
            let argument_constraints = proof_plan
                .store_constraints(expression_constraints(program, machine, state, argument));
            let constraints = proof_plan.store_constraint_nodes(program, base_type, constraints);

            proof_plan.push_obligation(ProofObligation::BoundedCallArgument(
                BoundedCallArgumentObligation {
                    machine_symbol: machine.symbol,
                    machine: machine.name.clone(),
                    state_symbol: state.symbol,
                    state: state.name.clone(),
                    statement_index,
                    receiver: call.receiver.clone(),
                    target_symbol: call.target_symbol,
                    target: call.target.clone(),
                    parameter_symbol: parameter.symbol,
                    parameter: parameter.name.clone(),
                    argument,
                    argument_constraints,
                    base_type,
                    constraints,
                },
            ));
        }
    }

    fn collect_bounded_local_initializer_obligation(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        local: &TableLocalData,
        proof_plan: &mut ProofPlan<'_>,
    ) {
        if local.ordinary_value_initializer().is_none()
            || matches!(
                type_reference_table(program).type_reference(local.type_reference),
                TypeReferenceNode::Reference { .. }
            )
        {
            return;
        }
        collect_receiving_initializer_obligation(
            program,
            machine,
            state,
            local.initial_value,
            local.type_reference,
            ProofObligationOwner::StateLocal {
                machine_symbol: machine.symbol,
                machine: machine.name.clone(),
                state_symbol: state.symbol,
                state: state.name.clone(),
                local_symbol: local.symbol,
                local: local.name.clone(),
                statement_index,
            },
            proof_plan,
        );
    }

    fn collect_receiving_initializer_obligation(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        value: ExpressionHandle,
        reference: TypeReferenceHandle,
        owner: ProofObligationOwner,
        proof_plan: &mut ProofPlan<'_>,
    ) {
        if let Some((_, _, statement_index)) = owner.state_statement() {
            collect_array_element_obligations(
                program,
                machine,
                state,
                statement_index,
                value,
                reference,
                proof_plan,
            );
        }
        if program.primitive_type_reference(reference).is_none() {
            return;
        }
        let Some((base_type, constraints)) = constrained_type_reference(program, reference) else {
            return;
        };
        let value_constraints =
            proof_plan.store_constraints(expression_constraints(program, machine, state, value));
        let constraints = proof_plan.store_constraint_nodes(program, base_type, constraints);
        proof_plan.push_obligation(ProofObligation::BoundedInitializer(
            BoundedInitializerObligation {
                owner,
                value,
                value_constraints,
                base_type,
                constraints,
            },
        ));
    }

    fn collect_array_element_obligations(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        value: ExpressionHandle,
        reference: TypeReferenceHandle,
        proof_plan: &mut ProofPlan<'_>,
    ) {
        if let Some(borrow) = program.expression_table.borrow_expression(value) {
            collect_array_element_obligations(
                program,
                machine,
                state,
                statement_index,
                borrow.target,
                reference,
                proof_plan,
            );
            return;
        }
        if let Some(dispatch) = program.expression_table.match_node(value) {
            for arm in program.expression_table.match_arms(dispatch.arms) {
                collect_array_element_obligations(
                    program,
                    machine,
                    state,
                    statement_index,
                    arm.value,
                    reference,
                    proof_plan,
                );
            }
            return;
        }
        let Some(elements) = program.expression_table.array_literal_node(value) else {
            return;
        };
        let Some(element_type) = validation::collection_element_type_reference(program, reference)
        else {
            return;
        };
        for (element_index, element) in expression_table(program)
            .expression_handles(*elements)
            .iter()
            .enumerate()
        {
            collect_receiving_initializer_obligation(
                program,
                machine,
                state,
                *element,
                element_type,
                ProofObligationOwner::ArrayElement {
                    machine_symbol: machine.symbol,
                    machine: machine.name.clone(),
                    state_symbol: state.symbol,
                    state: state.name.clone(),
                    statement_index,
                    construction: value,
                    element_index,
                },
                proof_plan,
            );
        }
    }

    fn prior_exit_guards(prior: &[StatementNode]) -> Vec<ExpressionHandle> {
        prior
            .iter()
            .filter_map(StatementNode::as_transition)
            .filter_map(|transition| match transition.guard {
                TransitionGuardNode::When(guard)
                    if transition.target.is_valid() && !transition.continuation.is_valid() =>
                {
                    Some(guard)
                }
                _ => None,
            })
            .collect()
    }

    fn collect_bounded_state_return_obligation(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        return_type: TypeReferenceHandle,
        proof_plan: &mut ProofPlan<'_>,
    ) {
        let statements = statement_table(program).statements(state.statement_nodes);
        for (statement_index, statement) in statements.iter().enumerate() {
            match statement {
                StatementNode::Expression(value) if statement_index + 1 == statements.len() => {
                    push_bounded_state_return_obligation(
                        program,
                        machine,
                        state,
                        statement_index,
                        *value,
                        TransitionGuardNode::Always,
                        Vec::new(),
                        return_type,
                        proof_plan,
                    );
                }
                StatementNode::Transition(transition) => {
                    let refuted = prior_exit_guards(&statements[..statement_index]);
                    for (target, guard) in [
                        (transition.target, transition.guard),
                        (transition.continuation, TransitionGuardNode::Always),
                    ] {
                        if !target.is_valid() {
                            continue;
                        }
                        let statement::TransitionTargetNode::Value(value) =
                            statement_table(program).transition_target(target)
                        else {
                            continue;
                        };
                        let call_free = !expression_contains_call_node(program, *value);
                        push_bounded_state_return_obligation(
                            program,
                            machine,
                            state,
                            statement_index,
                            *value,
                            if call_free {
                                guard
                            } else {
                                TransitionGuardNode::Always
                            },
                            if call_free {
                                refuted.clone()
                            } else {
                                Vec::new()
                            },
                            return_type,
                            proof_plan,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_bounded_state_return_obligation(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        value: ExpressionHandle,
        guard: TransitionGuardNode,
        refuted_exit_guards: Vec<ExpressionHandle>,
        return_type: TypeReferenceHandle,
        proof_plan: &mut ProofPlan<'_>,
    ) {
        collect_array_element_obligations(
            program,
            machine,
            state,
            statement_index,
            value,
            return_type,
            proof_plan,
        );
        let Some((base_type, constraints)) = constrained_type_reference(program, return_type)
        else {
            return;
        };
        let value_constraints =
            proof_plan.store_constraints(expression_constraints(program, machine, state, value));
        let constraints = proof_plan.store_constraint_nodes(program, base_type, constraints);

        proof_plan.push_obligation(ProofObligation::BoundedStateReturn(
            BoundedStateReturnObligation {
                machine_symbol: machine.symbol,
                machine: machine.name.clone(),
                state_symbol: state.symbol,
                state: state.name.clone(),
                statement_index,
                value,
                value_constraints,
                base_type,
                constraints,
                binary_operands: binary_value_operands(program, machine, state, value),
                guard,
                refuted_exit_guards,
            },
        ));
    }

    fn call_target_parameters(
        program: &dyn ::validation::CheckingInput,
        target_symbol: SymbolHandle,
    ) -> Option<&[StateParameter]> {
        program
            .machine_by_symbol(target_symbol)
            .and_then(|machine| program.machine_states(machine).first())
            .or_else(|| state_by_symbol(program, target_symbol))
            .map(|state| program.state_parameters(state))
            .or_else(|| {
                program
                    .machine_parameter_signature(target_symbol)
                    .map(|(_, signature)| program.state_signature_parameters(signature))
            })
    }

    fn display_name_path(path: &[Identifier]) -> Identifier {
        let mut display = String::new();

        for member in path {
            if !display.is_empty() {
                display.push('.');
            }
            display.push_str(member.as_str());
        }

        Identifier::generated(display)
    }

    fn state_by_symbol(
        program: &dyn ::validation::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&State> {
        program.state_by_symbol(symbol)
    }

    fn incoming_state_guard(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        target_state: &State,
    ) -> Option<(TransitionGuardNode, SymbolHandle)> {
        let mut guard: Option<(TransitionGuardNode, SymbolHandle)> = None;

        for source_state in program.machine_states(machine) {
            for statement in statement_table(program).statements(source_state.statement_nodes) {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };

                let Some((resolved_target, _)) = table_transition_target_state_and_arguments(
                    program,
                    source_state,
                    transition.target,
                ) else {
                    continue;
                };

                if resolved_target.symbol != target_state.symbol {
                    continue;
                }

                let transition_guard = transition.guard;
                let TransitionGuardNode::When(_) = &transition_guard else {
                    return None;
                };

                match &guard {
                    Some((existing, _))
                        if !guards_equivalent_for_precondition(
                            program,
                            existing,
                            &transition_guard,
                        ) =>
                    {
                        return None;
                    }
                    Some(_) => {}
                    None => guard = Some((transition_guard, source_state.symbol)),
                }
            }
        }

        guard
    }

    fn table_transition_target_state_and_arguments<'program>(
        program: &'program dyn ::validation::CheckingInput,
        state: &'program State,
        target: TransitionTargetHandle,
    ) -> Option<(&'program State, &'program [expression::ExpressionHandle])> {
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = program.statement_table.transition_target(target)
        else {
            return None;
        };
        let path_members = program.statement_table.name_path_members(path.members);

        state_by_symbol(program, path.symbol)
            .or_else(|| {
                matches!(path_members, [member] if member.is_self_receiver()).then_some(state)
            })
            .map(|target_state| {
                (
                    target_state,
                    program.statement_table.expression_handles(*arguments),
                )
            })
    }

    fn guards_equivalent_for_precondition(
        program: &dyn ::validation::CheckingInput,
        left: &TransitionGuardNode,
        right: &TransitionGuardNode,
    ) -> bool {
        match (left, right) {
            (TransitionGuardNode::Always, TransitionGuardNode::Always) => true,
            (TransitionGuardNode::When(left), TransitionGuardNode::When(right)) => {
                expressions_equivalent_for_precondition(program, *left, *right)
            }
            _ => false,
        }
    }

    fn expressions_equivalent_for_precondition(
        program: &dyn ::validation::CheckingInput,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        if left == right {
            return true;
        }

        match (
            program.expression_table.expression(left),
            program.expression_table.expression(right),
        ) {
            (ExpressionNode::Borrow(left), _) => {
                expressions_equivalent_for_precondition(program, left.target, right)
            }
            (_, ExpressionNode::Borrow(right)) => {
                expressions_equivalent_for_precondition(program, left, right.target)
            }
            (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
                program.expression_table.name_path_members(left.members)
                    == program.expression_table.name_path_members(right.members)
            }
            (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
                left.target == right.target
                    && left.target_symbol == right.target_symbol
                    && left.arguments.count() == right.arguments.count()
                    && match (left.receiver.is_valid(), right.receiver.is_valid()) {
                        (true, true) => expressions_equivalent_for_precondition(
                            program,
                            left.receiver,
                            right.receiver,
                        ),
                        (false, false) => true,
                        _ => false,
                    }
                    && expression_table(program)
                        .expression_handles(left.arguments)
                        .iter()
                        .zip(program.expression_table.expression_handles(right.arguments))
                        .all(|(left_argument, right_argument)| {
                            expressions_equivalent_for_precondition(
                                program,
                                *left_argument,
                                *right_argument,
                            )
                        })
            }
            (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
                left.member == right.member
                    && left.member_symbol == right.member_symbol
                    && expressions_equivalent_for_precondition(
                        program,
                        left.receiver,
                        right.receiver,
                    )
            }
            (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
                left.operator == right.operator
                    && expressions_equivalent_for_precondition(program, left.left, right.left)
                    && expressions_equivalent_for_precondition(program, left.right, right.right)
            }
            (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
            (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
            (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
            (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
            _ => false,
        }
    }

    fn callable_parameters<'program>(
        program: &'program dyn ::validation::CheckingInput,
        state: &'program State,
    ) -> impl Iterator<Item = &'program StateParameter> {
        program.state_parameters(state).iter()
    }
}
mod constraints {
    use crate::obligations::ProofConstraint;
    use crate::obligations::plan::ConstraintBuffer;
    use crate::obligations::ranges::{
        arithmetic_domain_from_constraints, constraints_prove_finite, float_binary_range,
        float_range_from_constraints, has_named_constraint, integer_binary_range,
        integer_constraints_are_exact, integer_constraints_are_wrapping,
        integer_range_from_constraints,
    };
    use crate::typed_expressions::{
        call_expression_return_type, expression_type_reference, type_reference_table,
    };
    use numerics::bignum::BigInt;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, FloatLiteral};
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::operator::{
        declaration_by_symbol, primitive_float_binary_semantics, resolve_named_expression_call,
    };
    use typed_trees::state::State;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    const RANGE_ARITHMETIC_OPERATORS: [BinaryOperator; 6] = [
        BinaryOperator::Add,
        BinaryOperator::Modulo,
        BinaryOperator::Multiply,
        BinaryOperator::ShiftLeft,
        BinaryOperator::ShiftRight,
        BinaryOperator::Subtract,
    ];

    pub(crate) fn expression_constraints(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> ConstraintBuffer {
        match program.expression_table.expression(expression) {
            ExpressionNode::SavedSubject(_) | ExpressionNode::Match(_) => ConstraintBuffer::new(),
            ExpressionNode::Atomic(atomic) => {
                expression_constraints(program, machine, state, atomic.value)
            }
            ExpressionNode::Binary(binary) => {
                let left = expression_constraints(program, machine, state, binary.left);
                let right = expression_constraints(program, machine, state, binary.right);
                derived_binary_constraints(binary.operator, &left, &right)
            }
            ExpressionNode::Range(range) => {
                let mut constraints = ConstraintBuffer::new();
                if range.start.is_valid() {
                    constraints.extend(expression_constraints(
                        program,
                        machine,
                        state,
                        range.start,
                    ));
                }
                if range.end.is_valid() {
                    constraints.extend(expression_constraints(program, machine, state, range.end));
                }
                constraints
            }
            ExpressionNode::Call(call) => {
                if let Some(constraints) =
                    derived_float_operator_constraints(program, machine, state, call)
                {
                    return constraints;
                }

                if let Some(operator) = resolve_named_expression_call(program, call)
                    && let Some((source, target)) =
                        validation::float_format_conversion_contract(program, operator)
                    && let [operand] = program.expression_table.expression_handles(call.arguments)
                {
                    let mut constraints =
                        collect_constraints_in_state(program, machine, state, operator.return_type);
                    constraints.extend(converted_float_constraints(
                        &expression_constraints(program, machine, state, *operand),
                        source,
                        target,
                        program.arithmetic_domain_for_type_reference(operator.return_type),
                    ));
                    return constraints;
                }
                if let Some(operator) = resolve_named_expression_call(program, call) {
                    let mut constraints =
                        collect_constraints_in_state(program, machine, state, operator.return_type);
                    if let [operand] = program.expression_table.expression_handles(call.arguments)
                        && let Some(source) = float_range_from_constraints(&expression_constraints(
                            program, machine, state, *operand,
                        ))
                        && let Some((minimum, maximum)) =
                            validation::float_integer_conversion_bounds(
                                program,
                                operator,
                                source.minimum,
                                source.maximum,
                            )
                    {
                        constraints.push(ProofConstraint::IntegerRange { minimum, maximum });
                    }
                    return constraints;
                }
                if let Some(return_type) =
                    call_expression_return_type(program, machine, state, call)
                {
                    return collect_constraints_in_state(program, machine, state, return_type);
                }

                ConstraintBuffer::new()
            }
            ExpressionNode::Cast(cast) => {
                let mut constraints = expression_constraints(program, machine, state, cast.value);
                if expression_type_reference(program, machine, state, cast.value)
                    .and_then(|source| program.primitive_type_reference(source))
                    == Some(PrimitiveType::Bool)
                    && program
                        .primitive_type_reference(cast.target_type)
                        .is_some_and(|primitive| primitive.accepts_integer_literal())
                {
                    if !has_named_constraint(&constraints, "non_negative") {
                        constraints.push(ProofConstraint::Named(Identifier::generated_static(
                            "non_negative",
                        )));
                    }
                    constraints.push(ProofConstraint::IntegerRange {
                        minimum: BigInt::zero(),
                        maximum: BigInt::from_i64(1),
                    });
                }
                constraints
            }
            ExpressionNode::Unary(unary) => {
                expression_constraints(program, machine, state, unary.operand)
            }
            ExpressionNode::Float(value) => float_literal_constraints(value),
            ExpressionNode::UndeterminedNaN(_) => ConstraintBuffer::new(),
            ExpressionNode::Integer(value) => integer_literal_constraints(value),
            ExpressionNode::Indexed(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Borrow(_)
            | ExpressionNode::Name(_) => {
                expression_type_reference(program, machine, state, expression)
                    .map(|type_reference| {
                        collect_constraints_in_state(program, machine, state, type_reference)
                    })
                    .unwrap_or_default()
            }
            ExpressionNode::ArrayLiteral(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::String(_)
            | ExpressionNode::StructLiteral(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => ConstraintBuffer::new(),
        }
    }

    fn float_format(primitive: PrimitiveType) -> Option<numerics::float_semantics::FloatFormat> {
        match primitive {
            PrimitiveType::F32 => Some(numerics::float_semantics::FloatFormat::BINARY32),
            PrimitiveType::F64 => Some(numerics::float_semantics::FloatFormat::BINARY64),
            _ => None,
        }
    }

    fn converted_float_constraints(
        source_constraints: &ConstraintBuffer,
        source: PrimitiveType,
        target: PrimitiveType,
        domain: numerics::arithmetic::ArithmeticDomain,
    ) -> ConstraintBuffer {
        use numerics::float_semantics::{FloatMeaning, FloatSemantics};

        let mut constraints = ConstraintBuffer::new();
        if domain != numerics::arithmetic::ArithmeticDomain::Exact {
            constraints.push(ProofConstraint::ArithmeticDomain(domain));
        }
        let Some(target_format) = float_format(target) else {
            return constraints;
        };
        let mut source_range = float_range_from_constraints(source_constraints);
        if constraints_prove_finite(source_constraints) {
            let largest = match source {
                PrimitiveType::F32 => f64::from(f32::MAX),
                PrimitiveType::F64 => f64::MAX,
                _ => return constraints,
            };
            let finite = crate::obligations::FloatRange {
                minimum: -largest,
                maximum: largest,
                maximum_inclusive: true,
            };
            source_range = Some(source_range.map_or(finite, |range| range.intersect(finite)));
        }
        let Some(source_range) = source_range else {
            return constraints;
        };
        let minimum =
            FloatSemantics::convert(target_format, &FloatMeaning::from_f64(source_range.minimum))
                .to_f64();
        let maximum =
            FloatSemantics::convert(target_format, &FloatMeaning::from_f64(source_range.maximum))
                .to_f64();
        let narrowed = source == PrimitiveType::F64 && target == PrimitiveType::F32;
        let converted = crate::obligations::FloatRange {
            minimum,
            maximum,
            maximum_inclusive: narrowed || source_range.maximum_inclusive,
        };
        if converted.proves_finite() {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "finite",
            )));
        }
        constraints.push(ProofConstraint::FloatRange {
            minimum: FloatLiteral::new(converted.minimum),
            maximum: FloatLiteral::new(converted.maximum),
            maximum_inclusive: converted.maximum_inclusive,
        });
        constraints
    }

    pub(crate) fn collect_constraints(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> ConstraintBuffer {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => collect_constraints(program, *referee),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let mut derived = collect_constraints(program, *base_type);
                derived.extend_iter(
                    type_reference_table(program)
                        .constraints(*constraints)
                        .iter()
                        .filter_map(|constraint| {
                            ProofConstraint::from_node_with_domain_intervals(
                                program, *base_type, constraint,
                            )
                        }),
                );
                augment_constraints_with_named_facts(&mut derived);
                derived
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                collect_constraints(program, *element_type)
            }
            TypeReferenceNode::Generic { arguments, .. } => type_reference_table(program)
                .type_reference_handles(*arguments)
                .iter()
                .flat_map(|argument| collect_constraints(program, *argument))
                .collect(),
            TypeReferenceNode::Slice { element_type } => {
                collect_constraints(program, *element_type)
            }
            TypeReferenceNode::DynamicTrait { .. } => ConstraintBuffer::new(),
            TypeReferenceNode::Named { name, .. } => primitive_constraints(name),
            TypeReferenceNode::ConstExpression(_) => ConstraintBuffer::new(),
            TypeReferenceNode::Unit => ConstraintBuffer::new(),
        }
    }

    fn collect_constraints_in_state(
        program: &dyn ::validation::CheckingInput,
        _machine: &Machine,
        _state: &State,
        type_reference: TypeReferenceHandle,
    ) -> ConstraintBuffer {
        collect_constraints(program, type_reference)
    }

    fn primitive_constraints(name: &Identifier) -> ConstraintBuffer {
        let mut constraints = ConstraintBuffer::new();
        let range = match name.as_str() {
            "u8" => Some((BigInt::zero(), BigInt::from_u64(u8::MAX as u64))),
            "u16" => Some((BigInt::zero(), BigInt::from_u64(u16::MAX as u64))),
            "u32" => Some((BigInt::zero(), BigInt::from_u64(u32::MAX as u64))),
            "u64" => Some((BigInt::zero(), BigInt::from_u64(u64::MAX))),
            _ => None,
        };
        if let Some((minimum, maximum)) = range {
            constraints.push(ProofConstraint::IntegerRange { minimum, maximum });
        }
        augment_constraints_with_named_facts(&mut constraints);
        constraints
    }

    fn integer_literal_constraints(
        literal: &numerics::literals::IntegerLiteral,
    ) -> ConstraintBuffer {
        let mut constraints = ConstraintBuffer::new();
        let Some(value) = literal.value_bignum() else {
            return constraints;
        };
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "exact",
        )));

        if !value.is_negative() {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "non_negative",
            )));
        }

        if !value.is_negative() && !value.is_zero() {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "positive",
            )));
        }

        constraints.push(ProofConstraint::IntegerRange {
            minimum: value.clone(),
            maximum: value,
        });

        constraints
    }

    fn float_literal_constraints(value: &FloatLiteral) -> ConstraintBuffer {
        let value = value.landed_f64();
        if !value.is_finite() {
            return ConstraintBuffer::new();
        }

        let mut constraints = ConstraintBuffer::new();
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "finite",
        )));
        constraints.push(ProofConstraint::FloatRange {
            minimum: FloatLiteral::new(value),
            maximum: FloatLiteral::new(value),
            maximum_inclusive: true,
        });
        constraints
    }

    pub(crate) fn derived_binary_constraints(
        operator: BinaryOperator,
        left_constraints: &ConstraintBuffer,
        right_constraints: &ConstraintBuffer,
    ) -> ConstraintBuffer {
        let mut constraints = ConstraintBuffer::new();
        let arithmetic_domain = arithmetic_domain_from_constraints(left_constraints)
            .combine(arithmetic_domain_from_constraints(right_constraints));
        if arithmetic_domain != numerics::arithmetic::ArithmeticDomain::Exact {
            constraints.push(ProofConstraint::ArithmeticDomain(arithmetic_domain));
        }

        if arithmetic_domain == numerics::arithmetic::ArithmeticDomain::Saturating
            && constraints_prove_finite(left_constraints)
            && constraints_prove_finite(right_constraints)
            && matches!(
                operator,
                BinaryOperator::Add | BinaryOperator::Multiply | BinaryOperator::Subtract
            )
        {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "finite",
            )));
        }

        if integer_constraints_are_exact(left_constraints)
            && integer_constraints_are_exact(right_constraints)
            && RANGE_ARITHMETIC_OPERATORS.contains(&operator)
        {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "exact",
            )));
        }

        if integer_constraints_are_wrapping(left_constraints)
            && RANGE_ARITHMETIC_OPERATORS.contains(&operator)
        {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "wrapping",
            )));
        }

        if let (Some(left_range), Some(right_range)) = (
            integer_range_from_constraints(left_constraints),
            integer_range_from_constraints(right_constraints),
        ) && let Some(range) = integer_binary_range(operator, left_range, right_range)
        {
            if !range.minimum.is_negative() {
                constraints.push(ProofConstraint::Named(Identifier::generated_static(
                    "non_negative",
                )));
            }
            if !range.minimum.is_negative() && !range.minimum.is_zero() {
                constraints.push(ProofConstraint::Named(Identifier::generated_static(
                    "positive",
                )));
            }
            constraints.push(ProofConstraint::IntegerRange {
                minimum: range.minimum,
                maximum: range.maximum,
            });
        }

        if let (Some(left_range), Some(right_range)) = (
            float_range_from_constraints(left_constraints),
            float_range_from_constraints(right_constraints),
        ) && let Some(range) = float_binary_range(operator, left_range, right_range)
        {
            if range.proves_finite() {
                constraints.push(ProofConstraint::Named(Identifier::generated_static(
                    "finite",
                )));
            }
            constraints.push(ProofConstraint::FloatRange {
                minimum: FloatLiteral::new(range.minimum),
                maximum: FloatLiteral::new(range.maximum),
                maximum_inclusive: range.maximum_inclusive,
            });
        }

        constraints
    }

    fn derived_float_operator_constraints(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        call: &typed_trees::expression::TableCallExpression,
    ) -> Option<ConstraintBuffer> {
        if !matches!(
            call.target.as_str(),
            "add" | "subtract" | "multiply" | "divide"
        ) {
            return None;
        }
        let owner = program.machine_holding_state(call.target_symbol)?;
        if !matches!(
            owner.spelling,
            Some(language_core::operator_spelling::OperatorSpelling::Add)
                | Some(language_core::operator_spelling::OperatorSpelling::Subtract)
                | Some(language_core::operator_spelling::OperatorSpelling::Multiply)
                | Some(language_core::operator_spelling::OperatorSpelling::Divide)
        ) {
            return None;
        }
        let operator = declaration_by_symbol(program, owner.symbol)?;
        let (operation, _format) = primitive_float_binary_semantics(program, operator)?;
        let [left, right] = program.expression_table.expression_handles(call.arguments) else {
            return None;
        };
        let left_constraints = expression_constraints(program, machine, state, *left);
        let right_constraints = expression_constraints(program, machine, state, *right);
        Some(derived_binary_constraints(
            operation,
            &left_constraints,
            &right_constraints,
        ))
    }

    fn augment_constraints_with_named_facts(constraints: &mut ConstraintBuffer) {
        if constraints.iter().any(|constraint| {
            matches!(
                constraint,
                ProofConstraint::IntegerRange { minimum, maximum } if minimum == maximum
            ) || matches!(
                constraint,
                ProofConstraint::FloatRange {
                    minimum,
                    maximum,
                    maximum_inclusive,
                } if minimum == maximum && *maximum_inclusive
            )
        }) && !has_named_constraint(constraints, "exact")
        {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "exact",
            )));
        }

        if let Some(range) = integer_range_from_constraints(constraints) {
            if !range.minimum.is_negative() && !has_named_constraint(constraints, "non_negative") {
                constraints.push(ProofConstraint::Named(Identifier::generated_static(
                    "non_negative",
                )));
            }
            if !range.minimum.is_negative()
                && !range.minimum.is_zero()
                && !has_named_constraint(constraints, "positive")
            {
                constraints.push(ProofConstraint::Named(Identifier::generated_static(
                    "positive",
                )));
            }
        }

        if float_range_from_constraints(&*constraints).is_some_and(|range| range.proves_finite())
            && !has_named_constraint(constraints, "finite")
        {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "finite",
            )));
        }
    }
}
mod identity {
    use crate::obligations::plan::{
        BinaryValueOperands, IntegerRange, ProofConstraint, ProofObligation, ProofObligationOwner,
        ProofPlan,
    };
    use crate::typed_expressions::{expression_table, statement_table, type_reference_table};
    use arena::HandleSpan;
    use std::fmt;
    use symbols::JoinedPath;
    use symbols::SymbolHandle;
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, FloatLiteral, MatchPattern,
        QuotientOperationRequest, StaticMachineArgument, StaticSymbolApplication,
        TableCallExpression, TableNamePath,
    };
    use typed_trees::statement::TransitionGuardNode;
    use typed_trees::typed_trees::StaticRequirementDispatch;

    macro_rules! atom {
        ($tag:expr => $value:expr) => {
            atom($tag, $value)
        };
    }

    pub const KEY_SCHEMA: u32 = 2;

    struct IdentityContext<'program> {
        program: &'program dyn validation::CheckingInput,
        unavailable: &'program std::cell::Cell<bool>,
        captures: &'program [arena::Handle<typed_trees::expression::CompletedValueCapture>],
    }

    impl<'program> std::ops::Deref for IdentityContext<'program> {
        type Target = dyn validation::CheckingInput + 'program;
        fn deref(&self) -> &Self::Target {
            self.program
        }
    }

    #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct ProofObligationKey(String);

    impl ProofObligationKey {
        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    impl fmt::Debug for ProofObligationKey {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt::Display::fmt(self, formatter)
        }
    }

    impl fmt::Display for ProofObligationKey {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(&self.0)
        }
    }

    pub fn proof_obligation_key(
        proof_plan: &ProofPlan<'_>,
        obligation: &ProofObligation,
    ) -> Option<ProofObligationKey> {
        let unavailable = std::cell::Cell::new(false);
        let context = IdentityContext {
            program: proof_plan.program,
            unavailable: &unavailable,
            captures: &[],
        };
        let program = &context;
        let body = match obligation {
            ProofObligation::BoundedAssignment(obligation) => compound(
                "bounded-assignment",
                [
                    machine_state_identity(
                        program,
                        obligation.machine_symbol,
                        obligation.state_symbol,
                    ),
                    atom!("guard-source" => symbol_identity(program, obligation.state_guard_source)),
                    obligation
                        .state_guard
                        .as_ref()
                        .map(|guard| guard_identity(program, guard))
                        .unwrap_or_else(|| "unguarded".to_owned()),
                    compound("target", [expression_identity(program, obligation.target)]),
                    compound("value", [expression_identity(program, obligation.value)]),
                    constraints_identity(
                        proof_plan,
                        obligation.value_constraints,
                        "value-constraints",
                    ),
                    atom!("base" => program.package_qualified_type_identity_text(obligation.base_type)),
                    constraints_identity(proof_plan, obligation.constraints, "constraints"),
                    binary_operands_identity(program, &obligation.binary_operands),
                    compound(
                        "witness-bounds",
                        obligation
                            .ensures_witness_bounds
                            .iter()
                            .map(|(place, bound)| {
                                compound("witness", [atom("place", place), bound.to_string()])
                            }),
                    ),
                ],
            ),
            ProofObligation::BoundedCallArgument(obligation) => compound(
                "bounded-call-argument",
                [
                    machine_state_identity(
                        program,
                        obligation.machine_symbol,
                        obligation.state_symbol,
                    ),
                    obligation
                        .receiver
                        .as_ref()
                        .map(|receiver| atom("receiver", receiver.as_str()))
                        .unwrap_or_else(|| "receiver:none".to_owned()),
                    atom("target", symbol_identity(program, obligation.target_symbol)),
                    atom!("parameter" => symbol_identity(program, obligation.parameter_symbol)),
                    compound(
                        "argument",
                        [expression_identity(program, obligation.argument)],
                    ),
                    constraints_identity(
                        proof_plan,
                        obligation.argument_constraints,
                        "argument-constraints",
                    ),
                    atom!("base" => program.package_qualified_type_identity_text(obligation.base_type)),
                    constraints_identity(proof_plan, obligation.constraints, "constraints"),
                ],
            ),
            ProofObligation::BoundedInitializer(obligation) => compound(
                "bounded-initializer",
                [
                    owner_identity(program, &obligation.owner),
                    compound("value", [expression_identity(program, obligation.value)]),
                    constraints_identity(
                        proof_plan,
                        obligation.value_constraints,
                        "value-constraints",
                    ),
                    atom!("base" => program.package_qualified_type_identity_text(obligation.base_type)),
                    constraints_identity(proof_plan, obligation.constraints, "constraints"),
                ],
            ),
            ProofObligation::BoundedStateReturn(obligation) => compound(
                "bounded-state-return",
                [
                    machine_state_identity(
                        program,
                        obligation.machine_symbol,
                        obligation.state_symbol,
                    ),
                    compound("value", [expression_identity(program, obligation.value)]),
                    constraints_identity(
                        proof_plan,
                        obligation.value_constraints,
                        "value-constraints",
                    ),
                    atom!("base" => program.package_qualified_type_identity_text(obligation.base_type)),
                    constraints_identity(proof_plan, obligation.constraints, "constraints"),
                    binary_operands_identity(program, &obligation.binary_operands),
                    guard_identity(program, &obligation.guard),
                    compound(
                        "refuted",
                        obligation
                            .refuted_exit_guards
                            .iter()
                            .map(|guard| expression_identity(program, *guard)),
                    ),
                ],
            ),
            ProofObligation::BoundedValue(obligation) => compound(
                "bounded-value",
                [
                    owner_identity(program, &obligation.owner),
                    atom!("base" => program.package_qualified_type_identity_text(obligation.base_type)),
                    constraints_identity(proof_plan, obligation.constraints, "constraints"),
                ],
            ),
            ProofObligation::BoundedTransitionArgument(obligation) => compound(
                "bounded-transition-argument",
                [
                    machine_state_identity(
                        program,
                        obligation.machine_symbol,
                        obligation.state_symbol,
                    ),
                    atom!("parameter" => symbol_identity(program, obligation.parameter_symbol)),
                    compound(
                        "argument",
                        [expression_identity(program, obligation.argument)],
                    ),
                    constraints_identity(
                        proof_plan,
                        obligation.argument_constraints,
                        "argument-constraints",
                    ),
                    atom!("base" => program.package_qualified_type_identity_text(obligation.base_type)),
                    constraints_identity(proof_plan, obligation.constraints, "constraints"),
                    guard_identity(program, &obligation.guard),
                    compound(
                        "refuted",
                        obligation
                            .refuted_exit_guards
                            .iter()
                            .map(|guard| expression_identity(program, *guard)),
                    ),
                ],
            ),
            ProofObligation::GuardedTransition(obligation) => compound(
                "guarded-transition",
                [
                    machine_state_identity(
                        program,
                        obligation.machine_symbol,
                        obligation.state_symbol,
                    ),
                    guard_identity(program, &obligation.guard),
                ],
            ),
        };
        (!unavailable.get()).then(|| {
            ProofObligationKey(compound(
                "obligation",
                [atom("schema", KEY_SCHEMA.to_string()), body],
            ))
        })
    }

    fn machine_state_identity(
        program: &IdentityContext<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> String {
        compound(
            "in",
            [
                atom("machine", symbol_identity(program, machine)),
                atom("state", symbol_identity(program, state)),
            ],
        )
    }

    fn owner_identity(program: &IdentityContext<'_>, owner: &ProofObligationOwner) -> String {
        match owner {
            ProofObligationOwner::Unknown => "owner:unknown".to_owned(),
            ProofObligationOwner::MachineOwnedData {
                machine_symbol,
                data_symbol,
                ..
            } => compound(
                "owner",
                [
                    atom!("machine-data-machine" => symbol_identity(program, *machine_symbol)),
                    atom("machine-data", symbol_identity(program, *data_symbol)),
                ],
            ),
            ProofObligationOwner::StateParameter {
                machine_symbol,
                state_symbol,
                parameter_symbol,
                ..
            } => compound(
                "owner",
                [
                    atom!("state-param-machine" => symbol_identity(program, *machine_symbol)),
                    atom("state-param-state", symbol_identity(program, *state_symbol)),
                    atom("state-param", symbol_identity(program, *parameter_symbol)),
                ],
            ),
            ProofObligationOwner::StateReturn {
                machine_symbol,
                state_symbol,
                ..
            } => compound(
                "owner",
                [
                    atom!("state-return-machine" => symbol_identity(program, *machine_symbol)),
                    atom!("state-return-state" => symbol_identity(program, *state_symbol)),
                ],
            ),
            ProofObligationOwner::StateLocal {
                machine_symbol,
                state_symbol,
                local_symbol,
                statement_index,
                ..
            } => compound(
                "owner",
                [
                    atom!("state-local-machine" => symbol_identity(program, *machine_symbol)),
                    atom("state-local-state", symbol_identity(program, *state_symbol)),
                    atom("state-local", symbol_identity(program, *local_symbol)),
                    atom("state-local-statement", statement_index.to_string()),
                ],
            ),
            ProofObligationOwner::ArrayElement {
                machine_symbol,
                state_symbol,
                statement_index,
                construction,
                element_index,
                ..
            } => compound(
                "owner",
                [
                    atom("array-machine", symbol_identity(program, *machine_symbol)),
                    atom("array-state", symbol_identity(program, *state_symbol)),
                    atom("array-statement", statement_index.to_string()),
                    atom!("array-construction" => expression_identity(program, *construction)),
                    atom(
                        "array-occurrence",
                        expression_occurrence_identity(
                            program,
                            *machine_symbol,
                            *state_symbol,
                            *statement_index,
                            *construction,
                        ),
                    ),
                    atom("array-element", element_index.to_string()),
                ],
            ),
            ProofObligationOwner::ConstructorField {
                machine_symbol,
                state_symbol,
                field_symbol,
                statement_index,
                ..
            } => compound(
                "owner",
                [
                    atom!("constructor-machine" => symbol_identity(program, *machine_symbol)),
                    atom("constructor-state", symbol_identity(program, *state_symbol)),
                    atom("constructor-field", symbol_identity(program, *field_symbol)),
                    atom("constructor-statement", statement_index.to_string()),
                ],
            ),
        }
    }

    fn expression_occurrence_identity(
        program: &IdentityContext<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> String {
        let occurrence = (|| {
            let (_, state) = program.machine_state(machine, state)?;
            let statement = statement_table(program)
                .statements(state.statement_nodes)
                .get(statement_index)?;
            let mut expressions = Vec::new();
            validation::expression_traversal::collect_statement_expression_trees(
                program.program,
                statement,
                &mut expressions,
            );
            expressions
                .iter()
                .position(|candidate| *candidate == expression)
        })();
        format!("{occurrence:?}")
    }

    fn constraints_identity(
        proof_plan: &ProofPlan<'_>,
        constraints: HandleSpan<ProofConstraint>,
        tag: &str,
    ) -> String {
        compound(
            tag,
            proof_plan
                .type_constraints
                .span_or_empty(constraints)
                .iter()
                .map(constraint_identity),
        )
    }

    fn constraint_identity(constraint: &ProofConstraint) -> String {
        match constraint {
            ProofConstraint::Named(name) => atom("named", name.as_str()),
            ProofConstraint::IntegerRange { minimum, maximum } => {
                compound("integer-range", [minimum.to_string(), maximum.to_string()])
            }

            ProofConstraint::FloatRange {
                minimum,
                maximum,
                maximum_inclusive,
            } => compound(
                "float-range",
                [
                    float_identity(minimum),
                    float_identity(maximum),
                    maximum_inclusive.to_string(),
                ],
            ),
            ProofConstraint::ArithmeticDomain(domain) => atom("arithmetic-domain", domain.name()),
        }
    }

    fn float_identity(literal: &FloatLiteral) -> String {
        format!("{:016x}", literal.value_f64().to_bits())
    }

    fn guard_identity(program: &IdentityContext<'_>, guard: &TransitionGuardNode) -> String {
        match guard {
            TransitionGuardNode::Always => "guard:always".to_owned(),
            TransitionGuardNode::When(condition) => {
                compound("guard-when", [expression_identity(program, *condition)])
            }
        }
    }

    fn binary_operands_identity(
        program: &IdentityContext<'_>,
        operands: &Option<BinaryValueOperands>,
    ) -> String {
        match operands {
            None => "binary-operands:none".to_owned(),
            Some(operands) => compound(
                "binary-operands",
                [
                    binary_operator_tag(operands.operator).to_owned(),
                    expression_identity(program, operands.left),
                    integer_range_identity(operands.left_range.as_ref()),
                    expression_identity(program, operands.right),
                    integer_range_identity(operands.right_range.as_ref()),
                ],
            ),
        }
    }

    fn integer_range_identity(range: Option<&IntegerRange>) -> String {
        match range {
            None => "unbounded".to_owned(),
            Some(range) => compound(
                "declared-range",
                [range.minimum.to_string(), range.maximum.to_string()],
            ),
        }
    }

    fn binary_operator_tag(operator: BinaryOperator) -> &'static str {
        match operator {
            BinaryOperator::Add => "add",
            BinaryOperator::And => "and",
            BinaryOperator::BitwiseAnd => "bit-and",
            BinaryOperator::BitwiseOr => "bit-or",
            BinaryOperator::BitwiseXor => "bit-xor",
            BinaryOperator::CaseMembership => "case-membership",
            BinaryOperator::Divide => "div",
            BinaryOperator::Equal => "eq",
            BinaryOperator::Greater => "gt",
            BinaryOperator::GreaterOrEqual => "ge",
            BinaryOperator::Less => "lt",
            BinaryOperator::LessOrEqual => "le",
            BinaryOperator::Modulo => "mod",
            BinaryOperator::Multiply => "mul",
            BinaryOperator::NotEqual => "ne",
            BinaryOperator::Or => "or",
            BinaryOperator::ShiftLeft => "shl",
            BinaryOperator::ShiftRight => "shr",
            BinaryOperator::Subtract => "sub",
        }
    }

    fn unary_operator_tag(operator: typed_trees::expression::UnaryOperator) -> &'static str {
        match operator {
            typed_trees::expression::UnaryOperator::BitwiseNot => "bit-not",
            typed_trees::expression::UnaryOperator::LogicalNot => "logical-not",
        }
    }

    fn symbol_identity(program: &IdentityContext<'_>, symbol: SymbolHandle) -> String {
        if !symbol.is_valid() {
            return "unresolved".to_owned();
        }
        if let Ok(identity) = program.normalized_hermetic_symbol_identity(symbol) {
            return identity;
        }
        program.symbols.display_path(symbol, "::")
    }

    fn name_path_identity(program: &IdentityContext<'_>, path: &TableNamePath) -> String {
        if path.symbol.is_valid() {
            return atom("name", symbol_identity(program, path.symbol));
        }
        let members = program.expression_table.name_path_members(path.members);
        let member_symbols =
            expression_table(program).name_path_member_symbols(path.member_symbols);
        let identity = members
            .iter()
            .enumerate()
            .map(|(index, member)| {
                member_symbols
                    .get(index)
                    .filter(|symbol| symbol.is_valid())
                    .map(|symbol| symbol_identity(program, *symbol))
                    .unwrap_or_else(|| member.as_str().to_owned())
            })
            .collect::<Vec<_>>()
            .join("::");
        atom("name-spelled", identity)
    }

    fn expression_identity(program: &IdentityContext<'_>, expression: ExpressionHandle) -> String {
        if !expression.is_valid() {
            return "expr:invalid".to_owned();
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::SavedSubject(saved) => {
                if program.saved_value_source(saved).is_none() {
                    program.unavailable.set(true);
                    return "unbound-capture".to_owned();
                }
                let Some(depth) = program
                    .captures
                    .iter()
                    .rev()
                    .position(|capture| *capture == saved.capture)
                else {
                    program.unavailable.set(true);
                    return "unbound-capture".to_owned();
                };
                compound(
                    "saved-subject",
                    [
                        atom("depth", depth.to_string()),
                        expression_identity(program, saved.pattern_head),
                    ],
                )
            }
            ExpressionNode::Match(node) => {
                let subject = expression_identity(program, node.subject);
                let mut captures = program.captures.to_vec();
                if node.capture.is_valid() {
                    if !expression_table(program).contains_value_capture(node.capture)
                        || expression_table(program)
                            .value_capture(node.capture)
                            .definition
                            != expression
                        || captures.contains(&node.capture)
                    {
                        program.unavailable.set(true);
                        return "invalid-capture".to_owned();
                    }
                    captures.push(node.capture);
                }
                let context = IdentityContext {
                    program: program.program,
                    unavailable: program.unavailable,
                    captures: &captures,
                };
                let nested = &context;
                compound("match", std::iter::once(subject).chain(program.expression_table.match_arms(node.arms).iter().map(|arm| {
                let pattern = match arm.pattern {
                    MatchPattern::Value(pattern) => expression_identity(nested, pattern),
                    MatchPattern::Wildcard => "wildcard".to_owned(),
                    MatchPattern::Destructure(pattern) => {
                        let fields = program.expression_table.match_fields(pattern.fields).iter().map(|field| {
                            let action = match &field.action {
                                typed_trees::expression::MatchFieldAction::Bind(_) => "bind".to_owned(),
                                typed_trees::expression::MatchFieldAction::Equal(value) => compound("equal", [expression_identity(nested, *value)]),
                                typed_trees::expression::MatchFieldAction::Waive => "waive".to_owned(),
                            };
                            compound("field", [symbol_identity(nested, field.field_symbol), action])
                        });
                        let selectors = program.expression_table.match_proof_selectors(pattern.proof_selectors).iter().enumerate().map(|(ordinal, selector)| {
                            compound("proof-selector", [atom("output", selector.output_field.as_str()), ordinal.to_string()])
                        });
                        compound("destructure", [
                            expression_identity(nested, pattern.head),
                            compound("fields", fields),
                            atom("rest", pattern.has_rest.to_string()),
                            if pattern.guard.is_valid() { expression_identity(nested, pattern.guard) } else { "no-guard".to_owned() },
                            compound("proof-selectors", selectors),
                        ])
                    }
                };
                compound("arm", [pattern, expression_identity(nested, arm.value)])
            })))
            }
            ExpressionNode::ArrayLiteral(elements) => compound(
                "array",
                expression_table(program)
                    .expression_handles(*elements)
                    .iter()
                    .map(|element| expression_identity(program, *element)),
            ),
            ExpressionNode::Atomic(node) => compound(
                "atomic",
                [
                    expression_identity(program, node.value),
                    expression_identity(program, node.result),
                    format!("{:?}", node.ordering),
                    format!("{:?}", node.result_custody),
                ],
            ),
            ExpressionNode::Binary(node) => compound(
                binary_operator_tag(node.operator),
                [
                    expression_identity(program, node.left),
                    expression_identity(program, node.right),
                ],
            ),
            ExpressionNode::Boolean(value) => atom("bool", value.to_string()),
            ExpressionNode::Cast(node) => compound(
                "cast",
                [
                    expression_identity(program, node.value),
                    atom!("target" => program.package_qualified_type_identity_text(node.target_type)),
                    atom("domain", node.domain.name()),
                    atom!("domain-symbol" => symbol_identity(program, node.semantic_domain_symbol)),
                    compound(
                        "domain-arguments",
                        type_reference_table(program)
                            .type_reference_handles(node.semantic_domain_arguments)
                            .iter()
                            .map(|type_reference| {
                                program
                                    .package_qualified_type_identity_text(*type_reference)
                                    .to_owned()
                            }),
                    ),
                    format!("{:?}", node.form),
                ],
            ),
            ExpressionNode::Call(node) => call_identity(program, node),
            ExpressionNode::Float(value) => atom("float", float_identity(value)),
            ExpressionNode::UndeterminedNaN(format) => atom("undetermined-nan", format.name()),
            ExpressionNode::Indexed(node) => compound(
                "index",
                [
                    expression_identity(program, node.collection),
                    expression_identity(program, node.index),
                ],
            ),
            ExpressionNode::Integer(value) => match value.value_bignum() {
                Some(value) => atom("int", value.to_string()),
                None => "int:undecoded".to_owned(),
            },
            ExpressionNode::Member(node) => compound(
                "member",
                [
                    expression_identity(program, node.receiver),
                    if node.member_symbol.is_valid() {
                        atom("member", symbol_identity(program, node.member_symbol))
                    } else {
                        atom("member-name", node.member.as_str())
                    },
                    node.case_variant
                        .as_ref()
                        .map(|variant| atom("case", variant.as_str()))
                        .unwrap_or_else(|| "case:none".to_owned()),
                ],
            ),
            ExpressionNode::Borrow(node) => compound(
                "borrow",
                [
                    expression_identity(program, node.target),
                    format!("{:?}", node.access),
                ],
            ),
            ExpressionNode::Name(path) => name_path_identity(program, path),
            ExpressionNode::Range(node) => compound(
                "range",
                [
                    expression_identity(program, node.start),
                    expression_identity(program, node.end),
                    node.end_inclusive.to_string(),
                ],
            ),
            ExpressionNode::StructLiteral(node) => compound(
                "struct",
                [
                    atom("type", symbol_identity(program, node.type_symbol)),
                    node.case_symbol
                        .map(|case| atom("case", symbol_identity(program, case)))
                        .unwrap_or_else(|| "case:none".to_owned()),
                ]
                .into_iter()
                .chain(
                    expression_table(program)
                        .struct_fields(node.fields)
                        .iter()
                        .map(|field| {
                            compound(
                                "field",
                                [
                                    if field.field_symbol.is_valid() {
                                        atom("field", symbol_identity(program, field.field_symbol))
                                    } else {
                                        atom("field-name", field.name.as_str())
                                    },
                                    expression_identity(program, field.value),
                                ],
                            )
                        }),
                ),
            ),
            ExpressionNode::String(bytes) => atom("str", hex(bytes)),
            ExpressionNode::Unary(node) => compound(
                unary_operator_tag(node.operator),
                [expression_identity(program, node.operand)],
            ),
            ExpressionNode::TypeExpression(type_reference) => atom(
                "type",
                program.package_qualified_type_identity_text(*type_reference),
            ),
            ExpressionNode::ZeroValue(type_reference) => atom(
                "zero",
                program.package_qualified_type_identity_text(*type_reference),
            ),
        }
    }

    fn call_identity(program: &IdentityContext<'_>, call: &TableCallExpression) -> String {
        let installed = call.installed_interrupt_operation.as_ref().map(|request| {
            compound(
                "installed-interrupt",
                [
                    atom(
                        "registrar-trait",
                        symbol_identity(program, request.registrar_trait),
                    ),
                    static_argument_identity(program, &request.registrar),
                    static_argument_identity(program, &request.timer_slot),
                ],
            )
        });
        compound(
        "call",
        [
            expression_identity(program, call.receiver),
            atom("target", symbol_identity(program, call.target_symbol)),
            atom!("static-parameter" => symbol_identity(program, call.static_machine_parameter)),
            requirement_dispatch_identity(program, call.static_requirement_dispatch.as_ref()),
            compound(
                "machine-arguments",
                call.machine_arguments
                    .iter()
                    .map(|argument| static_argument_identity(program, argument)),
            ),
            quotient_identity(program, call.quotient_operation.as_ref()),
            call.private_layout_operation
                .as_ref()
                .map(|request| {
                    compound(
                        "private-layout",
                        [static_argument_identity(program, &request.selected_slot)],
                    )
                })
                .unwrap_or_else(|| "private-layout:none".to_owned()),
            compound(
                "arguments",
                expression_table(program)
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| expression_identity(program, *argument)),
            ),
            compound(
                "evidence-arguments",
                call.evidence_arguments
                    .iter()
                    .map(|name| atom("evidence", name.as_str())),
            ),
            format!("{:?}", call.operational_acknowledgement),
        ]
        .into_iter()
        .chain(installed),
    )
    }

    fn requirement_dispatch_identity(
        program: &IdentityContext<'_>,
        dispatch: Option<&StaticRequirementDispatch>,
    ) -> String {
        match dispatch {
            None => "dispatch:none".to_owned(),
            Some(dispatch) => compound(
                "dispatch",
                [
                    format!(
                        "commitment:{}",
                        hex(&dispatch.application_commitment.as_bytes())
                    ),
                    format!(
                        "fingerprint:{:016x}",
                        dispatch.application_report_fingerprint
                    ),
                    atom("trait", symbol_identity(program, dispatch.declaring_trait)),
                    atom!("requirement" => symbol_identity(program, dispatch.requirement)),
                    atom!("machine" => symbol_identity(program, dispatch.realization_machine)),
                    atom!("state" => symbol_identity(program, dispatch.realization_state)),
                ],
            ),
        }
    }

    fn quotient_identity(
        program: &IdentityContext<'_>,
        request: Option<&QuotientOperationRequest>,
    ) -> String {
        match request {
            None => "quotient:none".to_owned(),
            Some(request) => compound(
                "quotient",
                [
                    format!("{:?}", request.kind),
                    static_argument_identity(program, &request.representative_operation),
                    compound(
                        "theorems",
                        request.theorem_evidence.iter().map(|theorem| {
                            compound(
                                "theorem",
                                [
                                    format!("{:?}", theorem.role),
                                    static_argument_identity(program, &theorem.application),
                                ],
                            )
                        }),
                    ),
                ],
            ),
        }
    }

    fn static_argument_identity(
        program: &IdentityContext<'_>,
        argument: &StaticMachineArgument,
    ) -> String {
        compound(
            "static-argument",
            [
                if argument.type_reference.is_valid() {
                    atom(
                        "type",
                        program.package_qualified_type_identity_text(argument.type_reference),
                    )
                } else {
                    "type:none".to_owned()
                },
                atom("path", argument.path.joined_path()),
                argument
                    .application
                    .as_deref()
                    .map(|application| static_symbol_application_identity(program, application))
                    .unwrap_or_else(|| "application:none".to_owned()),
                argument
                    .const_literal
                    .as_ref()
                    .and_then(|literal| literal.value_bignum())
                    .map(|value| atom("const", value.to_string()))
                    .unwrap_or_else(|| "const:none".to_owned()),
                argument
                    .evidence_projection
                    .as_ref()
                    .map(|projection| {
                        atom(
                            "evidence-projection",
                            format!(
                                "{}.{}",
                                projection.term.as_str(),
                                projection.member.as_str()
                            ),
                        )
                    })
                    .unwrap_or_else(|| "evidence-projection:none".to_owned()),
                atom("symbol", symbol_identity(program, argument.symbol)),
            ],
        )
    }

    fn static_symbol_application_identity(
        program: &IdentityContext<'_>,
        application: &StaticSymbolApplication,
    ) -> String {
        compound(
            "application",
            [
                compound(
                    "lifetimes",
                    application
                        .lifetime_arguments
                        .iter()
                        .map(|name| atom("lifetime", name.as_str())),
                ),
                compound(
                    "arguments",
                    application
                        .arguments
                        .iter()
                        .map(|argument| static_argument_identity(program, argument)),
                ),
            ],
        )
    }

    fn atom(tag: &str, value: impl AsRef<str>) -> String {
        format!("{tag}:{}", escape(value.as_ref()))
    }

    fn compound(tag: &str, parts: impl IntoIterator<Item = String>) -> String {
        let mut body = String::with_capacity(32);
        for part in parts {
            if !body.is_empty() {
                body.push(',');
            }
            body.push_str(&part);
        }
        format!("{tag}({body})")
    }

    fn escape(text: &str) -> String {
        let mut escaped = String::with_capacity(text.len());
        for character in text.chars() {
            match character {
                '\\' | '(' | ')' | ',' => {
                    escaped.push('\\');
                    escaped.push(character);
                }
                _ => escaped.push(character),
            }
        }
        escaped
    }

    fn hex(bytes: &[u8]) -> String {
        let mut text = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            let _ = fmt::Write::write_fmt(&mut text, format_args!("{byte:02x}"));
        }
        text
    }

    #[cfg(test)]
    mod tests {
        use super::proof_obligation_key as try_proof_obligation_key;
        use crate::obligations::ProofObligationKey;
        use crate::obligations::{
            BinaryValueOperands, BoundedAssignmentObligation, GuardedTransitionObligation,
            IntegerRange, ProofConstraint, ProofObligation, ProofPlan,
        };
        use crate::test_program::bounded_value;
        use numerics::bignum::BigInt;
        use numerics::literals::IntegerLiteral;
        use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::TypedTrees;
        use typed_trees::expression;
        use typed_trees::expression::{
            BinaryExpression, BinaryOperator, Expression, ExpressionHandle, NamePath,
        };
        use typed_trees::name::Identifier;
        use typed_trees::statement::TransitionGuardNode;
        use typed_trees::types::TypeReferenceNode;

        fn proof_obligation_key(
            plan: &ProofPlan<'_>,
            obligation: &ProofObligation,
        ) -> ProofObligationKey {
            try_proof_obligation_key(plan, obligation)
                .expect("formed obligation has a canonical key")
        }

        struct Program {
            typed_trees: TypedTrees,
            machine: SymbolHandle,
            state: SymbolHandle,
            parameter: SymbolHandle,
            data: [SymbolHandle; 2],
            int_type: SymbolHandle,
        }

        fn program() -> Program {
            let mut builder = SymbolTableBuilder::new();
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let machine = SymbolTableBuilder::child_handles(
                builder
                    .insert_children(root, [(SymbolKind::Machine, SymbolNameRef::Static("Main"))]),
            )
            .next()
            .expect("machine");
            let members = SymbolTableBuilder::child_handles(builder.insert_children(
                machine,
                [
                    (SymbolKind::State, SymbolNameRef::Static("run")),
                    (SymbolKind::Parameter, SymbolNameRef::Static("limit")),
                    (SymbolKind::Data, SymbolNameRef::Static("count")),
                    (SymbolKind::Data, SymbolNameRef::Static("total")),
                    (SymbolKind::BuiltinType, SymbolNameRef::Static("Int")),
                ],
            ))
            .collect::<Vec<_>>();
            Program {
                typed_trees: TypedTrees {
                    symbols: builder.finish(),
                    ..TypedTrees::default()
                },
                machine,
                state: members[0],
                parameter: members[1],
                data: [members[2], members[3]],
                int_type: members[4],
            }
        }

        impl Program {
            fn int_reference(&mut self) -> typed_trees::types::TypeReferenceHandle {
                self.typed_trees
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: self.int_type,
                        name: Identifier::generated("Int"),
                    })
            }

            fn name(&mut self, spelling: &str, symbol: SymbolHandle) -> ExpressionHandle {
                self.typed_trees
                    .expression_table
                    .insert_tree(&Expression::Name(NamePath::resolved(
                        vec![Identifier::generated(spelling)],
                        symbol,
                        symbol,
                    )))
            }

            fn integer(&mut self, value: i64) -> ExpressionHandle {
                self.typed_trees
                    .expression_table
                    .insert_tree(&Expression::Integer(IntegerLiteral::from_value(value)))
            }
        }

        fn bounded_assignment(
            machine: SymbolHandle,
            state: SymbolHandle,
            target: ExpressionHandle,
            value: ExpressionHandle,
            statement_index: usize,
            constraints: arena::HandleSpan<ProofConstraint>,
            base: typed_trees::types::TypeReferenceHandle,
            binary_operands: Option<BinaryValueOperands>,
        ) -> ProofObligation {
            ProofObligation::BoundedAssignment(BoundedAssignmentObligation {
                machine_symbol: machine,
                machine: Identifier::generated("Main"),
                state_symbol: state,
                state: Identifier::generated("run"),
                statement_index,
                state_guard: None,
                state_guard_source: state,
                target,
                value,
                value_constraints: arena::HandleSpan::empty(),
                base_type: base,
                constraints,
                binary_operands,
                ensures_witness_bounds: Vec::new(),
            })
        }

        #[test]
        fn statement_index_does_not_disturb_identity() {
            let mut program = program();
            let base = program.int_reference();
            let target = program.name("count", program.data[0]);
            let value = program.integer(3);
            let machine = program.machine;
            let state = program.state;
            let mut plan = ProofPlan::new(&program.typed_trees);
            let constraints = plan
                .type_constraints
                .insert_many([ProofConstraint::Named(Identifier::generated("Finite"))]);
            let first = proof_obligation_key(
                &plan,
                &bounded_assignment(machine, state, target, value, 2, constraints, base, None),
            );
            let second = proof_obligation_key(
                &plan,
                &bounded_assignment(machine, state, target, value, 9, constraints, base, None),
            );
            assert_eq!(first, second, "statement position is not semantic content");
        }

        #[test]
        fn constraint_content_and_order_enter_identity() {
            let mut program = program();
            let base = program.int_reference();
            let machine = program.machine;
            let data = program.data[0];
            let zero_seven = ProofConstraint::IntegerRange {
                minimum: BigInt::from_i64(0),
                maximum: BigInt::from_i64(7),
            };
            let zero_eight = ProofConstraint::IntegerRange {
                minimum: BigInt::from_i64(0),
                maximum: BigInt::from_i64(8),
            };
            let finite = ProofConstraint::Named(Identifier::generated("Finite"));

            let mut plan = ProofPlan::new(&program.typed_trees);
            let range = plan.type_constraints.insert_many([zero_seven.clone()]);
            let key = proof_obligation_key(
                &plan,
                &bounded_value(machine, "Main", data, "count", base, range),
            );
            let again = proof_obligation_key(
                &plan,
                &bounded_value(machine, "Main", data, "count", base, range),
            );
            assert_eq!(key, again, "keys are deterministic");

            let wider_span = plan.type_constraints.insert_many([zero_eight.clone()]);
            let wider = proof_obligation_key(
                &plan,
                &bounded_value(machine, "Main", data, "count", base, wider_span),
            );
            assert_ne!(key, wider, "the range bound is content");

            let pair = plan
                .type_constraints
                .insert_many([finite.clone(), zero_seven.clone()]);
            let reordered = plan
                .type_constraints
                .insert_many([zero_seven.clone(), finite.clone()]);
            assert_ne!(
                proof_obligation_key(
                    &plan,
                    &bounded_value(machine, "Main", data, "count", base, pair)
                ),
                proof_obligation_key(
                    &plan,
                    &bounded_value(machine, "Main", data, "count", base, reordered)
                ),
                "constraint order is retained, not normalized away"
            );
        }

        #[test]
        fn float_ranges_key_by_exact_value() {
            let mut program = program();
            let base = program.int_reference();
            let machine = program.machine;
            let data = program.data[0];
            let float_range = |minimum: f64, maximum: f64, inclusive| ProofConstraint::FloatRange {
                minimum: expression::FloatLiteral::from_f64(minimum),
                maximum: expression::FloatLiteral::from_f64(maximum),
                maximum_inclusive: inclusive,
            };
            let mut plan = ProofPlan::new(&program.typed_trees);
            let inclusive = plan
                .type_constraints
                .insert_many([float_range(0.0, 1.0, true)]);
            let key = proof_obligation_key(
                &plan,
                &bounded_value(machine, "Main", data, "count", base, inclusive),
            );
            let same = plan
                .type_constraints
                .insert_many([float_range(0.0, 1.0, true)]);
            assert_eq!(
                key,
                proof_obligation_key(
                    &plan,
                    &bounded_value(machine, "Main", data, "count", base, same)
                )
            );
            let exclusive = plan
                .type_constraints
                .insert_many([float_range(0.0, 1.0, false)]);
            assert_ne!(
                key,
                proof_obligation_key(
                    &plan,
                    &bounded_value(machine, "Main", data, "count", base, exclusive)
                ),
                "the inclusive marker is semantic"
            );
        }

        #[test]
        fn expression_payloads_distinguish_obligations() {
            let mut program = program();
            let parameter = program.parameter;
            let machine = program.machine;
            let state = program.state;
            let guard = |program: &mut TypedTrees, value: i64| {
                program
                    .expression_table
                    .insert_tree(&Expression::Binary(Box::new(BinaryExpression {
                        left: Expression::Name(NamePath::resolved(
                            vec![Identifier::generated("limit")],
                            parameter,
                            parameter,
                        )),
                        operator: BinaryOperator::LessOrEqual,
                        right: Expression::Integer(IntegerLiteral::from_value(value)),
                    })))
            };
            let first_guard = guard(&mut program.typed_trees, 8);
            let second_guard = guard(&mut program.typed_trees, 9);
            let plan = ProofPlan::new(&program.typed_trees);
            let obligation = |guard| {
                ProofObligation::GuardedTransition(GuardedTransitionObligation {
                    machine_symbol: machine,
                    machine: Identifier::generated("Main"),
                    state_symbol: state,
                    state: Identifier::generated("run"),
                    guard,
                })
            };
            assert_ne!(
                proof_obligation_key(&plan, &obligation(TransitionGuardNode::When(first_guard))),
                proof_obligation_key(&plan, &obligation(TransitionGuardNode::When(second_guard))),
                "guard expression content is semantic"
            );
            assert_ne!(
                proof_obligation_key(&plan, &obligation(TransitionGuardNode::When(first_guard))),
                proof_obligation_key(&plan, &obligation(TransitionGuardNode::Always)),
                "a missing guard is a distinct statement"
            );
        }

        #[test]
        fn binary_operand_ranges_enter_identity() {
            let mut program = program();
            let left = program.integer(1);
            let right = program.integer(2);
            let base = program.int_reference();
            let machine = program.machine;
            let state = program.state;
            let plan = ProofPlan::new(&program.typed_trees);
            let operands = |left_range: Option<IntegerRange>| {
                bounded_assignment(
                    machine,
                    state,
                    left,
                    right,
                    0,
                    arena::HandleSpan::empty(),
                    base,
                    Some(BinaryValueOperands {
                        operator: BinaryOperator::Add,
                        left,
                        left_range,
                        right,
                        right_range: None,
                    }),
                )
            };
            let declared = Some(IntegerRange {
                minimum: BigInt::from_i64(0),
                maximum: BigInt::from_i64(8),
            });
            assert_ne!(
                proof_obligation_key(&plan, &operands(None)),
                proof_obligation_key(&plan, &operands(declared)),
                "declared operand ranges are dependency context, not display"
            );
        }
    }
}
mod plan {
    use crate::obligations::collection::estimated_proof_obligation_capacity;
    use crate::typed_expressions::type_reference_table;
    use arena::{Arena, HandleSpan};
    pub(crate) use facts::IntegerRange;
    use language_semantics::value_domain::ValueDomain;
    use std::fmt;
    use symbols::SymbolHandle;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, FloatLiteral};
    use typed_trees::name::Identifier;
    use typed_trees::statement::TransitionGuardNode;
    use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProofPlan<'program> {
        pub program: &'program dyn ::validation::CheckingInput,
        pub obligations: Arena<ProofObligation>,
        pub type_constraints: Arena<ProofConstraint>,
        pub assignment_value_ranges: Arena<BoundedAssignmentObligation>,
    }

    impl<'program> ProofPlan<'program> {
        pub(crate) fn new(program: &'program dyn ::validation::CheckingInput) -> Self {
            let obligation_capacity = estimated_proof_obligation_capacity(program);
            let constraint_capacity = type_reference_table(program)
                .constraint_count()
                .saturating_add(obligation_capacity);

            Self {
                program,
                obligations: Arena::with_capacity(obligation_capacity),
                type_constraints: Arena::with_capacity(constraint_capacity),
                assignment_value_ranges: Arena::with_capacity(obligation_capacity),
            }
        }

        pub(crate) fn push_obligation(&mut self, obligation: ProofObligation) {
            self.obligations.append(obligation);
        }

        pub(crate) fn store_constraints(
            &mut self,
            constraints: ConstraintBuffer,
        ) -> HandleSpan<ProofConstraint> {
            self.type_constraints.insert_many(constraints)
        }

        pub(crate) fn store_constraint_nodes(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            base_type: TypeReferenceHandle,
            constraints: HandleSpan<TypeConstraintNode>,
        ) -> HandleSpan<ProofConstraint> {
            self.store_constraint_nodes_with(
                program,
                base_type,
                constraints,
                ProofConstraint::from_node,
            )
        }

        pub(crate) fn store_constraint_nodes_with_domain_intervals(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            base_type: TypeReferenceHandle,
            constraints: HandleSpan<TypeConstraintNode>,
        ) -> HandleSpan<ProofConstraint> {
            self.store_constraint_nodes_with(
                program,
                base_type,
                constraints,
                ProofConstraint::from_node_with_domain_intervals,
            )
        }

        fn store_constraint_nodes_with(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            base_type: TypeReferenceHandle,
            constraints: HandleSpan<TypeConstraintNode>,
            from_node: impl Fn(
                &dyn ::validation::CheckingInput,
                TypeReferenceHandle,
                &TypeConstraintNode,
            ) -> Option<ProofConstraint>,
        ) -> HandleSpan<ProofConstraint> {
            self.type_constraints.insert_many(
                type_reference_table(program)
                    .constraints(constraints)
                    .iter()
                    .filter_map(|constraint| from_node(program, base_type, constraint)),
            )
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ProofConstraint {
        Named(Identifier),
        IntegerRange {
            minimum: numerics::bignum::BigInt,
            maximum: numerics::bignum::BigInt,
        },
        FloatRange {
            minimum: FloatLiteral,
            maximum: FloatLiteral,
            maximum_inclusive: bool,
        },
        ArithmeticDomain(numerics::arithmetic::ArithmeticDomain),
    }

    impl Default for ProofConstraint {
        fn default() -> Self {
            Self::Named(Identifier::default())
        }
    }

    impl ProofConstraint {
        pub(crate) fn from_node(
            _program: &dyn ::validation::CheckingInput,
            _base_type: TypeReferenceHandle,
            constraint: &TypeConstraintNode,
        ) -> Option<Self> {
            match constraint {
                TypeConstraintNode::Named(name) => Some(Self::Named(name.clone())),
                TypeConstraintNode::Domain(domain) => ValueDomain::from_name(domain.as_str())
                    .map(|domain| Self::Named(Identifier::generated_static(domain.proof_name()))),

                TypeConstraintNode::ArithmeticDomain(domain) => {
                    Some(Self::ArithmeticDomain(*domain))
                }
            }
        }

        pub(crate) fn from_node_with_domain_intervals(
            program: &dyn ::validation::CheckingInput,
            base_type: TypeReferenceHandle,
            constraint: &TypeConstraintNode,
        ) -> Option<Self> {
            Self::from_node(program, base_type, constraint).or_else(|| match constraint {
                TypeConstraintNode::Domain(domain) => {
                    let (minimum, maximum) = validation::exact_declared_domain_carrier_interval(
                        program,
                        program.primitive_type_reference(base_type)?,
                        domain,
                    )?;
                    Some(Self::IntegerRange { minimum, maximum })
                }
                _ => None,
            })
        }
    }

    const INLINE_PROOF_CONSTRAINTS: usize = 4;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ConstraintBuffer {
        inline: [Option<ProofConstraint>; INLINE_PROOF_CONSTRAINTS],
        overflow: Vec<ProofConstraint>,
        count: usize,
    }

    impl ConstraintBuffer {
        pub(crate) fn new() -> Self {
            Self {
                inline: std::array::from_fn(|_| None),
                overflow: Vec::new(),
                count: 0,
            }
        }

        pub(crate) fn push(&mut self, constraint: ProofConstraint) {
            if self.count < INLINE_PROOF_CONSTRAINTS {
                self.inline[self.count] = Some(constraint);
            } else {
                self.overflow.push(constraint);
            }

            self.count = self
                .count
                .checked_add(1)
                .expect("proof constraint count overflow");
        }

        pub(crate) fn extend(&mut self, constraints: ConstraintBuffer) {
            self.extend_iter(constraints);
        }

        pub(crate) fn extend_iter(
            &mut self,
            constraints: impl IntoIterator<Item = ProofConstraint>,
        ) {
            for constraint in constraints {
                self.push(constraint);
            }
        }

        pub(crate) fn is_empty(&self) -> bool {
            self.count == 0
        }

        pub(crate) fn iter(&self) -> ConstraintBufferIter<'_> {
            self.inline.iter().flatten().chain(self.overflow.iter())
        }
    }

    pub(crate) type ConstraintBufferIter<'a> = std::iter::Chain<
        std::iter::Flatten<std::slice::Iter<'a, Option<ProofConstraint>>>,
        std::slice::Iter<'a, ProofConstraint>,
    >;

    impl<'a> IntoIterator for &'a ConstraintBuffer {
        type IntoIter = ConstraintBufferIter<'a>;
        type Item = &'a ProofConstraint;

        fn into_iter(self) -> Self::IntoIter {
            self.iter()
        }
    }

    impl Default for ConstraintBuffer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl IntoIterator for ConstraintBuffer {
        type IntoIter = std::iter::Chain<
            std::iter::Flatten<
                std::array::IntoIter<Option<ProofConstraint>, INLINE_PROOF_CONSTRAINTS>,
            >,
            std::vec::IntoIter<ProofConstraint>,
        >;
        type Item = ProofConstraint;

        fn into_iter(self) -> Self::IntoIter {
            self.inline.into_iter().flatten().chain(self.overflow)
        }
    }

    impl std::iter::FromIterator<ProofConstraint> for ConstraintBuffer {
        fn from_iter<T: IntoIterator<Item = ProofConstraint>>(iter: T) -> Self {
            let mut constraints = Self::new();
            constraints.extend_iter(iter);
            constraints
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ProofObligation {
        BoundedAssignment(BoundedAssignmentObligation),
        BoundedCallArgument(BoundedCallArgumentObligation),
        BoundedInitializer(BoundedInitializerObligation),
        BoundedStateReturn(BoundedStateReturnObligation),
        BoundedValue(BoundedValueObligation),
        BoundedTransitionArgument(BoundedTransitionArgumentObligation),
        GuardedTransition(GuardedTransitionObligation),
    }

    impl Default for ProofObligation {
        fn default() -> Self {
            Self::BoundedValue(BoundedValueObligation {
                owner: ProofObligationOwner::default(),
                base_type: TypeReferenceHandle::invalid(),
                constraints: HandleSpan::empty(),
            })
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub enum ProofObligationOwner {
        #[default]
        Unknown,
        MachineOwnedData {
            machine_symbol: SymbolHandle,
            machine: Identifier,
            data_symbol: SymbolHandle,
            data: Identifier,
        },
        StateParameter {
            machine_symbol: SymbolHandle,
            machine: Identifier,
            state_symbol: SymbolHandle,
            state: Identifier,
            parameter_symbol: SymbolHandle,
            parameter: Identifier,
        },
        StateReturn {
            machine_symbol: SymbolHandle,
            machine: Identifier,
            state_symbol: SymbolHandle,
            state: Identifier,
        },
        StateLocal {
            machine_symbol: SymbolHandle,
            machine: Identifier,
            state_symbol: SymbolHandle,
            state: Identifier,
            local_symbol: SymbolHandle,
            local: Identifier,
            statement_index: usize,
        },
        ConstructorField {
            machine_symbol: SymbolHandle,
            machine: Identifier,
            state_symbol: SymbolHandle,
            state: Identifier,
            field_symbol: SymbolHandle,
            field: Identifier,
            statement_index: usize,
        },
        ArrayElement {
            machine_symbol: SymbolHandle,
            machine: Identifier,
            state_symbol: SymbolHandle,
            state: Identifier,
            statement_index: usize,
            construction: ExpressionHandle,
            element_index: usize,
        },
    }

    impl ProofObligationOwner {
        pub(crate) fn state_statement(&self) -> Option<(SymbolHandle, SymbolHandle, usize)> {
            match self {
                Self::StateLocal {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    ..
                }
                | Self::ConstructorField {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    ..
                }
                | Self::ArrayElement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    ..
                } => Some((*machine_symbol, *state_symbol, *statement_index)),
                Self::Unknown
                | Self::MachineOwnedData { .. }
                | Self::StateParameter { .. }
                | Self::StateReturn { .. } => None,
            }
        }
    }

    impl fmt::Display for ProofObligationOwner {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Unknown => formatter.write_str("unknown"),
                Self::MachineOwnedData { machine, data, .. } => {
                    write!(formatter, "machine `{machine}` owned data `{data}`")
                }
                Self::StateParameter {
                    machine,
                    state,
                    parameter,
                    ..
                } => write!(
                    formatter,
                    "machine `{machine}` state `{state}` parameter `{parameter}`"
                ),
                Self::StateReturn { machine, state, .. } => {
                    write!(
                        formatter,
                        "machine `{machine}` state `{state}` return value"
                    )
                }
                Self::StateLocal {
                    machine,
                    state,
                    local,
                    ..
                } => write!(
                    formatter,
                    "machine `{machine}` state `{state}` local `{local}`"
                ),
                Self::ConstructorField {
                    machine,
                    state,
                    field,
                    ..
                } => write!(
                    formatter,
                    "machine `{machine}` state `{state}` constructor field `{field}`"
                ),
                Self::ArrayElement {
                    machine,
                    state,
                    element_index,
                    ..
                } => write!(
                    formatter,
                    "machine `{machine}` state `{state}` array element {element_index}"
                ),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BoundedValueObligation {
        pub owner: ProofObligationOwner,
        pub base_type: TypeReferenceHandle,
        pub constraints: HandleSpan<ProofConstraint>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct GuardedTransitionObligation {
        pub machine_symbol: SymbolHandle,
        pub machine: Identifier,
        pub state_symbol: SymbolHandle,
        pub state: Identifier,
        pub guard: TransitionGuardNode,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct BoundedAssignmentObligation {
        pub machine_symbol: SymbolHandle,
        pub machine: Identifier,
        pub state_symbol: SymbolHandle,
        pub state: Identifier,
        pub statement_index: usize,
        pub state_guard: Option<TransitionGuardNode>,
        pub state_guard_source: SymbolHandle,
        pub target: ExpressionHandle,
        pub value: ExpressionHandle,
        pub value_constraints: HandleSpan<ProofConstraint>,
        pub base_type: TypeReferenceHandle,
        pub constraints: HandleSpan<ProofConstraint>,
        pub binary_operands: Option<BinaryValueOperands>,
        pub ensures_witness_bounds: Vec<(String, i64)>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BoundedCallArgumentObligation {
        pub machine_symbol: SymbolHandle,
        pub machine: Identifier,
        pub state_symbol: SymbolHandle,
        pub state: Identifier,
        pub statement_index: usize,
        pub receiver: Option<Identifier>,
        pub target_symbol: SymbolHandle,
        pub target: Identifier,
        pub parameter_symbol: SymbolHandle,
        pub parameter: Identifier,
        pub argument: ExpressionHandle,
        pub argument_constraints: HandleSpan<ProofConstraint>,
        pub base_type: TypeReferenceHandle,
        pub constraints: HandleSpan<ProofConstraint>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BoundedInitializerObligation {
        pub owner: ProofObligationOwner,
        pub value: ExpressionHandle,
        pub value_constraints: HandleSpan<ProofConstraint>,
        pub base_type: TypeReferenceHandle,
        pub constraints: HandleSpan<ProofConstraint>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BoundedStateReturnObligation {
        pub machine_symbol: SymbolHandle,
        pub machine: Identifier,
        pub state_symbol: SymbolHandle,
        pub state: Identifier,
        pub statement_index: usize,
        pub value: ExpressionHandle,
        pub value_constraints: HandleSpan<ProofConstraint>,
        pub base_type: TypeReferenceHandle,
        pub constraints: HandleSpan<ProofConstraint>,
        pub binary_operands: Option<BinaryValueOperands>,
        pub guard: TransitionGuardNode,
        pub refuted_exit_guards: Vec<ExpressionHandle>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BoundedTransitionArgumentObligation {
        pub machine_symbol: SymbolHandle,
        pub machine: Identifier,
        pub state_symbol: SymbolHandle,
        pub state: Identifier,
        pub statement_index: usize,
        pub parameter_symbol: SymbolHandle,
        pub parameter: Identifier,
        pub argument: ExpressionHandle,
        pub argument_constraints: HandleSpan<ProofConstraint>,
        pub base_type: TypeReferenceHandle,
        pub constraints: HandleSpan<ProofConstraint>,
        pub guard: TransitionGuardNode,
        pub refuted_exit_guards: Vec<ExpressionHandle>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BinaryValueOperands {
        pub operator: BinaryOperator,
        pub left: ExpressionHandle,
        pub(crate) left_range: Option<IntegerRange>,
        pub right: ExpressionHandle,
        pub(crate) right_range: Option<IntegerRange>,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub(crate) struct FloatRange {
        pub(crate) minimum: f64,
        pub(crate) maximum: f64,
        pub(crate) maximum_inclusive: bool,
    }

    impl FloatRange {
        pub(crate) fn closed(value: f64) -> Self {
            Self {
                minimum: value,
                maximum: value,
                maximum_inclusive: true,
            }
        }

        pub(crate) fn intersect(self, other: Self) -> Self {
            let maximum = self.maximum.min(other.maximum);
            let maximum_inclusive = if self.maximum == other.maximum {
                self.maximum_inclusive && other.maximum_inclusive
            } else if self.maximum < other.maximum {
                self.maximum_inclusive
            } else {
                other.maximum_inclusive
            };
            Self {
                minimum: self.minimum.max(other.minimum),
                maximum,
                maximum_inclusive,
            }
        }

        pub(crate) fn contains_range(&self, value: &Self) -> bool {
            value.minimum >= self.minimum
                && (value.maximum < self.maximum
                    || (value.maximum == self.maximum
                        && (self.maximum_inclusive || !value.maximum_inclusive)))
        }

        pub(crate) fn proves_finite(&self) -> bool {
            self.minimum.is_finite() && (self.maximum.is_finite() || !self.maximum_inclusive)
        }
    }
}
mod ranges {
    use crate::obligations::plan::{ConstraintBuffer, FloatRange};
    use crate::obligations::{IntegerRange, ProofConstraint};
    use arena::HandleSpan;
    use numerics::bignum::BigInt;
    use typed_trees::expression::BinaryOperator;
    use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

    pub(crate) fn integer_constraints_are_exact(constraints: &ConstraintBuffer) -> bool {
        has_named_constraint(constraints, "exact")
            || integer_range_from_constraints(constraints).is_some()
    }

    pub(crate) fn integer_constraints_are_wrapping(constraints: &ConstraintBuffer) -> bool {
        has_named_constraint(constraints, "wrapping")
            || arithmetic_domain_from_constraints(constraints)
                == numerics::arithmetic::ArithmeticDomain::Wrapping
    }

    pub(crate) fn arithmetic_domain_from_constraints(
        constraints: &ConstraintBuffer,
    ) -> numerics::arithmetic::ArithmeticDomain {
        constraints
            .iter()
            .find_map(|constraint| match constraint {
                ProofConstraint::ArithmeticDomain(domain) => Some(*domain),
                _ => None,
            })
            .unwrap_or(numerics::arithmetic::ArithmeticDomain::Exact)
    }

    pub(crate) fn constraints_prove_finite(constraints: &ConstraintBuffer) -> bool {
        has_named_constraint(constraints, "finite")
            || float_range_from_constraints(constraints).is_some_and(|range| range.proves_finite())
    }

    pub(crate) fn has_named_constraint(constraints: &ConstraintBuffer, name: &str) -> bool {
        constraints.iter().any(|constraint| {
            matches!(
                constraint,
                ProofConstraint::Named(constraint_name) if constraint_name.as_str() == name
            )
        })
    }

    pub(crate) fn integer_range_from_constraints(
        constraints: &ConstraintBuffer,
    ) -> Option<IntegerRange> {
        integer_range_from_constraint_iter(constraints.iter())
    }

    pub(crate) fn integer_range_from_constraint_iter<'a>(
        constraints: impl Iterator<Item = &'a ProofConstraint>,
    ) -> Option<IntegerRange> {
        let constraints: Vec<&ProofConstraint> = constraints.collect();
        let mut range: Option<IntegerRange> = None;

        for constraint in constraints.iter() {
            let ProofConstraint::IntegerRange { minimum, maximum } = constraint else {
                continue;
            };

            let candidate = IntegerRange {
                minimum: minimum.clone(),
                maximum: maximum.clone(),
            };

            range = Some(match range {
                Some(existing) => IntegerRange {
                    minimum: existing.minimum.max(candidate.minimum),
                    maximum: existing.maximum.min(candidate.maximum),
                },
                None => candidate,
            });
        }

        for constraint in constraints.iter() {
            let ProofConstraint::Named(name) = constraint else {
                continue;
            };
            let floor = match name.as_str() {
                "non_negative" => BigInt::zero(),
                "positive" => BigInt::from_i64(1),
                _ => continue,
            };
            if let Some(existing) = range.as_mut()
                && existing.minimum < floor
            {
                existing.minimum = floor;
            }
        }

        range
    }

    pub(crate) fn float_range_from_constraints<'a>(
        constraints: impl IntoIterator<Item = &'a ProofConstraint>,
    ) -> Option<FloatRange> {
        let mut range: Option<FloatRange> = None;

        for constraint in constraints {
            let ProofConstraint::FloatRange {
                minimum,
                maximum,
                maximum_inclusive,
            } = constraint
            else {
                continue;
            };

            let candidate = FloatRange {
                minimum: minimum.landed_f64(),
                maximum: maximum.landed_f64(),
                maximum_inclusive: *maximum_inclusive,
            };

            range = Some(match range {
                Some(existing) => existing.intersect(candidate),
                None => candidate,
            });
        }

        range
    }

    pub(crate) use validation::integer_binary_range;

    pub(crate) fn float_binary_range(
        operator: BinaryOperator,
        left: FloatRange,
        right: FloatRange,
    ) -> Option<FloatRange> {
        if !left.proves_finite() || !right.proves_finite() {
            return None;
        }
        match operator {
            BinaryOperator::Add => Some(FloatRange {
                minimum: left.minimum + right.minimum,
                maximum: left.maximum + right.maximum,
                maximum_inclusive: true,
            }),
            BinaryOperator::Subtract => Some(FloatRange {
                minimum: left.minimum - right.maximum,
                maximum: left.maximum - right.minimum,
                maximum_inclusive: true,
            }),
            BinaryOperator::Multiply => {
                let products = [
                    left.minimum * right.minimum,
                    left.minimum * right.maximum,
                    left.maximum * right.minimum,
                    left.maximum * right.maximum,
                ];
                Some(FloatRange {
                    minimum: products.iter().copied().fold(f64::INFINITY, f64::min),
                    maximum: products.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    maximum_inclusive: true,
                })
            }
            BinaryOperator::Divide => {
                if right.minimum <= 0.0 && right.maximum >= 0.0 {
                    return None;
                }

                let quotients = [
                    left.minimum / right.minimum,
                    left.minimum / right.maximum,
                    left.maximum / right.minimum,
                    left.maximum / right.maximum,
                ];
                Some(FloatRange {
                    minimum: quotients.iter().copied().fold(f64::INFINITY, f64::min),
                    maximum: quotients.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    maximum_inclusive: true,
                })
            }
            BinaryOperator::And
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
            | BinaryOperator::Equal
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Modulo
            | BinaryOperator::NotEqual
            | BinaryOperator::Or
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::CaseMembership => None,
        }
    }

    pub(crate) fn constrained_type_reference(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, HandleSpan<TypeConstraintNode>)> {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                constrained_type_reference(program, *referee)
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => Some((*base_type, *constraints)),
            TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Named { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => None,
        }
    }
}

pub use collection::build_proof_plan;
pub(crate) use collection::declared_integer_range;
pub(crate) use constraints::expression_constraints;
pub use facts::IntegerRange;
pub use identity::{ProofObligationKey, proof_obligation_key};
pub(crate) use plan::FloatRange;
pub use plan::{
    BinaryValueOperands, BoundedAssignmentObligation, BoundedCallArgumentObligation,
    BoundedInitializerObligation, BoundedStateReturnObligation,
    BoundedTransitionArgumentObligation, BoundedValueObligation, GuardedTransitionObligation,
    ProofConstraint, ProofObligation, ProofObligationOwner, ProofPlan,
};
pub(crate) use ranges::{
    float_range_from_constraints, integer_binary_range, integer_range_from_constraint_iter,
};

#[cfg(test)]
mod range_tests {
    use super::{FloatRange, ProofConstraint};
    use crate::integer_range;
    use crate::obligations::constraints::{derived_binary_constraints, expression_constraints};
    use crate::obligations::plan::ConstraintBuffer;
    use crate::obligations::ranges::{
        constraints_prove_finite, float_binary_range, has_named_constraint, integer_binary_range,
        integer_range_from_constraints,
    };
    use numerics::bignum::BigInt;
    use symbols::{
        BuiltinTypeAtom, SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder,
        builtin_type_symbols,
    };
    use typed_trees::TypedTrees;
    use typed_trees::expression::{
        BinaryOperator, ExpressionNode, FloatLiteral, TableCastExpression,
    };
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::state::State;
    use typed_trees::types::TypeReferenceNode;

    #[test]
    fn float_ranges_intersect_and_contain_with_strict_endpoints() {
        let strict = FloatRange {
            minimum: 0.0,
            maximum: 1.5,
            maximum_inclusive: false,
        };
        let inclusive = FloatRange {
            minimum: 0.0,
            maximum: 1.5,
            maximum_inclusive: true,
        };
        let below = FloatRange::closed(1.0);

        assert!(strict.contains_range(&below));
        assert!(!strict.contains_range(&inclusive));
        assert!(strict.contains_range(&strict));
        assert!(inclusive.contains_range(&strict));
        assert_eq!(strict.intersect(inclusive), strict);
    }

    #[test]
    fn float_binary_range_refuses_operands_that_admit_infinity() {
        let admits_infinity = FloatRange {
            minimum: 0.0,
            maximum: f64::INFINITY,
            maximum_inclusive: true,
        };
        let finite = FloatRange::closed(1.0);

        for operator in [
            BinaryOperator::Add,
            BinaryOperator::Subtract,
            BinaryOperator::Multiply,
            BinaryOperator::Divide,
        ] {
            assert_eq!(float_binary_range(operator, admits_infinity, finite), None);
            assert_eq!(float_binary_range(operator, finite, admits_infinity), None);
        }
    }

    #[test]
    fn derived_float_range_stays_honest_through_magnitude_overflow() {
        let mut wide = ConstraintBuffer::new();
        wide.push(ProofConstraint::FloatRange {
            minimum: FloatLiteral::new(0.0),
            maximum: FloatLiteral::new(1e308),
            maximum_inclusive: true,
        });

        let constraints = derived_binary_constraints(BinaryOperator::Add, &wide, &wide);

        assert!(constraints.iter().any(|constraint| matches!(
            constraint,
            ProofConstraint::FloatRange { maximum, .. } if maximum.value() == f64::INFINITY
        )));
        assert!(!has_named_constraint(&constraints, "finite"));
        assert!(!constraints_prove_finite(&constraints));
    }

    #[test]
    fn derived_float_range_proves_finite_when_the_bound_is_finite() {
        let mut narrow = ConstraintBuffer::new();
        narrow.push(ProofConstraint::FloatRange {
            minimum: FloatLiteral::new(0.0),
            maximum: FloatLiteral::new(1.5),
            maximum_inclusive: true,
        });

        let constraints = derived_binary_constraints(BinaryOperator::Add, &narrow, &narrow);

        assert!(has_named_constraint(&constraints, "finite"));
        assert!(constraints_prove_finite(&constraints));
    }

    #[test]
    fn integer_binary_range_folds_or_and_xor_under_the_wider_mask() {
        for operator in [BinaryOperator::BitwiseOr, BinaryOperator::BitwiseXor] {
            assert_eq!(
                integer_binary_range(operator, integer_range(0, 1), integer_range(0, 1)),
                Some(integer_range(0, 1)),
            );
            assert_eq!(
                integer_binary_range(operator, integer_range(0, 15), integer_range(0, 1)),
                Some(integer_range(0, 15)),
            );
        }

        assert_eq!(
            integer_binary_range(
                BinaryOperator::BitwiseOr,
                integer_range(2, 3),
                integer_range(0, 1)
            ),
            Some(integer_range(2, 3)),
        );
        assert_eq!(
            integer_binary_range(
                BinaryOperator::BitwiseXor,
                integer_range(2, 3),
                integer_range(0, 1)
            ),
            Some(integer_range(0, 3)),
        );

        assert_eq!(
            integer_binary_range(
                BinaryOperator::BitwiseOr,
                integer_range(0, 0),
                integer_range(0, 0)
            ),
            Some(integer_range(0, 0)),
        );

        for operator in [BinaryOperator::BitwiseOr, BinaryOperator::BitwiseXor] {
            assert_eq!(
                integer_binary_range(operator, integer_range(-8, -1), integer_range(0, 1)),
                None,
            );
            assert_eq!(
                integer_binary_range(operator, integer_range(0, 1), integer_range(-8, 7)),
                None,
            );
        }
    }

    #[test]
    fn bool_to_integer_cast_constraints_mint_the_two_value_range() {
        let (mut program, bool_carrier, i32_carrier) = {
            let mut builder = SymbolTableBuilder::new();
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let builtins = SymbolTableBuilder::child_handles(
                builder.insert_children(root, builtin_type_symbols()),
            )
            .collect::<Vec<_>>();
            let mut program = TypedTrees {
                symbols: builder.finish(),
                ..TypedTrees::default()
            };
            let bool_carrier = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: builtins[BuiltinTypeAtom::Bool.ordinal()],
                    name: Identifier::generated("bool"),
                });
            let i32_carrier = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: builtins[BuiltinTypeAtom::I32.ordinal()],
                    name: Identifier::generated("i32"),
                });
            (program, bool_carrier, i32_carrier)
        };
        let machine = Machine::default();
        let state = State::default();
        let source = program
            .expression_table
            .insert(ExpressionNode::ZeroValue(bool_carrier));
        let cast = program
            .expression_table
            .insert(ExpressionNode::Cast(TableCastExpression {
                value: source,
                target_type: i32_carrier,
                result_type: i32_carrier,
                target_label: arena::HandleSpan::empty(),
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
                semantic_domain: arena::HandleSpan::empty(),
                semantic_domain_arguments: arena::HandleSpan::empty(),
                semantic_domain_symbol: SymbolHandle::invalid(),
                semantic_domain_id: language_semantics::SemanticDomainId::NULL,
                form: language_core::cast_form::CastForm::Value,
            }));

        let constraints = expression_constraints(&program, &machine, &state, cast);

        assert_eq!(
            integer_range_from_constraints(&constraints),
            Some(integer_range(0, 1)),
        );
        assert!(has_named_constraint(&constraints, "non_negative"));

        let mut flag = ConstraintBuffer::new();
        flag.push(ProofConstraint::IntegerRange {
            minimum: BigInt::zero(),
            maximum: BigInt::from_i64(1),
        });
        assert_eq!(
            integer_binary_range(
                BinaryOperator::BitwiseOr,
                integer_range_from_constraints(&flag).unwrap(),
                integer_range_from_constraints(&flag).unwrap()
            ),
            Some(integer_range(0, 1)),
        );
    }
}

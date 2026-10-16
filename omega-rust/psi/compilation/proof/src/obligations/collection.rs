//! Building a proof plan from a typed program: walking every bounded
//! value, initializer, assignment, call argument, state return and
//! transition argument into its obligation.
//! A selected scalar constructor field receives a first value just as a local
//! does. Its declaration supplies the required predicate, never the operand's
//! proof. Both share the occurrence-bound initializer check; later mutable
//! field writes retain the separate invariant-window rules.
//! Array literals likewise initialize each element under its receiving type.
//! Walk construction and destination together; the array's inferred result
//! type is not evidence for a stricter receiving element. Copies and reference
//! formation retain their separate custody checks, without invented stores.

use crate::obligations::constraints::{
    collect_constraints, derived_binary_constraints, expression_constraints,
};
use crate::obligations::plan::ConstraintBuffer;
use crate::obligations::ranges::{constrained_type_reference, integer_range_from_constraints};
use crate::obligations::{
    BinaryValueOperands, BoundedAssignmentObligation, BoundedCallArgumentObligation,
    BoundedInitializerObligation, BoundedStateReturnObligation,
    BoundedTransitionArgumentObligation, BoundedValueObligation, GuardedTransitionObligation,
    IntegerRange, ProofConstraint, ProofObligation, ProofObligationOwner, ProofPlan,
};
use crate::typed_expressions::{
    dehoisted_operand, expression_contains_call_node, expression_type_reference,
};
use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TableAssignment, TableLocalData, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The caller's argument for the SIBLING a sibling-length atom names --
/// `None`/invalid when the parameter's constraints carry no such atom or the
/// sibling name matches no parameter. Positional: parameters and arguments
/// pair index-for-index (self excluded on calls).
fn sibling_argument_for<'plan>(
    constraints: &[ProofConstraint],
    parameter_names: impl Iterator<Item = &'plan Identifier>,
    arguments: &[ExpressionHandle],
) -> ExpressionHandle {
    let Some(sibling) = constraints.iter().find_map(|constraint| match constraint {
        ProofConstraint::IntegerRangeSiblingLenMax { sibling, .. } => Some(sibling.clone()),
        _ => None,
    }) else {
        return ExpressionHandle::invalid();
    };
    for (index, name) in parameter_names.enumerate() {
        if name.as_str() == sibling.as_str() {
            return arguments.get(index).copied().unwrap_or_default();
        }
    }
    ExpressionHandle::invalid()
}

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
            // Every assignment in the state shares the state's incoming guard;
            // it is found on the first assignment that needs it.
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
                        let receiver = program
                            .statement_table
                            .name_path_members(table_call.receiver);
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
                                arguments: program
                                    .statement_table
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
                    StatementNode::RootBinding(_)
                    | StatementNode::AssemblyFact(_)
                    | StatementNode::Expression(_) => {}
                }
                // Calls and aggregate fields receive evaluated operands just
                // like statement calls and local initializers. One structural
                // walk locates both, retaining the selected destination and
                // statement occurrence for the ordinary bounded-value checker.
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
                            program
                                .statement_table
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

fn collect_bounded_value_obligation(
    program: &dyn ::validation::CheckingInput,
    owner: ProofObligationOwner,
    type_reference: TypeReferenceHandle,
    proof_plan: &mut ProofPlan<'_>,
) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_bounded_value_obligation(program, owner, *referee, proof_plan);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = proof_plan.store_constraint_nodes(program, *base_type, *constraints);
            proof_plan.push_obligation(ProofObligation::BoundedValue(BoundedValueObligation {
                owner,
                base_type: *base_type,
                constraints,
            }));
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_bounded_value_obligation(program, owner, *element_type, proof_plan);
        }
        TypeReferenceNode::Slice { element_type } => {
            collect_bounded_value_obligation(program, owner, *element_type, proof_plan);
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_bounded_value_obligation(program, owner.clone(), *argument, proof_plan);
            }
        }
        TypeReferenceNode::DynamicTrait { .. } => {}
        TypeReferenceNode::Named { .. } => {}
        TypeReferenceNode::ConstExpression(_) => {}
        TypeReferenceNode::Unit => {}
    }
}

fn collect_bounded_initializer_obligation(
    program: &dyn ::validation::CheckingInput,
    owner: ProofObligationOwner,
    type_reference: TypeReferenceHandle,
    value: ExpressionHandle,
    proof_plan: &mut ProofPlan<'_>,
) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_bounded_initializer_obligation(program, owner, *referee, value, proof_plan);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = proof_plan.store_constraint_nodes(program, *base_type, *constraints);
            proof_plan.push_obligation(ProofObligation::BoundedInitializer(
                BoundedInitializerObligation {
                    owner,
                    value,
                    value_constraints: HandleSpan::empty(),
                    base_type: *base_type,
                    constraints,
                },
            ));
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_bounded_initializer_obligation(
                program,
                owner,
                *element_type,
                value,
                proof_plan,
            );
        }
        TypeReferenceNode::Slice { element_type } => {
            collect_bounded_initializer_obligation(
                program,
                owner,
                *element_type,
                value,
                proof_plan,
            );
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_bounded_initializer_obligation(
                    program,
                    owner.clone(),
                    *argument,
                    value,
                    proof_plan,
                );
            }
        }
        TypeReferenceNode::DynamicTrait { .. } => {}
        TypeReferenceNode::Named { .. } => {}
        TypeReferenceNode::ConstExpression(_) => {}
        TypeReferenceNode::Unit => {}
    }
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
    // R4 containment intake: the INCLUSIVE upper bounds boundary-call
    // ensures prove for `&mut` argument places, live at THIS statement.
    // Resolved calls preserve bounds outside their shared R5 may-write frame;
    // opaque calls invalidate everything and overlapping writes drop a bound.
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
    // A window receives elements of its collection, not one scalar. Extent and
    // selector admission belong to checking; this walk owes element predicates.
    let collection_window = program
        .expression_table
        .indexed_node(target)
        .filter(|indexed| program.expression_table.range_node(indexed.index).is_some());
    let construction_type = collection_window
        .and_then(|indexed| expression_type_reference(program, machine, state, indexed.collection))
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
    // A window replacement is a sequence of element stores, not a scalar
    // assignment to the collection's element type. Its construction obligations
    // above own the bounds; treating the whole literal as one element rejects
    // lawful stores and records a false scalar range for an aggregate value.
    if collection_window.is_some() {
        return;
    }
    let bounded_target = constrained_type_reference(program, target_type);
    let (base_type, constraints) = bounded_target
        .map(|(base_type, constraints)| {
            (
                base_type,
                // The written place is the one obligation that owns an
                // exact-interval domain's interval; `check_domain_field_writes`
                // leaves those domains here.
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
    // A top-level integer BINARY value carries its operands with their DECLARED
    // ranges so the checker can refold with the (stability-gated) edge guard
    // filling in an operand the declaration leaves unbounded.
    // Each operand is DE-HOISTED first: the operand hoist rewrites a
    // runtime-indexed read into `let __hoist_N = tallies[self.k];` and the
    // value into `__hoist_N + 1`, hiding the place from both the guard match
    // and the element's declared range. A bare name whose call-free
    // initializer is a PLACE read resolves to that initializer. Sound: the
    // checker's stability gate re-collects read paths from these handles (an
    // aliasing write drops the fact), and a REASSIGNED user local keeps its
    // own name in the read paths via the obligation value, so the rebind
    // kills the fact conservatively.
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

/// The declared (constraint-derived) integer range of an arbitrary expression
/// evaluated in `state`. `binary_value_operands` only carries the top-level
/// pair; the checker's nested-operand refold asks the same question of inner
/// operands, so both read from this one provider rather than reopening
/// `expression_constraints` at the decision site.
pub(crate) fn declared_integer_range(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<IntegerRange> {
    integer_range_from_constraints(&expression_constraints(program, machine, state, expression))
}

/// Walk `statements[..upto]` maintaining the live boundary-ensures witness
/// set. Resolved calls invalidate only witnesses overlapping their shared R5
/// may-write frame; opaque calls invalidate everything. A boundary call then
/// adds its own ensures-bounded `&mut` argument places. Assignments invalidate
/// overlapping paths. (Sibling of the validation recast walk and the checker
/// ranges walk; the resolved requirement query is the shared
/// `typed_trees::boundary::called_boundary_signature` -- validation's
/// stays cache-based because it also covers `contains`-clause receivers.)
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
        // Operand evaluation precedes the store or invocation. A nested
        // mutating call can retire a witness even when the enclosing target
        // writes unrelated storage or only initializes a local.
        if !witnesses.is_empty() {
            if let Some(written) = call_frames
                .and_then(|frames| frames.statement_value_may_write_paths(machine, statement))
            {
                witnesses.retain(|(place, _)| {
                    written
                        .iter()
                        .all(|written| !validation::frame_paths_overlap(place, written))
                });
            } else {
                witnesses.clear();
            }
        }
        match statement {
            StatementNode::Call(call) => {
                if let Some(written) =
                    call_frames.and_then(|frames| frames.may_write_paths(machine, call))
                {
                    witnesses.retain(|(place, _)| {
                        written
                            .iter()
                            .all(|written| !validation::frame_paths_overlap(place, written))
                    });
                } else {
                    witnesses.clear();
                }
                let Some(signature) =
                    typed_trees::boundary::called_boundary_signature(program, call)
                else {
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
            // Guarantees describe actual backing, not the alias spelling of
            // a later store. The existing complete assignment frame owns that
            // correspondence, including recasts and stored references.
            StatementNode::Assignment(_) if !witnesses.is_empty() => {
                let written = call_frames.and_then(|frames| {
                    frames
                        .assignment_write_frame(machine, statement)
                        .into_complete_paths()
                });
                if let Some(written) = written {
                    witnesses.retain(|(place, _)| {
                        written
                            .iter()
                            .all(|written| !validation::frame_paths_overlap(place, written))
                    });
                } else {
                    witnesses.clear();
                }
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
    // Prior EXIT transitions in this state whose guards control reaching
    // this statement refutes (see the obligation field's doc).
    let statements = program.statement_table.statements(state.statement_nodes);
    let Some(statement_index) = statements.iter().position(|statement| {
        table_statement.is_some_and(|current| std::ptr::eq(statement, current))
    }) else {
        return;
    };
    let mut refuted_exit_guards = Vec::new();
    for statement in &statements[..statement_index] {
        let Some(prior) = statement.as_transition() else {
            continue;
        };
        if let TransitionGuardNode::When(guard) = prior.guard
            && prior.target.is_valid()
            && !prior.continuation.is_valid()
        {
            refuted_exit_guards.push(guard);
        }
    }
    let Some((target_state, arguments)) =
        table_transition_target_state_and_arguments(program, state, table_transition.target)
    else {
        return;
    };

    // The arm guard and its arguments evaluate at the same dispatch, so the
    // guard soundly narrows argument places -- UNLESS a sibling argument
    // contains a call, which may mutate the guarded place between the guard's
    // evaluation and a later argument's. Downgrade the guard to Always then.
    let arguments_are_call_free = arguments
        .iter()
        .all(|argument| !expression_contains_call_node(program, *argument));

    for (parameter, argument) in callable_parameters(program, target_state).zip(arguments.iter()) {
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
        // SOUND sum-payload-range narrowing under a case arm: a destructured
        // payload binding `v` (`P::One { v } -> use_v(v)` or `... use_v(v * 10)`)
        // rewrites to `self.p.v`, whose type resolution loses the payload field's
        // declared range (payload fields are `DataMember::Variant` payloads, not
        // plain `DataMember::Field`s). When the arm's CO-LOCATED guard PROVES
        // `self.p`'s case is the variant that owns `v`, the read is provably in
        // that field's declared range (construction store-enforces each variant's
        // payload range), so `guarded_argument_constraints` resolves it -- as the
        // whole argument (`use_v(v)`) or as an operand folded into the argument's
        // arithmetic (`use_v(v * 10)`). Gated on the guard proving the case
        // (direct payload access outside a case-arm has no such guard, so it stays
        // unproven -- its "case is active" obligation is undischarged) and on
        // call-free siblings (a sibling call could re-case `self.p` between the
        // dispatch guard and this argument). Deliberately NOT folded into the
        // general field resolver, which is guard-blind and also feeds direct
        // access -- that was the unsound path. See memory
        // sum-payload-range-not-propagated.
        let argument_constraint_buffer = if arguments_are_call_free {
            guarded_argument_constraints(program, machine, state, argument, &transition_guard)
        } else {
            expression_constraints(program, machine, state, argument)
        };
        let argument_constraints = proof_plan.store_constraints(argument_constraint_buffer);
        let constraints = proof_plan.store_constraint_nodes(program, base_type, constraints);
        let sibling_argument = sibling_argument_for(
            proof_plan.type_constraints.span(constraints).unwrap_or(&[]),
            callable_parameters(program, target_state).map(|parameter| &parameter.name),
            arguments,
        );

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
                sibling_argument,
            },
        ));
    }
}

/// `expression_constraints` with SOUND payload-range narrowing under a case
/// arm's `guard`: a payload-field leaf (`self.p.v`) whose owning case the guard
/// proves resolves to that field's declared constraints (via
/// `payload_field_constraints_under_case_guard`), and arithmetic over such
/// leaves (`self.p.v * 10`) folds their ranges exactly as a normal field's would
/// -- the Binary/Cast/Unary arms mirror `expression_constraints` but with
/// guard-aware operands. Every other shape falls back to the plain resolver.
/// Only reached for CALL-FREE transition arguments, where the co-located guard
/// soundly narrows the argument places.
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
            let left = guarded_argument_constraints(program, machine, state, binary.left, guard);
            let right = guarded_argument_constraints(program, machine, state, binary.right, guard);
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

/// SOUND sum-payload-range narrowing (see memory
/// sum-payload-range-not-propagated). When `argument` is a payload-field member
/// access (`self.p.v`, from a destructured `P::One { v } -> ...` binding) AND
/// the arm `guard` proves `self.p`'s case is the variant that OWNS `v`, resolve
/// that payload field's declared-range constraints. Under the proven case the
/// read is provably in range (construction store-enforces each variant's
/// payload range), so this is sound. Kept OUT of the general field resolver
/// (`data_field_in_definition`) on purpose: that path is guard-blind and also
/// feeds DIRECT payload access outside a case-arm (which carries no such guard
/// and stays correctly unproven).
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

/// Walk a guard condition (through `&&` conjunctions and `== true` wrappers) for
/// a case-membership conjunct `Equal(receiver, Type::Case)` whose left side is
/// the SAME place as `receiver`, returning the matched case's variant symbol.
/// Case membership lowers to exactly this equality shape, with the case
/// reference a `Name` whose `.symbol` is the variant symbol (see
/// `03_symbol-resolved-trees-to-typed-trees/src/domain_membership.rs`).
fn case_guard_proven_variant(
    program: &dyn ::validation::CheckingInput,
    condition: ExpressionHandle,
    receiver: ExpressionHandle,
) -> Option<SymbolHandle> {
    let Some(binary) = program.expression_table.binary_node(condition) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => case_guard_proven_variant(program, binary.left, receiver)
            .or_else(|| case_guard_proven_variant(program, binary.right, receiver)),
        BinaryOperator::Equal => {
            // `cond == true` wrapper: recurse into the non-boolean side.
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
            // Case-membership equality: `Equal(receiver, Type::Case)` (the case
            // reference is always lowered to the RIGHT of `value`).
            let case_reference = program.expression_table.name_path_node(binary.right)?;
            let variant_symbol = case_reference.symbol;
            (variant_symbol.is_valid()
                && expressions_equivalent_for_precondition(program, binary.left, receiver))
            .then_some(variant_symbol)
        }
        _ => None,
    }
}

/// The declared type of the payload field named by `member_*` in the variant
/// identified by `variant_symbol`, or None when that variant has no such
/// payload field. Searches ONLY `DataMember::Variant` payloads -- the
/// complement of `data_field_in_definition`, which searches plain fields.
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

/// One call a statement evaluates: the statement call itself, or a call
/// expression anywhere in the statement's expressions.
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
    let mut visited = Vec::new();
    for (_, root) in program.statement_table.expression_roots(statement) {
        validation::expression_traversal::visit_post_order(
            program,
            root,
            &mut visited,
            |_, node| {
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
                            literal,
                            field.field_symbol,
                        ) else {
                            continue;
                        };
                        fields.push((field, reference));
                    }
                }
            },
        );
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
        let argument_constraints =
            proof_plan.store_constraints(expression_constraints(program, machine, state, argument));
        let constraints = proof_plan.store_constraint_nodes(program, base_type, constraints);
        let sibling_argument = sibling_argument_for(
            proof_plan.type_constraints.span(constraints).unwrap_or(&[]),
            parameters.iter().map(|parameter| &parameter.name),
            call.arguments,
        );

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
                sibling_argument,
            },
        ));
    }
}

/// A local's initializer is the local's first store: its declared constraints
/// are owed exactly as an assignment's are. A reference binding stores the
/// reference, not a value of the referee, so it owes the referee nothing here.
fn collect_bounded_local_initializer_obligation(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    local: &TableLocalData,
    proof_plan: &mut ProofPlan<'_>,
) {
    if !local.initial_value.is_valid()
        || matches!(
            program
                .type_reference_table
                .type_reference(local.type_reference),
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

/// A scalar receiving declaration cannot establish its operand's facts. Field
/// and local first stores share this obligation; changing the storage home must
/// not mint finiteness or a range. The ordinary checker consumes the exact
/// owner/operand coordinate and proves the complete receiving constraint.
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
    // Aggregate and reference custody is not a scalar receiving obligation.
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

/// Each literal element is a first store, including nested arrays and arrays
/// selected by a match. One receiving walk covers locals, fields, replacements,
/// calls, returns and transitions; it never grants the destination's bounds to
/// the source expression. The ordinary bounded-initializer checker retains full
/// mathematical bounds and the element's exact evaluation occurrence.
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
        // Traverse formation, not an existing referee's contents. A named
        // backing reaches no literal and therefore creates no new stores.
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
    for (element_index, element) in program
        .expression_table
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

fn collect_bounded_state_return_obligation(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    return_type: TypeReferenceHandle,
    proof_plan: &mut ProofPlan<'_>,
) {
    let Some(typed_trees::statement::StatementNode::Expression(value)) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
    else {
        return;
    };
    let value = *value;
    let statement_index = program
        .statement_table
        .statements(state.statement_nodes)
        .len()
        - 1;
    collect_array_element_obligations(
        program,
        machine,
        state,
        statement_index,
        value,
        return_type,
        proof_plan,
    );
    let Some((base_type, constraints)) = constrained_type_reference(program, return_type) else {
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
        },
    ));
}

fn call_target_parameters(
    program: &dyn ::validation::CheckingInput,
    target_symbol: SymbolHandle,
) -> Option<&[StateParameter]> {
    // A machine target enters at its first state.
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

/// Resolve a stored state without depending on the checking transformation.
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
        for statement in program
            .statement_table
            .statements(source_state.statement_nodes)
        {
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
) -> Option<(
    &'program State,
    &'program [typed_trees::expression::ExpressionHandle],
)> {
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(target)
    else {
        return None;
    };
    let path_members = program.statement_table.name_path_members(path.members);

    state_by_symbol(program, path.symbol)
        .or_else(|| matches!(path_members, [member] if member.is_self_receiver()).then_some(state))
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
                && program
                    .expression_table
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
                && expressions_equivalent_for_precondition(program, left.receiver, right.receiver)
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && expressions_equivalent_for_precondition(program, left.left, right.left)
                && expressions_equivalent_for_precondition(program, left.right, right.right)
        }
        // LEAVES: without these, two structurally-identical guards from
        // DIFFERENT statements (distinct handles) never matched -- `(sp >= 0 &&
        // sp < 16) == true` on two funnel edges hit `(Integer(0), Integer(0))`
        // and fell through to `false`, so equivalent multi-edge guards never
        // reached the checker (the "equivalent guards on N edges don't prove"
        // keystone gap).
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

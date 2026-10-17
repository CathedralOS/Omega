//! An assignment statement: the target place, the assigned expression's
//! type and domain, and the flow-sensitive value update.

use super::{StatementOutputs, StatementScope};
use crate::value_custody::places::validate_assignment_target_handle;
use crate::{
    machine_calls::calls, proof_contracts::arithmetic_domains, proof_contracts::domain_weakening,
    value_custody::atomic_operations, value_custody::expression_types, value_custody::placed_views,
    value_custody::places, value_custody::struct_literals,
};
use typed_trees::statement::StatementNode;

pub(super) fn validate<'a, 'p>(
    scope: &StatementScope<'a>,
    outputs: &mut StatementOutputs<'_>,
    statement: &StatementNode,
    direct_written: Option<Vec<String>>,
    bound_lookup: &mut (
        &'p dyn crate::CheckingInput,
        Option<crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<'p>>,
    ),
) {
    let Some(assignment) = statement.as_assignment() else {
        unreachable!("dispatched assignment_statements")
    };
    let StatementScope {
        program,
        machine,
        state_name,
        current_state,
        writable_roots,
        ..
    } = *scope;
    let value_environment = &mut *outputs.value_environment;
    let arithmetic_decisions = &mut *outputs.arithmetic_decisions;
    let diagnostics = &mut *outputs.diagnostics;
    let state = current_state;
    // A placed atomic field is authorized by its plan's permission rows in
    // `placed_views`; a core atomic cell is authorized by the carried
    // operation's receiver polarity. Neither is an ordinary exclusive write.
    if !state.is_some_and(|state| {
        placed_views::assignment_is_placed_atomic_operation(program, machine, state, assignment)
    }) {
        validate_assignment_target_handle(
            program,
            assignment.target,
            writable_roots,
            diagnostics,
            machine,
            state,
            state_name,
            atomic_operations::assignment_receiver_polarity(program, machine, state, assignment),
        );
    }
    calls::validate_asm_value_destination(program, machine, state, assignment, diagnostics);
    let assignment_target_type =
        places::declared_place_type(program, machine, state, assignment.target)
            // An indexed target (`self.xs[i] = ..`) has no member path, so
            // `declared_place_type` returns None -- fall back to the array/slice
            // ELEMENT type so the store checks below see the real slot type.
            .or_else(|| {
                places::declared_indexed_projection_type(
                    program,
                    machine,
                    current_state,
                    assignment.target,
                )
            });
    // Weakening is about the target's STATIC qualification, so retain
    // the Constrained shell that the older representation/class checks
    // intentionally unwrap above.
    let assignment_target_type_raw =
        places::declared_place_type_raw(program, machine, state, assignment.target).or_else(|| {
            places::declared_indexed_projection_type_raw(
                program,
                machine,
                current_state,
                assignment.target,
            )
        });
    let assignment_target_primitive =
        assignment_target_type.and_then(|handle| program.primitive_type_reference(handle));
    if let Some(state) = current_state {
        struct_literals::validate_array_window_elements(
            program,
            machine,
            state,
            assignment.target,
            assignment.value,
            bound_lookup,
            diagnostics,
        );
    }
    // An array-literal RHS into a `[T; N]` target: check each element's
    // class + narrowing against T. The scalar guards below skip a non-
    // primitive (array) target, so this is the element-level complement.
    if let Some(current_state) = current_state
        && let Some(target_type) = assignment_target_type
    {
        struct_literals::validate_array_literal_elements(
            program,
            machine,
            current_state,
            assignment.value,
            target_type,
            diagnostics,
        );
    }
    let owner = format!("machine `{}` state `{state_name}` assignment", machine.name);
    // The sealed atomic event consumes its authored inputs at the cell's value
    // type. Its generated resident-update model is neither a source value nor
    // evidence that each input fits. Ordinary assignment has one delivered RHS;
    // store, swap, fetch and compare-exchange use the same delivery checks below.
    let atomic = atomic_operations::atomic_assignment_carrier(program, assignment);
    let ordinary_value = [assignment.value];
    let delivered_values = atomic.as_ref().map_or(ordinary_value.as_slice(), |atomic| {
        atomic.operands.as_slice()
    });
    for &value in delivered_values {
        // Cross-class scalar guard: `self.i32 = true` (a bool into a numeric
        // field) is a soundness hole -- the backend silently stores `1` --
        // whether the bool arrives as a literal or through a `self.bool_field`
        // place. Reject the unambiguous cross-class cases before value-range
        // analysis (which assumes a class-compatible RHS).
        if let Some(target_primitive) = assignment_target_primitive {
            expression_types::report_cross_class_store(
                program,
                Some(machine),
                current_state,
                value,
                target_primitive,
                &owner,
                "place",
                diagnostics,
            );
        }
        // Nominal guard: `self.foo = self.bar` (a `Bar` value into a `Foo` place)
        // is silently accepted -- the wrong-data-type complement of the scalar
        // guard above.
        if let Some(target_type) = assignment_target_type {
            expression_types::report_data_type_conflict(
                program,
                machine,
                current_state,
                value,
                target_type,
                &owner,
                "place",
                diagnostics,
            );
            expression_types::report_delivered_type_mismatch(
                program,
                machine.symbol,
                value,
                target_type,
                &owner,
                diagnostics,
            );
            // Shape guard: `self.scalar = self.xs` (array into a scalar place) --
            // caught by the backend today (a crude `NeedsMachineOwnedWrite`); this
            // gives a clear frontend message and covers the mirror.
            expression_types::report_array_scalar_shape_mismatch(
                program,
                machine,
                current_state,
                value,
                target_type,
                &owner,
                "place",
                diagnostics,
            );
            // Scalar-vs-data shape guard: `self.struct_field = 5` / `self.scalar
            // = self.struct` (a scalar into a struct slot or the mirror), between
            // the scalar-class and nominal gates.
            expression_types::report_scalar_data_shape_mismatch(
                program,
                machine,
                current_state,
                value,
                target_type,
                &owner,
                "place",
                diagnostics,
            );
        }
        if let Some(target_type) = assignment_target_type_raw {
            domain_weakening::validate_implicit_domain_weakening(
                program,
                machine,
                current_state,
                value,
                target_type,
                &owner,
                diagnostics,
            );
        }
        if atomic.is_some()
            && let Some(target_primitive) = assignment_target_primitive
        {
            arithmetic_domains::check_value_narrowing(
                program,
                machine,
                current_state,
                value,
                target_primitive,
                assignment_target_type_raw.map_or(
                    numerics::arithmetic::ArithmeticDomain::Exact,
                    |reference| {
                        program.arithmetic_domain_for_type_reference(places::assignment_value_type(
                            program, reference,
                        ))
                    },
                ),
                value_environment,
                &owner,
                diagnostics,
            );
        }
    }
    // Assignment operands share the executable producer's ordered evaluation.
    // Source validation owns types, access, effects and restoration debt, not
    // a second signature/shape whitelist predicting which calls can lower.
    // Missing operand plans still fail product construction before execution.
    // A target's index operands are evaluated like any read: `items[top] = v`
    // bounds `top` in the environment the store runs in, exactly as the read
    // `items[top]` does. The value walk below sees only the right-hand side,
    // so without this the range checker has no interval for a written index.
    if let Some((projections, _)) =
        crate::expression_traversal::projection_climb(program, assignment.target, usize::MAX)
    {
        for projection in projections {
            if let typed_trees::expression::ExpressionNode::Indexed(indexed) =
                program.expression_table.expression(projection)
            {
                arithmetic_domains::validate_value_range_retaining_decisions(
                    program,
                    machine,
                    current_state,
                    indexed.index,
                    value_environment,
                    None,
                    numerics::arithmetic::ArithmeticDomain::Exact,
                    &owner,
                    diagnostics,
                    arithmetic_decisions,
                );
            }
        }
    }
    let before = diagnostics.len();
    let first_checked_interval = arithmetic_decisions.expression_values.len();
    let assignment_target_domain = assignment_target_type
        .map(|handle| program.arithmetic_domain_for_type_reference(handle))
        .unwrap_or(numerics::arithmetic::ArithmeticDomain::Exact);
    let (interval, source_primitive, source_domain) = assignment_target_type_raw
        .and_then(|destination| {
            arithmetic_domains::validate_anonymous_integer_range(
                program,
                places::assignment_value_type(program, destination),
                assignment.value,
                &owner,
                diagnostics,
            )
        })
        .unwrap_or_else(|| {
            arithmetic_domains::validate_value_range_retaining_decisions(
                program,
                machine,
                current_state,
                assignment.value,
                value_environment,
                assignment_target_primitive,
                assignment_target_domain,
                &owner,
                diagnostics,
                arithmetic_decisions,
            )
        });
    // The declared policy is read through the RAW destination (its `in
    // Wrapping` qualification lives on the Constrained shell that
    // `assignment_target_type` unwraps), matching the store's actual slot.
    let declared_target_domain = assignment_target_type_raw
        .map(|handle| {
            program.arithmetic_domain_for_type_reference(places::assignment_value_type(
                program, handle,
            ))
        })
        .unwrap_or(numerics::arithmetic::ArithmeticDomain::Exact);
    // Carrier compatibility is independent of arithmetic definedness: an
    // overflowing expression does not authorize an implicit conversion.
    let (source_primitive, source_domain) = arithmetic_domains::stored_value_carrier(
        program,
        machine,
        current_state,
        assignment.value,
        source_primitive,
        source_domain,
    );
    arithmetic_domains::check_narrowing_assignment(
        assignment_target_primitive,
        declared_target_domain,
        interval,
        source_primitive,
        source_domain,
        &owner,
        diagnostics,
    );
    // Only a clean value supplies a successful storage-fit judgment.
    if diagnostics.len() == before {
        let writes = arithmetic_domains::validate_stored_integer_write(
            program,
            machine,
            current_state,
            assignment.target,
            assignment.value,
            scope.local_write_origins,
            interval,
            source_primitive,
            value_environment,
            &owner,
            diagnostics,
        );
        outputs.stored_integer_writes.extend(writes);
        // Containment for non-literal stores into ranged places (see
        // the local arm's twin note; literal stores refuse through
        // the proof plan).
        if let Some(handle) = assignment_target_type_raw {
            arithmetic_domains::enforce_symbolic_range(
                program,
                machine,
                current_state,
                handle,
                assignment.value,
                value_environment,
                &owner,
                diagnostics,
            );
        }
        if let Some(handle) = assignment_target_type {
            arithmetic_domains::check_range_containment(
                program,
                handle,
                interval,
                &owner,
                diagnostics,
            );
        }
        if let Some(stated) = current_state.and_then(|state| {
            crate::proof_contracts::default_domains::stored_field_where_interval(
                program,
                machine,
                state,
                assignment.target,
            )
        }) {
            let contained = arithmetic_domains::check_where_interval_containment(
                stated,
                interval,
                &owner,
                diagnostics,
            );
            if contained
                && diagnostics.len() == before
                && let (Some(state), Some(target_type)) =
                    (current_state, assignment_target_type_raw)
                && let Some(decision) = checked_integer_assignment(
                    program,
                    machine,
                    state,
                    scope.statement_index,
                    assignment,
                    target_type,
                    stated,
                )
            {
                arithmetic_decisions.integer_assignments.push(decision);
            }
        }
    }
    let copied_value = arithmetic_domains::CopiedIntegerValue::capture(
        program,
        assignment.value,
        value_environment,
        &arithmetic_decisions.expression_values[first_checked_interval..],
    );
    // Analyze the RHS against pre-store facts, then invalidate every
    // spelling of the storage before recording the new target value.
    let copied_float = arithmetic_domains::capture_float_assignment(
        program,
        machine,
        current_state,
        value_environment,
        assignment_target_primitive,
        assignment.value,
    );
    if let Some(written) = direct_written {
        value_environment.invalidate_assignment_paths(
            program,
            machine,
            current_state,
            statement,
            scope.call_frames,
            &written,
        );
    } else {
        value_environment.invalidate_unknown_writes(program, current_state);
    }
    let assignment_path = arithmetic_domains::place_path(program, assignment.target);
    arithmetic_domains::record_assignment(
        value_environment,
        assignment_path.clone(),
        interval,
        places::declared_place_type_raw(program, machine, current_state, assignment.target)
            .and_then(|handle| arithmetic_domains::enforced_declared_range(program, handle)),
    );
    if let Some(path) = assignment_path.as_deref() {
        arithmetic_domains::publish_slice_length(
            program,
            machine,
            current_state,
            value_environment,
            path,
            assignment_root_symbol(program, assignment.target),
            assignment.value,
        );
    }
    if diagnostics.len() == before {
        copied_value.record(
            value_environment,
            arithmetic_domains::place_path(program, assignment.target)
                .or_else(|| arithmetic_domains::element_place_path(program, assignment.target)),
            places::canonical_place_from_expression(program, assignment.target),
        );
        arithmetic_domains::record_boolean_assignment(
            program,
            machine,
            current_state,
            value_environment,
            assignment.target,
            assignment.value,
        );
        arithmetic_domains::record_unsigned_literal_assignment(
            program,
            value_environment,
            arithmetic_domains::place_path(program, assignment.target),
            assignment_target_primitive,
            assignment.value,
        );
        arithmetic_domains::record_float_assignment(
            value_environment,
            arithmetic_domains::place_path(program, assignment.target),
            copied_float,
        );
    }
}

/// Retain the complete receiving interval only after this statement's value
/// passed containment. The carrier supplies unstated ends, and declared type
/// bounds intersect the data's bounds just as field retention does. This is
/// not the state walk's establishment flag: a later repairing store cannot
/// authorize this occurrence's earlier value.
fn assignment_root_symbol(
    program: &dyn crate::CheckingInput,
    target: typed_trees::expression::ExpressionHandle,
) -> symbols::SymbolHandle {
    let Some(place) = places::canonical_place_from_expression(program, target) else {
        return symbols::SymbolHandle::default();
    };
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return symbols::SymbolHandle::default();
    };
    if !place.segments.is_empty() {
        return symbols::SymbolHandle::default();
    }
    symbol
}

fn checked_integer_assignment(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    assignment: &typed_trees::statement::TableAssignment,
    target_type: typed_trees::types::TypeReferenceHandle,
    stated: arithmetic_domains::Interval,
) -> Option<checked_trees::CheckedIntegerAssignmentDecision> {
    let primitive = program.primitive_type_reference(target_type)?;
    let (mut minimum, mut maximum) =
        crate::proof_contracts::proof_embeddings::primitive_range(primitive)?;
    for interval in std::iter::once(stated).chain(arithmetic_domains::enforced_declared_range(
        program,
        target_type,
    )) {
        if let Some(lower) = interval.low() {
            minimum = minimum.max(numerics::bignum::BigInt::from_i64(lower));
        }
        if let Some(upper) = interval.high() {
            maximum = maximum.min(numerics::bignum::BigInt::from_i64(upper));
        }
    }
    (minimum <= maximum).then_some(checked_trees::CheckedIntegerAssignmentDecision {
        machine: machine.symbol,
        state: state.symbol,
        statement_index,
        target: assignment.target,
        value: assignment.value,
        target_type,
        minimum,
        maximum,
    })
}

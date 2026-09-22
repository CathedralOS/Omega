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

pub(super) fn validate(
    scope: &StatementScope<'_>,
    outputs: &mut StatementOutputs<'_>,
    statement: &StatementNode,
    direct_written: Option<Vec<String>>,
) {
    let StatementNode::Assignment(assignment) = statement else {
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
    let exact_integer_casts = &mut *outputs.exact_integer_casts;
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
            assignment.value,
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
            assignment.value,
            target_type,
            &owner,
            "place",
            diagnostics,
        );
        // Shape guard: `self.scalar = self.xs` (array into a scalar place) --
        // caught by the backend today (a crude `NeedsMachineOwnedWrite`); this
        // gives a clear frontend message and covers the mirror.
        expression_types::report_array_scalar_shape_mismatch(
            program,
            machine,
            current_state,
            assignment.value,
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
            assignment.value,
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
            assignment.value,
            target_type,
            &owner,
            diagnostics,
        );
    }
    calls::report_nested_call_in_local_assignment(
        program,
        machine,
        current_state,
        state_name,
        assignment,
        diagnostics,
    );
    let before = diagnostics.len();
    let assignment_target_domain = assignment_target_type
        .map(|handle| program.arithmetic_domain_for_type_reference(handle))
        .unwrap_or(numerics::arithmetic::ArithmeticDomain::Exact);
    let (interval, source_primitive) = assignment_target_type_raw
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
            arithmetic_domains::validate_value_range(
                program,
                machine,
                current_state,
                assignment.value,
                value_environment,
                assignment_target_primitive,
                assignment_target_domain,
                &owner,
                diagnostics,
            )
        });
    // Only a CLEANLY-analyzed RHS reaches the narrowing check -- an RHS that
    // already erred (its own overflow, a type error) is not re-flagged.
    if diagnostics.len() == before {
        arithmetic_domains::check_narrowing_assignment(
            assignment_target_primitive,
            interval,
            source_primitive,
            &owner,
            diagnostics,
        );
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
    }
    // Analyze the RHS against pre-store facts, then invalidate every
    // spelling of the storage before recording the new target value.
    arithmetic_domains::collect_exact_integer_cast_facts(
        program,
        machine,
        current_state,
        assignment.value,
        value_environment,
        exact_integer_casts,
    );
    if let Some(written) = direct_written {
        value_environment.invalidate_assignment_paths(
            program,
            machine,
            current_state,
            assignment.target,
            &written,
        );
    } else {
        value_environment.clear();
    }
    arithmetic_domains::record_assignment(
        value_environment,
        arithmetic_domains::place_path(program, assignment.target),
        interval,
        places::declared_place_type_raw(program, machine, current_state, assignment.target)
            .and_then(|handle| arithmetic_domains::enforced_declared_range(program, handle)),
    );
    if diagnostics.len() == before {
        arithmetic_domains::record_unsigned_literal_assignment(
            program,
            value_environment,
            arithmetic_domains::place_path(program, assignment.target),
            assignment_target_primitive,
            assignment.value,
        );
        arithmetic_domains::record_float_literal_assignment(
            program,
            value_environment,
            arithmetic_domains::place_path(program, assignment.target),
            assignment_target_primitive,
            assignment.value,
        );
    }
}

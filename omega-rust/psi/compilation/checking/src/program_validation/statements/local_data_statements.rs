//! A local data statement: the declared type, its initializer and the
//! value environment it introduces.

use super::{StatementOutputs, StatementScope};
use crate::value_custody::type_references::TypeReferenceOwner;
use crate::value_custody::type_references::validate_type_reference_handle;
use crate::{
    machine_calls::calls, proof_contracts::arithmetic_domains, proof_contracts::domain_weakening,
    value_custody::expression_types, value_custody::struct_literals,
};
use diagnostics::Diagnostic;
use typed_trees::statement::StatementNode;

pub(super) fn validate(
    scope: &StatementScope<'_>,
    outputs: &mut StatementOutputs<'_>,
    statement: &StatementNode,
) {
    let Some(local_data) = statement.as_local_data() else {
        unreachable!("dispatched local_data_statements")
    };
    let StatementScope {
        program,
        machine,
        state_name,
        current_state,
        statement_index,
        symbols,
        ..
    } = *scope;
    let value_environment = &mut *outputs.value_environment;
    let arithmetic_decisions = &mut *outputs.arithmetic_decisions;
    let diagnostics = &mut *outputs.diagnostics;
    let (type_parameters, lifetime_parameters) = program
        .machine_type_reference_scope(machine, |existing, parameter| {
            existing.symbol == parameter.symbol
        });
    validate_type_reference_handle(
        program,
        local_data.type_reference,
        symbols,
        diagnostics,
        TypeReferenceOwner::StateLocalData {
            machine,
            state: state_name,
            local: local_data.name.as_str(),
            generic_depth: 0,
        },
        crate::value_custody::type_references::TypeParameterScope {
            operand_lifetime_owner: symbols::SymbolHandle::invalid(),
            lexical_owner: machine.symbol,
            type_parameters: &type_parameters,
            lifetime_parameters: &lifetime_parameters,
        },
    );
    let local_target_primitive = program.primitive_type_reference(local_data.type_reference);
    // An array-literal initializer (`let a: [T; N] = [300, ..]`) is checked
    // element-wise against T.
    if let Some(current_state) = current_state {
        struct_literals::validate_array_literal_elements(
            program,
            machine,
            current_state,
            local_data.initial_value,
            local_data.type_reference,
            diagnostics,
        );
    }
    let owner = format!(
        "machine `{}` state `{state_name}` local `{}`",
        machine.name,
        local_data.name.as_str()
    );
    // Unit is not storage for an initializer's value. Generated
    // bindings can retain an inference sentinel, but an authored
    // annotation must never silently discard a produced value.
    if local_data.initial_value.is_valid()
        && local_data.type_reference.is_valid()
        && !local_data.type_is_inferred
        && matches!(
            program
                .type_reference_table
                .type_reference(local_data.type_reference),
            typed_trees::types::TypeReferenceNode::Unit
        )
    {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} has an explicit Unit type and cannot bind an initializer value; use the value's type or explicitly discard the result"
        )));
    }
    // Cross-class guard: `let x: i32 = true` stores a bool into a numeric
    // local -- a silent miscompile, same as the assignment / arg / field
    // positions. Only an INITIALIZED `let` has a value to class-check (a
    // bare `let x: bool;` filled later by an `&mut` out-param has an invalid
    // initializer). (Narrowing on the initializer is checked below.)
    if local_data.initial_value.is_valid()
        && let Some(target_primitive) = local_target_primitive
    {
        expression_types::report_cross_class_store(
            program,
            Some(machine),
            current_state,
            local_data.initial_value,
            target_primitive,
            &owner,
            "local",
            diagnostics,
        );
    }
    // Nominal guard: `let f: Foo = self.bar` binds a `Bar` value to a `Foo`
    // local -- wrong data type. Only an initialized `let` has a value.
    if local_data.initial_value.is_valid() {
        let prior_statements = current_state.map_or(&[][..], |state| {
            &program.statement_table.statements(state.statement_nodes)[..statement_index]
        });
        let access_refused =
            crate::value_custody::reference_access::report_reference_access_conflict(
                program,
                machine,
                current_state,
                prior_statements,
                local_data.initial_value,
                local_data.type_reference,
                &owner,
                diagnostics,
            );
        // One refused store reports one reason: an access refusal already
        // explains a delivered reference of the wrong access.
        if !access_refused {
            expression_types::report_delivered_type_mismatch(
                program,
                machine.symbol,
                local_data.initial_value,
                local_data.type_reference,
                &owner,
                diagnostics,
            );
        }
        expression_types::report_data_type_conflict(
            program,
            machine,
            current_state,
            local_data.initial_value,
            local_data.type_reference,
            &owner,
            "local",
            diagnostics,
        );
        // Shape guard: `let y: i32 = self.xs` (array -> scalar) or
        // `let xs: [i32; 3] = 5` (scalar -> array) otherwise bind a
        // wrong-shaped value silently (the array case read a ZII 0).
        expression_types::report_array_scalar_shape_mismatch(
            program,
            machine,
            current_state,
            local_data.initial_value,
            local_data.type_reference,
            &owner,
            "local",
            diagnostics,
        );
        expression_types::report_scalar_data_shape_mismatch(
            program,
            machine,
            current_state,
            local_data.initial_value,
            local_data.type_reference,
            &owner,
            "local",
            diagnostics,
        );
        domain_weakening::validate_implicit_domain_weakening(
            program,
            machine,
            current_state,
            local_data.initial_value,
            local_data.type_reference,
            &owner,
            diagnostics,
        );
    }
    calls::report_nested_call_in_local_initializer(
        program,
        machine,
        state_name,
        local_data.initial_value,
        diagnostics,
    );
    let before = diagnostics.len();
    let first_checked_interval = arithmetic_decisions.expression_values.len();
    let local_target_domain =
        program.arithmetic_domain_for_type_reference(local_data.type_reference);
    let (interval, source_primitive, source_domain) =
        arithmetic_domains::validate_anonymous_integer_range(
            program,
            local_data.type_reference,
            local_data.initial_value,
            &owner,
            diagnostics,
        )
        .unwrap_or_else(|| {
            arithmetic_domains::validate_value_range_retaining_decisions(
                program,
                machine,
                current_state,
                local_data.initial_value,
                value_environment,
                local_target_primitive,
                local_target_domain,
                &owner,
                diagnostics,
                arithmetic_decisions,
            )
        });
    // Keep the declared carrier check independent of arithmetic failures.
    if local_data.initial_value.is_valid() {
        let (source_primitive, source_domain) = arithmetic_domains::stored_value_carrier(
            program,
            machine,
            current_state,
            local_data.initial_value,
            source_primitive,
            source_domain,
        );
        arithmetic_domains::check_narrowing_assignment(
            local_target_primitive,
            local_target_domain,
            interval,
            source_primitive,
            source_domain,
            &owner,
            diagnostics,
        );
    }
    if local_data.initial_value.is_valid() && diagnostics.len() == before {
        // Containment against a declared Exact `[a..=b]`: literal
        // initializers refuse through the proof plan, but a
        // NON-LITERAL initializer's interval was never checked --
        // `let idx: u32 [0..=11] = <expr provably up to 12>` stored
        // unproven and the index prover then TRUSTED the range (a
        // confirmed native OOB read, found landing the R3 product
        // rule). The enforced range only exists under Exact shells,
        // so non-Exact declarations are untouched (the
        // `range-constraints-require-exact-domain` gate
        // already rejects range+domain combinations).
        // A reference binding stores the reference, not a fresh value
        // into the referee. Its referee facts are checked by ordinary
        // borrow compatibility and, for a stated recast, by the
        // bidirectional representation judgment. Treating the borrow
        // expression as a numeric store into the referee incorrectly
        // rejects every range-refined reference initializer.
        if !matches!(
            program
                .type_reference_table
                .type_reference(local_data.type_reference),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        ) {
            arithmetic_domains::enforce_symbolic_range(
                program,
                machine,
                current_state,
                local_data.type_reference,
                local_data.initial_value,
                value_environment,
                &owner,
                diagnostics,
            );
            arithmetic_domains::check_range_containment(
                program,
                local_data.type_reference,
                interval,
                &owner,
                diagnostics,
            );
        }
    }
    if local_data.initial_value.is_valid() {
        let mut copied_value = arithmetic_domains::CopiedIntegerValue::capture(
            program,
            local_data.initial_value,
            value_environment,
            &arithmetic_decisions.expression_values[first_checked_interval..],
        );
        copied_value.capture_local_relations(
            program,
            machine,
            current_state,
            local_data,
            value_environment,
        );
        let copied_float = arithmetic_domains::capture_float_assignment(
            program,
            machine,
            current_state,
            value_environment,
            program.primitive_type_reference(local_data.type_reference),
            local_data.initial_value,
        );
        arithmetic_domains::record_assignment(
            value_environment,
            Some(local_data.name.as_str().to_owned()),
            interval,
            local_data
                .type_reference
                .is_valid()
                .then(|| {
                    arithmetic_domains::enforced_declared_range(program, local_data.type_reference)
                })
                .flatten(),
        );
        arithmetic_domains::publish_slice_length(
            program,
            machine,
            current_state,
            value_environment,
            local_data.name.as_str(),
            local_data.symbol,
            local_data.initial_value,
        );
        if diagnostics.len() == before {
            copied_value.record(
                value_environment,
                Some(local_data.name.as_str().to_owned()),
                Some(crate::value_custody::places::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(local_data.symbol),
                    segments: Vec::new(),
                }),
            );
            arithmetic_domains::record_boolean_local(
                program,
                machine,
                current_state,
                value_environment,
                local_data,
            );
            arithmetic_domains::record_unsigned_literal_assignment(
                program,
                value_environment,
                Some(local_data.name.as_str().to_owned()),
                program.primitive_type_reference(local_data.type_reference),
                local_data.initial_value,
            );
            arithmetic_domains::record_float_assignment(
                value_environment,
                Some(local_data.name.as_str().to_owned()),
                copied_float,
            );
        }
    }
}

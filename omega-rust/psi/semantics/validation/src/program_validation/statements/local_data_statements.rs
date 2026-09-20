//! A local data statement: the declared type, its initializer and the
//! value environment it introduces.

use super::{StatementOutputs, StatementScope};
use crate::value_custody::type_references::TypeReferenceOwner;
use crate::value_custody::type_references::validate_type_reference_handle_with_type_parameters;
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
    let StatementNode::LocalData(local_data) = statement else {
        unreachable!("dispatched local_data_statements")
    };
    let StatementScope {
        program,
        machine,
        state_name,
        current_state,
        symbols,
        ..
    } = *scope;
    let value_env = &mut *outputs.value_env;
    let exact_integer_casts = &mut *outputs.exact_integer_casts;
    let diagnostics = &mut *outputs.diagnostics;
    let mut type_parameters = program.machine_type_parameters(machine).to_vec();
    let mut lifetime_parameters = machine.lifetime_parameters.clone();
    if let Some(attached_data) = &machine.attached_data
        && let Some(definition) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name == *attached_data)
    {
        for parameter in program.data_type_parameters(definition) {
            if !type_parameters
                .iter()
                .any(|existing| existing.symbol == parameter.symbol)
            {
                type_parameters.push(parameter.clone());
            }
        }
        for parameter in &definition.lifetime_parameters {
            if !lifetime_parameters
                .iter()
                .any(|existing| existing == parameter)
            {
                lifetime_parameters.push(parameter.clone());
            }
        }
    }
    validate_type_reference_handle_with_type_parameters(
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
        &type_parameters,
        &lifetime_parameters,
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
    let (interval, source_primitive) = arithmetic_domains::validate_anonymous_integer_range(
        program,
        local_data.type_reference,
        local_data.initial_value,
        &owner,
        diagnostics,
    )
    .unwrap_or_else(|| {
        arithmetic_domains::validate_value_range(
            program,
            machine,
            current_state,
            local_data.initial_value,
            value_env,
            local_target_primitive,
            program.arithmetic_domain_for_type_reference(local_data.type_reference),
            &owner,
            diagnostics,
        )
    });
    // A cleanly-analyzed initializer whose value cannot fit the declared
    // local type is a silent narrowing (`let x: i8 = 300`).
    if local_data.initial_value.is_valid() && diagnostics.len() == before {
        arithmetic_domains::check_narrowing_assignment(
            local_target_primitive,
            interval,
            source_primitive,
            &owner,
            diagnostics,
        );
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
        arithmetic_domains::collect_exact_integer_cast_facts(
            program,
            machine,
            current_state,
            local_data.initial_value,
            value_env,
            exact_integer_casts,
        );
        arithmetic_domains::record_assignment(
            value_env,
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
        if diagnostics.len() == before {
            arithmetic_domains::record_unsigned_literal_assignment(
                program,
                value_env,
                Some(local_data.name.as_str().to_owned()),
                program.primitive_type_reference(local_data.type_reference),
                local_data.initial_value,
            );
            arithmetic_domains::record_float_literal_assignment(
                program,
                value_env,
                Some(local_data.name.as_str().to_owned()),
                program.primitive_type_reference(local_data.type_reference),
                local_data.initial_value,
            );
        }
    }
}

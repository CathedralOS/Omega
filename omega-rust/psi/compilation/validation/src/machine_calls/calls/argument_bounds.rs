//! Numeric parameter delivery consumes the caller's evaluated argument bounds.

use crate::frozen_program::expression_table;
use crate::proof_contracts::arithmetic_domains::{self, ValueEnvironment};
use arithmetic_domains::Interval;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

/// Check both carrier width and the declared range at the resolved parameter.
/// A wholly anonymous argument first lands here; its integrality diagnostics
/// cannot be discarded as if an earlier typed operation had already checked it.
pub(super) fn report_argument_bounds(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_environment: &ValueEnvironment,
    argument: ExpressionHandle,
    parameter: &StateParameter,
    target_name: &str,
    retains_float_result_policy: bool,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    report_accessor_scalar_write_predicate(
        program,
        current_machine,
        current_state,
        value_environment,
        argument,
        parameter,
        diagnostics,
    );
    let Some(parameter_primitive) = program.primitive_type_reference(parameter.type_reference)
    else {
        return;
    };
    let owner = format!(
        "machine `{}` state `{target_name}` argument `{}`",
        current_machine.name, parameter.name,
    );
    arithmetic_domains::validate_anonymous_address_landing(
        program,
        current_machine,
        current_state,
        argument,
        Some(parameter_primitive),
        &owner,
        diagnostics,
        formation_obligations,
    );
    let before = diagnostics.len();
    if let Some((interval, _, _)) = arithmetic_domains::validate_anonymous_integer_range(
        program,
        parameter.type_reference,
        argument,
        &owner,
        diagnostics,
    ) {
        if diagnostics.len() == before {}
        return;
    }
    let parameter_domain = program.arithmetic_domain_for_type_reference(parameter.type_reference);
    // Primitive Float adapters consume the evaluated operand policy, including
    // computed operands. Ordinary parameter delivery uses its declared policy.
    let target_domain =
        if retains_float_result_policy && parameter_domain == ArithmeticDomain::Exact {
            let mut arithmetic_diagnostics = Vec::new();
            let (_, _, operand_domain) = arithmetic_domains::validate_value_range(
                program,
                current_machine,
                current_state,
                argument,
                value_environment,
                Some(parameter_primitive),
                ArithmeticDomain::Exact,
                &owner,
                &mut arithmetic_diagnostics,
            );
            // The expression validator owns formation diagnostics; this query
            // selects only the delivery policy, as check_value_narrowing does.
            operand_domain.unwrap_or(parameter_domain)
        } else {
            parameter_domain
        };
    arithmetic_domains::check_value_narrowing(
        program,
        current_machine,
        current_state,
        argument,
        parameter_primitive,
        target_domain,
        value_environment,
        &owner,
        diagnostics,
    );
    // Parameter delivery is a store too. Width compatibility alone does not
    // establish the narrower declared range that the callee's reads assume.
}

fn report_accessor_scalar_write_predicate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    argument: ExpressionHandle,
    parameter: &StateParameter,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(primitive) = program.primitive_type_reference(parameter.type_reference) else {
        return;
    };
    let parent = program.symbols.get(parameter.symbol).parent;
    let mut fields = program.access_view_plans.iter().flat_map(|plan| {
        plan.fields
            .iter()
            .filter(move |field| {
                field
                    .accessor_targets
                    .iter()
                    .any(|target| target.state_symbol == parent && target.operation == "write")
            })
            .map(move |field| (plan, field))
    });
    let Some((plan, field)) = fields.next() else {
        return;
    };
    if fields.next().is_some() {
        diagnostics.push(Diagnostic::error(
            "placed scalar write has competing schema field origins",
        ));
        return;
    }
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == plan.schema_symbol)
    else {
        return;
    };
    let Some(bounds) = crate::data_where_field_intervals(program, definition) else {
        let dependent = program
            .data_members(definition)
            .iter()
            .filter_map(|member| member.as_field())
            .find(|source| source.symbol == field.field_symbol)
            .is_none_or(|source| {
                !crate::declarations::data_queries::data_field_is_where_independent(
                    program, definition, source,
                )
            });
        if dependent {
            diagnostics.push(Diagnostic::error(format!(
                "accessor write to schema field `{}` requires a complete source declaration predicate guard",
                field.field_name,
            )));
        }
        return;
    };
    let Some(required) = bounds
        .into_iter()
        .filter(|(symbol, _, _)| *symbol == field.field_symbol)
        .map(|(_, low, high)| Interval { low, high })
        .reduce(Interval::intersect)
    else {
        return;
    };
    if required.low.is_none() && required.high.is_none() {
        return;
    }
    let mut formation_diagnostics = Vec::new();
    let (actual, _, _) = arithmetic_domains::validate_value_range(
        program,
        machine,
        state,
        argument,
        environment,
        Some(primitive),
        ArithmeticDomain::Exact,
        "placed schema field payload",
        &mut formation_diagnostics,
    );
    let preserved = required
        .low
        .is_none_or(|minimum| actual.low.is_some_and(|value| value >= minimum))
        && required
            .high
            .is_none_or(|maximum| actual.high.is_some_and(|value| value <= maximum));
    if !formation_diagnostics.is_empty() || !preserved {
        diagnostics.push(Diagnostic::error(format!(
            "accessor write to schema field `{}` does not preserve its source declaration predicate",
            field.field_name,
        )));
    }
}

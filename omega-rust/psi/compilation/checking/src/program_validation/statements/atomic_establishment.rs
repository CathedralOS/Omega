//! A single-attempt initializer establishes its conditional resident write,
//! independently from constructing its outcome. Failure keeps the old resident.

use super::StatementScope;
use crate::proof_contracts::arithmetic_domains;
use crate::value_custody::places;
use diagnostics::Diagnostic;
use std::borrow::Cow;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_once_resident(
    scope: &StatementScope<'_>,
    local: &typed_trees::statement::TableLocalData,
    event: &crate::value_custody::atomic_operations::AtomicCarrierAccess,
    environment: &arithmetic_domains::ValueEnvironment,
    decisions: &mut arithmetic_domains::ArithmeticDecisions,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let program = scope.program;
    let Some(state) = scope.current_state else {
        return;
    };
    let Some(stated) = crate::proof_contracts::default_domains::stored_field_where_interval(
        program,
        scope.machine,
        state,
        event.place,
    ) else {
        return;
    };
    let Some(target_type) =
        places::declared_place_type_raw(program, scope.machine, Some(state), event.place)
    else {
        return;
    };
    let Some(resident_type) = crate::atomic_resident_type_reference(program, target_type) else {
        return;
    };
    let Some(replacement) = event.operands.get(1).copied() else {
        return;
    };
    let primitive = program.primitive_type_reference(resident_type);
    let domain = program.arithmetic_domain_for_type_reference(resident_type);
    let before = diagnostics.len();
    let anonymous = arithmetic_domains::validate_anonymous_integer_range(
        program,
        resident_type,
        replacement,
        owner,
        diagnostics,
    );
    // Receiver and expected evaluation precede the replacement. Source entry
    // facts are not a substitute for the replacement's actual completion cut.
    let observed = if anonymous.is_some() {
        Cow::Borrowed(environment)
    } else if let Some(observed) = arithmetic_domains::occurrence_value_environment(
        program,
        scope.machine.symbol,
        state.symbol,
        replacement,
    ) {
        Cow::Owned(observed)
    } else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} cannot establish atomic resident validity at replacement completion"
        )));
        return;
    };
    let (interval, source_primitive, source_domain) = anonymous.unwrap_or_else(|| {
        arithmetic_domains::validate_value_range(
            program,
            scope.machine,
            Some(state),
            replacement,
            &observed,
            primitive,
            domain,
            owner,
            diagnostics,
        )
    });
    let (source_primitive, source_domain) = arithmetic_domains::stored_value_carrier(
        program,
        scope.machine,
        Some(state),
        replacement,
        source_primitive,
        source_domain,
    );
    arithmetic_domains::check_narrowing_assignment(
        primitive,
        domain,
        interval,
        source_primitive,
        source_domain,
        owner,
        diagnostics,
    );
    let contained = if anonymous.is_some() {
        arithmetic_domains::check_where_interval_containment(
            program,
            stated,
            arithmetic_domains::IntegerValueObservation {
                machine: scope.machine,
                state: Some(state),
                expression: replacement,
                environment: &observed,
                interval,
            },
            owner,
            diagnostics,
        )
    } else {
        let proven =
            crate::proof_contracts::contract_entailment::completed_value_fits_integer_interval(
                program,
                scope.machine,
                state,
                scope.statement_index,
                replacement,
                stated,
            );
        if !proven {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} stores a value not provably within its data's `where` facts at replacement completion"
            )));
        }
        proven
    };
    if contained
        && diagnostics.len() == before
        && let Some(decision) = super::integer_establishment::checked_integer_establishment(
            program,
            scope.machine,
            state,
            scope.statement_index,
            event.place,
            local.initial_value,
            target_type,
            stated,
        )
    {
        decisions.integer_assignments.push(decision);
    }
}

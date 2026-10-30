//! Project a settled whole-plan row onto its retained executable coordinates.
//! Template settlement owns original map identity and complete finite coverage.
//! Selected requirement rows separately retain outer applications and local
//! tuples. Keep both axes: equal bodies do not erase phantom receiver arguments,
//! and closing only the outer trait does not fabricate a specialization receipt.

use super::{AdapterReceiver, AdapterRow, ResolvedAdapterRow};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

pub(super) fn project(
    typed: &TypedTrees,
    declarations: &typed_trees::references::DeclarationSelection,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    specializations: &[effects::SelectedRequirementSpecialization],
    identities: &terminal_psi::TerminalIdentityTable,
    settled: Vec<ResolvedAdapterRow>,
) -> Result<Vec<ResolvedAdapterRow>, Diagnostic> {
    // Top-level requirements own their application projection already. An
    // ineligible trait family retains its explicit refusal, not an executable
    // roster; selected body facts cannot turn partial coverage into dispatch.
    if settled.iter().any(|resolved| match resolved {
        ResolvedAdapterRow::Adapter(adapter) => {
            matches!(adapter.receiver, AdapterReceiver::Direct { .. })
        }
        ResolvedAdapterRow::GenericRequirement(_) => true,
    }) {
        return Ok(settled);
    }
    let effects::provider_plan::ProviderBinding::CheckedAdapter {
        machine_identity, ..
    } = row.binding
    else {
        return Ok(settled);
    };
    let coordinates = specializations
        .iter()
        .filter(|coordinate| {
            coordinate.provider_plan_report_identity == plan.report_fingerprint()
                && identities.text(coordinate.selected_requirement_identity)
                    == Some(plan.identities.text(row.requirement_identity))
                && identities.text(coordinate.selected_machine_identity)
                    == Some(plan.identity_text(machine_identity))
        })
        .collect::<Vec<_>>();
    let mut projected = Vec::new();
    let mut projection_counts = vec![0usize; coordinates.len()];
    for resolved in settled {
        let ResolvedAdapterRow::Adapter(adapter) = resolved else {
            projected.push(resolved);
            continue;
        };
        if coordinates.is_empty() {
            projected.push(ResolvedAdapterRow::Adapter(adapter));
            continue;
        }
        for (coordinate_index, coordinate) in coordinates.iter().enumerate() {
            let identity = identities
                .text(coordinate.requirement_identity)
                .unwrap_or_default();
            let mut applications = typed
                .closed_requirement_applications()
                .filter(|application| {
                    application.template == adapter.requirement_symbol
                        && typed
                            .trait_definition_by_symbol(application.declaring_trait)
                            .is_some_and(|owner| {
                                typed.trait_requirement_callable_identity_text(
                                    owner,
                                    application.signature,
                                ) == identity
                            })
                });
            let Some(application) = applications.next() else {
                continue;
            };
            if applications.next().is_some() {
                return Err(Diagnostic::error(
                    "selected trait coordinate has multiple executable requirement applications",
                ));
            }
            let Some(realization) = typed.machine_by_callable_identity(
                typed.callable_identity_by_text(
                    identities
                        .text(coordinate.machine_identity)
                        .unwrap_or_default(),
                ),
                declarations,
            ) else {
                return Err(Diagnostic::error(
                    "selected trait coordinate lost its realization body",
                ));
            };
            let Some(entry) = typed.machine_states(realization).first() else {
                return Err(Diagnostic::error(
                    "selected trait coordinate lost its executable entry",
                ));
            };
            if entry.symbol != adapter.symbol {
                continue;
            }
            let tuple = typed
                .expression_table
                .static_machine_application_arguments(application.application)
                .iter()
                .map(|argument| typed.static_const_argument_identity(argument))
                .collect::<Option<Vec<_>>>();
            if tuple.as_deref() != Some(adapter.family_tuple.as_ref()) {
                continue;
            }
            let mut closed: AdapterRow = adapter.clone();
            closed.requirement_symbol = application.signature.symbol;
            closed.requirement_identity = identity.to_owned();
            projected.push(ResolvedAdapterRow::Adapter(closed));
            projection_counts[coordinate_index] += 1;
        }
    }
    // Filtering one template row by body and tuple is not permission to drop
    // a selected coordinate. Every retained coordinate, including uncalled
    // ones, must be associated once with the complete settled row roster.
    // This consumes the selected relation; it does not recheck conformance.
    if projection_counts.iter().any(|count| *count != 1) {
        return Err(Diagnostic::error(
            "selected trait coordinate roster requires exactly one executable adapter projection per coordinate",
        ));
    }
    Ok(projected)
}

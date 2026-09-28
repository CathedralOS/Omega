//! Rebind template-keyed selected plan rows to their exact closed
//! requirement instances once specialization evidence exists.
//!
//! Provider plans derive before checked specialization settles, so a generic
//! token-machine requirement's selected row retains the authored template
//! identities while Terminal boundary declarations and provider candidates
//! name the closed identities specialization minted. This derivation replays
//! the retained specialization receipts and the provider instance's recorded
//! operator realizations — never display names or runtime signatures — and
//! publishes the exact template↔closed roster the selected closure carries.
//! Every retained closed requirement instance of a bound template must
//! resolve to exactly one specialized provider instance of the selected plan;
//! missing or ambiguous coverage rejects here rather than surfacing as a
//! mismatched Terminal identity downstream.

use super::adapter_conformance::exact_checked_adapter;
use super::requirement_identities::satisfied_row_requirement;
use crate::provider_planning::ProviderBinding;

pub(crate) fn derive_selected_requirement_specializations(
    checked: &checked_trees::CheckedTrees,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<effects::SelectedRequirementSpecialization>, Vec<diagnostics::Diagnostic>> {
    let typed = &checked.typed;
    let mut diagnostics = Vec::new();
    let mut rows = Vec::new();
    let mut top_level_symbols = None;
    for plan in selected.plans() {
        let templates = typed
            .machines()
            .iter()
            .filter(|requirement| {
                requirement.supply_mode
                    == language_semantics::MachineSupplyMode::TopLevelRequirement
                    && !typed.machine_type_parameters(requirement).is_empty()
                    && crate::service_schema::from_typed_boundary_requirement(typed, requirement)
                        .as_ref()
                        == Some(&plan.schema)
            })
            .collect::<Vec<_>>();
        let [template] = templates.as_slice() else {
            if templates.len() > 1 {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` binds {} generic requirement templates",
                    plan.name,
                    templates.len(),
                )));
            }
            continue;
        };
        let template_identity = typed
            .normalized_machine_overload_identity(template)
            .map(|identity| identity.identity())
            .unwrap_or_default();
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter {
                machine_identity: selected_machine_identity,
                machine_package_identity,
            } = &row.binding
            else {
                continue;
            };
            if row.requirement_identity != template_identity {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` checked-adapter row `{}` does not retain the generic requirement template `{template_identity}` its schema binds",
                    plan.name, row.requirement_identity,
                )));
                continue;
            }
            let provider = match exact_checked_adapter(typed, plan, row) {
                Ok(provider) => provider,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            for receipt in typed
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.template == provider.symbol)
            {
                // A provider instance may realize several requirement
                // templates through separate conformance edges; only the
                // retained realization naming this template serves it.
                if !receipt
                    .operator_realizations
                    .iter()
                    .any(|application| application.requirement_symbol == template.symbol)
                {
                    continue;
                }
                let Some(instance) = typed
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == receipt.instance)
                else {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "selected provider `{}` specialization retains no typed instance",
                        provider.name,
                    )));
                    continue;
                };
                if !typed.machine_type_parameters(instance).is_empty() {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "selected provider specialization instance `{}` remains generic",
                        instance.name,
                    )));
                    continue;
                }
                if typed.symbols.symbol_package_identity(instance.symbol)
                    != *machine_package_identity
                {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "selected provider instance `{}` drifts from its template's exact package identity",
                        instance.name,
                    )));
                    continue;
                }
                let Some(closed) =
                    satisfied_row_requirement(typed, &mut top_level_symbols, instance, template)
                else {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "selected provider instance `{}` realizes requirement template `{}` without one exact closed requirement",
                        instance.name, template.name,
                    )));
                    continue;
                };
                if closed.symbol == template.symbol {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "selected provider instance `{}` replays requirement template `{}` onto itself",
                        instance.name, template.name,
                    )));
                    continue;
                }
                let requirement_identity = typed
                    .normalized_machine_overload_identity(closed)
                    .map(|identity| identity.identity())
                    .unwrap_or_default();
                let machine_identity = typed
                    .normalized_machine_overload_identity(instance)
                    .map(|identity| identity.identity())
                    .unwrap_or_default();
                if requirement_identity.is_empty() || machine_identity.is_empty() {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "selected provider instance `{}` or closed requirement `{}` has no normalized identity",
                        instance.name, closed.name,
                    )));
                    continue;
                }
                rows.push(effects::SelectedRequirementSpecialization {
                    provider_plan_report_identity: plan.report_fingerprint(),
                    selected_requirement_identity: row.requirement_identity.clone(),
                    selected_machine_identity: selected_machine_identity.clone(),
                    requirement_identity,
                    machine_identity,
                });
            }
        }
        // Coverage: every retained closed requirement instance of the bound
        // template must resolve through exactly one roster row under the
        // selected plan. A closed instance exists because checked demand
        // applied the template, so an uncovered one names a provider that can
        // never serve it.
        for closed_receipt in typed
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.template == template.symbol)
        {
            let Some(instance) = typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == closed_receipt.instance)
            else {
                continue;
            };
            if instance.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement {
                continue;
            }
            let Some(identity) = typed
                .normalized_machine_overload_identity(instance)
                .map(|identity| identity.identity())
            else {
                continue;
            };
            let matching = rows
                .iter()
                .filter(|row| {
                    row.provider_plan_report_identity == plan.report_fingerprint()
                        && row.requirement_identity == identity
                })
                .count();
            if matching != 1 {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` resolves retained closed requirement `{identity}` to {matching} specialized provider instances; exactly one must serve it",
                    plan.name,
                )));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(rows)
    } else {
        Err(diagnostics)
    }
}

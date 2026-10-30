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
use checked_trees::typed_trees::machine::Machine;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderPlan, ProviderPlanRow};

mod trait_requirements;

struct CheckedAdapterDeclaration<'typed, 'plan> {
    plan: &'plan ProviderPlan,
    row: &'plan ProviderPlanRow,
    machine: Result<&'typed Machine, Diagnostic>,
}

/// Invocation scratch: one immutable source judgment per original selected row.
/// Errors remain data until each existing expansion site consumes that row.
struct CheckedAdapterDeclarations<'typed, 'plan> {
    rows: Vec<CheckedAdapterDeclaration<'typed, 'plan>>,
}

impl<'typed, 'plan> CheckedAdapterDeclarations<'typed, 'plan> {
    fn prepare(
        typed: &'typed checked_trees::typed_trees::TypedTrees,
        selected: &'plan effects::SelectedProviderPlanFacts,
    ) -> Self {
        let context = crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
        let mut rows = Vec::new();
        for plan in selected.plans() {
            for row in &plan.rows {
                if matches!(row.binding, ProviderBinding::CheckedAdapter { .. }) {
                    rows.push(CheckedAdapterDeclaration {
                        plan,
                        row,
                        machine: exact_checked_adapter(context, plan, row),
                    });
                }
            }
        }
        Self { rows }
    }

    fn machine(
        &self,
        plan: &ProviderPlan,
        row: &ProviderPlanRow,
    ) -> Result<&'typed Machine, Diagnostic> {
        self.rows
            .iter()
            .find(|prepared| std::ptr::eq(prepared.plan, plan) && std::ptr::eq(prepared.row, row))
            .ok_or_else(|| {
                Diagnostic::error(
                    "checked provider instance lost its original prepared plan/row custody",
                )
            })?
            .machine
            .clone()
    }

    fn callback_declarations(&self) -> Vec<provider_selection::SelectedCallbackRow<'plan>> {
        self.rows.iter().map(|prepared| provider_selection::SelectedCallbackRow {
            plan: prepared.plan,
            row: prepared.row,
            checked_adapter_body: prepared.machine.as_ref()
                .map(|machine| machine.supplies_checked_body())
                .map_err(|diagnostic| format!("callback forwarding provider is not its exact checked adapter: {diagnostic:?}")),
        }).collect()
    }
}

/// Complete source-only instance preparation, then return only whole plans,
/// exact row facts and closed applications. The result borrows selected-plan
/// custody, not the checked program. Callback policy runs in provider-selection.
pub fn prepare_callback_providers<'plan>(
    program: &checked_trees::CheckedTrees,
    selected: &'plan effects::SelectedProviderPlanFacts,
    declarations: &terminal_psi::CallbackDeclarationRoster,
) -> Result<provider_selection::CallbackProviderDeclarations<'plan>, Vec<Diagnostic>> {
    if declarations.uses.is_empty() {
        return Ok(provider_selection::CallbackProviderDeclarations::unused());
    }
    let adapters = CheckedAdapterDeclarations::prepare(&program.typed, selected);
    let instances = project_with_adapters(&program.typed, &program.facts, selected, &adapters)?;
    Ok(provider_selection::CallbackProviderDeclarations::prepared(
        instances,
        adapters.callback_declarations(),
    ))
}

/// Source-only consumers request this projection at their boundary. Ordinary
/// native production must not reconstruct checked instances just to discard
/// them when it joins the canonical Terminal provider roster.
pub fn project_checked_provider_instances(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<effects::SelectedProviderPlanFacts, Vec<diagnostics::Diagnostic>> {
    let adapters = CheckedAdapterDeclarations::prepare(typed, selected);
    project_with_adapters(typed, check_facts, selected, &adapters)
}

fn project_with_adapters(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    selected: &effects::SelectedProviderPlanFacts,
    adapters: &CheckedAdapterDeclarations<'_, '_>,
) -> Result<effects::SelectedProviderPlanFacts, Vec<diagnostics::Diagnostic>> {
    let (rows, identities) =
        derive_selected_requirement_specializations(typed, check_facts, selected, adapters)?;
    selected
        .clone()
        .with_specialized_requirement_rows(rows, &identities)
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])
}

fn derive_selected_requirement_specializations(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    selected: &effects::SelectedProviderPlanFacts,
    adapters: &CheckedAdapterDeclarations<'_, '_>,
) -> Result<
    (
        Vec<effects::SelectedRequirementSpecialization>,
        terminal_psi::TerminalIdentityTable,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    let provider_context =
        crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let generic_top_level_templates = typed
        .machines()
        .iter()
        .filter(|requirement| {
            requirement.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                && !typed.machine_type_parameters(requirement).is_empty()
        })
        .filter_map(|requirement| {
            let schema = crate::service_schema::from_typed_boundary_requirement(
                provider_context,
                requirement,
            )?;
            Some((requirement, schema))
        })
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    let mut rows = Vec::new();
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    for plan in selected.plans() {
        trait_requirements::append(
            typed,
            check_facts,
            plan,
            adapters,
            &mut rows,
            &mut identities,
            &mut diagnostics,
        );
        let templates = generic_top_level_templates
            .iter()
            .filter_map(|(requirement, schema)| (*schema == plan.schema).then_some(*requirement))
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
            .machine_callable_identity_text(template)
            .unwrap_or_default();
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter {
                machine_identity: selected_machine_identity,
                machine_package_identity,
            } = &row.binding
            else {
                continue;
            };
            if plan.identities.text(row.requirement_identity) != template_identity.as_str() {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` checked-adapter row `{}` does not retain the generic requirement template `{template_identity}` its schema binds",
                    plan.name,
                    plan.identities.text(row.requirement_identity),
                )));
                continue;
            }
            let provider = match adapters.machine(plan, row) {
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
                let Some(instance) = typed.machine(receipt.instance) else {
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
                    satisfied_row_requirement(typed, check_facts, instance, template)
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
                    .machine_callable_identity_text(closed)
                    .unwrap_or_default();
                let machine_identity = typed
                    .machine_callable_identity_text(instance)
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
                    selected_requirement_identity: identities
                        .intern(plan.identities.text(row.requirement_identity)),
                    selected_machine_identity: identities
                        .intern(plan.identity_text(*selected_machine_identity)),
                    requirement_identity: identities.intern(&requirement_identity),
                    machine_identity: identities.intern(&machine_identity),
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
            let Some(instance) = typed.machine(closed_receipt.instance) else {
                continue;
            };
            if instance.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement {
                continue;
            }
            let Some(identity) = typed.machine_callable_identity_text(instance) else {
                continue;
            };
            let matching = rows
                .iter()
                .filter(|row| {
                    row.provider_plan_report_identity == plan.report_fingerprint()
                        && identities.text(row.requirement_identity) == Some(identity.as_str())
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
        Ok((rows, identities.build()))
    } else {
        Err(diagnostics)
    }
}

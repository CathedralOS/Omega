//! Project checked trait-family coordinates through their one selected plan.
//!
//! The named map continues to choose the authored realization template. Psi's
//! closed-requirement facts and exact specialization receipts determine each
//! executable tuple. Inherited coordinates borrow that same plan identity;
//! they neither create parent plans nor choose a compatible body by its ABI.

use crate::provider_planning::{ProviderBinding, ProviderPlan};

pub(super) fn append(
    checked: &checked_trees::CheckedTrees,
    plan: &ProviderPlan,
    rows: &mut Vec<effects::SelectedRequirementSpecialization>,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let context = crate::ProviderPlanningContext::new(checked, source::DependencyScope::Product);
    for row in &plan.rows {
        let ProviderBinding::CheckedAdapter {
            machine_identity: selected_machine_identity,
            machine_package_identity,
        } = &row.binding
        else {
            continue;
        };
        let applications = checked
            .closed_requirement_applications()
            .filter(|application| {
                checked
                    .trait_machine_signature(application.template)
                    .is_some_and(|(owner, signature)| {
                        checked.trait_requirement_callable_identity_text(owner, signature)
                            == plan.identities.text(row.requirement_identity)
                            && crate::service_schema::from_typed_instance(
                                context,
                                &[],
                                owner,
                                application.trait_arguments,
                            )
                            .is_some_and(|demand| plan.schema.supplies_receiver_schema(&demand))
                    })
            })
            .collect::<Vec<_>>();
        if applications.is_empty() {
            continue;
        }
        let provider =
            match super::super::adapter_conformance::exact_checked_adapter(context, plan, row) {
                Ok(provider) => provider,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
        for application in applications {
            let matches = checked
                .facts
                .provider_closed_requirements
                .iter()
                .filter(|fact| {
                    fact.template == application.template
                        && fact.closed_requirement == application.signature.symbol
                        && checked.requirement_realization_matches(
                            fact.closed_requirement,
                            provider.symbol,
                            fact.provider,
                        )
                })
                .collect::<Vec<_>>();
            let [fact] = matches.as_slice() else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` resolves requirement family coordinate to {} exact checked realization instances; exactly one must serve it",
                    plan.name, matches.len(),
                )));
                continue;
            };
            let Some(instance) = checked.machine(fact.provider) else {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "closed provider fact lost its realization body",
                ));
                continue;
            };
            if !checked.machine_type_parameters(instance).is_empty()
                || checked.symbols.symbol_package_identity(instance.symbol)
                    != *machine_package_identity
            {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "closed provider realization has an open telescope or a different package",
                ));
                continue;
            }
            let Some(owner) = checked.trait_definition_by_symbol(application.declaring_trait)
            else {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "closed requirement fact lost its declaring trait",
                ));
                continue;
            };
            let requirement_identity =
                checked.trait_requirement_callable_identity_text(owner, application.signature);
            let machine_identity = checked
                .machine_callable_identity_text(instance)
                .unwrap_or_default();
            if requirement_identity.is_empty() || machine_identity.is_empty() {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "closed provider coordinate has no normalized callable identity",
                ));
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
}

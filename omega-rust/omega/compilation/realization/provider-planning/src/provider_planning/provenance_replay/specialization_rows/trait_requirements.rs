//! Project checked trait-family coordinates through their one selected plan.
//!
//! The named map continues to choose the authored realization template. Psi's
//! closed-requirement facts and exact specialization receipts determine each
//! executable tuple. Inherited coordinates borrow that same plan identity;
//! they neither create parent plans nor choose a compatible body by its ABI.

use crate::provider_planning::{ProviderBinding, ProviderPlan};

pub(super) fn append(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    plan: &ProviderPlan,
    rows: &mut Vec<effects::SelectedRequirementSpecialization>,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let context = crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let selecting_map = crate::provider_identity::named_provider(context, plan);
    for row in &plan.rows {
        if matches!(
            row.binding,
            ProviderBinding::Import { .. }
                | ProviderBinding::CompilerIntrinsic { .. }
                | ProviderBinding::Syscall { .. }
        ) {
            append_leaf_rows(typed, plan, row, rows, identities, diagnostics);
            continue;
        }
        let ProviderBinding::CheckedAdapter {
            machine_identity: selected_machine_identity,
            machine_package_identity,
        } = &row.binding
        else {
            continue;
        };
        let provider =
            match super::super::adapter_conformance::exact_checked_adapter(context, plan, row) {
                Ok(provider) => provider,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
        let applications = typed
            .closed_requirement_applications()
            .filter(|application| {
                typed
                    .trait_machine_signature(application.template)
                    .is_some_and(|(owner, signature)| {
                        typed.trait_requirement_callable_identity_text(owner, signature)
                            == plan.identities.text(row.requirement_identity)
                    })
            })
            .collect::<Vec<_>>();
        if applications.is_empty() {
            continue;
        }
        for application in applications {
            // A named selection owns these associations even when other maps
            // choose the same realization template. Direct satisfactions are
            // instead owned by the concrete machine that carries the edge.
            // Counting body-only facts would turn those independent maps into
            // duplicate instances of this one selected requirement.
            // Join once: a direct satisfaction is owned by its checked concrete
            // instance, not the authored template selected by the plan. Testing
            // template ownership in an earlier filter loses valid instances.
            // The recorded receiving application retains the checked region
            // substitution; an unapplied pattern schema cannot replace it.
            let matches = check_facts
                .provider_closed_requirements
                .iter()
                .filter(|fact| {
                    fact.template == application.template
                        && fact.closed_requirement == application.signature.symbol
                        && fact.receiving_application == application.trait_application_identity
                        && fact.selecting_declaration
                            == selecting_map.map_or(fact.provider, |map| map.symbol)
                        && typed.requirement_realization_matches(
                            fact.closed_requirement,
                            provider.symbol,
                            fact.provider,
                        )
                })
                .collect::<Vec<_>>();
            if matches.is_empty() {
                continue;
            }
            let [fact] = matches.as_slice() else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` resolves requirement family coordinate to {} exact checked realization instances; exactly one must serve it",
                    plan.name, matches.len(),
                )));
                continue;
            };
            let Some(instance) = typed.machine(fact.provider) else {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "closed provider fact lost its realization body",
                ));
                continue;
            };
            if !typed.machine_type_parameters(instance).is_empty()
                || typed.symbols.symbol_package_identity(instance.symbol)
                    != *machine_package_identity
            {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "closed provider realization has an open telescope or a different package",
                ));
                continue;
            }
            let Some(owner) = typed.trait_definition_by_symbol(application.declaring_trait) else {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "closed requirement fact lost its declaring trait",
                ));
                continue;
            };
            let requirement_identity =
                typed.trait_requirement_callable_identity_text(owner, application.signature);
            let machine_identity = typed
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

/// A leaf binding (a foreign import, compiler intrinsic or system call) has
/// no provider instance per static tuple: the one physical leaf realizes every
/// closed application of its template, and method-level selections such as a
/// native callback's machine are materialized at each call. Each closed
/// application therefore rebinds to the template row itself, and the row
/// names the template as its own realization.
fn append_leaf_rows(
    typed: &checked_trees::typed_trees::TypedTrees,
    plan: &ProviderPlan,
    row: &crate::provider_planning::ProviderPlanRow,
    rows: &mut Vec<effects::SelectedRequirementSpecialization>,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let template = plan.identities.text(row.requirement_identity);
    for application in typed
        .closed_requirement_applications()
        .filter(|application| {
            typed
                .trait_machine_signature(application.template)
                .is_some_and(|(owner, signature)| {
                    typed.trait_requirement_callable_identity_text(owner, signature) == template
                })
        })
    {
        let Some(owner) = typed.trait_definition_by_symbol(application.declaring_trait) else {
            diagnostics.push(diagnostics::Diagnostic::error(
                "closed requirement application lost its declaring trait",
            ));
            continue;
        };
        let requirement_identity =
            typed.trait_requirement_callable_identity_text(owner, application.signature);
        if requirement_identity.is_empty() || requirement_identity == template {
            continue;
        }
        let template = identities.intern(template);
        rows.push(effects::SelectedRequirementSpecialization {
            provider_plan_report_identity: plan.report_fingerprint(),
            selected_requirement_identity: template,
            selected_machine_identity: template,
            requirement_identity: identities.intern(&requirement_identity),
            machine_identity: template,
        });
    }
}

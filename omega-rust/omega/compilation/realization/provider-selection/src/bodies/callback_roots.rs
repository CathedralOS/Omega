// Include the functions that a reached foreign registration or installed entry
// can call when selecting their target operations. Keep dormant uses in the
// checked roster without treating them as executed code.
//
// CallbackProviderDeclarations owns foreign-versus-checked forwarding and exact
// checked-body identity. Registration errors precede calling-plan errors.

use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use language_semantics::callback_declarations::CallbackRegistrationDeclaration;

pub struct SelectedCallbackRow<'plan> {
    pub plan: &'plan ProviderPlan,
    pub row: &'plan ProviderPlanRow,
    pub checked_adapter_body: Result<bool, String>,
}

pub struct CallbackProviderDeclarations<'plan> {
    selected: Option<effects::SelectedProviderPlanFacts>,
    checked_adapters: Vec<SelectedCallbackRow<'plan>>,
}

impl<'plan> CallbackProviderDeclarations<'plan> {
    pub fn prepare(
        selected: &'plan effects::SelectedProviderPlanFacts,
        declarations: &terminal_psi::CallbackDeclarationRoster,
        module: &terminal_psi::TerminalModule,
    ) -> Result<Self, Vec<Diagnostic>> {
        if declarations.uses.is_empty() {
            return Ok(Self::unused());
        }
        let instances =
            backend_plan::close_checked_provider_specializations(selected.clone(), module)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let mut checked_adapters = Vec::new();
        for plan in selected.plans() {
            for row in &plan.rows {
                if !matches!(row.binding, ProviderBinding::CheckedAdapter { .. }) {
                    continue;
                }
                checked_adapters.push(SelectedCallbackRow {
                    plan,
                    row,
                    checked_adapter_body: backend_plan::selected_checked_adapter_header(
                        plan, row, module,
                    )
                    .map(|header| header.supplies_checked_body),
                });
            }
        }
        Ok(Self::prepared(instances, checked_adapters))
    }

    pub fn unused() -> Self {
        Self {
            selected: None,
            checked_adapters: Vec::new(),
        }
    }

    pub fn prepared(
        selected: effects::SelectedProviderPlanFacts,
        checked_adapters: Vec<SelectedCallbackRow<'plan>>,
    ) -> Self {
        Self {
            selected: Some(selected),
            checked_adapters,
        }
    }

    // The installed operation owns these three ordinary helpers. Native scope
    // consumes the checked identity of each helper.
    // Each error remains attached to its operation until that operation is used.
    pub fn installed_helper_roots(
        module: &terminal_psi::TerminalModule,
    ) -> Vec<(
        semantic_vocabulary::OperationId,
        Result<[semantic_vocabulary::MachineId; 3], String>,
    )> {
        if module.installed_interrupt_registrations.is_empty() {
            return Vec::new();
        }
        let mut bodies = module
            .machines
            .iter()
            .filter_map(|machine| {
                machine.declaration.as_ref().map(|declaration| {
                    (
                        (declaration.callable, declaration.contract),
                        machine,
                        declaration,
                    )
                })
            })
            .collect::<Vec<_>>();
        bodies.sort_unstable_by_key(|(contract, _, _)| *contract);
        let mut applications = module.callback_applications.iter().collect::<Vec<_>>();
        applications.sort_unstable_by_key(|application| {
            (application.registrar, application.static_machine_ordinal)
        });
        let mut result = Vec::new();
        for registration in &module.installed_interrupt_registrations {
            let roots = (|| {
                let mut roots = Vec::with_capacity(3);
                for ordinal in [1, 2, 3] {
                    let key = (registration.operation, ordinal);
                    let first = applications.partition_point(|application| {
                        (application.registrar, application.static_machine_ordinal) < key
                    });
                    let mut selections = applications[first..].iter().take_while(|application| {
                        (application.registrar, application.static_machine_ordinal) == key
                    });
                    let (Some(selection), None) = (selections.next(), selections.next()) else {
                        return Err(format!(
                            "interrupt installation {:?} has no unique checked helper at ordinal {ordinal}",
                            registration.operation
                        ));
                    };
                    let first = bodies.partition_point(|(contract, _, _)| {
                        *contract < (selection.body.callable, selection.body.contract)
                    });
                    let mut matching = bodies[first..]
                        .iter()
                        .take_while(|(contract, _, _)| {
                            *contract == (selection.body.callable, selection.body.contract)
                        })
                        .filter(|(_, _, declaration)| {
                            declaration.same_identity_as(
                                &module.identities,
                                &selection.body,
                                &module.identities,
                            )
                        });
                    let (Some((_, body, _)), None) = (matching.next(), matching.next()) else {
                        return Err(format!(
                            "interrupt installation {:?} helper {ordinal} has no unique retained body",
                            registration.operation
                        ));
                    };
                    roots.push(body.id);
                }
                roots
                    .try_into()
                    .map_err(|_| "installed helper count changed during preparation".to_owned())
            })();
            result.push((registration.operation, roots));
        }
        result.sort_unstable_by_key(|(operation, _)| *operation);
        let mut first = 0;
        while first < result.len() {
            let end = result.partition_point(|(operation, _)| *operation <= result[first].0);
            if end != first + 1 {
                for (operation, roots) in &mut result[first..end] {
                    *roots = Err(format!(
                        "interrupt installation {operation:?} has duplicate retained registrations"
                    ));
                }
            }
            first = end;
        }
        result.dedup_by_key(|(operation, _)| *operation);
        result
    }

    fn selected_row(&self, identity: &str) -> Option<SelectedCallbackRow<'_>> {
        let (plan, row) = self.selected.as_ref()?.selected_requirement_row(identity)?;
        let checked_adapter_body = if matches!(row.binding, ProviderBinding::CheckedAdapter { .. })
        {
            self.checked_adapters.iter()
                .find(|prepared| prepared.plan == plan && prepared.row == row)
                .map(|prepared| prepared.checked_adapter_body.clone())
                .unwrap_or_else(|| Err("callback forwarding provider lost its exact prepared checked-adapter declaration".into()))
        } else {
            Ok(false)
        };
        Some(SelectedCallbackRow {
            plan,
            row,
            checked_adapter_body,
        })
    }
}

pub fn select_callback_declarations(
    declarations: &terminal_psi::CallbackDeclarationRoster,
    calling_requirements: &[terminal_psi::CallbackRequirementId],
    providers: &CallbackProviderDeclarations<'_>,
) -> Result<Vec<terminal_psi::CallbackUseId>, Vec<Diagnostic>> {
    if declarations.uses.is_empty() {
        return Ok(Vec::new());
    }
    if providers.selected.is_none() {
        return Err(vec![Diagnostic::error(
            "callback admission requires completed whole-selected-instance preparation",
        )]);
    }
    let mut roots = Vec::new();
    let mut diagnostics = Vec::new();
    for (usage_id, usage) in declarations.uses.iter() {
        let declaration = declarations.declaration(usage);
        let provider = match &declaration.registration {
            Ok(CallbackRegistrationDeclaration::External(identity)) => {
                providers.selected_row(identity)
            }
            _ => None,
        };
        let foreign_registration = match registration_is_foreign(declaration.registration, provider)
        {
            Ok(foreign) => foreign,
            Err(reason) => {
                diagnostics.push(Diagnostic::error(reason));
                continue;
            }
        };
        let calling_plan_matches = calling_requirements
            .iter()
            .filter(|requirement| **requirement == usage.requirement)
            .count();
        if !declaration.has_checked_entry {
            if foreign_registration && declaration.satisfaction_is_boundary {
                diagnostics.push(Diagnostic::error(
                    "foreign callback registration requires an explicit selected callback calling plan",
                ));
            } else if calling_plan_matches != 0 {
                diagnostics.push(Diagnostic::error(format!(
                    "nominal callback use for `{}` has a target calling plan without a checked callback entry",
                    declaration.requirement_overload,
                )));
            }
            continue;
        }
        if calling_plan_matches != 1 {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` resolves to {} target calling-plan realizations; exactly one is required",
                declaration.requirement_overload, calling_plan_matches,
            )));
            continue;
        }
        if foreign_registration {
            roots.push(usage_id);
        }
    }
    if diagnostics.is_empty() {
        Ok(roots)
    } else {
        Err(diagnostics)
    }
}

fn registration_is_foreign(
    declaration: Result<CallbackRegistrationDeclaration, String>,
    provider: Option<SelectedCallbackRow<'_>>,
) -> Result<bool, String> {
    match declaration? {
        CallbackRegistrationDeclaration::CheckedSource => return Ok(false),
        CallbackRegistrationDeclaration::External(_) => {}
    }
    let Some(provider) = provider else {
        return Ok(false);
    };
    match provider.row.binding {
        ProviderBinding::CheckedAdapter { .. } => {
            if !provider.checked_adapter_body? {
                return Err("callback forwarding provider does not supply a checked body".into());
            }
            Ok(false)
        }
        _ => Ok(true),
    }
}

#[cfg(test)]
#[path = "callback_roots_tests.rs"]
mod tests;

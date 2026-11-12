// Include the functions that a reached foreign registration or installed entry
// can call when selecting their target operations. Keep dormant uses in the
// checked roster without treating them as executed code.
//
// CallbackProviderDeclarations owns foreign-versus-checked forwarding and exact
// checked-body identity. Source provider demand and native admission consume
// those same decisions; registration errors still precede calling-plan errors.

use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use language_semantics::callback_declarations::CallbackRegistrationDeclaration;

// This borrowed index is built once for one canonical program. It associates
// emitted registrations with their checked selections without retaining another
// callback roster or treating dormant declarations as executed calls.
pub(crate) type CallbackApplicationUse<'program> = (
    terminal_psi::ProofDeclarationId,
    &'program terminal_psi::TerminalCallbackApplication,
    Result<terminal_psi::ProofDeclarationId, String>,
    Result<Vec<terminal_psi::CallbackUseId>, String>,
    bool, // Borrowed from an actual installed registration's callback roster.
);
pub(crate) type InstalledHelperUse = (
    terminal_psi::ProofDeclarationId,
    semantic_vocabulary::OperationId,
    Result<Vec<terminal_psi::ProofDeclarationId>, String>,
);

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
        provider_declarations: &[backend_plan::TerminalTargetProviderPlan],
        module: &terminal_psi::TerminalModule,
    ) -> Result<Self, Vec<Diagnostic>> {
        if declarations.uses.is_empty() {
            return Ok(Self::unused());
        }
        let instances = backend_plan::close_checked_provider_specializations(
            selected.clone(),
            provider_declarations,
            module,
        )
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

    // The installed operation owns these three ordinary helpers. Share its
    // existing exact evidence with native scope and source provider demand.
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

    pub(crate) fn application_uses<'program>(
        prepared: &psi_terminal_production::PreparedExternalProviderDeclarations,
        module: &'program terminal_psi::TerminalModule,
    ) -> Result<
        (
            Vec<CallbackApplicationUse<'program>>,
            Vec<InstalledHelperUse>,
        ),
        Vec<Diagnostic>,
    > {
        let scope = &module.scalar_applications.proof_scope;
        let mut callables = scope
            .callables
            .iter()
            .filter_map(|(_, association)| match association {
                terminal_psi::ProofCallableAssociation::Callable {
                    declaration,
                    callable,
                } => Some((*callable, *declaration)),
                terminal_psi::ProofCallableAssociation::Missing => None,
            })
            .collect::<Vec<_>>();
        callables.sort_unstable_by_key(|(callable, _)| *callable);
        let declaration_for = |callable| {
            let first = callables.partition_point(|(candidate, _)| *candidate < callable);
            let mut matches = callables[first..]
                .iter()
                .take_while(|(candidate, _)| *candidate == callable);
            match (matches.next(), matches.next()) {
                (Some((_, declaration)), None) => Some(*declaration),
                _ => None,
            }
        };
        // Reachability records each plan's machine, while an emitted function
        // may name one of its states. Use the plan's exact state relation, not
        // lexical ancestry, to compare registering callers with that result.
        let mut owners = Vec::new();
        for (_, machine) in scope.plan.machines.iter() {
            owners.push((machine.machine, machine.machine));
            owners.extend(
                scope
                    .plan
                    .symbols
                    .span_or_empty(machine.states)
                    .iter()
                    .map(|state| (*state, machine.machine)),
            );
        }
        owners.sort_unstable();
        owners.dedup();
        let owner_for = |declaration| {
            let first = owners.partition_point(|(candidate, _)| *candidate < declaration);
            let mut matching = owners[first..]
                .iter()
                .take_while(|(candidate, _)| *candidate == declaration);
            match (matching.next(), matching.next()) {
                (Some((_, owner)), None) => Some(*owner),
                _ => None,
            }
        };
        let mut operations = Vec::new();
        let mut bodies = Vec::new();
        for machine in &module.machines {
            let owner = declaration_for(terminal_psi::TargetMachineCallable::Machine(machine.id))
                .and_then(owner_for);
            for operation in machine.operations() {
                if let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = operation.kind {
                    operations.push((operation.id, owner, boundary));
                }
            }
        }
        operations.sort_unstable_by_key(|(operation, _, _)| *operation);
        for (ordinal, declaration) in scope.declarations.storage_slice().iter().enumerate() {
            if let terminal_psi::ProofScopeDeclaration::Callable(body) = declaration {
                let ordinal = u32::try_from(ordinal)
                    .ok()
                    .and_then(|ordinal| ordinal.checked_add(1))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "callback proof declaration namespace is exhausted",
                        )]
                    })?;
                bodies.push((
                    (body.callable, body.contract),
                    terminal_psi::ProofDeclarationId(ordinal),
                    body,
                ));
            }
        }
        bodies.sort_unstable_by_key(|(contract, _, _)| *contract);
        let mut requests = Vec::new();
        let applications = module
            .callback_applications
            .iter()
            .map(|application| (application, None))
            .chain(
                module
                    .installed_interrupt_registrations
                    .iter()
                    .flat_map(|registration| {
                        registration
                            .callbacks
                            .iter()
                            .map(|application| (application, Some(&registration.registrar)))
                    }),
            );
        for (application, installed) in applications {
            let first =
                operations.partition_point(|(operation, _, _)| *operation < application.registrar);
            let mut matching = operations[first..]
                .iter()
                .take_while(|(operation, _, _)| *operation == application.registrar);
            let (Some((_, Some(owner), boundary)), None) = (matching.next(), matching.next())
            else {
                return Err(vec![Diagnostic::error(
                    "callback demand has no unique registering proof declaration",
                )]);
            };
            let declaration = module
                .boundary_machine(*boundary)
                .and_then(|boundary| boundary.declaration.as_ref())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "callback demand lost its exact registrar declaration",
                    )]
                })?;
            let first = bodies.partition_point(|(contract, _, _)| {
                *contract < (application.body.callable, application.body.contract)
            });
            let mut matching = bodies[first..]
                .iter()
                .take_while(|(contract, _, _)| {
                    *contract == (application.body.callable, application.body.contract)
                })
                .filter(|(_, _, body)| {
                    application
                        .body
                        .same_identity_as(&module.identities, body, &module.identities)
                });
            let body = match (matching.next(), matching.next()) {
                (Some((_, body, _)), None) => Ok(*body),
                _ => {
                    Err("reached callback demand has no unique exact checked proof body".to_owned())
                }
            };
            let registrars = match installed {
                Some(registrar) => [registrar, registrar],
                None => [&declaration.source.machine, &declaration.source.state],
            };
            requests.push((*owner, application, body, registrars, installed.is_some()));
        }
        let associated = prepared
            .callbacks
            .corresponding_uses(
                &prepared.identities,
                &module.identities,
                requests
                    .iter()
                    .map(|(_, application, _, registrars, installed)| {
                        (*application, *registrars, *installed)
                    }),
            )
            .collect::<Vec<_>>();
        let result = requests
            .into_iter()
            .zip(associated)
            .map(|((owner, application, body, _, installed), uses)| {
                (owner, application, body, uses, installed)
            })
            .collect();
        let mut helpers = Vec::new();
        for (operation, roots) in Self::installed_helper_roots(module) {
            let first = operations.partition_point(|(candidate, _, _)| *candidate < operation);
            let mut callers = operations[first..]
                .iter()
                .take_while(|(candidate, _, _)| *candidate == operation);
            let (Some((_, Some(owner), _)), None) = (callers.next(), callers.next()) else {
                return Err(vec![Diagnostic::error(
                    "installed helper demand has no unique registering proof declaration",
                )]);
            };
            let roots = roots.and_then(|roots| {
                roots
                    .into_iter()
                    .map(|machine| {
                        declaration_for(terminal_psi::TargetMachineCallable::Machine(machine))
                            .ok_or_else(|| {
                                "reached installed helper lost its exact proof declaration"
                                    .to_owned()
                            })
                    })
                    .collect()
            });
            helpers.push((*owner, operation, roots));
        }
        Ok((result, helpers))
    }

    pub(crate) fn extend_reached_roots(
        associations: &[CallbackApplicationUse<'_>],
        selected: &[terminal_psi::CallbackUseId],
        installed: &[terminal_psi::CallbackUseId],
        reached: &[terminal_psi::ProofDeclarationId],
        roots: &mut Vec<terminal_psi::ProofDeclarationId>,
    ) -> Result<bool, Vec<Diagnostic>> {
        let before = roots.len();
        let mut additional = Vec::new();
        for (owner, application, body, uses, installed_entry) in associations {
            if reached.binary_search(owner).is_err() {
                continue;
            }
            // Non-entry helper selections are handled by the installed
            // operation's existing helper evidence, not as native callbacks.
            if !application.entry_admitted {
                continue;
            }
            let uses = uses
                .as_ref()
                .map_err(|reason| vec![Diagnostic::error(reason.clone())])?;
            if uses.is_empty() {
                return Err(vec![Diagnostic::error(
                    "reached callback occurrence lost its exact checked use",
                )]);
            }
            // These IDs are the existing owner's foreign/forwarding decision.
            // A checked registrar may leave its supplied policy unused. Installed
            // entries instead consume their own checked installation selection.
            let selected = if *installed_entry {
                installed
            } else {
                selected
            };
            if !uses.iter().any(|usage| {
                selected
                    .binary_search_by_key(&(usage.arena_index(), usage.generation()), |candidate| {
                        (candidate.arena_index(), candidate.generation())
                    })
                    .is_ok()
            }) {
                continue;
            }
            let body = body
                .as_ref()
                .map_err(|reason| vec![Diagnostic::error(reason.clone())])?;
            if roots.binary_search(body).is_err() {
                additional.push(*body);
            }
        }
        roots.extend(additional);
        roots.sort_unstable();
        roots.dedup();
        Ok(roots.len() != before)
    }

    pub(crate) fn extend_reached_installed_helpers(
        helpers: &[InstalledHelperUse],
        reached: &[terminal_psi::ProofDeclarationId],
        roots: &mut Vec<terminal_psi::ProofDeclarationId>,
    ) -> Result<bool, Vec<Diagnostic>> {
        let before = roots.len();
        let mut additional = Vec::new();
        for (owner, _, helpers) in helpers {
            if reached.binary_search(owner).is_err() {
                continue;
            }
            for helper in helpers
                .as_ref()
                .map_err(|reason| vec![Diagnostic::error(reason.clone())])?
            {
                if roots.binary_search(helper).is_err() {
                    additional.push(*helper);
                }
            }
        }
        roots.extend(additional);
        roots.sort_unstable();
        roots.dedup();
        Ok(roots.len() != before)
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

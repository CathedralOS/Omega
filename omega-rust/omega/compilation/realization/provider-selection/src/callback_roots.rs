//! Select pre-optimization callback roots from prepared occurrence evidence.
//!
//! Dormant and symbolic uses remain in their original roster. This policy does
//! not infer executable reachability or place ABI entries; Terminal settlement
//! owns native placement. Opaque root owners travel with their evidence.

use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use language_semantics::callback_declarations::CallbackRegistrationDeclaration;

/// The exact selected plan/row and its independently prepared source-body fact.
/// The body result is consumed only for a checked-adapter binding.
pub struct SelectedCallbackRow<'plan> {
    pub plan: &'plan ProviderPlan,
    pub row: &'plan ProviderPlanRow,
    pub checked_adapter_body: Result<bool, String>,
}

/// Source preparation has completed the whole selected instance projection.
/// Evidence borrows only the original immutable plans, never source trees.
/// The projected selection owns its closure metadata; its plan values remain
/// equal to the originals that supplied each exact checked-body declaration.
pub struct CallbackProviderDeclarations<'plan> {
    selected: Option<effects::SelectedProviderPlanFacts>,
    checked_adapters: Vec<SelectedCallbackRow<'plan>>,
}

impl<'plan> CallbackProviderDeclarations<'plan> {
    /// No nominal uses means no instance preparation is required.
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

    fn selected_row(&self, identity: &str) -> Option<SelectedCallbackRow<'_>> {
        let (plan, row) = self.selected.as_ref()?.selected_requirement_row(identity)?;
        let checked_adapter_body = if matches!(row.binding, ProviderBinding::CheckedAdapter { .. })
        {
            // The specialization projection clones plans. Compare the complete
            // plan and row, not pointers across that clone or compact reports.
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

/// Preserve occurrence order and duplicate roots. Registration errors precede
/// and suppress calling-plan errors for that occurrence, while independent
/// occurrences continue accumulating diagnostics.
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
        // Unselected declarations have no native placement. Their calling-plan
        // obligations still participate in the occurrence policy above.
        return Ok(false);
    };
    match provider.row.binding {
        ProviderBinding::CheckedAdapter { .. } => {
            if !provider.checked_adapter_body? {
                return Err("callback forwarding provider does not supply a checked body".into());
            }
            Ok(false)
        }
        // Imports, syscalls, intrinsics and opaque table dispatch are external;
        // none borrows the checked-adapter exemption.
        _ => Ok(true),
    }
}

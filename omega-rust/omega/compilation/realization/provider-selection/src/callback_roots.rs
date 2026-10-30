//! Select pre-optimization callback roots from prepared occurrence evidence.
//!
//! Dormant and symbolic uses remain in their original roster. This policy does
//! not infer executable reachability or place ABI entries; Terminal settlement
//! owns native placement. Opaque root owners travel with their evidence.

use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use language_semantics::callback_declarations::{
    CallbackDeclaration, CallbackRegistrationDeclaration,
};

/// The exact selected plan/row and its independently prepared source-body fact.
/// The body result is consumed only for a checked-adapter binding.
pub struct SelectedCallbackRow<'plan> {
    pub plan: &'plan ProviderPlan,
    pub row: &'plan ProviderPlanRow,
    pub checked_adapter_body: Result<bool, String>,
}

/// One original occurrence, never an emitted-demand substitute. Calling-plan
/// matches count exact requirement-owner/member associations with no open
/// boundary arguments; deciding the admissible cardinality belongs here.
pub struct CallbackRootCandidate<'usage, 'plan, Owner> {
    pub owner: Owner,
    pub declaration: CallbackDeclaration<'usage>,
    pub provider: Option<SelectedCallbackRow<'plan>>,
    pub calling_plan_matches: usize,
}

/// Preserve occurrence order and duplicate roots. Registration errors precede
/// and suppress calling-plan errors for that occurrence, while independent
/// occurrences continue accumulating diagnostics.
pub fn select_callback_roots<'usage, 'plan, Owner>(
    candidates: impl IntoIterator<Item = CallbackRootCandidate<'usage, 'plan, Owner>>,
) -> Result<Vec<Owner>, Vec<Diagnostic>> {
    let mut roots = Vec::new();
    let mut diagnostics = Vec::new();
    for candidate in candidates {
        let foreign_registration =
            match registration_is_foreign(candidate.declaration.registration, candidate.provider) {
                Ok(foreign) => foreign,
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(reason));
                    continue;
                }
            };
        if !candidate.declaration.has_checked_entry {
            if foreign_registration && candidate.declaration.satisfaction_is_boundary {
                diagnostics.push(Diagnostic::error(
                    "foreign callback registration requires an explicit selected callback calling plan",
                ));
            } else if candidate.calling_plan_matches != 0 {
                diagnostics.push(Diagnostic::error(format!(
                    "nominal callback use for `{}` has a target calling plan without a checked callback entry",
                    candidate.declaration.requirement_overload,
                )));
            }
            continue;
        }
        if candidate.calling_plan_matches != 1 {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` resolves to {} target calling-plan realizations; exactly one is required",
                candidate.declaration.requirement_overload, candidate.calling_plan_matches,
            )));
            continue;
        }
        if foreign_registration {
            roots.push(candidate.owner);
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

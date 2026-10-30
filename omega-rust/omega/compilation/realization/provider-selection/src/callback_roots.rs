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

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(binding: ProviderBinding) -> ProviderPlan {
        ProviderPlan {
            rows: vec![ProviderPlanRow {
                method: "visit".into(),
                requirement_identity: Default::default(),
                requirement_lifetime_partition: Vec::new(),
                admitted_same_stack_bytes: 0,
                binding,
            }],
            ..ProviderPlan::default()
        }
    }

    fn candidate<'plan>(
        owner: u32,
        plan: Option<&'plan ProviderPlan>,
        body: Result<bool, String>,
        has_entry: bool,
        matches: usize,
    ) -> CallbackRootCandidate<'static, 'plan, u32> {
        CallbackRootCandidate {
            owner,
            declaration: CallbackDeclaration {
                registration: Ok(CallbackRegistrationDeclaration::External(
                    "registrar".into(),
                )),
                satisfaction_is_boundary: true,
                has_checked_entry: has_entry,
                requirement_overload: "callback",
            },
            provider: plan.map(|plan| SelectedCallbackRow {
                plan,
                row: &plan.rows[0],
                checked_adapter_body: body,
            }),
            calling_plan_matches: matches,
        }
    }

    #[test]
    fn roots_preserve_occurrence_order_and_duplicates() {
        let foreign = plan(ProviderBinding::Syscall { number: 1 });
        let mut checked = candidate(99, None, Ok(false), true, 1);
        checked.declaration.registration = Ok(CallbackRegistrationDeclaration::CheckedSource);
        let roots = select_callback_roots([
            candidate(
                7,
                Some(&foreign),
                Err("inapplicable adapter error".into()),
                true,
                1,
            ),
            checked,
            candidate(8, Some(&foreign), Ok(false), true, 1),
            candidate(7, Some(&foreign), Ok(false), true, 1),
            candidate(100, None, Ok(false), true, 1),
        ])
        .expect("exact foreign occurrences");
        assert_eq!(roots, [7, 8, 7]);
    }

    #[test]
    fn registration_errors_suppress_only_their_own_plan_errors() {
        let mut missing = candidate(1, None, Ok(false), true, 0);
        missing.declaration.registration = Err("lost registration".into());
        let errors = select_callback_roots([missing, candidate(2, None, Ok(false), true, 2)])
            .expect_err("independent occurrence errors accumulate");
        let text = format!("{errors:?}");
        assert_eq!(errors.len(), 2);
        assert!(
            text.find("lost registration")
                .expect("registration diagnostic")
                < text.find("resolves to 2").expect("cardinality diagnostic")
        );
        assert!(!text.contains("resolves to 0"));
    }

    #[test]
    fn absent_entry_and_unselected_rows_keep_their_plan_obligations() {
        let foreign = plan(ProviderBinding::Syscall { number: 1 });
        let errors = select_callback_roots([
            candidate(1, Some(&foreign), Ok(false), false, 2),
            candidate(2, None, Ok(false), false, 1),
            candidate(3, None, Ok(false), true, 0),
        ])
        .expect_err("entry and cardinality obligations");
        assert_eq!(errors.len(), 3);
        assert!(
            format!("{:?}", errors[0])
                .contains("requires an explicit selected callback calling plan")
        );
        assert!(format!("{:?}", errors[1]).contains("without a checked callback entry"));
        assert!(format!("{:?}", errors[2]).contains("resolves to 0"));
    }

    #[test]
    fn checked_adapter_exemption_consumes_its_exact_body_evidence() {
        let checked = plan(ProviderBinding::CheckedAdapter {
            machine_identity: Default::default(),
            machine_package_identity: None,
        });
        assert!(
            select_callback_roots([candidate(1, Some(&checked), Ok(true), true, 1),])
                .expect("checked adapter is not a foreign root")
                .is_empty()
        );
        let errors = select_callback_roots([
            candidate(1, Some(&checked), Ok(false), true, 1),
            candidate(
                2,
                Some(&checked),
                Err("wrong package or application".into()),
                true,
                1,
            ),
        ])
        .expect_err("no unchecked adapter exemption");
        assert_eq!(errors.len(), 2);
        assert!(format!("{:?}", errors[0]).contains("does not supply a checked body"));
        assert!(format!("{:?}", errors[1]).contains("wrong package or application"));
    }
}

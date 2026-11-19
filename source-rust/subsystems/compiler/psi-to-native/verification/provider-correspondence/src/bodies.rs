/*
Selected execution bodies: Psi's checked bodies, external numeric execution
meaning, and pre-optimization callback roots.

select_terminal_checked_body joins selected rows to Psi's
checked declaration and application inventory without reconstructing source
conformance or specialization. A generic declaration is eligible policy, never
a substitute for the distinct closed-body roster a concrete application must
supply.

selected_external_execution joins the selected external leaf to Psi's exact
satisfaction edge and reconciles retained numeric execution meaning so a
same-signature provider or a declaration from another package cannot replace
the selected realization.
*/

mod callback_roots;

pub use callback_roots::{
    CallbackProviderDeclarations, SelectedCallbackRow, select_callback_declarations,
};

use crate::external_providers::reconcile_numeric_meaning;
use diagnostics::Diagnostic;
use native_intrinsics::CompilerIntrinsicExecutionIdentity;
use provider_plans::{ProviderBinding, ProviderPlan};

struct CheckedBodySelection<'plan> {
    callable: &'plan str,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    target: &'plan str,
}

impl<'plan> CheckedBodySelection<'plan> {
    fn new(
        plan: &'plan ProviderPlan,
        row: &provider_plans::ProviderPlanRow,
    ) -> Result<Self, Diagnostic> {
        let ProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity,
        } = &row.binding
        else {
            return Err(Diagnostic::error(format!(
                "ProviderPlan `{}` is not a checked-body selection",
                plan.name
            )));
        };
        if *machine_package_identity != plan.origin_package_identity {
            return Err(Diagnostic::error(
                "selected checked body belongs to a different package",
            ));
        }
        Ok(Self {
            callable: plan.identity_text(*machine_identity),
            package: *machine_package_identity,
            target: &plan.target,
        })
    }

    fn matches(
        &self,
        callable: &str,
        package: Option<semantic_vocabulary::PackageKeyIdentity>,
        target: Option<&str>,
    ) -> bool {
        callable == self.callable
            && package == self.package
            && target.is_none_or(|target| target == self.target)
    }
}

pub(crate) fn select_terminal_checked_body<'module>(
    module: &'module terminal_psi::TerminalModule,
    demand: &terminal_psi::TerminalBoundaryApplicationDemand,
    plan: &ProviderPlan,
    row: &provider_plans::ProviderPlanRow,
) -> Result<&'module terminal_psi::CheckedMachineDeclaration, Diagnostic> {
    let selection = CheckedBodySelection::new(plan, row)?;
    let mut candidates = module
        .interfaces.boundary_application_candidates
        .iter()
        .filter(|candidate| {
            candidate.requirement == *demand.requirement()
                && candidate.application == *demand.application()
                && module
                    .identities
                    .text(candidate.selection_identity)
                    .is_some_and(|callable| {
                        selection.matches(
                            callable,
                            candidate.selection_package,
                            candidate.target.as_deref(),
                        )
                    })
        });
    match (candidates.next(), candidates.next()) {
        (Some(candidate), None) => Ok(&candidate.body),
        _ => Err(Diagnostic::error(
            "selected boundary body has no unique checked Terminal candidate",
        )),
    }
}

pub fn selected_external_execution(
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    requirement: &terminal_psi::NominalDeclarationIdentity,
    plan: &ProviderPlan,
    row: &provider_plans::ProviderPlanRow,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
    realization: Option<&str>,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, String> {
    let requirement = identities
        .text(requirement.identity())
        .ok_or("numeric provider requirement has no nominal identity")?;
    if let Some(candidate) = crate::external_providers::selected_sealed_native_provider(
        external_providers,
        identities,
        requirement,
        realization,
        plan,
        row,
    )? {
        let expected =
            crate::external_providers::canonical_external_execution(candidate, &plan.target)?
                .ok_or("sealed native provider lost its canonical execution meaning")?;
        return crate::external_providers::reconcile_sealed_execution(expected, retained)
            .map(Some)
            .map_err(str::to_owned);
    }
    let candidate = crate::external_providers::selected_numeric_provider(
        external_providers,
        identities,
        requirement,
        realization,
        plan,
        row,
    )?;
    let Some(candidate) = candidate else {
        if retained.is_some_and(|execution| !execution.is_numeric()) {
            return Err(
                "selected nonnumeric compiler execution has no exact admitted native meaning"
                    .into(),
            );
        }
        reconcile_numeric_meaning(None, retained)?;
        return Ok(retained);
    };
    candidate.validate(identities)?;
    let meaning = candidate
        .numeric_intrinsic
        .ok_or("numeric provider has no canonical meaning")?;
    let execution = CompilerIntrinsicExecutionIdentity::from(meaning);
    if retained.is_some() {
        reconcile_numeric_meaning(Some(meaning), retained)?;
    }
    Ok(Some(execution))
}

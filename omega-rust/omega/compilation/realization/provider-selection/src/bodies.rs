/*
Selected execution bodies: Psi's checked bodies, external numeric execution
meaning, pre-optimization callback roots, and target-body availability.

select_checked_body and select_terminal_checked_body join selected rows to Psi's
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
pub(crate) use callback_roots::{CallbackApplicationUse, InstalledHelperUse};
mod target_bodies;

pub use callback_roots::{
    CallbackProviderDeclarations, SelectedCallbackRow, select_callback_declarations,
};
pub use language_semantics::CheckedBodyCandidate;
pub use target_bodies::{DirectTargetBodyAbsences, reject_reached_inert_calls};

use diagnostics::Diagnostic;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::external_providers::reconcile_numeric_meaning;
use effects::provider_plan::{ProviderBinding, ProviderPlan};

pub fn select_checked_body<'candidate, Body>(
    plan: &ProviderPlan,
    candidates: impl IntoIterator<Item = CheckedBodyCandidate<'candidate, Body>>,
) -> Result<Body, Diagnostic> {
    let [row] = plan.rows.as_slice() else {
        return Err(Diagnostic::error(format!(
            "checked boundary-operator ProviderPlan `{}` requires one realization row",
            plan.name,
        )));
    };
    let selection = CheckedBodySelection::new(plan, row)?;
    let mut matching = candidates.into_iter().filter(|candidate| {
        let declaration = candidate.declaration;
        candidate.is_product
            && declaration.is_product
            && declaration.requirement
                == plan
                    .schema
                    .identity_text(plan.schema.declaration.trait_name)
            && declaration.requirement_package == plan.schema.declaration.trait_package_identity
            && declaration.requirement == plan.identities.text(row.requirement_identity)
            && selection.matches(&declaration.callable, declaration.package, candidate.target)
            && declaration.provider == plan.provider_type
            && declaration.provider_package == plan.provider_type_package_identity
            && declaration.provider_application.as_deref() == plan.application_identity()
            && declaration.requirement_lifetime_partition == row.requirement_lifetime_partition
    });
    match (matching.next(), matching.next()) {
        (Some(candidate), None) => Ok(candidate.body),
        _ => Err(Diagnostic::error(format!(
            "selected checked boundary-operator ProviderPlan `{}` has no unique exact checked candidate",
            plan.name,
        ))),
    }
}

struct CheckedBodySelection<'plan> {
    callable: &'plan str,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    target: &'plan str,
}

impl<'plan> CheckedBodySelection<'plan> {
    fn new(
        plan: &'plan ProviderPlan,
        row: &effects::provider_plan::ProviderPlanRow,
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
    row: &effects::provider_plan::ProviderPlanRow,
) -> Result<&'module terminal_psi::CheckedMachineDeclaration, Diagnostic> {
    let selection = CheckedBodySelection::new(plan, row)?;
    let mut candidates = module
        .boundary_application_candidates
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
    row: &effects::provider_plan::ProviderPlanRow,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
    realization: Option<&str>,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, String> {
    let requirement = identities
        .text(requirement.identity())
        .ok_or("numeric provider requirement has no nominal identity")?;
    selected_execution_for_identity(
        external_providers,
        identities,
        requirement,
        plan,
        row,
        retained,
        realization,
    )
}

pub(crate) fn selected_execution_for_identity(
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    requirement: &str,
    plan: &ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
    realization: Option<&str>,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, String> {
    let candidate = effects::external_providers::selected_numeric_provider(
        external_providers,
        identities,
        requirement,
        realization,
        plan,
        row,
    )?;
    let Some(candidate) = candidate else {
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

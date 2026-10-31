//! Join selected rows to Psi's checked declaration/application inventory.
//! The caller supplies one declaration roster or one exact application's body
//! roster. This owner never reconstructs source conformance or specialization.

use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
pub use language_semantics::CheckedBodyCandidate;

/// Select exactly one retained declaration or one checked application body.
/// A generic declaration is eligible policy, never a substitute for the
/// distinct closed-body roster a concrete application must supply.
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
            && declaration.requirement == plan.schema.identity_text(plan.schema.trait_name)
            && declaration.requirement_package == plan.schema.trait_package_identity
            && declaration.requirement == plan.identities.text(row.requirement_identity)
            && selection.matches(&declaration.callable, declaration.package)
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

/// The common callable/package/target join. Source declarations additionally
/// establish provider application, lifetime and Product eligibility; canonical
/// candidates instead bind an exact nominal requirement and closed application.
/// Neither representation invents the fields owned by the other scope.
struct CheckedBodySelection<'plan> {
    callable: &'plan str,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
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
        })
    }

    fn matches(
        &self,
        callable: &str,
        package: Option<semantic_vocabulary::PackageKeyIdentity>,
    ) -> bool {
        callable == self.callable && package == self.package
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
                        selection.matches(callable, candidate.selection_package)
                    })
        });
    match (candidates.next(), candidates.next()) {
        (Some(candidate), None) => Ok(&candidate.body),
        _ => Err(Diagnostic::error(
            "selected boundary body has no unique checked Terminal candidate",
        )),
    }
}

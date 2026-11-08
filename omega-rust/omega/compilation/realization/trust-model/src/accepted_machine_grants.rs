// We resolve non-provider root trust grants (`resolve_non_provider_trust_grant`)
// against Psi's `TrustDeclarations` roster and enforce package-level non-provider
// grant rejection (`reject_package_non_provider_grants`). Provider selectors
// resolve as a complete pass before non-provider selectors so package-aware
// compilations reject individual accepted-machine grants while permitting
// selected provider grants.

use crate::ResolvedSelectedProviderGrant;
use diagnostics::Diagnostic;
use language_semantics::trust_declarations::{AcceptedMachineDeclaration, TrustDeclarations};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NonProviderTrustGrant {
    AcceptedMachine(symbols::SymbolHandle),
}

struct NonProviderTrustGrantCandidate<'name> {
    subject: NonProviderTrustGrant,
    kind: &'static str,
    name: &'name str,
}

pub fn reject_package_non_provider_grants(
    declarations: &TrustDeclarations,
    root_grants: &[String],
    provider_plans: &[&effects::provider_plan::ProviderPlan],
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let provider_grants = crate::resolve_selected_provider_grants(
        provider_plans,
        selected_provider_plans,
        root_grants,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    reject_package_grants_with_providers(declarations, root_grants, &provider_grants)
}

pub(crate) fn reject_package_grants_with_providers(
    declarations: &TrustDeclarations,
    root_grants: &[String],
    provider_grants: &[ResolvedSelectedProviderGrant],
) -> Result<(), Vec<Diagnostic>> {
    for grant in root_grants {
        if provider_grants
            .iter()
            .any(|provider_grant| provider_grant.selector == *grant)
        {
            continue;
        }
        match resolve_non_provider_trust_grant(declarations, grant)
            .map_err(|diagnostic| vec![diagnostic])?
        {
            NonProviderTrustGrant::AcceptedMachine(_) => {
                return Err(vec![Diagnostic::error(format!(
                    "package-aware compilation cannot admit individual accepted machine `{grant}`; package claims require complete package-level review",
                ))]);
            }
        }
    }
    Ok(())
}

pub(crate) fn resolve_non_provider_trust_grant(
    declarations: &TrustDeclarations,
    grant: &str,
) -> Result<NonProviderTrustGrant, Diagnostic> {
    let candidates = declarations
        .machines
        .iter()
        .map(|machine| NonProviderTrustGrantCandidate {
            subject: NonProviderTrustGrant::AcceptedMachine(machine.machine),
            kind: "accepted machine",
            name: machine.name.as_str(),
        })
        .collect::<Vec<_>>();
    let exact = candidates
        .iter()
        .filter(|candidate| candidate.name == grant)
        .collect::<Vec<_>>();
    match exact.as_slice() {
        [candidate] => return validate_grant_subject(grant, candidate),
        [] => {}
        _ => return Err(ambiguous_grant(grant, &exact)),
    }
    if !grant.contains("::") {
        let leaf = candidates
            .iter()
            .filter(|candidate| candidate.name.rsplit("::").next() == Some(grant))
            .collect::<Vec<_>>();
        match leaf.as_slice() {
            [candidate] => return validate_grant_subject(grant, candidate),
            [] => {}
            _ => return Err(ambiguous_grant(grant, &leaf)),
        }
    }
    Err(Diagnostic::error(format!(
        "root grant `{grant}` does not name an exact accepted machine or selected provider plan; domain and arbitrary-string trust grants are unsupported",
    )))
}

fn validate_grant_subject(
    grant: &str,
    candidate: &NonProviderTrustGrantCandidate<'_>,
) -> Result<NonProviderTrustGrant, Diagnostic> {
    let NonProviderTrustGrant::AcceptedMachine(symbol) = candidate.subject;
    if !symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "root grant `{grant}` resolves to {} `{}` with no valid exact symbol",
            candidate.kind, candidate.name,
        )));
    }
    Ok(candidate.subject)
}

fn ambiguous_grant(grant: &str, candidates: &[&NonProviderTrustGrantCandidate<'_>]) -> Diagnostic {
    let mut names = candidates
        .iter()
        .map(|candidate| format!("{} `{}`", candidate.kind, candidate.name))
        .collect::<Vec<_>>();
    names.sort();
    Diagnostic::error(format!(
        "root grant `{grant}` is ambiguous across non-provider trust subjects: {}",
        names.join(", "),
    ))
}

pub(crate) fn accepted_machine(
    declarations: &TrustDeclarations,
    symbol: symbols::SymbolHandle,
) -> Result<&AcceptedMachineDeclaration, Diagnostic> {
    let machines = declarations
        .machines
        .iter()
        .filter(|machine| machine.machine == symbol)
        .collect::<Vec<_>>();
    let [machine] = machines.as_slice() else {
        return Err(Diagnostic::error(match machines.len() {
            0 => {
                format!("granted accepted-machine symbol {symbol:?} has no exact typed definition")
            }
            count => format!(
                "granted accepted-machine symbol {symbol:?} has {count} exact typed definitions"
            ),
        }));
    };
    Ok(*machine)
}

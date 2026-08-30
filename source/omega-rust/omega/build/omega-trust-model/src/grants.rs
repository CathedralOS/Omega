use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonProviderTrustGrant {
    AcceptedMachine(psi_symbols::SymbolHandle),
}

struct NonProviderTrustGrantCandidate<'name> {
    subject: NonProviderTrustGrant,
    kind: &'static str,
    name: &'name str,
}

pub fn resolve_non_provider_trust_grant(
    typed: &TypedTrees,
    grant: &str,
) -> Result<NonProviderTrustGrant, Diagnostic> {
    let candidates = typed
        .machines()
        .iter()
        .filter(|machine| grantable_accepted_machine(typed, machine))
        .map(|machine| NonProviderTrustGrantCandidate {
            subject: NonProviderTrustGrant::AcceptedMachine(machine.symbol),
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

pub(crate) fn grantable_accepted_machine(
    typed: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> bool {
    machine.supply_mode == psi_language_semantics::MachineSupplyMode::AdmissionClaim
        && !typed.machine_specializations.iter().any(|specialization| {
            specialization.accepted_template_commitment.is_some()
                && specialization.instance == machine.symbol
                && specialization.instance != specialization.template
        })
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
    typed: &TypedTrees,
    symbol: psi_symbols::SymbolHandle,
) -> Result<&psi_typed_trees::machine::Machine, Diagnostic> {
    let machines = typed
        .machines()
        .iter()
        .filter(|machine| grantable_accepted_machine(typed, machine) && machine.symbol == symbol)
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

pub fn reject_package_non_provider_grants(
    typed: &TypedTrees,
    root_grants: &[String],
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let provider_grants = crate::resolve_selected_provider_grants(
        provider_plans,
        selected_provider_plans,
        root_grants,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    for grant in root_grants {
        if provider_grants
            .iter()
            .any(|provider_grant| provider_grant.selector == *grant)
        {
            continue;
        }
        match resolve_non_provider_trust_grant(typed, grant)
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

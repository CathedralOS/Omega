//! Accepted-machine grant selection over Psi's complete declaration roster.
//! Root package-grant policy belongs to `root_grants`; provider selectors
//! resolve independently against selected provider plans.

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

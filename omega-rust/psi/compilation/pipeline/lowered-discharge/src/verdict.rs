/*
The route's verdict on one program: the one result a whole-compilation router
takes from the lowered-form route, and the only way it should take one.

`wiki/drafts/designs/check_on_lowered_form.md` ("Route one", Build): "An
obligation the prover cannot discharge is a compile error for the program,
never a slower guarded form." Omega 00 trusts every obligation of the module
it is given and fills evidence only where a seal arrives, so a module with an
undischarged obligation must never reach it: Omega 00 would accept that
obligation with no evidence. `RouteVerdict` therefore has no variant for a
partly proved closure. A closure is either accepted with every obligation
discharged by the kernel, rejected with one diagnostic per unproved
obligation, or handed back whole to Psi 05 and 06 because the route does not
serve some construct in it (`RefusalReason::Unsupported`; the design: "falling
back to Psi 05 and 06 on any `RefusalReason::Unsupported`").

`Unsupported` wins over `Unproved`: an unproved obligation in a closure the
route cannot judge whole is the current checker's to decide. That is not a
weaker check; Psi 05 and 06 still judge the closure. Any other refusal is a
defect of the route itself (an identity that does not resolve) and is
returned as `Err`; a router must not accept the program on it.
*/

use diagnostics::Diagnostic;
use semantic_vocabulary::{MachineId, ObligationId};
use structure_elaboration::{Elaboration, SourceOrigin};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason, UnprovedCause};
use terminal_codec::ObligationEvidenceSeal;

use crate::{Discharge, discharge, refusal};

/// What the route decided for one closure.
#[derive(Debug)]
pub enum RouteVerdict {
    /// Every obligation of every machine was discharged with a certificate
    /// the kernel accepted. `seals` are the operation obligations' evidence
    /// for Omega 00.
    Accepted(Box<AcceptedClosure>),
    /// The program is rejected: one entry per obligation the prover could not
    /// discharge, in machine and obligation order. Never empty.
    Rejected(Vec<UnprovedObligation>),
    /// The route does not serve some construct or obligation of the closure,
    /// so Psi 05 and 06 decide the whole closure. The first such refusal.
    FallBack(Refusal),
}

/// A closure the route accepted.
#[derive(Debug)]
pub struct AcceptedClosure {
    pub elaboration: Elaboration,
    pub discharge: Discharge,
    pub seals: Vec<ObligationEvidenceSeal>,
}

/// One obligation the prover could not discharge, as a program error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnprovedObligation {
    pub machine: MachineId,
    pub obligation: ObligationId,
    pub kind: ObligationKind,
    /// Where the obligation arises in the source, from elaboration's own
    /// source correspondence.
    pub site: SourceOrigin,
    pub cause: UnprovedCause,
    /// The prover's or kernel's own account. Diagnostic text only.
    pub detail: String,
}

impl UnprovedObligation {
    /// The program diagnostic. The message opens with a stable part (the
    /// obligation kind and the cause) that corpus expectations can match,
    /// followed by the prover's detail.
    pub fn diagnostic(&self) -> Diagnostic {
        let message = format!(
            "unproved obligation: {}: {} ({})",
            describe_kind(self.kind),
            self.cause.describe(),
            self.detail
        );
        let diagnostic = Diagnostic::error(message);
        match self.site {
            SourceOrigin::Authored(span) => diagnostic.with_source_span(span),
            SourceOrigin::Unpositioned(_) => diagnostic,
        }
    }
}

/// The obligation kind as a diagnostic names it. Clause positions count from
/// one, as the source does.
fn describe_kind(kind: ObligationKind) -> String {
    match kind {
        ObligationKind::OperationSafety { .. } => String::from("operation safety"),
        ObligationKind::CallRequirement { clause, .. } => {
            format!(
                "requires clause {} of the called machine",
                u64::from(clause) + 1
            )
        }
        ObligationKind::NormalGuarantee { clause, .. } => {
            format!(
                "ensures clause {} on a normal return",
                u64::from(clause) + 1
            )
        }
        ObligationKind::PublishedGuarantee { clause } => {
            format!("ensures clause {}", u64::from(clause) + 1)
        }
        ObligationKind::ArrivalInvariant { .. } => String::from("block invariant"),
        ObligationKind::CrashCoverage { .. } => String::from("crash coverage"),
        ObligationKind::WellFoundedness { .. } | ObligationKind::RankComparison { .. } => {
            String::from("termination")
        }
        ObligationKind::Claim { .. }
        | ObligationKind::Loan { .. }
        | ObligationKind::RestorationWindow { .. }
        | ObligationKind::DomainDebt { .. }
        | ObligationKind::EstablishmentDebt { .. } => String::from("conserved obligation"),
    }
}

/// Discharges an elaborated closure and returns the route's verdict on it.
/// It takes elaboration's result as is, so an elaboration refused as
/// `Unsupported` falls back like an undecided obligation does.
pub fn judge(elaborated: Checked<Elaboration>) -> Checked<RouteVerdict> {
    let mut elaboration = match elaborated {
        Ok(elaboration) => elaboration,
        Err(refused) => return fall_back_or_defect(refused),
    };
    let discharged = match discharge(&mut elaboration) {
        Ok(discharged) => discharged,
        Err(refused) => return fall_back_or_defect(refused),
    };
    let mut unproved = Vec::new();
    for (machine, result) in discharged.machines.iter() {
        let tables = terminal_check_tables::row(
            &elaboration.tables.machines,
            machine,
            machine,
            Construct::Machine,
        )?;
        let sources = elaboration.sources.get(machine).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Machine,
                "machine has no source correspondence",
            )
        })?;
        if result.verdicts.len() != tables.obligations.len() {
            return Err(refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Machine,
                "verdicts do not cover the machine's obligations one to one",
            ));
        }
        for (obligation, verdict) in result.verdicts.iter() {
            let Err(refused) = verdict else {
                continue;
            };
            let RefusalReason::Unproved(cause) = refused.reason else {
                return fall_back_or_defect(refused.clone());
            };
            let concerns = Construct::Obligation(obligation);
            let row =
                terminal_check_tables::row(&tables.obligations, obligation, machine, concerns)?;
            let site =
                *terminal_check_tables::row(&sources.obligations, obligation, machine, concerns)?;
            unproved.push(UnprovedObligation {
                machine,
                obligation,
                kind: row.kind,
                site,
                cause,
                detail: refused.detail.clone(),
            });
        }
    }
    if !unproved.is_empty() {
        return Ok(RouteVerdict::Rejected(without_restated_clauses(unproved)));
    }
    let seals = match discharged.operation_seals(&elaboration) {
        Ok(seals) => seals,
        Err(refused) => return fall_back_or_defect(refused),
    };
    Ok(RouteVerdict::Accepted(Box::new(AcceptedClosure {
        elaboration,
        discharge: discharged,
        seals,
    })))
}

fn fall_back_or_defect(refused: Refusal) -> Checked<RouteVerdict> {
    match refused.reason {
        RefusalReason::Unsupported => Ok(RouteVerdict::FallBack(refused)),
        RefusalReason::Unproved(_)
        | RefusalReason::NotConserved
        | RefusalReason::Conflict
        | RefusalReason::ForeignIdentity => Err(refused),
    }
}

/// A published `ensures` clause fails because a return's own guarantee
/// failed, and that return is already reported with its own site. The
/// clause is kept only when no return of it is reported.
fn without_restated_clauses(unproved: Vec<UnprovedObligation>) -> Vec<UnprovedObligation> {
    let restated = |candidate: &UnprovedObligation| {
        matches!(candidate.kind, ObligationKind::PublishedGuarantee { .. })
            && unproved.iter().any(|exit| {
                exit.machine == candidate.machine
                    && matches!(
                        exit.kind,
                        ObligationKind::NormalGuarantee { published, .. }
                            if published == candidate.obligation
                    )
            })
    };
    let keep: Vec<bool> = unproved.iter().map(|entry| !restated(entry)).collect();
    unproved
        .into_iter()
        .zip(keep)
        .filter_map(|(entry, keep)| keep.then_some(entry))
        .collect()
}

//! Certificates the checker accepted although the mathematical kernel did not
//! judge them.
//!
//! The bounded checker verifies a certificate on its own rules and then
//! translates it into the mathematical kernel, which judges it again. For a
//! few shapes the translation does not exist yet ("a carrier bound of an
//! integer whose width is a parameter", for one). The kernel then records
//! `MathematicalCoreDecision::Refused`, and the certificate is accepted on
//! the bounded rule alone. Identity goals are refused in that case
//! (`lib.rs`, `verify_certificate`); every other goal is accepted. This
//! module counts those acceptances, per obligation kind and shape, so a
//! reporter can say how much of an accepted program rests on the bounded
//! checker without the kernel. It decides nothing.

use crate::{Discharge, Discharged};
use proof_admission::{AcceptedFactRoute, MathematicalCoreDecision};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::program::Elaboration;

/// One group of certificates the kernel did not judge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unjudged {
    /// What the certificate proved, in words.
    pub kind: &'static str,
    /// Why the kernel could not judge it: the shape the translation lacks.
    pub shape: &'static str,
    pub count: u32,
}

/// Adds the certificates of one accepted part to `groups`.
pub(crate) fn count(elaboration: &Elaboration, discharge: &Discharge, groups: &mut Vec<Unjudged>) {
    for (machine, judged) in discharge.machines.iter() {
        let Some(tables) = elaboration.tables.machines.get(machine) else {
            continue;
        };
        for (obligation, verdict) in judged.verdicts.iter() {
            let Ok(Discharged::Certificate(evidence)) = verdict else {
                continue;
            };
            let AcceptedFactRoute::CertificateDerived { acceptance, .. } = &evidence.fact.route
            else {
                continue;
            };
            let MathematicalCoreDecision::Refused(shape) = acceptance.mathematical_core else {
                continue;
            };
            let Some(row) = tables.obligations.get(obligation) else {
                continue;
            };
            let kind = kind(&row.kind);
            match groups
                .iter_mut()
                .find(|group| group.kind == kind && group.shape == shape)
            {
                Some(group) => group.count += 1,
                None => groups.push(Unjudged {
                    kind,
                    shape,
                    count: 1,
                }),
            }
        }
    }
}

/// The note a reporter shows for `groups`, largest first. `None` when the
/// kernel judged every accepted certificate.
pub(crate) fn note(mut groups: Vec<Unjudged>) -> Option<diagnostics::Diagnostic> {
    let total: u32 = groups.iter().map(|group| group.count).sum();
    if total == 0 {
        return None;
    }
    groups.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then(left.kind.cmp(right.kind))
            .then(left.shape.cmp(right.shape))
    });
    let listed = groups
        .iter()
        .map(|group| format!("{} {} ({})", group.count, group.kind, group.shape))
        .collect::<Vec<_>>()
        .join("; ");
    Some(diagnostics::Diagnostic::note(format!(
        "{total} accepted certificate(s) rest on the bounded checker alone, because the \
         mathematical kernel cannot judge their shape yet: {listed}"
    )))
}

/// What an obligation of `kind` asks, in words.
fn kind(kind: &ObligationKind) -> &'static str {
    match kind {
        ObligationKind::Claim { .. } => "for an owned claim",
        ObligationKind::Loan { .. } => "for a loan",
        ObligationKind::RestorationWindow { .. } => "for restoring borrowed storage",
        ObligationKind::DomainDebt { .. } => "for a domain a write broke",
        ObligationKind::EstablishmentDebt { .. } => "for establishing storage",
        ObligationKind::IntegerWidthPositive { .. } => "for a positive integer width",
        ObligationKind::TheoremRequirement { .. } => "for a theorem's premise",
        ObligationKind::CallRequirement { .. } => "for a call's requires",
        ObligationKind::OperationSafety { .. } => "for an operation's safety",
        ObligationKind::DataDomain { .. } => "for a data type's requires",
        ObligationKind::ArrivalInvariant { .. } => "for an invariant at a join",
        ObligationKind::NormalGuarantee { .. } => "for an ensures at a return",
        ObligationKind::PublishedGuarantee { .. } => "for a published ensures",
        ObligationKind::CrashCoverage { .. } => "for a crash route",
        ObligationKind::WellFoundedness { .. } => "for a ranking's well-foundedness",
        ObligationKind::RankComparison { .. } => "for a rank comparison",
    }
}

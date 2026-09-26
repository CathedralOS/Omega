//! Validation of retained disposition events, certificates and call uses
//! against independent replay.

use crate::checks::borrows::resources::reborrow_drafts::{
    CheckedReborrowContainmentCertificateDraft, CheckedReborrowDispositionEventDraft,
    CheckedReborrowLoanResourceDraft, CheckedReborrowRestoredCallUseCertificateDraft,
    ResourceHandles,
};
use checked_trees::{BorrowFacts, CheckedDirectBorrowLoanResource};
use diagnostics::Diagnostic;

pub(crate) fn validate_retained_disposition_events(
    borrow: &BorrowFacts,
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
    expected: &[CheckedReborrowDispositionEventDraft],
) -> Result<(), Vec<Diagnostic>> {
    let handles = ResourceHandles {
        direct: borrow
            .direct_loan_resources
            .iter()
            .map(|(handle, _)| handle)
            .collect(),
        reborrows: borrow
            .reborrow_loan_resources
            .iter()
            .map(|(handle, _)| handle)
            .collect(),
    };
    if handles.direct.len() != direct.len() || handles.reborrows.len() != reborrows.len() {
        return Err(reborrow_disposition_drift());
    }
    let retained = borrow
        .reborrow_disposition_events
        .iter()
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    if retained.len() != expected.len()
        || retained
            .into_iter()
            .zip(expected)
            .any(|(retained, expected)| retained != &expected.close(borrow, &handles))
    {
        return Err(reborrow_disposition_drift());
    }
    Ok(())
}

pub(crate) fn validate_retained_containment_certificates(
    borrow: &BorrowFacts,
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
    expected: &[CheckedReborrowContainmentCertificateDraft],
) -> Result<(), Vec<Diagnostic>> {
    let handles = ResourceHandles {
        direct: borrow
            .direct_loan_resources
            .iter()
            .map(|(handle, _)| handle)
            .collect(),
        reborrows: borrow
            .reborrow_loan_resources
            .iter()
            .map(|(handle, _)| handle)
            .collect(),
    };
    if handles.direct.len() != direct.len() || handles.reborrows.len() != reborrows.len() {
        return Err(reborrow_containment_drift());
    }
    let retained = borrow
        .reborrow_containment_certificates
        .iter()
        .map(|(_, certificate)| certificate)
        .collect::<Vec<_>>();
    if retained.len() != expected.len()
        || retained
            .into_iter()
            .zip(expected)
            .any(|(retained, expected)| retained != &expected.close(&handles))
    {
        return Err(reborrow_containment_drift());
    }
    Ok(())
}

pub(crate) fn validate_retained_restored_call_uses(
    borrow: &BorrowFacts,
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
    dispositions: &[CheckedReborrowDispositionEventDraft],
    containments: &[CheckedReborrowContainmentCertificateDraft],
    expected: &[CheckedReborrowRestoredCallUseCertificateDraft],
) -> Result<(), Vec<Diagnostic>> {
    let resources = ResourceHandles {
        direct: borrow
            .direct_loan_resources
            .iter()
            .map(|(handle, _)| handle)
            .collect(),
        reborrows: borrow
            .reborrow_loan_resources
            .iter()
            .map(|(handle, _)| handle)
            .collect(),
    };
    let disposition_handles = borrow
        .reborrow_disposition_events
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    let containment_handles = borrow
        .reborrow_containment_certificates
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    if resources.direct.len() != direct.len()
        || resources.reborrows.len() != reborrows.len()
        || disposition_handles.len() != dispositions.len()
        || containment_handles.len() != containments.len()
    {
        return Err(reborrow_restored_call_use_drift());
    }
    let retained = borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .map(|(_, certificate)| certificate)
        .collect::<Vec<_>>();
    if retained.len() != expected.len()
        || retained
            .into_iter()
            .zip(expected)
            .any(|(retained, expected)| {
                retained != &expected.close(&resources, &disposition_handles, &containment_handles)
            })
    {
        return Err(reborrow_restored_call_use_drift());
    }
    Ok(())
}

pub(crate) fn reborrow_disposition_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked reborrow resource-lifecycle disposition drifted from semantic-phase replay",
    )]
}

pub(crate) fn reborrow_containment_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked reborrow suspension/freeze-containment evidence drifted from exact lifecycle replay",
    )]
}

pub(crate) fn reborrow_restored_call_use_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked reborrow restored mutating-call use drifted from exact lifecycle and call replay",
    )]
}

pub(crate) fn reborrow_resource_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked direct-reborrow resource closure drifted from independent topological replay",
    )]
}

pub(crate) fn invalid_reborrow_attenuation_diagnostic(
    parent: &checked_trees::BorrowAccessKind,
    child: &checked_trees::BorrowAccessKind,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot derive {} reborrow authority from an exact {} parent loan; allowed direct reborrow access pairs are Read->Read, Mutable->Read, Mutable->Mutable, Mutable->WriteOnly, and WriteOnly->WriteOnly",
        borrow_access_name(child),
        borrow_access_name(parent),
    ))
}

fn borrow_access_name(access: &checked_trees::BorrowAccessKind) -> &'static str {
    match access {
        checked_trees::BorrowAccessKind::Read => "Read",
        checked_trees::BorrowAccessKind::Mutable => "Mutable",
        checked_trees::BorrowAccessKind::WriteOnly => "WriteOnly",
    }
}

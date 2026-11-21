//! A build report reads a prepared package's resource receipt without linking
//! preparation. It lists the admission (the root's grant and the checked
//! ceiling) beside the frozen values of the constants the build selected.
//!
//! The report does not identify a result. Under the evaluation spec a result
//! key covers the normalized implementation closure, the arguments, the
//! selected providers, the target applications and the evaluator version. A
//! prepared package publishes none of those as a key, so two different
//! implementations that freeze the same value give equal value lists. The
//! receipt records a ceiling, not work used, so the report records no usage.
use omega_prepared::{CertifiedResources, PreparationGrant, PreparationResourceReceipt, PreparedTrees, PreparedValue};
use omega_resolved::DeclarationId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildReport {
    /// Each selected constant, by its exact declaration identity, and its
    /// frozen value, in request order.
    pub values: Vec<(DeclarationId, PreparedValue)>,
    pub admission: Admission,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Admission {
    /// Preparation ran without a grant. This is different from a zero ceiling.
    Unsponsored,
    /// The root's grant and the checked ceiling it admitted, kept exactly.
    Certified { grant: PreparationGrant, ceiling: CertifiedResources },
}

/// The build that requested preparation knows whether it supplied a checked
/// bound and for which subject. The report describes only that request.
#[derive(Clone, Copy, Debug)]
pub enum Requested<'a> {
    Unsponsored,
    CheckedSubject(&'a [u8]),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReportError {
    /// The package froze no value for this exact declaration.
    MissingValue(DeclarationId),
    /// The package carries a receipt, but the build supplied no bound.
    UnexpectedReceipt,
    /// The build supplied a bound, but the package carries no receipt.
    MissingReceipt,
    /// The receipt's ceiling was checked for another subject.
    WrongSubject { requested: Vec<u8>, found: Vec<u8> },
}

/// The caller names each constant by its declaration identity, which includes
/// its package. Two packages can each declare `VALUE`; a name alone would pick
/// one of them.
pub fn build_report(
    prepared: &PreparedTrees<'_>,
    declarations: &[&DeclarationId],
    requested: Requested<'_>,
) -> Result<BuildReport, ReportError> {
    let admission = match (requested, prepared.resource_receipt()) {
        (Requested::Unsponsored, None) => Admission::Unsponsored,
        (Requested::Unsponsored, Some(_)) => return Err(ReportError::UnexpectedReceipt),
        (Requested::CheckedSubject(_), None) => return Err(ReportError::MissingReceipt),
        (Requested::CheckedSubject(subject), Some(receipt)) => certified(receipt, subject)?,
    };
    let values = declarations
        .iter()
        .map(|&declaration| {
            prepared
                .constant(declaration)
                .map(|value| (declaration.clone(), value.clone()))
                .ok_or_else(|| ReportError::MissingValue(declaration.clone()))
        })
        .collect::<Result<_, _>>()?;
    Ok(BuildReport { values, admission })
}

// Name every field so that a new receipt field, such as measured usage, must be
// placed deliberately rather than silently copied into the ceiling.
fn certified(
    receipt: &PreparationResourceReceipt,
    requested: &[u8],
) -> Result<Admission, ReportError> {
    let PreparationResourceReceipt {
        grant: PreparationGrant { work_budget, peak_live_cells },
        bound: CertifiedResources {
            checked_subject,
            work_schedule,
            maximum_work,
            maximum_live_cells,
        },
    } = receipt;
    if checked_subject.as_ref() != requested {
        return Err(ReportError::WrongSubject {
            requested: requested.to_vec(),
            found: checked_subject.to_vec(),
        });
    }
    Ok(Admission::Certified {
        grant: PreparationGrant {
            work_budget: *work_budget,
            peak_live_cells: *peak_live_cells,
        },
        ceiling: CertifiedResources {
            checked_subject: checked_subject.clone(),
            work_schedule: *work_schedule,
            maximum_work: *maximum_work,
            maximum_live_cells: *maximum_live_cells,
        },
    })
}

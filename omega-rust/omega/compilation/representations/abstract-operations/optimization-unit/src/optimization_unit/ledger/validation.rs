//! Independent provenance and work-custody validation.

use super::{BTreeSet, InvalidPsiTransformationLedger, ProvenanceRewrite};
pub(super) fn validate_provenance(
    rows: &[ProvenanceRewrite],
) -> Result<(), InvalidPsiTransformationLedger> {
    if rows.is_empty() || rows.iter().any(|row| row.sources.is_empty()) {
        return Err(InvalidPsiTransformationLedger::EmptyProvenance);
    }
    if rows
        .windows(2)
        .any(|pair| pair[0].canonical_sort_key() >= pair[1].canonical_sort_key())
    {
        return Err(InvalidPsiTransformationLedger::NonCanonicalProvenance);
    }
    for group in rows.chunk_by(|left, right| left.input == right.input) {
        if group.len() > 1
            && (group.iter().any(|row| !row.disposition.is_realized())
                || group
                    .iter()
                    .skip(1)
                    .any(|row| row.sources != group[0].sources || row.work != group[0].work))
        {
            return Err(InvalidPsiTransformationLedger::NonCanonicalProvenance);
        }
    }
    for row in rows {
        let sources = row.sources.iter().copied().collect::<BTreeSet<_>>();
        if sources.len() != row.sources.len() {
            return Err(InvalidPsiTransformationLedger::NonCanonicalProvenance);
        }
        if row.input.machine() != row.disposition.site().machine() {
            return Err(InvalidPsiTransformationLedger::NonCanonicalProvenance);
        }
        let work = row
            .work
            .iter()
            .map(|settlement| settlement.site)
            .collect::<BTreeSet<_>>();
        if work.len() != row.work.len() || work != sources {
            return Err(InvalidPsiTransformationLedger::WorkProvenanceMismatch);
        }
        if row.work.iter().any(|settlement| settlement.units == 0) {
            return Err(InvalidPsiTransformationLedger::ZeroWorkSettlement);
        }
    }
    Ok(())
}

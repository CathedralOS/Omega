//! Independent provenance and fuel-custody validation.

use super::*;

pub(super) fn validate_provenance(
    rows: &[ProvenanceRewrite],
) -> Result<(), InvalidPsiTransformationLedger> {
    if rows.is_empty() || rows.iter().any(|row| row.sources.is_empty()) {
        return Err(InvalidPsiTransformationLedger::EmptyProvenance);
    }
    if rows.windows(2).any(|pair| {
        let left = (
            pair[0].input,
            pair[0].disposition.canonical_tag(),
            pair[0].disposition.site(),
        );
        let right = (
            pair[1].input,
            pair[1].disposition.canonical_tag(),
            pair[1].disposition.site(),
        );
        left >= right
    }) {
        return Err(InvalidPsiTransformationLedger::NonCanonicalProvenance);
    }
    for group in rows.chunk_by(|left, right| left.input == right.input) {
        if group.len() > 1
            && (group.iter().any(|row| !row.disposition.is_realized())
                || group
                    .iter()
                    .skip(1)
                    .any(|row| row.sources != group[0].sources || row.fuel != group[0].fuel))
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
        let fuel = row
            .fuel
            .iter()
            .map(|settlement| settlement.site)
            .collect::<BTreeSet<_>>();
        if fuel.len() != row.fuel.len() || fuel != sources {
            return Err(InvalidPsiTransformationLedger::FuelProvenanceMismatch);
        }
        if row.fuel.iter().any(|settlement| settlement.units == 0) {
            return Err(InvalidPsiTransformationLedger::ZeroFuelSettlement);
        }
    }
    Ok(())
}

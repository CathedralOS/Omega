//! Independent receiving of retained induction roots against trusted source
//! contracts. The expected roster is supplied by the receiving authority; an
//! untrusted module cannot authorize its own theorem or assumption positions.

use std::collections::BTreeSet;

use proof_admission::{Budget, certificate_assumption_closure, verify_mathematical_certificates};
use terminal_psi::{TerminalIndexedTheorem, TerminalModule};

use crate::{
    MathematicalProofActualError as Error, decode_mathematical_certificate,
    mathematical_carrier_key,
};

pub fn verify_indexed_theorem_roots(
    module: &TerminalModule,
    expected: &TerminalModule,
    mut grants: impl FnMut(&TerminalIndexedTheorem) -> Result<BTreeSet<u32>, Error>,
    budget: &mut Budget,
) -> Result<(), Error> {
    if module.indexed_theorems.len() != expected.indexed_theorems.len() {
        return Err(Error::KernelCarrierMismatch);
    }
    let mut matched = vec![false; expected.indexed_theorems.len()];
    let mut pending = Vec::new();
    for actual in &module.indexed_theorems {
        if actual.evidence_source_position as usize >= actual.source_contract.parameter_types.len()
            || module
                .identities
                .text(actual.type_identity)
                .is_none_or(str::is_empty)
            || module
                .identities
                .text(actual.source_contract.semantic_declaration)
                .is_none_or(str::is_empty)
            || module
                .identities
                .text(actual.source_contract.callable)
                .is_none_or(str::is_empty)
        {
            return Err(Error::SourceCarrierMismatch);
        }
        let candidates = expected
            .indexed_theorems
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                actual.source_contract.same_identity_as(
                    &module.identities,
                    &row.source_contract,
                    &expected.identities,
                ) && actual.ensure_ordinal == row.ensure_ordinal
                    && actual.evidence_source_position == row.evidence_source_position
                    && module
                        .identities
                        .text(actual.type_identity)
                        .filter(|text| !text.is_empty())
                        == expected
                            .identities
                            .text(row.type_identity)
                            .filter(|text| !text.is_empty())
            })
            .collect::<Vec<_>>();
        let [(position, trusted)] = candidates.as_slice() else {
            return Err(Error::SourceCarrierMismatch);
        };
        if matched[*position] {
            return Err(Error::SourceCarrierMismatch);
        }
        matched[*position] = true;
        let decoded = decode_mathematical_certificate(&actual.certificate).map_err(Error::Wire)?;
        let expected_judgment =
            decode_mathematical_certificate(&trusted.certificate).map_err(Error::Wire)?;
        if decoded.certificate.level_arity != 0 || !decoded.certificate.context.is_empty() {
            return Err(Error::OpenContext);
        }
        if mathematical_carrier_key(&decoded.arena, &decoded.certificate).map_err(Error::Wire)?
            != mathematical_carrier_key(&expected_judgment.arena, &expected_judgment.certificate)
                .map_err(Error::Wire)?
        {
            return Err(Error::KernelCarrierMismatch);
        }
        pending.push((decoded, actual, grants(trusted)?));
    }
    let judgments = pending
        .iter()
        .map(|(decoded, _, _)| (&decoded.arena, &decoded.certificate))
        .collect::<Vec<_>>();
    verify_mathematical_certificates(&judgments, budget).map_err(Error::KernelBatch)?;
    for (decoded, actual, granted) in &pending {
        let closure = certificate_assumption_closure(&decoded.arena, &decoded.certificate);
        if closure.iter().copied().collect::<Vec<_>>() != actual.assumption_closure {
            return Err(Error::KernelCarrierMismatch);
        }
        let ungranted = closure
            .difference(granted)
            .copied()
            .collect::<BTreeSet<_>>();
        if !ungranted.is_empty() {
            return Err(Error::UngrantedAssumptions(ungranted));
        }
    }
    Ok(())
}
